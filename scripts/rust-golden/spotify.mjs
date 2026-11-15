// Golden section for electron/spotify.ts. The real SpotifyHandler runs against a
// scripted fake `fetch` that records every request and serves canned responses.
// Each scenario is a list of steps on ONE handler; a step records its result and
// the requests it made, in order, with the response each one got. The Rust test
// replays the same responses and checks the same requests and results.
import { writeFileSync } from 'node:fs';

export default async ({ OUT, literals }) => {
  const { SpotifyHandler } = await import('../../dist-electron/electron/spotify.js');

  const TOKEN = 'BQ_anon_token_abc';
  const T0 = 1_700_000_000_000;
  const MIN = 60_000;
  const HASH = 'abcdef0123456789'.repeat(4);
  const HASH2 = '0123456789abcdef'.repeat(4);
  const EMB = 'https://open.spotify.com/embed/';
  const GQL = 'https://api-partner.spotify.com/pathfinder/v1/query';
  const HOME = 'https://open.spotify.com/';
  const wire = (v) => (v === undefined ? null : JSON.parse(JSON.stringify(v)));

  // ── The fake network ────────────────────────────────────────────────────────
  let clock = T0;
  let router = null;
  let calls = [];
  const bugs = [];
  const RealNow = Date.now;
  const realFetch = globalThis.fetch;
  Date.now = () => clock;
  globalThis.fetch = async (url, init = {}) => {
    const rec = {
      req: {
        url: String(url), method: init.method ?? 'GET', headers: Object.entries(init.headers ?? {}),
        body: init.body ?? null, signal: init.signal instanceof AbortSignal,
      },
      resp: { throw: true }, advance: 0,
    };
    calls.push(rec);
    let spec;
    try { spec = router(String(url)); } catch (e) { bugs.push(e.stack); }
    if (!spec || spec.throw) throw new Error('network');
    rec.resp = { text: spec.text };
    rec.advance = spec.advance ?? 0;
    clock += rec.advance;
    return { ok: true, status: 200, text: async () => spec.text, json: async () => JSON.parse(spec.text) };
  };

  // ── Response and route helpers ──────────────────────────────────────────────
  const text = (t, extra) => ({ text: t, ...extra });
  const json = (o, extra) => text(JSON.stringify(o), extra);
  const html = (inner) => text(`<html><body>${inner}</body></html>`);
  const page = (nextData) => html(`<script id="__NEXT_DATA__" type="application/json">${JSON.stringify(nextData)}</script>`);
  const nd = (entity, token = TOKEN) => ({ props: { pageProps: { state: { data: { entity, ...(token ? { session: { accessToken: token, accessTokenExpirationTimestampMs: 5 } } : {}) } } } } });
  const gqlOf = (url) => {
    const u = new URL(url);
    return { op: u.searchParams.get('operationName'), vars: JSON.parse(u.searchParams.get('variables')), hash: JSON.parse(u.searchParams.get('extensions')).persistedQuery.sha256Hash };
  };
  /** A router over the four kinds of URL the module fetches; a missing handler is a network failure. */
  function site({ embed, gql, home, other } = {}) {
    let n = 0;
    return (url) => {
      if (url.startsWith(EMB)) { const [type, id] = url.slice(EMB.length).split('/'); return embed?.({ type, id, url }); }
      if (url.startsWith(GQL)) return gql?.({ ...gqlOf(url), n: ++n, url });
      if (url === HOME) return home?.();
      return other?.(url);
    };
  }
  const scripts = (...srcs) => html(srcs.map((s) => `<script src="${s}"></script>`).join(''));
  const bundleWith = (op, hash = HASH) => text(`...${op}...,"sha256Hash":"${hash}"...`);
  /** Home page with the given bundles served from https://cdn/<name>.js. */
  const healSite = (bundles, order = Object.keys(bundles)) => site({
    home: () => scripts(...order.map((n) => `https://cdn/${n}.js`)),
    other: (url) => bundles[url.slice('https://cdn/'.length, -3)],
  });

  // ── Fixtures ────────────────────────────────────────────────────────────────
  const trackEntity = {
    type: 'track', uri: 'spotify:track:TR1', name: 'Single Song', artists: [{ name: 'Solo Artist' }, { name: 'Feature' }],
    album: { name: 'The Album', date: { year: 2019 } }, coverArt: { sources: [{ url: 'small.jpg', width: 64 }, { url: 'big.jpg', width: 640 }] },
    duration: 201000, releaseDate: { isoString: '2019-05-01T00:00:00Z' },
  };
  const playlistEntity = {
    type: 'playlist', uri: 'spotify:playlist:PL1', name: 'My Playlist', subtitle: 'a fine mix', owner: { name: 'DJ Embed' },
    coverArt: { sources: [{ url: 'plcover.jpg' }] },
    trackList: [
      { uri: 'spotify:track:e1', title: 'Embed Song 1', subtitle: 'Embed Artist 1', duration: 111000 },
      { uri: 'spotify:track:e2', title: 'Embed Song 2', subtitle: 'Embed Artist 2', duration: 222000 },
    ],
  };
  const albumEntity = { type: 'album', uri: 'spotify:album:AL1', name: 'Greatest Hits', artists: [{ name: 'Embed Band' }], coverArt: { sources: [{ url: 'aecover.jpg' }] }, trackList: [{ uri: 'spotify:track:ae1', title: 'AE1', subtitle: 'X' }] };
  const art = (n) => ({ items: [{ profile: { name: n } }] });
  const ALB = { name: 'Alb', coverArt: { sources: [{ url: 'c0.jpg' }, { url: 'c1.jpg' }] }, date: { year: 2021 } };
  const plTrack = (i, o = {}) => ({ uri: `spotify:track:g${i}`, name: `S${i}`, artists: art(`R${i}`), albumOfTrack: ALB, trackDuration: { totalMilliseconds: 1000 + i }, trackNumber: i, ...o });
  const plItem = (i, o) => ({ itemV2: { data: plTrack(i, o) } });
  const alItem = (i, o = {}) => ({ track: { uri: `spotify:track:a${i}`, name: `T${i}`, artists: art(`R${i}`), trackNumber: i + 1, duration: { totalMilliseconds: 2000 + i }, ...o } });
  const plRoot = (items, total, extra = {}) => ({ name: 'PL', description: 'desc', ownerV2: { data: { name: 'Own' } }, images: { items: [{ sources: [{ url: 'p.jpg' }] }] }, content: { totalCount: total, items }, ...extra });
  const plBody = (items, total, extra) => ({ data: { playlistV2: plRoot(items, total, extra) } });
  const alRoot = (items, total, extra = {}) => ({ name: 'AL', artists: art('Band'), coverArt: { sources: [{ url: 'ac.jpg' }] }, date: { year: 2020 }, tracksV2: { totalCount: total, items }, ...extra });
  const alBody = (items, total, extra) => ({ data: { albumUnion: alRoot(items, total, extra) } });
  const listOf = (n, mk, from = 0) => Array.from({ length: Math.max(0, n) }, (_, i) => mk(from + i));
  const idOf = (vars) => vars.uri.split(':')[2];

  // ── Scenarios ───────────────────────────────────────────────────────────────
  const scenarios = [];
  const S = (name, steps, route) => scenarios.push({ name, steps, route });
  const fetchPl = (arg, at) => ({ op: 'fetchPlaylist', arg, at });
  const fetchAl = (arg, at) => ({ op: 'fetchAlbum', arg, at });
  const fetchTr = (arg, at) => ({ op: 'fetchTrack', arg, at });
  const search = (arg, at) => ({ op: 'search', arg, at });
  const tokenAt = (at) => ({ op: 'token', at });
  const state = (at) => ({ op: 'state', at });
  const heal = (arg, at) => ({ op: 'heal', arg, at });

  // The fixtures of electron/spotify.test.ts.
  S('track embed', [fetchTr('https://open.spotify.com/track/TR1')], site({ embed: () => page(nd(trackEntity)) }));
  S('token off the known path', [fetchTr('TR1'), tokenAt()], site({ embed: () => page({ props: { pageProps: { somewhere: { deep: { accessToken: TOKEN } } } }, entity: trackEntity }) }));
  S('playlist over graphql', [fetchPl('spotify:playlist:PL1')], site({
    embed: () => page(nd(playlistEntity)),
    gql: () => json(plBody([plItem(1), plItem(2)], 2, { name: 'GQL Playlist Name', description: 'real description', ownerV2: { data: { name: 'Real Owner' } }, images: { items: [{ sources: [{ url: 'gqlplcover.jpg' }] }] } })),
  }));
  S('album paged 50 then 10', [fetchAl('AL1')], site({
    embed: () => page(nd({ type: 'album', uri: 'spotify:album:AL1', name: 'Greatest Hits' })),
    gql: ({ vars }) => json(alBody(vars.offset === 0 ? listOf(50, alItem) : listOf(10, alItem, 50), 60)),
  }));
  S('album page 2 fails', [fetchAl('AL1'), state()], site({
    embed: () => page(nd({ type: 'album', uri: 'spotify:album:AL1', name: 'Greatest Hits' })),
    gql: ({ vars }) => (vars.offset === 0 ? json(alBody(listOf(50, alItem), 120)) : undefined),
  }));
  S('stale hash heals from the bundle', [fetchPl('PL1'), state()], site({
    embed: () => page(nd(playlistEntity)),
    gql: ({ n }) => (n === 1 ? json({ errors: [{ message: 'PersistedQueryNotFound' }] }) : json({ data: { playlistV2: { name: 'Healed', content: { totalCount: 1, items: [plItem(1)] } } } })),
    home: () => scripts('https://open.spotifycdn.com/web-player.xyz.js'),
    other: () => text(`...fetchPlaylist...,"sha256Hash":"${HASH}"...`),
  }));
  S('tier 3 without a token', [fetchPl('PL1'), tokenAt()], site({ embed: () => page(nd(playlistEntity, null)) }));
  S('token expiry boundary', [
    { op: 'state' }, { op: 'seedToken', value: 'X', expiresAt: T0 + 1000, at: T0 }, tokenAt(T0 + 999), tokenAt(T0 + 1000),
    { op: 'seedToken', value: 'X', expiresAt: T0 + 1000, at: T0 }, tokenAt(T0 + 1001), state(), tokenAt(T0),
  ], site());
  S('search with no network', [search('daft punk')], site());
  S('unrecognisable id', [fetchTr('!!')], site({ other: () => text('') }));
  S('search primes a token', [search('daft punk'), search('daft punk')], site({
    embed: () => page(nd(trackEntity)), gql: () => json({ data: { searchV2: { tracksV2: { items: [] } } } }),
  }));
  S('search heals the placeholder hash', [search('daft punk'), state()], site({
    embed: () => page(nd(trackEntity)),
    gql: ({ n }) => (n === 1 ? json({ errors: [{ message: 'PersistedQueryNotFound' }] }) : json({ data: { searchV2: { tracksV2: { items: [] } } } })),
    home: () => scripts('https://open.spotifycdn.com/web-player.xyz.js'),
    other: () => text(`...searchDesktop...,"sha256Hash":"${HASH}"...`),
  }));
  S('every fetch aborts', [fetchTr('TR1'), search('daft punk'), fetchPl('PL1'), fetchAl('AL1')], site());

  // ── URL forms ───────────────────────────────────────────────────────────────
  const urlForms = [...new Set([
    ...literals('electron/spotify.test.ts'), ...literals('electron/url-detector.test.ts'),
    'https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT', 'https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT?si=abc123',
    'https://open.spotify.com/intl-fr/track/4cOdK2wGLETKBW3PvgPWqT', 'https://open.spotify.com/intl-FR/playlist/37i9dQZF1DX', 'https://open.spotify.com/intl-fra/track/abc',
    'https://open.spotify.com/intl-f/track/abc', 'https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M', 'https://open.spotify.com/album/1A2B3C',
    'https://open.spotify.com/artist/0OdUWJ0sBjDrqHygGUXeCF', 'http://open.spotify.com/track/abc', 'open.spotify.com/track/abc', 'https://OPEN.SPOTIFY.COM/TRACK/ABC',
    'https://open.spotify.com/Track/abc', 'spotify:track:4cOdK2wGLETKBW3PvgPWqT', 'SPOTIFY:ALBUM:abc', 'spotify:playlist:PL1', 'spotify:artist:xyz',
    'spotify:user:me:playlist:abc', 'spotify:episode:abc', 'spotify:show:abc', 'https://open.spotify.com/episode/abc', 'https://open.spotify.com/show/abc',
    'https://open.spotify.com/user/me/playlist/abc', 'https://open.spotify.com/track/', 'https://open.spotify.com/track/-abc', 'https://open.spotify.com/track/ab-c',
    'https://open.spotify.com/track/ab_c', 'https://open.spotify.com/track/éabc', 'https://open.spotify.com/track/abcé', 'https://open.spotify.com/track/١٢',
    'https://open.spotify.com/embed/track/abc', 'https://open.spotify.com/embed/playlist/abc', 'https://evil.com/?u=open.spotify.com/track/abc', 'xopen.spotify.com/track/abc',
    'https://open.spotify.com/track:abc', 'https://open.spotify.com/track/abc/extra', 'https://open.spotify.com/track/abc?si=x#frag', '  https://open.spotify.com/track/abc  ',
    'text before spotify:track:abc after', 'spotify:track', 'spotify:track:', 'spotify::abc', 'spotify:tracks:abc', 'spotify:trackabc', 'open.spotifyXcom/track/abc',
    'abc', ' abc ', 'abc\n', ' abc ', '﻿abc', 'ab c', 'ab-c', 'ab_c', 'é', '١٢', 'ABC123xyz', '0', '', ' ', '!!', '..', 'a/b', 'a:b',
    '4cOdK2wGLETKBW3PvgPWqT', 'a'.repeat(500), '😀', 'abc ', ' abc ', 'abc\0',
  ])];
  S('url parsing', urlForms.flatMap((arg) => [{ op: 'parseUrl', arg }, { op: 'resolveId', arg }]), site());

  // ── Collection sizes and pagination ─────────────────────────────────────────
  const plSizes = { P0: [0, true], P1: [1, true], P100: [100, true], P101: [101, true], P250: [250, true], P100n: [100, false], P250n: [250, false] };
  S('playlist sizes', [...Object.keys(plSizes).map((id) => fetchPl(id)), state()], site({
    embed: () => page(nd(playlistEntity)),
    gql: ({ vars }) => {
      const [N, withTotal] = plSizes[idOf(vars)];
      return json(plBody(listOf(Math.min(100, N - vars.offset), plItem, vars.offset), withTotal ? N : undefined));
    },
  }));
  const alSizes = { A0: [0, true], A1: [1, true], A50: [50, true], A51: [51, true], A120: [120, true], A50n: [50, false] };
  S('album sizes', Object.keys(alSizes).map((id) => fetchAl(id)), site({
    embed: () => page(nd(albumEntity)),
    gql: ({ vars }) => {
      const [N, withTotal] = alSizes[idOf(vars)];
      return json(alBody(listOf(Math.min(50, N - vars.offset), alItem, vars.offset), withTotal ? N : undefined));
    },
  }));
  // Items that never map to a track never end a page early, so the loop runs to its cap.
  S('album page cap', [fetchAl('CAP')], site({
    embed: () => page(nd(albumEntity)),
    gql: ({ vars }) => json(alBody(listOf(50, (i) => (vars.offset === 0 && i === 0 ? alItem(0) : null)))),
  }));
  S('playlist page cap', [fetchPl('CAP')], site({
    embed: () => page(nd(playlistEntity)),
    gql: ({ vars }) => json(plBody(listOf(100, (i) => (vars.offset === 0 && i === 0 ? plItem(0) : null)))),
  }));
  S('no tracks anywhere falls to tier 3', [fetchPl('CAP')], site({
    embed: () => page(nd(playlistEntity)), gql: () => json(plBody(listOf(100, () => null))),
  }));
  const totals = { T1: [1, 3], T2: [10, 3], T3: [0, 3], T4: [null, 3], T5: [3, 3], T6: [-1, 3], T7: [2.5, 3] };
  S('total versus items', Object.keys(totals).flatMap((id) => [fetchPl(id), fetchAl(id)]), site({
    embed: () => page(nd(playlistEntity)),
    gql: ({ op, vars }) => {
      const [total, n] = totals[idOf(vars)];
      return json(op === 'fetchPlaylist' ? plBody(listOf(n, plItem), total) : alBody(listOf(n, alItem), total));
    },
  }));
  S('token expires between pages', [fetchAl('AL1'), tokenAt(), state()], site({
    embed: () => page(nd(albumEntity)),
    gql: ({ vars }) => json(alBody(listOf(50, alItem, vars.offset), 120), { advance: 26 * MIN }),
  }));
  S('token reused while cached, minted again by an embed', [
    fetchPl('PL1', T0), fetchPl('PL1', T0 + 10 * MIN), fetchPl('PL1', T0 + 24 * MIN + 59_999), fetchPl('PL1', T0 + 25 * MIN + 60_000), state(),
  ], site({
    embed: (() => { let n = 0; return () => page(nd(playlistEntity, ++n === 4 ? TOKEN : null)); })(),
    gql: () => json(plBody([plItem(1)], 1)),
  }));
  S('first embed gives a token and the next does not', [fetchPl('PL1'), fetchPl('PL1', T0 + 1), fetchPl('PL1', T0 + 25 * MIN)], site({
    embed: (() => { let n = 0; return () => (++n === 1 ? page(nd(playlistEntity)) : page(nd(playlistEntity, null))); })(),
    gql: () => json(plBody([plItem(1)], 1)),
  }));

  // ── Track nodes over graphql ────────────────────────────────────────────────
  const trackShapes = [
    null, {}, { itemV2: null }, { itemV2: { data: null } }, { itemV2: { data: 5 } }, { itemV2: { data: 'str' } }, { itemV2: { data: [] } },
    { itemV2: { data: { uri: 'spotify:episode:e1', name: 'Ep' } } }, { itemV2: { data: { uri: 'spotify:local:a:b:c:1', name: 'Local' } } },
    { itemV2: { data: { uri: '', name: 'Empty' } } }, { itemV2: { data: { uri: null, name: 'NullUri' } } }, { itemV2: { data: { name: 'NoUri' } } },
    { itemV2: { data: { uri: 'SPOTIFY:TRACK:upper', name: 'Upper' } } }, { itemV2: { data: { uri: ' spotify:track:space', name: 'Space' } } },
    { itemV2: { data: { uri: 'spotify:track:', name: 'NoId' } } }, { itemV2: { data: { uri: 'spotify:track:bare' } } },
    { itemV2: { data: { uri: 'spotify:track:nm0', name: null, trackNumber: null } } }, { itemV2: { data: { uri: 'spotify:track:nm1', name: '' } } },
    { itemV2: { data: { uri: 'spotify:track:nm2', name: 'N', albumOfTrack: null } } }, { itemV2: { data: { uri: 'spotify:track:nm3', name: 'N', albumOfTrack: 'x' } } },
    { itemV2: { data: { uri: 'spotify:track:nm4', name: 'N', albumOfTrack: { name: null } } } }, { itemV2: { data: { uri: 'spotify:track:nm5', name: '日本😀' } } },
  ];
  const durations = [
    undefined, null, 0, 1, 123456, 2.5, { totalMilliseconds: null }, { totalMilliseconds: 0 }, { totalMilliseconds: 99 },
  ].flatMap((v) => [{ trackDuration: v }, { duration: v }, { trackDuration: v, duration: { totalMilliseconds: 7 } }]);
  const artistShapes = [
    undefined, null, [], {}, { items: [] }, { items: null }, { items: {} }, 'str', 5, true,
    { items: [{ profile: { name: 'A' } }, { profile: { name: 'B' } }] }, [{ profile: { name: 'A' } }], [{ name: 'A' }, { name: 'B' }], ['A', 'B'], [{ profile: { name: 'A' }, name: 'ignored' }],
    [{ profile: { name: '' }, name: 'fallback' }], [{ profile: {}, name: 'fb' }], [{ profile: null, name: 'fb' }], [{ profile: { name: null }, name: 'fb' }], [{ profile: { name: 5 }, name: 'fb' }],
    [{ profile: { name: 5 } }], [null, 5, '', {}, 'ok', { name: '' }, { name: 'ok2' }, [], { name: ['x'] }], [{ name: 'A' }, null, { name: 'C' }], { items: [{ name: 'X' }], other: 1 }, { items: [{ profile: { name: 'É😀' } }] },
  ];
  const dateShapes = [
    undefined, null, '', 0, false, {}, [], { year: 2021 }, { year: '2021' }, { year: 0 }, { year: 0, isoString: '1999-01-01' }, { year: null, isoString: '1999-01-01T00:00:00Z' },
    { isoString: '2005-05-05T00:00Z' }, { isoString: '99-01' }, { isoString: '12345' }, { isoString: 20200101 }, { isoString: null }, { isoString: '' }, { isoString: ' 2005' },
    '2010-10-10', '2010', '201', 'abcd', '١٢٣٤', 2020, true, { year: 2019.5 }, { year: true }, { year: { a: 1 } }, { year: [2001, 2002] }, { year: 'abc' }, { year: -5 },
    { year: '', isoString: '1980-01-01' }, { isoString: { a: 1 } }, { isoString: '1999' }, { isoString: '19999-01-01' },
  ];
  const coverShapes = [
    undefined, null, {}, [], 'str', 5, { sources: [] }, { sources: null }, { sources: {} }, { sources: 'x' }, { sources: [{}] }, { sources: [null] }, { sources: [{ url: null }] }, { sources: [{ url: '' }] },
    { sources: [{ url: 'a' }] }, { sources: [{ url: 'a' }, { url: 'b' }] }, { sources: [{ url: 'a' }, {}] }, { sources: [{ url: 'a' }, null] }, { sources: [{ url: 'a' }, { url: '' }] },
    [{ url: 'bare' }], [{ url: 'x' }, { url: 'y' }], { sources: [{ url: 'a', width: 1 }, { url: 'b', width: 0 }] }, { url: 'direct' }, { sources: ['plain'] },
  ];
  const albumWith = (o) => ({ name: 'Alb', ...o });
  const plPages = {
    mixed: plBody(trackShapes.map((t) => t), 99),
    mixednt: plBody([plItem(1), null, plItem(2), {}, plItem(3)], undefined),
    durations: plBody(durations.map((d, i) => plItem(i, { trackDuration: undefined, duration: undefined, ...d })), 99),
    artists: plBody(artistShapes.map((a, i) => plItem(i, { artists: a })), 99),
    dates: plBody(dateShapes.map((d, i) => plItem(i, { albumOfTrack: albumWith({ date: d }) })), 99),
    covers: plBody(coverShapes.map((c, i) => plItem(i, { albumOfTrack: albumWith({ coverArt: c }) })), 99),
    nums: plBody([plItem(1, { trackNumber: 0 }), plItem(2, { trackNumber: null }), plItem(3, { trackNumber: undefined }), plItem(4, { trackNumber: 7 }), plItem(5, { trackNumber: 2.5 })], 99),
    thrownum: plBody([plItem(1), plItem(2, { uri: 5 }), plItem(3)], 3),
    throwtrue: plBody([plItem(1, { uri: true })], 1),
    throwfalse: plBody([plItem(1, { uri: false })], 1),
    throwobj: plBody([plItem(1, { uri: {} })], 1),
    throwarr: plBody([plItem(1, { uri: ['spotify:track:x'] })], 1),
    throwzero: plBody([plItem(1, { uri: 0 })], 1),
    // Playlist level fields.
    mnullname: plBody([plItem(1)], 1, { name: null }), memptyname: plBody([plItem(1)], 1, { name: '' }),
    mnoowner: plBody([plItem(1)], 1, { ownerV2: null }), mowner0: plBody([plItem(1)], 1, { ownerV2: { data: { name: '' } } }), mowner1: plBody([plItem(1)], 1, { ownerV2: { data: null } }),
    mdesc5: plBody([plItem(1)], 1, { description: 5 }), mdescnull: plBody([plItem(1)], 1, { description: null }), mdescempty: plBody([plItem(1)], 1, { description: '' }),
    mdescobj: plBody([plItem(1)], 1, { description: { a: 1 } }), mdescarr: plBody([plItem(1)], 1, { description: ['a', null, 'b'] }), mdescbool: plBody([plItem(1)], 1, { description: false }),
    mdescfloat: plBody([plItem(1)], 1, { description: 2.5 }), mdescbig: plBody([plItem(1)], 1, { description: 1e20 }),
    mimg1: plBody([plItem(1)], 1, { images: { items: [] }, coverArt: { sources: [{ url: 'meta-cover.jpg' }] } }), mimg2: plBody([plItem(1)], 1, { images: null }),
    mimg3: plBody([plItem(1)], 1, { images: { items: [{ sources: [] }] }, coverArt: { sources: [{ url: 'meta-cover.jpg' }] } }), mimg4: plBody([plItem(1)], 1, { images: { items: { 0: { sources: [{ url: 'obj0.jpg' }] } } } }),
    mimg5: plBody([plItem(1)], 1, { images: { items: [{ sources: [{ url: '' }] }] }, coverArt: null }),
  };
  S('playlist track shapes', [...Object.keys(plPages).map((id) => fetchPl(id)), { op: 'search', arg: 'x', at: T0 }], site({
    embed: () => page(nd(playlistEntity)),
    gql: ({ op, vars }) => (op === 'fetchPlaylist' ? json(plPages[idOf(vars)]) : json({ data: { searchV2: { tracksV2: { items: [] } } } })),
  }));

  const alPages = {
    mixed: alBody(trackShapes.map((t) => (t && t.itemV2 ? { track: t.itemV2.data } : t)), 99),
    artists: alBody(artistShapes.map((a, i) => alItem(i, { artists: a })), 99),
    dates: dateShapes.map((d) => alBody([alItem(1)], 1, { date: d })),
    covers: coverShapes.map((c) => alBody([alItem(1)], 1, { coverArt: c })),
    owners: [undefined, null, {}, [], 'str', { items: [] }, { items: [{ profile: { name: '' } }, { profile: { name: 'Second' } }] }, ['Str', 'Two'], [{ name: 'Plain' }]].map((a) => alBody([alItem(1)], 1, { artists: a })),
    throwuri: alBody([alItem(1, { uri: 12 })], 1),
    meta: [{ name: null }, { name: '' }, { artists: undefined }, { name: 'Z', coverArt: undefined }].map((o) => alBody([alItem(1)], 1, o)),
    nodate: alBody([alItem(1)], 1, { date: undefined }),
    noroot: { data: { albumUnion: null } }, nodata: {}, nodata2: { data: null }, rootstr: { data: { albumUnion: 'str' } }, rootarr: { data: { albumUnion: [] } },
    tracksnull: { data: { albumUnion: { name: 'X', tracksV2: null } } }, itemsstr: { data: { albumUnion: { name: 'X', tracksV2: { items: 'abc', totalCount: 3 } } } },
    itemsobj: { data: { albumUnion: { name: 'X', tracksV2: { items: {}, totalCount: 3 } } } },
  };
  const alSteps = [];
  const alIds = {};
  for (const [key, v] of Object.entries(alPages)) {
    (Array.isArray(v) ? v : [v]).forEach((body, i) => { const id = `${key}${i}`; alIds[id] = body; alSteps.push(fetchAl(id)); });
  }
  S('album shapes', alSteps, site({ embed: () => page(nd(albumEntity)), gql: ({ vars }) => json(alIds[idOf(vars)]) }));
  S('album without an embed entity', [fetchAl('NOENT'), fetchAl('NOENT2')], site({
    embed: ({ id }) => page(nd(undefined)), gql: ({ vars }) => (idOf(vars) === 'NOENT' ? json(alBody([alItem(1)], 1, { name: undefined, artists: undefined })) : undefined),
  }));

  // ── Search ──────────────────────────────────────────────────────────────────
  const sItem = (i, o) => ({ item: { data: plTrack(i, o) } });
  const sBare = (i, o) => ({ data: plTrack(i, o) });
  const searchBodies = {
    'one track': { data: { searchV2: { tracksV2: { items: [sItem(1)] } } } },
    bare: { data: { searchV2: { tracksV2: { items: [sBare(1), sBare(2)] } } } },
    legacy: { data: { searchV2: { tracks: { items: [sItem(1)] } } } },
    'v2 null falls to tracks': { data: { searchV2: { tracksV2: { items: null }, tracks: { items: [sItem(2)] } } } },
    'v2 missing falls to tracks': { data: { searchV2: { tracksV2: null, tracks: { items: [sItem(3)] } } } },
    'v2 empty wins': { data: { searchV2: { tracksV2: { items: [] }, tracks: { items: [sItem(2)] } } } },
    'items not an array': { data: { searchV2: { tracksV2: { items: {} }, tracks: { items: [sItem(2)] } } } },
    'items string': { data: { searchV2: { tracksV2: { items: 'abc' } } } },
    'no results': { data: { searchV2: { tracksV2: { items: [] } } } },
    'no searchV2': { data: {} }, 'data null': { data: null }, empty: {}, 'search null': { data: { searchV2: null } },
    mixed: { data: { searchV2: { tracksV2: { items: [null, 5, {}, { item: null }, { item: { data: null } }, sItem(1), { item: { data: { uri: 'spotify:episode:x' } } }, sBare(2), { item: { data: sItem(3).item.data }, data: sBare(9).data }, { item: {}, data: sBare(4).data }, { item: { data: null }, data: sBare(5).data }] } } } },
    throws: { data: { searchV2: { tracksV2: { items: [sItem(1), sItem(2, { uri: 9 })] } } } },
    errors: { errors: [{ message: 'x' }], data: { searchV2: { tracksV2: { items: [sItem(1)] } } } },
    'errors empty': { errors: [], data: { searchV2: { tracksV2: { items: [sItem(1)] } } } },
    'errors null': { errors: null, data: { searchV2: { tracksV2: { items: [sItem(1)] } } } },
    'errors zero': { errors: 0, data: { searchV2: { tracksV2: { items: [sItem(1)] } } } },
    'errors string': { errors: 'x', data: { searchV2: { tracksV2: { items: [sItem(1)] } } } },
    'errors object': { errors: {}, data: { searchV2: { tracksV2: { items: [sItem(1)] } } } },
  };
  const rawBodies = { null: 'null', 'not json': 'not json', blank: '', array: '[]', zero: '0', five: '5', str: '"x"', false: 'false', html: '<html></html>', 'ws json': '  {"data":{"searchV2":{"tracksV2":{"items":[]}}}}\n' };
  const searchTerms = ['daft punk', 'a&b=c?d#e', 'Ünïcode 日本 😀', 'quote"s \\ back', 'new\nline\ttab', '', ' ', 'x'.repeat(60), "100% (live)! *~'", '/path/with/slashes', 'emoji 🎵 + plus', 'ctl\u0001\u001f\u007f ', 'café', '{"json":1}'];
  const hash0 = (n) => (n === 1 ? json({ errors: [{ message: 'PersistedQueryNotFound' }] }) : null);
  const termSteps = [search('daft punk'), ...searchTerms.map((t) => search(t))];
  S('search terms', termSteps, site({
    embed: () => page(nd(trackEntity)),
    gql: ({ n, vars }) => hash0(n) ?? json({ data: { searchV2: { tracksV2: { items: [sItem(1)] } } } }),
    home: () => scripts('https://cdn/s.js'), other: () => bundleWith('searchDesktop'),
  }));
  for (const [name, body] of Object.entries(searchBodies)) {
    S(`search body: ${name}`, [search('q'), state()], site({
      embed: () => page(nd(trackEntity)), gql: ({ n }) => hash0(n) ?? json(body), home: () => scripts('https://cdn/s.js'), other: () => bundleWith('searchDesktop'),
    }));
  }
  for (const [name, body] of Object.entries(rawBodies)) {
    S(`search raw body: ${name}`, [search('q'), state()], site({
      embed: () => page(nd(trackEntity)), gql: ({ n }) => hash0(n) ?? text(body), home: () => scripts('https://cdn/s.js'), other: () => bundleWith('searchDesktop'),
    }));
  }
  S('search with a token but a failing embed bootstrap', [search('q')], site({ embed: () => html('<p>no data</p>'), gql: () => json({}) }));
  S('search seeded token skips the bootstrap', [{ op: 'seedToken', value: 'SEED', expiresAt: T0 + MIN, at: T0 }, search('q'), search('q', T0 + MIN), state()], site({
    embed: () => page(nd(trackEntity, 'MINTED')), gql: ({ n }) => hash0(n) ?? json(searchBodies['one track']), home: () => scripts('https://cdn/s.js'), other: () => bundleWith('searchDesktop'),
  }));
  S('search heal fails then cooldown', [search('q', T0), search('q', T0 + 30 * MIN - 1), search('q', T0 + 61 * MIN), state()], site({
    embed: () => page(nd(trackEntity)), gql: () => json({ errors: [{}] }),
  }));

  // ── Embed pages: Tier 1 and Tier 3 ──────────────────────────────────────────
  const embedPages = {
    'plain': page(nd(trackEntity)),
    'no script': html('<p>nothing</p>'), 'empty body': text(''),
    'single quoted id': html(`<script id='__NEXT_DATA__'>${JSON.stringify(nd(trackEntity))}</script>`),
    'extra attributes': html(`<script id="__NEXT_DATA__" type="application/json" nonce="n" data-x='>'>${JSON.stringify(nd(trackEntity))}</script>`),
    'id not first': html(`<script type="application/json" id="__NEXT_DATA__">${JSON.stringify(nd(trackEntity))}</script>`),
    'no attributes after id': html(`<script id="__NEXT_DATA__">${JSON.stringify(nd(trackEntity))}</script>`),
    'upper case tag': html(`<SCRIPT id="__NEXT_DATA__">${JSON.stringify(nd(trackEntity))}</SCRIPT>`),
    'two scripts first wins': html(`<script id="__NEXT_DATA__">${JSON.stringify(nd(trackEntity, 'FIRST'))}</script><script id="__NEXT_DATA__">${JSON.stringify(nd(trackEntity, 'SECOND'))}</script>`),
    'other script first': html(`<script id="other">{}</script><script id="__NEXT_DATA__">${JSON.stringify(nd(trackEntity))}</script>`),
    'unterminated': html(`<script id="__NEXT_DATA__">${JSON.stringify(nd(trackEntity))}`),
    'whitespace around json': html(`<script id="__NEXT_DATA__">\n  ${JSON.stringify(nd(trackEntity))}\r\n\t</script>`),
    'closing tag inside a string': html(`<script id="__NEXT_DATA__">${JSON.stringify(nd({ ...trackEntity, name: 'a</script>b' }))}</script>`),
    'broken json': html('<script id="__NEXT_DATA__">{"props": </script>'), 'empty json': html('<script id="__NEXT_DATA__"></script>'),
    'trailing comma': html('<script id="__NEXT_DATA__">{"a":1,}</script>'), 'json zero': html('<script id="__NEXT_DATA__">0</script>'),
    'json null': html('<script id="__NEXT_DATA__">null</script>'), 'json array': html('<script id="__NEXT_DATA__">[]</script>'), 'json string': html('<script id="__NEXT_DATA__">"x"</script>'),
    'json number': html('<script id="__NEXT_DATA__">5</script>'), 'json false': html('<script id="__NEXT_DATA__">false</script>'),
    'entity only deep': page({ props: { pageProps: { x: { entity: trackEntity } } }, session: { accessToken: TOKEN } }),
    'entity at root': page({ entity: trackEntity }), 'entity in props': page({ props: { entity: trackEntity } }),
    'no entity': page(nd(undefined)), 'entity null': page(nd(null)), 'entity false': page(nd(false)), 'entity zero': page(nd(0)), 'entity empty string': page(nd('')),
    'entity empty array': page(nd([])), 'entity empty object': page(nd({})), 'entity string': page(nd('str')), 'entity number': page(nd(5)),
    'entity null with deep': page({ props: { pageProps: { state: { data: { entity: null } }, x: { entity: trackEntity } } } }),
    'deep order nested first': page({ a: { b: { entity: { ...trackEntity, name: 'Nested' } } }, c: { entity: { ...trackEntity, name: 'Sibling' } } }),
    'deep order own key first': page({ a: { entity: { ...trackEntity, name: 'Nested' } }, entity: { ...trackEntity, name: 'Own' } }),
    'deep through array': page({ list: [{ x: 1 }, { entity: trackEntity }] }),
    'known path beats deep': page({ x: { entity: { ...trackEntity, name: 'Deep' } }, props: { pageProps: { state: { data: { entity: trackEntity } } } } }),
    'token empty': page(nd(trackEntity, '')), 'token missing': page(nd(trackEntity, null)),
    'token number': page({ props: { pageProps: { state: { data: { entity: trackEntity, session: { accessToken: 12345 } } } } } }),
    'token object': page({ props: { pageProps: { state: { data: { entity: trackEntity, session: { accessToken: { a: 1 } } } } } } }),
    'token known empty deep set': page({ props: { pageProps: { state: { data: { entity: trackEntity, session: { accessToken: '' } } } } }, x: { accessToken: 'DEEP' } }),
    'token known null deep set': page({ props: { pageProps: { state: { data: { entity: trackEntity, session: { accessToken: null } } } } }, x: { accessToken: 'DEEP' } }),
    'token deep number first': page({ a: { accessToken: 5 }, b: { accessToken: 'later' }, entity: trackEntity }),
    'token in array': page({ list: [{ accessToken: 'IN_ARRAY' }], entity: trackEntity }),
    'token deep empty first': page({ a: { accessToken: '' }, b: { accessToken: 'later' }, entity: trackEntity }),
    'token unicode': page(nd(trackEntity, 'tokén😀')),
    'duplicate keys': html('<script id="__NEXT_DATA__">{"entity":{"name":"first"},"entity":{"name":"second","uri":"spotify:track:DUP"},"accessToken":"a","accessToken":"b"}</script>'),
  };
  for (const [name, res] of Object.entries(embedPages)) S(`embed page: ${name}`, [fetchTr('TR1'), tokenAt(), fetchTr('TR2')], site({ embed: () => res }));

  // Single track entities, by id.
  const trEnts = {
    full: trackEntity, bare: {}, nouri: { name: 'No Uri' }, uriOk: { uri: 'spotify:track:FROMURI', name: 'U' }, episode: { uri: 'spotify:episode:e', name: 'E' },
    local: { uri: 'spotify:local:x', name: 'L' }, uriEmpty: { uri: '', name: 'Empty' }, uriNull: { uri: null, name: 'Null' }, uriZero: { uri: 0, name: 'Zero' }, uriFalse: { uri: false, name: 'False' },
    uriNum: { uri: 5, name: 'Num' }, uriObj: { uri: {}, name: 'Obj' }, uriArr: { uri: [], name: 'Arr' }, uriTrue: { uri: true, name: 'True' }, uriPrefixOnly: { uri: 'spotify:track:' },
    uriUpper: { uri: 'SPOTIFY:TRACK:x' }, title: { title: 'Title' }, nameBeatsTitle: { name: 'N', title: 'T' }, nameNull: { name: null, title: 'T' }, nameEmpty: { name: '', title: 'T' },
    subtitle: { subtitle: 'Sub' }, subtitleNum: { subtitle: 5 }, subtitleObj: { subtitle: {} }, subtitleArr: { subtitle: ['a', null, 'b'] }, subtitleNested: { subtitle: [[1, 2], 3] }, subtitleTrue: { subtitle: true },
    subtitleFalse: { subtitle: false }, subtitleEmpty: { subtitle: '' }, subtitleZero: { subtitle: 0 }, subtitleFloat: { subtitle: 1.5 },
    artistsEmptyKeepsNoSub: { artists: [], subtitle: 'Sub' }, artistsEmptyStr: { artists: '', subtitle: 'Sub' }, artistsZero: { artists: 0, subtitle: 'Sub' }, artistsNull: { artists: null, subtitle: 'Sub' },
    artistsNone: { artists: [{}], subtitle: 'Sub' }, artistsStrs: { artists: ['A', 'B'] }, artistsItems: { artists: { items: [{ profile: { name: 'P' } }] } }, artistsStr: { artists: 'solo', subtitle: 'S' },
    artistsTrue: { artists: true, subtitle: 'S' }, artistsObj: { artists: {}, subtitle: 'S' },
    album: { album: { name: 'AL', date: { year: 1999 } } }, albumNull: { album: null }, albumStr: { album: 'str' }, albumNameNull: { album: { name: null } },
    relDate: { releaseDate: { isoString: '2001-02-03' }, album: { date: { year: 1999 } } }, relEmpty: { releaseDate: '', album: { date: { year: 1999 } } }, relNull: { releaseDate: null, album: { date: { year: 1999 } } },
    relZero: { releaseDate: 0, album: { date: { year: 1999 } } }, relObj: { releaseDate: {}, album: { date: { year: 1999 } } }, relStr: { releaseDate: '2003-04-05' }, relYear: { releaseDate: { year: 2004 } },
    num: { trackNumber: 4 }, numZero: { trackNumber: 0 }, numNull: { trackNumber: null },
    cover: { coverArt: { sources: [{ url: 'a' }, { url: 'b' }] } }, coverEmpty: { coverArt: {} },
    ...Object.fromEntries([
      ['dur int', 201000], ['dur zero', 0], ['dur str', '180000'], ['dur padded', ' 42 '], ['dur hex', '0x10'], ['dur bin', '0b11'], ['dur junk', '12abc'], ['dur empty', ''], ['dur blank', '  '],
      ['dur inf', 'Infinity'], ['dur neg', -5], ['dur negstr', '-5'], ['dur float', 1.5], ['dur exp', '1e3'], ['dur true', true], ['dur false', false], ['dur arr0', []], ['dur arr1', [5]], ['dur arr2', [1, 2]],
      ['dur arrstr', ['7']], ['dur arrnull', [null]], ['dur obj', {}], ['dur dot', '.5'], ['dur plus', '+9'], ['dur underscore', '1_000'], ['dur arabic', '١٢'], ['dur nbsp', ' 7 '],
    ].map(([k, v]) => [k.replace(' ', ''), { duration: v }])),
    durNullMs: { duration: null, duration_ms: 77 }, durMsOnly: { duration_ms: '88' }, durZeroMs: { duration: 0, duration_ms: 66 }, durBothNull: { duration: null, duration_ms: null }, durEmptyMs: { duration: '', duration_ms: 5 },
  };
  S('embed single track entities', Object.keys(trEnts).map((id) => fetchTr(id)), site({ embed: ({ id }) => page(nd(trEnts[id], null)) }));
  S('single track from a playlist url keeps the url id', [fetchTr('https://open.spotify.com/playlist/PL9'), fetchTr('spotify:album:AL9'), fetchTr('https://open.spotify.com/artist/AR9')], site({ embed: () => page(nd(trackEntity, null)) }));

  // Tier 3 collections by id.
  const trackListShapes = [
    null, 5, 'str', true, [], {}, { uri: 'spotify:episode:x' }, { uri: 'spotify:local:x' }, { uri: 'spotify:track:' }, { uri: '' }, { uri: 0 }, { uri: false }, { uri: null, title: 'NoUri' }, { name: 'NoUri' },
    { uri: 'spotify:track:t1', title: 'A', subtitle: 'S' }, { uri: 'spotify:track:t2', name: 'N', title: 'ignored', artists: [{ name: 'X' }, 'Y'] }, { uri: 'spotify:track:t3', artists: [], subtitle: 'S' },
    { uri: 'spotify:track:t4', artists: '', subtitle: 'S2' }, { uri: 'spotify:track:t5', subtitle: 5 }, { uri: 'spotify:track:t6', subtitle: {} }, { uri: 'spotify:track:t7', subtitle: ['a', 'b'] },
    { uri: 'spotify:track:t8', duration: '180000' }, { uri: 'spotify:track:t9', duration_ms: '123' }, { uri: 'spotify:track:t10', duration: 0, duration_ms: 5 }, { uri: 'spotify:track:t11', duration: 'x', duration_ms: 5 },
    { uri: 'spotify:track:t12', coverArt: { sources: [{ url: 'own.jpg' }] } }, { uri: 'spotify:track:t13', coverArt: { sources: [] } }, { uri: 'spotify:track:t14', coverArt: null },
    { uri: 'spotify:track:t15', releaseDate: { isoString: '2001-02-03' } }, { uri: 'spotify:track:t16', album: { name: 'AL', date: { year: 2002 } } }, { uri: 'spotify:track:t17', releaseDate: '', album: { date: { year: 2002 } } },
    { uri: 'spotify:track:t18', releaseDate: null, album: { date: { year: 2002 } } }, { uri: 'spotify:track:t19', trackNumber: 3 }, { uri: 'spotify:track:t20', title: '日本😀', subtitle: 'É' },
  ];
  const tier3 = {
    shapes: { ...playlistEntity, trackList: trackListShapes },
    notarray: { ...playlistEntity, trackList: {} }, strlist: { ...playlistEntity, trackList: 'abc' }, nolist: { ...playlistEntity, trackList: undefined }, nulllist: { ...playlistEntity, trackList: null },
    emptylist: { ...playlistEntity, trackList: [] },
    throwuri: { ...playlistEntity, trackList: [{ uri: 'spotify:track:ok1' }, { uri: 7 }] },
    throwobj: { ...playlistEntity, trackList: [{ uri: {} }] }, throwtrue: { ...playlistEntity, trackList: [{ uri: true }] },
    cover: { ...playlistEntity, coverArt: undefined }, covernull: { ...playlistEntity, coverArt: null }, coverbare: { ...playlistEntity, coverArt: [{ url: 'x' }, { url: 'bare.jpg' }] },
    named: { trackList: playlistEntity.trackList }, title: { title: 'Only Title', trackList: [] }, nameVsTitle: { name: 'N', title: 'T' }, nameNull: { name: null, title: 'T' }, nameEmpty: { name: '', title: 'T' },
    sub: { subtitle: 'Sub' }, subNum: { subtitle: 5 }, subEmpty: { subtitle: '' }, subNull: { subtitle: null }, subObj: { subtitle: { a: 1 } }, subArr: { subtitle: ['a', 'b'] },
    ownerNone: { name: 'n' }, ownerEmpty: { owner: { name: '' } }, ownerNull: { owner: null }, ownerStr: { owner: 'str' },
    arr: [], str: 'str', num: 5, zero: 0, empty: '', false: false, nul: null, undef: undefined,
  };
  const t3Steps = [];
  const t3Ents = {};
  for (const [k, v] of Object.entries(tier3)) {
    t3Ents[`pl${k}`] = v; t3Steps.push(fetchPl(`pl${k}`));
    t3Ents[`al${k}`] = Array.isArray(v) || typeof v !== 'object' || v === null ? v : { ...v, artists: v.artists ?? [{ name: 'Embed Band' }, { name: 'Second' }] };
    t3Steps.push(fetchAl(`al${k}`));
  }
  for (const [k, artists] of Object.entries({ none: undefined, empty: [], strs: ['Str', 'Two'], items: { items: [{ profile: { name: 'Prof' } }] }, blanks: [{ name: '' }, { name: 'Second' }], str: 'solo', obj: {} })) {
    t3Ents[`alo${k}`] = { name: 'Alb', artists }; t3Steps.push(fetchAl(`alo${k}`));
  }
  S('embed collections', t3Steps, site({ embed: ({ id }) => page(nd(t3Ents[id], null)) }));
  // The same entities with a token whose graphql then fails, so the embed list is the fallback.
  S('embed collections after a failed graphql', [fetchPl('PL1'), fetchAl('AL1')], site({
    embed: ({ type }) => page(nd(type === 'playlist' ? playlistEntity : albumEntity)), gql: () => json({ errors: [{}] }),
  }));

  // ── Graphql responses and the self-heal ─────────────────────────────────────
  const plOne = plBody([plItem(1)], 1);
  const gqlVariants = {
    'errors': json({ errors: [{ message: 'x' }] }), 'errors empty array': json({ errors: [], ...plOne }), 'errors string': json({ errors: 'x', ...plOne }),
    'errors object': json({ errors: {}, ...plOne }), 'errors null': json({ errors: null, ...plOne }), 'errors zero': json({ errors: 0, ...plOne }), 'errors false': json({ errors: false, ...plOne }),
    'errors empty string': json({ errors: '', ...plOne }), 'errors with data': json({ errors: [{}], ...plOne }),
    'body null': text('null'), 'body empty': text(''), 'body not json': text('<html>502</html>'), 'body zero': text('0'), 'body false': text('false'), 'body empty string': text('""'),
    'body empty array': text('[]'), 'body string': text('"x"'), 'body number': text('5'), 'body empty object': text('{}'), 'no data': json({ extensions: {} }), 'data null': json({ data: null }),
    'playlist null': json({ data: { playlistV2: null } }), 'playlist string': json({ data: { playlistV2: 'x' } }), 'playlist zero': json({ data: { playlistV2: 0 } }), 'playlist empty object': json({ data: { playlistV2: {} } }),
    'content null': json({ data: { playlistV2: { name: 'X', content: null } } }), 'items empty': json({ data: { playlistV2: { name: 'X', content: { items: [], totalCount: 5 } } } }),
    'items null': json({ data: { playlistV2: { name: 'X', content: { items: null } } } }), 'items object': json({ data: { playlistV2: { name: 'X', content: { items: { length: 1 } } } } }),
    'items string': json({ data: { playlistV2: { name: 'X', content: { items: 'abc' } } } }),
    'wrong kind': json({ data: { albumUnion: { name: 'Wrong', tracksV2: { items: [alItem(1)], totalCount: 1 } } } }),
    'ok': json(plOne),
  };
  // Heal impossible: the home page cannot be fetched, so the failed response decides.
  for (const [name, res] of Object.entries(gqlVariants)) {
    S(`graphql response: ${name}`, [fetchPl('PL1'), state()], site({ embed: () => page(nd(playlistEntity)), gql: () => res }));
  }
  // The same first response, then a heal that works and a good retry.
  for (const [name, res] of Object.entries(gqlVariants)) {
    S(`graphql response then heal: ${name}`, [fetchPl('PL1'), state()], site({
      embed: () => page(nd(playlistEntity)), gql: ({ n }) => (n === 1 ? res : json(plOne)), home: () => scripts('https://cdn/a.js'), other: () => bundleWith('fetchPlaylist'),
    }));
  }
  S('heal works but the retry fails again', [fetchPl('PL1'), state(), fetchPl('PL1', T0 + 2 * MIN), state()], site({
    embed: () => page(nd(playlistEntity)), gql: () => json({ errors: [{}] }), home: () => scripts('https://cdn/a.js'), other: () => bundleWith('fetchPlaylist'),
  }));
  S('healed hash is used first afterwards', [fetchPl('PL1'), fetchPl('PL1', T0 + 1), fetchAl('AL1', T0 + 2), state()], site({
    embed: ({ type }) => page(nd(type === 'playlist' ? playlistEntity : albumEntity)),
    gql: ({ op, hash }) => (op === 'fetchPlaylist' ? (hash === HASH ? json(plOne) : json({ errors: [{}] })) : json(alBody([alItem(1)], 1))),
    home: () => scripts('https://cdn/a.js'), other: () => bundleWith('fetchPlaylist'),
  }));
  S('healed hash goes stale again and the cooldown holds', [fetchPl('PL1', T0), fetchPl('PL1', T0 + MIN - 1), fetchPl('PL1', T0 + MIN), state()], site({
    embed: () => page(nd(playlistEntity)), gql: () => json({ errors: [{}] }), home: () => scripts('https://cdn/a.js'), other: () => bundleWith('fetchPlaylist', HASH2),
  }));
  S('heal fails then cooldown then retried', [fetchPl('PL1', T0), fetchPl('PL1', T0 + 10 * 1000), fetchAl('AL1', T0 + 20 * 1000), fetchPl('PL1', T0 + MIN), state()], site({
    embed: ({ type }) => page(nd(type === 'playlist' ? playlistEntity : albumEntity)), gql: () => json({ errors: [{}] }),
  }));
  S('heal clock after a slow heal', [fetchPl('PL1', T0), fetchPl('PL1', T0 + 70 * 1000), state()], site({
    embed: () => page(nd(playlistEntity)), gql: () => json({ errors: [{}] }), home: () => text(scripts('https://cdn/a.js', 'https://cdn/b.js').text, { advance: 50 * 1000 }),
    other: () => text('nothing', { advance: 5000 }),
  }));

  // ── The heal on its own: home page scripts and bundle text ──────────────────
  const padding = (n) => 'x'.repeat(n);
  const OP = 'fetchPlaylist';
  const healCases = {
    basic: healSite({ a: bundleWith(OP) }),
    'second bundle has it': healSite({ a: text('nothing here'), b: bundleWith(OP) }),
    'first hit wins': healSite({ a: bundleWith(OP, HASH), b: bundleWith(OP, HASH2) }),
    'first bundle throws': healSite({ b: bundleWith(OP) }, ['a', 'b']),
    'all bundles throw': healSite({}, ['a', 'b']),
    'no bundle has the op': healSite({ a: text('nothing'), b: text('also nothing') }),
    'op twice, hash only after the second': healSite({ a: text(`${OP}${padding(500)} ${OP}"${HASH}"`) }),
    'op twice, hash after the first': healSite({ a: text(`${OP}"${HASH}" ${OP}"${HASH2}"`) }),
    'op twice, second bundle rescues': healSite({ a: text(`${OP}${padding(500)}${OP}"${HASH}"`), b: bundleWith(OP, HASH2) }),
    'hash at the edge 323': healSite({ a: text(`${OP}${padding(323)}${HASH}`) }),
    'hash over the edge 324': healSite({ a: text(`${OP}${padding(324)}${HASH}`) }),
    'hash over the edge 330': healSite({ a: text(`${OP}${padding(330)}${HASH}`) }),
    'hash starts at 399': healSite({ a: text(`${OP}${padding(386)}${HASH}`) }),
    'hash far away': healSite({ a: text(`${OP}${padding(1000)}${HASH}`) }),
    'hash before the op': healSite({ a: text(`"${HASH}" ${OP} nothing after`) }),
    'op at the end': healSite({ a: text(`${HASH} ${OP}`) }),
    'uppercase hash': healSite({ a: bundleWith(OP, HASH.toUpperCase()) }),
    'mixed case hash': healSite({ a: bundleWith(OP, 'AbCdEf0123456789'.repeat(4)) }),
    '65 hex digits': healSite({ a: bundleWith(OP, HASH + 'a') }),
    '63 hex digits': healSite({ a: bundleWith(OP, HASH.slice(1)) }),
    '63 hex then non hex': healSite({ a: bundleWith(OP, HASH.slice(1) + 'g') }),
    'non hex in the middle': healSite({ a: bundleWith(OP, HASH.slice(0, 30) + 'g' + HASH.slice(31)) }),
    'two short runs': healSite({ a: bundleWith(OP, HASH.slice(0, 32) + '-' + HASH.slice(32, 64)) }),
    'short run then full run': healSite({ a: bundleWith(OP, 'abc' + '-' + HASH2) }),
    'hash after a newline': healSite({ a: text(`${OP}\n\r\n${HASH}`) }),
    'fullwidth digits': healSite({ a: bundleWith(OP, 'ａ'.repeat(64)) }),
    'after 150 astral characters': healSite({ a: text(`${OP}${'😀'.repeat(150)}${HASH}`) }),
    'after 190 astral characters': healSite({ a: text(`${OP}${'😀'.repeat(190)}${HASH}`) }),
    'after 193 astral characters': healSite({ a: text(`${OP}${'😀'.repeat(193)}${HASH}`) }),
    'after 300 two byte characters': healSite({ a: text(`${OP}${'п'.repeat(300)}${HASH}`) }),
    'after 330 two byte characters': healSite({ a: text(`${OP}${'п'.repeat(330)}${HASH}`) }),
    'astral before the op': healSite({ a: text(`${'😀'.repeat(100)}${OP}"${HASH}"`) }),
    'op inside a longer word': healSite({ a: text(`xx${OP}V2:"${HASH}"`) }),
    'other op only': healSite({ a: bundleWith('getAlbum') }),
    'op case differs': healSite({ a: bundleWith('FETCHPLAYLIST') }),
    'empty bundle': healSite({ a: text('') }),
    'home without scripts': site({ home: () => html('<p>none</p>') }),
    'home empty': site({ home: () => text('') }),
    'home throws': site({}),
    'home with sources of every kind': site({
      home: () => html([
        '<script src="https://cdn/x.mjs"></script>', '<script src="https://cdn/y.js?v=1"></script>', "<script src='https://cdn/z.js'></script>", '<script>var src="https://cdn/inline.js";</script>',
        '<script src="https://cdn/ok1.js"></script>', '<SCRIPT src="https://cdn/upper.js"></SCRIPT>', '<script defer type="module" crossorigin src="https://cdn/ok2.js"></script>',
        '<script src="https://cdn/first.js" async src="https://cdn/last.js"></script>', '<scripts src="https://cdn/odd.js">', '<link src="https://cdn/link.js">', '<script src=""></script>', '<script src=".js"></script>',
        '<script\nsrc="https://cdn/newline.js"></script>', '<script src="https://cdn/a b.js"></script>', '<script src="https://cdn/é.js"></script>', '<script src="https://cdn/dup.js"></script><script src="https://cdn/dup.js"></script>',
      ].join('\n')),
      other: (url) => (url.endsWith('/ok2.js') ? bundleWith(OP) : text('no')),
    }),
    'relative sources are fetched as given': site({ home: () => scripts('/static/a.js', '//cdn/b.js', 'a.js', 'https://cdn/c.js'), other: (url) => (url === 'https://cdn/c.js' ? bundleWith(OP) : undefined) }),
    'only the first eight bundles': healSite(Object.fromEntries(Array.from({ length: 10 }, (_, i) => [`b${i}`, i === 8 ? bundleWith(OP) : text('no')]))),
    'the eighth bundle counts': healSite(Object.fromEntries(Array.from({ length: 10 }, (_, i) => [`b${i}`, i === 7 ? bundleWith(OP) : text('no')]))),
    'every bundle is requested even after a hit': healSite(Object.fromEntries(Array.from({ length: 5 }, (_, i) => [`b${i}`, bundleWith(OP, i === 0 ? HASH : HASH2)]))),
    'home page body is large': site({ home: () => html(`${padding(100000)}<script src="https://cdn/a.js"></script>`), other: () => bundleWith(OP) }),
  };
  for (const [name, route] of Object.entries(healCases)) {
    S(`heal: ${name}`, [heal(OP), state(), heal('getAlbum', T0 + 1), state()], route);
  }
  S('heal cooldown boundaries', [
    heal(OP, T0), heal(OP, T0 + 1), heal(OP, T0 + MIN - 1), state(), heal(OP, T0 + MIN), heal(OP, T0 + MIN + 1), heal('getAlbum', T0 + MIN + 2), state(),
  ], site());
  S('heal cooldown after a success', [heal(OP, T0), heal(OP, T0 + MIN - 1), heal(OP, T0 + MIN), state()], site({ home: () => scripts('https://cdn/a.js'), other: () => bundleWith(OP) }));
  S('heal with a clock under a minute', [heal(OP, 1000), state(), heal(OP, 59_999), heal(OP, 60_000), state()], site({ home: () => scripts('https://cdn/a.js'), other: () => bundleWith(OP) }));
  S('heal with a clock at zero', [heal(OP, 0), heal(OP, 1)], site({ home: () => scripts('https://cdn/a.js'), other: () => bundleWith(OP) }));
  S('heal unknown operation', [heal('somethingElse', T0), state()], site({ home: () => scripts('https://cdn/a.js'), other: () => bundleWith('somethingElse') }));
  S('initial state', [state(), tokenAt(), state()], site());

  // ── Run ─────────────────────────────────────────────────────────────────────
  const exec = (h, step) => {
    switch (step.op) {
      case 'parseUrl': return h.parseUrl(step.arg);
      case 'resolveId': return h.resolveId(step.arg);
      case 'token': return h.getToken();
      case 'seedToken': h.token = { value: step.value, expiresAt: step.expiresAt }; return null;
      case 'heal': return h.healHashes(step.arg);
      case 'state': return { hashes: h.hashes, lastHealAt: h.lastHealAt, token: h.token ?? null };
      default: return h[step.op](step.arg);
    }
  };
  const golden = [];
  try {
    for (const s of scenarios) {
      const h = new SpotifyHandler();
      router = s.route;
      clock = T0;
      const steps = [];
      for (const step of s.steps) {
        calls = [];
        if (step.at !== undefined) clock = step.at;
        const rec = { ...step };
        try { rec.result = wire(await exec(h, step)); } catch (e) { rec.throws = true; }
        rec.calls = calls;
        steps.push(rec);
      }
      golden.push({ name: s.name, steps });
    }
  } finally {
    Date.now = RealNow;
    globalThis.fetch = realFetch;
  }
  if (bugs.length) throw new Error(`spotify.mjs: a route threw\n${bugs[0]}`);
  writeFileSync(`${OUT}/spotify.json`, JSON.stringify(golden) + '\n');
  const stepCount = golden.reduce((n, s) => n + s.steps.length, 0);
  const requests = golden.reduce((n, s) => n + s.steps.reduce((m, st) => m + st.calls.length, 0), 0);
  console.log(`spotify.json: ${golden.length} scenarios, ${stepCount} steps, ${requests} requests`);
};
