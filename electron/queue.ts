// N-slot FIFO download queue + retry / cancel / pause machinery.
// See HOW-THE-APP-WORKS.md §7 (the queue and its state machine) and §8
// (retries with exponential backoff, cancellation grace period).
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { isAbsolute } from 'node:path';
import type { DownloadTask, DownloadProgress, DownloadStatus } from '../shared/types.js';
import { canTransition } from './state-machine.js';
import { classifyError, explainError, needsClientFallback, type Downloader } from './downloader.js';
import { getMainWindow } from './window.js';

// ── Module-level singleton state (N slots, oldest-first) ───────────────────
const queue: DownloadTask[] = [];
/** Jobs currently holding a slot, keyed by taskId. Values are the LIVE task
 *  objects, and every downloader callback is admitted by REFERENCE equality
 *  (active.get(id) === task), never by id alone: scheduleRetry reuses a taskId
 *  with a fresh object while a killed process's closure still holds the old one,
 *  so an id-only guard would admit a dead proc.
 *
 *  Pause→resume is the case reference equality does NOT cover: it re-queues the
 *  SAME object, so a stale closure holding it still matches. What protects that
 *  path is killing the proc before parking the task, plus (for a Spotify bridge
 *  pre-pass, where no proc exists yet) the per-attempt token in downloader.ts. */
const active = new Map<string, DownloadTask>();
/** Effective slot count. Never derived here from config or the plan — ipc.ts
 *  pushes the already-clamped value in at boot, on config change, on plan flip. */
let maxSlots = 1;
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

/** Drop tasks from history (and the waiting queue). Active jobs are ignored —
 *  every one of them, not just the first: deleting a live row mid-download
 *  corrupts insertion order when its next transition() re-inserts it. */
export function removeTasks(ids: string[]): boolean {
  for (const id of ids) {
    if (active.has(id)) continue;
    history.delete(id);
    const idx = queue.findIndex((t) => t.taskId === id);
    if (idx !== -1) queue.splice(idx, 1);
    // Mirror cancelTask()'s retry-timer branch — an uncleared timer would still fire
    // and re-queue a task whose history entry was just deleted (§B10).
    const timer = retryTimers.get(id);
    if (timer) { clearTimeout(timer); retryTimers.delete(id); }
  }
  // Removing the last parked row by hand reaches the same wedge cancelAll does.
  clearPauseIfNothingParked();
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

/** Set how many downloads may run at once. The caller owns the tier clamp
 *  (clampConcurrency); the queue only schedules. Raising the cap fills the new
 *  slots from the queue immediately; lowering it never kills a running job — it
 *  only blocks new starts and lets `active` drain by attrition. */
export function setMaxSlots(n: number): void {
  maxSlots = Math.max(1, Math.floor(n));
  processNextQueueItem();
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
    // playlistLimit is the basic tier's collection cap. It was being dropped here —
    // ipc.ts computed it and handed it in, createTask never copied it out, so
    // buildYtDlpArgs never saw it and --playlist-end was never passed. The free
    // ceiling silently did nothing on every collection download.
    ...(partial.playlistLimit !== undefined ? { playlistLimit: partial.playlistLimit } : {}),
    ...(partial.fallbackProfile !== undefined ? { fallbackProfile: partial.fallbackProfile } : {}),
    // Same trap playlistLimit fell into: assembled by ipc.ts and handed in, so it
    // has to be copied out here or buildYtDlpArgs never sees a single extra.
    ...(partial.extras !== undefined ? { extras: partial.extras } : {}),
  };
}

export function enqueueTask(task: DownloadTask): void {
  // Enqueued while the whole queue is paused: park it as 'paused' rather than
  // leaving it 'queued'. Two reasons, one of them a bug.
  //   Honesty — 'queued' promises "about to start", and it is not.
  //   Reachability — the renderer infers "everything is paused" from row statuses
  //   (pausedCount > 0 && liveCount === 0). A 'queued' row makes that false, so
  //   the control renders as "Pause all", whose click hits the `if (paused)`
  //   guard and does nothing: the user cannot resume the queue they paused.
  // Resume flips every parked row back, so this costs nothing on that path.
  if (paused) transition(task, 'paused');
  queue.push(task);
  recordRecent(task);
  recordHistory(task);
  emit('download:queued', task.taskId, task);
  schedulePersist();
  processNextQueueItem();
}

// ── Per-task pause / resume ─────────────────────────────────────────────────
/** Pause one task. Active job: kill the process (its .part file stays for
 *  --continue), park it at the front of the queue, and free ITS slot so the next
 *  waiting job starts — the other running jobs are untouched. Waiting job: flip
 *  it to paused in place. */
