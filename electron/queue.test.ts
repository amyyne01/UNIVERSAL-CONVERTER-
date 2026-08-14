import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import type { DownloadStatus } from '../shared/types.js';

// Mocks per testing-vitest §7b — same specifiers queue.ts imports.
vi.mock('./window.js', () => ({
  getMainWindow: vi.fn(() => ({ webContents: { send: vi.fn() } })),
}));

vi.mock('./downloader.js', () => ({
  classifyError: vi.fn(() => 'permanent'),
}));

vi.mock('node:fs', () => {
  const existsSync = vi.fn(() => false);
  const readFileSync = vi.fn();
  const writeFileSync = vi.fn();
  const statSync = vi.fn(() => ({ size: 0 }));
  return { default: { existsSync, readFileSync, writeFileSync, statSync }, existsSync, readFileSync, writeFileSync, statSync };
});

// queue.ts is a module-level singleton (one slot, shared history/timers). Each test
// gets a pristine instance via resetModules + a fresh dynamic import, with fake timers
// to drive the backoff / grace / next-job delays deterministically.
type Queue = typeof import('./queue');
let Q: Queue;
let classify: ReturnType<typeof vi.fn>;
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

describe('single-slot processing', () => {
  it('runs strictly one job at a time; the next starts only after the first finishes', () => {
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

describe('finishCurrent idle notification (B1)', () => {
  it('notifies subscribers after isDownloading flips false, not just from the pre-finish emit', () => {
    const snapshots: boolean[] = [];
    Q.onQueueChange(() => snapshots.push(Q.hasUnfinishedWork()));
    const a = Q.createTask({ url: 'a', title: 'A' });
    Q.enqueueTask(a);
    snapshots.length = 0;                                 // ignore enqueue-time notifications
    onDoneOf()(a, '');
    // Before the fix, the only notification fires from handleDone's emit() while
    // isDownloading is still true — the drain-watcher never observes true idle.
    expect(snapshots.at(-1)).toBe(false);
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
