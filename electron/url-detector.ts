import type { UrlDetection, SourcePlatform, ContentType } from '../shared/types.js';

// HOW-THE-APP-WORKS.md §2 — ordered URL-recognition cascade.
// Trim → prepend https:// if no scheme → walk a FIXED ordered list, first match wins,
// more specific shapes before general. Output drives the whole download pipeline.

const PLATFORM_LABEL: Record<SourcePlatform, string> = {
  youtube: 'YouTube',
  spotify: 'Spotify',
  soundcloud: 'SoundCloud',
  instagram: 'Instagram',
  tiktok: 'TikTok',
  facebook: 'Facebook',
  direct: 'Direct',
  unknown: 'Unknown',
};

function build(url: string, platform: SourcePlatform, contentType: ContentType, id?: string): UrlDetection {
  const isCollection = contentType === 'playlist' || contentType === 'album';
  const label =
    platform === 'direct' ? 'Direct media' :
    platform === 'unknown' ? 'Unknown link' :
    `${PLATFORM_LABEL[platform]} ${contentType}`;
  return { url, platform, contentType, id, isCollection, label };
}

/** Recognise Spotify links in both the web (open.spotify.com/[intl-xx/]<type>/<id>)
 *  and URI (spotify:<type>:<id>) notations across the four content types. */
export function parseSpotifyUrl(url: string): { type: 'track' | 'playlist' | 'album' | 'artist' | null; id: string | null } {
  const m = url.match(
    /(?:open\.spotify\.com\/(?:intl-[a-z]{2}\/)?|spotify:)(track|playlist|album|artist)[/:]([A-Za-z0-9]+)/i,
  );
  if (!m) return { type: null, id: null };
  return { type: m[1].toLowerCase() as 'track' | 'playlist' | 'album' | 'artist', id: m[2] };
}

// The fixed ordered cascade. Each matcher returns a detection or null; first non-null wins.
const MATCHERS: Array<(url: string) => UrlDetection | null> = [
  // YouTube playlist (before the generic single-video rule).
  (url) => {
    const m = url.match(/youtube\.com\/playlist\?.*\blist=([A-Za-z0-9_-]+)/i);
    return m ? build(url, 'youtube', 'playlist', m[1]) : null;
  },
  // YouTube video — 11-char id after watch?v= / youtu.be/ / /embed/ / /shorts/ / /live/.
  (url) => {
    const m = url.match(
      /(?:youtube\.com\/(?:watch\?(?:[^#]*&)?v=|embed\/|shorts\/|live\/)|youtu\.be\/)([A-Za-z0-9_-]{11})/i,
    );
    return m ? build(url, 'youtube', 'video', m[1]) : null;
  },
  // Spotify (web + uri), track/playlist/album/artist.
  (url) => {
    const sp = parseSpotifyUrl(url);
    return sp.type ? build(url, 'spotify', sp.type, sp.id ?? undefined) : null;
  },
  // SoundCloud set = playlist (before the looser track rule).
  (url) => {
    const m = url.match(/soundcloud\.com\/[^/]+\/sets\/([^/?#]+)/i);
    return m ? build(url, 'soundcloud', 'playlist', m[1]) : null;
  },
  // SoundCloud track — human slug, negative-lookahead excludes profile sub-pages.
  (url) => {
    const m = url.match(
      /soundcloud\.com\/[^/]+\/(?!(?:sets|likes|reposts|followers|following|tracks|albums|you)(?:[/?#]|$))([^/?#]+)/i,
    );
    return m ? build(url, 'soundcloud', 'track', m[1]) : null;
  },
  // Instagram short-form video.
  (url) => {
    const m = url.match(/instagram\.com\/(?:reel|reels|p|tv)\/([^/?#]+)/i);
    return m ? build(url, 'instagram', 'video', m[1]) : null;
  },
  // TikTok short-form video. Only a real video URL matches — a bare profile
  // (tiktok.com/@user) or search page falls through to "unknown", not a doomed job.
  (url) => {
    if (!/tiktok\.com/i.test(url)) return null;
    // Canonical @user/video/<numeric id>
    const canonical = url.match(/@[^/]+\/video\/(\d+)/i);
    if (canonical) return build(url, 'tiktok', 'video', canonical[1]);
    // Short links: vm./vt.tiktok.com/<code> or tiktok.com/t/<code>
    const short = url.match(/(?:vm\.|vt\.)tiktok\.com\/([A-Za-z0-9]+)|tiktok\.com\/t\/([A-Za-z0-9]+)/i);
    if (short) return build(url, 'tiktok', 'video', short[1] ?? short[2]);
    return null;
  },
  // Facebook short-form video (must carry a video id, not just the domain).
  (url) => {
    if (!/(?:facebook\.com|fb\.watch)/i.test(url)) return null;
    const m = url.match(/(?:\/videos\/|\/reel\/|[?&]v=|fb\.watch\/)([A-Za-z0-9]+)/i);
    return m ? build(url, 'facebook', 'video', m[1]) : null;
  },
  // Final safety net: direct media file by extension.
  (url) => {
    const m = url.match(/\.(?:mp4|mkv|webm|avi|mov)(?:[?#]|$)/i);
    return m ? build(url, 'direct', 'video') : null;
  },
];

export function detectUrl(input: string): UrlDetection {
  const trimmed = input.trim();
  // A bare address (no scheme) gets https://; spotify:/http(s): URIs keep theirs.
  const url = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(trimmed) ? trimmed : `https://${trimmed}`;

  for (const match of MATCHERS) {
    const hit = match(url);
    if (hit) return hit;
  }
  return build(url, 'unknown', 'unknown');
}
