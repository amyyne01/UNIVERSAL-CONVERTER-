// Golden section for electron/downloader.ts metadata normalisation. toNum,
// thumb, compact, toSearchResult and platformOf are module-private, so they are
// reached through the public fetchMetadata / searchYouTube / searchSoundCloud
// with the instance's JSON runner replaced by one returning the corpus items.
import { writeFileSync } from 'node:fs';

export default async ({ OUT }) => {
  const { Downloader } = await import('../../dist-electron/electron/downloader.js');
  const d = new Downloader();
  // What the app would see: records that came through JSON.parse.
  const wire = (v) => JSON.parse(JSON.stringify(v));
  const serve = (items) => { d['runJson'] = async () => wire(items); };
  const attempt = async (fn) => { try { return { ok: wire(await fn()) }; } catch (e) { return { err: e.message }; } };

  // ── Search results ─────────────────────────────────────────────────────────
  const values = ['abc', '', ' ', null, 0, 5, 1.5, -1, true, false, ['x'], [], {}, { a: 1 }, 1e21, 1e-7, 123456789012345680000, 0.1,
    '日本', [null], [1, [2, 3]], [true], [{}], '12', ' 12 ', '12abc', '0x1F', '1e3', 'Infinity', 'NaN', '1_0', '+5', '.5', '5.',
    '0b11', '0o17', '١٢', ' \n\t5 ', '﻿5', '1e400', '-0x10', '0x', '00012', '1 2', [5], ['5'], [[7]], [' 3 '], ['x', 'y'],
    1e300, 12345678.9, 1e15, 0.000001, 'null', 'undefined', 'true'];
  const fields = ['id', 'title', 'uploader', 'channel', 'uploader_id', 'duration', 'thumbnail', 'thumbnails', 'url', 'webpage_url'];
  const baseItem = { id: 'v1', title: 'T', uploader: 'U', duration: 10, thumbnail: 'http://t/x.jpg', url: 'http://u/1', webpage_url: 'http://w/1' };
  const itemSets = [];
  for (const f of fields) for (const v of values) itemSets.push([{ ...baseItem, [f]: v }]);
  for (const f of fields) itemSets.push([Object.fromEntries(Object.entries(baseItem).filter(([k]) => k !== f))]);
  const thumbs = [[], [{ url: 'a' }], [{ url: 'a' }, { url: 'b' }], [{ url: 'a' }, {}], [{ url: 'a' }, null], [null], [{ url: 5 }], [{ url: null }], [{ url: '' }],
    ['str'], [1], 'notarray', { length: 1 }, { 0: { url: 'a' }, length: 1 }, [{ url: ['q'] }], [{ url: { a: 1 } }], [{ url: 0 }], [{ url: false }], [[]], [{ uri: 'a' }]];
  for (const t of thumbs) for (const th of ['', 0, null, 'x', undefined, false, [], {}]) itemSets.push([{ id: 'a', thumbnail: th, thumbnails: t }]);
  for (const t of thumbs) itemSets.push([{ id: 'a', thumbnails: t }]);
  const idsets = [
    {}, { id: 'a' }, { channel: 'C' }, { uploader: 'U', channel: 'C' }, { uploader: null, channel: 'C', uploader_id: 'I' }, { uploader: '', channel: 'C' },
    { uploader_id: 'I' }, { uploader: null, channel: null, uploader_id: null }, { url: 'u' }, { url: 'u', webpage_url: '' }, { url: 'u', webpage_url: null },
    { url: '', webpage_url: 'w' }, { url: null }, { webpage_url: 0 }, { url: 0, webpage_url: null }, { url: false },
    { id: 'a', title: 't', uploader: 'u', duration: 5, thumbnail: 't', url: 'u' }, { __proto__: null, id: 'p' },
    { 'id ': 'x', Title: 'X', ID: 1 },
  ];
  for (const o of idsets) itemSets.push([o]);
  itemSets.push([], [null], [5], ['str'], [true], [[]], [[1, 2]], [{}], [null, null], [5, { id: 'a' }, null, 'x'],
    [baseItem, { ...baseItem, id: 'v2' }, { ...baseItem, id: 'v3', duration: '7' }],
    Array.from({ length: 12 }, (_, i) => ({ id: `id${i}`, title: `Title ${i}`, duration: i * 60.5, webpage_url: `https://x/${i}` })));
  const search = [];
  for (const items of itemSets) {
    serve(items);
    search.push({
      items: wire(items),
      youtube: await attempt(() => d.searchYouTube('q')),
      soundcloud: await attempt(() => d.searchSoundCloud('q')),
    });
  }

  // ── fetchMetadata ──────────────────────────────────────────────────────────
  const urls = [
    'https://www.youtube.com/watch?v=dQw4w9WgXcQ', 'https://youtu.be/dQw4w9WgXcQ', 'https://www.youtube.com/watch?v=HnvTFVudNbU&list=RDHnvTFVudNbU&start_radio=1',
    'https://www.youtube.com/playlist?list=PLabc123', 'https://soundcloud.com/a/b', 'https://soundcloud.com/a/sets/b', 'https://SOUNDCLOUD.com/a/b',
    'https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT', 'https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M', 'spotify:album:abc',
    'https://www.instagram.com/reel/Dba6wNXFJ_A/', 'https://www.tiktok.com/@user/video/123', 'https://vimeo.com/123456', 'https://example.com/a?soundcloud',
    'https://example.com/a?spotify=youtube', 'https://example.com/youtu', 'https://example.com/a.mp4', 'ytsearch:artist - title', '', '   ',
    '  https://youtu.be/dQw4w9WgXcQ  ', 'https://spİotify.com/x', 'https://www.mixcloud.com/dj/set-1/', 'https://artist.bandcamp.com/album/x',
    'https://www.twitch.tv/videos/1', 'not a url', 'https://www.facebook.com/watch/?v=1', 'https://x.com/a/status/1',
  ];
  const full = {
    title: 'Video title', uploader: 'Chan', duration: 212.5, thumbnail: 'http://t/x.jpg', webpage_url: 'https://www.youtube.com/watch?v=abc',
    description: 'Some text', width: 1920, height: 1080, view_count: 1234567, like_count: 89, upload_date: '20240131', extractor_key: 'Youtube', extractor: 'youtube',
  };
  const entry = (i) => ({ id: `e${i}`, title: `Entry ${i}`, uploader: `U${i}`, duration: 30 + i, thumbnails: [{ url: `t${i}a` }, { url: `t${i}b` }], url: `https://e/${i}` });
  const headSets = [
    [], [null], [5], ['str'], [[]], [{}], [{ ...full }],
    [{ title: 'only title' }], [{ playlist_title: 'PL', title: 'T' }], [{ playlist_title: '', title: 'T' }], [{ playlist_title: null, title: 'T' }],
    [{ playlist_title: 0, title: 'T' }], [{ playlist_title: 5 }], [{ playlist_title: ['a', 'b'] }], [{ title: null }], [{ title: 12 }],
    [{ uploader: 'U', channel: 'C' }], [{ uploader: null, channel: 'C' }], [{ uploader: '', channel: 'C' }], [{ channel: 'C' }], [{ uploader_id: 'I' }],
    [{ duration: '90' }], [{ duration: 'x' }], [{ duration: null }], [{ duration: -4 }], [{ duration: 1e21 }],
    [{ thumbnail: 'T' }], [{ thumbnails: [{ url: 'a' }, { url: 'b' }] }], [{ thumbnail: '', thumbnails: [{ url: 'z' }] }], [{ thumbnails: [] }],
    [{ webpage_url: 'https://w/1' }], [{ webpage_url: '' }], [{ webpage_url: null }], [{ webpage_url: 5 }], [{ url: 'https://only-url' }],
    [{ description: '' }], [{ description: 0 }], [{ description: 'x' }], [{ description: ['a', 'b'] }], [{ description: null }],
    [{ width: 0 }], [{ width: 1280 }], [{ width: '1280' }], [{ width: null }], [{ width: -5 }], [{ width: 'abc' }], [{ width: '0' }], [{ width: ' ' }], [{ width: [640] }],
    [{ height: 0.5 }], [{ height: true }], [{ view_count: 0 }], [{ view_count: 7 }], [{ view_count: '1,234' }], [{ view_count: 1e21 }],
    [{ like_count: 3 }], [{ like_count: null }], [{ like_count: '0' }], [{ like_count: '0x10' }],
    [{ upload_date: '' }], [{ upload_date: '20240101' }], [{ upload_date: 20240101 }], [{ upload_date: null }],
    [{ extractor_key: 'TikTok', extractor: 'tiktok' }], [{ extractor: 'tiktok' }], [{ extractor_key: '', extractor: 'tiktok' }], [{ extractor_key: null, extractor: null }],
    [{ extractor_key: 0 }], [{ extractor_key: 'Instagram' }],
    [{ title: 'Playlist', playlist_title: 'PL', ...entry(0) }],
    [entry(0)], [entry(0), entry(1)], [entry(0), entry(1), entry(2)], [{ ...full, ...entry(0) }, entry(1)],
    [null, entry(1)], [5, null, 'x'], [{ playlist_title: 'PL', ...entry(0) }, entry(1), null],
    [{ ...entry(0), _type: 'playlist', entries: [entry(5), entry(6)], playlist_title: 'Nested' }],
    [{ _type: 'playlist', id: 'PLx', title: 'A playlist', entries: [entry(1)], webpage_url: 'https://www.youtube.com/playlist?list=PLx' }],
    Array.from({ length: 12 }, (_, i) => entry(i)),
  ];
  const metadata = [];
  for (const url of urls) {
    for (const items of headSets) {
      serve(items);
      metadata.push({ url, items: wire(items), out: await attempt(() => d.fetchMetadata(url)) });
    }
  }

  const golden = { search, metadata };
  // serde_json (without its float_roundtrip feature) can misread a float by one
  // digit, so every number that is not a safe integer travels as {"$f": "<text>"}
  // and the Rust test parses the text with Rust's own, exact, parser.
  const tag = (_k, v) => (typeof v === 'number' && Number.isFinite(v) && !Number.isSafeInteger(v) ? { $f: String(v) } : v);
  writeFileSync(`${OUT}/metadata.json`, JSON.stringify(golden, tag) + '\n');
  console.log(`metadata.json: search ${search.length}, metadata ${metadata.length}`);
};
