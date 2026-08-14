// Downloads — the queue, ranked by liveness. A transfer in flight is the loudest
// thing on screen (taller row, three-line mono telemetry, moving progress trace);
// a finished one is deliberately quiet (a single check mark, one final size, no
// bar). Failed rows keep their volume because they still need a decision.
//
// NOTE ON GROUPING: DownloadTask carries no timestamp (see shared/types.ts) —
// there is no createdAt/completedAt to group by day with, and the store keeps
// insertion order only. Rather than invent a date, the list's spine is status:
// In progress → Needs attention → Completed.
import { memo, useMemo, useState } from 'react';
import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import {
  Download, Close, Cancel, Retry, OpenFolder, Check, Alert, Clock,
  CheckboxChecked, CheckboxEmpty, Remove, CheckboxMixed, Track, Pause, Play,
} from '@/components/ui/icons';
import { useAppStore } from '@/store';
import { Button, IconButton, ProgressBar } from '@/components/ui';
import { PLATFORMS } from '@/constants';
import { formatBytes, formatDuration } from '@/lib/format';
import { revealFile } from '@/lib/reveal';
import type { DownloadStatus, DownloadTask, SourcePlatform } from '@shared/types';

// ── formatters ────────────────────────────────────────────────────────────────
// Byte/duration formatting lives once in src/lib/format.ts (hour-aware).

const fmtSpeed = (bps: number): string => bps > 0 ? `${formatBytes(bps)}/s` : '—';

function fmtEta(secs: number): string {
  if (secs <= 0 || !isFinite(secs)) return '—';
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return m >= 60
    ? `${Math.floor(m / 60)}h ${m % 60}m`
    : `${m}:${String(s).padStart(2, '0')}`;
}

/** In flight the FRACTION is the story, so the unit is spelled once ("6.8 / 15.2
 *  MB") — two numbers that share a scale compare at a glance. Once a file is
 *  complete the fraction is meaningless, so callers switch to formatBytes(). */
function fmtTransferred(done: number, total: number): string {
  const t = formatBytes(total);
  const d = formatBytes(done);
  const unit = t.slice(t.indexOf(' ') + 1);
  return `${d.endsWith(unit) ? d.slice(0, -unit.length - 1) : d} / ${t}`;
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

/** The word a row shows in its telemetry column. `downloading` and `done` are
 *  absent on purpose: those two states show a number instead (percent / final
 *  size), which says more than the word ever did. */
const STATUS_WORD: Partial<Record<DownloadStatus, { label: string; tone: string }>> = {
  queued:        { label: 'Queued',     tone: 'text-text-muted' },
  fetching_info: { label: 'Fetching',   tone: 'text-text-secondary' },
  converting:    { label: 'Converting', tone: 'text-warning' },
  embedding:     { label: 'Tagging',    tone: 'text-warning' },
  paused:        { label: 'Paused',     tone: 'text-warning' },
  retrying:      { label: 'Retrying',   tone: 'text-warning' },
  failed:        { label: 'Failed',     tone: 'text-error' },
  cancelled:     { label: 'Cancelled',  tone: 'text-text-muted' },
};

/** The quiet state mark — one 13px glyph in a fixed slot, replacing the row of
 *  identical status pills. Live rows get nothing here; their progress trace and
 *  percentage already say what they are. */
const STATE_MARK: Partial<Record<DownloadStatus, { icon: typeof Check; tone: string; label: string }>> = {
  done:      { icon: Check,  tone: 'text-success',      label: 'Done' },
  failed:    { icon: Alert,  tone: 'text-error',        label: 'Failed' },
  cancelled: { icon: Close,  tone: 'text-text-muted',   label: 'Cancelled' },
  paused:    { icon: Pause,  tone: 'text-warning',      label: 'Paused' },
  queued:    { icon: Clock,  tone: 'text-text-muted',   label: 'Queued' },
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
  direct: 'Direct',    unknown: 'Unknown',
};

const PLAT_TEXT: Record<SourcePlatform, string> = {
  youtube: 'text-youtube',      spotify: 'text-spotify',  soundcloud: 'text-soundcloud',
  instagram: 'text-reels',      tiktok: 'text-reels',     facebook: 'text-reels',
  direct: 'text-text-secondary', unknown: 'text-text-secondary',
};

// ── shared constants ──────────────────────────────────────────────────────────

// ponytail: Set for O(1) membership tests across filters + row render
const ACTIVE = new Set<DownloadStatus>([
  'queued', 'fetching_info', 'downloading', 'converting', 'embedding', 'paused', 'retrying',
]);
/** In-flight family — the only states that own a progress trace and the taller
 *  row. A queued task has nothing to show yet; a finished one has nothing left. */
const IN_FLIGHT = new Set<DownloadStatus>([
  'fetching_info', 'downloading', 'converting', 'embedding', 'retrying', 'paused',
]);
/** States where pausing is offered — mid-conversion kills waste finished work. */
const PAUSABLE = new Set<DownloadStatus>(['queued', 'fetching_info', 'downloading']);
const TERMINAL = new Set<DownloadStatus>(['done', 'failed', 'cancelled']);

// ── filter ────────────────────────────────────────────────────────────────────

type Filter = 'all' | 'active' | 'done' | 'failed';

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
}

