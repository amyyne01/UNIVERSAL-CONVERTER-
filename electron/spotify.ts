// Credential-free 3-tier Spotify metadata pipeline. See HOW-THE-APP-WORKS.md §3.2.
//   Tier 1: scrape an anonymous access token (+ entity) from the public embed page's
//           __NEXT_DATA__ block; cache the token ~25 min and reuse it.
//   Tier 2: with the token, page through Spotify's internal api-partner persisted-query
//           GraphQL for the full track list; self-heal stale query hashes from the live
//           web-player bundle (behind a cooldown so an outage can't cause a retry storm).
//   Tier 3: fall back to the condensed track list already embedded in the page.
// Audio itself is bridged to yt-dlp elsewhere (§4); this module is metadata only.
import { parseSpotifyUrl } from './url-detector.js';
import type { Track, Playlist } from '../shared/types.js';

const EMBED = 'https://open.spotify.com/embed';
const GQL_URL = 'https://api-partner.spotify.com/pathfinder/v1/query';
const HOME = 'https://open.spotify.com/';
const UA =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36';

const TOKEN_TTL_MS = 25 * 60 * 1000; // §3.2: cache the anonymous token ~25 min.
const HEAL_COOLDOWN_MS = 60 * 1000; // §3.2: short cooldown so an outage can't storm self-heal.
const FETCH_TIMEOUT_MS = 10_000; // bound every scraper fetch so a stalled socket can't hang the IPC handler forever.
const MAX_BUNDLES = 8; // bound how many web-player chunks we scan when self-healing.
const HEAL_TIMEOUT_MS = 15_000; // aggregate budget for the whole self-heal (homepage + all bundle fetches in parallel).

// A stable, well-known public track — only ever used to mint an anonymous embed-page session
// token when search() has none cached yet. ponytail: fabricated searchDesktop hash below relies
// on self-heal to stay correct; this bootstrap id is the same kind of pragmatic anchor.
const TOKEN_BOOTSTRAP_TRACK_ID = '7tFiyTwD0nx5a1eklYtX2J';

const PLAYLIST_PAGE = 100,
  PLAYLIST_CAP = 10_000; // §3.2: playlists 100/page up to 10k.
const ALBUM_PAGE = 50,
  ALBUM_CAP = 2_000; // §3.2: albums 50/page up to 2k.

type Kind = 'playlist' | 'album';

// Persisted-query hashes. Outdated values self-heal from the live bundle when stale,
// so these only need to be a plausible starting point. ponytail: self-heal is the upgrade path.
// searchDesktop below is an unverified placeholder (no offline way to confirm it against the
// live bundle) — it is expected to 404 as PersistedQueryNotFound on first use and self-heal
// immediately; the hardened parallel healHashes() path is the reliable fallback, not this value.
const DEFAULT_HASHES: Record<string, string> = {
  fetchPlaylist: '73a3b3470804983e4d55d83cd6cc99715019228fd999d51429cc69473a18789d',
  getAlbum: '46ae954ef809572f5e21cea2bedf2ea9ff1c5a73af2b90d8a52a96d0e6e98018',
  searchDesktop: '919c08e0c4c5b0a8e8a3b1e6f2e0a6d0c4c5b0a8e8a3b1e6f2e0a6d0c4c5b0a8',
};

export class SpotifyHandler {
  private token?: { value: string; expiresAt: number };
  private hashes: Record<string, string> = { ...DEFAULT_HASHES };
  private healInFlight = new Map<string, Promise<string | null>>();
  private lastHealAt: Record<string, number> = {};

  /** Delegate URL parsing to the single source of truth (url-detector). */
  parseUrl(url: string): ReturnType<typeof parseSpotifyUrl> {
    return parseSpotifyUrl(url);
  }

  // ── Public API ────────────────────────────────────────────────────────────

  /** Single track — read directly from the single-entity embed (Tier 1). */
  async fetchTrack(idOrUrl: string): Promise<Track | null> {
    const id = this.resolveId(idOrUrl);
    if (!id) return null;
    const { entity } = await this.fetchEmbed('track', id);
    if (!entity) return null;
    return mapEmbedTrack(entity, '', id);
  }

  fetchPlaylist(idOrUrl: string): Promise<Playlist | null> {
    return this.fetchCollection('playlist', idOrUrl);
  }

  fetchAlbum(idOrUrl: string): Promise<Playlist | null> {
    return this.fetchCollection('album', idOrUrl);
  }

