// Writes golden files for the Rust port: the real TypeScript functions are run
// over a corpus and their outputs saved, so the Rust tests compare against what
// the Electron app actually does. Run `npm run build:electron` first, then
// `node scripts/rust-golden.mjs`.
import { readFileSync, writeFileSync } from 'node:fs';

import { register } from 'node:module';

const OUT = 'crates/ahg-engine/tests/golden';

// Some modules import 'electron' for parts the Rust engine does not port. Plain
// Node cannot load it, so it resolves to an empty stand-in; only pure functions
// are called here, and none of them touches it.
register(`data:text/javascript,${encodeURIComponent(`
  const STUB = 'data:text/javascript,' + encodeURIComponent(
    'export const app = {}; export const session = {}; export default {};');
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
