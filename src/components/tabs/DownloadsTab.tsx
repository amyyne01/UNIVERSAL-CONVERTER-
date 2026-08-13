// Downloads — informative card queue. Live yt-dlp progress (percent / size /
// speed / ETA), thumbnail + duration, per-task pause / resume / cancel, bulk
// actions on selection, and history persisted in main (hydrated in App.tsx).
import { memo, useMemo, useState } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import {
  Download, X, XCircle, RotateCcw, FolderOpen,
  CheckSquare, Square, Trash2, Minus, Music2, Pause, Play,
} from 'lucide-react';
import { useAppStore } from '@/store';
import { Button, IconButton, ProgressBar } from '@/components/ui';
import { PLATFORMS } from '@/constants';
import { formatBytes, formatDuration } from '@/lib/format';
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

/** progress.filename is a full path — cards show just the file name. */
const baseName = (p: string): string => p.split(/[\\/]/).pop() ?? '';

/** Tasks started via hotkey/palette/history may lack a thumbnail — YouTube's
 *  is derivable from the video id, so those cards still render art. */
function thumbFor(task: DownloadTask): string {
  if (task.thumbnailUrl) return task.thumbnailUrl;
  if (task.source === 'youtube') {
    const m = task.url.match(/(?:v=|youtu\.be\/|\/shorts\/)([\w-]{11})/);
    if (m) return `https://i.ytimg.com/vi/${m[1]}/mqdefault.jpg`;
  }
  return '';
}

// ── status config ─────────────────────────────────────────────────────────────

