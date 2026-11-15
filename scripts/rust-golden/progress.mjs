// Golden section for electron/downloader.ts progress parsing: parseLine and the
// size/speed/eta helpers called directly, and the whole stdout/stderr handling
// of spawnDownload driven through a real child process (node printing chunks).
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export default async ({ OUT, literals }) => {
  const { Downloader } = await import('../../dist-electron/electron/downloader.js');
  const d = new Downloader();

  // JSON cannot carry NaN or Infinity: they travel as their names.
  const enc = (n) => (Number.isFinite(n) ? n : String(n));
  const copy = (p) => JSON.parse(JSON.stringify(p));
  const base = (init = {}) => ({
    status: 'downloading', percent: 0, speed: 0, eta: 0, downloaded: 0, total: 0,
    filename: '', error: '', playlistIndex: 0, playlistTotal: 0, ...init,
  });
  const feed = (line, progress, last) => {
    const events = [];
    const out = d['parseLine'](line, progress, last, (p) => events.push(copy(p)));
    return { progress: copy(progress), last: enc(out), events };
  };

  // ── Lines ──────────────────────────────────────────────────────────────────
  const hand = new Set(literals('electron/downloader.test.ts'));
  const add = (...a) => a.flat().forEach((x) => hand.add(x));
  add('', ' ', '[PROGRESS]', '[PROGRESS] ', '[PROGRESS]|||||', '[PROGRESS]a|b|c|d|e|f|g', ' [PROGRESS]1|2|NA|1|1',
    'x[PROGRESS]1|2|NA|1|1', '[progress]1|2|NA|1|1', '[PROGRESS]5', '[PROGRESS]5|10', '[PROGRESS]5|10|NA', '[PROGRESS]5|10|NA|1',
    '[PROGRESS]5|10|NA|1|2|', '[PROGRESS]5|10|NA|1|2|extra|more', '[PROGRESS]5|10|NA|1|2\r', '[PROGRESS]\u00e9|\u00e9|\u00e9|\u00e9|\u00e9',
    '[PROGRESS]5|10|NA|1.5|2.5', '[PROGRESS]1e9|2e9|NA|1e6|10', '[PROGRESS]1000000000000000|2000000000000000|NA|1|1',
    '[PROGRESS]0|0|0|0|0', '[PROGRESS]NA|NA|NA|NA|NA', '[PROGRESS]Unknown|Unknown|Unknown|Unknown|Unknown',
    '[PROGRESS]5|-10|NA|1|2', '[PROGRESS]-5|10|NA|-1|-2', '[PROGRESS]20|10|NA|1|2', '[PROGRESS]1e300|1e-300|NA|1|1',
    '[ExtractAudio] Destination: a.mp3', '[ExtractAudio]', '[Merger] Merging formats into "a.mkv"', '[Merger]x', ' [Merger] x',
    '[Metadata] Adding metadata', '[EmbedThumbnail] ffmpeg: Adding thumbnail', '[ThumbnailsConvertor] Converting', '[EmbedSubtitle] x',
    '[Metadata]', '[metadata] x', '[FixupM3u8] x', '[Fixup] x', '[VideoConvertor] x', '[SponsorBlock] x',
    '[download] Downloading item 1 of 3', '[download] Downloading item 12', '[download] Downloading item x', '[download] Downloading item \u0661',
    '[download] Downloading playlist: x', ' [download] Downloading item 2', '[download]  Downloading item 2',
    '[download] C:\\d\\a.mp3 has already been downloaded', '[download] has already been downloaded', '[download]  has already been downloaded',
    '[download] \u00e9\u00e8 \u65e5\u672c\u8a9e.mp3 has already been downloaded', '[download] Destination: C:\\d\\\u00e9.mp3',
    '[FILEPATH]C:\\d\\a.mp3', '[FILEPATH]\\\\x\\y', 'ERROR: boom', '  ERROR: boom', 'WARNING: x', '[youtube] Extracting URL: x',
    '[download] 100% of 5.23MiB in 00:03 at 1.7MiB/s', '[download]  45.2% of ~5.23MiB at 1.2MiB/s ETA 00:12 (frag 3/10)',
    '[download]  45.2% of ~5.23MiB at 1.2MiB/s ETA 00:12\r', '45.2% of 5MiB at 1MiB/s ETA 00:01', '45% of 5MiB at 1MiB/s ETA',
    '[download]  45.2%  of  5MiB  at  1MiB/s  ETA  00:01', '[download] 45.2% of5MiB at 1MiB/s ETA 00:01',
    '[download] 45.2% of 5MiB at 1MiB/s eta 00:01', '[download] 45.2% OF 5MiB AT 1MiB/s ETA 00:01',
    '[download] \uff14\uff15.2% of 5MiB at 1MiB/s ETA 00:01', '[download] .5% of 5MiB at 1MiB/s ETA 00:01',
    '[download] 1.% of 5MiB at 1MiB/s ETA 00:01', '[download] 0045.50% of 005.00MiB at 001MiB/s ETA 0:1',
    '[download] 50% of 5MiB at 1MiB/s ETA 00:01 [download] 80% of 9MiB at 2MiB/s ETA 00:02',
    '[download] 50%\u00a0of\u00a05MiB\u00a0at\u00a01MiB/s\u00a0ETA\u00a000:01', '[download] 50%\ufeffof\ufeff5MiB at 1MiB/s ETA 00:01',
    '[download] 50%\u0085of 5MiB at 1MiB/s ETA 00:01', '[download] 50% of 5MiB at 1MiB/s ETA 00:01\u2028x',
    '[download] 50% of 400' + '0'.repeat(300) + 'GiB at 1MiB/s ETA 00:01', '[download] 0% of 400' + '0'.repeat(300) + 'GiB at 1MiB/s ETA 00:01',
    '[download] 50% of 5MiB at ' + '9'.repeat(300) + 'B/s ETA 00:01',
    '\u65e5\u672c\u8a9e', '\ud83d\ude00 [PROGRESS]5|10|NA|1|1', '[PROGRESS]\ud83d\ude00');
  const nums = ['0', '1', '500', '12.5', 'NA', 'Unknown', '', '-5', '1e3', '0x10', ' 7 ', '1e400', '99999999999', '1_0', '+3', '.5', '5.',
    '0b11', 'Infinity', '-0', '1\r', '\u00a09', '0o7', '1,5'];
  const gen = new Set();
  for (const dl of nums) for (const t of nums) gen.add(`[PROGRESS]${dl}|${t}|NA|1|1`);
  for (const t of ['NA', '0', '', '0.0']) for (const e of nums) gen.add(`[PROGRESS]50|${t}|${e}|1|1`);
  for (const s of nums) for (const e of nums) gen.add(`[PROGRESS]50|100|NA|${s}|${e}`);
  const pcts = ['0', '0.0', '45.2', '100', '100.0', '99.99', '250', '007'];
  const sizes = ['5.23MiB', '~5.23MiB', '~ 5.23MiB', '10KiB', '1GB', '2tb', '3 MiB', '5.23', '5.23xyz', '1.2.3KiB', '12B', 'Unknown', '~', '1.5PiB'];
  const speeds = ['1.2MiB/s', '500KiB/s', 'Unknown', 'N/A', '2.5kb/s', '0B/s', '1.5MIB/S', '1e3B/s', '10', '1.2MiB/sec'];
  const etas = ['00:12', '1:02:03', 'Unknown', 'NA', '00:00', '12', '::', '1:2:3:4', '00:0x', '-1:30', 'na'];
  const fb = (p, s, v, e) => `[download] ${p}% of ${s} at ${v} ETA ${e}`;
  for (const p of pcts) gen.add(fb(p, sizes[0], speeds[0], etas[0]));
  for (const s of sizes) gen.add(fb(pcts[2], s, speeds[0], etas[0]));
  for (const v of speeds) gen.add(fb(pcts[2], sizes[0], v, etas[0]));
  for (const e of etas) gen.add(fb(pcts[2], sizes[0], speeds[0], e));
  let seed = 12345;
  const rnd = (n) => { seed = (seed * 1103515245 + 12345) % 2147483648; return seed % n; };
  for (let i = 0; i < 300; i++) gen.add(fb(pcts[rnd(pcts.length)], sizes[rnd(sizes.length)], speeds[rnd(speeds.length)], etas[rnd(etas.length)]));

  const states = [
    { status: 'downloading', percent: 0, last: 0 },
    { status: 'converting', percent: 77, last: 50 },
    { status: 'embedding', percent: 12, last: 12 },
    { status: 'queued', percent: 5, last: 5 },
  ];
  const parseLine = [];
  for (const line of hand) for (const s of states) {
    const p = base({ status: s.status, percent: s.percent });
    parseLine.push({ line, init: s, out: feed(line, p, s.last) });
  }
  for (const line of gen) {
    if (hand.has(line)) continue;
    parseLine.push({ line, init: states[0], out: feed(line, base(), 0) });
  }

  // ── Sequences through one progress object ──────────────────────────────────
  const pool = [...hand].filter((l) => l.length < 200);
  const seqs = [
    ['[download] Destination: x', '[PROGRESS]10|100|NA|5|9', '[PROGRESS]50|100|NA|5|5', '[PROGRESS]50|100|NA|5|5', '[PROGRESS]30|100|NA|5|5',
      '[PROGRESS]100|100|NA|5|0', '[Merger] Merging', '[PROGRESS]100|100|NA|0|0', '[Metadata] x', '[PROGRESS]1|2|NA|1|1', '[ExtractAudio] y',
      '[download] Downloading item 2 of 3', '[PROGRESS]5|100|NA|1|9', '[PROGRESS]5|100|NA|1|9'],
    ['[PROGRESS]10|100|NA|1|1', '[PROGRESS]20|NA|NA|1|1', '[PROGRESS]30|NA|200|1|1', '[PROGRESS]40|0|0|1|1', '[PROGRESS]NA|NA|NA|NA|NA', '[PROGRESS]5|100|NA|1|1'],
    ['[download]  10.0% of ~5.00MiB at 1.0MiB/s ETA 00:05', '[download]  60.0% of ~5.00MiB at 1.0MiB/s ETA 00:02', '[download]  60.0% of ~5.00MiB at 1.0MiB/s ETA 00:02',
      '[download]  20.0% of ~5.00MiB at 1.0MiB/s ETA 00:04', '[download] 250% of 5.00MiB at 1.0MiB/s ETA 00:00', '[download] 100% of 5.00MiB in 00:03',
      '[PROGRESS]1|10|NA|1|1', '[download]  50.0% of 8.00MiB at 3.0MiB/s ETA 00:01'],
    ['[EmbedThumbnail] a', '[PROGRESS]1|10|NA|1|1', '[ExtractAudio] b', '[PROGRESS]2|10|NA|1|1', '[download] Downloading item 3', '[PROGRESS]3|10|NA|1|1',
      '[Merger] c', '[download] Downloading item 4 of 4', '[Metadata] d', '[download] 70% of 1MiB at 1MiB/s ETA 00:01'],
  ];
  for (let i = 0; i < 80; i++) seqs.push(Array.from({ length: 14 }, () => pool[rnd(pool.length)]));
  const sequences = seqs.map((lines) => {
    const p = base();
    let last = 0;
    return {
      lines,
      steps: lines.map((line) => { const r = feed(line, p, last); last = typeof r.last === 'number' ? r.last : last; return r; }),
    };
  });

  // ── Size, speed, eta ───────────────────────────────────────────────────────
  const values = ['0', '1', '1.5', '.5', '5.', '1.2.3', '..', '.', '007', '123456789012345678901234567890', '1e3', '2.5e', '-5', '1,5'];
  const units = ['B', 'b', 'KB', 'kb', 'Kb', 'KiB', 'kib', 'KIB', 'MB', 'MiB', 'mib', 'GB', 'GiB', 'TB', 'TiB', 'tib', 'PB', 'EB', 'XB', 'K', 'Ki', 'iB', '', 'KBB', 'MiBx'];
  const seps = ['', ' ', '  ', '\t', '\u00a0', '\u2003', '\ufeff', '\u0085', '\u180e', '\n'];
  const sizeStrs = new Set(['', ' ', 'MiB', '~5MiB', 'x5MiBy', 'at 5 MB/s', '5 5 MB', '5MiB 6GiB', '\u0661KB', '5K\u212aB', '5\u017fB', '5M\u0131B', '400' + '0'.repeat(300) + 'B']);
  for (const v of values) for (const u of units) for (const s of seps) sizeStrs.add(v + s + u);
  const size = [...sizeStrs].map((s) => ({ s, out: enc(d['parseSize'](s)) }));
  const speedStrs = new Set(sizeStrs);
  for (const s of ['1.5MiB', '2KB', '3 GB', '5B', '0B', 'MiB', '']) for (const x of ['/s', '/S', '/s ', '/sec', '/s/s', '/', 's', '/s\n', ' /s']) speedStrs.add(s + x);
  const speed = [...speedStrs].map((s) => ({ s, out: enc(d['parseSpeed'](s)) }));
  const etaStrs = ['', '00:12', '1:02:03', '12', '0', 'Unknown', 'unknown', 'UNKNOWN', 'NA', 'na', 'N/A', 'nA', 'Na', 'banana', 'NaN', '1:', '::', ':5', '1::2',
    '-1:30', '+1:30', '1:-30', '00:0x', '0x10', '1e3', '1.5', '1.5:2.5', '  5', '5  ', ' : ', '\u0661\u0662', '12:34:56:78', '999999999999999999999',
    '1:60', 'abc', 'Unk', 'n', 'a', '\uff2e\uff21', '\u017f', '\u0130', 'n\u0131', '\u212a', '-0', '-0:-0', '1:2\r', '0012:0034', '1:2:3:4:5:6:7:8:9',
    '1_000', '\u00a05', '5\u00a0', '1 2', '1:2 3', '+', '-', '--5', '+-5', '5-', '1e999:1'];
  const eta = etaStrs.map((s) => ({ s, out: enc(d['parseEta'](s)) }));

  // ── The whole output handling of spawnDownload, via a real child process ────
  const dir = mkdtempSync(path.join(os.tmpdir(), 'ahg-progress-'));
  const child = path.join(dir, 'child.cjs');
  writeFileSync(child,
    'const steps = JSON.parse(process.env.AHG_STEPS); const code = Number(process.env.AHG_CODE);\n' +
    '(async () => { for (const [s, t] of steps) { process[s].write(t); await new Promise((r) => setTimeout(r, 25)); } process.exitCode = code; })();\n');
  const ascii = (s) => s.replace(/[\u0080-\uffff]/g, (c) => '\\u' + c.charCodeAt(0).toString(16).padStart(4, '0'));
  let n = 0;
  const run = ({ steps, code }) => new Promise((resolve) => {
    const events = [];
    process.env.AHG_STEPS = ascii(JSON.stringify(steps));
    process.env.AHG_CODE = String(code);
    d['spawnDownload'](process.execPath, { taskId: `t${n++}` }, [child], (p) => events.push(copy(p)),
      (_task, filepath, filepaths, alreadyOnDisk) => resolve({ events, done: { path: filepath, filepaths, alreadyOnDisk } }),
      (_task, error) => resolve({ events, error }));
  });

  const A = '[PROGRESS]5|10|NA|1|2\r\n[Merger] x\nERROR: bo\u00f6m\n  ERROR: 2\n[download]  50.0% of 5MiB at 1MiB/s ETA 00:01\n[Metadata] y';
  const B = '[FILEPATH]C:\\d\\a\u00e9.mp3\r\n[download] C:\\d\\a.mp3 has already been downloaded\n[PROGRESS]7|10|NA|1|2\n[FILEPATH]\\\\x\\y\n[FILEPATH]C:/d/b.mp3 \n';
  const C = '[download] C:\\d\\\u65e5\u672c.mp3 has already been downloaded\r\n[FILEPATH]C:\\d\\\u65e5\u672c.mp3\n[FILEPATH]  C:\\d\\z.mp3\n[FILEPATH]C:\\d\\tail.mp3';
  const scenarios = [];
  for (const [text, stream, code] of [[A, 'stderr', 1], [A, 'stdout', 0], [B, 'stdout', 0], [C, 'stdout', 0], [C, 'stderr', 2]]) {
    const chars = [...text];
    for (let i = 0; i <= chars.length; i++) {
      scenarios.push({ steps: [[stream, chars.slice(0, i).join('')], [stream, chars.slice(i).join('')]], code });
    }
  }
  const hs = (s, t) => [s, t];
  scenarios.push(
    { steps: [], code: 0 }, { steps: [], code: 1 }, { steps: [hs('stdout', '')], code: 0 },
    { steps: [hs('stderr', '[download]  45.2% of ~5.23MiB at 1.2MiB/s ETA 00:12\n')], code: 0 },
    { steps: [hs('stdout', '[FILEPATH]C:\\Downloads\\Test Song.mp3\n')], code: 0 },
    { steps: [hs('stdout', '[PROGRESS]420|1000|NA|1|1\n[Merger] x\n[FILEPATH]C:\\Downloads\\stale.mp3\n')], code: 1 },
    { steps: [hs('stdout', '[FILEPATH]C:\\Downloads\\Song - Part1.mp3\n[FILEPATH]C:\\Down'), hs('stdout', 'loads\\Song - Part2.mp3\n')], code: 0 },
    { steps: [hs('stdout', '[PROGRESS]500|1000|NA|100|3\n'), hs('stdout', '[PROGRESS]600|NA|NA|4096|NA\n'), hs('stdout', '[PROGRESS]700|NA|NA|NA|NA\n')], code: 0 },
    { steps: [hs('stdout', '[youtube] Extracting URL\n[download] Destination: C:/x.webm\nC:/bare/path.mp3\n')], code: 0 },
    { steps: [hs('stdout', '[FILE'), hs('stdout', 'PATH]C:/d/x.mp3\n')], code: 0 },
    { steps: [hs('stdout', '[FILEPATH]\\\\attacker\\share\\x.mp3\n[FILEPATH]\\\\?\\C:\\x.mp3\n')], code: 0 },
    { steps: [hs('stdout', '[FILEPATH]C:/d/1.mp3\n[download] x\n[FILEPATH]C:/d/2.mp3\n[FILEPATH]C:/d/3.mp3\n')], code: 0 },
    { steps: [hs('stdout', '[download] C:/Downloads/a.mp3 has already been downloaded\n[FILEPATH]C:/Downloads/a.mp3\n')], code: 0 },
    { steps: [hs('stdout', '[download] C:\\d\\1.mp3 has already been downloaded\n[FILEPATH]C:\\d\\1.mp3\n[FILEPATH]C:\\d\\2.mp3\n')], code: 0 },
    { steps: [hs('stdout', '[download] C:\\d\\1.mp3 has already been downloaded\n[FILEPATH]C:\\d\\1.mp3\n[FILEPATH]C:\\d\\1.mp3\n')], code: 0 },
    { steps: [hs('stdout', '[download]  C:\\d\\1.mp3  has already been downloaded\n[FILEPATH]C:\\d\\1.mp3\n')], code: 0 },
    // An indented marker, and a marker inside other text (a title can hold one): neither is a report.
    { steps: [hs('stdout', '   [FILEPATH]C:/spoof/lead.mp3\nDeleting original file C:/o/[PROGRESS]1|2.mp4\n')], code: 0 },
    { steps: [hs('stdout', '[PROGRESS]10|100|NA|1|1\n[Merger] Merging\n[Metadata] Adding\n[PROGRESS]100|100|NA|1|0\n[download] Downloading item 2 of 3\n[PROGRESS]5|100|NA|1|9\n')], code: 0 },
    { steps: [hs('stderr', 'ERROR: [youtube] x: Video unavailable\n'), hs('stderr', 'ERROR: second\n')], code: 1 },
    { steps: [hs('stderr', 'WARNING: only a warning\n')], code: 1 },
    { steps: [hs('stderr', 'ERROR: no newline at the end')], code: 1 },
    { steps: [hs('stderr', '   \n  ERROR: indented\n')], code: 1 },
    { steps: [hs('stderr', 'this ERROR: is not at the start\n')], code: 1 },
    { steps: [hs('stderr', 'ERROR: with CRLF\r\nERROR: second\r\n')], code: 1 },
    { steps: [hs('stdout', 'ERROR: on stdout\n')], code: 1 },
    { steps: [hs('stderr', '  \n'), hs('stderr', ' ')], code: 1 },
    { steps: [hs('stderr', '[PROGRESS]5|10|NA|1|1'), hs('stdout', '[PROGRESS]6|10|NA|1|1'), hs('stderr', '\n'), hs('stdout', '\n')], code: 0 },
    { steps: [hs('stderr', '[PROGRESS]5|10|NA|1|1'), hs('stdout', '[FILEPATH]C:/q.mp3')], code: 0 },
    { steps: [hs('stdout', '[FILEPATH]C:/a.mp3\n[Merger] x\n[PROGRESS]1|2|NA|1|1\n')], code: 7 },
  );

  const results = new Array(scenarios.length);
  let next = 0;
  await Promise.all(Array.from({ length: 12 }, async () => {
    for (;;) { const i = next++; if (i >= scenarios.length) return; results[i] = await run(scenarios[i]); }
  }));
  rmSync(dir, { recursive: true, force: true });
  const spawned = scenarios.map((s, i) => ({ ...s, out: results[i] }));

  const golden = { parseLine, sequences, size, speed, eta, scenarios: spawned };
  // serde_json (without its float_roundtrip feature) can misread a float by one
  // digit, so every number that is not a safe integer travels as {"$f": "<text>"}
  // and the Rust test parses the text with Rust's own, exact, parser.
  const tag = (_k, v) => (typeof v === 'number' && Number.isFinite(v) && !Number.isSafeInteger(v) ? { $f: String(v) } : v);
  writeFileSync(`${OUT}/progress.json`, JSON.stringify(golden, tag) + '\n');
  console.log(`progress.json: parseLine ${parseLine.length}, sequences ${sequences.length}, size ${size.length}, speed ${speed.length}, eta ${eta.length}, scenarios ${spawned.length}`);
};
