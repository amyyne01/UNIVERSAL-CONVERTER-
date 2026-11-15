// Writes golden files for the Rust port: the real TypeScript functions are run
// over a corpus and their outputs saved, so the Rust tests compare against what
// the Electron app actually does. Run `npm run build:electron` first, then run
// this under ELECTRON'S OWN Node, because that is the runtime the app ships on
// and Node's behaviour differs between versions (path.win32 does, for one):
//
//   ELECTRON_RUN_AS_NODE=1 node_modules/electron/dist/electron.exe scripts/rust-golden.mjs
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';

import { register } from 'node:module';

if (!process.versions.electron) {
  console.error('Run this with Electron\'s Node, not system Node (see the comment at the top of this file).');
  process.exit(1);
}

const OUT = 'crates/ahg-engine/tests/golden';

// Some modules import 'electron'. Plain Node cannot load it, so it resolves to
// a stand-in whose only working part is app.getPath(), answered from
// AHG_GOLDEN_<NAME> environment variables set by the sections below.
register(`data:text/javascript,${encodeURIComponent(`
  const STUB = 'data:text/javascript,' + encodeURIComponent(
    'export const app = { getPath: (name) => process.env["AHG_GOLDEN_" + name.toUpperCase()] };' +
    'export const session = {}; export default {};');
  export async function resolve(specifier, context, next) {
    return specifier === 'electron' ? { url: STUB, shortCircuit: true } : next(specifier, context);
  }
`)}`);