// memo: progress events tick several times per second while downloading —
// only the row whose task object changed should re-render.
export const QueueCard = memo(function QueueCard({ task, selected, onToggle, onCancel, onPause, onResume, onRetry, onOpen, onRemove }: CardProps) {
  const reduced = useReducedMotion();
  const { progress, source, format, title, uploader } = task;
  const { status, percent, speed, eta, downloaded, total } = progress;
  const isPaused = status === 'paused';
  const isDone = status === 'done';
  const isFailed = status === 'failed' || status === 'cancelled';
  const isActive = ACTIVE.has(status);
  const inFlight = IN_FLIGHT.has(status);
  const isTransferring = status === 'downloading';
  const pct = Math.max(0, Math.min(100, isDone ? 100 : percent));
  const word = STATUS_WORD[status];
  const mark = STATE_MARK[status];
  const meta = `${formatDuration(task.duration)}${task.duration ? ' · ' : ''}${format || '—'}`;

  return (
    // Two row heights, one rule: in flight is 76px, everything else 62px. The
    // height difference is the cheapest, most legible "this is happening now".
    <motion.div
      layout={!reduced}
      initial={{ opacity: 0, y: reduced ? 0 : -4 }}
      animate={{ opacity: 1, y: 0 }}
      // Exits stay subtler than entrances so removals don't pull the eye.
      exit={{ opacity: 0, y: reduced ? 0 : 2 }}
      transition={{ duration: reduced ? 0 : 0.2, ease: [0.25, 0.46, 0.45, 0.94] }}
      className={`group relative flex items-center gap-3.5 px-4 transition-colors ${
        inFlight ? 'h-[76px]' : 'h-[62px]'
      } ${
        selected ? 'bg-accent-soft'
        : status === 'failed' ? 'bg-error/[0.05] hover:bg-error/[0.09]'
        : 'hover:bg-bg-hover'
      }`}
    >
      {/* checkbox — w-5 box column-aligns with the select-all above */}
      <button
        onClick={onToggle}
        className="no-drag grid place-items-center w-5 shrink-0 text-text-muted hover:text-accent transition-colors"
        aria-label={selected ? 'Deselect' : 'Select'}
      >
        {selected
          ? <CheckboxChecked size={15} className="text-accent" />
          : <CheckboxEmpty size={15} />}
      </button>

      {/* thumbnail */}
      <div className="relative w-10 h-10 rounded-md overflow-hidden bg-bg-tertiary border border-border-soft shrink-0">
        {thumbFor(task) ? (
          <img src={thumbFor(task)} alt="" className="w-full h-full object-cover" loading="lazy" />
        ) : (
          <div className="w-full h-full grid place-items-center">
            <Track size={15} className="text-text-muted" />
          </div>
        )}
      </div>

      {/* title + platform/uploader (or the failure reason) — the only elastic
          cell in the row, so every pixel the pane gains lands here. Both lines
          stay single-line + truncate: a 200-character title or a paragraph-long
          yt-dlp error must not change the row's height, and the full string is
          recoverable from the tooltip. */}
      <div className="min-w-0 flex-1">
        <p
          title={title || baseName(progress.filename) || 'Untitled'}
          className={`text-[13px] truncate leading-snug ${
            isDone || status === 'cancelled'
              ? 'font-normal text-text-secondary'
              : 'font-medium text-text-primary'
          }`}
        >
          {title || baseName(progress.filename) || 'Untitled'}
        </p>

        <p
          title={isFailed && progress.error ? progress.error : undefined}
          className="text-[11px] mt-0.5 truncate leading-snug"
        >
          <span className={`font-mono ${PLAT_TEXT[source]}`}>{PLAT_LABEL[source]}</span>
          {/* Muted, matching every other surface's subtitle. It measures 5.9:1 on
              light surface / 4.9:1 on the tertiary — comfortably past the bar. */}
          <span className={isFailed && progress.error ? 'text-error' : 'text-text-muted'}>
            {isFailed && progress.error
              ? ` · ${progress.error}`
              : (uploader ? ` · ${uploader}` : '') +
                (task.isPlaylist && progress.playlistTotal > 0
                  ? ` · ${progress.playlistIndex}/${progress.playlistTotal}`
                  : '')}
          </span>
        </p>
      </div>

      {/* state mark — fixed slot so the marks read as their own column */}
      <div className="w-4 shrink-0 grid place-items-center">
        {mark && (
          <span role="img" aria-label={mark.label} title={mark.label}>
            <mark.icon size={13} className={mark.tone} />
          </span>
        )}
      </div>

      {/* Numeric column: fixed width + right-aligned + tabular so the right edge
          is a true edge. Transferring gets three lines (it earns them); every
          other state gets two.
          Hidden only below a 576px pane — a width the 960px minimum window
          (≈812px pane) never reaches, so at every real size the numbers are
          present. Stated as @max-xl rather than a min-width so the safe state
          (visible) is the default if this row is ever rendered outside the
          pane container. */}
      <div className="flex @max-xl:hidden flex-col items-end justify-center w-[128px] shrink-0 font-mono tabular-nums leading-tight">
        {isTransferring ? (
          <>
            <span className="text-[13px] font-semibold text-text-primary">{Math.round(pct)}%</span>
            <span className="text-[10.5px] text-text-secondary mt-0.5">
              {total > 0 ? fmtTransferred(downloaded, total) : formatBytes(downloaded)}
            </span>
            <span className="text-[10.5px] text-text-muted">
              {fmtSpeed(speed)} · {fmtEta(eta)}
            </span>
          </>
        ) : (
          <>
            {isDone
              // Complete: the total is the fact. No fraction, no "x / x".
              ? <span className="text-[12px] text-text-secondary">{formatBytes(total || downloaded)}</span>
              : word && <span className={`text-[11px] ${word.tone}`}>{word.label}</span>}
            <span className="text-[10.5px] text-text-muted mt-0.5">{meta}</span>
          </>
        )}
      </div>

      {/* Actions — fixed-width slot (never wider than two buttons) so the numeric
          column above keeps a constant right edge whatever a row's state is.
          Destructive/secondary actions are revealed on hover AND focus-within,
          so keyboard users get them without a pointer. */}
      <div className="flex items-center justify-end gap-0.5 w-[60px] shrink-0">
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
          finished row was the reason done and live looked the same. */}
      {inFlight && (
        <ProgressBar
          percent={pct}
          status={status}
          label={`${title || 'Download'} progress`}
          className="absolute bottom-0 left-0 right-0 !h-0.5 !rounded-none"
        />
      )}
    </motion.div>
  );
}, (a, b) => a.task === b.task && a.selected === b.selected);

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

