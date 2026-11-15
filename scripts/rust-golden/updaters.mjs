// Golden file for the two self-updaters: electron/updater.ts (the app) and
// electron/ytdlp-updater.ts (the yt-dlp engine). The pure functions are called
// over a wide corpus; the real classes are driven through scripted scenarios
// with globalThis.fetch replaced by a fake that records every request, a window
// that records every send, and a real scratch folder for the files they write.
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const UA = 'AHG-Universal-Converter';
const API = 'https://api.github.com/repos/amyyne01/UNIVERSAL-CONVERTER-/releases/latest';
const YAPI = 'https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest';
const EXE = 'AHG-Converter-1.2.0.exe';
const BASE = 'https://github.com/amyyne01/UNIVERSAL-CONVERTER-/releases/download/v1.2.0/';
const ASSET_URL = BASE + EXE;
const SUMS_URL = `${BASE}SHA256SUMS.txt`;
const BYTES = 'pretend this is a portable exe';
const YASSET = process.platform === 'win32' ? 'yt-dlp.exe' : 'yt-dlp';
const YBASE = 'https://github.com/yt-dlp/yt-dlp/releases/download/2026.07.01/';
const YASSET_URL = YBASE + YASSET;
const YSUMS_URL = `${YBASE}SHA2-256SUMS`;
const YBYTES = '#!/fake-yt-dlp binary';

const sha = (s) => createHash('sha256').update(s).digest('hex');
const asset = (name, url) => ({ name, browser_download_url: url });
const feed = (over = {}) => ({ tag_name: 'v1.2.0', assets: [asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)], ...over });
const yfeed = (over = {}) => ({ tag_name: '2026.07.01', assets: [asset(YASSET, YASSET_URL), asset('SHA2-256SUMS', YSUMS_URL)], ...over });
/** A downloadable payload: its bytes as text, the sha the owner would compute, the chunk sizes it streams in. */
const payload = (bytes, over = {}) => {
  const n = Buffer.byteLength(bytes);
  return { bytes, sha: sha(bytes), chunks: n > 10 ? [10, n - 10] : [n], ...over };
};
const std = (over = {}) => ({
  [API]: { json: feed() },
  [SUMS_URL]: { text: `${sha(BYTES)}  ${EXE}\n` },
  [ASSET_URL]: payload(BYTES),
  ...over,
});
const ystd = (over = {}) => ({
  [YAPI]: { json: yfeed() },
  [YSUMS_URL]: { text: `${sha(YBYTES)}  ${YASSET}\n` },
  [YASSET_URL]: payload(YBYTES),
  ...over,
});

/** Every string in a value, replaced by f(string). */
const deep = (v, f) => (typeof v === 'string' ? f(v) : Array.isArray(v) ? v.map((x) => deep(x, f))
  : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, deep(x, f)])) : v);
const plain = (v) => (v === undefined ? null : JSON.parse(JSON.stringify(v)));

/** The scripted fetch. A route is {ok,status,url,json,text,bytes,sha,chunks,noBody,contentLength,throws,streamError}. */
function makeFetch(routes, requests, gate) {
  return async (url, init) => {
    requests.push({ url, headers: init?.headers ?? null });
    if (gate.hold) { const g = gate.hold; gate.hold = null; await g; }
    const r = routes[url];
    if (!r) throw new Error(`no route: ${url}`);
    if (r.throws !== undefined) throw new Error(r.throws);
    const ok = r.ok ?? true;
    const buf = Buffer.from(r.bytes ?? '');
    const res = {
      ok, status: r.status ?? (ok ? 200 : 500),
      json: async () => r.json, text: async () => r.text ?? '',
      arrayBuffer: async () => buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength),
      headers: { get: (h) => (h === 'content-length' ? (r.contentLength === undefined ? String(buf.length) : r.contentLength) : null) },
    };
    if (r.url !== undefined) res.url = r.url;
    if (r.bytes !== undefined && !r.noBody) {
      let at = 0;
      let sent = 0;
      res.body = new ReadableStream({
        pull(c) {
          if (r.streamError && sent === r.streamError.after) return c.error(new Error(r.streamError.message));
          if (sent >= r.chunks.length) return c.close();
          c.enqueue(new Uint8Array(buf.subarray(at, at + r.chunks[sent])));
          at += r.chunks[sent++];
        },
      });
    }
    return res;
  };
}

/** Every file under `dir`, as relative path -> text; null when it does not exist. */
function snapshot(dir) {
  if (!fs.existsSync(dir)) return null;
  const out = {};
  const walk = (d) => {
    for (const name of fs.readdirSync(d).sort()) {
      const p = path.join(d, name);
      if (fs.statSync(p).isDirectory()) { out[`${path.relative(dir, p)}\\`] = null; walk(p); } else out[path.relative(dir, p)] = fs.readFileSync(p, 'utf-8');
    }
  };
  walk(dir);
  return out;
}

const tick = (ms = 15) => new Promise((r) => setTimeout(r, ms));

