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
