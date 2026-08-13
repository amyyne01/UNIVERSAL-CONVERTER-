import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { SpotifyHandler } from './spotify';

// ── Fixtures ──────────────────────────────────────────────────────────────
const TOKEN = 'BQ_anon_token_abc';

function res(body: unknown, isText = false) {
  const text = typeof body === 'string' ? body : JSON.stringify(body);
  return {
    ok: true,
    status: 200,
    text: async () => text,
    json: async () => (isText ? JSON.parse(text) : body),
  } as Response;
}

/** Wrap NEXT_DATA in the embed page's <script id="__NEXT_DATA__"> the scraper looks for. */
function embedPage(nextData: unknown): Response {
  return res(`<html><body><script id="__NEXT_DATA__" type="application/json">${JSON.stringify(nextData)}</script></body></html>`);
}

function nextData(entity: unknown, withToken = true): unknown {
  const data: any = { entity };
  if (withToken) data.session = { accessToken: TOKEN, accessTokenExpirationTimestampMs: Date.now() + 3_600_000 };
  return { props: { pageProps: { state: { data } } } };
}

const trackEntity = {
  type: 'track',
  uri: 'spotify:track:TR1',
  name: 'Single Song',
  artists: [{ name: 'Solo Artist' }, { name: 'Feature' }],
  album: { name: 'The Album', date: { year: 2019 } },
  coverArt: { sources: [{ url: 'small.jpg', width: 64 }, { url: 'big.jpg', width: 640 }] },
  duration: 201000,
  releaseDate: { isoString: '2019-05-01T00:00:00Z' },
};

const playlistEntity = {
  type: 'playlist',
  uri: 'spotify:playlist:PL1',
  name: 'My Playlist',
  subtitle: 'a fine mix',
  owner: { name: 'DJ Embed' },
  coverArt: { sources: [{ url: 'plcover.jpg' }] },
  trackList: [
    { uri: 'spotify:track:e1', title: 'Embed Song 1', subtitle: 'Embed Artist 1', duration: 111000 },
    { uri: 'spotify:track:e2', title: 'Embed Song 2', subtitle: 'Embed Artist 2', duration: 222000 },
  ],
};

function gqlPlaylistItem(i: number) {
  return {
    itemV2: {
      data: {
        uri: `spotify:track:g${i}`,
        name: `GQL Song ${i}`,
        artists: { items: [{ profile: { name: `GQL Artist ${i}` } }] },
        albumOfTrack: { name: 'GQL Album', coverArt: { sources: [{ url: 'gqlcover.jpg' }] }, date: { year: 2021 } },
        trackDuration: { totalMilliseconds: 180000 + i },
        trackNumber: i,
      },
    },
  };
}

function gqlAlbumItem(i: number) {
  return {
    track: {
      uri: `spotify:track:a${i}`,
      name: `Album Track ${i}`,
      artists: { items: [{ profile: { name: `Track Artist ${i}` } }] },
      trackNumber: i + 1,
      duration: { totalMilliseconds: 200000 + i },
    },
  };
}

function albumPage(items: unknown[], total: number) {
  return {
    data: {
      albumUnion: {
        name: 'Greatest Hits',
        artists: { items: [{ profile: { name: 'The Band' } }] },
        coverArt: { sources: [{ url: 'albumcover.jpg' }] },
        date: { year: 2020 },
        tracksV2: { totalCount: total, items },
      },
    },
  };
}

/** Install a fetch stub that routes by URL; returns the spy. */
function stubFetch(router: (url: string, init?: RequestInit) => Response): ReturnType<typeof vi.fn> {
  const fn = vi.fn(async (url: string, init?: RequestInit) => router(url, init));
  vi.stubGlobal('fetch', fn);
  return fn;
}

const calls = (fn: ReturnType<typeof vi.fn>, sub: string) =>
  fn.mock.calls.filter((c) => String(c[0]).includes(sub));

beforeEach(() => vi.clearAllMocks());
afterEach(() => vi.unstubAllGlobals());

