import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import type { DownloadStatus } from '../shared/types.js';
import { BASIC_LIMITS, MAX_CONCURRENT_DOWNLOADS, clampConcurrency } from '../shared/types.js';

// Mocks per testing-vitest §7b — same specifiers queue.ts imports.
vi.mock('./window.js', () => ({
  getMainWindow: vi.fn(() => ({ webContents: { send: vi.fn() } })),
}));

vi.mock('./downloader.js', () => ({
  classifyError: vi.fn(() => 'permanent'),
  // The retry path reads both of these. needsClientFallback decides whether the
  // retry swaps player clients (the PO-token gate); explainError is what the row
  // finally shows. Defaults here keep every existing case on the raw-text path.
  needsClientFallback: vi.fn(() => false),
  explainError: vi.fn((t: string) => t),
}));

vi.mock('node:fs', () => {
  const existsSync = vi.fn(() => false);
  const readFileSync = vi.fn();
  const writeFileSync = vi.fn();
  const statSync = vi.fn(() => ({ size: 0 }));
  return { default: { existsSync, readFileSync, writeFileSync, statSync }, existsSync, readFileSync, writeFileSync, statSync };
});

// queue.ts is a module-level singleton (N slots, shared history/timers). Each test
// gets a pristine instance via resetModules + a fresh dynamic import, with fake timers
// to drive the backoff / grace / next-job delays deterministically.
type Queue = typeof import('./queue');
let Q: Queue;
let classify: ReturnType<typeof vi.fn>;
let gate: ReturnType<typeof vi.fn>;
let existsSyncMock: ReturnType<typeof vi.fn>;
let readFileSyncMock: ReturnType<typeof vi.fn>;
let writeFileSyncMock: ReturnType<typeof vi.fn>;
let statSyncMock: ReturnType<typeof vi.fn>;

// A controllable downloader: download() records its callbacks so a test can drive
// onDone / onError / onProgress; cancel / cancelAll are plain spies.
let dl: { download: ReturnType<typeof vi.fn>; cancel: ReturnType<typeof vi.fn>; cancelAll: ReturnType<typeof vi.fn> };
const lastCall = () => dl.download.mock.calls.at(-1)!;
const onDoneOf = () => lastCall()[3] as (t: unknown, filepath: string) => void;
const onErrorOf = () => lastCall()[4] as (t: unknown, error: string) => void;
// With several jobs in flight, "the last one started" is no longer "the one I mean" —
// these address a specific slot by the order its download() call was made.
const callAt = (i: number) => dl.download.mock.calls[i]!;
const onProgressAt = (i: number) => callAt(i)[2] as (p: unknown) => void;
const onDoneAt = (i: number) => callAt(i)[3] as (t: unknown, filepath: string) => void;
const onErrorAt = (i: number) => callAt(i)[4] as (t: unknown, error: string) => void;
const startedIds = () => dl.download.mock.calls.map((x) => x[0].taskId as string);
/** Create + enqueue n tasks titled A, B, C… and hand them back in that order. */
const enqueueMany = (n: number) =>
  Array.from({ length: n }, (_, i) => {
    const t = Q.createTask({ url: `u${i}`, title: String.fromCharCode(65 + i) });
    Q.enqueueTask(t);
    return t;
  });
const statusOf = (id: string): DownloadStatus | undefined =>
  Q.getAllTasks().find((t) => t.taskId === id)?.progress.status;

