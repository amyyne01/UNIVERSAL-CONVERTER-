// Single-slot FIFO download queue + retry / cancel / pause machinery.
// See HOW-THE-APP-WORKS.md §7 (one-at-a-time queue, state machine) and §8
// (retries with exponential backoff, cancellation grace period).
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { isAbsolute } from 'node:path';
import type { DownloadTask, DownloadProgress, DownloadStatus } from '../shared/types.js';
import { canTransition } from './state-machine.js';
import { classifyError, type Downloader } from './downloader.js';
import { getMainWindow } from './window.js';

// ── Module-level singleton state (one slot, oldest-first) ──────────────────
const queue: DownloadTask[] = [];
let isDownloading = false;
let currentTaskId: string | null = null;
let currentTask: DownloadTask | null = null;
let downloader: Downloader | null = null;
let paused = false; // §14 "pause all": blocks the queue from starting new jobs.

/** Recently-seen tasks, most-recent first (§14 tray "recent downloads"). References
 *  stay live, so each entry's status reflects the task's current progress. */
const recent: DownloadTask[] = [];
const RECENT_CAP = 8;
/** onQueueChange subscribers — fired on every status change / enqueue / finish via emit(). */
const changeListeners = new Set<() => void>();
/** #26 onDownloadFailure subscribers — fired with the raw error text on every failure. */
const failureListeners = new Set<(error: string) => void>();

/** Pending retry timers, keyed by the retried task's id (§8: clearable). */
const retryTimers = new Map<string, ReturnType<typeof setTimeout>>();
/** Tasks cancelled within the grace window — their dying-process error is suppressed (§8). */
const recentlyCancelled = new Map<string, ReturnType<typeof setTimeout>>();

const MAX_RETRIES = 3;
const BACKOFF_MS = [5000, 15000, 45000]; // §8: 5s, 15s, 45s (each delay triples).
const NEXT_PAUSE_MS = 500; // ponytail: brief deliberate pause between jobs (§7).
const CANCEL_GRACE_MS = 3000; // window during which the killed process's error is expected.

// ── Persistent download history (hydrates the renderer across restarts) ─────
const TERMINAL = new Set<DownloadStatus>(['done', 'failed', 'cancelled']);
/** Every task this install has seen, insertion order, capped. Values are LIVE
 *  task references, so persisted snapshots always carry current progress. */
const history = new Map<string, DownloadTask>();
const HISTORY_CAP = 200;
let historyFile = '';
let persistTimer: ReturnType<typeof setTimeout> | null = null;

/** Load saved history and park interrupted (non-terminal) tasks as paused,
 *  back in the queue — yt-dlp resumes their .part files on a later resume. */
export function initHistory(file: string): void {
  historyFile = file;
  if (!existsSync(file)) return;
  try {
    const saved = JSON.parse(readFileSync(file, 'utf8')) as DownloadTask[];
    for (const raw of saved) {
      if (!raw?.taskId || !raw.progress) continue;
      // Restore THROUGH createTask rather than trusting the row verbatim. A file
      // written by an older build (or truncated mid-write) is still valid JSON but
      // can be missing fields the renderer reads unconditionally — a row with no
      // `format` threw on `format.toUpperCase()` and took the whole window with it.
      // Defaults are laid first, the saved values win on top; same shape as config.
      const base = createTask(raw);
      const t: DownloadTask = {
        ...base,
        ...raw,
        taskId: raw.taskId,
        progress: { ...base.progress, ...raw.progress },
      };
      if (!TERMINAL.has(t.progress.status)) {
        t.progress = { ...t.progress, status: 'paused', speed: 0, eta: 0, error: '' };
        t.interrupted = true; // renderer asks: finish it or erase it
        queue.push(t);
      }
      history.set(t.taskId, t);
    }
  } catch { /* corrupt history — start fresh */ }
}

/** Insertion-ordered snapshot for the renderer's boot hydration. */
export function getAllTasks(): DownloadTask[] {
  return [...history.values()];
}

/** Drop tasks from history (and the waiting queue). The active job is ignored. */
export function removeTasks(ids: string[]): boolean {
  for (const id of ids) {
    if (id === currentTaskId) continue;
    history.delete(id);
    const idx = queue.findIndex((t) => t.taskId === id);
    if (idx !== -1) queue.splice(idx, 1);
    // Mirror cancelTask()'s retry-timer branch — an uncleared timer would still fire
    // and re-queue a task whose history entry was just deleted (§B10).
    const timer = retryTimers.get(id);
    if (timer) { clearTimeout(timer); retryTimers.delete(id); }
  }
  schedulePersist();
  return true;
}