// ── (a) Tier 1: embed __NEXT_DATA__ carries token + entity ───────────────────
describe('Tier 1 — single track embed', () => {
  it('maps the embed entity into a Track', async () => {
    stubFetch((url) => {
      if (url.includes('/embed/track/TR1')) return embedPage(nextData(trackEntity));
      throw new Error(`unexpected fetch ${url}`);
    });
    const track = await new SpotifyHandler().fetchTrack('https://open.spotify.com/track/TR1');
    expect(track).toEqual({
      id: 'TR1',
      name: 'Single Song',
      artist: 'Solo Artist',
      artists: ['Solo Artist', 'Feature'],
      album: 'The Album',
      albumYear: '2019',
      trackNumber: 0,
      durationMs: 201000,
      thumbnailUrl: 'big.jpg', // largest source picked
      spotifyUrl: 'https://open.spotify.com/track/TR1',
    });
  });

  it('falls back to a recursive deep-scan when the token is off the known path', async () => {
    const buried = { props: { pageProps: { somewhere: { deep: { accessToken: TOKEN } } } }, entity: trackEntity };
    stubFetch(() => embedPage(buried));
    const h = new SpotifyHandler();
    await h.fetchTrack('TR1');
    expect((h as any).getToken()).toBe(TOKEN); // deep-scan recovered + cached the token
  });
});

// ── (b) Tier 2: GraphQL tracklist + token caching/reuse ──────────────────────
describe('Tier 2 — playlist via GraphQL', () => {
  it('maps GraphQL tracks (not the embed list) and reuses the cached token', async () => {
    const fetchFn = stubFetch((url) => {
      if (url.includes('/embed/playlist/PL1')) return embedPage(nextData(playlistEntity));
      if (url.includes('api-partner')) {
        return res({ data: { playlistV2: {
          name: 'GQL Playlist Name',
          description: 'real description',
          ownerV2: { data: { name: 'Real Owner' } },
          images: { items: [{ sources: [{ url: 'gqlplcover.jpg' }] }] },
          content: { totalCount: 2, items: [gqlPlaylistItem(1), gqlPlaylistItem(2)] },
        } } });
      }
      throw new Error(`unexpected fetch ${url}`);
    });

    const pl = await new SpotifyHandler().fetchPlaylist('spotify:playlist:PL1');
    expect(pl!.name).toBe('GQL Playlist Name'); // GraphQL metadata wins over embed entity
    expect(pl!.owner).toBe('Real Owner');
    expect(pl!.thumbnailUrl).toBe('gqlplcover.jpg');
    expect(pl!.trackCount).toBe(2);
    expect(pl!.tracks.map((t) => t.name)).toEqual(['GQL Song 1', 'GQL Song 2']); // Tier 2, not Tier 3
    expect(pl!.tracks[0]).toMatchObject({
      id: 'g1', artist: 'GQL Artist 1', album: 'GQL Album', albumYear: '2021',
      durationMs: 180001, trackNumber: 1, thumbnailUrl: 'gqlcover.jpg',
      spotifyUrl: 'https://open.spotify.com/track/g1',
    });

    // Token minted once from the embed, reused on every GraphQL call.
    expect(calls(fetchFn, '/embed/').length).toBe(1);
    const gql = calls(fetchFn, 'api-partner');
    expect(gql.length).toBe(1);
    expect((gql[0][1] as RequestInit).headers).toMatchObject({ authorization: `Bearer ${TOKEN}` });
  });

  it('paginates an album (50/page) and maps it into the Playlist shape', async () => {
    const fetchFn = stubFetch((url) => {
      if (url.includes('/embed/album/AL1')) return embedPage(nextData({ type: 'album', uri: 'spotify:album:AL1', name: 'Greatest Hits' }));
      if (url.includes('api-partner')) {
        const offset = JSON.parse(new URL(url).searchParams.get('variables')!).offset;
        const items = offset === 0
          ? Array.from({ length: 50 }, (_, i) => gqlAlbumItem(i))
          : Array.from({ length: 10 }, (_, i) => gqlAlbumItem(50 + i));
        return res(albumPage(items, 60));
      }
      throw new Error(`unexpected fetch ${url}`);
    });

    const album = await new SpotifyHandler().fetchAlbum('AL1');
    expect(album!.tracks).toHaveLength(60); // 50 + 10 across two pages
    expect(album!.trackCount).toBe(60);
    expect(album!.name).toBe('Greatest Hits');
    expect(album!.owner).toBe('The Band'); // album artist → owner
    expect(album!.url).toBe('https://open.spotify.com/album/AL1');
    expect(album!.tracks[0]).toMatchObject({
      id: 'a0', name: 'Album Track 0', artist: 'Track Artist 0', album: 'Greatest Hits',
      albumYear: '2020', trackNumber: 1, durationMs: 200000, thumbnailUrl: 'albumcover.jpg',
    });
    expect(calls(fetchFn, 'api-partner').length).toBe(2); // two pages fetched
    expect(calls(fetchFn, '/embed/').length).toBe(1); // token reused across both pages
  });

  it('B5: trackCount reflects only the tracks actually fetched when a later page fails', async () => {
    const fetchFn = stubFetch((url) => {
      if (url.includes('/embed/album/AL1')) return embedPage(nextData({ type: 'album', uri: 'spotify:album:AL1', name: 'Greatest Hits' }));
      if (url.includes('api-partner')) {
        const offset = JSON.parse(new URL(url).searchParams.get('variables')!).offset;
        if (offset === 0) return res(albumPage(Array.from({ length: 50 }, (_, i) => gqlAlbumItem(i)), 120));
        throw new Error('network down'); // page 2 (offset 50) fails
      }
      throw new Error(`unexpected fetch ${url}`); // home/bundle self-heal attempts also fail
    });

    const album = await new SpotifyHandler().fetchAlbum('AL1');
    expect(album!.tracks).toHaveLength(50); // only page 1 was ever fetched
    expect(album!.trackCount).toBe(50); // must not carry forward the un-fetched total (120)
    void fetchFn;
  });
});

