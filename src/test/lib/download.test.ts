import { describe, it, expect } from 'vitest';
import { buildDownloadRequest, spotifyDownloadUrl } from '@/lib/download';
import type { AppConfig, Track } from '@shared/types';

const config: AppConfig = {
  outputDir: '',
  defaultFormat: 'mp3',
  defaultQuality: '320',
  defaultVideoQuality: '1080p',
  concurrentDownloads: 1,
  embedThumbnail: true,
  embedMetadata: true,
  skipExisting: true,
  theme: 'dark',
  rememberLastDir: true,
  autoPaste: false,
  clipboardWatch: false,
  showNotifications: true,
  showVisualizer: false,
  showAmbientParticles: false,
  rateLimit: '',
  proxy: '',
  ffmpegPath: '',
  minimizeToTray: false,
  globalHotkey: '',
  discordRichPresence: false,
  scheduleEnabled: false,
  scheduleTime: '',
  scheduleDays: [],
  scheduleShutdown: false,
};

describe('buildDownloadRequest', () => {
  it('takes format/quality/embed/skipExisting defaults from config', () => {
    const req = buildDownloadRequest(config, { url: 'https://x', source: 'youtube' });
    expect(req).toMatchObject({
      format: 'mp3',
      quality: '320',
      videoQuality: '1080p',
      embedThumbnail: true,
      embedMetadata: true,
      skipExisting: true,
      url: 'https://x',
      source: 'youtube',
    });
  });

  it('falls back to hardcoded defaults when config is missing', () => {
    const req = buildDownloadRequest(null, { url: 'https://x', source: 'youtube' });
    expect(req).toMatchObject({
      format: 'mp3',
      quality: '320',
      videoQuality: 'best',
      embedThumbnail: true,
      embedMetadata: true,
      skipExisting: true,
    });
  });

  it('lets overrides win over config defaults', () => {
    const req = buildDownloadRequest(config, { format: 'flac', skipExisting: false });
    expect(req.format).toBe('flac');
    expect(req.skipExisting).toBe(false);
  });
});

describe('spotifyDownloadUrl', () => {
  const track: Track = {
    id: '1',
    name: 'Song',
    artist: 'Artist',
    artists: ['Artist'],
    album: 'Album',
    trackNumber: 1,
    durationMs: 200000,
    thumbnailUrl: '',
  };

  it('uses the spotifyUrl when present', () => {
    expect(spotifyDownloadUrl({ ...track, spotifyUrl: 'https://open.spotify.com/track/1' }))
      .toBe('https://open.spotify.com/track/1');
  });

  it('falls back to a ytsearch query when no spotifyUrl', () => {
    expect(spotifyDownloadUrl(track)).toBe('ytsearch:Artist - Song');
  });
});