/** One immediate, non-debounced save — called at shutdown (§10 pattern). */
export function flushHistory(): void {
  if (!historyFile) return;
  if (persistTimer) { clearTimeout(persistTimer); persistTimer = null; }
  try { writeFileSync(historyFile, JSON.stringify([...history.values()])); } catch { /* best-effort */ }
}

function schedulePersist(): void {
  if (!historyFile || persistTimer) return;
  persistTimer = later(() => {
    persistTimer = null;
    flushHistory();
  }, 500);
}

function recordHistory(task: DownloadTask): void {
  history.set(task.taskId, task);
  if (history.size <= HISTORY_CAP) return;
  // Oldest-first, terminal entries only: finished work is the cheapest to forget.
  for (const [id, t] of history) {
    if (history.size <= HISTORY_CAP) break;
    if (TERMINAL.has(t.progress.status)) history.delete(id);
  }
  // ponytail: HISTORY_CAP is a soft cap by design. If nothing terminal is left to
  // evict, every remaining row is work in flight — active, waiting in `queue`, or
  // mid-retry-backoff (initHistory even re-queues interrupted tasks from the last
  // run) — and the only way to get back under the cap would be to delete downloads
  // the user still expects. An oversized history is the better failure. Upgrade
  // path if the file ever gets heavy: persist terminal rows trimmed, not dropped.
}

/** Notified when a download truly completes, with every file the engine wrote.
 *  Injected (rather than imported) so the queue owns no statistics logic. */
let onCompleted: ((task: DownloadTask, filepaths: string[]) => void) | null = null;
export function onDownloadCompleted(cb: (task: DownloadTask, filepaths: string[]) => void): void {
  onCompleted = cb;
}

export function setDownloader(d: Downloader): void {
  downloader = d;
}

// ── Task creation (§7: unique id, sensible defaults) ───────────────────────
export function createTask(partial: Partial<DownloadTask>): DownloadTask {
  return {
    taskId: `task_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`,
    url: partial.url ?? '',
    source: partial.source ?? 'unknown',
    outputDir: partial.outputDir ?? '',
    format: partial.format ?? 'mp3',
    quality: partial.quality ?? '320',
    videoQuality: partial.videoQuality ?? 'best',
    isAudioOnly: partial.isAudioOnly ?? true,
    isPlaylist: partial.isPlaylist ?? false,
    playlistName: partial.playlistName ?? '',
    embedThumbnail: partial.embedThumbnail ?? true,
    embedMetadata: partial.embedMetadata ?? true,
    skipExisting: partial.skipExisting ?? true,
    title: partial.title ?? '',
    thumbnailUrl: partial.thumbnailUrl ?? '',
    duration: partial.duration ?? 0,
    uploader: partial.uploader ?? '',
    // Stamped here so it survives into the persisted history: without it the
    // Downloads list can only ever group by status, and "when did I get this?"
    // is unanswerable. A restored task keeps the stamp it was created with.
    createdAt: partial.createdAt ?? Date.now(),
    progress: {
      status: 'queued', percent: 0, speed: 0, eta: 0,
      downloaded: 0, total: 0, filename: '', error: '',
      playlistIndex: 0, playlistTotal: 0,
    },
    ...(partial.track !== undefined ? { track: partial.track } : {}),
    ...(partial.retryCount !== undefined ? { retryCount: partial.retryCount } : {}),
    ...(partial.originalTaskId !== undefined ? { originalTaskId: partial.originalTaskId } : {}),
  };
}

export function enqueueTask(task: DownloadTask): void {
  queue.push(task);
  recordRecent(task);
  recordHistory(task);
  emit('download:queued', task.taskId, task);
  schedulePersist();
  processNextQueueItem();
}

// ── Per-task pause / resume ─────────────────────────────────────────────────
/** Pause one task. Active job: kill the process (its .part file stays for
 *  --continue), park it at the front of the queue, and free the slot so the
 *  next job starts. Waiting job: flip it to paused in place. */
