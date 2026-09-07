// Downloads — the queue.
//
// THE ONE RULE OF THIS SURFACE: a row's right-hand column always answers the
// same question in the same place — "what is this row doing?" A transfer in
// flight answers with a number (47%) and a rate line; every other state answers
// with one glyph and one word. Nothing about the row's geometry changes when
// the answer changes, so a row going downloading → done recolours and reletters
// but never moves.
//
// Everything a user *looks up* rather than *glances at* (duration, size,
// container format, uploader) sits on the quiet second line under the title,
// in one place, in every state — so the loud right column is free to be loud.
//
// NOTE ON GROUPING: DownloadTask carries no timestamp (see shared/types.ts) —
// there is no createdAt/completedAt to group by day with, and the store keeps
// insertion order only. Rather than invent a date, the list's spine is status:
// In progress → Needs attention → Completed.
import { memo, useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from 'react';
import { AnimatePresence, motion, Reorder, useDragControls, useReducedMotion } from 'framer-motion';
import {
  Download, Close, Cancel, Retry, OpenFolder, Check, Alert, Clock,
  CheckboxChecked, CheckboxEmpty, Remove, CheckboxMixed, Track, Pause, Play,
  Loading, DragHandle, MoveToFront,
} from '@/components/ui/icons';
import { useAppStore } from '@/store';
import { Button, IconButton, ProgressBar } from '@/components/ui';
import { PLATFORMS } from '@/constants';
import { formatBytes, formatDuration } from '@/lib/format';
import { revealFile } from '@/lib/reveal';
import type { DownloadStatus, DownloadTask, SourcePlatform } from '@shared/types';

// ── formatters ────────────────────────────────────────────────────────────────
// Byte/duration formatting lives once in src/lib/format.ts (hour-aware).

const fmtSpeed = (bps: number): string => bps > 0 ? `${formatBytes(bps)}/s` : '';

function fmtEta(secs: number): string {
  if (secs <= 0 || !isFinite(secs)) return '';
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return m >= 60
    ? `${Math.floor(m / 60)}h ${m % 60}m`
    : `${m}:${String(s).padStart(2, '0')}`;
}

/** progress.filename is a full path — rows show just the file name. */
const baseName = (p: string): string => p.split(/[\\/]/).pop() ?? '';

/** Tasks started via hotkey/palette/history may lack a thumbnail — YouTube's
 *  is derivable from the video id, so those rows still render art. */
function thumbFor(task: DownloadTask): string {
  if (task.thumbnailUrl) return task.thumbnailUrl;
  if (task.source === 'youtube') {
    const m = task.url.match(/(?:v=|youtu\.be\/|\/shorts\/)([\w-]{11})/);
    if (m) return `https://i.ytimg.com/vi/${m[1]}/mqdefault.jpg`;
  }
  return '';
}

// ── status config ─────────────────────────────────────────────────────────────

/** One glyph + one word per state, rendered in the row's state column. The
 *  glyph is what reads at a glance (shape + tone), the word is what confirms
 *  it — `downloading` is absent on purpose: a live transfer answers with its
 *  percentage instead, which is a stronger signal than any word.
 *
 *  Shapes are chosen to be told apart without reading: pause bars, a retry
 *  arc, a warning disc, a check, a clock. Colour is the second channel, never
 *  the only one. */
const STATE: Partial<Record<DownloadStatus, { icon: typeof Check; label: string; tone: string }>> = {
  queued:        { icon: Clock,   label: 'Queued',     tone: 'text-text-muted' },
  fetching_info: { icon: Loading, label: 'Fetching',   tone: 'text-text-secondary' },
  converting:    { icon: Loading, label: 'Converting', tone: 'text-warning' },
  embedding:     { icon: Loading, label: 'Tagging',    tone: 'text-warning' },
  paused:        { icon: Pause,   label: 'Paused',     tone: 'text-warning' },
  retrying:      { icon: Retry,   label: 'Retrying',   tone: 'text-warning' },
  done:          { icon: Check,   label: 'Done',       tone: 'text-success' },
  failed:        { icon: Alert,   label: 'Failed',     tone: 'text-error' },
  cancelled:     { icon: Close,   label: 'Cancelled',  tone: 'text-text-muted' },
};

// ── platform maps ─────────────────────────────────────────────────────────────

// youtube/spotify/soundcloud labels derive from the shared PLATFORMS constant;
// the short-form platforms and the direct/unknown fallbacks have no PLATFORMS
// entry (they're all grouped under one 'reels' nav tab), so those stay local.
const PLAT_LABEL: Record<SourcePlatform, string> = {
  youtube: PLATFORMS.find(p => p.key === 'youtube')!.label,
  spotify: PLATFORMS.find(p => p.key === 'spotify')!.label,
  soundcloud: PLATFORMS.find(p => p.key === 'soundcloud')!.label,
  instagram: 'Reels',  tiktok: 'TikTok',    facebook: 'Facebook',
  direct: 'Direct',    generic: 'Link',      unknown: 'Unknown',
};

const PLAT_TEXT: Record<SourcePlatform, string> = {
  youtube: 'text-youtube',      spotify: 'text-spotify',  soundcloud: 'text-soundcloud',
  instagram: 'text-reels',      tiktok: 'text-reels',     facebook: 'text-reels',
  direct: 'text-text-secondary', generic: 'text-text-secondary', unknown: 'text-text-secondary',
};

// ── shared constants ──────────────────────────────────────────────────────────

// ponytail: Set for O(1) membership tests across filters + row render
const ACTIVE = new Set<DownloadStatus>([
  'queued', 'fetching_info', 'downloading', 'converting', 'embedding', 'paused', 'retrying',
]);
/** In-flight family: the only states that own a progress trace. A queued task
 *  has nothing to show yet; a finished one has nothing left to show. */
const IN_FLIGHT = new Set<DownloadStatus>([
  'fetching_info', 'downloading', 'converting', 'embedding', 'retrying', 'paused',
]);
/** Actually moving right now — the block that owns the loud %-columns. Paused and
 *  queued rows are in flight / waiting but are not transferring, and with several
 *  slots running at once that distinction is what keeps the list legible. */
const LIVE = new Set<DownloadStatus>([
  'fetching_info', 'downloading', 'converting', 'embedding', 'retrying',
]);
/** States where pausing is offered: mid-conversion kills waste finished work. */
const PAUSABLE = new Set<DownloadStatus>(['queued', 'fetching_info', 'downloading']);
const TERMINAL = new Set<DownloadStatus>(['done', 'failed', 'cancelled']);

/** Every row is exactly this tall, in every state. The old list grew live rows
 *  by 14px to shout "this is happening now" — which meant the whole list below
 *  a row shifted the moment it finished. Liveness is carried by the progress
 *  trace and the percentage instead, both of which cost no vertical space. */
const ROW = 'h-16 px-4 gap-3';
/** Column widths shared by rows and by the empty states, so a queue with no
 *  rows still reads on the same grid as one with fifty. */
/** The drag handle's cell. Present as an empty spacer in EVERY row of every
 *  group (and in the ghost/empty rows) so the columns stay aligned queue-wide —
 *  only queued rows ever put a control in it. */
const COL_HANDLE = 'w-4 shrink-0';
const COL_CHECK = 'w-5 shrink-0';
const COL_ART = 'w-10 h-10 shrink-0';
const COL_STATE = 'w-[128px] shrink-0';
// Three actions on a queued row (Do this next / Pause / Cancel) — widened in
// EVERY state, ghosts included, so the state column's right edge never moves.
const COL_ACTIONS = 'w-[88px] shrink-0';

// ── filter ────────────────────────────────────────────────────────────────────

type Filter = 'all' | 'active' | 'done' | 'failed';

const FILTERS: { key: Filter; label: string }[] = [
  { key: 'all',    label: 'All' },
  { key: 'active', label: 'Active' },
  { key: 'done',   label: 'Done' },
  { key: 'failed', label: 'Failed' },
];

function passes(task: DownloadTask, f: Filter): boolean {
  const s = task.progress.status;
  if (f === 'active') return ACTIVE.has(s);
  if (f === 'done')   return s === 'done';
  if (f === 'failed') return s === 'failed' || s === 'cancelled';
  return true;
}

// ── QueueCard ─────────────────────────────────────────────────────────────────

interface CardProps {
  task: DownloadTask;
  selected: boolean;
  onToggle: () => void;
  onCancel: () => void;
  onPause: () => void;
  onResume: () => void;
  onRetry: () => void;
  onOpen: () => void;
  onRemove: () => void;
  /** Row lives inside the queued tail's Reorder.Group, so it wraps in a
   *  Reorder.Item instead of a plain motion.li (that is what carries `layout`). */
  reorderable?: boolean;
  /** >=2 rows are queued, so the handle is a real control, never a dead grabber. */
  draggable?: boolean;
  /** A keyboard grab is held on this row. */
  grabbed?: boolean;
  /** One-shot landing tint after a "Do this next" jump. */
  pulse?: boolean;
  onReorderKey?: (e: ReactKeyboardEvent<HTMLButtonElement>) => void;
  onReorderBlur?: () => void;
  /** Pointer drag ended — commit the new order to the engine. */
  onDragCommit?: () => void;
  /** Absent when the row is already first in the queued tail (or isn't queued). */
  onPromote?: () => void;
}

// memo: progress events tick several times per second while downloading —
// only the row whose task object changed should re-render.
export const QueueCard = memo(function QueueCard({
  task, selected, onToggle, onCancel, onPause, onResume, onRetry, onOpen, onRemove,
  reorderable = false, draggable = false, grabbed = false, pulse = false,
  onReorderKey, onReorderBlur, onDragCommit, onPromote,
}: CardProps) {
  const reduced = useReducedMotion();
  // Created unconditionally (hooks rule); only the Reorder.Item wrapper reads it.
  const controls = useDragControls();
  const [dragging, setDragging] = useState(false);
  const { progress, source, format, title, uploader } = task;
  const { status, percent, speed, eta, downloaded, total } = progress;
  const isPaused = status === 'paused';
  const isDone = status === 'done';
  const isFailed = status === 'failed' || status === 'cancelled';
  const isActive = ACTIVE.has(status);
  const inFlight = IN_FLIGHT.has(status);
  const isTransferring = status === 'downloading';
  const pct = Math.max(0, Math.min(100, isDone ? 100 : percent));
  const state = STATE[status];
  const label = title || baseName(progress.filename) || 'Untitled';

  // The look-up facts, one line, one order, every state. Empty parts drop out
  // rather than render a placeholder — an unknown size shouldn't cost ink.
  const bytes = total || downloaded;
  const specs = [
    task.isPlaylist && progress.playlistTotal > 0 ? `${progress.playlistIndex}/${progress.playlistTotal}` : '',
    formatDuration(task.duration),
    bytes > 0 ? formatBytes(bytes) : '',
    format,
  ].filter(Boolean).join(' · ');

  // Rate line under the percentage. Both halves are optional (yt-dlp reports
  // neither in the first second), and the line keeps its slot either way so the
  // percentage above it never slides.
  const rate = [fmtSpeed(speed), fmtEta(eta)].filter(Boolean).join(' · ') || '–';

  // One class string, both wrappers. A lifted row rises out of the list (surface
  // background + shadow) so the gap it left reads as its destination.
  const rowClass = `group relative flex items-center transition-colors ${ROW} ${
    dragging ? 'shadow-md bg-bg-surface z-10'
    : grabbed || selected ? 'bg-accent-soft'
    : status === 'failed' ? 'bg-error/[0.05] hover:bg-error/[0.09]'
    : 'hover:bg-bg-hover'
  }`;

  const body = (
    <>
      {/* Drag handle — an empty cell on every row that cannot move, so the six
          columns to its right sit on the same grid in every group and state. */}
      <div className={`grid place-items-center ${COL_HANDLE}`}>
        {draggable && (
          <button
            type="button"
            aria-label={`Reorder ${label}`}
            aria-pressed={grabbed}
            onPointerDown={(e) => controls.start(e)}
            onKeyDown={onReorderKey}
            onBlur={onReorderBlur}
            className={`no-drag focus-visible:focus-ring grid place-items-center w-4 h-7 rounded cursor-grab active:cursor-grabbing transition-opacity duration-150 motion-reduce:transition-none ${
              grabbed
                ? 'opacity-100 text-accent'
                : 'opacity-0 text-text-muted hover:text-text-secondary focus-visible:opacity-100 group-hover:opacity-100 group-focus-within:opacity-100'
            }`}
          >
            <DragHandle size={14} />
          </button>
        )}
      </div>

      {/* checkbox — column-aligns with the select-all in the strip above */}
      <button
        onClick={onToggle}
        aria-label={selected ? 'Deselect' : 'Select'}
        aria-pressed={selected}
        className={`no-drag grid place-items-center text-text-muted hover:text-accent transition-colors ${COL_CHECK}`}
      >
        {selected
          ? <CheckboxChecked size={15} className="text-accent" />
          : <CheckboxEmpty size={15} />}
      </button>

      {/* artwork */}
      <div className={`relative rounded-md overflow-hidden bg-bg-tertiary border border-border-soft ${COL_ART}`}>
        {thumbFor(task) ? (
          <img src={thumbFor(task)} alt="" className="w-full h-full object-cover" loading="lazy" />
        ) : (
          <div className="w-full h-full grid place-items-center">
            <Track size={15} className="text-text-muted" />
          </div>
        )}
      </div>

      {/* Title + the look-up line — the only elastic cell, so every pixel the
          pane gains lands on the title. Both lines stay single-line + truncate:
          a 200-character title or a paragraph-long yt-dlp error must not change
          the row's height, and the full string is in the tooltip. */}
      <div className="min-w-0 flex-1">
        <p
          title={label}
          className={`text-[13px] truncate leading-snug ${
            isDone || status === 'cancelled'
              ? 'font-normal text-text-secondary'
              : 'font-medium text-text-primary'
          }`}
        >
          {label}
        </p>

        <p
          title={isFailed && progress.error ? progress.error : undefined}
          className="text-[11px] mt-0.5 truncate leading-snug"
        >
          <span className={`font-medium ${PLAT_TEXT[source]}`}>{PLAT_LABEL[source]}</span>
          {/* Muted, matching every other surface's subtitle. It measures 5.9:1 on
              light surface / 4.9:1 on the tertiary — comfortably past the bar. */}
          {isFailed && progress.error ? (
            <span className="text-error">{` · ${progress.error}`}</span>
          ) : (
            <>
              {uploader && <span className="text-text-muted">{` · ${uploader}`}</span>}
              {specs && <span className="text-text-muted font-mono tabular-nums">{` · ${specs}`}</span>}
            </>
          )}
        </p>
      </div>

      {/* State column — fixed width, right-aligned, present in EVERY state, so
          the row's answer to "what are you doing?" never moves. Keyed on the
          coarse phase so a state change fades its new content in rather than
          swapping it mid-blink; opacity only, no layout involved. */}
      <motion.div
        key={isTransferring ? 'live' : status}
        initial={reduced ? false : { opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: reduced ? 0 : 0.18 }}
        className={`flex flex-col items-end justify-center leading-tight ${COL_STATE}`}
      >
        {isTransferring ? (
          <>
            <span className="font-mono tabular-nums text-[14px] font-semibold text-text-primary">
              {Math.round(pct)}%
            </span>
            <span className="font-mono tabular-nums text-[10.5px] text-text-muted mt-0.5">{rate}</span>
          </>
        ) : state && (
          <span className={`inline-flex items-center gap-1.5 text-[11.5px] ${state.tone}`}>
            <state.icon size={13} />
            {state.label}
          </span>
        )}
      </motion.div>

      {/* Actions — fixed-width slot (never wider than the three a queued row
          carries) so the state column above keeps a constant right edge whatever
          a row's state is. Anything that stops a live transfer stays permanently
          visible, and so does Retry — a recovery action must not hide. Only the
          housekeeping actions wait for hover, and those reveal on focus-within
          too so keyboard users get them without a pointer. */}
      <div className={`flex items-center justify-end gap-0.5 ${COL_ACTIONS}`}>
        {onPromote && (
          <IconButton icon={MoveToFront} label="Do this next" size={14} onClick={onPromote} className="w-7 h-7 rounded" />
        )}
        {PAUSABLE.has(status) && (
          <IconButton icon={Pause} label="Pause" size={14} onClick={onPause} className="w-7 h-7 rounded" />
        )}
        {isPaused && (
          <IconButton icon={Play} label="Resume" size={14} onClick={onResume} className="w-7 h-7 rounded text-accent" />
        )}
        {isActive && (
          <IconButton icon={Close} label="Cancel" size={14} onClick={onCancel} className="w-7 h-7 rounded" />
        )}
        {isFailed && (
          <IconButton icon={Retry} label="Retry" size={14} onClick={onRetry} className="w-7 h-7 rounded text-error hover:bg-error/10" />
        )}
        {isDone && (
          <IconButton
            icon={OpenFolder} label="Show in folder" size={14} onClick={onOpen}
            className="w-7 h-7 rounded opacity-0 focus-visible:opacity-100 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity duration-150 motion-reduce:transition-none"
          />
        )}
        {TERMINAL.has(status) && (
          <IconButton
            icon={Remove} label="Remove" size={14} onClick={onRemove}
            className="w-7 h-7 rounded opacity-0 focus-visible:opacity-100 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity duration-150 motion-reduce:transition-none"
          />
        )}
      </div>

      {/* The progress trace belongs to in-flight rows only — a bar under every
          finished row was the reason done and live looked the same. It doubles
          as the pause/retry tell: downloading fills in accent, paused holds
          still in warning, retrying sweeps (ProgressBar's indeterminate mode). */}
      {inFlight && (
        <ProgressBar
          percent={pct}
          status={status}
          label={`${title || 'Download'} progress`}
          // !absolute is load-bearing: ProgressBar's own root carries `relative`,
          // and Tailwind emits .relative after .absolute, so an unweighted
          // `absolute` loses — the trace then renders as a 0-width in-flow flex
          // child (invisible, and it stole 12px of gap from the columns).
          className="!absolute bottom-0 left-0 right-0 !h-[3px] !rounded-none"
        />
      )}

      {/* Landing tint after a "Do this next" jump. The row may have travelled
          from off-screen, so this carries information — where it landed — and
          that is why it survives: one shot, 600ms, quint. Under reduced motion
          it holds the tint for the same 600ms instead of fading. */}
      {pulse && (
        <motion.span
          aria-hidden
          className="absolute inset-0 pointer-events-none bg-accent-soft"
          initial={{ opacity: 1 }}
          animate={{ opacity: reduced ? 1 : 0 }}
          transition={{ duration: reduced ? 0 : 0.6, ease: [0.22, 1, 0.36, 1] }}
        />
      )}
    </>
  );

  if (reorderable) {
    return (
      // dragListener={false} + dragControls: only the handle starts a drag, so a
      // click anywhere else in the row still selects/pauses/cancels as before,
      // and dragging never toggles the selection.
      <Reorder.Item
        value={task.taskId}
        dragListener={false}
        dragControls={controls}
        onDragStart={() => setDragging(true)}
        onDragEnd={() => { setDragging(false); onDragCommit?.(); }}
        whileDrag={reduced ? undefined : { scale: 1.01 }}
        // The app's canonical spring (SegmentedCapsule's numbers) parts the
        // siblings; reduced motion swaps them with no spring at all.
        transition={reduced ? { duration: 0 } : { type: 'spring', stiffness: 480, damping: 34 }}
        className={rowClass}
      >
        {body}
      </Reorder.Item>
    );
  }

  return (
    <motion.li
      initial={{ opacity: 0, y: reduced ? 0 : -4 }}
      animate={{ opacity: 1, y: 0 }}
      // Exits stay subtler than entrances so removals don't pull the eye.
      exit={{ opacity: 0, y: reduced ? 0 : 2 }}
      transition={{ duration: reduced ? 0 : 0.2, ease: [0.25, 0.46, 0.45, 0.94] }}
      className={rowClass}
    >
      {body}
    </motion.li>
  );
}, (a, b) =>
  a.task === b.task && a.selected === b.selected
  && a.reorderable === b.reorderable && a.draggable === b.draggable
  && a.grabbed === b.grabbed && a.pulse === b.pulse
  // Only its presence matters — the handler reads live order through a ref.
  && !a.onPromote === !b.onPromote);

// ── list spine ────────────────────────────────────────────────────────────────

type GroupKey = 'active' | 'attention' | 'done';

const GROUP_LABEL: Record<GroupKey, string> = {
  active:    'In progress',
  attention: 'Needs attention',
  done:      'Completed',
};

const GROUP_ORDER: GroupKey[] = ['active', 'attention', 'done'];

function groupOf(status: DownloadStatus): GroupKey {
  if (status === 'failed' || status === 'cancelled') return 'attention';
  if (status === 'done') return 'done';
  return 'active';
}

// ── inline confirm ────────────────────────────────────────────────────────────

/** Two-step guard for an irreversible action: the first click arms it, the
 *  second executes. Escape and a click outside both disarm without running
 *  the action — the same escape hatches a modal would offer, without one. */
function useInlineConfirm() {
  const [armed, setArmed] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!armed) return;
    function onKeyDown(e: KeyboardEvent): void {
      if (e.key === 'Escape') setArmed(false);
    }
    function onPointerDown(e: MouseEvent): void {
      if (ref.current && !ref.current.contains(e.target as Node)) setArmed(false);
    }
    document.addEventListener('keydown', onKeyDown);
    document.addEventListener('mousedown', onPointerDown);
    return () => {
      document.removeEventListener('keydown', onKeyDown);
      document.removeEventListener('mousedown', onPointerDown);
    };
  }, [armed]);

  return { armed, ref, arm: () => setArmed(true), disarm: () => setArmed(false) };
}

