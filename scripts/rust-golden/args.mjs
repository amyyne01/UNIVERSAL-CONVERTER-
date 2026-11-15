// Golden file for the yt-dlp argument builder (Downloader.buildYtDlpArgs).
// Every case is a task (a shared base plus overrides, to keep the file small),
// an ffmpeg path, and the argument list the real TypeScript builds for them.
import { writeFileSync } from 'node:fs';

export default async ({ OUT, literals }) => {
  const { Downloader } = await import('../../dist-electron/electron/downloader.js');
  const d = new Downloader();

  const base = {
    taskId: 'task_test', url: 'https://youtube.com/watch?v=dQw4w9WgXcQ', outputDir: 'C:\\Downloads', format: 'mp3',
    quality: '320', videoQuality: 'best', isAudioOnly: true, isPlaylist: false, playlistName: '', embedThumbnail: true,
    embedMetadata: true, skipExisting: true, title: 'Test Song', thumbnailUrl: '', duration: 212, uploader: 'Artist',
    source: 'youtube',
    progress: { status: 'queued', percent: 0, speed: 0, eta: 0, downloaded: 0, total: 0, filename: '', error: '', playlistIndex: 0, playlistTotal: 0 },
  };
  const cases = [];
  const add = (over = {}, ffmpeg = null) => {
    const task = { ...base, ...over };
    cases.push({ over, ffmpeg, args: d.buildYtDlpArgs(task, ffmpeg) });
  };

  const qualities = ['360p', '480p', '720p', '1080p', '2160p', 'best'];
  const containers = ['mp4', 'mkv', 'webm'];
  const codecs = ['any', 'h264', 'vp9', 'av1'];
  const audioFormats = ['mp3', 'aac', 'm4a', 'ogg', 'opus', 'flac', 'wav', 'alac'];
  const odd = ['FLAC', 'Mp3', '', 'mp3x', '..\\x', 'a b', 'm4a2', 'mp3\n', '\u00FCn\u00EF', 'wav ', 'mp-3', '0'];
  const bitrates = ['128', '192', '320', 'lossless', '', '0', '5.5', 'LOSSLESS', 'abc', ' 320'];
  const flags = [[true, true, true], [false, false, false], [true, false, true], [false, true, false]];
  const sources = ['youtube', 'spotify', 'soundcloud', 'instagram', 'tiktok', 'facebook', 'direct', 'twitter', 'vimeo', 'twitch',
    'bandcamp', 'reddit', 'dailymotion', 'mixcloud', 'rumble', 'generic', 'unknown'];
  const track = {
    id: '4cOdK2', name: 'Song', artist: 'Artist', artists: ['Artist'], album: 'Album', trackNumber: 1, durationMs: 200000,
    thumbnailUrl: '',
  };

  // Audio: every format and bitrate, with thumbnails on and off.
  for (const format of [...audioFormats, ...odd]) {
    for (const quality of bitrates) {
      for (const embedThumbnail of [true, false]) {
        add({ format, quality, embedThumbnail, embedMetadata: !embedThumbnail, skipExisting: quality.length % 2 === 0 });
      }
    }
  }

  // Video: quality x container x codec x thumbnail.
  for (const videoQuality of qualities) {
    for (const videoContainer of containers) {
      for (const videoCodec of codecs) {
        for (const embedThumbnail of [true, false]) {
          add({ isAudioOnly: false, videoQuality, embedThumbnail, extras: { videoContainer, videoCodec } });
        }
      }
    }
  }
  for (const [embedThumbnail, embedMetadata, skipExisting] of flags) {
    add({ embedThumbnail, embedMetadata, skipExisting, isAudioOnly: false });
    add({ embedThumbnail, embedMetadata, skipExisting });
  }

  // Collections.
  const names = [
    'Playlist', 'My Mix', '', '   ', 'a<b>c:d"e/f\\g|h?i*j', 'CON', 'con', 'Nul', 'COM1', 'LPT9', 'COM0', 'LPT0', 'COM10', 'CON.txt',
    'CON ', ' CON', '..', '...', ' .. ', '. .', '.a', 'a.', '\u0001\u001F x', '\u007F', 'x'.repeat(300), '\uD83D\uDE00 \u00E9',
    '\u00A0x\u00A0', '\uFEFF', '%(title)s', 'a\u2028b', 'CON\n', '\u017F', 'COM\u00B2', '<<>>', '\\..\\..\\x', '../x', 'a/b', 'AUX', 'prn', 'COM9',
  ];
  const limits = [undefined, 0, -1, 1, 5, 5.7, 0.5, 20, 1e9, 1e20, 1e21, 1.5e22];
  const items = [undefined, '', '1', '1,3-5', ' 1', '1-2-3', '--x'];
  for (const playlistLimit of limits) {
    for (const playlistItems of items) {
      add({ isPlaylist: true, playlistName: 'Playlist', playlistLimit, playlistItems });
      add({ isPlaylist: false, playlistName: 'Playlist', playlistLimit, playlistItems });
    }
  }
  for (const playlistName of names) {
    for (const [isAudioOnly, format, videoQuality] of [[true, 'mp3', 'best'], [true, 'flac', 'best'], [false, 'mp3', '720p']]) {
      add({ isPlaylist: true, playlistName, isAudioOnly, format, videoQuality });
    }
    add({ isPlaylist: false, playlistName });
  }

  // Titles.
  for (const title of names) {
    for (const [isAudioOnly, format, videoQuality] of [[true, 'mp3', 'best'], [true, 'ogg', 'best'], [false, 'mp3', '360p'], [false, 'mp3', 'best']]) {
      add({ title, isAudioOnly, format, videoQuality });
    }
  }

  // Output folders and templates, including hostile ones.
  const dirs = [
    'C:\\Downloads', 'C:/Downloads', 'C:\\Downloads\\', 'D:', 'D:\\', 'C:a\\b', '\\\\server\\share\\x', '\\\\server\\share', '//server/share/x',
    '\\\\server\\share\\', '\\\\server', '\\\\.\\C:\\a', '\\\\?\\C:\\a', '\\\\?\\COM1:\\a', '\\\\.\\x', '', 'relative\\dir', '..\\x', 'C:\\a\\..\\b',
    'C:\\a\\..\\..\\..\\b', 'CON:', 'nul:x', 'CON:\\x', 'con', 'CONx', 'COM\u00B9:y', '/unix/path', '\\unix', '/', '\\', 'C:', 'a:b', 'a:\\b', 'x:',
    '\u00E9\\\uD83D\uDE00', 'C:\\a b\\', 'C:\\a\\.\\b', 'C:\\\\a\\\\b', '.', '..', '...', '  C:\\x', 'C:\\x  ', '\\\\\\x', '///x', '\\\\\\\\x\\y\\z',
  ];
  const templates = [
    '', ' ', '%(title)s', '%(uploader)s - %(title)s', '..', '../x', 'a/../b', 'a\\..\\b', 'a/../../b', '/abs', '\\abs', 'C:\\x', 'c:x', 'C:', 'z:/y',
    'con:x', 'NUL:', 'a:b', 'a:/b', 'a/b:c', 'a/./b', './a', 'a//b', 'a\\\\b', '  a/b  ', '\ta\n', '..a', 'a..', '.../x', 'a/.. ', '%(title)s/%(id)s',
    'x/', '/', '\\', '.', '\u00E9\uD83D\uDE00/z', 'CON', 'con', 'lpt1:y', 'COM\u00B9:z', 'a/b/../../..', '?', '*', 'a|b', '\u2028 x \u2028',
    '\uFEFFx', ' ../ ', 'a/ ../b', '%(playlist)s/%(playlist_index)s - %(title)s', 'x'.repeat(300), '1:2', 'a/con:b', 'Con:', 'COM0:x', 'a:',
    '\\\\server\\share\\x', 'ab:c', '.\\x', 'x/..', 'x/../..', '../', 'a\u2028..\u2028/b',
  ];
  for (const outputDir of dirs) {
    for (const outputTemplate of ['', '%(title)s', 'a/b', 'con:x']) {
      add({ outputDir, extras: { outputTemplate } });
    }
    for (const playlistName of ['', 'Playlist', 'CON', 'a:b']) add({ outputDir, isPlaylist: true, playlistName });
    add({ outputDir, title: 'a:b' });
    add({ outputDir, extras: { splitChapters: true } });
    add({ outputDir, isPlaylist: true, playlistName: 'L', extras: { splitChapters: true, outputTemplate: 'x/y' } });
  }
  for (const outputTemplate of templates) {
    add({ extras: { outputTemplate } });
    add({ outputDir: '', extras: { outputTemplate } });
    add({ isPlaylist: true, playlistName: 'Playlist', extras: { outputTemplate } });
    add({ outputDir: 'D:\\x\\', isPlaylist: true, playlistName: '', extras: { outputTemplate, splitChapters: true } });
  }

  // Extras, one field at a time.
  const pools = {
    subtitleMode: ['off', 'embed', 'file', 'both'],
    subtitleLangs: ['en', '', ' ', 'en,fr', ' en , fr ', 'all', '-live_chat,en', 'fr,,de'],
    subtitleAuto: [true, false],
    embedChapters: [true, false],
    splitChapters: [true, false],
    sponsorBlock: ['off', 'mark', 'remove'],
    sponsorBlockCategories: [['sponsor'], [], ['sponsor', 'intro', 'outro', 'selfpromo', 'interaction', 'preview', 'music_offtopic'],
      ['bogus'], ['sponsor', 'bogus', 'intro'], ['SPONSOR'], [''], ['music_offtopic', 'sponsor']],
    writeThumbnail: [true, false],
    writeInfoJson: [true, false],
    writeDescription: [true, false],
    videoContainer: containers,
    videoCodec: codecs,
    cookieBrowser: ['', 'chrome', 'edge', 'firefox', 'brave', 'chromium', 'opera', 'vivaldi'],
    cookiesFile: ['', 'C:\\c\\cookies.txt', 'C:/c/cookies.txt', '\\\\srv\\sh\\c.txt', '/c.txt', '\\c.txt', 'c.txt', 'C:c.txt', 'C:', 'C:\\',
      '--exec=evil', ' C:\\x', 'Z:\\z', '\u00E9:\\x', 'C:\\a b\\c.txt', './c.txt'],
    outputTemplate: ['', '%(title)s', '../x', 'C:\\x', 'a/b'],
    rateLimit: ['', '2M', ' ', '0', '500K'],
    proxy: ['', 'http://p:1', ' ', 'socks5://127.0.0.1:9050'],
  };
  const shapes = [
    { isAudioOnly: true },
    { isAudioOnly: false, videoQuality: '1080p' },
    { isAudioOnly: true, isPlaylist: true, playlistName: 'L' },
  ];
  for (const [field, values] of Object.entries(pools)) {
    for (const value of values) {
      for (const shape of shapes) add({ ...shape, extras: { [field]: value } });
    }
  }
  // Cookies file against browser, and extras present but empty.
  for (const cookiesFile of pools.cookiesFile) {
    for (const cookieBrowser of ['', 'chrome']) add({ extras: { cookiesFile, cookieBrowser } });
  }
  add({ extras: {} });
  add({ extras: undefined });

  // Extras in combination: a seeded pick from every pool.
  let seed = 20260715;
  const rand = () => {
    seed = (seed + 0x6D2B79F5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const pick = (a) => a[Math.floor(rand() * a.length)];
  for (let i = 0; i < 400; i++) {
    const extras = {};
    for (const [field, values] of Object.entries(pools)) if (rand() < 0.6) extras[field] = pick(values);
    add({
      extras,
      isAudioOnly: rand() < 0.5,
      format: pick(audioFormats),
      quality: pick(bitrates),
      videoQuality: pick(qualities),
      isPlaylist: rand() < 0.4,
      playlistName: pick(names),
      title: pick(names),
      outputDir: pick(dirs),
      embedThumbnail: rand() < 0.5,
      embedMetadata: rand() < 0.5,
      skipExisting: rand() < 0.5,
      playlistLimit: pick(limits),
      playlistItems: pick(items),
      fallbackProfile: pick([undefined, true, false]),
    }, pick([null, 'C:\\ffmpeg.exe']));
  }

  // Sources, URLs, the retry profile and the ffmpeg path.
  const urls = [
    'https://youtube.com/watch?v=dQw4w9WgXcQ', 'https://open.spotify.com/track/4cOdK2', 'ytsearch:artist - title', '-rf', '--exec=calc',
    '', 'https://a.b/\nc', 'https://a.b/c d', '\uD83D\uDE00',
  ];
  for (const source of sources) {
    for (const url of urls) add({ source, url, track: source === 'spotify' ? track : undefined });
    add({ source, url: 'https://x.y/z', track });
  }
  add({ source: 'spotify', track: undefined, url: 'https://open.spotify.com/track/4cOdK2' });
  for (const fallbackProfile of [undefined, true, false]) {
    for (const isAudioOnly of [true, false]) for (const isPlaylist of [true, false]) add({ fallbackProfile, isAudioOnly, isPlaylist, playlistName: 'L' });
  }
  for (const ffmpeg of [null, '', 'C:\\ffmpeg.exe', 'C:\\Program Files\\ffmpeg\\ffmpeg.exe', '-x', ' ', '\u00E9\\ffmpeg']) {
    for (const over of [{}, { isAudioOnly: false }, { isPlaylist: true, playlistName: 'L', fallbackProfile: true }]) add(over, ffmpeg);
  }

  // Every string literal the existing tests use, as a title, folder, template and URL.
  const seen = new Set();
  for (const s of literals('electron/downloader.test.ts')) {
    if (seen.has(s) || s.length > 200) continue;
    seen.add(s);
    add({ title: s });
    add({ isPlaylist: true, playlistName: s, outputDir: s });
    add({ extras: { outputTemplate: s }, url: s });
  }

  writeFileSync(`${OUT}/args.json`, JSON.stringify({ base, cases }) + '\n');
  console.log(`args.json: ${cases.length} cases`);
};