/** Every plain string literal in a test file: the inputs the suite already cares about. */
function literals(file) {
  const src = readFileSync(file, 'utf8');
  const found = [];
  // Anchored on what surrounds a literal in code, so an apostrophe inside a
  // test name or comment cannot shift the pairing.
  for (const m of src.matchAll(/(?<=[[(,:=\s])(?:'(?:[^'\\\n]|\\.)*'|"(?:[^"\\\n]|\\.)*")(?=[\]),;\s.])/g)) {
    try { found.push(Function(`return ${m[0]}`)()); } catch { /* not a standalone literal */ }
  }
  return found;
}

const { detectUrl, parseSpotifyUrl } = await import('../dist-electron/electron/url-detector.js');

const extra = [
  '', '   ', 'buy me a coffee', 'localhost', 'http://localhost/x.mp4', 'ftp://example.com/a.mp4',
  'example.com', 'EXAMPLE.COM/Path', 'https://example.com:8080/a?b#c', 'https://1.2.3.4/v', 'https://[::1]/v',
  'https://user:pw@vimeo.com/123456', 'https://VIMEO.com/123456/abcdef0/', 'https://vimeo.com/123456/zzz',
  '\uFEFF https://youtu.be/dQw4w9WgXcQ \u2028', '\u0085https://youtu.be/dQw4w9WgXcQ',
  'https://ti\u212Atok.com/@a/video/123', 'https://www.youtube.com/watch?v=dQw4w9WgXc\u0660',
  'https://youtube.com/playlist?x=1\nlist=PL123', 'https://youtube.com/playlist?x=1\u2028list=PL123',
  'https://youtube.com/playlist?mylist=PL123', 'https://youtube.com/playlist?a=1&list=PL-_9',
  'https://www.youtube.com/watch?feature=x&v=dQw4w9WgXcQ#t=1', 'https://www.youtube.com/watch?nov=dQw4w9WgXcQ',
  'https://www.youtube.com/live/dQw4w9WgXcQ', 'https://www.youtube.com/embed/short',
  'spotify:track:4cOdK2wGLETKBW3PvgPWqT', 'SPOTIFY:ALBUM:abc', 'https://open.spotify.com/intl-fr/artist/xyz?si=1',
  'https://open.spotify.com/episode/abc', 'ytsearch:artist - title', 'ytsearch:watch youtube.com/watch?v=dQw4w9WgXcQ',
  'ytsearch:some file.mp4', 'https://soundcloud.com/a/likes', 'https://soundcloud.com/a/likes2', 'https://soundcloud.com/a/sets',
  'https://soundcloud.com/a/sets/b?x', 'https://soundcloud.com/a/TRACKS/', 'https://soundcloud.com/a/b/c',
  'https://instagram.com/reels/Cx1/', 'https://instagram.com/stories/a/1', 'https://www.tiktok.com/@user',
  'https://vt.tiktok.com/ZS123/', 'https://www.tiktok.com/t/ZT9/', 'https://m.facebook.com/watch/?v=123', 'https://fb.watch/abC1/',
  'https://facebook.com/profile', 'https://x.com/i/web/status/99', 'https://mobile.twitter.com/a/status/12/photo/1',
  'https://twitch.tv.evil.com/videos/1', 'https://evil.com/twitch.tv/videos/1', 'https://www.twitch.tv/name', 'https://www.twitch.tv/name/clips/',
  'https://artist.bandcamp.com', 'https://artist.bandcamp.com/music/', 'https://www.bandcamp.com/', 'https://a.bandcamp.com/track/x?y',
  'https://old.reddit.com/r/a/comments/abc/t/', 'https://v.redd.it/xyz', 'https://dai.ly/x8', 'https://www.dailymotion.com/embed/video/x9',
  'https://www.mixcloud.com/dj/set-1/', 'https://www.mixcloud.com/dj/uploads/', 'https://www.mixcloud.com/discover/x/', 'https://www.mixcloud.com/dj',
  'https://www.mixcloud.com/discover', 'https://rumble.com/v1abc-title.html', 'https://rumble.com/videos', 'https://rumble.com/video-x',
  'https://cdn.example.com/a.MP4?x=1', 'https://cdn.example.com/a.mp4x', 'file:///c:/a.mkv', 'javascript:alert(1).mov',
  'https://exa_mple.com/x', 'https://xn--bcher-kva.example/x', 'https://b\u00FCcher.example/x', 'https://example.com./x',
  'a'.repeat(2100) + '.com', 'https://youtu.be/' + 'x'.repeat(2100), '\uD83D\uDE00'.repeat(1030) + 'youtu.be/dQw4w9WgXcQ',
];

// ── Outside-input validators (electron/security.ts) ──────────────────────────
{
  const s = await import('../dist-electron/electron/security.js');
  /** Result of a validator that throws: the value, or the error message. */
  const attempt = (fn) => { try { return { ok: fn() }; } catch (e) { return { err: e.message }; } };
  const urls = [
    '', 'https://youtu.be/x', 'http://a.b/c', 'HTTPS://A.B', 'ytsearch:artist - title', 'YTSEARCH:x', 'ytsearch5:x',
    '-rf', '--exec=calc', ' -x', 'https://a.b/\nc', 'https://a.b/\rc', 'https://a.b/\u2028c', 'ftp://a.b', 'file:///c:/x',
    'spotify:track:abc', 'javascript:alert(1)', 'a'.repeat(2048), 'a'.repeat(2049), 'https://' + 'a'.repeat(2040),
    'https://' + 'a'.repeat(2041), '\uD83D\uDE00'.repeat(1024), '\uD83D\uDE00'.repeat(1025), 'https://x.y/\uD83D\uDE00',
  ];
  const items = [
    '', '1', '1,3-5', '1-2-3', '1,', ',1', '1,,2', ' 1', '1 ', '-1', '1-', 'a', '1,3-5,10-12,99', '\u0661', '1\n',
    '0', '007', '1'.repeat(256), '1'.repeat(257), '1,'.repeat(127) + '1', '1,'.repeat(128) + '1',
  ];
  const row = 'example.com\tTRUE\t/\tFALSE\t0\tname\tvalue';
  const cookies = [
    '', row, row + '\n', row + '\r\n' + row, '# Netscape HTTP Cookie File\n' + row, '# only a comment\n', '\n\n', '   \n' + row,
    'example.com\tTRUE\t/\tFALSE\t0\tname', row + '\textra', row + '\n{"json":true}', row + '\0', '#HttpOnly_' + row,
    ' ' + row, '\t\t\t\t\t\t', 'a\rb', row + '\rnot a row', row + '\n \t \n' + row, '\uFEFF' + row,
    row + '\n' + 'x'.repeat(2 * 1024 * 1024 - row.length - 1), row + '\n' + 'x'.repeat(2 * 1024 * 1024 - row.length),
  ];
  const R = 'C:\\Users\\x\\Music';
  const confine = [
    ['song.mp3', R], ['a\\b\\song.mp3', R], ['a/b/song.mp3', R], ['', R], ['.', R], ['..', R], ['..\\Music\\a.mp3', R],
    ['..\\Music2\\a.mp3', R], ['a\\..\\..\\b', R], [R, R], [R + '\\', R], [R + '\\a.mp3', R], ['c:\\users\\x\\music\\a.mp3', R],
    ['C:\\Users\\x\\Music2\\a.mp3', R], ['C:\\Windows\\System32\\calc.exe', R], ['D:\\a.mp3', R], ['\\\\server\\share\\a.mp3', R],
    ['\\a.mp3', R], ['/a.mp3', R], ['C:a.mp3', R], ['a.mp3', R + '\\'], ['a.mp3', 'C:\\'], ['..', 'C:\\'], ['C:\\', 'C:\\'],
    ['a.mp3.', R], ['a .mp3 ', R], ['sub\\.\\a.mp3', R], ['a.mp3', 'C:/Users/x/Music'], ['a.mp3', 'C:\\Users\\x\\..\\x\\Music'],
    ['\u00E9t\u00E9\\\uD83D\uDE00.mp3', R], ['a.mp3', '\\\\server\\share\\dir'], ['..\\other\\a.mp3', '\\\\server\\share\\dir'],
  ];
  const golden = {
    maxCookiesBytes: s.MAX_COOKIES_BYTES,
    urlArg: urls.map((v) => ({ v, out: attempt(() => s.urlArg(v)) })),
    mediaUrlArg: urls.map((v) => ({ v, out: attempt(() => s.mediaUrlArg(v)) })),
    playlistItemsArg: items.map((v) => ({ v, out: attempt(() => s.playlistItemsArg(v)) })),
    isNetscapeCookies: cookies.map((v) => ({ v, out: s.isNetscapeCookies(v) })),
    confineToRoot: confine.map(([candidate, root]) => ({ candidate, root, out: s.confineToRoot(candidate, root) })),
  };
  writeFileSync(`${OUT}/validate.json`, JSON.stringify(golden) + '\n');
  console.log(`validate.json: ${Object.values(golden).filter(Array.isArray).reduce((n, a) => n + a.length, 0)} cases`);
}

// ── Settings file (electron/config.ts) ───────────────────────────────────────
// Drives the real ConfigManager in a scratch folder. That folder's path is
// written as {TMP}; the Rust test builds the same layout in its own folder.
{
  const fs = await import('node:fs');
  const os = await import('node:os');
  const path = await import('node:path');
  const { ConfigManager, defaultConfig, migrateConfig } = await import('../dist-electron/electron/config.js');

  const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'ahg-golden-'));
  fs.mkdirSync(path.join(TMP, 'Downloads'));
  fs.mkdirSync(path.join(TMP, 'out'));
  fs.writeFileSync(path.join(TMP, 'file.txt'), 'x');
  process.env.AHG_GOLDEN_DOWNLOADS = path.join(TMP, 'Downloads');
  const real = (v) => (typeof v === 'string' ? v.replaceAll('{TMP}', TMP) : v);
  const tok = (v) =>
    typeof v === 'string' ? v.replaceAll(TMP, '{TMP}')
    : Array.isArray(v) ? v.map(tok)
    : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, tok(x)]))
    : v;
  /** The same replacement inside JSON text, where backslashes are doubled. */
  const tokText = (t) => t.replaceAll(TMP.replaceAll('\\', '\\\\'), '{TMP}');
  /** File contents written from a case: {TMP} becomes the real path, JSON-escaped. */
  const realText = (t) => t.replaceAll('{TMP}', TMP.replaceAll('\\', '\\\\'));
  let n = 0;
  /** A manager on a folder that does not exist: nothing to load, and saves fail quietly. */
  const fresh = () => {
    process.env.AHG_GOLDEN_USERDATA = path.join(TMP, 'u', String(n++));
    return new ConfigManager();
  };

  const generic = ['', 'x', 0, 1, 2.5, -1, true, false, null, [], [1], ['a'], [1, 'a'], {}];
  const specific = {
    outputDir: ['{TMP}\\out', '{TMP}/out', '{TMP}\\file.txt', '{TMP}\\missing', '{TMP}\\out\\..\\out', '{TMP}\\out\\', 'relative',
      'out', '\\\\server\\share\\dir', 'C:\\Windows', 'c:\\windows\\Temp', 'C:/Windows', 'C:\\', 'c:', 'C:\\Program Files',
      'C:\\ProgramData\\x', 'C:\\Users'],
    proxy: ['http://a:1', 'https://a', 'socks5://127.0.0.1:9050', 'socks5h://a', 'socks4://a', 'HTTP://a', 'host:8080', ' http://a'],
    rateLimit: ['2M', '500K', '1G', '100', '2 MB', '2m', '2MB', '\u0661', '2M\n', '0', 'K'],
    palette: ['aurora', 'slate', 'moss', 'ember', 'paper', 'lagoon', 'orchid', 'Aurora', 'nope', ' moss'],
    theme: ['dark', 'light', 'system', 'neon'],
    defaultVideoQuality: ['1080p', 'best', '8k'],
    scheduleDays: [[0, 6], [1.5], [7, -1], [[1]], [null], [true]],
    sponsorBlockCategories: [['sponsor', 'intro'], ['made-up'], [''], [1], [null]],
    maxConcurrentDownloads: [5, 20, 0, 1e21, 0.1],
  };
  const keys = [...Object.keys(defaultConfig), 'nope', '__proto__', 'constructor', 'prototype', 'toString', 'hasOwnProperty'];
  const setCases = keys.flatMap((key) => [...generic, ...(Object.hasOwn(specific, key) ? specific[key] : [])].map((value) => {
    const c = fresh();
    const ok = c.set(key, real(value));
    const all = c.getAll();
    // Own keys only: `all['__proto__']` would read the built-in prototype.
    return tok({ key, value, ok, after: Object.hasOwn(all, key) ? all[key] : undefined });
  }));

  const good = (o) => JSON.stringify(o);
  const loads = [
    { file: null }, { file: '' }, { file: '{' }, { file: 'null' }, { file: '5' }, { file: '"str"' }, { file: '[]' },
    { file: '[1,2]' }, { file: '{}' }, { file: good({ theme: 'dark' }) }, { file: '\uFEFF' + good({ theme: 'dark' }) },
    { file: good({ theme: 'dark', nope: 1, toString: 'x', constructor: 2 }) },
    { file: '{"__proto__":{"polluted":true},"theme":"dark","theme":"light"}' },
    { file: good({ theme: 5, embedThumbnail: 'yes', scheduleDays: 'mon', maxConcurrentDownloads: '3', globalHotkey: null }) },
    { file: good({ schemaVersion: 0, theme: 'dark' }) }, { file: good({ schemaVersion: 1, theme: 'dark' }) },
    { file: good({ schemaVersion: 2, theme: 'dark' }) }, { file: good({ schemaVersion: '1', theme: 'dark' }) },
    { file: good({ outputDir: '', palette: '', proxy: '', rateLimit: '' }) },
    { file: good({ outputDir: '{TMP}\\out', palette: 'moss', proxy: 'socks5://a', rateLimit: '2M' }) },
    { file: good({ outputDir: 'C:\\Windows', palette: 'nope', proxy: 'a', rateLimit: 'fast' }) },
    { file: good({ scheduleDays: [1, 2], sponsorBlockCategories: ['intro'], maxConcurrentDownloads: 4, scheduleTime: '25:99' }) },
    { file: good({ scheduleDays: ['1'], sponsorBlockCategories: [1], subtitleMode: 'bogus', videoCodec: 'h265' }) },
    { file: '{', backups: [good({ theme: 'dark' })] },
    { file: null, backups: ['{', good({ theme: 'light' }), good({ theme: 'dark' })] },
    { file: '', backups: ['null', '5', good({ palette: 'ember' })] },
    { file: '{', backups: ['[]'] },
    { file: '{', backups: ['{', '{', '{'] },
    { file: 'null', backups: [good({ theme: 'dark' })] },
    { file: '{', backups: [good({ nope: 1, schemaVersion: 0, outputDir: '' })] },
  ];
  const loadCases = loads.map(({ file, backups = [] }) => {
    const dir = path.join(TMP, 'l', String(n++));
    fs.mkdirSync(dir, { recursive: true });
    if (file !== null) fs.writeFileSync(path.join(dir, 'config.json'), realText(file));
    backups.forEach((b, i) => fs.writeFileSync(path.join(dir, `config.backup.${i + 1}.json`), realText(b)));
    process.env.AHG_GOLDEN_USERDATA = dir;
    const c = new ConfigManager();
    return { file, backups, out: tok(c.getAll()), restored: c.wasRestoredFromBackup() };
  });

  // What ends up on disk, write after write.
  const dir = path.join(TMP, 'w');
  fs.mkdirSync(dir);
  process.env.AHG_GOLDEN_USERDATA = dir;
  const c = new ConfigManager();
  const read = (name) => { try { return tokText(fs.readFileSync(path.join(dir, name), 'utf-8')); } catch { return null; } };
  const snapshot = () => ({
    config: read('config.json'), backup1: read('config.backup.1.json'), backup2: read('config.backup.2.json'),
    backup3: read('config.backup.3.json'), tmp: fs.existsSync(path.join(dir, 'config.json.tmp')),
  });
  const steps = [
    { theme: 'dark', scheduleDays: [1, 3], maxConcurrentDownloads: 2.5, sponsorBlockCategories: ['sponsor', 'intro'], nope: 1 },
    { palette: 'moss', proxy: 'bad', outputDir: '' },
    { globalHotkey: 'Ctrl+"\\\u00E9\u2028\u0001', rateLimit: '2M' },
    { outputTemplate: '%(title)s', scheduleDays: [] },
    { theme: 'light' },
  ];
  const writeCases = steps.map((update) => { c.update(update); c.saveNow(); return { update, after: snapshot() }; });

  const golden = {
    defaults: tok(fresh().getAll()),
    set: setCases,
    load: loadCases,
    write: writeCases,
    migrate: [{}, { schemaVersion: 0 }, { schemaVersion: 1 }, { schemaVersion: 5, a: 1 }, { schemaVersion: '1', a: 1 }, { theme: 'dark' }]
      .map((obj) => ({ obj, out: migrateConfig(obj) })),
  };
  writeFileSync(`${OUT}/config.json`, JSON.stringify(golden) + '\n');
  console.log(`config.json: ${setCases.length + loadCases.length + writeCases.length + golden.migrate.length} cases`);
  // Debounced saves from the set cases are still pending; let them fail against
  // their missing folders before the scratch folder goes.
  setTimeout(() => fs.rmSync(TMP, { recursive: true, force: true }), 600);
}