export function pauseTask(taskId: string): boolean {
  const task = active.get(taskId);
  if (task) {
    downloader?.cancel(taskId);
    markRecentlyCancelled(taskId);
    transition(task, 'paused');
    task.progress = { ...task.progress, speed: 0, eta: 0 };
    queue.unshift(task);
    active.delete(taskId);
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

// ── Reordering the pending slice (§7) ──────────────────────────────────────
/** Reorder the QUEUED tasks inside `queue` into the order `taskIds` gives.
 *  Only waiting work can move: a job that has already STARTED lives in `active`,
 *  never in `queue`, so it is structurally impossible to reorder one ahead of
 *  itself. Individually-paused tasks keep the exact slots they sit in — the
 *  queued tasks are written back into the queued slots only, so a reorder can
 *  neither jump a task past a paused one nor wake it.
 *  Hostile/stale input is dropped, never rejected: ids that are unknown,
 *  duplicated, or no longer queued (a row can finish mid-drag) are skipped, and
 *  queued tasks the caller didn't name keep their relative order behind the
 *  ones it did. */
export function reorderQueue(taskIds: string[]): boolean {
  const slots: number[] = [];
  for (let i = 0; i < queue.length; i++) {
    if (queue[i].progress.status === 'queued') slots.push(i);
  }
  if (slots.length < 2) return true; // zero or one mover — nothing can change
  const pending = slots.map((i) => queue[i]);
  const named: DownloadTask[] = [];
  const seen = new Set<string>();
  for (const id of taskIds) {
    if (seen.has(id)) continue;
    const t = pending.find((x) => x.taskId === id);
    if (!t) continue;
    seen.add(id);
    named.push(t);
  }
  const ordered = [...named, ...pending.filter((t) => !seen.has(t.taskId))];
  slots.forEach((slot, i) => { queue[slot] = ordered[i]; });
  return true;
}

/** "Do this next": move one queued task to the front of the pending slice.
 *  Returns false when the task isn't waiting any more (already running, gone). */
export function promoteTask(taskId: string): boolean {
  const task = queue.find((t) => t.taskId === taskId);
  if (!task || task.progress.status !== 'queued') return false;
  reorderQueue([taskId]);
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
  return active.size > 0 || queue.length > 0;
}

/** Pause everything: stop every active job (reuse cancel machinery — kill the
 *  process, suppress its dying error) and re-queue them at the front; block new
 *  starts (§8/§14). */
export function pauseAllDownloads(): void {
  if (paused) return;
  paused = true;
  const parked = [...active.values()]; // insertion order = the order they started
  for (const task of parked) {
    downloader?.cancel(task.taskId);
    markRecentlyCancelled(task.taskId);
    transition(task, 'paused');
    emit('download:progress', task.taskId, task.progress);
  }
  // ONE unshift, not one per task: unshifting in a loop reverses their relative
  // order, so resumeAllDownloads would restart them back to front.
  if (parked.length) queue.unshift(...parked); // resume restarts them from the front
  active.clear();
}

/** The global pause exists to HOLD PARKED WORK. Once none remains — cancelled,
 *  removed, or drained — the flag has to clear, or the queue stays wedged for the
 *  rest of the session: processNextQueueItem() returns immediately on `paused`, so
 *  every newly pasted link sits at 'queued' forever.
 *
 *  It is unreachable rather than merely stuck, which is what makes it worth a
 *  guard: the renderer INFERS "everything is paused" from row statuses
 *  (`pausedCount > 0 && liveCount === 0`), so with zero paused rows the button
 *  reads "Pause all" and its click hits pauseAllDownloads' `if (paused) return`.
 *  Only a restart clears it.
 *
 *  ponytail: clearing the flag is smaller than publishing the engine's paused
 *  state over IPC and fixes every route into the wedge. If the renderer ever needs
 *  to DISPLAY "queue paused" on its own, publish the flag then. */
function clearPauseIfNothingParked(): void {
  if (paused && !queue.some((t) => t.progress.status === 'paused')) paused = false;
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
  // An active job: kill its process and suppress its dying error for a grace
  // period. Exactly one slot is freed; sibling jobs keep running.
  const running = active.get(taskId);
  if (running) {
    downloader?.cancel(taskId);
    markRecentlyCancelled(taskId);
    transition(running, 'cancelled');
    emit('download:cancelled', taskId);
    finishTask(taskId);
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
  // Cancel-all after pause-all destroys every parked row, which is the shortest
  // route into the wedge: the flag would survive with nothing left to reveal a
  // Resume control.
  clearPauseIfNothingParked();
  // Mark EVERY active id before the kill, one per slot. With several processes
  // dying at once, an unmarked one passes handleError's recentlyCancelled check
  // and paints a red "yt-dlp exited with code 1" row AFTER the user cancelled.
  for (const [id, task] of active) {
    markRecentlyCancelled(id);
    transition(task, 'cancelled');
    emit('download:cancelled', id);
  }
  downloader?.cancelAll();
  active.clear();
  schedulePersist();
}

// ── Processing: up to maxSlots at a time, oldest first (§7) ─────────────────
function processNextQueueItem(): void {
  // Occupancy is re-read on every iteration and never precomputed as a "free
  // slots" count: two jobs finishing within NEXT_PAUSE_MS queue two pumps, both
  // fire, and a count cached at entry would let the second one over-start.
  while (!paused && active.size < maxSlots) {
    const dl = downloader;
    if (!dl) return;
    // Individually-paused tasks stay parked in the queue; take the first live one.
    const idx = queue.findIndex((t) => t.progress.status !== 'paused');
    if (idx === -1) return;
    const [task] = queue.splice(idx, 1);

    active.set(task.taskId, task);
    transition(task, 'fetching_info'); // queued → fetching_info
    emit('download:progress', task.taskId, task.progress);

    try {
      dl.download(
        task,
        task.track,
        (p) => handleProgress(task, p),
        (t, filepath, filepaths) => handleDone(t, filepath, filepaths),
        (t, error) => handleError(t, error),
      );
    } catch (e) {
      // A synchronous throw (non-string outputDir, arg-build or spawn failure) would
      // otherwise hold the slot forever — route it through the normal error path so
      // finishTask() frees it. Caught PER ITERATION on purpose: a throw starting job
      // 2 of a burst must not abort filling slot 3.
      handleError(task, String(e));
    }
  }
}

function handleProgress(task: DownloadTask, p: DownloadProgress): void {
  if (active.get(task.taskId) !== task) return; // late callback from a killed/finished proc
  // Validate status changes through the state machine; on an illegal change keep
  // the current status but still surface the updated metrics.
  const status = transition(task, p.status) ? p.status : task.progress.status;
  task.progress = { ...p, status };
  emit('download:progress', task.taskId, task.progress);
}

function handleDone(task: DownloadTask, filepath: string, filepaths: string[] = []): void {
  if (active.get(task.taskId) !== task) return;
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
  finishTask(task.taskId);
}

function handleError(task: DownloadTask, error: string): void {
  if (active.get(task.taskId) !== task) return;
  if (recentlyCancelled.has(task.taskId)) { finishTask(task.taskId); return; } // expected dying-process error
  for (const cb of failureListeners) cb(error);
  const retryCount = task.retryCount ?? 0;
  if (classifyError(error) === 'retryable' && retryCount < MAX_RETRIES) {
    scheduleRetry(task, error);
  } else {
    transition(task, 'failed');
    // Raw stderr went straight to the row until now — a user reading
    // "ERROR: unable to download video data: HTTP Error 403: Forbidden" learns
    // nothing they can act on. The listeners above still get the raw text, because
    // isEngineStale() matches on it.
    const shown = explainError(error);
    task.progress = { ...task.progress, status: 'failed', error: shown };
    emit('download:error', task.taskId, shown);
  }
  finishTask(task.taskId);
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
  // A retry that repeats the failed command byte for byte is only useful when the
  // failure was the network. The PO-token gate (§5) is not — it reproduces exactly,
  // three times, and then shows a red row. Latch the fallback client set instead, so
  // the retry is a genuinely different attempt and a gated video lands as a file.
  const retried = createTask({
    ...task,
    retryCount: prev + 1,
    originalTaskId: task.originalTaskId ?? task.taskId,
    // Only ever set to true — `|| undefined` keeps a plain network retry from
    // writing a meaningless `"fallbackProfile": false` into persisted history.
    fallbackProfile: task.fallbackProfile || needsClientFallback(error) || undefined,
  });
  retried.taskId = task.taskId;
  history.set(retried.taskId, retried);
  // recent[] holds live references — swap in the fresh object or tray/Discord "recent"
  // stays frozen showing the old task stuck at 'retrying' for the rest of the session (§B9).
  const ri = recent.indexOf(task);
  if (ri !== -1) recent[ri] = retried;
  const timer = later(() => {
    retryTimers.delete(retried.taskId);
    // Front of the queue, not a reserved slot: if every slot is busy when the
    // timer fires, the retry WAITS there. It owned a slot when it failed, but that
    // slot was refilled long ago, and a retry must not push past maxSlots.
    queue.unshift(retried);
    emit('download:queued', retried.taskId, retried);
    processNextQueueItem();
  }, delay);
  retryTimers.set(retried.taskId, timer);
}

// ── Helpers ────────────────────────────────────────────────────────────────
/** Free ONE slot. The delete MUST precede the notification: the scheduled-shutdown
 *  drain-watcher reads hasUnfinishedWork() from this callback, and notifying first
 *  leaves the last job's slot still looking occupied, so the shutdown never fires
 *  (§B1, now per-slot). */
function finishTask(taskId: string): void {
  active.delete(taskId);
  for (const cb of changeListeners) { try { cb(); } catch { /* subscriber isolation */ } }
  // Deferred, never synchronous: a sync pump would re-enter a fill loop that is
  // still mid-iteration and let two frames both observe the same free slot (§7).
  later(processNextQueueItem, NEXT_PAUSE_MS); // pick up the next job after a brief pause
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