export function pauseTask(taskId: string): boolean {
  if (isDownloading && currentTaskId === taskId && currentTask) {
    const task = currentTask;
    downloader?.cancel(taskId);
    markRecentlyCancelled(taskId);
    transition(task, 'paused');
    task.progress = { ...task.progress, speed: 0, eta: 0 };
    queue.unshift(task);
    isDownloading = false;
    currentTaskId = null;
    currentTask = null;
    emit('download:progress', task.taskId, task.progress);
    later(processNextQueueItem, NEXT_PAUSE_MS);
    return true;
  }
  const waiting = queue.find((t) => t.taskId === taskId);
  if (waiting && transition(waiting, 'paused')) {
    emit('download:progress', waiting.taskId, waiting.progress);
    return true;
  }
  return false;
}

/** Resume a paused task: back to queued, then let the queue pick it up. */
export function resumeTask(taskId: string): boolean {
  const task = queue.find((t) => t.taskId === taskId);
  if (!task || task.progress.status !== 'paused' || !transition(task, 'queued')) return false;
  task.interrupted = false; // the user chose to finish it
  emit('download:progress', task.taskId, task.progress);
  processNextQueueItem();
  return true;
}

// ── Ambient-layer accessors (§14: tray, presence, pause-all) ───────────────
/** Most-recent-first snapshot for the tray menu (live statuses, capped). */
export function getRecentDownloads(): { title: string; status: DownloadStatus }[] {
  return recent.map((t) => ({ title: t.title || t.url, status: t.progress.status }));
}

/** Subscribe to queue activity (status change / enqueue / finish). Returns unsubscribe. */
export function onQueueChange(cb: () => void): () => void {
  changeListeners.add(cb);
  return () => { changeListeners.delete(cb); };
}

/** #26: subscribe to raw download-failure text (before retry classification), so the
 *  engine updater can react to an extractor-stale signal. Returns unsubscribe. */
export function onDownloadFailure(cb: (error: string) => void): () => void {
  failureListeners.add(cb);
  return () => { failureListeners.delete(cb); };
}

/** True when there's active downloading or anything still queued (including paused
 *  tasks). The scheduled-OS-shutdown watcher uses this so the machine is never shut
 *  down while paused downloads remain in the queue. */
export function hasUnfinishedWork(): boolean {
  return isDownloading || queue.length > 0;
}

/** Pause everything: stop the active job (reuse cancel machinery — kill the process,
 *  suppress its dying error) and re-queue it at the front; block new starts (§8/§14). */
export function pauseAllDownloads(): void {
  if (paused) return;
  paused = true;
  if (isDownloading && currentTask) {
    const task = currentTask;
    downloader?.cancel(task.taskId);
    markRecentlyCancelled(task.taskId);
    transition(task, 'paused');
    queue.unshift(task); // resume restarts it from the front
    isDownloading = false;
    currentTaskId = null;
    currentTask = null;
    emit('download:progress', task.taskId, task.progress);
  }
}

/** Resume: flip paused jobs back to queued and pick processing back up. */
export function resumeAllDownloads(): void {
  if (!paused) return;
  paused = false;
  for (const t of queue) {
    if (t.progress.status === 'paused') {
      transition(t, 'queued');
      emit('download:progress', t.taskId, t.progress);
    }
  }
  processNextQueueItem();
}

// ── Cancellation (§8) ──────────────────────────────────────────────────────
export function cancelTask(taskId: string): boolean {
  // A scheduled-but-not-yet-started retry: just clear its timer.
  const timer = retryTimers.get(taskId);
  if (timer) {
    clearTimeout(timer);
    retryTimers.delete(taskId);
    // scheduleRetry() replaced the history entry with a fresh 'queued' task; without
    // this transition it would persist non-terminal and resurrect as a ghost on restart.
    const retried = history.get(taskId);
    if (retried) transition(retried, 'cancelled');
    emit('download:cancelled', taskId);
    return true;
  }
  // The active job: kill the process and suppress its dying error for a grace period.
  if (isDownloading && currentTaskId === taskId) {
    downloader?.cancel(taskId);
    markRecentlyCancelled(taskId);
    if (currentTask) transition(currentTask, 'cancelled');
    emit('download:cancelled', taskId);
    finishCurrent();
    return true;
  }
  // A merely-waiting job: remove it from the list.
  const idx = queue.findIndex((t) => t.taskId === taskId);
  if (idx !== -1) {
    const [task] = queue.splice(idx, 1);
    transition(task, 'cancelled');
    emit('download:cancelled', taskId);
    return true;
  }
  return false;
}