// ── empty states ──────────────────────────────────────────────────────────────

/** A placeholder row: the exact geometry of a real one, drawn as bars. Two of
 *  these under the message keep the empty surface reading as a list waiting to
 *  fill, not as a paragraph floating in a box. */
function GhostRow({ fade }: { fade: string }) {
  return (
    <li aria-hidden className={`flex items-center ${ROW} ${fade}`}>
      <span className={COL_HANDLE} />
      <span className={COL_CHECK} />
      <span className={`rounded-md bg-bg-tertiary ${COL_ART}`} />
      <span className="min-w-0 flex-1 flex flex-col gap-1.5">
        <span className="h-2 w-2/5 rounded-full bg-bg-tertiary" />
        <span className="h-2 w-1/4 rounded-full bg-bg-tertiary" />
      </span>
      <span className={`flex justify-end ${COL_STATE}`}>
        <span className="h-2 w-12 rounded-full bg-bg-tertiary" />
      </span>
      <span className={COL_ACTIONS} />
    </li>
  );
}

/** Empty/no-match states are rows, not centred paragraphs: same height, same
 *  columns, message where a title goes, the recovery action where a row's
 *  actions go. */
function EmptyRow({ icon: Icon, title, body, action }: {
  icon: typeof Download;
  title: string;
  body: string;
  action?: ReactNode;
}) {
  return (
    <li className={`flex items-center ${ROW}`}>
      <span className={COL_HANDLE} />
      <span className={COL_CHECK} />
      <span className={`grid place-items-center rounded-md bg-bg-tertiary border border-border-soft ${COL_ART}`}>
        <Icon size={17} weight="thin" className="text-text-muted" />
      </span>
      <div className="min-w-0 flex-1">
        <p className="text-[13px] font-medium text-text-primary leading-snug">{title}</p>
        <p className="text-[11px] text-text-secondary truncate leading-snug mt-0.5">{body}</p>
      </div>
      <div className="flex items-center justify-end shrink-0">{action}</div>
    </li>
  );
}

