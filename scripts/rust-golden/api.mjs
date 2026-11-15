// Golden file for how a download request becomes a task: the `download:start`
// handler in electron/ipc.ts, called through the real registration with a real
// settings store, a scripted plan and no downloader (so the task stays queued).
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export default async function apiSection({ OUT }) {
  const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'ahg-golden-api-'));
  fs.mkdirSync(path.join(TMP, 'Downloads'));
  fs.mkdirSync(path.join(TMP, 'Music'));
  process.env.AHG_GOLDEN_DOWNLOADS = path.join(TMP, 'Downloads');
  process.env.AHG_GOLDEN_PACKAGED = '0'; // a developer build: the handler accepts the local dev page as sender
  const tok = (v) => JSON.parse(JSON.stringify(v ?? null).replaceAll(TMP.replaceAll('\\', '\\\\'), '{TMP}'));

  const realNow = Date.now;
  const realRandom = Math.random;
  let now = 1_763_200_000_000;
  let seed = 7;
  Date.now = () => now;
  Math.random = () => { seed = (seed * 48271) % 2147483647; return seed / 2147483647; };

  const { ConfigManager } = await import('../../dist-electron/electron/config.js');
  const { registerIpcHandlers } = await import('../../dist-electron/electron/ipc.js');
  const frame = { url: 'http://localhost:5173/' };
  frame.top = frame;
  const event = { senderFrame: frame };

  const track = { id: 't1', name: 'Song', artist: 'Artist', artists: ['Artist', 'Guest'], album: 'Album', albumYear: '2019', trackNumber: 7, durationMs: 213000, thumbnailUrl: 'https://i.scdn.co/x', spotifyUrl: 'https://open.spotify.com/track/t1' };
  const base = { url: 'https://youtu.be/dQw4w9WgXcQ', source: 'youtube' };
  const long = (n) => 'x'.repeat(n);
  // Each entry is merged over `base`; `undefined` removes the key.
  const variations = [
    {}, { url: undefined }, { url: '' }, { url: 5 }, { url: null }, { url: 'ytsearch:Artist - Song' }, { url: 'YTSEARCH:x' }, { url: 'ftp://x.y/z' },
    { url: '-rf' }, { url: 'https://a.b/\nc' }, { url: 'https://' + long(2040) }, { url: 'https://' + long(2041) }, { url: 'spotify:track:abc' },
    { source: undefined }, { source: 'spotify' }, { source: 'generic' }, { source: 'unknown' }, { source: 'YouTube' }, { source: 5 }, { source: null }, { source: 'napster' },
    { format: 'flac' }, { format: 'wav' }, { format: 'alac' }, { format: 'opus' }, { format: 'mp4' }, { format: '' }, { format: long(16) }, { format: long(17) }, { format: 5 }, { format: null },
    { quality: 'lossless' }, { quality: '128' }, { quality: '' }, { quality: long(17) }, { quality: 320 },
    { videoQuality: '360p' }, { videoQuality: '1080p' }, { videoQuality: '2160p' }, { videoQuality: 'best' }, { videoQuality: '8k' }, { videoQuality: 1080 }, { videoQuality: null },
    { isAudioOnly: false }, { isAudioOnly: true }, { isAudioOnly: 0 }, { isAudioOnly: 'false' }, { isAudioOnly: null }, { isAudioOnly: '' },
    { isPlaylist: true }, { isPlaylist: false }, { isPlaylist: 'yes' }, { isPlaylist: 0 }, { isPlaylist: true, playlistName: 'Mix' },
    { playlistItems: '1,3-5' }, { playlistItems: '1,3-5', isPlaylist: true }, { playlistItems: 'x' }, { playlistItems: '' }, { playlistItems: 5 }, { playlistItems: null },
    { playlistLimit: 3 }, { playlistName: long(512) }, { playlistName: long(513) }, { playlistName: 5 },
    { title: 'A title', uploader: 'Someone', thumbnailUrl: 'https://i.ytimg.com/x.jpg', duration: 212.5 }, { title: long(513) }, { uploader: null },
    { thumbnailUrl: 'https://' + long(2040) }, { thumbnailUrl: 'https://' + long(2041) }, { duration: '212' }, { duration: null }, { duration: -1 }, { duration: 1e21 },
    { embedThumbnail: false }, { embedMetadata: false }, { skipExisting: false }, { embedThumbnail: 0, embedMetadata: '', skipExisting: null }, { embedThumbnail: 'no' },
    { outputDir: 'C:\\Windows', extras: { proxy: 'http://evil', cookiesFile: 'C:\\x' }, taskId: 'mine', progress: { status: 'done' }, retryCount: 9, createdAt: 1, fallbackProfile: true, errorCode: 'drm' },
    { track }, { source: 'spotify', url: 'ytsearch:Artist - Song', track }, { track: null }, { track: 5 }, { track: [] }, { track: {} }, { track: { name: 'N' } }, { track: { name: 'N', artist: 'A' } },
    { track: { ...track, id: null } }, { track: { ...track, id: 5 } }, { track: { ...track, id: long(257) } }, { track: { ...track, name: long(513) } }, { track: { ...track, artist: 5 } },
    { track: { ...track, artists: 'Artist' } }, { track: { ...track, artists: [5] } }, { track: { ...track, artists: Array.from({ length: 70 }, (_, i) => `A${i}`) } },
    { track: { ...track, artists: [long(513)] } }, { track: { ...track, album: undefined, albumYear: undefined, thumbnailUrl: undefined, spotifyUrl: undefined } },
    { track: { ...track, albumYear: long(17) } }, { track: { ...track, trackNumber: '7', durationMs: null } }, { track: { ...track, trackNumber: 1.5, durationMs: -3 } },
    { track: { ...track, extra: 'dropped', __proto__: { polluted: true } } }, { track: { ...track, thumbnailUrl: 5 } }, { track: { ...track, spotifyUrl: 'x'.repeat(2049) } },
  ];
  const requests = [null, 5, 'x', [], undefined].map((r) => ({ raw: r === undefined ? '__undefined__' : r }))
    .concat(variations.map((v) => { const r = { ...base, ...v }; for (const k of Object.keys(r)) if (r[k] === undefined) delete r[k]; return { raw: r }; }));

  const settings = [
    { name: 'defaults', update: {} },
    { name: 'customised', update: {
      outputDir: '{TMP}\\Music', defaultFormat: 'flac', defaultQuality: 'lossless', defaultVideoQuality: '2160p', embedThumbnail: false, embedMetadata: false, skipExisting: false,
      subtitleMode: 'both', subtitleLangs: 'en, fr ,de', subtitleAuto: true, embedChapters: false, splitChapters: true, sponsorBlock: 'remove',
      sponsorBlockCategories: ['sponsor', 'intro'], writeThumbnail: true, writeInfoJson: true, writeDescription: true, videoContainer: 'mkv', videoCodec: 'av1',
      cookieBrowser: 'firefox', outputTemplate: '%(uploader)s/%(title)s', rateLimit: '2M', proxy: 'socks5://127.0.0.1:9050',
    }, cookies: true },
    { name: 'empty languages', update: { subtitleLangs: ' , ', subtitleMode: 'embed' } },
  ];

  const cases = [];
  let n = 0;
  try {
    for (const setting of settings) {
      for (const plan of ['basic', 'premium']) {
        const dir = path.join(TMP, String(n++));
        fs.mkdirSync(dir);
        process.env.AHG_GOLDEN_USERDATA = dir;
        if (setting.cookies) fs.writeFileSync(path.join(dir, 'cookies.txt'), '# Netscape HTTP Cookie File\n');
        const config = new ConfigManager();
        config.update(JSON.parse(JSON.stringify(setting.update).replaceAll('{TMP}', TMP.replaceAll('\\', '\\\\'))));
        globalThis.__ahgIpc = new Map();
        registerIpcHandlers(config, {}, {}, { getPlan: () => plan }, {}, {}, {});
        const start = globalThis.__ahgIpc.get('download:start');
        const out = requests.map(({ raw }) => {
          now += 61_000; // each call lands in a fresh minute, so the handler's rate limit never trips
          let result;
          try { result = { task: start(event, raw === '__undefined__' ? undefined : raw) }; } catch (e) { result = { error: e.message }; }
          return { request: raw, now, ...tok(result) };
        });
        cases.push({ settings: setting.name, plan, config: tok(config.getAll()), cookiesFile: setting.cookies ? '{TMP}\\' + path.basename(dir) + '\\cookies.txt' : null, out });
      }
    }
  } finally {
    Date.now = realNow;
    Math.random = realRandom;
    delete globalThis.__ahgIpc;
  }
  fs.writeFileSync(`${OUT}/api.json`, JSON.stringify(cases) + '\n');
  console.log(`api.json: ${cases.reduce((s, c) => s + c.out.length, 0)} requests, ${cases.reduce((s, c) => s + c.out.filter((o) => o.error).length, 0)} refused`);
  // The settings stores still have debounced saves pending; let them finish first.
  setTimeout(() => fs.rmSync(TMP, { recursive: true, force: true }), 800);
}