export function cancelAllTasks(): void {
  // Mid-backoff history entries must terminalize too, or they persist as 'queued'
  // and resurrect on next launch (mirrors cancelTask()'s retry-timer branch, §B4).
  for (const [id, timer] of retryTimers) {
    clearTimeout(timer);
    const retried = history.get(id);
    if (retried) transition(retried, 'cancelled');
    emit('download:cancelled', id);
  }
  retryTimers.clear();
  // Mark every waiting job cancelled (terminal) before dropping it, so history
  // persists it correctly and it doesn't resurrect as 'interrupted' on next launch.
  for (const t of queue) {
    transition(t, 'cancelled');
    emit('download:cancelled', t.taskId);
  }
  queue.length = 0;
  downloader?.cancelAll();
  if (currentTaskId) {
    markRecentlyCancelled(currentTaskId);
    if (currentTask) transition(currentTask, 'cancelled');
    emit('download:cancelled', currentTaskId);
  }
  isDownloading = false;
  currentTaskId = null;
  currentTask = null;
  schedulePersist();
}

// ── Processing: strictly one at a time, oldest first (§7) ───────────────────
function processNextQueueItem(): void {
  if (paused || isDownloading || !downloader) return;
  // Individually-paused tasks stay parked in the queue; take the first live one.
  const idx = queue.findIndex((t) => t.progress.status !== 'paused');
  if (idx === -1) return;
  const [task] = queue.splice(idx, 1);

  isDownloading = true;
  currentTaskId = task.taskId;
  currentTask = task;
  transition(task, 'fetching_info'); // queued → fetching_info
  emit('download:progress', task.taskId, task.progress);

  try {
    downloader.download(
      task,
      task.track,
      (p) => handleProgress(task, p),
      (t, filepath, filepaths) => handleDone(t, filepath, filepaths),
      (t, error) => handleError(t, error),
    );
  } catch (e) {
    // A synchronous throw (non-string outputDir, arg-build or spawn failure) would
    // otherwise leave the slot acquired forever — route it through the normal error
    // path so finishCurrent() frees the queue instead of wedging every later job.
    handleError(task, String(e));
  }
}

function handleProgress(task: DownloadTask, p: DownloadProgress): void {
  if (task.taskId !== currentTaskId) return; // ignore late callbacks from a finished job
  // Validate status changes through the state machine; on an illegal change keep
  // the current status but still surface the updated metrics.
  const status = transition(task, p.status) ? p.status : task.progress.status;
  task.progress = { ...p, status };
  emit('download:progress', task.taskId, task.progress);
}

function handleDone(task: DownloadTask, filepath: string, filepaths: string[] = []): void {
  if (task.taskId !== currentTaskId) return;
  // ponytail: process exit 0 is authoritative completion — force `done` even if the
  // last seen status was `downloading` (no post-processing markers were emitted).
  task.progress = { ...task.progress, status: 'done', percent: 100, filename: filepath || task.progress.filename };
  // Final size from disk — instant finishes (skip-existing) emit no progress lines.
  // Only a real absolute path is statable / usable for "show in folder"; a bare display
  // title (when yt-dlp emits no filepath) must not be statSync'd or offered to the shell.
  if (task.progress.filename && isAbsolute(task.progress.filename)) {
    try {
      const size = statSync(task.progress.filename).size;
      task.progress = { ...task.progress, downloaded: size, total: size };
    } catch { /* file moved or path unparsed — leave counters as reported */ }
  }
  task.completedAt = Date.now(); // forced 'done' skips transition()'s stamp too
  // Counted here, and only here: this is the one place a download is genuinely
  // finished. A collection writes many files and the engine reports each one, so
  // the count is files rather than tasks. Failed/cancelled never reach this line,
  // and a retry only arrives once — on the attempt that actually completed.
  const written = filepaths.length ? filepaths : (task.progress.filename ? [task.progress.filename] : []);
  onCompleted?.(task, written);
  emit('download:done', task.taskId, task.progress);
  schedulePersist(); // forced 'done' bypasses transition() — persist explicitly
  finishCurrent();
}

function handleError(task: DownloadTask, error: string): void {
  if (task.taskId !== currentTaskId) return;
  if (recentlyCancelled.has(task.taskId)) { finishCurrent(); return; } // expected dying-process error
  for (const cb of failureListeners) cb(error);
  const retryCount = task.retryCount ?? 0;
  if (classifyError(error) === 'retryable' && retryCount < MAX_RETRIES) {
    scheduleRetry(task, error);
  } else {
    transition(task, 'failed');
    task.progress = { ...task.progress, status: 'failed', error };
    emit('download:error', task.taskId, error);
  }
  finishCurrent();
}

