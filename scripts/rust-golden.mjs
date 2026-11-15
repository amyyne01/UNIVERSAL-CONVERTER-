// Writes golden files for the Rust port: the real TypeScript functions are run
// over a corpus and their outputs saved, so the Rust tests compare against what
// the Electron app actually does. Run `npm run build:electron` first, then
// `node scripts/rust-golden.mjs`.
import { readFileSync, writeFileSync } from 'node:fs';

const OUT = 'crates/ahg-engine/tests/golden';

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

const inputs = [...new Set([...literals('electron/url-detector.test.ts'), ...extra])];
const cases = inputs.map((input) => ({ input, detect: detectUrl(input), spotify: parseSpotifyUrl(input) }));
writeFileSync(`${OUT}/url_detector.json`, JSON.stringify(cases, null, 1) + '\n');
console.log(`url_detector.json: ${cases.length} cases`);
