// Golden files for the small services: the redacting log (electron/log.ts), the
// monthly counters (electron/stats.ts) and the scheduler (electron/scheduler.ts).
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export default async function services({ OUT, literals }) {
  // ── log.ts: pure text functions ────────────────────────────────────────────
  const log = await import('../../dist-electron/electron/log.js');
  const texts = [...new Set([
    ...literals('electron/log.test.ts'), ...literals('electron/queue.test.ts'), ...literals('electron/downloader.test.ts'),
    '', ' ', 'plain text', 'ghp_abcdefghij', 'ghp_abcdefghi', 'xghp_abcdefghijklmnop', 'x ghp_abcdefghijklmnop y', 'GHP_abcdefghijklmnop',
    'gho_1234567890 ghu_1234567890 ghs_1234567890 ghr_1234567890 ghx_1234567890', 'github_pat_11ABCDEFG0_abcdefghijklmnop',
    '\u00E9ghp_abcdefghijklmnop', 'Authorization: Bearer abc.def-ghi_jkl~mno+pqr/stu=', 'bearer\u00A0tok', 'BASIC dXNlcjpwYXNz rest',
    'Bearer', 'Bearer  ', 'xBearer abc', 'Bearer\u0085abc', 'Bearer\uFEFFabc', 'Cookie: a=b; c=d\nnext line', 'set-cookie=a=b\r\nnext',
    'SET-COOKIE : x', 'cookie:', 'mycookie: a=b', 'cookie monster', 'authorization = Basic abc', 'Authorization:Bearer abc',
    'token=abc123&next=1', 'access_token: "abc", "refresh_token":"def"}', "api_key='abc' secret = xyz", 'TOKEN=abc', 'password=p@ss,w0rd',
    'sig=abc}rest', 'key:value', 'keyboard: qwerty', 'monkey=banana', 'token=', 'token= spaced', 'token="', 'tokens=5',
    'https://user:pw@example.com:8443/a/b?token=abc#frag and http://x.y/z', 'see HTTPS://EXAMPLE.COM/Path', 'ftp://files.example.com/a',
    'ytsearch://x', 'a://b', '1http://x.y', 'http://', 'http:// x', 'file:///C:/Users/me/a.mp4', 'https://[::1]:8080/x', 'https://b\u00FCcher.example/x',
    'spotify:track:abc', 'mailto:a@b.c', 'https://a.b/c"quoted" <https://d.e/f>', "it's https://a.b/c's", 'x'.repeat(1999), 'x'.repeat(2000),
    'x'.repeat(2001), 'y' + 'x'.repeat(2000), '\uD83D\uDE00'.repeat(1000), 'a' + '\uD83D\uDE00'.repeat(1000),
    'ERROR: [youtube] abc: Sign in to confirm https://www.youtube.com/watch?v=abc&token=zzz Cookie: SID=1',
  ])];
  const logGolden = {
    capBytes: log.LOG_CAP_BYTES,
    cases: texts.map((t) => ({
      t, redact: log.redact(t), host: log.hostOf(t), scrub: log.scrubUrls(t),
      // A cut inside a surrogate pair cannot travel through JSON; the length is enough to check it.
      tail: log.tail(t).isWellFormed() ? log.tail(t) : null, tailLen: log.tail(t).length,
    })),
  };
  fs.writeFileSync(`${OUT}/log.json`, JSON.stringify(logGolden) + '\n');
  console.log(`log.json: ${logGolden.cases.length} cases`);

  // ── A clock the sections below can set: `new Date()` returns `fake`. ───────
  const RealDate = Date;
  let fake = new RealDate(2026, 0, 15, 12, 0);
  globalThis.Date = class extends RealDate {
    constructor(...args) { if (args.length) super(...args); else super(fake.getTime()); }
    static now() { return fake.getTime(); }
  };
  /** [year, month 1-12, day, hour, minute] in local time. */
  const at = ([y, mo, d, h = 12, mi = 0]) => { fake = new RealDate(y, mo - 1, d, h, mi); };

  try {
    // ── stats.ts ─────────────────────────────────────────────────────────────
    const { StatsStore, monthKey, statsFilePath } = await import('../../dist-electron/electron/stats.js');
    const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'ahg-golden-stats-'));
    for (const [name, size] of [['a.bin', 10], ['b.bin', 2500], ['empty.bin', 0]]) fs.writeFileSync(path.join(TMP, name), Buffer.alloc(size));
    const real = (v) => (typeof v === 'string' ? v.replaceAll('{TMP}', TMP) : Array.isArray(v) ? v.map(real) : v);
    const month = (m, files, bytes, byPlatform = {}) => ({ month: m, files, bytes, byPlatform });
    // Each scenario: an optional starting file, then steps run against one store.
    const scenarios = [
      { file: null, steps: [
        ['get', [2026, 1, 15]], ['record', [2026, 1, 15], 'youtube', ['{TMP}\\a.bin'], 0], ['record', [2026, 1, 20], 'youtube', ['{TMP}\\a.bin', '{TMP}\\b.bin'], 999],
        ['record', [2026, 1, 20], 'spotify', [], 4096], ['record', [2026, 1, 21], 'soundcloud', ['', '   ', '{TMP}\\missing.bin'], 7],
        ['record', [2026, 1, 21], 'tiktok', ['{TMP}\\empty.bin'], 5], ['record', [2026, 1, 21], 'youtube', ['{TMP}\\missing.bin'], 0],
        ['get', [2026, 1, 31, 23, 59]], ['get', [2026, 2, 1, 0, 0]], ['record', [2026, 2, 3], 'generic', ['{TMP}\\b.bin'], 0],
        ['get', [2026, 6, 1]], ['get', [2026, 6, 2]], ['record', [2027, 1, 1], 'youtube', ['{TMP}\\a.bin'], 0], ['get', [2026, 12, 1]],
      ] },
      { file: JSON.stringify({ current: month('2026-01', 3, 300, { youtube: 2, spotify: 1 }), previous: month('2025-11', 9, 900, { vimeo: 9 }) }),
        steps: [['get', [2026, 1, 15]], ['record', [2026, 1, 15], 'spotify', ['{TMP}\\a.bin'], 0], ['record', [2026, 1, 15], 'vimeo', ['{TMP}\\a.bin'], 0], ['get', [2026, 2, 1]]] },
      { file: JSON.stringify({ current: month('2025-03', 1, 1), previous: null }), steps: [['get', [2026, 1, 15]]] },
      { file: JSON.stringify({ current: { month: '2026-01' } }), steps: [['get', [2026, 1, 15]], ['record', [2026, 1, 15], 'youtube', [], 0]] },
      { file: JSON.stringify({ current: { byPlatform: { x: 1 }, bytes: 5, files: 2, month: '2026-01' }, previous: { month: '' } }), steps: [['get', [2026, 1, 15]], ['record', [2026, 1, 15], 'youtube', [], 1]] },
      { file: JSON.stringify({ previous: month('2025-12', 1, 1) }), steps: [['get', [2026, 1, 15]]] },
      { file: JSON.stringify({ current: { month: '' } }), steps: [['get', [2026, 1, 15]]] },
      { file: '{', steps: [['get', [2026, 1, 15]], ['record', [2026, 1, 15], 'youtube', [], 0]] },
      { file: '', steps: [['get', [2026, 1, 15]]] },
      { file: 'null', steps: [['get', [2026, 1, 15]]] },
      { file: '[]', steps: [['get', [2026, 1, 15]]] },
      { file: '5', steps: [['get', [2026, 1, 15]]] },
    ];
    const statsCases = scenarios.map(({ file, steps }, i) => {
      const dir = path.join(TMP, String(i));
      fs.mkdirSync(dir);
      const target = statsFilePath(dir);
      if (file !== null) fs.writeFileSync(target, file);
      at(steps[0][1]);
      const store = new StatsStore();
      store.init(target);
      const disk = () => { try { return fs.readFileSync(target, 'utf-8'); } catch { return null; } };
      const out = [{ step: ['init', steps[0][1]], disk: disk() }];
      for (const step of steps) {
        const [op, when, platform, files, fallback] = step;
        at(when);
        const result = op === 'get' ? JSON.parse(JSON.stringify(store.get())) : (store.record(platform, real(files), fallback), null);
        out.push({ step, result, disk: disk() });
      }
      return { file, out };
    });
    const months = [[2026, 1, 1, 0, 0], [2026, 9, 30, 23, 59], [2026, 10, 1], [2026, 12, 31, 23, 59], [999, 5, 5], [12345, 1, 1]];
    fs.writeFileSync(`${OUT}/stats.json`, JSON.stringify({
      fileName: path.basename(statsFilePath('x')),
      monthKey: months.map((m) => { at(m); return { at: m, key: monthKey() }; }),
      scenarios: statsCases,
    }) + '\n');
    console.log(`stats.json: ${statsCases.reduce((n, s) => n + s.out.length, 0)} steps`);
    fs.rmSync(TMP, { recursive: true, force: true });

    // ── scheduler.ts ─────────────────────────────────────────────────────────
    const { Scheduler } = await import('../../dist-electron/electron/scheduler.js');
    const cfg = (o = {}) => ({ scheduleEnabled: true, scheduleTime: '03:00', scheduleDays: [], scheduleShutdown: false, ...o });
    // 2026-01-15 is a Thursday (getDay() 4).
    const day = (h, mi, d = 15) => [2026, 1, d, h, mi];
    const times = ['03:00', '3:0', ' 3 : 0 ', '03', '', ':', '03:', ':30', '3:30:99', '24:00', '23:30', '-1:00', '3.5:0', '0x3:0x1e', '1e0:0',
      'aa:bb', '03:xx', 'Infinity:0', '03:00am', '3:60', '99:99', '2:59.5', '+3:+0', '3;0'];
    // Each sequence is run on one scheduler, so the once-a-day gate carries over.
    const sequences = [
      ...times.map((scheduleTime) => [2, 3, 4].flatMap((h) => [0, 59].map((mi) => ({ cfg: cfg({ scheduleTime }), premium: true, at: day(h, mi), via: 'tick' })))),
      [day(2, 59), day(3, 0), day(3, 1), day(3, 59), day(4, 0), day(3, 0, 16), day(3, 30, 16), day(3, 0, 17)].map((t) => ({ cfg: cfg(), premium: true, at: t, via: 'tick' })),
      [day(3, 10), day(3, 20)].map((t, i) => ({ cfg: cfg({ scheduleShutdown: true }), premium: true, at: t, via: i ? 'tick' : 'missed' })),
      [{ cfg: cfg(), premium: false, at: day(3, 0), via: 'tick' }, { cfg: cfg(), premium: true, at: day(3, 5), via: 'tick' }],
      [{ cfg: cfg({ scheduleEnabled: false }), premium: true, at: day(3, 0), via: 'missed' }, { cfg: cfg(), premium: true, at: day(3, 5), via: 'missed' }],
      ...[[4], [0, 6], [4.0, 9], [3], [-1], [4.5], []].map((scheduleDays) => [15, 16, 17, 18].map((d) => ({ cfg: cfg({ scheduleDays }), premium: true, at: day(3, 0, d), via: 'tick' }))),
      [day(23, 30), day(23, 59), day(0, 0, 16), day(0, 29, 16)].map((t) => ({ cfg: cfg({ scheduleTime: '23:30' }), premium: true, at: t, via: 'tick' })),
      [[2026, 1, 31, 3, 0], [2026, 2, 1, 3, 0], [2026, 12, 31, 3, 0], [2027, 1, 1, 3, 0]].map((t) => ({ cfg: cfg(), premium: true, at: t, via: 'tick' })),
    ];
    const schedulerCases = sequences.map((steps) => {
      let step;
      let triggers = 0;
      let shutdowns = 0;
      const s = new Scheduler({ getConfig: () => step.cfg, isPremium: () => step.premium, onTrigger: () => { triggers++; }, onShutdown: () => { shutdowns++; } });
      return steps.map((current) => {
        step = current;
        at(current.at);
        const before = [triggers, shutdowns];
        if (current.via === 'tick') s.tick(); else s.checkMissedRun();
        return { ...current, weekday: new Date().getDay(), triggered: triggers - before[0], shutdown: shutdowns - before[1] };
      });
    });
    fs.writeFileSync(`${OUT}/scheduler.json`, JSON.stringify(schedulerCases) + '\n');
    console.log(`scheduler.json: ${schedulerCases.reduce((n, s) => n + s.length, 0)} steps`);
  } finally {
    globalThis.Date = RealDate;
  }
}
