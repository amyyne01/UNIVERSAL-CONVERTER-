// Shared renderer download-request helpers — single source of truth for the
// config-default fallbacks and the Spotify→ytsearch bridge (previously
// hand-duplicated, and diverging, across PlatformTab/CommandPalette/PlaylistPreview).
import type { AppConfig, DownloadTask, Track } from '@shared/types';

/** Merge saved config defaults with per-download overrides for `download.start`. */
export function buildDownloadRequest(
  config: AppConfig | null | undefined,
  overrides: Partial<DownloadTask>,
): Partial<DownloadTask> {
  return {
    format: config?.defaultFormat ?? 'mp3',
    quality: config?.defaultQuality ?? '320',
    videoQuality: config?.defaultVideoQuality ?? 'best',
    embedThumbnail: config?.embedThumbnail ?? true,
    embedMetadata: config?.embedMetadata ?? true,
    skipExisting: config?.skipExisting ?? true,
    ...overrides,
  };
}

/** Spotify audio isn't directly downloadable — bridge to a yt-dlp search query. */
export function spotifyDownloadUrl(track: Track): string {
  return track.spotifyUrl || `ytsearch:${track.artist} - ${track.name}`;
}

/** 1-based item numbers → yt-dlp `--playlist-items` spec, runs collapsed: [1,2,3,5] → "1-3,5". */
export function toPlaylistItems(nums: number[]): string {
  const sorted = [...new Set(nums)].sort((a, b) => a - b);
  const parts: string[] = [];
  for (let i = 0; i < sorted.length; i++) {
    let j = i;
    while (sorted[j + 1] === sorted[j] + 1) j++;
    parts.push(j > i ? `${sorted[i]}-${sorted[j]}` : String(sorted[i]));
    i = j;
  }
  return parts.join(',');
}