// ── (c) Stale persisted-query hash → self-heal from the web-player bundle ─────
describe('Tier 2 — stale hash self-heal', () => {
  it('refetches the hash from the bundle and retries on PersistedQueryNotFound', async () => {
    let gqlCount = 0;
    const fetchFn = stubFetch((url) => {
      if (url.includes('/embed/playlist/PL1')) return embedPage(nextData(playlistEntity));
      if (url.includes('api-partner')) {
        gqlCount += 1;
        if (gqlCount === 1) return res({ errors: [{ message: 'PersistedQueryNotFound' }] });
        return res({ data: { playlistV2: { name: 'Healed', content: { totalCount: 1, items: [gqlPlaylistItem(1)] } } } });
      }
      if (url === 'https://open.spotify.com/') return res('<script src="https://open.spotifycdn.com/web-player.xyz.js"></script>');
      if (url.includes('web-player.xyz.js')) return res('...fetchPlaylist...,"sha256Hash":"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"...');
      throw new Error(`unexpected fetch ${url}`);
    });

    const pl = await new SpotifyHandler().fetchPlaylist('PL1');
    expect(pl!.tracks.map((t) => t.name)).toEqual(['GQL Song 1']); // recovered via Tier 2 after heal
    expect(gqlCount).toBe(2); // stale call + retry with fresh hash
    expect(calls(fetchFn, 'open.spotify.com/').length).toBeGreaterThan(0); // homepage scanned
    expect(calls(fetchFn, 'web-player.xyz.js').length).toBe(1); // bundle scanned
  });

  it('B11: concurrent healHashes calls for the same op share one in-flight attempt instead of degrading', async () => {
    let homeFetches = 0;
    const fetchFn = stubFetch((url) => {
      if (url === 'https://open.spotify.com/') {
        homeFetches += 1;
        return res('<script src="https://open.spotifycdn.com/web-player.xyz.js"></script>');
      }
      if (url.includes('web-player.xyz.js')) {
        return res('...fetchPlaylist...,"sha256Hash":"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"...');
      }
      throw new Error(`unexpected fetch ${url}`);
    });

    const h = new SpotifyHandler() as any;
    const [a, b] = await Promise.all([h.healHashes('fetchPlaylist'), h.healHashes('fetchPlaylist')]);
    expect(a).toBe('abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789');
    expect(b).toBe(a); // second concurrent caller awaits the same attempt, doesn't get null
    expect(homeFetches).toBe(1); // only one shared heal attempt made
    void fetchFn;
  });
});

// ── (d) Tier 3: token missing → condensed embed list ─────────────────────────
describe('Tier 3 — embed fallback when the token is missing', () => {
  it('uses the condensed embed trackList and never calls GraphQL', async () => {
    const fetchFn = stubFetch((url) => {
      if (url.includes('/embed/playlist/PL1')) return embedPage(nextData(playlistEntity, /*withToken*/ false));
      throw new Error(`unexpected fetch ${url}`);
    });

    const pl = await new SpotifyHandler().fetchPlaylist('PL1');
    expect(pl!.name).toBe('My Playlist');
    expect(pl!.owner).toBe('DJ Embed');
    expect(pl!.thumbnailUrl).toBe('plcover.jpg');
    expect(pl!.trackCount).toBe(2);
    expect(pl!.tracks).toEqual([
      { id: 'e1', name: 'Embed Song 1', artist: 'Embed Artist 1', artists: ['Embed Artist 1'], album: '', trackNumber: 0, durationMs: 111000, thumbnailUrl: 'plcover.jpg', spotifyUrl: 'https://open.spotify.com/track/e1' },
      { id: 'e2', name: 'Embed Song 2', artist: 'Embed Artist 2', artists: ['Embed Artist 2'], album: '', trackNumber: 0, durationMs: 222000, thumbnailUrl: 'plcover.jpg', spotifyUrl: 'https://open.spotify.com/track/e2' },
    ]);
    expect(calls(fetchFn, 'api-partner').length).toBe(0); // no token → no GraphQL
  });
});

