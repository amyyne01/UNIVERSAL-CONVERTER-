// Normalizes engine search/track shapes into one row model for result lists.
// Lifted out of PlatformTab so PlatformTab and the Dashboard's unified search
// share one home for this logic instead of two copies drifting apart.
import type { SearchResult, Track, SourcePlatform } from '@shared/types';

/** A row in a results list — normalized from a SearchResult or a Spotify Track. */
export interface ResultItem {
  id: string;
  title: string;
  subtitle: string;
  thumbnailUrl: string;
  duration: number;
  url: string;
  track?: Track;
  /** Detected source for direct-URL results (reels tab hosts several platforms). */
  source?: SourcePlatform;
  /** True when this is THE fetched item for a pasted URL (renders as a big card). */
  direct?: boolean;
  /** Human label from url:detect (e.g. "YouTube video") for the result-card eyebrow. */
  label?: string;
}

export function searchToItem(r: SearchResult): ResultItem {
  return { id: r.id, title: r.title, subtitle: r.uploader, thumbnailUrl: r.thumbnailUrl, duration: r.duration, url: r.url };
}

export function trackToItem(t: Track): ResultItem {
  return {
    id: t.id,
    title: t.name,
    subtitle: t.artist,
    thumbnailUrl: t.thumbnailUrl,
    duration: Math.round(t.durationMs / 1000),
    url: t.spotifyUrl ?? '',
    track: t,
  };
}
