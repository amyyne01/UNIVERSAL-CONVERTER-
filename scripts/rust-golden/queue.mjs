// Golden file for the download queue (electron/queue.ts).
//
// Each scenario loads a fresh copy of the real queue module and drives it with a
// scripted list of steps. Everything the queue touches from outside is replaced
// by a recorder: the clock and timers, the downloader, the main window, and the
// change / failure / completion listeners. After every step the recorder's
// contents and the queue's visible state are saved.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export default async function queueSection({ OUT }) {
  const { errorCodeOf } = await import('../../dist-electron/electron/downloader.js');
  const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'ahg-golden-queue-'));
  fs.writeFileSync(path.join(TMP, 'song.mp3'), Buffer.alloc(4321));
  const real = (v) =>
    typeof v === 'string' ? v.replaceAll('{TMP}', TMP)
    : Array.isArray(v) ? v.map(real)
    : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, real(x)]))
    : v;
  const tok = (v) => JSON.parse(JSON.stringify(v ?? null).replaceAll(TMP.replaceAll('\\', '\\\\'), '{TMP}'));

  // ── A clock and timers the scenario controls ───────────────────────────────
  const realGlobals = { setTimeout: globalThis.setTimeout, clearTimeout: globalThis.clearTimeout, now: Date.now, random: Math.random };
  let now = 0;
  let timers = [];
  let timerSeq = 0;
  let seed = 1;
  const install = () => {
    now = 1_763_200_000_000;
    timers = [];
    timerSeq = 0;
    seed = 1;
    Date.now = () => now;
    // A fixed sequence, so task ids are the same on every run.
    Math.random = () => { seed = (seed * 48271) % 2147483647; return seed / 2147483647; };
    globalThis.setTimeout = (fn, ms = 0) => { const t = { at: now + ms, seq: timerSeq++, fn, unref() {} }; timers.push(t); return t; };
    globalThis.clearTimeout = (t) => { timers = timers.filter((x) => x !== t); };
  };
  const restore = () => {
    globalThis.setTimeout = realGlobals.setTimeout;
    globalThis.clearTimeout = realGlobals.clearTimeout;
    Date.now = realGlobals.now;
    Math.random = realGlobals.random;
    delete globalThis.__ahgWindow;
  };
  /** Move the clock forward, firing every timer that falls due, earliest first. */
  const advance = (ms) => {
    const target = now + ms;
    for (;;) {
      const due = timers.filter((t) => t.at <= target).sort((a, b) => a.at - b.at || a.seq - b.seq)[0];
      if (!due) break;
      timers = timers.filter((t) => t !== due);
      now = Math.max(now, due.at);
      due.fn();
    }
    now = target;
  };

  const p = (o = {}) => ({ status: 'downloading', percent: 0, speed: 0, eta: 0, downloaded: 0, total: 0, filename: '', error: '', playlistIndex: 0, playlistTotal: 0, ...o });
  const yt = (n, o = {}) => ({ url: `https://youtu.be/${String(n).padStart(11, 'a')}`, source: 'youtube', outputDir: 'C:\\Music', title: `Song ${n}`, ...o });
  const track = { id: 't1', name: 'Song', artist: 'Artist', artists: ['Artist'], album: 'Album', trackNumber: 1, durationMs: 200000, thumbnailUrl: '' };
  const GATED = 'ERROR: unable to download video data: HTTP Error 403: Forbidden';
  const NET = 'ERROR: Connection reset by peer';
  const GONE = 'ERROR: Video unavailable';

  // Steps: tasks are named by the order they were enqueued (0, 1, ...), and
  // downloads by the order the queue started them (0, 1, ...).
  const scenarios = {
    'one download from start to finish': [
      ['slots', 1], ['enqueue', yt(1)], ['progress', 0, p({ percent: 10, speed: 1000, total: 5000, downloaded: 500 })],
      ['advance', 100], ['progress', 0, p({ percent: 20 })], ['progress', 0, p({ percent: 30 })], ['advance', 200], ['advance', 300],
      ['progress', 0, p({ percent: 60, status: 'converting' })], ['progress', 0, p({ percent: 80, status: 'embedding' })],
      ['done', 0, '{TMP}\\song.mp3', ['{TMP}\\song.mp3'], false], ['advance', 499], ['advance', 1], ['advance', 1000], ['flush'],
    ],
    'the progress throttle and its trailing send': [
      ['slots', 1], ['enqueue', yt(1)], ['progress', 0, p({ percent: 1 })], ['progress', 0, p({ percent: 2 })], ['progress', 0, p({ percent: 3 })],
      ['advance', 249], ['advance', 1], ['progress', 0, p({ percent: 4 })], ['advance', 250], ['progress', 0, p({ percent: 5 })],
      ['progress', 0, p({ percent: 100 })], ['progress', 0, p({ percent: 6 })], ['advance', 100], ['progress', 0, p({ percent: 7 })],
      ['cancel', 0], ['advance', 1000],
    ],
    'progress that tries an impossible status change': [
      ['slots', 1], ['enqueue', yt(1)], ['progress', 0, p({ status: 'done', percent: 50 })], ['progress', 0, p({ status: 'queued', percent: 51 })],
      ['progress', 0, p({ status: 'downloading', percent: 52 })], ['progress', 0, p({ status: 'embedding', percent: 53 })],
      ['progress', 0, p({ status: 'converting', percent: 54 })], ['progress', 0, p({ status: 'downloading', percent: 55 })],
      ['progress', 0, p({ status: 'done', percent: 100 })], ['done', 0, '', [], false], ['advance', 600],
    ],
    'three waiting for one slot, then two slots': [
      ['slots', 1], ['enqueue', yt(1)], ['enqueue', yt(2)], ['enqueue', yt(3)], ['done', 0, 'Song 1.mp3', [], false], ['advance', 499],
      ['advance', 1], ['slots', 2], ['done', 1, '', ['a', 'b', ''], false], ['done', 2, '', [], true], ['advance', 500], ['advance', 500], ['flush'],
    ],
    'two jobs finishing inside the same half second': [
      ['slots', 2], ['enqueue', yt(1)], ['enqueue', yt(2)], ['enqueue', yt(3)], ['enqueue', yt(4)], ['enqueue', yt(5)],
      ['done', 0, '', [], false], ['advance', 100], ['done', 1, '', [], false], ['advance', 400], ['advance', 100], ['advance', 500],
    ],
    'lowering and raising the slot count mid-run': [
      ['slots', 3], ['enqueue', yt(1)], ['enqueue', yt(2)], ['enqueue', yt(3)], ['enqueue', yt(4)], ['slots', 1], ['done', 0, '', [], false],
      ['advance', 500], ['done', 1, '', [], false], ['advance', 500], ['slots', 0], ['slots', 2.9], ['slots', -4], ['done', 2, '', [], false], ['advance', 500],
    ],
    'pausing and resuming one running job': [
      ['slots', 1], ['enqueue', yt(1)], ['enqueue', yt(2)], ['progress', 0, p({ percent: 40, speed: 900, eta: 12 })], ['pause', 0],
      ['error', 0, 'ERROR: killed'], ['progress', 0, p({ percent: 41 })], ['advance', 500], ['resume', 0], ['resume', 0], ['done', 1, '', [], false],
      ['advance', 500], ['done', 2, '', [], false], ['advance', 4000], ['flush'],
    ],
    'pausing waiting jobs, and things that cannot be paused or resumed': [
      ['slots', 1], ['enqueue', yt(1)], ['enqueue', yt(2)], ['enqueue', yt(3)], ['pause', 2], ['pause', 2], ['resume', 1], ['pause', 9], ['resume', 9],
      ['done', 0, '', [], false], ['advance', 500], ['done', 1, '', [], false], ['advance', 500], ['resume', 2], ['advance', 500],
    ],
    'pause all keeps the order jobs started in': [
      ['slots', 3], ['enqueue', yt(1)], ['enqueue', yt(2)], ['enqueue', yt(3)], ['enqueue', yt(4)], ['pauseAll'], ['pauseAll'], ['enqueue', yt(5)],
      ['error', 0, 'x'], ['error', 1, 'x'], ['error', 2, 'x'], ['advance', 500], ['resumeAll'], ['resumeAll'], ['advance', 4000],
    ],
    'cancel all after pause all clears the global pause': [
      ['slots', 2], ['enqueue', yt(1)], ['enqueue', yt(2)], ['enqueue', yt(3)], ['pauseAll'], ['cancelAll'], ['enqueue', yt(4)], ['advance', 4000], ['flush'],
    ],
    'removing the last parked row clears the global pause': [
      ['slots', 1], ['enqueue', yt(1)], ['pauseAll'], ['remove', [0]], ['enqueue', yt(2)], ['advance', 4000],
    ],
    'cancelling a waiting job, a running job and an unknown one': [
      ['slots', 1], ['enqueue', yt(1)], ['enqueue', yt(2)], ['enqueue', yt(3)], ['cancel', 1], ['cancel', 0], ['error', 0, 'ERROR: yt-dlp exited with code 1'],
      ['cancel', 0], ['cancel', 9], ['advance', 500], ['advance', 3000], ['flush'],
    ],
    'an error after the cancel grace has run out is shown': [
      ['slots', 2], ['enqueue', yt(1)], ['enqueue', yt(2)], ['pause', 0], ['advance', 3000], ['resume', 0], ['advance', 500], ['error', 2, GONE], ['advance', 500],
    ],
    'a late error from a cancelled job inside the grace window is swallowed': [
      ['slots', 1], ['enqueue', yt(1)], ['pause', 0], ['resume', 0], ['advance', 500], ['error', 1, 'ERROR: late'], ['advance', 3000], ['flush'],
    ],
    'a network error retries three times with growing waits, then fails': [
      ['slots', 1], ['enqueue', yt(1)], ['error', 0, NET], ['advance', 4999], ['advance', 1], ['error', 1, NET], ['advance', 14999], ['advance', 1],
      ['error', 2, NET], ['advance', 44999], ['advance', 1], ['error', 3, NET], ['advance', 60000], ['flush'],
    ],
    'a permanent error fails at once': [
      ['slots', 1], ['enqueue', yt(1)], ['enqueue', yt(2)], ['error', 0, GONE], ['advance', 500], ['error', 1, ''], ['advance', 500], ['flush'],
    ],
    'the token gate retries with the fallback clients latched': [
      ['slots', 1], ['enqueue', yt(1)], ['error', 0, GATED], ['advance', 5000], ['error', 1, NET], ['advance', 15000], ['done', 2, '', [], false], ['advance', 500], ['flush'],
    ],
    'a retry waits at the front when every slot is busy': [
      ['slots', 1], ['enqueue', yt(1)], ['enqueue', yt(2)], ['enqueue', yt(3)], ['error', 0, NET], ['advance', 500], ['advance', 4500], ['done', 1, '', [], false], ['advance', 500],
      ['done', 2, '', [], false], ['advance', 500], ['done', 3, '', [], false], ['advance', 500],
    ],
    'cancelling during the retry wait': [
      ['slots', 1], ['enqueue', yt(1)], ['error', 0, NET], ['advance', 1000], ['cancel', 0], ['cancel', 0], ['advance', 10000], ['flush'],
    ],
    'cancel all with a retry pending, jobs running and jobs waiting': [
      ['slots', 2], ['enqueue', yt(1)], ['enqueue', yt(2)], ['enqueue', yt(3)], ['enqueue', yt(4)], ['error', 0, NET], ['advance', 500], ['pause', 3],
      ['cancelAll'], ['error', 1, 'x'], ['error', 2, 'x'], ['advance', 10000], ['flush'],
    ],
    'removing rows: running ones are skipped, a pending retry is dropped': [
      ['slots', 1], ['enqueue', yt(1)], ['enqueue', yt(2)], ['enqueue', yt(3)], ['error', 0, NET], ['advance', 500], ['remove', [0, 1, 2, 9]],
      ['advance', 10000], ['done', 1, '', [], false], ['advance', 500], ['remove', [1]], ['flush'],
    ],
    'reordering and promoting waiting jobs around a paused one': [
      ['slots', 1], ['enqueue', yt(1)], ['enqueue', yt(2)], ['enqueue', yt(3)], ['enqueue', yt(4)], ['enqueue', yt(5)], ['pause', 3], ['reorder', [4, 2, 1]],
      ['reorder', [4, 4, 9, 0, 3]], ['reorder', []], ['promote', 2], ['promote', 3], ['promote', 0], ['promote', 9], ['reorder', [1]], ['cancelAll'], ['reorder', [1, 2]],
    ],
    'a Spotify task carries its track to the downloader': [
      ['slots', 1], ['enqueue', { url: 'ytsearch:Artist - Song', source: 'spotify', track, format: 'flac', quality: 'lossless', isAudioOnly: true }], ['error', 0, NET], ['advance', 5000],
      ['done', 1, '{TMP}\\song.mp3', [], false], ['advance', 500], ['flush'],
    ],
    'everything a task can be created with': [
      ['slots', 1], ['enqueue', {}], ['enqueue', { url: 'u', source: 'vimeo', outputDir: 'D:\\', format: 'mp4', quality: '', videoQuality: '720p', isAudioOnly: false, isPlaylist: true, playlistName: 'P', playlistLimit: 20, playlistItems: '1,3-5', embedThumbnail: false, embedMetadata: false, skipExisting: false, title: 'T', thumbnailUrl: 'th', duration: 12.5, uploader: 'U', createdAt: 5, retryCount: 2, originalTaskId: 'orig', fallbackProfile: false, extras: { subtitleMode: 'embed', proxy: 'socks5://a' }, errorCode: 'drm', interrupted: true, matchedUrl: 'm', matchConfidence: 0.5, completedAt: 9, progress: p({ percent: 77 }), taskId: 'ignored' }],
      ['error', 0, NET], ['advance', 60000],
    ],
    'the recent list keeps the last eight': [
      ['slots', 1], ...Array.from({ length: 10 }, (_, i) => ['enqueue', yt(i + 1, i === 4 ? { title: '' } : {})]), ['error', 0, NET], ['advance', 5000], ['cancelAll'],
    ],
    'enqueueing while everything is paused parks the new job': [
      ['slots', 1], ['enqueue', yt(1)], ['pauseAll'], ['enqueue', yt(2)], ['resumeAll'], ['advance', 4000],
    ],
    'the history cap drops the oldest finished rows only': [
      ['slots', 1], ['enqueue', yt(0)], ['done', 0, '', [], false], ['advance', 500],
      ['enqueueMany', 203], ['cancel', 3], ['enqueue', yt(900)], ['enqueue', yt(901)], ['flush'],
    ],
  };

  const restored = [
    { name: 'history from a previous run is restored and interrupted jobs are parked', file: (() => {
      const row = (id, status, extra = {}) => ({ taskId: id, url: `https://youtu.be/${id}`, source: 'youtube', outputDir: 'C:\\Music', format: 'mp3', quality: '320', videoQuality: 'best', isAudioOnly: true, isPlaylist: false, playlistName: '', embedThumbnail: true, embedMetadata: true, skipExisting: true, title: id, thumbnailUrl: '', duration: 0, uploader: '', createdAt: 1, progress: p({ status, percent: 42, speed: 7, eta: 9, error: 'old error' }), ...extra });
      return JSON.stringify([
        row('done1', 'done', { completedAt: 2 }), row('down1', 'downloading'), row('fail1', 'failed', { errorCode: 'network' }), row('queued1', 'queued', { retryCount: 1 }),
        row('paused1', 'paused', { interrupted: false }), row('retry1', 'retrying'), row('canc1', 'cancelled'), row('conv1', 'converting', { extras: { proxy: 'http://p' } }),
        { taskId: 'thin1', progress: { status: 'embedding' } }, { taskId: 'thin2', progress: {} }, { taskId: '', progress: p() }, { taskId: 'noprogress' }, null, 5, 'x',
        { ...row('extra1', 'done'), someFutureKey: { a: 1 } },
      ]);
    })(), steps: [['slots', 2], ['advance', 1000], ['resumeId', 'down1'], ['resumeId', 'queued1'], ['advance', 500], ['flush']] },
    { name: 'a corrupt history file starts fresh', file: '[{"taskId":', steps: [['slots', 1], ['enqueue', yt(1)], ['flush']] },
    { name: 'a history file that is not a list starts fresh', file: '{"taskId":"a","progress":{}}', steps: [['slots', 1], ['enqueue', yt(1)], ['flush']] },
    { name: 'an empty history file', file: '', steps: [['slots', 1], ['flush']] },
    { name: 'no history file', file: null, steps: [['slots', 1], ['flush'], ['enqueue', yt(1)], ['advance', 499], ['advance', 1]] },
  ];

  const all = [...Object.entries(scenarios).map(([name, steps]) => ({ name, file: null, steps })), ...restored];
  const cases = [];
  let n = 0;
  try {
    for (const { name, file, steps } of all) {
      install();
      const rec = { sends: [], changes: 0, failures: [], dl: [], completed: [] };
      const drain = () => { const out = tok(rec); rec.sends = []; rec.changes = 0; rec.failures = []; rec.dl = []; rec.completed = []; return out; };
      globalThis.__ahgWindow = { webContents: { send: (channel, ...args) => { rec.sends.push(JSON.parse(JSON.stringify([channel, ...args]))); } } };
      const q = await import(`../../dist-electron/electron/queue.js?scenario=${n}`);
      const calls = [];
      q.setDownloader({
        download(task, trk, onProgress, onDone, onError) {
          delete task.errorCode; // as the real downloader does before each attempt
          calls.push({ task, onProgress, onDone, onError });
          rec.dl.push(['download', JSON.parse(JSON.stringify(task)), trk === undefined ? null : JSON.parse(JSON.stringify(trk))]);
        },
        cancel(id) { rec.dl.push(['cancel', id]); },
        cancelAll() { rec.dl.push(['cancelAll']); },
      });
      q.onQueueChange(() => { rec.changes++; });
      q.onDownloadFailure((e) => { rec.failures.push(e); });
      q.onDownloadCompleted((task, filepaths) => { rec.completed.push([task.taskId, [...filepaths]]); });

      const dir = path.join(TMP, String(n++));
      fs.mkdirSync(dir);
      const historyFile = path.join(dir, 'downloads-history.json');
      if (file !== null) fs.writeFileSync(historyFile, file);
      q.initHistory(historyFile);
      const ids = [];
      const state = () => tok({
        all: q.getAllTasks(), recent: q.getRecentDownloads(), unfinished: q.hasUnfinishedWork(),
        disk: (() => { try { return JSON.parse(fs.readFileSync(historyFile, 'utf8')); } catch { return null; } })(),
        timers: timers.map((t) => t.at - now).sort((a, b) => a - b),
      });
      const out = [{ step: ['init'], ret: null, ...drain(), state: state() }];
      for (const step of steps) {
        const [op, a, b, c, d] = step;
        const id = (i) => ids[i] ?? `missing_${i}`;
        let ret = null;
        if (op === 'slots') q.setMaxSlots(a);
        else if (op === 'enqueue') { const task = q.createTask(real(a)); ids.push(task.taskId); ret = JSON.parse(JSON.stringify(task)); q.enqueueTask(task); }
        else if (op === 'enqueueMany') {
          // One recorded step for many enqueues, so the file stays small; the ids come back as the result.
          ret = [];
          for (let i = 0; i < a; i++) { const task = q.createTask(yt(1000 + i)); ids.push(task.taskId); ret.push(task.taskId); q.enqueueTask(task); }
        }
        else if (op === 'pause') ret = q.pauseTask(id(a));
        else if (op === 'resume') ret = q.resumeTask(id(a));
        else if (op === 'resumeId') ret = q.resumeTask(a);
        else if (op === 'cancel') ret = q.cancelTask(id(a));
        else if (op === 'pauseAll') q.pauseAllDownloads();
        else if (op === 'resumeAll') q.resumeAllDownloads();
        else if (op === 'cancelAll') q.cancelAllTasks();
        else if (op === 'remove') ret = q.removeTasks(a.map(id));
        else if (op === 'reorder') ret = q.reorderQueue(a.map(id));
        else if (op === 'promote') ret = q.promoteTask(id(a));
        else if (op === 'progress') calls[a].onProgress({ ...b });
        else if (op === 'done') calls[a].onDone(calls[a].task, real(b), real(c), d);
        else if (op === 'error') { calls[a].task.errorCode = errorCodeOf(b); calls[a].onError(calls[a].task, b); }
        else if (op === 'advance') advance(a);
        else if (op === 'flush') q.flushHistory();
        else throw new Error(`unknown step ${op}`);
        out.push({ step, ret: tok(ret), ...drain(), state: state() });
      }
      cases.push({ name, file, out });
    }
  } finally {
    restore();
  }
  fs.writeFileSync(`${OUT}/queue.json`, JSON.stringify(cases) + '\n');
  console.log(`queue.json: ${cases.length} scenarios, ${cases.reduce((s, c) => s + c.out.length, 0)} steps`);
  fs.rmSync(TMP, { recursive: true, force: true });
}