  /** Best-effort catalogue search; resolves to [] gracefully if it can't. */
  async search(query: string): Promise<Track[]> {
    try {
      if (!this.getToken()) await this.fetchEmbed('track', TOKEN_BOOTSTRAP_TRACK_ID); // mirror fetchCollection: prime the anonymous token on a fresh session
      const json = await this.gql('searchDesktop', {
        searchTerm: query,
        offset: 0,
        limit: 20,
        numberOfTopResults: 20,
        includeAudiobooks: false,
      });
      const items = json?.data?.searchV2?.tracksV2?.items ?? json?.data?.searchV2?.tracks?.items ?? [];
      if (!Array.isArray(items)) return [];
      return items
        .map((it: any) => {
          const data = it?.item?.data ?? it?.data;
          return mapTrackData(data, data?.albumOfTrack);
        })
        .filter((t: Track | null): t is Track => t !== null);
    } catch {
      return [];
    }
  }

  // ── Tier 1: embed page → token + entity ────────────────────────────────────

  private async fetchEmbed(type: string, id: string): Promise<{ entity: any; token: string | null }> {
    try {
      const res = await fetch(`${EMBED}/${type}/${id}`, { headers: { 'user-agent': UA }, signal: AbortSignal.timeout(FETCH_TIMEOUT_MS) });
      const html = await res.text();
      const data = extractNextData(html);
      if (!data) return { entity: null, token: null };
      const token = extractToken(data);
      if (token) this.setToken(token);
      return { entity: extractEntity(data), token };
    } catch {
      return { entity: null, token: null };
    }
  }

  private setToken(value: string): void {
    this.token = { value, expiresAt: Date.now() + TOKEN_TTL_MS };
  }

  /** Cached anonymous token, or null when absent/expired (never fetches — embeds mint it). */
  private getToken(): string | null {
    if (this.token && Date.now() < this.token.expiresAt) return this.token.value;
    this.token = undefined;
    return null;
  }

  // ── Tiers 2 & 3: full track list, with embed fallback ──────────────────────

  private async fetchCollection(kind: Kind, idOrUrl: string): Promise<Playlist | null> {
    const id = this.resolveId(idOrUrl);
    if (!id) return null;
    const { entity, token } = await this.fetchEmbed(kind, id);

    // Tier 2 — full list over the internal GraphQL (token reused across pages).
    // Gated on HAVING a token, not on this page having minted one: within the
    // 25-min TTL an embed response that omits it is normal, and testing only the
    // fresh value silently dropped those fetches to the capped Tier 3 list.
    if (token || this.getToken()) {
      const { tracks, meta, total, complete } = await this.collectTracks(kind, id);
      if (tracks.length)
        return this.buildCollection(kind, id, entity, meta, complete ? total : tracks.length, tracks);
    }

    // Tier 3 — condensed list embedded in the page (capped, less detail).
    if (!entity) return null;
    const fallbackCover = coverUrl(entity.coverArt);
    const list = Array.isArray(entity.trackList) ? entity.trackList : [];
    const tracks = list
      .map((t: any) => mapEmbedTrack(t, fallbackCover))
      .filter((t: Track | null): t is Track => t !== null);
    return this.buildCollection(kind, id, entity, undefined, tracks.length, tracks);
  }

  private async collectTracks(
    kind: Kind,
    id: string,
  ): Promise<{ tracks: Track[]; meta: any; total: number; complete: boolean }> {
    const op = kind === 'playlist' ? 'fetchPlaylist' : 'getAlbum';
    const pageSize = kind === 'playlist' ? PLAYLIST_PAGE : ALBUM_PAGE;
    const cap = kind === 'playlist' ? PLAYLIST_CAP : ALBUM_CAP;
    const uri = `spotify:${kind}:${id}`;
    const tracks: Track[] = [];
    let meta: any;
    let total = 0;
    let complete = false;

    for (let offset = 0; offset < cap; offset += pageSize) {
      const json = await this.gql(op, { uri, offset, limit: pageSize });
      if (!json) break;
      const root = kind === 'playlist' ? json.data?.playlistV2 : json.data?.albumUnion;
      if (!root) break;
      if (!meta) meta = root;
      const content = kind === 'playlist' ? root.content : root.tracksV2;
      const items = content?.items;
      total = content?.totalCount ?? total;
      if (!Array.isArray(items) || items.length === 0) break;
      for (const it of items) {
        const data = kind === 'playlist' ? it?.itemV2?.data : it?.track;
        const albumNode = kind === 'playlist' ? data?.albumOfTrack : root;
        const t = mapTrackData(data, albumNode);
        if (t) tracks.push(t);
      }
      if (items.length < pageSize) {
        complete = true;
        break;
      }
      if (total && tracks.length >= total) {
        complete = true;
        break;
      }
    }
    return { tracks, meta, total, complete };
  }