/** Section header: a sentence-case name, a mono count, and a hairline running to
 *  the right edge — a spine you can scan, not a tracked-uppercase eyebrow. */
function GroupHeader({ label, count }: { label: string; count: number }) {
  return (
    <div className="flex items-center gap-2.5 px-1 pb-2">
      <h3 className="text-[12px] font-medium text-text-secondary">{label}</h3>
      <span className="font-mono text-[10.5px] tabular-nums text-text-muted">{count}</span>
      <span aria-hidden className="flex-1 h-px bg-border-soft" />
    </div>
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

  // Select-all is scoped to what's currently visible, not the whole history.
  const allSelected  = filtered.length > 0 && filtered.every(t => selectedIds.has(t.taskId));
  const someSelected = !allSelected && filtered.some(t => selectedIds.has(t.taskId));
  const selectedTasks = tasks.filter(t => selectedIds.has(t.taskId));
  const cancellableSelected = selectedTasks.some(t => ACTIVE.has(t.progress.status));
  const removableSelected = selectedTasks.some(t => TERMINAL.has(t.progress.status));

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
  }

  function resumeInterrupted(): void {
    for (const t of interrupted) void window.electronAPI.download.resume(t.taskId);
  }

  function discardInterrupted(): void {
    const ids = interrupted.map(t => t.taskId);
    void window.electronAPI.download.remove(ids);
    for (const id of ids) removeDownload(id);
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

  function renderRow(task: DownloadTask) {
    return (
      <QueueCard
        key={task.taskId}
        task={task}
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

  return (
    // The pane is the container, not the viewport: what changes width here is
    // the content column (viewport minus the 68px rail, minus whatever outer
    // container App installs), so every rule below is a @container rule.
    //
    // WIDTH CEILING — 1440px, centred. A queue row is scanned left-to-right
    // (title → numbers → actions); past ~1400px the eye has to travel so far
    // from the title to the right-aligned telemetry that the row stops reading
    // as one object. The extra width up to that ceiling is spent entirely on
    // the title, which is the part that truncates today. Nothing else was
    // added to "earn" a 2560px row: DownloadTask has no timestamp (see the file
    // header) and the only other fact — the output path — is already one click
    // away via Show in folder, so a path column would double the row's ink for
    // a question that is asked rarely.
    <div className="@container h-full min-h-0 w-full">
    <div className="flex flex-col h-full min-h-0 w-full max-w-[1440px] mx-auto px-6 @4xl:px-10">

      {/* ── page header ──
          flex-wrap + the tighter top padding below @4xl (≈964px window, the
          960 minimum) keep the title and the two actions on one line at the
          minimum size, and stop "Really cancel 12?" from crushing the title
          when the confirm expands. */}
      <div className="flex flex-wrap items-end gap-4 pt-7 @4xl:pt-10 pb-5 flex-none">
        <div>
          <h2 className="font-display text-[32px] font-semibold text-text-primary tracking-[-0.025em] leading-[1.1]">
            Queue
          </h2>
          <p className="font-mono text-[10.5px] uppercase tracking-[0.08em] text-text-muted mt-1">
            <span className="tabular-nums text-text-secondary">{counts.active}</span> active
            {' · '}
            <span className="tabular-nums text-text-secondary">{counts.all}</span> total
          </p>
        </div>

        <div className="ml-auto flex items-center gap-2">
          {counts.done > 0 && (
            <Button variant="ghost" size="sm" icon={Remove} onClick={clearCompletedAndPersist}>
              Clear completed
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

      {/* ── filter chips + select-all ──
          The four chips + select-all measure ~430px, so they never wrap at the
          960 minimum; flex-wrap is the guard for a localised label, not a
          layout we design for. */}
      <div className="flex flex-wrap items-center gap-2 pb-3 flex-none">
        <button
          onClick={() => allSelected ? clearDownloadSelection() : selectAllDownloads(filtered.map(t => t.taskId))}
          className="no-drag grid place-items-center w-6 h-6 ml-[17px] mr-2 text-text-muted hover:text-accent transition-colors"
          aria-label="Toggle select all"
        >
          {allSelected
            ? <CheckboxChecked size={15} className="text-accent" />
            : someSelected
            ? <CheckboxMixed size={15} />
            : <CheckboxEmpty size={15} />}
        </button>
        {(['all', 'active', 'done', 'failed'] as const).map(f => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            aria-pressed={filter === f}
            className={`no-drag inline-flex items-center gap-1.5 h-8 px-3 rounded-md text-[12.5px] font-medium transition-colors ${
              filter === f
                ? 'bg-accent-soft text-accent border border-accent/40'
                : 'text-text-secondary border border-border hover:text-text-primary hover:bg-bg-hover'
            }`}
          >
            <span className="capitalize">{f}</span>
            <span className={`font-mono text-[10px] tabular-nums ${filter === f ? 'text-accent' : 'text-text-muted'}`}>
              {counts[f]}
            </span>
          </button>
        ))}
      </div>

      {/* ── interrupted-downloads decision banner ── */}
      {interrupted.length > 0 && (
        // The sentence is flex-1 min-w-0 so it takes the slack, but it can run
        // to two lines at 960 — wrapping keeps Resume/Discard whole instead of
        // squeezing them.
        <div role="status" className="flex flex-wrap items-center gap-3 mb-3 px-4 py-3 rounded-lg border border-warning/30 bg-warning/10 flex-none">
          <Pause size={15} className="text-warning shrink-0" />
          <p className="text-[12.5px] text-text-primary flex-1 min-w-0">
            <b className="font-mono tabular-nums">{interrupted.length}</b>
            {` download${interrupted.length === 1 ? '' : 's'} interrupted when the app closed — finish ${interrupted.length === 1 ? 'it' : 'them'} now?`}
          </p>
          <Button variant="primary" size="sm" icon={Play} onClick={resumeInterrupted}>
            Resume
          </Button>
          <Button variant="ghost" size="sm" icon={Remove} onClick={discardInterrupted}>
            Discard
          </Button>
        </div>
      )}

      {/* ── bulk-action bar (appears with a selection) ── */}
      {selectedIds.size > 0 && (
        <div className="flex flex-wrap items-center gap-2 pb-3 flex-none">
          <span className="font-mono text-[11px] tabular-nums text-text-secondary mr-1">
            {selectedIds.size} selected
          </span>
          <Button variant="ghost" size="sm" icon={Cancel} onClick={cancelSelected} disabled={!cancellableSelected}>
            Cancel
          </Button>
          <Button variant="ghost" size="sm" icon={Remove} onClick={removeSelected} disabled={!removableSelected}>
            Remove
          </Button>
          <button
            onClick={clearDownloadSelection}
            className="no-drag text-[11.5px] text-text-muted hover:text-text-primary transition-colors ml-1"
          >
            Clear selection
          </button>
        </div>
      )}

      {/* ── row list ── */}
      <div className="flex-1 overflow-y-auto min-h-0 pb-10">
        {tasks.length === 0 ? (
          // Nothing has ever been queued — the only state that should teach.
          <div className="flex flex-col items-center justify-center h-64 gap-3 text-center select-none">
            <Download size={28} weight="thin" className="text-text-muted" />
            <p className="text-[14px] font-medium text-text-primary">Your queue is empty</p>
            <p className="text-[12.5px] text-text-secondary max-w-[34ch] leading-relaxed">
              Paste a link on any source tab and hit Download — everything you
              grab lands here, with live speed and progress.
            </p>
          </div>
        ) : filtered.length === 0 ? (
          // A filter that matches nothing is a different problem: there IS
          // history, it's just hidden. Offer the way back instead of a lesson.
          <div className="flex flex-col items-center justify-center h-48 gap-2.5 text-center select-none">
            <p className="text-[13px] text-text-secondary">
              {filter === 'active' ? 'Nothing is downloading right now.'
                : filter === 'done' ? 'No finished downloads yet.'
                : 'No failed or cancelled downloads — all clear.'}
            </p>
            <Button variant="ghost" size="sm" onClick={() => setFilter('all')}>
              Show all {counts.all}
            </Button>
          </div>
        ) : (
          <div className="flex flex-col gap-5">
            {groups.map(g => (
              <section key={g.key}>
                {groups.length > 1 && <GroupHeader label={GROUP_LABEL[g.key]} count={g.tasks.length} />}
                <div className="rounded-lg bg-bg-surface border border-border-soft shadow-sm overflow-hidden divide-y divide-border-soft">
                  <AnimatePresence initial={false}>
                    {g.tasks.map(renderRow)}
                  </AnimatePresence>
                </div>
              </section>
            ))}
          </div>
        )}
      </div>
    </div>
    </div>
  );
}