// ── DownloadsTab ───────────────────────────────────────────────────────────────

export function DownloadsTab() {
  // Narrow selectors — each re-renders only when its slice changes.
  const downloads            = useAppStore(s => s.downloads);
  const selectedIds          = useAppStore(s => s.selectedDownloadIds);
  const toggleDownloadSelection = useAppStore(s => s.toggleDownloadSelection);
  const selectAllDownloads   = useAppStore(s => s.selectAllDownloads);
  const clearDownloadSelection  = useAppStore(s => s.clearDownloadSelection);
  const clearCompletedAndPersist = useAppStore(s => s.clearCompletedAndPersist);
  const removeDownload       = useAppStore(s => s.removeDownload);

  const [filter, setFilter] = useState<Filter>('all');
  const [confirmCancelAll, setConfirmCancelAll] = useState(false);
  const [busyAll, setBusyAll] = useState(false);

  // ── queued-tail ordering ──
  // The engine owns the real order (its `queue` array); the store only knows
  // insertion order. This is the OPTIMISTIC local view of it: the queued ids in
  // the order the user last put them. Every mutation goes through applyOrder so
  // orderRef stays exact — the row handlers are memoized and would otherwise
  // read a stale array out of their closure.
  const [order, setOrder] = useState<string[]>([]);
  const orderRef = useRef<string[]>([]);
  const applyOrder = (next: string[]): void => { orderRef.current = next; setOrder(next); };
  // Live queued ids in display order — what the key handlers move within.
  const pendingIdsRef = useRef<string[]>([]);
  const [grabbedId, setGrabbedId] = useState<string | null>(null);
  const grabbedRef = useRef<string | null>(null);
  const grabOriginRef = useRef<string[]>([]);
  // The order the list held before the current pointer drag started — onReorder
  // has already overwritten orderRef by the time the drop commits, so the only
  // way back from a rejected reorder is a snapshot taken on the first move.
  const dragOriginRef = useRef<string[] | null>(null);
  const [announce, setAnnounce] = useState('');
  const [pulseId, setPulseId] = useState<string | null>(null);
  const pulseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => { if (pulseTimer.current) clearTimeout(pulseTimer.current); }, []);
  // Both "Clear completed" and the bulk "Remove" button delete rows for good —
  // each gets its own inline arm/confirm/dismiss cycle.
  const clearConfirm = useInlineConfirm();
  const removeConfirm = useInlineConfirm();

  const tasks = useMemo(() => Object.values(downloads), [downloads]);

  // Restored mid-download by a previous session — the user decides their fate.
  const interrupted = useMemo(
    () => tasks.filter(t => t.interrupted && t.progress.status === 'paused'),
    [tasks],
  );

  const counts = useMemo<Record<Filter, number>>(() => ({
    all:    tasks.length,
    active: tasks.filter(t => ACTIVE.has(t.progress.status)).length,
    done:   tasks.filter(t => t.progress.status === 'done').length,
    failed: tasks.filter(t => t.progress.status === 'failed' || t.progress.status === 'cancelled').length,
  }), [tasks]);

  const filtered = useMemo(() => tasks.filter(t => passes(t, filter)), [tasks, filter]);

  // Status is the spine (no timestamps exist to group by day — see file header).
  // Empty groups are dropped, and headers only appear once there's more than one
  // group to tell apart.
  const groups = useMemo(() => {
    const by: Record<GroupKey, DownloadTask[]> = { active: [], attention: [], done: [] };
    for (const t of filtered) by[groupOf(t.progress.status)].push(t);
    return GROUP_ORDER.filter(k => by[k].length > 0).map(k => ({ key: k, tasks: by[k] }));
  }, [filtered]);

  // WITH SEVERAL SLOTS RUNNING, ORDER IS WHAT KEEPS THE LIST READABLE.
  // Inside "In progress" the rows sort into three blocks: what is actually
  // moving (in start order — the store's insertion order is that order), then
  // what is held, then the queued tail the user can reorder. Nothing else about
  // a row changes; the loud %-columns simply stop being interleaved with rows
  // that have nothing to say yet.
  const lane = useMemo(() => {
    const rows = filtered.filter(t => groupOf(t.progress.status) === 'active');
    const rank = new Map(order.map((id, i) => [id, i]));
    return {
      live: rows.filter(t => LIVE.has(t.progress.status)),
      held: rows.filter(t => t.progress.status === 'paused'),
      // Array.sort is stable, so ids the user never moved keep queue order behind
      // the ones they did. 1e9 (not Infinity) — Infinity - Infinity is NaN.
      pending: rows.filter(t => t.progress.status === 'queued')
        .sort((a, b) => (rank.get(a.taskId) ?? 1e9) - (rank.get(b.taskId) ?? 1e9)),
    };
  }, [filtered, order]);

  const pendingIds = useMemo(() => lane.pending.map(t => t.taskId), [lane]);
  useEffect(() => { pendingIdsRef.current = pendingIds; }, [pendingIds]);

  // Summed live speed is real information (total bandwidth in use). There is
  // deliberately no aggregate percent bar: averaging unequal files is a number
  // that looks precise and means nothing.
  const laneSpeed = lane.live.reduce((n, t) => n + t.progress.speed, 0);
  // "Everything is paused" is read off the rows, not off a mirrored engine flag.
  // ponytail: with ONLY queued rows (nothing running yet) Pause all still blocks
  // the queue but nothing repaints, so the button keeps saying "Pause all" —
  // upgrade path is a queue:paused event if that edge ever bites.
  const liveCount = tasks.filter(t => LIVE.has(t.progress.status)).length;
  const pausedCount = tasks.filter(t => t.progress.status === 'paused').length;
  const allPaused = pausedCount > 0 && liveCount === 0;
  const canPauseAll = pausedCount > 0 || tasks.some(t => PAUSABLE.has(t.progress.status));

  // Select-all is scoped to what's currently visible, not the whole history.
  const allSelected  = filtered.length > 0 && filtered.every(t => selectedIds.has(t.taskId));
  const someSelected = !allSelected && filtered.some(t => selectedIds.has(t.taskId));
  const selectedTasks = tasks.filter(t => selectedIds.has(t.taskId));
  const cancellableSelected = selectedTasks.some(t => ACTIVE.has(t.progress.status));
  const removableCount = selectedTasks.filter(t => TERMINAL.has(t.progress.status)).length;
  const removableSelected = removableCount > 0;

  // Re-enqueue a failed task as a fresh download, then drop the old entry.
  // outputDir/retryCount/originalTaskId are dropped: outputDir is always
  // resolved server-side from config (never taken from the renderer, see
  // ipc.ts), and retryCount/originalTaskId only track the queue's own
  // automatic backoff retries, not a fresh manual re-enqueue.
  async function handleRetry(task: DownloadTask): Promise<void> {
    try {
      await window.electronAPI.download.start({
        url: task.url,
        source: task.source,
        format: task.format,
        quality: task.quality,
        videoQuality: task.videoQuality,
        isAudioOnly: task.isAudioOnly,
        isPlaylist: task.isPlaylist,
        playlistName: task.playlistName,
        embedThumbnail: task.embedThumbnail,
        embedMetadata: task.embedMetadata,
        skipExisting: task.skipExisting,
        title: task.title,
        thumbnailUrl: task.thumbnailUrl,
        duration: task.duration,
        uploader: task.uploader,
        track: task.track,
      });
      removeTask(task.taskId);
    } catch {
      // leave the failed row in place so the user can retry again
    }
  }

  /** Push the current queued order to the engine. Optimistic: the list already
   *  shows it. A rejected call snaps back to what was on screen before. */
  function commitOrder(ids: string[], previous: string[]): void {
    applyOrder(ids);
    window.electronAPI.queue.reorder(ids).catch(() => applyOrder(previous));
  }

  /** "Do this next" — front of the pending tail, then a one-shot landing tint so
   *  a row that jumped in from off-screen says where it ended up. */
  function handlePromote(taskId: string): void {
    const before = orderRef.current;
    const next = [taskId, ...pendingIdsRef.current.filter(id => id !== taskId)];
    applyOrder(next);
    window.electronAPI.queue.promote(taskId).catch(() => applyOrder(before));
    setPulseId(taskId);
    if (pulseTimer.current) clearTimeout(pulseTimer.current);
    pulseTimer.current = setTimeout(() => setPulseId(null), 600);
    setAnnounce(`Moved to position 1 of ${next.length}`);
  }

  /** The drag handle's keyboard twin: Space/Enter grabs and drops, the arrows
   *  move one slot, Escape restores the slot the row was grabbed from. Every
   *  outcome is announced — the position change is invisible to a screen reader
   *  otherwise. Reads order through refs: the rows are memoized, so a closure
   *  captured at grab time would move stale ids. */
  function handleReorderKey(taskId: string, e: ReactKeyboardEvent<HTMLButtonElement>): void {
    const ids = pendingIdsRef.current;
    const i = ids.indexOf(taskId);
    if (i === -1) return;

    const grab = (id: string | null): void => { grabbedRef.current = id; setGrabbedId(id); };

    if (e.key === ' ' || e.key === 'Enter') {
      e.preventDefault();
      if (grabbedRef.current === taskId) {
        grab(null);
        commitOrder(ids, grabOriginRef.current);
        setAnnounce('Dropped');
      } else {
        grabOriginRef.current = orderRef.current;
        grab(taskId);
        setAnnounce(`Grabbed. Use the arrow keys to move, Enter to drop, Escape to cancel.`);
      }
      return;
    }
    if (grabbedRef.current !== taskId) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      applyOrder(grabOriginRef.current);
      grab(null);
      setAnnounce('Reorder cancelled');
      return;
    }
    const dir = e.key === 'ArrowUp' ? -1 : e.key === 'ArrowDown' ? 1 : 0;
    if (!dir) return;
    e.preventDefault();
    const j = i + dir;
    if (j < 0 || j >= ids.length) return;
    const next = [...ids];
    [next[i], next[j]] = [next[j], next[i]];
    pendingIdsRef.current = next; // arrows can repeat faster than the effect syncs
    applyOrder(next);
    setAnnounce(`Moved to position ${j + 1} of ${next.length}`);
  }

  /** A grab abandoned by clicking away commits what the user built. Without this
   *  the optimistic order stays on screen while the engine keeps its own — the
   *  list silently lies about what will download next. Enter and Escape have
   *  explicit outcomes; blur is the third exit and needed one too. */
  function handleReorderBlur(taskId: string): void {
    if (grabbedRef.current !== taskId) return;
    grabbedRef.current = null;
    setGrabbedId(null);
    commitOrder(pendingIdsRef.current, grabOriginRef.current);
    setAnnounce('Dropped');
  }

  /** Pause all / Resume all — both reversible, so neither is confirmed. */
  async function handlePauseAll(): Promise<void> {
    setBusyAll(true);
    try {
      if (allPaused) await window.electronAPI.download.resumeAll();
      else await window.electronAPI.download.pauseAll();
    } catch {
      // A rejected call (rate limit, handler throw) left nothing paused, so
      // saying so beats an unhandled rejection the user never sees.
      useAppStore.getState().showError('Could not reach the download queue. Try again.');
    } finally {
      setBusyAll(false);
    }
  }

  /** Remove a row locally AND from the persisted history in main. */
  function removeTask(id: string): void {
    void window.electronAPI.download.remove([id]);
    removeDownload(id);
  }

  function cancelSelected(): void {
    for (const t of selectedTasks) {
      if (ACTIVE.has(t.progress.status)) {
        void window.electronAPI.download.cancel(t.taskId);
        // Deselect only what actually got cancelled — tasks this can't act on
        // (already done/failed) stay selected so the mix isn't silently dropped.
        toggleDownloadSelection(t.taskId);
      }
    }
  }

  function removeSelected(): void {
    const ids = selectedTasks.filter(t => TERMINAL.has(t.progress.status)).map(t => t.taskId);
    if (!ids.length) return;
    void window.electronAPI.download.remove(ids);
    for (const id of ids) removeDownload(id);
    removeConfirm.disarm();
  }

  function resumeInterrupted(): void {
    for (const t of interrupted) void window.electronAPI.download.resume(t.taskId);
  }

  function discardInterrupted(): void {
    const ids = interrupted.map(t => t.taskId);
    void window.electronAPI.download.remove(ids);
    for (const id of ids) removeDownload(id);
  }

  function handleClearCompleted(): void {
    clearCompletedAndPersist();
    clearConfirm.disarm();
  }

  // Cancel-all guards against a slip when several downloads are live: first
  // click arms an inline confirm (auto-resets), second click executes.
  function handleCancelAll(): void {
    if (counts.active > 2 && !confirmCancelAll) {
      setConfirmCancelAll(true);
      setTimeout(() => setConfirmCancelAll(false), 4000);
      return;
    }
    setConfirmCancelAll(false);
    void window.electronAPI.download.cancelAll();
  }

  function renderRow(task: DownloadTask, extra: Partial<CardProps> = {}) {
    return (
      <QueueCard
        key={task.taskId}
        task={task}
        {...extra}
        selected={selectedIds.has(task.taskId)}
        onToggle={() => toggleDownloadSelection(task.taskId)}
        onCancel={() => { void window.electronAPI.download.cancel(task.taskId); }}
        onPause={() => { void window.electronAPI.download.pause(task.taskId); }}
        onResume={() => { void window.electronAPI.download.resume(task.taskId); }}
        onRetry={() => { void handleRetry(task); }}
        onOpen={() => {
          if (task.progress.filename) {
            void revealFile(task.progress.filename, task.title);
          }
        }}
        onRemove={() => removeTask(task.taskId)}
      />
    );
  }

  // One surface holds the whole list — group bands live inside it rather than
  // each group being its own card. Three stacked cards read as three slabs of
  // equal weight; one surface with an internal spine reads as one object.
  const surface = 'rounded-lg bg-bg-surface border border-border-soft shadow-sm overflow-hidden';

  return (
    // The pane is the container, not the viewport: what changes width here is
    // the content column (viewport minus the 68px rail, minus whatever outer
    // container App installs), so every rule below is a @container rule.
    //
    // WIDTH CEILING — 1440px, centred. A queue row is scanned left-to-right
    // (title → state → actions); past ~1400px the eye has to travel so far from
    // the title to the right-aligned state column that the row stops reading as
    // one object. The extra width up to that ceiling is spent entirely on the
    // title, which is the part that truncates today.
    <div className="@container h-full min-h-0 w-full">
    <div className="flex flex-col h-full min-h-0 w-full max-w-[1440px] mx-auto px-6 @4xl:px-10">

      {/* ── page header ──
          The tallest, loudest band on the surface and the only one carrying
          display type — so the three bands below it (control strip, group
          bands, rows) are unmistakably subordinate rather than four equal
          slabs. flex-wrap keeps the title and the two actions on one line at
          the 960 minimum and stops "Really cancel 12?" from crushing the title
          when the confirm expands. */}
      <div className="flex flex-wrap items-end gap-4 pt-7 @4xl:pt-10 pb-5 flex-none">
        <div>
          <h2 className="text-h1 font-semibold tracking-tight text-text-primary leading-[1.1]">
            Queue
          </h2>
          <p className="font-mono text-[11px] uppercase tracking-[0.08em] text-text-muted mt-1">
            <span className="tabular-nums text-text-secondary">{counts.active}</span> active
            {/* Colour is never the only channel — the word rides along with it. */}
            {allPaused && <span className="text-warning"> · paused</span>}
            {' · '}
            <span className="tabular-nums text-text-secondary">{counts.all}</span> total
          </p>
        </div>

        <div className="ml-auto flex items-center gap-2">
          {counts.done > 0 && (
            clearConfirm.armed ? (
              <div ref={clearConfirm.ref} role="group" aria-label="Confirm clear completed" className="flex items-center gap-1.5">
                <span className="text-[12.5px] text-text-secondary">Clear {counts.done}?</span>
                <Button variant="danger" size="sm" onClick={handleClearCompleted}>Confirm</Button>
                <Button variant="ghost" size="sm" onClick={clearConfirm.disarm}>Cancel</Button>
              </div>
            ) : (
              <Button variant="ghost" size="sm" icon={Remove} onClick={clearConfirm.arm}>
                Clear completed
              </Button>
            )
          )}
          {/* Recoverable before destructive: reading order matches severity, and
              neither of these two needs a confirm — both undo each other. */}
          {canPauseAll && (
            <Button
              variant="ghost" size="sm"
              icon={allPaused ? Play : Pause}
              className={allPaused ? 'text-accent' : ''}
              loading={busyAll}
              onClick={() => { void handlePauseAll(); }}
              aria-live="polite"
            >
              {allPaused ? 'Resume all' : 'Pause all'}
            </Button>
          )}
          {counts.active > 0 && (
            <Button
              variant="ghost" size="sm" icon={Cancel}
              className={confirmCancelAll ? 'text-error border-error/40' : ''}
              onClick={handleCancelAll}
              aria-live="polite"
            >
              {confirmCancelAll ? `Really cancel ${counts.active}?` : 'Cancel all'}
            </Button>
          )}
        </div>
      </div>

      {/* ── control strip ──
          One strip, not two bands: the select-all, the filters and the bulk
          actions all belong to the list, so they sit together directly above
          it. The old layout put filters in one row and grew a second row the
          moment anything was selected, pushing the whole list down; here the
          selection actions fill the strip's empty right half instead, and
          min-h-9 keeps the strip's height identical whether they're there or
          not — selecting a row moves nothing.

          The filters are one segmented track rather than four outlined chips:
          four bordered pills read as four separate controls competing with the
          header, a single inset track reads as one control with a state. */}
      <div className="flex flex-wrap items-center gap-2 min-h-9 pb-3 flex-none">
        <button
          onClick={() => allSelected ? clearDownloadSelection() : selectAllDownloads(filtered.map(t => t.taskId))}
          aria-label="Toggle select all"
          aria-pressed={allSelected ? true : someSelected ? 'mixed' : false}
          // ml-11 = px-4 (16) + handle (16) + gap-3 (12): the row's checkbox moved
          // one column right when the drag handle landed, and this tracks it.
          className={`no-drag grid place-items-center ml-11 mr-2 text-text-muted hover:text-accent transition-colors ${COL_CHECK}`}
        >
          {allSelected
            ? <CheckboxChecked size={15} className="text-accent" />
            : someSelected
            ? <CheckboxMixed size={15} />
            : <CheckboxEmpty size={15} />}
        </button>

        <div role="group" aria-label="Filter queue" className="inline-flex items-center gap-0.5 p-0.5 rounded-lg bg-bg-secondary border border-border-soft">
          {FILTERS.map(({ key, label }) => (
            <button
              key={key}
              onClick={() => setFilter(key)}
              aria-pressed={filter === key}
              className={`no-drag inline-flex items-center gap-1.5 h-7 px-2.5 rounded-md text-[12.5px] font-medium transition-colors ${
                filter === key
                  ? 'bg-bg-surface text-text-primary shadow-sm'
                  : 'text-text-secondary hover:text-text-primary'
              }`}
            >
              {label}
              <span className={`font-mono text-[10px] tabular-nums ${filter === key ? 'text-accent' : 'text-text-muted'}`}>
                {counts[key]}
              </span>
            </button>
          ))}
        </div>

        {selectedIds.size > 0 && (
          <div className="ml-auto flex flex-wrap items-center gap-2">
            <span className="font-mono text-[11px] tabular-nums text-text-secondary">
              {selectedIds.size} selected
            </span>
            <Button variant="ghost" size="sm" icon={Cancel} onClick={cancelSelected} disabled={!cancellableSelected}>
              Cancel
            </Button>
            {/* Two-step delete guard: Remove only arms the prompt below; the
                onConfirm step (the Confirm button) is what actually removes. */}
            {removeConfirm.armed ? (
              <div ref={removeConfirm.ref} role="group" aria-label="Confirm remove selected" className="flex items-center gap-1.5">
                <span className="text-[11px] text-text-secondary">Remove {removableCount}?</span>
                <Button variant="danger" size="sm" onClick={removeSelected}>Confirm</Button>
                <Button variant="ghost" size="sm" onClick={removeConfirm.disarm}>Cancel</Button>
              </div>
            ) : (
              <Button variant="ghost" size="sm" icon={Remove} onClick={removeConfirm.arm} disabled={!removableSelected}>
                Remove
              </Button>
            )}
            <button
              onClick={clearDownloadSelection}
              className="no-drag text-[11.5px] text-text-muted hover:text-text-primary transition-colors"
            >
              Clear selection
            </button>
          </div>
        )}
      </div>

      {/* ── interrupted-downloads decision banner ── */}
      {interrupted.length > 0 && (
        // The sentence is flex-1 min-w-0 so it takes the slack, but it can run
        // to two lines at 960: wrapping keeps Resume/Discard whole instead of
        // squeezing them.
        <div role="status" className="flex flex-wrap items-center gap-3 mb-3 px-4 py-3 rounded-lg border border-warning/30 bg-warning/10 flex-none">
          <Pause size={15} className="text-warning shrink-0" />
          <p className="text-[12.5px] text-text-primary flex-1 min-w-0">
            <b className="font-mono tabular-nums">{interrupted.length}</b>
            {` download${interrupted.length === 1 ? '' : 's'} interrupted when the app closed. Finish ${interrupted.length === 1 ? 'it' : 'them'} now?`}
          </p>
          <Button variant="primary" size="sm" icon={Play} onClick={resumeInterrupted}>
            Resume
          </Button>
          <Button variant="ghost" size="sm" icon={Remove} onClick={discardInterrupted}>
            Discard
          </Button>
        </div>
      )}

      {/* Reorder outcomes are pure position changes — invisible to a screen
          reader unless they are said out loud. */}
      <p aria-live="polite" className="sr-only">{announce}</p>

      {/* ── row list ── */}
      <div className="flex-1 overflow-y-auto min-h-0 pb-10">
        {tasks.length === 0 ? (
          // Nothing has ever been queued — the only state that should teach.
          // It teaches on the row grid, with two ghost rows showing what will
          // land here, rather than as centred text floating in the void.
          <div className={surface}>
            <ul className="divide-y divide-border-soft">
              <EmptyRow
                icon={Download}
                title="Your queue is empty"
                body="Paste a link on any source tab and hit Download — it lands here with live progress."
              />
              <GhostRow fade="opacity-50" />
              <GhostRow fade="opacity-25" />
            </ul>
          </div>
        ) : filtered.length === 0 ? (
          // A filter that matches nothing is a different problem: there IS
          // history, it's just hidden. Offer the way back instead of a lesson.
          <div className={surface}>
            <ul>
              <EmptyRow
                icon={filter === 'failed' ? Check : Clock}
                title={
                  filter === 'active' ? 'Nothing is downloading right now'
                    : filter === 'done' ? 'No finished downloads yet'
                    : 'No failed or cancelled downloads'
                }
                body={`${counts.all} download${counts.all === 1 ? '' : 's'} in the full queue.`}
                action={
                  <Button variant="ghost" size="sm" onClick={() => setFilter('all')}>
                    Show all {counts.all}
                  </Button>
                }
              />
            </ul>
          </div>
        ) : (
          <div className={surface}>
            {groups.map(g => {
              // Only "In progress" splits into blocks — the other two groups are
              // one flat list, exactly as they shipped.
              const isQueueLane = g.key === 'active';
              const moving = isQueueLane ? [...lane.live, ...lane.held] : g.tasks;
              const tail = isQueueLane ? lane.pending : [];
              // The sub-band is the seam between the two blocks, so it exists
              // only when there are two blocks to separate.
              const subBand = moving.length > 0 && tail.length > 0;
              return (
              /* first:border-t-0 — the surface's own edge already draws the top
                 hairline; every later group closes off the group above it. */
              <section key={g.key} aria-labelledby={`queue-group-${g.key}`} className="border-t border-border-soft first:border-t-0">
                {groups.length > 1 && (
                  // A band inside the surface, not a heading floating above a
                  // card — it separates without adding another container.
                  <div className="flex items-center gap-2 h-8 px-4 bg-bg-secondary/60 border-b border-border-soft">
                    <h3 id={`queue-group-${g.key}`} className="text-[11.5px] font-medium text-text-secondary">
                      {GROUP_LABEL[g.key]}
                      {isQueueLane && allPaused && <span className="text-warning"> · paused</span>}
                    </h3>
                    <span className="font-mono text-[10.5px] tabular-nums text-text-muted">{g.tasks.length}</span>
                    {/* Total bandwidth in use — the one aggregate that is true
                        when several rows are moving at once. */}
                    {isQueueLane && lane.live.length > 0 && (
                      <span
                        className="ml-auto font-mono text-[10.5px] tabular-nums text-text-secondary"
                        aria-label={`${lane.live.length} transferring${laneSpeed > 0 ? `, ${fmtSpeed(laneSpeed)} total` : ''}`}
                      >
                        {`↓ ${lane.live.length}`}{laneSpeed > 0 ? ` · ${fmtSpeed(laneSpeed)}` : ''}
                      </span>
                    )}
                  </div>
                )}

                {moving.length > 0 && (
                  <ul className="divide-y divide-border-soft">
                    <AnimatePresence initial={false}>
                      {moving.map(t => renderRow(t))}
                    </AnimatePresence>
                  </ul>
                )}

                {/* One hairline of type does three jobs: it separates the loud
                    %-columns from the rows that are only waiting, it labels the
                    reorderable region, and it is that region's landmark. */}
                {subBand && (
                  <div
                    id="queue-up-next"
                    className="flex items-center h-7 pl-32 pr-4 border-t border-border-soft font-mono text-[10.5px] uppercase tracking-[0.08em] text-text-muted"
                  >
                    {`Up next · ${tail.length}`}
                  </div>
                )}

                {tail.length > 0 && (
                  <Reorder.Group
                    axis="y"
                    values={pendingIds}
                    // Live during the drag; the engine hears about it on drop.
                    onReorder={(ids: string[]) => {
                      dragOriginRef.current ??= orderRef.current.length ? orderRef.current : pendingIdsRef.current;
                      applyOrder(ids);
                    }}
                    aria-labelledby={subBand ? 'queue-up-next' : undefined}
                    className={`divide-y divide-border-soft ${moving.length > 0 ? 'border-t border-border-soft' : ''}`}
                  >
                    {tail.map((t, i) => renderRow(t, {
                      reorderable: true,
                      draggable: tail.length > 1,
                      grabbed: grabbedId === t.taskId,
                      pulse: pulseId === t.taskId,
                      onReorderKey: (e) => handleReorderKey(t.taskId, e),
                      onReorderBlur: () => handleReorderBlur(t.taskId),
                      onDragCommit: () => {
                        const before = dragOriginRef.current ?? orderRef.current;
                        dragOriginRef.current = null;
                        commitOrder(pendingIdsRef.current, before);
                      },
                      // Hidden on the row that is already first — an action that
                      // cannot act should not be drawn.
                      ...(i > 0 ? { onPromote: () => handlePromote(t.taskId) } : {}),
                    }))}
                  </Reorder.Group>
                )}
              </section>
              );
            })}
          </div>
        )}
      </div>
    </div>
    </div>
  );
}