export default async function updaters({ OUT, literals }) {
  const up = await import('../../dist-electron/electron/updater.js');
  const yd = await import('../../dist-electron/electron/ytdlp-updater.js');
  const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'ahg-golden-upd-'));
  const tmpOf = (v) => deep(v, (s) => s.replaceAll(TMP, '{TMP}'));
  const realOf = (v) => deep(v, (s) => s.replaceAll('{TMP}', TMP));

  // ── pure functions ─────────────────────────────────────────────────────────
  const files = ['electron/updater.test.ts', 'electron/ytdlp-updater.test.ts'];
  const lits = [...new Set(files.flatMap((f) => literals(f)))];

  const hostUrls = [
    'https://github.com/a/b', 'https://objects.githubusercontent.com/x', 'https://release-assets.githubusercontent.com/x', 'https://api.github.com/x',
    'https://GITHUB.COM/a', 'https://Objects.GithubUserContent.com/a', 'https://github.com', 'https://api.github.com:443/x', 'https://github.com:8443/a',
    'https://github.com:443/a', 'https://github.com:99999/a', 'https://github.com:/a', 'https://github.com./a', 'https://github.com../a', 'https://.github.com/a',
    'https://github.com.evil.com/a', 'https://evilgithub.com/a', 'https://evil-github.com/a', 'https://github.com@evil.com/a', 'https://evil.com@github.com/a',
    'https://evil.com\\@github.com/a', 'https://github.com\\@evil.com/a', 'https://github.com:pw@evil.com/a', 'https://user:pw@github.com/x', 'https://github.com:@github.com/x',
    'https://evil.com#@github.com/', 'https://evil.com?@github.com/', 'https://evil.com/@github.com/', 'https://github.com%2eevil.com/a', 'https://github%2ecom/a',
    'https://git%68ub.com/a', 'https://g\u0131thub.com/a', 'https://github.c\u00F6m/a', 'https://\uFF47ithub.com/a', 'https://github\u3002com/a',
    'https://github.com\u3002/a', 'https://xn--github-9ua.com/a', 'https://xn--ithub-.com/a', 'https://git\u200Bhub.com/a', 'https://b\u00FCcher.example/x',
    'https://a.githubusercontent.com/x', 'https://githubusercontent.com/x', 'https://raw.githubusercontent.com/x', 'https://codeload.github.com/x',
    'https://www.github.com/x', 'https://github.io/x', 'https://gist.github.com/x', 'https://api.github.com.evil.com/x', 'https://objects.githubusercontent.com.evil.com/x',
    'http://github.com/a', 'http://api.github.com/a', 'ftp://github.com/a', 'HTTPS://github.com/a', 'Https://GitHub.com/a', 'wss://github.com/a',
    '//github.com/a', '//evil.com/a', '//evil.com', '\\\\evil.com\\a', '\\\\github.com\\a', '/\\evil.com/a', '/\\github.com/a',
    'file:///c:/x', 'javascript:alert(1)', 'data:text/plain,x', 'blob:https://github.com/x', 'mailto:a@github.com',
    '', ' ', '\t\n', 'github.com/a.exe', '/a/b.exe', './a.exe', '../a.exe', 'a.exe', 'releases/download/v1/app.exe', '?x=1', '#frag',
    'https:github.com/a', 'https:/evil.com/a', 'https:evil.com/a', 'https:///github.com/a', 'https:///evil.com/a', 'https://github.com\\a', 'https://github.com\\\\evil.com',
    'https://[::1]/a', 'https://127.0.0.1/a', 'https://2130706433/a', 'https://0x7f.1/a', 'https://git hub.com/a', ' https://github.com/a', '\thttps://github.com/a\n',
    'https://github.com/a\nb', 'https://git\nhub.com/a', 'https://github.com/%2e%2e/x', 'https://evil.example/releases/download/v1/app.exe', 'https://evil.example/x',
    'https://' + 'a'.repeat(300) + '.com/x', 'https://github.com/' + 'a'.repeat(5000),
  ];
  const baseCases = [
    ['releases/x.exe', 'https://evil.com/'], ['x', 'https://api.github.com/a/b'], ['//github.com/x', 'http://evil.com/'], ['https://github.com/x', 'not a url'],
    ['/x', 'http://github.com/'], ['/x', 'https://objects.githubusercontent.com/a'], ['x', ''], ['', 'https://evil.com/'], ['', 'https://github.com/'],
  ];
  const trusted = [...new Set([...hostUrls, ...lits])].map((url) => ({ url, out: up.isTrustedReleaseHost(url) }))
    .concat(baseCases.map(([url, base]) => ({ url, base, out: up.isTrustedReleaseHost(url, base) })));

  const versions = [
    '1.0.0', '1.0.0', 'v1.0.0', '1.0', '1', '1.0.0.0', '1.0.0.1', '1.0.1', '1.1.0', '2.0.0', '0.9.9', '1.10.0', '1.9.0', '1.2.0', 'v1.2.0', 'V1.2.0', 'vv1.2.0',
    '1.2.0-beta', '1.2.0-beta.1', '1.2.0+build', '1.2.0rc1', '1.2.x', '1.x.0', 'x.y.z', 'garbage', '', 'v', '.', '..', '1..2', '.1', '1.', '01.02.03', '1.02', '1.2e1',
    '1.2.0 ', ' 1.2.0', '\u00A01.2.0', '\uFEFF1.2.0', '1. 2.0', '-1.0.0', '1.-1.0', '+1.0.0', '1.+2.0', '1.0.0-1', '99999999999999999999.0.0', '1' + '0'.repeat(400),
    '0x10.0', '1_0.0', '\uFF11.0.0', '\u0661.0.0', '2026.07.01', '2026.01.01', '1.2.3.4.5', '0.0.0', '0', 'v0.0.1', 'Infinity.0', 'NaN.0', '1,5.0',
  ];
  const newer = [];
  for (const a of versions) for (const b of versions) newer.push({ a, b, out: up.isNewerVersion(a, b) });
  for (const l of lits) { newer.push({ a: l, b: '1.0.0', out: up.isNewerVersion(l, '1.0.0') }); newer.push({ a: '1.0.0', b: l, out: up.isNewerVersion('1.0.0', l) }); }

  const H = sha(BYTES);
  const sumsTexts = [
    `${H}  ${EXE}\n`, `${H} ${EXE}`, `${H} *${EXE}`, `${H}  *${EXE}`, `${H}\t${EXE}`, `${H}\t\t*${EXE}\r\n`, `${H} \t ${EXE}`, `${H}${EXE}`, `${H}*${EXE}`,
    `${H.toUpperCase()}  ${EXE}`, `${H.slice(0, 32).toUpperCase()}${H.slice(32)}  ${EXE}`, `${H}  ${EXE}\r\n`, `${H}  ${EXE}\r\n\r\n`, `a\r\n${H}  ${EXE}\r\nb`,
    `${H}  ${EXE}\r`, `${H}  ${EXE}\rjunk`, `${H}  ${EXE}\u2028${H}  other.exe`, `x\ry\n${H}  ${EXE}`, `deadbeef  other.exe\n${H}  ${EXE}\n`, `${H}  other.exe\n${H}  ${EXE}\n`,
    `${'0'.repeat(64)}  ${EXE}\n${H}  ${EXE}\n`, `${H}  ${EXE}\n${'0'.repeat(64)}  ${EXE}\n`, `${H}  ${EXE}  \n`, `  ${H}  ${EXE}  `, `\u00A0${H}\u00A0\u00A0${EXE}\u00A0`,
    `${H}\u3000${EXE}`, `${H}\u200B${EXE}`, `\uFEFF${H}  ${EXE}`, `${H}  ${EXE} extra`, `${H}  dir/${EXE}`, `${H}  ./${EXE}`, `${H}  ${EXE.toUpperCase()}`, `${H}  ${EXE.toLowerCase()}`,
    `${H}  ${EXE} `, `${H}  ${EXE}\u00A0`, `${H.slice(0, 63)}  ${EXE}`, `${H}0  ${EXE}`, `${H}0${H}  ${EXE}`, `${H.slice(0, 63)}g  ${EXE}`, `${H}  `, `${H} *`, `${H}  *`, `${H} **${EXE}`,
    `${H}  *${EXE}\n${H}  ${EXE}`, '', '\n', '\r\n', '   ', 'garbage line', `# comment\n${H}  ${EXE}`, `SHA256 (${EXE}) = ${H}`, `${H}  ${EXE}`.replaceAll(' ', '\u00A0'),
    `${H}  ${EXE}\n`.repeat(3), `x${H}  ${EXE}`, `${H}x  ${EXE}`, `${H}\n${EXE}`, `${H}  \n${EXE}`, `${H} \u2028 ${EXE}`,
  ];
  const names = [EXE, 'other.exe', 'nope.exe', 'yt-dlp.exe', 'yt-dlp', '', ' ', '*', `*${EXE}`, EXE.toUpperCase(), `${EXE} `, ` ${EXE}`, `dir/${EXE}`, '\u00E9.exe'];
  const checksum = [];
  for (const sums of sumsTexts) for (const name of names) checksum.push({ sums, name, out: up.parseChecksum(sums, name) });
  for (const l of lits) {
    checksum.push({ sums: l, name: EXE, out: up.parseChecksum(l, EXE) });
    checksum.push({ sums: `${H}  ${l}`, name: l, out: up.parseChecksum(`${H}  ${l}`, l) });
  }

  const pathsS = ['C:\\staged\\new.exe', 'D:\\Apps\\app.exe', 'C:\\Users\\me\\AppData\\Roaming\\AHG\\updates\\AHG-Converter-1.2.0.exe', 'D:\\My Apps\\AHG Converter.exe',
    'C:\\a & b\\x.exe', 'C:\\100%\\x.exe', 'C:\\%TEMP%\\x.exe', 'C:\\^caret\\x.exe', 'C:\\p(aren)\\x.exe', 'C:\\!bang!\\x.exe', 'C:\\\u00E9t\u00E9 \u65E5\u672C\u8A9E\\\u{1F600}.exe',
    'C:\\say "hi"\\x.exe', '', ' ', '\\\\server\\share\\x.exe', 'x.exe', 'C:/forward/slash.exe', 'C:\\a\r\nb.exe', 'C:\\a|b>c<d.exe'];
  const swap = [];
  for (const staged of pathsS) for (const target of pathsS.slice(0, 11)) swap.push({ staged, target, out: up.swapScript(staged, target) });

  const stale = [...new Set([...lits, 'ERROR: Unable to extract player response', 'nsig extraction failed; please report this issue', 'ERROR: Video unavailable', '',
    'UNABLE TO EXTRACT', 'Unable To Extract', 'unable  to extract', 'unable to extrac', 'failed to extract', 'FAILED TO EXTRACT x', 'nsig extraction failed', 'NSIG EXTRACTION FAILED',
    'update to the latest version', 'Please UPDATE TO THE LATEST VERSION of yt-dlp', 'yt-dlp is out of date', 'YT-DLP IS OUT OF DATE', 'please report this issue', 'Please Report This Issue on https://github.com',
    'ERROR: [youtube] abc: Sign in to confirm you\u2019re not a bot', 'ERROR: Private video', 'HTTP Error 429', 'unable to extract\u0130', '\u212Aunable to extract', 'unable to extract'.toUpperCase(),
    'u\u0307nable to extract', 'unable\u00A0to extract', 'x'.repeat(5000) + 'unable to extract', 'yt-dlp is out of date\u0000', '\u0130 please report this issue', 'please report this issu\u0065',
  ])].map((text) => ({ text, out: yd.isEngineStale(text) }));

  const joinPairs = [['C:\\userData\\updates', 'x.exe'], ['C:\\userData\\updates\\', 'x.exe'], ['C:/userData/updates', 'x.exe'], ['C:\\a\\b', '..\\x.exe'], ['C:\\a\\b', '..\\..\\..\\x.exe'],
    ['C:\\a\\b', '.\\x.exe'], ['C:\\a\\b', 'c\\..\\x.exe'], ['C:\\a\\b', 'c/d/x.exe'], ['C:\\a\\\\b', 'x.exe'], ['C:\\a\\b', ''], ['', 'x.exe'], ['', ''], ['C:', 'x.exe'], ['C:a', 'x.exe'],
    ['C:\\', 'x.exe'], ['C:\\', '..\\x.exe'], ['\\a\\b', '..\\..\\x.exe'], ['a\\b', '..\\..\\..\\x.exe'], ['a\\b', 'x.exe'], ['.', 'x.exe'], ['..', 'x.exe'], ['C:\\a b\\c & d', 'x y.exe'],
    ['C:\\a\\b', 'C:\\other\\x.exe'], ['C:\\a\\b', 'C:x.exe'], ['C:\\a\\b', '...\\x.exe'], ['C:\\a\\b', 'x.exe\\'], ['C:\\a\\b\\', 'x.exe\\'], ['C:\\a\\b', '\u00E9\u65E5.exe'], ['C:\\a\\b', 'x.exe.tmp'],
    ['C:\\a.b\\..c', '..\\..\\d'], ['C:\\a\\b', '.'], ['C:\\a\\b', '..'], ['C:\\a\\b\\', '..\\']]
    .map(([a, b]) => ({ a, b, out: path.join(a, b) }));

  // ── scenarios ──────────────────────────────────────────────────────────────
  const norm = (s) => (typeof s === 'string' ? { op: s } : s);
  const S = (name, o = {}) => ({ name, current: '1.0.0', routes: std(), steps: ['check', 'download'], ...o });
  const real = { fetch: globalThis.fetch, portable: process.env.PORTABLE_EXECUTABLE_FILE };
  let n = 0;

  async function runUpdater(sc) {
    const sandbox = path.join(TMP, `s${n++}`);
    const updateDir = path.join(sandbox, 'upd');
    fs.mkdirSync(sandbox);
    const sent = [];
    const requests = [];
    const spawns = [];
    let quits = 0;
    let current = sc.current;
    const gate = { hold: null };
    const win = sc.noWindow ? null : { webContents: { send: (channel, payload) => sent.push({ channel, payload: plain(payload) }) } };
    const routes = structuredClone(sc.routes);
    globalThis.fetch = makeFetch(routes, requests, gate);
    if (sc.env === undefined) delete process.env.PORTABLE_EXECUTABLE_FILE; else process.env.PORTABLE_EXECUTABLE_FILE = sc.env;
    const deps = {
      getWindow: () => win, currentVersion: () => current, updateDir: sc.dir ? path.join(sandbox, sc.dir) : updateDir,
      spawnFn: (cmd, args, opts) => { spawns.push({ cmd, args, opts }); return { unref() {} }; }, quit: () => { quits++; },
    };
    if ('target' in sc) deps.targetExe = () => sc.target;
    const u = new up.Updater(deps);
    const out = [];
    try {
      for (const raw of sc.steps) {
        const step = norm(raw);
        if (step.current !== undefined) current = step.current;
        if (step.routes) Object.assign(routes, step.routes);
        sent.length = 0; requests.length = 0; spawns.length = 0; quits = 0;
        let threw = null;
        try {
          if (step.op === 'check') await u.check();
          else if (step.op === 'download') await u.download();
          else if (step.op === 'install') u.install();
          else if (step.op === 'cleanup') u.cleanup();
        } catch (e) { threw = e.message; }
        out.push(tmpOf({ step, requests: [...requests], events: [...sent], state: plain(u.getState()), threw, files: snapshot(sandbox), spawns: [...spawns], quits }));
      }
    } finally {
      globalThis.fetch = real.fetch;
      if (real.portable === undefined) delete process.env.PORTABLE_EXECUTABLE_FILE; else process.env.PORTABLE_EXECUTABLE_FILE = real.portable;
    }
    return { ...sc, sandbox: path.basename(sandbox), out };
  }

  const U = [];
  const full = ['check', 'state', 'download', 'state', 'install', 'cleanup'];
  U.push(S('happy path', { steps: full, target: 'D:\\Apps\\AHG-Converter.exe' }));
  U.push(S('happy path, no window', { steps: full, target: 'D:\\Apps\\AHG-Converter.exe', noWindow: true }));
  U.push(S('up to date', { current: '1.2.0', steps: ['check', 'download', 'install'] }));
  U.push(S('newer than the feed', { current: '9.9.9', steps: ['check', 'state'] }));
  U.push(S('download before any check', { steps: ['download', 'install', 'cleanup'] }));
  U.push(S('install before download', { steps: ['check', 'install'], target: 'D:\\x.exe' }));
  U.push(S('install, no exe to replace', { steps: ['check', 'download', 'install'], target: null }));
  U.push(S('install, empty exe', { steps: ['check', 'download', 'install'], target: '' }));
  U.push(S('install, env fallback', { steps: ['check', 'download', 'install'], env: 'E:\\Portable\\AHG.exe' }));
  U.push(S('install, empty env', { steps: ['check', 'download', 'install'], env: '' }));
  U.push(S('install, injected beats env', { steps: ['check', 'download', 'install'], env: 'E:\\Portable\\AHG.exe', target: 'D:\\Apps\\X.exe' }));
  U.push(S('install twice, cleanup twice', { steps: ['check', 'download', 'install', 'install', 'cleanup', 'cleanup', 'install'], target: 'D:\\Apps\\X.exe' }));
  U.push(S('cleanup when nothing exists', { steps: ['cleanup', 'state'] }));
  U.push(S('check twice then download', { steps: ['check', 'check', 'download', 'check', 'state'] }));
  U.push(S('download twice', { steps: ['check', 'download', 'download', 'state'] }));
  U.push(S('a newer feed replaces the available one, an older one clears it', {
    steps: ['check', { op: 'check', current: '1.2.0' }, 'download', { op: 'check', current: '1.0.0' }, 'download'],
  }));
  U.push(S('a failed re-check keeps the earlier available update', {
    steps: ['check', 'state', { op: 'check', routes: { [API]: { ok: false, status: 503 } } }, 'state', 'download'],
  }));
  U.push(S('a fixed current version changing between steps', { steps: [{ op: 'check', current: '1.1.9' }, { op: 'check', current: 'v1.2.0' }, { op: 'check', current: '1.2.0-rc1' }, { op: 'check', current: '' }, { op: 'check', current: 'junk' }] }));

  for (const [tag, cur] of [['v1.2.0', '1.0.0'], ['1.2.0', '1.0.0'], ['V1.2.0', '1.0.0'], [' v1.2.0 ', '1.0.0'], ['vv1.2.0', '1.0.0'], ['v 1.2.0', '1.0.0'], ['1.2.0-beta', '1.2.0'], ['1.2.0-beta', '1.1.0'],
    ['1.2', '1.2.0'], ['1.2.0.1', '1.2.0'], ['v', '1.0.0'], ['', '1.0.0'], ['  ', '1.0.0'], ['\u00A0v1.3.0\u00A0', '1.0.0'], ['\uFEFF1.3.0', '1.0.0'], ['v1.10.0', '1.9.0'], ['v2.0', 'v1.9.9'],
    ['junk', '1.0.0'], ['v1.2.0', 'junk'], ['v1.2.0', ''], [null, '1.0.0'], [undefined, '1.0.0'], [123, '1.0.0'], [1.2, '1.0.0'], [1.2, '1.1'], [true, '0'], [{}, '1.0.0'], [[], '1.0.0'], [['v2.0.0'], '1.0.0'],
    [[1, null, 3], '0.0.0'], [1e21, '1.0.0'], [1e-7, '0'], [-1, '-2'], [0, '0']]) {
    const f = feed();
    if (tag === undefined) delete f.tag_name; else f.tag_name = tag;
    U.push(S(`tag ${JSON.stringify(tag)} vs ${JSON.stringify(cur)}`, { current: cur, routes: std({ [API]: { json: f } }), steps: ['check', 'state', 'download'] }));
  }

  const jsonCases = {
    'no assets key': { tag_name: 'v1.2.0' }, 'assets null': feed({ assets: null }), 'assets empty': feed({ assets: [] }), 'assets object': feed({ assets: {} }),
    'assets string': feed({ assets: 'abc' }), 'assets number': feed({ assets: 5 }), 'assets true': feed({ assets: true }), 'assets zero': feed({ assets: 0 }), 'assets false': feed({ assets: false }),
    'no exe': feed({ assets: [asset('SHA256SUMS.txt', SUMS_URL)] }), 'no sums': feed({ assets: [asset(EXE, ASSET_URL)] }),
    'exe uppercase ext': feed({ assets: [asset('AHG.EXE', ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }), 'exe mixed case ext': feed({ assets: [asset('AHG.ExE', ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'exe zip': feed({ assets: [asset('AHG.exe.zip', ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }), 'exe no dot': feed({ assets: [asset('AHGexe', ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'exe only ".exe"': feed({ assets: [asset('.exe', ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }), 'exe trailing space': feed({ assets: [asset('AHG.exe ', ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'sums renamed': feed({ assets: [asset(EXE, ASSET_URL), asset('sha256sums.txt', SUMS_URL)] }), 'sums with space': feed({ assets: [asset(EXE, ASSET_URL), asset('SHA256SUMS.txt ', SUMS_URL)] }),
    'sums first': feed({ assets: [asset('SHA256SUMS.txt', SUMS_URL), asset(EXE, ASSET_URL)] }),
    'two exes, first wins': feed({ assets: [asset(EXE, ASSET_URL), asset('Other.exe', 'https://github.com/o/other.exe'), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'two exes, second named like a zip first': feed({ assets: [asset('a.zip', 'https://github.com/o/a.zip'), asset('B.EXE', 'https://github.com/o/b.exe'), asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'two sums, first wins': feed({ assets: [asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL), asset('SHA256SUMS.txt', 'https://evil.example/s')] }),
    'exe url missing': feed({ assets: [{ name: EXE }, asset('SHA256SUMS.txt', SUMS_URL)] }), 'exe url empty': feed({ assets: [asset(EXE, ''), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'exe url null': feed({ assets: [asset(EXE, null), asset('SHA256SUMS.txt', SUMS_URL)] }), 'sums url empty': feed({ assets: [asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', '')] }),
    'exe skipped for missing url, not retried': feed({ assets: [{ name: 'a.exe' }, asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'null element before the exe': feed({ assets: [null, asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'null element after the exe, before sums': feed({ assets: [asset(EXE, ASSET_URL), null, asset('SHA256SUMS.txt', SUMS_URL)] }),
    'null element after both': feed({ assets: [asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL), null] }),
    'number element': feed({ assets: [5, asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }), 'string element': feed({ assets: ['x', asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'array element': feed({ assets: [[], asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'element without name': feed({ assets: [{ browser_download_url: ASSET_URL }, asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'element with null name': feed({ assets: [{ name: null, browser_download_url: ASSET_URL }, asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'numeric name': feed({ assets: [{ name: 5, browser_download_url: ASSET_URL }, asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'object name': feed({ assets: [{ name: {}, browser_download_url: ASSET_URL }, asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'array name': feed({ assets: [{ name: ['a.exe'], browser_download_url: ASSET_URL }, asset('SHA256SUMS.txt', SUMS_URL)] }),
    'numeric name after the exe': feed({ assets: [asset(EXE, ASSET_URL), { name: 5 }, asset('SHA256SUMS.txt', SUMS_URL)] }),
    'json null': null, 'json number': 5, 'json string': 'abc', 'json true': true, 'json array': [], 'json empty object': {}, 'json assets array of arrays': { assets: [[]] },
    'extra fields': { ...feed(), body: 'x', draft: false, assets: [{ ...asset(EXE, ASSET_URL), size: 1, id: 7 }, asset('SHA256SUMS.txt', SUMS_URL)] },
    'unicode exe name': feed({ assets: [asset('\u00E9t\u00E9-\u65E5\u672C.exe', BASE + '%C3%A9.exe'), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'exe name with spaces': feed({ assets: [asset('AHG Converter 1.2.0.exe', BASE + 'AHG.Converter.1.2.0.exe'), asset('SHA256SUMS.txt', SUMS_URL)] }),
    'exe url truthy non-string': feed({ assets: [{ name: EXE, browser_download_url: 7 }, asset('SHA256SUMS.txt', SUMS_URL)] }),
    'exe url falsy zero': feed({ assets: [{ name: EXE, browser_download_url: 0 }, asset('SHA256SUMS.txt', SUMS_URL)] }),
  };
  for (const [name, json] of Object.entries(jsonCases)) {
    const routes = std({ [API]: { json } });
    U.push(S(`feed: ${name}`, { routes, steps: ['check', 'state', 'download'] }));
  }

  const exeUrls = hostUrls.filter((_, i) => i % 2 === 0 || i < 40);
  for (const url of exeUrls) {
    const routes = std({ [API]: { json: feed({ assets: [asset(EXE, url), asset('SHA256SUMS.txt', SUMS_URL)] }) }, [url]: payload(BYTES) });
    U.push(S(`exe on ${JSON.stringify(url).slice(0, 70)}`, { routes, steps: ['check', 'download'] }));
  }
  for (const url of hostUrls.slice(0, 45)) {
    const routes = std({ [API]: { json: feed({ assets: [asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', url)] }) }, [url]: { text: `${sha(BYTES)}  ${EXE}\n` } });
    U.push(S(`sums on ${JSON.stringify(url).slice(0, 70)}`, { routes, steps: ['check', 'download'] }));
  }
  U.push(S('both on the same untrusted host', { routes: std({ [API]: { json: feed({ assets: [asset(EXE, 'https://evil.example/a.exe'), asset('SHA256SUMS.txt', 'https://evil.example/s')] }) } }) }));
  U.push(S('exe trusted, sums untrusted', { routes: std({ [API]: { json: feed({ assets: [asset(EXE, ASSET_URL), asset('SHA256SUMS.txt', 'https://evil.example/s')] }) } }) }));

  const finals = ['https://github.com/x', 'https://objects.githubusercontent.com/x', 'https://release-assets.githubusercontent.com/x', 'https://api.github.com/x', 'https://evil.example/x', 'http://github.com/x',
    'https://github.com.evil.com/x', '', 'not a url', '/relative', 'https://GITHUB.com/x', 'https://github.com@evil.com/x', 'HTTPS://objects.githubusercontent.com/x'];
  for (const final of finals) {
    U.push(S(`redirect of the feed to ${JSON.stringify(final)}`, { routes: std({ [API]: { json: feed(), url: final } }), steps: ['check', 'state'] }));
    U.push(S(`redirect of the sums to ${JSON.stringify(final)}`, { routes: std({ [SUMS_URL]: { text: `${sha(BYTES)}  ${EXE}\n`, url: final } }) }));
    U.push(S(`redirect of the exe to ${JSON.stringify(final)}`, { routes: std({ [ASSET_URL]: payload(BYTES, { url: final }) }) }));
  }
  for (const status of [301, 304, 400, 403, 404, 429, 500, 503, 0, 99, 600]) {
    U.push(S(`feed answers HTTP ${status}`, { routes: std({ [API]: { ok: false, status, json: feed() } }), steps: ['check', 'state'] }));
    U.push(S(`sums answer HTTP ${status}`, { routes: std({ [SUMS_URL]: { ok: false, status, text: '' } }) }));
    U.push(S(`exe answers HTTP ${status}`, { routes: std({ [ASSET_URL]: payload(BYTES, { ok: false, status }) }) }));
  }
  U.push(S('ok false with a 200 status', { routes: std({ [API]: { ok: false, status: 200, json: feed() } }), steps: ['check'] }));
  U.push(S('ok true with a 404 status', { routes: std({ [API]: { ok: true, status: 404, json: feed() } }), steps: ['check', 'download'] }));
  U.push(S('network failure on the feed', { routes: std({ [API]: { throws: 'offline' } }), steps: ['check', 'state', 'download'] }));
  U.push(S('network failure on the sums', { routes: std({ [SUMS_URL]: { throws: 'sums offline' } }) }));
  U.push(S('network failure on the exe', { routes: std({ [ASSET_URL]: { throws: 'exe offline' } }) }));
  U.push(S('empty-message failure', { routes: std({ [API]: { throws: '' } }), steps: ['check', 'state'] }));
  U.push(S('no route at all', { routes: {}, steps: ['check', 'state'] }));
  U.push(S('stream breaks after the first chunk', { routes: std({ [ASSET_URL]: payload(BYTES, { streamError: { after: 1, message: 'connection reset' } }) }) }));
  U.push(S('stream breaks before the first chunk', { routes: std({ [ASSET_URL]: payload(BYTES, { streamError: { after: 0, message: 'reset at once' } }) }) }));

  const ok = `${sha(BYTES)}  ${EXE}\n`;
  for (const [i, sums] of sumsTexts.entries()) {
    U.push(S(`sums file ${i}`, { routes: std({ [SUMS_URL]: { text: sums } }) }));
  }
  U.push(S('sums text for a different exe name', { routes: std({ [SUMS_URL]: { text: `${sha(BYTES)}  Other.exe\n` } }) }));
  U.push(S('mismatching hash', { routes: std({ [ASSET_URL]: payload(`${BYTES}!`) }) }));
  U.push(S('hash upper-cased in the sums matches', { routes: std({ [SUMS_URL]: { text: `${sha(BYTES).toUpperCase()}  ${EXE}\n` } }) }));
  U.push(S('empty payload', { routes: std({ [ASSET_URL]: payload('', { chunks: [] }), [SUMS_URL]: { text: `${sha('')}  ${EXE}\n` } }) }));
  U.push(S('payload with non-ASCII text', { routes: std({ [ASSET_URL]: payload('h\u00E9llo \u65E5\u672C\u8A9E \u{1F600}'), [SUMS_URL]: { text: `${sha('h\u00E9llo \u65E5\u672C\u8A9E \u{1F600}')}  ${EXE}\n` } }) }));

  // progress: chunk sizes (summing to the payload length) against content-length headers
  const P = 'p'.repeat(1000);
  const split = (sizes) => sizes;
  const progressRoutes = (chunks, contentLength, extra = {}) => std({ [ASSET_URL]: payload(P, { chunks: split(chunks), contentLength, ...extra }), [SUMS_URL]: { text: `${sha(P)}  ${EXE}\n` } });
  const chunkSets = {
    'ten steps': Array(10).fill(100), 'one chunk': [1000], 'hundred and one': [...Array(100).fill(9), 100], 'many tiny': Array(1000).fill(1), 'one byte first': [1, 999],
    'empty first': [0, 500, 500], 'empty between': [250, 0, 0, 250, 500], 'empty last': [1000, 0], 'repeating percents': [1, 1, 1, 1, 1, 1, 995], 'skipping': [5, 495, 500], 'thirds': [333, 333, 334],
    'under a percent each': Array(100).fill(9).concat([100]), 'uneven': [7, 13, 400, 3, 577],
  };
  for (const [name, chunks] of Object.entries(chunkSets)) U.push(S(`progress ${name}`, { routes: progressRoutes(chunks, undefined) }));
  for (const cl of [null, '', '0', ' 0 ', 'abc', '1e3', ' 1000 ', '1000.5', '999', '500', '2000', '10', '1', '-1000', '-0', 'Infinity', '-Infinity', '0x3e8', '0b1', '1_000', '\u00A01000\u00A0', '1000abc', '+1000', '.5e3', 'NaN', '1e400', '5e-324']) {
    U.push(S(`progress with content-length ${JSON.stringify(cl)}`, { routes: progressRoutes([300, 300, 400], cl) }));
  }
  U.push(S('progress: body larger than announced', { routes: progressRoutes([600, 400], '500') }));
  U.push(S('progress: no body at all', { routes: progressRoutes([1000], undefined, { noBody: true }) }));
  U.push(S('progress: no body, content-length lies', { routes: progressRoutes([1000], '5', { noBody: true }) }));
  U.push(S('progress: progress after a window-less download', { routes: progressRoutes([500, 500], undefined), noWindow: true }));

  // the apply script with awkward paths
  const dirs = ['upd', 'up date & 100% \u00E9t\u00E9', 'a\\b\\c', 'dots.and spaces', '\u65E5\u672C\u8A9E', '\u{1F600}', 'caret^bang!', 'pa(ren)', 'trailing\\', 'x/y', 'with%PATH%'];
  for (const dir of dirs) {
    for (const target of ['D:\\Apps\\AHG-Converter.exe', 'D:\\My Apps\\AHG & Co\\100%.exe', 'C:\\\u00E9t\u00E9\\\u65E5\u672C.exe', 'C:\\say "hi".exe']) {
      U.push(S(`script for dir ${JSON.stringify(dir)} target ${JSON.stringify(target)}`, { dir, target, steps: ['check', 'download', 'install', 'cleanup'] }));
    }
  }
  for (const name of ['a b.exe', 'a&b.exe', '100%.exe', '\u00E9\u65E5.exe', '..\\evil.exe', '..\\..\\x.exe', '.\\same.exe', 'C:\\abs\\path.exe', 'C:rel.exe', '...\\x.exe', 'x.exe\\.exe', '\\root.exe', '/root.exe', 'AHG.exe.exe', '.exe', ' .exe', 'a\\..\\b.exe']) {
    const f = feed({ assets: [asset(name, ASSET_URL), asset('SHA256SUMS.txt', SUMS_URL)] });
    U.push(S(`asset named ${JSON.stringify(name)}`, { routes: std({ [API]: { json: f }, [SUMS_URL]: { text: `${sha(BYTES)}  ${name}\n` } }), target: 'D:\\Apps\\A.exe', steps: ['check', 'download', 'install', 'cleanup'] }));
  }

  const updaterOut = [];
  for (const sc of U) updaterOut.push(await runUpdater(sc));

  // ── yt-dlp updater ─────────────────────────────────────────────────────────
  async function runYtdlp(sc) {
    const sandbox = path.join(TMP, `s${n++}`);
    const binDir = path.join(sandbox, 'bin');
    fs.mkdirSync(sandbox);
    const sent = [];
    const requests = [];
    const gate = { hold: null };
    let current = sc.current;
    let clock = 0;
    let resets = 0;
    let auto = sc.auto ?? true;
    const routes = structuredClone(sc.routes);
    globalThis.fetch = makeFetch(routes, requests, gate);
    const win = sc.noWindow ? null : { webContents: { send: (channel, payload) => sent.push({ channel, payload: plain(payload) }) } };
    const u = new yd.YtdlpUpdater({
      downloader: { ytDlpVersion: () => current, resetBinaryCache: () => { resets++; } },
      getWindow: () => win, isAutoUpdateEnabled: () => auto, binDir, now: () => clock,
    });
    let fired = false;
    const original = u.update.bind(u);
    u.update = (...a) => { fired = true; return original(...a); };
    const out = [];
    const run = async (step) => {
      if (step.op === 'check') return u.check(step.force);
      if (step.op === 'update') return u.update();
      if (step.op === 'failure') { u.handleDownloadFailure(step.error); await tick(); return null; }
      return u.getStatus();
    };
    try {
      for (const raw of sc.steps) {
        const step = norm(raw);
        if (step.current !== undefined) current = step.current;
        if (step.routes) Object.assign(routes, step.routes);
        if (step.now !== undefined) clock = step.now;
        if (step.auto !== undefined) auto = step.auto;
        sent.length = 0; requests.length = 0; resets = 0; fired = false;
        let result = null;
        const during = [];
        if (step.hold) {
          let release;
          gate.hold = new Promise((r) => { release = r; });
          const p = run(step);
          await tick();
          for (const inner of step.during ?? []) {
            const s = norm(inner);
            during.push({ step: s, result: plain(await run(s)), fired });
          }
          release();
          result = plain(await p);
        } else result = plain(await run(step));
        out.push(tmpOf({ step, result, during, requests: [...requests], events: [...sent], status: plain(u.getStatus()), files: snapshot(sandbox), resets, fired }));
      }
    } finally { globalThis.fetch = real.fetch; }
    return { ...sc, sandbox: path.basename(sandbox), out };
  }
  const Y = (name, o = {}) => ({ name, current: '2026.01.01', routes: ystd(), steps: ['check', 'update'], ...o });
  const YS = [];
  YS.push(Y('check then update', { steps: ['status', 'check', 'status', 'update', 'status'] }));
  YS.push(Y('check then update, no window', { noWindow: true }));
  YS.push(Y('up to date', { current: '2026.07.01', steps: ['check', 'update'] }));
  for (const current of ['', ' 2026.07.01', '2026.07.01 ', '2026.7.1', 'v2026.07.01', '2027.01.01']) YS.push(Y(`current ${JSON.stringify(current)}`, { current, steps: ['check', 'update', 'check'] }));
  const TTL = 24 * 60 * 60 * 1000;
  YS.push(Y('cache window', { steps: [{ op: 'check', now: 1000 }, { op: 'check', now: 1001 }, { op: 'check', now: 1000 + TTL - 1 }, { op: 'check', now: 1000 + TTL }, { op: 'check', now: 1000 + TTL + 1 },
    { op: 'check', now: 1000 + TTL + 2, force: true }, { op: 'check', now: 1000 + 2 * TTL + 1 }, { op: 'check', now: 1000 + 2 * TTL + 2 }] }));
  YS.push(Y('a zero timestamp is no timestamp', { steps: [{ op: 'check', now: 0 }, { op: 'check', now: 1 }, { op: 'check', now: 2 }, { op: 'check', now: 1, force: true }] }));
  YS.push(Y('a clock that goes backwards', { steps: [{ op: 'check', now: 5000 }, { op: 'check', now: 100 }, { op: 'check', now: -5 }, { op: 'check', now: 5000 + TTL }] }));
  YS.push(Y('negative start', { steps: [{ op: 'check', now: -1000 }, { op: 'check', now: -999 }, { op: 'check', now: -1000 + TTL }] }));
  YS.push(Y('a failed check is not cached, a successful update is', {
    routes: ystd({ [YAPI]: { throws: 'offline' } }), steps: [{ op: 'check', now: 10 }, { op: 'check', now: 11 }, { op: 'update', now: 12 }, { op: 'check', now: 13 }],
  }));
  YS.push(Y('an update resets the cache clock', { steps: [{ op: 'check', now: 100 }, { op: 'update', now: 100 + TTL - 5 }, { op: 'check', now: 100 + TTL + 10 }, { op: 'check', now: 100 + 2 * TTL - 6 }, { op: 'check', now: 100 + 2 * TTL - 4 }] }));
  YS.push(Y('error keeps the previous status fields', { routes: ystd({ [YAPI]: { ok: false, status: 500 } }), steps: ['check', 'update', 'status'] }));
  YS.push(Y('error after a good check keeps latest', { steps: ['check', { op: 'update', routes: { [YAPI]: { throws: 'gone' } } }] }));

  for (const [tag, cur] of [['2026.07.01', '2026.07.01'], [' 2026.07.01 ', '2026.07.01'], ['v2026.07.01', '2026.07.01'], ['', '2026.07.01'], ['  ', ''], [null, ''], [undefined, ''], [123, '123'], [1.5, '1.5'], [true, 'true'], [{}, ''], [[], ''],
    [['a', 'b'], 'a,b'], ['\u00A02026\u00A0', '2026'], ['\uFEFF2026', '2026'], [1e21, '1e+21'], ['x', 'x'], ['X', 'x']]) {
    const f = yfeed();
    if (tag === undefined) delete f.tag_name; else f.tag_name = tag;
    YS.push(Y(`tag ${JSON.stringify(tag)} vs ${JSON.stringify(cur)}`, { current: cur, routes: ystd({ [YAPI]: { json: f } }), steps: ['check', 'update'] }));
  }
  const ya = (over) => yfeed({ assets: over });
  const yjson = {
    'no assets': { tag_name: '2026.07.01' }, 'assets null': yfeed({ assets: null }), 'assets empty': ya([]), 'assets object': yfeed({ assets: {} }), 'assets string': yfeed({ assets: 'x' }), 'assets number': yfeed({ assets: 3 }),
    'assets true': yfeed({ assets: true }), 'assets zero': yfeed({ assets: 0 }), 'no binary': ya([asset('SHA2-256SUMS', YSUMS_URL)]), 'no sums': ya([asset(YASSET, YASSET_URL)]),
    'binary renamed': ya([asset('yt-dlp_x86.exe', YASSET_URL), asset('SHA2-256SUMS', YSUMS_URL)]), 'binary uppercase': ya([asset(YASSET.toUpperCase(), YASSET_URL), asset('SHA2-256SUMS', YSUMS_URL)]),
    'sums renamed': ya([asset(YASSET, YASSET_URL), asset('SHA256SUMS.txt', YSUMS_URL)]), 'sums lowercase': ya([asset(YASSET, YASSET_URL), asset('sha2-256sums', YSUMS_URL)]),
    'the app sums name': ya([asset(YASSET, YASSET_URL), asset('SHA256SUMS.txt', YSUMS_URL)]),
    'duplicated binary': ya([asset(YASSET, YASSET_URL), asset(YASSET, 'https://github.com/o/other')]), 'duplicated sums': ya([asset(YASSET, YASSET_URL), asset('SHA2-256SUMS', YSUMS_URL), asset('SHA2-256SUMS', 'https://evil.example/s')]),
    'binary url missing': ya([{ name: YASSET }, asset('SHA2-256SUMS', YSUMS_URL)]), 'binary url empty': ya([asset(YASSET, ''), asset('SHA2-256SUMS', YSUMS_URL)]), 'binary url null': ya([asset(YASSET, null), asset('SHA2-256SUMS', YSUMS_URL)]),
    'binary url zero': ya([asset(YASSET, 0), asset('SHA2-256SUMS', YSUMS_URL)]), 'binary url number': ya([asset(YASSET, 7), asset('SHA2-256SUMS', YSUMS_URL)]), 'sums url empty': ya([asset(YASSET, YASSET_URL), asset('SHA2-256SUMS', '')]),
    'first binary has no url, second does': ya([{ name: YASSET }, asset(YASSET, YASSET_URL), asset('SHA2-256SUMS', YSUMS_URL)]),
    'null element first': ya([null, asset(YASSET, YASSET_URL), asset('SHA2-256SUMS', YSUMS_URL)]), 'null element between': ya([asset(YASSET, YASSET_URL), null, asset('SHA2-256SUMS', YSUMS_URL)]),
    'null element last': ya([asset(YASSET, YASSET_URL), asset('SHA2-256SUMS', YSUMS_URL), null]), 'null element after the sums': ya([asset('SHA2-256SUMS', YSUMS_URL), asset(YASSET, YASSET_URL), null]),
    'number element': ya([4, asset(YASSET, YASSET_URL), asset('SHA2-256SUMS', YSUMS_URL)]), 'string element': ya(['x', asset(YASSET, YASSET_URL), asset('SHA2-256SUMS', YSUMS_URL)]),
    'numeric names': ya([{ name: 5 }, asset(YASSET, YASSET_URL), asset('SHA2-256SUMS', YSUMS_URL)]),
    'json null': null, 'json number': 3, 'json string': 'abc', 'json array': [], 'json empty': {},
  };
  for (const [name, json] of Object.entries(yjson)) YS.push(Y(`feed: ${name}`, { routes: ystd({ [YAPI]: { json } }), steps: ['check', 'update', 'status'] }));

  const yurls = hostUrls.filter((_, i) => i % 2 === 1 || i < 40);
  for (const url of yurls) {
    YS.push(Y(`binary on ${JSON.stringify(url).slice(0, 70)}`, { routes: ystd({ [YAPI]: { json: ya([asset(YASSET, url), asset('SHA2-256SUMS', YSUMS_URL)]) }, [url]: payload(YBYTES) }), steps: ['update'] }));
  }
  for (const url of hostUrls.slice(0, 40)) {
    YS.push(Y(`sums on ${JSON.stringify(url).slice(0, 70)}`, { routes: ystd({ [YAPI]: { json: ya([asset(YASSET, YASSET_URL), asset('SHA2-256SUMS', url)]) }, [url]: { text: `${sha(YBYTES)}  ${YASSET}\n` } }), steps: ['update'] }));
  }
  for (const final of finals) {
    YS.push(Y(`redirect of the feed to ${JSON.stringify(final)}`, { routes: ystd({ [YAPI]: { json: yfeed(), url: final } }), steps: ['check', 'update'] }));
    YS.push(Y(`redirect of the binary to ${JSON.stringify(final)}`, { routes: ystd({ [YASSET_URL]: payload(YBYTES, { url: final }) }), steps: ['update'] }));
    YS.push(Y(`redirect of the sums to ${JSON.stringify(final)}`, { routes: ystd({ [YSUMS_URL]: { text: `${sha(YBYTES)}  ${YASSET}\n`, url: final } }), steps: ['update'] }));
  }
  for (const status of [301, 404, 429, 500, 0, 600]) {
    YS.push(Y(`feed answers HTTP ${status}`, { routes: ystd({ [YAPI]: { ok: false, status, json: yfeed() } }), steps: ['check', 'update'] }));
    YS.push(Y(`binary answers HTTP ${status}`, { routes: ystd({ [YASSET_URL]: payload(YBYTES, { ok: false, status }) }), steps: ['update'] }));
    YS.push(Y(`sums answer HTTP ${status}`, { routes: ystd({ [YSUMS_URL]: { ok: false, status, text: '' } }), steps: ['update'] }));
  }
  YS.push(Y('network failure on the binary', { routes: ystd({ [YASSET_URL]: { throws: 'bin offline' } }), steps: ['update'] }));
  YS.push(Y('network failure on the sums, after the binary', { routes: ystd({ [YSUMS_URL]: { throws: 'sums offline' } }), steps: ['update'] }));
  YS.push(Y('empty-message failure', { routes: ystd({ [YAPI]: { throws: '' } }), steps: ['check', 'update'] }));
  YS.push(Y('no routes', { routes: {}, steps: ['check', 'update'] }));
  const yh = sha(YBYTES);
  const ysums = [`${yh}  ${YASSET}\n`, `${yh} *${YASSET}`, `${yh.toUpperCase()}  ${YASSET}\n`, `${yh}\t${YASSET}`, `${yh}  ${YASSET}\r\n`, `${'a'.repeat(64)}  yt-dlp\n${yh}  ${YASSET}\n`, `${yh}  yt-dlp_x86.exe\n`, `${yh}  ${YASSET}x`, '', 'garbage line',
    `${'b'.repeat(64)}  ${YASSET}\n`, `${yh}  ./${YASSET}`, `${yh}  ${YASSET}\n${'b'.repeat(64)}  ${YASSET}`, `${'b'.repeat(64)}  ${YASSET}\n${yh}  ${YASSET}`, `${yh}  ${YASSET.toUpperCase()}`, `${yh.slice(0, 63)}  ${YASSET}`,
    `${yh} ${YASSET} `, `${yh}  ${YASSET}\u00A0\n`, `${yh}  yt-dlp.exe\n${yh}  yt-dlp\n`];
  for (const [i, text] of ysums.entries()) YS.push(Y(`sums file ${i}`, { routes: ystd({ [YSUMS_URL]: { text } }), steps: ['update', 'status'] }));
  YS.push(Y('mismatching payload', { routes: ystd({ [YASSET_URL]: payload('other bytes') }), steps: ['update', 'update'] }));
  YS.push(Y('empty payload', { routes: ystd({ [YASSET_URL]: payload(''), [YSUMS_URL]: { text: `${sha('')}  ${YASSET}\n` } }), steps: ['update'] }));
  YS.push(Y('non-ASCII payload', { routes: ystd({ [YASSET_URL]: payload('\u00E9\u65E5'), [YSUMS_URL]: { text: `${sha('\u00E9\u65E5')}  ${YASSET}\n` } }), steps: ['update'] }));
  YS.push(Y('update twice, replacing the binary', { steps: ['update', { op: 'update', routes: { [YASSET_URL]: payload('second binary'), [YSUMS_URL]: { text: `${sha('second binary')}  ${YASSET}
` } } }] }));
  YS.push(Y('an update after a failed one', { routes: ystd({ [YASSET_URL]: payload('other') }), steps: ['update', 'status'] }));

  // reactive trigger
  const staleErr = 'ERROR: Unable to extract player response';
  YS.push(Y('reactive: fires once', { steps: [{ op: 'failure', error: staleErr }, { op: 'failure', error: staleErr }, 'status', 'update'] }));
  YS.push(Y('reactive: auto off', { auto: false, steps: [{ op: 'failure', error: staleErr }, 'status', { op: 'failure', error: staleErr, auto: true }, 'status'] }));
  YS.push(Y('reactive: auto off then on does not burn the one shot', { auto: false, steps: [{ op: 'failure', error: staleErr }, { op: 'failure', error: staleErr, auto: true }, { op: 'failure', error: staleErr }] }));
  YS.push(Y('reactive: ordinary failures never burn the one shot', { steps: [{ op: 'failure', error: 'ERROR: Video unavailable' }, { op: 'failure', error: '' }, { op: 'failure', error: staleErr }, { op: 'failure', error: 'HTTP Error 429' }] }));
  for (const text of ['Unable to extract', 'nsig extraction failed', 'Please update to the latest version', 'yt-dlp is out of date', 'please report this issue', 'FAILED TO EXTRACT', 'x', '']) {
    YS.push(Y(`reactive: ${JSON.stringify(text)}`, { steps: [{ op: 'failure', error: text }, 'status'] }));
  }
  YS.push(Y('reactive: the update itself fails, the shot is spent', { routes: ystd({ [YAPI]: { throws: 'offline' } }), steps: [{ op: 'failure', error: staleErr }, { op: 'failure', error: staleErr }, 'update'] }));
  YS.push(Y('reactive: no window', { noWindow: true, steps: [{ op: 'failure', error: staleErr }] }));
  // busy: a second call while one is in flight returns the status as it is
  YS.push(Y('busy during a check', { steps: [{ op: 'check', force: true, hold: true, during: ['check', { op: 'check', force: true }, 'update', 'status', { op: 'failure', error: staleErr }, { op: 'failure', error: staleErr }] }, 'check', 'update'] }));
  YS.push(Y('busy during an update', { steps: [{ op: 'update', hold: true, during: ['check', { op: 'check', force: true }, 'update', { op: 'failure', error: staleErr }] }, 'status', { op: 'failure', error: staleErr }] }));
  YS.push(Y('busy: the held request fails', { routes: ystd({ [YAPI]: { throws: 'late failure' } }), steps: [{ op: 'check', hold: true, during: ['update', 'check'] }, 'check'] }));
  YS.push(Y('a cached answer is not blocked by busy bookkeeping', { steps: [{ op: 'check', now: 50 }, { op: 'check', now: 51, force: true, hold: true, during: [{ op: 'check', now: 52 }] }] }));

  const ytdlpOut = [];
  for (const sc of YS) ytdlpOut.push(await runYtdlp(sc));

  const golden = {
    constants: { api: API, sumsAsset: 'SHA256SUMS.txt', ytdlpApi: YAPI, ytdlpSumsAsset: 'SHA2-256SUMS', ytdlpAsset: YASSET, userAgent: UA },
    trusted, newer, checksum, swap, stale, joins: joinPairs,
    updater: updaterOut, ytdlp: ytdlpOut,
  };
  fs.writeFileSync(`${OUT}/updaters.json`, JSON.stringify(golden) + '\n');
  console.log(`updaters.json: ${trusted.length} hosts, ${newer.length} versions, ${checksum.length} checksums, ${swap.length} scripts, ${stale.length} stale texts, ${joinPairs.length} joins, `
    + `${updaterOut.length} updater scenarios (${updaterOut.reduce((s, x) => s + x.out.length, 0)} steps), ${ytdlpOut.length} yt-dlp scenarios (${ytdlpOut.reduce((s, x) => s + x.out.length, 0)} steps)`);
  fs.rmSync(TMP, { recursive: true, force: true });
}