// ── Misc: token cache expiry, graceful failures, URL delegation ──────────────
describe('SpotifyHandler — token cache & graceful behaviour', () => {
  it('caches the token and expires it', () => {
    const h = new SpotifyHandler() as any;
    h.token = { value: 'X', expiresAt: Date.now() + 1000 };
    expect(h.getToken()).toBe('X');
    h.token = { value: 'X', expiresAt: Date.now() - 1 };
    expect(h.getToken()).toBeNull();
  });

  it('search returns [] when no token can be resolved', async () => {
    stubFetch(() => { throw new Error('no network'); });
    expect(await new SpotifyHandler().search('daft punk')).toEqual([]);
  });

  it('returns null for an unrecognisable id/url', async () => {
    stubFetch(() => res(''));
    expect(await new SpotifyHandler().fetchTrack('!!')).toBeNull();
  });

  it('parseUrl delegates to the shared url-detector', () => {
    expect(new SpotifyHandler().parseUrl('spotify:album:AL1')).toEqual({ type: 'album', id: 'AL1' });
  });
});

// ── (e) search() primes its own token on a fresh session ─────────────────────
describe('search — token priming', () => {
  it('mints a token via the embed page before querying GraphQL when none is cached yet', async () => {
    const fetchFn = stubFetch((url) => {
      if (url.includes('/embed/track/')) return embedPage(nextData(trackEntity));
      if (url.includes('api-partner')) {
        return res({ data: { searchV2: { tracksV2: { items: [] } } } });
      }
      throw new Error(`unexpected fetch ${url}`);
    });

    const tracks = await new SpotifyHandler().search('daft punk');
    expect(tracks).toEqual([]);
    expect(calls(fetchFn, '/embed/track/').length).toBe(1); // bootstrap embed hit to mint a token
    const gql = calls(fetchFn, 'api-partner');
    expect(gql.length).toBe(1);
    expect((gql[0][1] as RequestInit).headers).toMatchObject({ authorization: `Bearer ${TOKEN}` });
  });

  it('self-heals when the placeholder searchDesktop hash is stale', async () => {
    let gqlCount = 0;
    const fetchFn = stubFetch((url) => {
      if (url.includes('/embed/track/')) return embedPage(nextData(trackEntity));
      if (url.includes('api-partner')) {
        gqlCount += 1;
        if (gqlCount === 1) return res({ errors: [{ message: 'PersistedQueryNotFound' }] });
        return res({ data: { searchV2: { tracksV2: { items: [] } } } });
      }
      if (url === 'https://open.spotify.com/') return res('<script src="https://open.spotifycdn.com/web-player.xyz.js"></script>');
      if (url.includes('web-player.xyz.js')) return res('...searchDesktop...,"sha256Hash":"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"...');
      throw new Error(`unexpected fetch ${url}`);
    });

    const tracks = await new SpotifyHandler().search('daft punk');
    expect(tracks).toEqual([]);
    expect(gqlCount).toBe(2); // stale placeholder hash + retry with the healed one
    void fetchFn;
  });
});

// ── (f) fetch timeouts: every network call is abortable, nothing hangs ───────
describe('fetch abort-safety', () => {
  it('passes an AbortSignal on every fetch call', async () => {
    const fetchFn = stubFetch((url) => {
      if (url.includes('/embed/track/TR1')) return embedPage(nextData(trackEntity));
      throw new Error(`unexpected fetch ${url}`);
    });
    await new SpotifyHandler().fetchTrack('TR1');
    for (const call of fetchFn.mock.calls) {
      expect((call[1] as RequestInit)?.signal).toBeInstanceOf(AbortSignal);
    }
  });

  it('fetchTrack resolves to null (does not hang) when the embed fetch aborts', async () => {
    stubFetch(() => { throw new DOMException('The operation was aborted.', 'AbortError'); });
    await expect(new SpotifyHandler().fetchTrack('TR1')).resolves.toBeNull();
  });

  it('search resolves to [] (does not hang) when every fetch aborts', async () => {
    stubFetch(() => { throw new DOMException('The operation was aborted.', 'AbortError'); });
    await expect(new SpotifyHandler().search('daft punk')).resolves.toEqual([]);
  });

  it('fetchPlaylist resolves to null (does not hang) when every fetch aborts', async () => {
    stubFetch(() => { throw new DOMException('The operation was aborted.', 'AbortError'); });
    await expect(new SpotifyHandler().fetchPlaylist('PL1')).resolves.toBeNull();
  });
});