// ── Retry: exponential backoff, re-queued at the FRONT (§8) ────────────────
function scheduleRetry(task: DownloadTask, error: string): void {
  const prev = task.retryCount ?? 0;
  const delay = BACKOFF_MS[prev] ?? BACKOFF_MS[BACKOFF_MS.length - 1];
  transition(task, 'retrying');
  task.progress = { ...task.progress, status: 'retrying', error };
  emit('download:progress', task.taskId, task.progress);

  // Retry IN PLACE: same taskId, bumped count, fresh 'queued' progress. Reusing the
  // id means the succeeded retry reaches a terminal state under the original history
  // entry — no ghost stranded at 'retrying' to resurrect on next launch, no dup row.
  const retried = createTask({ ...task, retryCount: prev + 1, originalTaskId: task.originalTaskId ?? task.taskId });
  retried.taskId = task.taskId;
  history.set(retried.taskId, retried);
  // recent[] holds live references — swap in the fresh object or tray/Discord "recent"
  // stays frozen showing the old task stuck at 'retrying' for the rest of the session (§B9).
  const ri = recent.indexOf(task);
  if (ri !== -1) recent[ri] = retried;
  const timer = later(() => {
    retryTimers.delete(retried.taskId);
    queue.unshift(retried); // re-queue at the front so it resumes promptly
    emit('download:queued', retried.taskId, retried);
    processNextQueueItem();
  }, delay);
  retryTimers.set(retried.taskId, timer);
}

// ── Helpers ────────────────────────────────────────────────────────────────
function finishCurrent(): void {
  isDownloading = false;
  currentTaskId = null;
  currentTask = null;
  // The true idle moment: notify subscribers (e.g. the scheduled-shutdown drain-watcher)
  // now, not just via emit() calls made before isDownloading flipped (§ B1).
  for (const cb of changeListeners) { try { cb(); } catch { /* subscriber isolation */ } }
  later(processNextQueueItem, NEXT_PAUSE_MS); // pick up the next job after a brief pause (§7)
}

/** Apply a validated status change in place. Returns false (and leaves the task untouched)
 *  for an illegal transition; a same-status change is a no-op success. */
function transition(task: DownloadTask, next: DownloadStatus): boolean {
  const cur = task.progress.status;
  if (cur === next) return true;
  if (!canTransition(cur, next)) return false;
  task.progress = { ...task.progress, status: next };
  // Stamp the moment it settled. Done here rather than at each call site because
  // a task reaches its end through several paths (finished, failed, cancelled,
  // cancel-all) and only this one is common to all of them.
  if (TERMINAL.has(next)) task.completedAt = Date.now();
  schedulePersist(); // a real status change is exactly what history must capture
  return true;
}

function markRecentlyCancelled(taskId: string): void {
  const existing = recentlyCancelled.get(taskId);
  if (existing) clearTimeout(existing);
  recentlyCancelled.set(taskId, later(() => recentlyCancelled.delete(taskId), CANCEL_GRACE_MS));
}

function emit(channel: string, ...args: unknown[]): void {
  getMainWindow()?.webContents.send(channel, ...args);
  // Every emit is a status change / enqueue / finish — the exact set onQueueChange fires on.
  for (const cb of changeListeners) { try { cb(); } catch { /* subscriber isolation — a listener fault must not corrupt queue state mid-mutation */ } }
  // Persistence is NOT triggered here — that would fsync twice a second through a
  // download's progress ticks. It's driven by status changes only (transition() +
  // enqueue/done/cancel), which is the set that actually needs to survive a restart.
}

// ponytail: records user-enqueued tasks; auto-retry copies aren't re-listed (same title).
function recordRecent(task: DownloadTask): void {
  const i = recent.indexOf(task);
  if (i !== -1) recent.splice(i, 1);
  recent.unshift(task);
  if (recent.length > RECENT_CAP) recent.length = RECENT_CAP;
}

/** setTimeout that won't keep the process alive on its own (Electron's loop stays up regardless). */
function later(fn: () => void, ms: number): ReturnType<typeof setTimeout> {
  const t = setTimeout(fn, ms);
  t.unref?.();
  return t;
}