  /** One GraphQL query with a single stale-hash self-heal + retry behind a cooldown. */
  private async gql(op: string, variables: Record<string, unknown>): Promise<any | null> {
    const token = this.getToken();
    if (!token) return null;
    let json = await this.gqlOnce(op, this.hashes[op], variables, token);
    if (isStale(json)) {
      const fresh = await this.healHashes(op);
      if (fresh) json = await this.gqlOnce(op, fresh, variables, token);
    }
    return json && !json.errors ? json : null;
  }

  private async gqlOnce(
    op: string,
    hash: string,
    variables: Record<string, unknown>,
    token: string,
  ): Promise<any | null> {
    const url =
      `${GQL_URL}?operationName=${encodeURIComponent(op)}` +
      `&variables=${encodeURIComponent(JSON.stringify(variables))}` +
      `&extensions=${encodeURIComponent(JSON.stringify({ persistedQuery: { version: 1, sha256Hash: hash } }))}`;
    try {
      const res = await fetch(url, {
        headers: {
          authorization: `Bearer ${token}`,
          'app-platform': 'WebPlayer',
          accept: 'application/json',
          'user-agent': UA,
        },
        signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
      });
      return await res.json();
    } catch {
      return null;
    }
  }

  /** Pull current persisted-query hashes from the live web-player bundle. Cooldown-guarded. */
  private async healHashes(op: string): Promise<string | null> {
    const inFlight = this.healInFlight.get(op);
    if (inFlight) return inFlight; // concurrent sibling requests await the same attempt instead of degrading to Tier 3

    const now = Date.now();
    if (now - (this.lastHealAt[op] ?? 0) < HEAL_COOLDOWN_MS) return null;

    const promise = this.doHeal(op).finally(() => {
      this.lastHealAt[op] = Date.now(); // cooldown spaces separate attempts only after this one completes
      this.healInFlight.delete(op);
    });
    this.healInFlight.set(op, promise);
    return promise;
  }

  private async doHeal(op: string): Promise<string | null> {
    try {
      const signal = AbortSignal.timeout(HEAL_TIMEOUT_MS); // one aggregate budget across homepage + every bundle fetch below
      const html = await (await fetch(HOME, { headers: { 'user-agent': UA }, signal })).text();
      const srcs = [...html.matchAll(/<script[^>]+src="([^"]+\.js)"/g)].map((m) => m[1]);
      const bundles = await Promise.allSettled(
        srcs.slice(0, MAX_BUNDLES).map((src) => fetch(src, { headers: { 'user-agent': UA }, signal }).then((r) => r.text())),
      );
      for (const b of bundles) {
        if (b.status !== 'fulfilled') continue;
        const hash = findHash(b.value, op);
        if (hash) {
          this.hashes[op] = hash;
          return hash;
        }
      }
    } catch {
      /* leave hashes as-is; the caller falls back to Tier 3 */
    }
    return null;
  }

  // ── Mapping helpers ─────────────────────────────────────────────────────────

  private buildCollection(
    kind: Kind,
    id: string,
    entity: any,
    meta: any,
    total: number,
    tracks: Track[],
  ): Playlist {
    const isAlbum = kind === 'album';
    return {
      id,
      name: meta?.name ?? entity?.name ?? entity?.title ?? '',
      description: String(meta?.description ?? entity?.subtitle ?? ''),
      owner: isAlbum
        ? artistNames(meta?.artists ?? entity?.artists)[0] ?? ''
        : meta?.ownerV2?.data?.name ?? entity?.owner?.name ?? '',
      tracks,
      thumbnailUrl:
        coverUrl(meta?.images?.items?.[0]) || coverUrl(meta?.coverArt) || coverUrl(entity?.coverArt),
      url: `https://open.spotify.com/${kind}/${id}`,
      trackCount: total || tracks.length,
    };
  }

  private resolveId(idOrUrl: string): string | null {
    const parsed = parseSpotifyUrl(idOrUrl);
    if (parsed.id) return parsed.id;
    const trimmed = idOrUrl.trim();
    return /^[A-Za-z0-9]+$/.test(trimmed) ? trimmed : null; // bare base62 id
  }
}

// ── Pure extraction / mapping (module-private, no `this`) ─────────────────────