// ── Tier clamps and extras (shared/types.ts) ─────────────────────────────────
{
  const t = await import('../dist-electron/shared/types.js');
  const plans = ['basic', 'premium'];
  const formats = ['mp3', 'aac', 'm4a', 'ogg', 'opus', 'flac', 'wav', 'alac', 'FLAC', '', 'weird'];
  const qualities = ['128', '192', '320', 'lossless', 'LOSSLESS', ''];
  // Numbers JSON cannot carry are written as strings and parsed back in the Rust test.
  const numbers = [-3, -0.5, 0, 0.5, 1, 1.9, 2, 2.9, 3, 4, 5, 5.5, 6, 20, 1e9, 'NaN', 'Infinity', '-Infinity'];
  const partials = [
    undefined,
    {},
    { subtitleLangs: 'en,fr, de ,,es', subtitleAuto: true, splitChapters: true, cookieBrowser: 'firefox', cookiesFile: 'C:\\x\\cookies.txt' },
    { subtitleLangs: ' , ,', subtitleMode: 'both' },
    { subtitleLangs: '', sponsorBlock: 'remove', sponsorBlockCategories: ['sponsor', 'intro'] },
    { subtitleLangs: 'fr', embedChapters: false, writeThumbnail: true, writeInfoJson: true, writeDescription: true },
    { videoContainer: 'mkv', videoCodec: 'av1', outputTemplate: '%(title)s.%(ext)s', rateLimit: '2M', proxy: 'socks5://127.0.0.1:9050' },
    { subtitleMode: undefined, subtitleLangs: undefined, proxy: undefined },
    { unknownKey: 1, proxy: 'http://p:1' },
  ];
  const golden = {
    defaultExtras: t.defaultExtras,
    retryBackoffMs: t.RETRY_BACKOFF_MS,
    basicLimits: t.BASIC_LIMITS,
    maxConcurrentDownloads: t.MAX_CONCURRENT_DOWNLOADS,
    videoQuality: t.VIDEO_QUALITIES.flatMap((q) => plans.map((plan) => (
      { q, plan, premium: t.isPremiumVideoQuality(q), out: t.clampVideoQuality(q, plan) }))),
    format: formats.flatMap((f) => plans.map((plan) => (
      { f, plan, premium: t.isPremiumFormat(f), out: t.clampFormat(f, plan) }))),
    audioQuality: qualities.flatMap((q) => plans.map((plan) => ({ q, plan, out: t.clampAudioQuality(q, plan) }))),
    concurrency: numbers.flatMap((n) => plans.map((plan) => ({ n: String(n), plan, out: t.clampConcurrency(Number(n), plan) }))),
    // `partial` goes through JSON, so an explicit undefined becomes an absent key:
    // the two are the same thing to mergeExtras.
    extras: partials.flatMap((partial) => [
      { partial: partial ?? null, plan: null, out: t.mergeExtras(partial) },
      ...plans.map((plan) => ({ partial: partial ?? null, plan, out: t.clampExtras(partial ?? {}, plan) })),
    ]),
  };
  writeFileSync(`${OUT}/tier.json`, JSON.stringify(golden, null, 1) + '\n');
  console.log(`tier.json: ${Object.values(golden).filter(Array.isArray).reduce((n, a) => n + a.length, 0)} cases`);
}

const inputs = [...new Set([...literals('electron/url-detector.test.ts'), ...extra])];
const cases = inputs.map((input) => ({ input, detect: detectUrl(input), spotify: parseSpotifyUrl(input) }));
writeFileSync(`${OUT}/url_detector.json`, JSON.stringify(cases, null, 1) + '\n');
console.log(`url_detector.json: ${cases.length} cases`);

// Further sections, one file each under scripts/rust-golden/. A section
// default-exports `async ({ OUT, literals }) => void` and writes its own file.
for (const name of readdirSync(new URL('./rust-golden/', import.meta.url)).filter((n) => n.endsWith('.mjs')).sort()) {
  await (await import(`./rust-golden/${name}`)).default({ OUT, literals });
}