beforeEach(async () => {
  vi.resetModules();
  vi.clearAllMocks(); // mock module instances persist across resetModules — reset their call history
  vi.useFakeTimers();
  const fs = await import('node:fs');
  existsSyncMock = vi.mocked(fs.existsSync);
  readFileSyncMock = vi.mocked(fs.readFileSync);
  writeFileSyncMock = vi.mocked(fs.writeFileSync);
  statSyncMock = vi.mocked(fs.statSync);
  existsSyncMock.mockReturnValue(false);
  statSyncMock.mockReturnValue({ size: 0 } as never);
  const dlMod = await import('./downloader.js');
  classify = vi.mocked(dlMod.classifyError);
  classify.mockReturnValue('permanent');
  gate = vi.mocked(dlMod.needsClientFallback);
  gate.mockReturnValue(false);
  vi.mocked(dlMod.explainError).mockImplementation((t: string) => t);
  Q = await import('./queue');
  dl = { download: vi.fn(), cancel: vi.fn(), cancelAll: vi.fn() };
  Q.setDownloader(dl as never);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('createTask', () => {
  it('creates a task with a unique taskId and queued status', () => {
    const t1 = Q.createTask({ url: 'https://youtube.com/watch?v=1' });
    const t2 = Q.createTask({ url: 'https://youtube.com/watch?v=2' });
    expect(t1.taskId).not.toBe(t2.taskId);
    expect(t1.progress.status).toBe('queued');
  });

  // Without a stamp the Downloads list can only group by status, and "when did I
  // get this?" is unanswerable — including for anything built on the history later.
  it('stamps createdAt, and keeps the original stamp on a restored task', () => {
    const fresh = Q.createTask({ url: 'https://youtube.com/watch?v=1' });
    expect(typeof fresh.createdAt).toBe('number');
    expect(fresh.createdAt).toBeGreaterThan(0);
    expect(fresh.completedAt).toBeUndefined();

    const restored = Q.createTask({ url: 'https://youtube.com/watch?v=1', createdAt: 1_700_000_000_000 });
    expect(restored.createdAt).toBe(1_700_000_000_000);
  });

  it('fills sensible default fallbacks', () => {
    const task = Q.createTask({ url: 'https://test.com' });
    expect(task.format).toBe('mp3');
    expect(task.quality).toBe('320');
    expect(task.isAudioOnly).toBe(true);
    expect(task.skipExisting).toBe(true);
  });

  it('allows field overrides', () => {
    const task = Q.createTask({ url: 'u', format: 'flac', quality: 'lossless' });
    expect(task.format).toBe('flac');
  });
});

// maxSlots defaults to 1, so every pre-existing suite below still describes the
// single-slot engine — concurrency is opt-in, pushed in from the IPC boundary.
describe('single-slot processing (the degenerate N = 1 case)', () => {
  it('runs strictly one job at a time; the next starts only after the first finishes', () => {
    Q.setMaxSlots(1);
    const a = Q.createTask({ url: 'a', title: 'A' });
    const b = Q.createTask({ url: 'b', title: 'B' });
    Q.enqueueTask(a);
    Q.enqueueTask(b);
    expect(dl.download).toHaveBeenCalledTimes(1);       // only A started
    expect(lastCall()[0].taskId).toBe(a.taskId);

    onDoneOf()(a, '');                                   // A completes (no filepath)
    vi.advanceTimersByTime(600);                         // NEXT_PAUSE_MS elapses
    expect(dl.download).toHaveBeenCalledTimes(2);        // now B starts
    expect(lastCall()[0].taskId).toBe(b.taskId);
    expect(statusOf(a.taskId)).toBe('done');
  });

  it('a synchronous throw in download() frees the slot instead of wedging the queue', () => {
    dl.download.mockImplementationOnce(() => { throw new Error('spawn failed'); });
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);                                    // throws → caught → handleError → slot freed
    expect(statusOf(a.taskId)).toBe('failed');

    const b = Q.createTask({ url: 'b', title: 'B' });
    Q.enqueueTask(b);                                    // slot is free → B starts immediately
    expect(dl.download).toHaveBeenCalledTimes(2);
    expect(lastCall()[0].taskId).toBe(b.taskId);
  });
});

describe('N-slot processing', () => {
  it('runs up to maxSlots jobs at once and holds the rest queued', () => {
    Q.setMaxSlots(3);
    const [a, b, c, d] = enqueueMany(4);
    expect(startedIds()).toEqual([a.taskId, b.taskId, c.taskId]); // oldest first
    expect(statusOf(d.taskId)).toBe('queued');                    // the 4th waits
  });

  it('a finishing job frees its slot and the next queued job starts after NEXT_PAUSE_MS', () => {
    Q.setMaxSlots(2);
    const [a, b, c] = enqueueMany(3);
    onDoneAt(0)(a, '');
    expect(dl.download).toHaveBeenCalledTimes(2);   // the refill is deferred, not synchronous
    vi.advanceTimersByTime(600);
    expect(dl.download).toHaveBeenCalledTimes(3);
    expect(lastCall()[0].taskId).toBe(c.taskId);
    expect(statusOf(b.taskId)).toBe('fetching_info'); // the sibling slot was never touched
  });

  it('a synchronous throw starting one job of a burst still fills the remaining slots', () => {
    // Paused, then resumed: that is what makes ONE fill loop start all three, which
    // is the only way the per-iteration catch can be told apart from a loop-level one.
    Q.setMaxSlots(3);
    Q.pauseAllDownloads();
    const [a, b, c] = enqueueMany(3);
    expect(dl.download).not.toHaveBeenCalled();
    dl.download.mockImplementation((t: { title: string }) => {
      if (t.title === 'B') throw new Error('spawn failed');
    });

    Q.resumeAllDownloads();
    expect(startedIds()).toEqual([a.taskId, b.taskId, c.taskId]); // C was still reached
    expect(statusOf(b.taskId)).toBe('failed');
    expect(statusOf(c.taskId)).toBe('fetching_info');
  });

  it('two jobs finishing in the same tick refill without double-starting anything', () => {
    Q.setMaxSlots(2);
    const [a, b, c, d] = enqueueMany(4);
    onDoneAt(0)(a, '');
    onDoneAt(1)(b, '');                 // two deferred pumps are now armed
    vi.advanceTimersByTime(600);        // both fire
    expect(startedIds()).toEqual([a, b, c, d].map((t) => t.taskId)); // each exactly once

    // …and the second pump must have re-read occupancy rather than trusting a free
    // count captured at entry: with both slots full again, a 5th job cannot start.
    Q.enqueueTask(Q.createTask({ url: 'e', title: 'E' }));
    expect(dl.download).toHaveBeenCalledTimes(4);
  });
});

describe('setMaxSlots', () => {
  it('raising the cap mid-session fills the new slots from the queue immediately', () => {
    const [a, b, c] = enqueueMany(3);
    expect(dl.download).toHaveBeenCalledTimes(1);
    Q.setMaxSlots(3);
    expect(startedIds()).toEqual([a.taskId, b.taskId, c.taskId]);
  });

  it('lowering the cap never kills a running job — it drains by attrition', () => {
    Q.setMaxSlots(3);
    const [a, b, c, d] = enqueueMany(4);
    expect(dl.download).toHaveBeenCalledTimes(3);

    Q.setMaxSlots(1); // e.g. a license refresh downgrades premium → basic mid-flight
    expect(dl.cancel).not.toHaveBeenCalled();
    expect(dl.cancelAll).not.toHaveBeenCalled();
    expect(statusOf(a.taskId)).toBe('fetching_info');

    onDoneAt(0)(a, '');
    vi.advanceTimersByTime(600);
    expect(dl.download).toHaveBeenCalledTimes(3); // 2 still running > the new cap
    onDoneAt(1)(b, '');
    vi.advanceTimersByTime(600);
    expect(dl.download).toHaveBeenCalledTimes(3); // 1 still running === the new cap
    onDoneAt(2)(c, '');
    vi.advanceTimersByTime(600);
    expect(lastCall()[0].taskId).toBe(d.taskId);  // drained to 0 → D finally starts
  });

  it('floors a nonsense slot count at 1 rather than starting nothing', () => {
    Q.setMaxSlots(0);
    enqueueMany(2);
    expect(dl.download).toHaveBeenCalledTimes(1);
  });
});

describe('clampConcurrency', () => {
  it('floors at 1, caps by tier, and coerces junk instead of rejecting it', () => {
    expect(clampConcurrency(1, 'basic')).toBe(1);
    expect(clampConcurrency(9, 'basic')).toBe(BASIC_LIMITS.maxConcurrent);
    expect(clampConcurrency(9, 'premium')).toBe(MAX_CONCURRENT_DOWNLOADS);
    expect(clampConcurrency(0, 'premium')).toBe(1);
    expect(clampConcurrency(-3, 'premium')).toBe(1);
    expect(clampConcurrency(2.9, 'premium')).toBe(2);
    expect(clampConcurrency(NaN, 'basic')).toBe(1);
    expect(clampConcurrency('4' as unknown as number, 'premium')).toBe(4);
  });
});

describe('late callbacks under concurrency', () => {
  it('rejects a dead proc\'s callback carrying the OLD task object under a reused id', () => {
    // scheduleRetry reuses the taskId with a FRESH object, so the id-based guard the
    // single-slot queue used would happily admit the killed attempt's progress ticks
    // and rewind the live download's percent.
    classify.mockReturnValue('retryable');
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    const staleProgress = onProgressAt(0);
    onErrorOf()(a, 'network error');
    vi.advanceTimersByTime(50000);
    expect(lastCall()[0]).not.toBe(a);          // the retry is a different object
    expect(lastCall()[0].taskId).toBe(a.taskId); // under the same id

    const cb = vi.fn();
    Q.onQueueChange(cb);
    staleProgress({ ...a.progress, status: 'downloading', percent: 5 });
    expect(cb).not.toHaveBeenCalled();           // dropped entirely, no emit
  });
});

describe('finishTask idle notification (B1)', () => {
  it('notifies subscribers after the slot is released, not just from the pre-finish emit', () => {
    const snapshots: boolean[] = [];
    Q.onQueueChange(() => snapshots.push(Q.hasUnfinishedWork()));
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    snapshots.length = 0;                                 // ignore enqueue-time notifications
    onDoneOf()(a, '');
    // Before the fix, the only notification fires from handleDone's emit() while
    // the slot still looks occupied — the drain-watcher never observes true idle.
    expect(snapshots.at(-1)).toBe(false);
  });

  it('stays busy until the LAST slot empties, not the first', () => {
    Q.setMaxSlots(2);
    const [a, b] = enqueueMany(2);
    const snapshots: boolean[] = [];
    Q.onQueueChange(() => snapshots.push(Q.hasUnfinishedWork()));

    onDoneAt(0)(a, '');
    expect(Q.hasUnfinishedWork()).toBe(true); // B still holds a slot
    expect(snapshots.at(-1)).toBe(true);

    onDoneAt(1)(b, '');
    expect(snapshots.at(-1)).toBe(false);     // only now is the machine idle
  });
});

describe('handleDone', () => {
  it('reads disk size only for an absolute filepath', () => {
    statSyncMock.mockReturnValue({ size: 4242 } as never);
    const a = Q.createTask({ url: 'a' });
    Q.enqueueTask(a);
    onDoneOf()(a, 'C:\\Music\\song.mp3');
    expect(statSyncMock).toHaveBeenCalled();
    const done = Q.getAllTasks().find((t) => t.taskId === a.taskId)!;
    expect(done.progress.status).toBe('done');
    expect(done.progress.downloaded).toBe(4242);
  });

  it('never stats a bare display title left in filename (skip-existing guard)', () => {
    statSyncMock.mockReturnValue({ size: 4242 } as never);
    const a = Q.createTask({ url: 'a', title: 'My Song' });
    Q.enqueueTask(a);
    lastCall()[0].progress = { ...lastCall()[0].progress, filename: 'My Song' };
    onDoneOf()(a, '');                                   // yt-dlp emitted no filepath
    expect(statSyncMock).not.toHaveBeenCalled();
    const done = Q.getAllTasks().find((t) => t.taskId === a.taskId)!;
    expect(done.progress.status).toBe('done');
    expect(done.progress.downloaded).toBe(0);            // counters untouched
  });
});

describe('retry with exponential backoff', () => {
  it('retries up to MAX_RETRIES with the same taskId, then fails', () => {
    classify.mockReturnValue('retryable');
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    const id = a.taskId;

    for (let i = 0; i < 4; i++) {
      const active = lastCall()[0];
      onErrorOf()(active, 'network timeout');
      vi.advanceTimersByTime(50000);                     // covers the longest 45s backoff + NEXT_PAUSE
    }

    expect(statusOf(id)).toBe('failed');                 // 3 retries exhausted, 4th error is terminal
    expect(dl.download).toHaveBeenCalledTimes(4);        // initial + 3 restarts, no 5th
  });

  it('a retryable error re-queues under the original taskId (no duplicate row)', () => {
    classify.mockReturnValue('retryable');
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    onErrorOf()(a, 'temporary failure');
    vi.advanceTimersByTime(50000);
    expect(Q.getAllTasks().filter((t) => t.taskId === a.taskId)).toHaveLength(1);
    expect(lastCall()[0].taskId).toBe(a.taskId);
  });

  it('a retry timer firing with every slot busy waits at the front instead of over-starting', () => {
    classify.mockReturnValue('retryable');
    Q.setMaxSlots(2);
    const [a, b, c] = enqueueMany(3);
    onErrorAt(0)(a, 'network error');   // A fails → retry scheduled, its slot freed
    vi.advanceTimersByTime(600);
    expect(lastCall()[0].taskId).toBe(c.taskId); // the freed slot went to C, 45s ago

    vi.advanceTimersByTime(50000);      // A's retry timer fires; B and C hold both slots
    expect(dl.download).toHaveBeenCalledTimes(3); // it waits — no reserved slot

    onDoneAt(1)(b, '');
    vi.advanceTimersByTime(600);
    expect(lastCall()[0].taskId).toBe(a.taskId);  // re-queued at the FRONT, so it goes first
  });

  it('swaps the stale reference in recent[] on retry so tray/Discord don\'t freeze at retrying (B9)', () => {
    classify.mockReturnValue('retryable');
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    onErrorOf()(a, 'network error');                     // schedules retry; old `a` ref is now 'retrying'
    const recentEntry = Q.getRecentDownloads().find((r) => r.title === 'A');
    expect(recentEntry?.status).toBe('queued');           // fresh retried task, not the stale 'retrying' ref
  });

  it('removeTasks clears a pending retry timer so the removed task cannot resurrect (B10)', () => {
    classify.mockReturnValue('retryable');
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    onErrorOf()(a, 'network error');                     // schedules a retry timer

    Q.removeTasks([a.taskId]);
    const before = dl.download.mock.calls.length;
    vi.advanceTimersByTime(60000);                        // uncleared timer would re-queue and start it
    expect(dl.download.mock.calls.length).toBe(before);
  });
});

describe('widened ErrorClass: auth/geo do not retry (#10/#46/#47 seam)', () => {
  it('does not retry an auth-classified error — fails immediately like permanent', () => {
    classify.mockReturnValue('auth');
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    onErrorOf()(a, 'Sign in to confirm your age');
    vi.advanceTimersByTime(50000);
    expect(statusOf(a.taskId)).toBe('failed');
    expect(dl.download).toHaveBeenCalledTimes(1); // no retry attempt made
  });

  it('does not retry a geo-classified error — fails immediately like permanent', () => {
    classify.mockReturnValue('geo');
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    onErrorOf()(a, 'Not available in your country');
    vi.advanceTimersByTime(50000);
    expect(statusOf(a.taskId)).toBe('failed');
    expect(dl.download).toHaveBeenCalledTimes(1);
  });
});

describe('cancelTask (all three branches)', () => {
  it('cancels a merely-waiting job', () => {
    const a = Q.createTask({ url: 'a', title: 'A' });
    const b = Q.createTask({ url: 'b', title: 'B' });
    Q.enqueueTask(a);                                    // A active
    Q.enqueueTask(b);                                    // B waiting
    expect(Q.cancelTask(b.taskId)).toBe(true);
    expect(statusOf(b.taskId)).toBe('cancelled');
    // Terminal states are stamped centrally in transition(), so cancelling counts
    // as settled just like finishing does.
    expect(typeof Q.getAllTasks().find((t) => t.taskId === b.taskId)?.completedAt).toBe('number');
  });

  it('cancels the active job, kills the process, and frees the slot', () => {
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    expect(Q.cancelTask(a.taskId)).toBe(true);
    expect(dl.cancel).toHaveBeenCalledWith(a.taskId);
    expect(statusOf(a.taskId)).toBe('cancelled');

    const b = Q.createTask({ url: 'b', title: 'B' });
    Q.enqueueTask(b);                                    // slot freed → B starts
    expect(dl.download).toHaveBeenCalledTimes(2);
    expect(lastCall()[0].taskId).toBe(b.taskId);
  });

  it('cancels ONE active job, freeing exactly one slot and leaving siblings running', () => {
    Q.setMaxSlots(2);
    const [a, b, c] = enqueueMany(3);
    expect(Q.cancelTask(a.taskId)).toBe(true);
    expect(dl.cancel).toHaveBeenCalledWith(a.taskId);
    expect(dl.cancel).toHaveBeenCalledTimes(1);       // only A's process was killed
    expect(dl.cancelAll).not.toHaveBeenCalled();
    expect(statusOf(b.taskId)).toBe('fetching_info'); // sibling untouched
    vi.advanceTimersByTime(600);
    expect(lastCall()[0].taskId).toBe(c.taskId);      // the one freed slot took C
    expect(dl.download).toHaveBeenCalledTimes(3);
  });

  it('cancels a task awaiting a retry timer, marking it terminal (no restart ghost)', () => {
    classify.mockReturnValue('retryable');
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    onErrorOf()(a, 'network error');                     // schedules a retry timer, history → 'queued'
    expect(statusOf(a.taskId)).toBe('queued');

    expect(Q.cancelTask(a.taskId)).toBe(true);
    expect(statusOf(a.taskId)).toBe('cancelled');        // persisted terminal, not resurrectable

    const before = dl.download.mock.calls.length;
    vi.advanceTimersByTime(60000);                        // the cleared timer must not fire
    expect(dl.download.mock.calls.length).toBe(before);
  });
});

describe('cancelAllTasks', () => {
  it('resets the slot and notifies subscribers so a fresh enqueue can start', () => {
    const cb = vi.fn();
    Q.onQueueChange(cb);
    Q.enqueueTask(Q.createTask({ url: 'a', title: 'A' }));
    Q.enqueueTask(Q.createTask({ url: 'b', title: 'B' }));
    cb.mockClear();

    Q.cancelAllTasks();
    expect(cb).toHaveBeenCalled();

    Q.enqueueTask(Q.createTask({ url: 'c', title: 'C' }));
    expect(lastCall()[0].title).toBe('C');               // slot reset → C started
  });

  it('cancels EVERY active job, not just one, and frees all their slots', () => {
    Q.setMaxSlots(3);
    const [a, b, c] = enqueueMany(3);
    const dying = [onErrorAt(0), onErrorAt(1), onErrorAt(2)];

    Q.cancelAllTasks();
    for (const t of [a, b, c]) expect(statusOf(t.taskId)).toBe('cancelled');
    expect(Q.hasUnfinishedWork()).toBe(false);

    // Every killed process reports a non-zero exit. Not one of them may surface as
    // a red failed row after the user pressed cancel-all — with the single-slot code
    // only the one tracked id was suppressed and the other N-1 painted failures.
    dying.forEach((onErr, i) => onErr([a, b, c][i], 'yt-dlp exited with code 1'));
    for (const t of [a, b, c]) expect(statusOf(t.taskId)).toBe('cancelled');

    Q.enqueueTask(Q.createTask({ url: 'd', title: 'D' }));
    expect(lastCall()[0].title).toBe('D');            // all slots reset
  });

  it('terminalizes a mid-backoff retry-timer entry instead of leaving it queued (B4)', () => {
    classify.mockReturnValue('retryable');
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    onErrorOf()(a, 'network error');                     // schedules a retry timer, history → 'queued'
    expect(statusOf(a.taskId)).toBe('queued');

    Q.cancelAllTasks();
    expect(statusOf(a.taskId)).toBe('cancelled');         // must not resurrect as paused/interrupted on restart

    const before = dl.download.mock.calls.length;
    vi.advanceTimersByTime(60000);                        // cleared timer must not fire
    expect(dl.download.mock.calls.length).toBe(before);
  });
});

describe('pause / resume', () => {
  it('pauseTask parks the active job; resumeTask restarts it', () => {
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    expect(Q.pauseTask(a.taskId)).toBe(true);
    expect(dl.cancel).toHaveBeenCalledWith(a.taskId);
    expect(statusOf(a.taskId)).toBe('paused');

    vi.advanceTimersByTime(600);                          // parked task is skipped, stays paused
    expect(statusOf(a.taskId)).toBe('paused');

    expect(Q.resumeTask(a.taskId)).toBe(true);
    expect(['queued', 'fetching_info']).toContain(statusOf(a.taskId));
  });

  it('pauseTask pauses a waiting job in place', () => {
    const a = Q.createTask({ url: 'a', title: 'A' });
    const b = Q.createTask({ url: 'b', title: 'B' });
    Q.enqueueTask(a);                                     // A active
    Q.enqueueTask(b);                                     // B waiting
    expect(Q.pauseTask(b.taskId)).toBe(true);
    expect(statusOf(b.taskId)).toBe('paused');
  });

  it('pauseTask parks ONE of several active jobs; the others run on and its slot refills', () => {
    Q.setMaxSlots(2);
    const [a, b, c] = enqueueMany(3);
    expect(Q.pauseTask(a.taskId)).toBe(true);
    expect(dl.cancel).toHaveBeenCalledWith(a.taskId);
    expect(dl.cancel).toHaveBeenCalledTimes(1);
    expect(statusOf(a.taskId)).toBe('paused');
    expect(statusOf(b.taskId)).toBe('fetching_info'); // sibling untouched

    vi.advanceTimersByTime(600);
    expect(lastCall()[0].taskId).toBe(c.taskId);      // parked A skipped, C takes the slot
  });

  it('pauseAllDownloads parks every active job, preserving their relative order', () => {
    Q.setMaxSlots(3);
    const [a, b, c] = enqueueMany(3);

    Q.pauseAllDownloads();
    expect(dl.cancel.mock.calls.map((x) => x[0])).toEqual([a.taskId, b.taskId, c.taskId]);
    for (const t of [a, b, c]) expect(statusOf(t.taskId)).toBe('paused');

    // One unshift, in insertion order. Unshifting per task reverses them, and resume
    // would restart C, B, A — a silently reordered queue.
    dl.download.mockClear();
    Q.resumeAllDownloads();
    expect(startedIds()).toEqual([a.taskId, b.taskId, c.taskId]);
  });

  it('pauseAllDownloads parks the active job; resumeAllDownloads re-queues it', () => {
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    Q.pauseAllDownloads();
    expect(dl.cancel).toHaveBeenCalledWith(a.taskId);
    expect(statusOf(a.taskId)).toBe('paused');

    Q.resumeAllDownloads();
    expect(['queued', 'fetching_info']).toContain(statusOf(a.taskId));
  });
});

describe('reordering the pending slice', () => {
  // Drain the job holding the slot so the queue starts the next one, and report
  // the order jobs actually started in.
  const finishTop = () => {
    const done = onDoneOf();
    done(lastCall()[0], 'C:\\out\\f.mp3');
    vi.advanceTimersByTime(600);
  };

  it('reorderQueue moves waiting jobs; a job that already STARTED cannot be moved ahead of itself', () => {
    const [a, b, c, d] = enqueueMany(4); // one slot: A runs, B/C/D wait
    expect(startedIds()).toEqual([a.taskId]);

    // A is running — naming it first must not put it back at the head of the queue.
    Q.reorderQueue([a.taskId, d.taskId, c.taskId]);
    finishTop();
    expect(startedIds()).toEqual([a.taskId, d.taskId]);
    finishTop();
    expect(startedIds()).toEqual([a.taskId, d.taskId, c.taskId]);
    finishTop();
    expect(startedIds()).toEqual([a.taskId, d.taskId, c.taskId, b.taskId]);
  });

  it('drops unknown and duplicated ids, and leaves unnamed jobs in queue order behind the named ones', () => {
    const [a, b, c, d] = enqueueMany(4);

    // A row can finish mid-drag, so a stale id is expected input, not an error:
    // the reorder still applies to everything that survived.
    Q.reorderQueue(['gone', d.taskId, d.taskId, 'also-gone']);
    finishTop(); finishTop(); finishTop();
    expect(startedIds()).toEqual([a.taskId, d.taskId, b.taskId, c.taskId]);
  });

  it('promoteTask puts a queued job at the front; a running or unknown id is refused', () => {
    const [a, b, c] = enqueueMany(3);
    expect(Q.promoteTask(c.taskId)).toBe(true);
    expect(Q.promoteTask(a.taskId)).toBe(false); // already started
    expect(Q.promoteTask('nope')).toBe(false);

    finishTop();
    expect(startedIds()).toEqual([a.taskId, c.taskId]);
    finishTop();
    expect(startedIds()).toEqual([a.taskId, c.taskId, b.taskId]);
  });

  it('never wakes or jumps a paused job — it keeps the slot it sits in', () => {
    const [a, b, c] = enqueueMany(3);
    Q.pauseTask(b.taskId); // paused in place, still in the queue array

    Q.reorderQueue([b.taskId, c.taskId]);
    finishTop();
    expect(startedIds()).toEqual([a.taskId, c.taskId]);
    expect(statusOf(b.taskId)).toBe('paused');
  });
});

describe('history persistence', () => {
  it('initHistory hydrates saved tasks and parks interrupted ones as paused', () => {
    existsSyncMock.mockReturnValue(true);
    readFileSyncMock.mockReturnValue(JSON.stringify([
      { taskId: 'done1', progress: { status: 'done', percent: 100 } },
      { taskId: 'mid1', progress: { status: 'downloading', percent: 40 } },
    ]));
    Q.initHistory('C:\\hist.json');
    const all = Q.getAllTasks();
    expect(all.map((t) => t.taskId).sort()).toEqual(['done1', 'mid1']);
    const mid = all.find((t) => t.taskId === 'mid1')!;
    expect(mid.progress.status).toBe('paused');
    expect(mid.interrupted).toBe(true);
  });

  // A history file written by an older build is still valid JSON but can lack
  // fields the renderer reads unconditionally. It used to reach the UI verbatim,
  // where `format.toUpperCase()` threw and unmounted the entire window.
  it('fills in every field a partial saved row is missing', () => {
    existsSyncMock.mockReturnValue(true);
    readFileSyncMock.mockReturnValue(JSON.stringify([
      { taskId: 'legacy', progress: { status: 'done' } },
    ]));
    Q.initHistory('C:\\hist.json');
    const t = Q.getAllTasks()[0];
    expect(t.taskId).toBe('legacy');
    expect(t.format).toBeTypeOf('string');
    expect(t.source).toBeTypeOf('string');
    expect(t.url).toBeTypeOf('string');
    expect(t.progress.filename).toBeTypeOf('string');
    expect(t.progress.percent).toBeTypeOf('number');
  });

  it('keeps the saved values of a complete row', () => {
    existsSyncMock.mockReturnValue(true);
    readFileSyncMock.mockReturnValue(JSON.stringify([
      { taskId: 'full', title: 'Kept', format: 'flac', progress: { status: 'done', percent: 100 } },
    ]));
    Q.initHistory('C:\\hist.json');
    const t = Q.getAllTasks()[0];
    expect(t.title).toBe('Kept');
    expect(t.format).toBe('flac');
    expect(t.progress.percent).toBe(100);
  });

  it('removeTasks drops entries and schedules a persist', () => {
    existsSyncMock.mockReturnValue(true);
    readFileSyncMock.mockReturnValue(JSON.stringify([
      { taskId: 'x', progress: { status: 'done' } },
      { taskId: 'y', progress: { status: 'failed' } },
    ]));
    Q.initHistory('C:\\hist.json');
    expect(Q.removeTasks(['x'])).toBe(true);
    expect(Q.getAllTasks().map((t) => t.taskId)).toEqual(['y']);
    vi.advanceTimersByTime(600);                          // debounced persist flushes
    expect(writeFileSyncMock).toHaveBeenCalled();
  });

  it('removeTasks skips EVERY active job, not just the first', () => {
    Q.setMaxSlots(2);
    const [a, b, c] = enqueueMany(3);           // A + B active, C waiting
    expect(Q.removeTasks([a.taskId, b.taskId, c.taskId])).toBe(true);
    // Deleting a live row mid-download strands it: its next transition() re-inserts
    // it without recordHistory, corrupting insertion order for the next hydration.
    expect(Q.getAllTasks().map((t) => t.taskId)).toEqual([a.taskId, b.taskId]);
  });

  it('flushHistory writes immediately', () => {
    existsSyncMock.mockReturnValue(true);
    readFileSyncMock.mockReturnValue(JSON.stringify([{ taskId: 'z', progress: { status: 'done' } }]));
    Q.initHistory('C:\\hist.json');
    Q.flushHistory();
    expect(writeFileSyncMock).toHaveBeenCalledTimes(1);
    expect(writeFileSyncMock.mock.calls[0][1]).toContain('"taskId":"z"');
  });

  it('evicts the oldest terminal entry once HISTORY_CAP is exceeded', () => {
    existsSyncMock.mockReturnValue(true);
    readFileSyncMock.mockReturnValue(JSON.stringify(
      Array.from({ length: 200 }, (_, i) => ({ taskId: `old${i}`, progress: { status: 'done' } })),
    ));
    Q.initHistory('C:\\hist.json');
    expect(Q.getAllTasks()).toHaveLength(200);

    Q.enqueueTask(Q.createTask({ url: 'new', title: 'NEW' }));
    const all = Q.getAllTasks();
    expect(all).toHaveLength(200);                        // capped
    expect(all.some((t) => t.title === 'NEW')).toBe(true);
    expect(all.some((t) => t.taskId === 'old0')).toBe(false); // oldest terminal evicted
  });
});

describe('subscribers', () => {
  it('onQueueChange fires on enqueue and stops after unsubscribe', () => {
    const cb = vi.fn();
    const off = Q.onQueueChange(cb);
    Q.enqueueTask(Q.createTask({ url: 'u', title: 'C' }));
    expect(cb).toHaveBeenCalled();
    off();
    cb.mockClear();
    Q.enqueueTask(Q.createTask({ url: 'u', title: 'D' }));
    expect(cb).not.toHaveBeenCalled();
  });

  it('isolates a throwing subscriber from queue mutation', () => {
    Q.onQueueChange(() => { throw new Error('listener boom'); });
    const a = Q.createTask({ url: 'a', title: 'A' });
    expect(() => Q.enqueueTask(a)).not.toThrow();         // fault swallowed
    expect(dl.download).toHaveBeenCalledTimes(1);         // enqueue still started the job
  });

  it('getRecentDownloads lists most-recent first', () => {
    Q.enqueueTask(Q.createTask({ url: 'u', title: 'A' }));
    Q.enqueueTask(Q.createTask({ url: 'u', title: 'B' }));
    expect(Q.getRecentDownloads()[0].title).toBe('B');
  });
});

// ── The PO-token gate (§5) ───────────────────────────────────────────────────
describe('retry after the PO-token gate', () => {
  it('latches fallbackProfile so the retry is a DIFFERENT attempt, not the same one', () => {
    classify.mockReturnValue('retryable');
    gate.mockReturnValue(true);
    const a = Q.createTask({ url: 'https://youtube.com/watch?v=x', title: 'Trailer' });
    Q.enqueueTask(a);

    onErrorOf()(lastCall()[0], 'ERROR: unable to download video data: HTTP Error 403: Forbidden');
    vi.advanceTimersByTime(50000);

    // Without this the queue re-ran the identical command three times and then
    // showed a red row — the failure reproduces exactly, so a plain retry is free
    // of any chance of succeeding.
    expect(lastCall()[0].fallbackProfile).toBe(true);
  });

  it('leaves an ordinary network retry alone — no silent quality downgrade', () => {
    classify.mockReturnValue('retryable');
    gate.mockReturnValue(false);
    const a = Q.createTask({ url: 'https://youtube.com/watch?v=y', title: 'Song' });
    Q.enqueueTask(a);

    onErrorOf()(lastCall()[0], 'ERROR: connection reset by peer');
    vi.advanceTimersByTime(50000);

    expect(lastCall()[0].fallbackProfile).toBeUndefined();
  });

  it('keeps the flag once set, so a later network blip cannot unlatch it', () => {
    classify.mockReturnValue('retryable');
    gate.mockReturnValue(true);
    const a = Q.createTask({ url: 'https://youtube.com/watch?v=z', title: 'Trailer' });
    Q.enqueueTask(a);
    onErrorOf()(lastCall()[0], 'HTTP Error 403: Forbidden');
    vi.advanceTimersByTime(50000);

    gate.mockReturnValue(false);
    onErrorOf()(lastCall()[0], 'ERROR: connection reset by peer');
    vi.advanceTimersByTime(50000);

    expect(lastCall()[0].fallbackProfile).toBe(true);
  });
});

describe('createTask field carry-through', () => {
  it('keeps playlistLimit — the basic tier cap was being dropped on the floor', () => {
    // ipc.ts computes this from BASIC_LIMITS and hands it in; createTask never
    // copied it out, so buildYtDlpArgs never saw it and --playlist-end was never
    // passed. The free ceiling silently did nothing on every collection download.
    expect(Q.createTask({ playlistLimit: 5 }).playlistLimit).toBe(5);
    expect(Q.createTask({}).playlistLimit).toBeUndefined();
  });
});

// ── Regression: the global pause must not outlive the work it parks ──────────
// Found in adversarial review. processNextQueueItem() returns immediately while
// `paused` is set, and the renderer infers "everything is paused" from ROW
// STATUSES — so with zero paused rows the button reads "Pause all" and its click
// hits the `if (paused) return` guard. The queue is then wedged for the rest of
// the session with no reachable control, and only a restart clears it.
//
// Each case asserts through the OBSERVABLE symptom — does a freshly enqueued task
// actually start — rather than by reaching for the private flag.
describe('the global pause never outlives its parked work', () => {
  /** Enqueue one more task and report whether the engine started it. */
  const startsNewWork = (): boolean => {
    const fresh = Q.createTask({ url: 'fresh', title: 'Fresh' });
    Q.enqueueTask(fresh);
    vi.runOnlyPendingTimers(); // the pump is deferred, never synchronous
    return startedIds().includes(fresh.taskId);
  };

  it('clears the pause when cancel-all destroys every parked row', () => {
    Q.setMaxSlots(2);
    enqueueMany(2);
    vi.runOnlyPendingTimers();
    Q.pauseAllDownloads();
    Q.cancelAllTasks();

    expect(startsNewWork()).toBe(true);
  });

  it('clears the pause when the last parked row is removed by hand', () => {
    Q.setMaxSlots(1);
    const [a] = enqueueMany(1);
    vi.runOnlyPendingTimers();
    Q.pauseAllDownloads();
    Q.removeTasks([a.taskId]);

    expect(startsNewWork()).toBe(true);
  });

  // Pausing an empty queue and THEN adding links is a legitimate order to work
  // in, so the latch is kept. What must not happen is the new row claiming to be
  // 'queued': the renderer reads that as "not paused", renders "Pause all", and
  // the click is swallowed by the `if (paused) return` guard — the user can no
  // longer resume the queue they paused. Parking it makes the state visible.
  it('parks work added to an already-paused queue instead of leaving it "queued"', () => {
    Q.setMaxSlots(1);
    Q.pauseAllDownloads();

    const fresh = Q.createTask({ url: 'fresh', title: 'Fresh' });
    Q.enqueueTask(fresh);
    vi.runOnlyPendingTimers();

    expect(dl.download).not.toHaveBeenCalled();
    expect(statusOf(fresh.taskId)).toBe('paused'); // reachable: "Resume all" now shows

    Q.resumeAllDownloads();
    expect(startedIds()).toContain(fresh.taskId);
  });

  it('still holds the pause while parked work remains', () => {
    Q.setMaxSlots(1);
    enqueueMany(1);
    vi.runOnlyPendingTimers();
    Q.pauseAllDownloads();

    // The guard must not overshoot into "pause never works".
    expect(startsNewWork()).toBe(false);
  });
});