function extractNextData(html: string): any | null {
  const m = html.match(/<script id="__NEXT_DATA__"[^>]*>([\s\S]*?)<\/script>/);
  if (!m) return null;
  try {
    return JSON.parse(m[1]);
  } catch {
    return null;
  }
}

function extractToken(data: any): string | null {
  const known = data?.props?.pageProps?.state?.data?.session?.accessToken;
  if (typeof known === 'string' && known) return known;
  const found = deepFind(data, 'accessToken'); // recursive fallback if the path moved
  return typeof found === 'string' && found ? found : null;
}

function extractEntity(data: any): any | null {
  return data?.props?.pageProps?.state?.data?.entity ?? deepFind(data, 'entity') ?? null;
}

/** First non-null value for `key` anywhere in the object graph. */
function deepFind(obj: any, key: string): any {
  if (!obj || typeof obj !== 'object') return undefined;
  if (obj[key] != null) return obj[key];
  for (const v of Object.values(obj)) {
    const found = deepFind(v, key);
    if (found !== undefined) return found;
  }
  return undefined;
}

/** Locate the 64-hex persisted-query hash sitting next to an operation name in the bundle. */
function findHash(js: string, op: string): string | null {
  const i = js.indexOf(op);
  if (i === -1) return null;
  const m = js.slice(i, i + 400).match(/[a-f0-9]{64}/i);
  return m ? m[0] : null;
}

function isStale(json: any): boolean {
  return !json || (Array.isArray(json.errors) && json.errors.length > 0);
}

/** GraphQL track node (playlist itemV2.data / album track / search item) → Track. */
function mapTrackData(d: any, albumNode: any): Track | null {
  const uri: string = d?.uri ?? '';
  if (!uri.startsWith('spotify:track:')) return null; // skip episodes / local / unavailable
  const id = uri.slice('spotify:track:'.length);
  const artists = artistNames(d.artists);
  const t: Track = {
    id,
    name: d.name ?? '',
    artist: artists[0] ?? '',
    artists,
    album: albumNode?.name ?? '',
    trackNumber: d.trackNumber ?? 0,
    durationMs: d.trackDuration?.totalMilliseconds ?? d.duration?.totalMilliseconds ?? 0,
    thumbnailUrl: coverUrl(albumNode?.coverArt),
    spotifyUrl: `https://open.spotify.com/track/${id}`,
  };
  const year = yearOf(albumNode?.date);
  if (year) t.albumYear = year;
  return t;
}

/** Embed entity / condensed embed trackList item → Track. */
function mapEmbedTrack(e: any, fallbackCover = '', fallbackId = ''): Track | null {
  const uri: string = e?.uri ?? '';
  // A collection's condensed trackList also carries episodes and local files.
  // Taking the last uri segment as an id turned those into Tracks pointing at
  // /track/<episodeId>, which the bridge then fed to yt-dlp as a search query.
  if (uri && !uri.startsWith('spotify:track:')) return null;
  const id = uri ? uri.slice('spotify:track:'.length) : fallbackId;
  if (!id) return null;
  const artists = e.artists ? artistNames(e.artists) : e.subtitle ? [String(e.subtitle)] : [];
  const t: Track = {
    id,
    name: e.name ?? e.title ?? '',
    artist: artists[0] ?? '',
    artists,
    album: e.album?.name ?? '',
    trackNumber: e.trackNumber ?? 0,
    durationMs: Number(e.duration ?? e.duration_ms) || 0,
    thumbnailUrl: coverUrl(e.coverArt) || fallbackCover,
    spotifyUrl: `https://open.spotify.com/track/${id}`,
  };
  const year = yearOf(e.releaseDate ?? e.album?.date);
  if (year) t.albumYear = year;
  return t;
}

function artistNames(a: any): string[] {
  const items = a?.items ?? a;
  if (!Array.isArray(items)) return [];
  return items
    .map((x: any) => (typeof x === 'string' ? x : x?.profile?.name ?? x?.name))
    .filter((n: unknown): n is string => typeof n === 'string' && n.length > 0);
}

/** Pick the largest cover image url from a `{ sources: [...] }` (or bare array) node. */
function coverUrl(node: any): string {
  const sources = node?.sources ?? node;
  if (!Array.isArray(sources) || sources.length === 0) return '';
  return sources[sources.length - 1]?.url ?? '';
}

function yearOf(date: any): string | undefined {
  if (!date) return undefined;
  if (date.year) return String(date.year);
  const iso = date.isoString ?? (typeof date === 'string' ? date : '');
  const m = String(iso).match(/^(\d{4})/);
  return m ? m[1] : undefined;
}