const STATUS_CFG: Record<DownloadStatus, { label: string; dot: string; pill: string; pulse: boolean }> = {
  queued:        { label: 'Queued',      dot: 'bg-text-muted', pill: 'bg-bg-hover text-text-muted',   pulse: false },
  fetching_info: { label: 'Fetching',    dot: 'bg-accent',     pill: 'bg-accent-soft text-accent',    pulse: true  },
  downloading:   { label: 'Downloading', dot: 'bg-accent',     pill: 'bg-accent-soft text-accent',    pulse: true  },
  converting:    { label: 'Converting',  dot: 'bg-warning',    pill: 'bg-warning/10 text-warning',    pulse: true  },
  embedding:     { label: 'Embedding',   dot: 'bg-warning',    pill: 'bg-warning/10 text-warning',    pulse: true  },
  paused:        { label: 'Paused',      dot: 'bg-warning',    pill: 'bg-warning/10 text-warning',    pulse: false },
  retrying:      { label: 'Retrying',    dot: 'bg-warning',    pill: 'bg-warning/10 text-warning',    pulse: true  },
  done:          { label: 'Done',        dot: 'bg-success',    pill: 'bg-success/10 text-success',    pulse: false },
  failed:        { label: 'Failed',      dot: 'bg-error',      pill: 'bg-error/10 text-error',        pulse: false },
  cancelled:     { label: 'Cancelled',   dot: 'bg-text-muted', pill: 'bg-bg-hover text-text-muted',   pulse: false },
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

// ponytail: Set for O(1) membership tests across filters + card render
const ACTIVE = new Set<DownloadStatus>([
  'queued', 'fetching_info', 'downloading', 'converting', 'embedding', 'paused', 'retrying',
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
// only the card whose task object changed should re-render.
export const QueueCard = memo(function QueueCard({ task, selected, onToggle, onCancel, onPause, onResume, onRetry, onOpen, onRemove }: CardProps) {
  const { progress, source, format, title, uploader } = task;
  const { status, percent, speed, eta, downloaded, total } = progress;
  const cfg = STATUS_CFG[status];
  const isPaused = status === 'paused';
  const isDone = status === 'done';
  const isFailed = status === 'failed' || status === 'cancelled';
  const isActive = ACTIVE.has(status);
  const pct = Math.max(0, Math.min(100, isDone ? 100 : percent));

  return (
    // 72px telemetry row (CATHODE): hairline-separated inside one Card, 40px
    // thumb, mono readouts, a 2px platform-tinted progress hairline at the base.
    <motion.div
      layout
      initial={{ opacity: 0, y: -6 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 4 }}
      transition={{ duration: 0.18 }}
      className={`relative flex items-center gap-3.5 h-[72px] px-4 transition-colors ${
        selected ? 'bg-accent-soft' : 'hover:bg-bg-hover'
      }`}
    >
      {/* selected: 2px accent bar on the leading edge */}
      {selected && <span aria-hidden className="absolute left-0 inset-y-2.5 w-[2px] rounded-r bg-accent" />}

      {/* checkbox — w-5 box column-aligns with the select-all above */}
      <button
        onClick={onToggle}
        className="no-drag grid place-items-center w-5 shrink-0 text-text-muted hover:text-accent transition-colors"
        aria-label={selected ? 'Deselect' : 'Select'}
      >
        {selected
          ? <CheckSquare size={15} className="text-accent" />
          : <Square size={15} />}
      </button>

      {/* 40px thumbnail */}
      <div className="relative w-10 h-10 rounded-md overflow-hidden bg-bg-tertiary border border-border-soft shrink-0">
        {thumbFor(task) ? (
          <img src={thumbFor(task)} alt="" className="w-full h-full object-cover" loading="lazy" />
        ) : (
          <div className="w-full h-full grid place-items-center">
            <Music2 size={15} className="text-text-muted" />
          </div>
        )}
      </div>

      {/* title + platform / status subtitle */}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2.5">
          <p className="flex-1 min-w-0 text-[13px] font-medium text-text-primary truncate leading-snug">
            {title || baseName(progress.filename) || 'Untitled'}
          </p>
          <span className={`inline-flex items-center gap-1.5 px-1.5 h-5 rounded-[4px] font-mono text-[10px] uppercase tracking-[0.06em] whitespace-nowrap shrink-0 ${cfg.pill}`}>
            <span className={`w-1.5 h-1.5 rounded-full flex-none ${cfg.dot}${cfg.pulse ? ' animate-pulse' : ''}`} />
            {cfg.label}
          </span>
        </div>

        {/* platform (mono, platform colour) + inline error/uploader — never hover-only */}
        <p className="text-[11px] mt-0.5 truncate leading-snug">
          <span className={`font-mono uppercase tracking-[0.06em] ${PLAT_TEXT[source]}`}>{PLAT_LABEL[source]}</span>
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

      {/* right-aligned mono telemetry readout */}
      <div className="hidden md:flex flex-col items-end justify-center gap-0.5 w-[132px] shrink-0 font-mono text-[10.5px] tabular-nums text-text-muted">
        {status === 'downloading' ? (
          <>
            <span className="text-text-secondary">{Math.round(pct)}%</span>
            <span><span className="text-accent">{fmtSpeed(speed)}</span> · {fmtEta(eta)}</span>
          </>
        ) : (downloaded > 0 || total > 0) ? (
          <span>{formatBytes(downloaded)}{total > 0 && ` / ${formatBytes(total)}`}</span>
        ) : null}
        <span className="uppercase opacity-70">{formatDuration(task.duration)} · {format || '—'}</span>
      </div>

      {/* actions — always visible for the row's state */}
      <div className="flex items-center gap-0.5 shrink-0">
        {PAUSABLE.has(status) && (
          <IconButton icon={Pause} label="Pause" size={14} onClick={onPause} className="w-7 h-7 rounded" />
        )}
        {isPaused && (
          <IconButton icon={Play} label="Resume" size={14} onClick={onResume} className="w-7 h-7 rounded text-accent" />
        )}
        {isActive && (
          <IconButton icon={X} label="Cancel" size={14} onClick={onCancel} className="w-7 h-7 rounded" />
        )}
        {isFailed && (
          <IconButton icon={RotateCcw} label="Retry" size={14} onClick={onRetry} className="w-7 h-7 rounded text-error hover:bg-error/10" />
        )}
        {isDone && (
          <IconButton icon={FolderOpen} label="Show in folder" size={14} onClick={onOpen} className="w-7 h-7 rounded" />
        )}
        {TERMINAL.has(status) && (
          <IconButton icon={Trash2} label="Remove" size={14} onClick={onRemove} className="w-7 h-7 rounded" />
        )}
      </div>

      {/* 2px progress hairline at the row's base — shared ProgressBar keeps the
          progressbar ARIA; overridden to a flush 2px trace */}
      <ProgressBar
        percent={pct}
        status={status}
        label={`${title || 'Download'} progress`}
        className="absolute bottom-0 left-0 right-0 !h-0.5 !rounded-none"
      />
    </motion.div>
  );
}, (a, b) => a.task === b.task && a.selected === b.selected);

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
      // leave the failed card in place so the user can retry again
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

  return (
    <div className="flex flex-col h-full min-h-0 w-full max-w-[1080px] mx-auto px-10">

      {/* ── page header ── */}
      <div className="flex items-end gap-4 pt-10 pb-5 flex-none">
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
            <Button variant="ghost" size="sm" icon={Trash2} onClick={clearCompletedAndPersist}>
              Clear completed
            </Button>
          )}
          {counts.active > 0 && (
            <Button
              variant="ghost" size="sm" icon={XCircle}
              className={confirmCancelAll ? 'text-error border-error/40' : ''}
              onClick={handleCancelAll}
              aria-live="polite"
            >
              {confirmCancelAll ? `Really cancel ${counts.active}?` : 'Cancel all'}
            </Button>
          )}
        </div>
      </div>

      {/* ── filter chips + select-all ── */}
      <div className="flex items-center gap-2 pb-3 flex-none">
        <button
          onClick={() => allSelected ? clearDownloadSelection() : selectAllDownloads(filtered.map(t => t.taskId))}
          className="no-drag grid place-items-center w-6 h-6 ml-[17px] mr-2 text-text-muted hover:text-accent transition-colors"
          aria-label="Toggle select all"
        >
          {allSelected
            ? <CheckSquare size={15} className="text-accent" />
            : someSelected
            ? <Minus size={15} />
            : <Square size={15} />}
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
        <div role="status" className="flex items-center gap-3 mb-3 px-4 py-3 rounded-lg border border-warning/30 bg-warning/10 flex-none">
          <Pause size={15} className="text-warning shrink-0" />
          <p className="text-[12.5px] text-text-primary flex-1 min-w-0">
            <b className="font-mono tabular-nums">{interrupted.length}</b>
            {` download${interrupted.length === 1 ? '' : 's'} interrupted when the app closed — finish ${interrupted.length === 1 ? 'it' : 'them'} now?`}
          </p>
          <Button variant="primary" size="sm" icon={Play} onClick={resumeInterrupted}>
            Resume
          </Button>
          <Button variant="ghost" size="sm" icon={Trash2} onClick={discardInterrupted}>
            Discard
          </Button>
        </div>
      )}

      {/* ── bulk-action bar (appears with a selection) ── */}
      {selectedIds.size > 0 && (
        <div className="flex items-center gap-2 pb-3 flex-none">
          <span className="font-mono text-[11px] tabular-nums text-text-secondary mr-1">
            {selectedIds.size} selected
          </span>
          <Button variant="ghost" size="sm" icon={XCircle} onClick={cancelSelected} disabled={!cancellableSelected}>
            Cancel
          </Button>
          <Button variant="ghost" size="sm" icon={Trash2} onClick={removeSelected} disabled={!removableSelected}>
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
          /* empty state */
          <div className="flex flex-col items-center justify-center h-56 gap-3 text-center select-none">
            <Download size={28} strokeWidth={1.5} className="text-text-muted" />
            <p className="text-text-secondary font-medium">No downloads yet</p>
            <p className="text-[13px] text-text-muted max-w-[30ch] leading-relaxed">
              Paste a link on any source tab and hit Download.
            </p>
          </div>
        ) : filtered.length === 0 ? (
          <div className="py-10 text-center text-[13px] text-text-muted">
            No {filter} downloads
          </div>
        ) : (
          <div className="rounded-lg bg-bg-surface border border-border-soft shadow-sm overflow-hidden divide-y divide-border-soft">
            <AnimatePresence initial={false}>
              {filtered.map(task => (
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
                      void window.electronAPI.shell.showItemInFolder(task.progress.filename);
                    }
                  }}
                  onRemove={() => removeTask(task.taskId)}
                />
              ))}
            </AnimatePresence>
          </div>
        )}
      </div>
    </div>
  );
}
