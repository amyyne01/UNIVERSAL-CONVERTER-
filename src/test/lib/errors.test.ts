import { ERROR_CODES } from '@shared/types';
import { ERROR_COPY, describeError, isPermanentError } from '@/lib/errors';

describe('error copy', () => {
  it('has a cause and a non-empty remedy for every ErrorCode', () => {
    for (const code of ERROR_CODES) {
      expect(ERROR_COPY[code].cause.length).toBeGreaterThan(0);
      expect(ERROR_COPY[code].remedy.length).toBeGreaterThan(0);
    }
  });
  it('falls back to unknown', () => {
    expect(describeError(undefined)).toBe(describeError('unknown'));
  });

  it('marks exactly the codes no retry can fix as permanent', () => {
    const permanent = ERROR_CODES.filter(isPermanentError);
    expect(permanent.sort()).toEqual(['drm', 'geo_blocked', 'removed', 'unsupported']);
  });

  it('never offers "try again" for a permanent failure', () => {
    for (const code of ERROR_CODES.filter(isPermanentError)) {
      expect(describeError(code)).not.toMatch(/try again|retry/i);
    }
  });

  it('keeps engine names, reinstall advice and em-dashes out of visible copy', () => {
    for (const code of ERROR_CODES) {
      for (const source of ['youtube', 'spotify', 'tiktok', undefined] as const) {
        expect(describeError(code, source)).not.toMatch(/yt-dlp|reinstall|—/i);
      }
    }
  });

  it('says "video" for YouTube and "link" elsewhere, never "post" for YouTube', () => {
    expect(describeError('private', 'youtube')).toMatch(/video/);
    expect(describeError('private', 'youtube')).not.toMatch(/post/);
    expect(describeError('private', 'soundcloud')).toMatch(/link/);
  });

  it('reads "removed" as not-found, and "unknown" does not blame the connection', () => {
    expect(describeError('removed')).toMatch(/couldn.t find anything at this link/i);
    expect(describeError('unknown')).not.toMatch(/connection/i);
  });

  it('gives Spotify failures a next step', () => {
    expect(describeError('unknown', 'spotify')).toMatch(/Spotify/);
  });
});
