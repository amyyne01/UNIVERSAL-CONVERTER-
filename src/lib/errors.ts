// ONE copy map for every failure the user can see: what happened, and what to do
// next. Keyed by the shared ErrorCode so main's classifier and this stay in step
// (the compiler flags a code with no entry). Plain words, no jargon; every remedy
// is an action.
import type { ErrorCode, SourcePlatform } from '@shared/types';

export interface ErrorCopy { cause: string; remedy: string }

// `{thing}` becomes "video" for the video sites and "link" everywhere else.
export const ERROR_COPY: Record<ErrorCode, ErrorCopy> = {
  private:        { cause: 'This {thing} is private.', remedy: 'Ask the owner for access, or sign in with your browser in Settings, then retry.' },
  removed:        { cause: 'We couldn’t find anything at this link.', remedy: 'Check the link, or paste a different one.' },
  age_restricted: { cause: 'This {thing} is age-restricted.', remedy: 'Sign in with your browser in Settings, then retry.' },
  login_required: { cause: 'This site wants you to sign in first.', remedy: 'Sign in with your browser in Settings, then retry.' },
  geo_blocked:    { cause: 'This {thing} isn’t available in your country.', remedy: 'Paste a different link, or use a VPN you trust.' },
  drm:            { cause: 'This {thing} is copy-protected.', remedy: 'It can’t be downloaded. Paste a different link.' },
  rate_limited:   { cause: 'The site is asking us to slow down.', remedy: 'Wait a few minutes, then retry.' },
  network:        { cause: 'The connection dropped.', remedy: 'Check your internet, then retry.' },
  unsupported:    { cause: 'This link isn’t something we can download.', remedy: 'Check the link, or paste one from YouTube, Spotify, SoundCloud, Vimeo and more.' },
  ffmpeg_missing: { cause: 'A part of the app needed for conversion is missing.', remedy: 'Download the app again, or restore the files it came with.' },
  disk_full:      { cause: 'There is no space left on the drive.', remedy: 'Free up some space or pick another folder in Settings, then retry.' },
  unknown:        { cause: 'Something went wrong.', remedy: 'Try again in a moment.' },
};

/** Failures no retry can fix: rows and Retry-all must not offer one. */
const PERMANENT = new Set<ErrorCode>(['removed', 'unsupported', 'drm', 'geo_blocked']);
export const isPermanentError = (code?: ErrorCode): boolean => !!code && PERMANENT.has(code);

const VIDEO_SOURCES = new Set<SourcePlatform>(['youtube', 'vimeo', 'dailymotion', 'rumble', 'twitch']);
// Spotify audio is matched by name, so a dead end there has one more way out.
const SPOTIFY_REMEDY = 'We find Spotify songs by name. Search for it in the Spotify tab, or paste a different link.';

/** One readable sentence pair. Unknown or missing codes fall back to `unknown`. */
export function describeError(code?: ErrorCode, source?: SourcePlatform): string {
  const { cause, remedy } = ERROR_COPY[code ?? 'unknown'] ?? ERROR_COPY.unknown;
  const thing = source && VIDEO_SOURCES.has(source) ? 'video' : 'link';
  const spotifyDeadEnd = source === 'spotify' && (!code || code === 'unknown' || code === 'removed' || code === 'unsupported');
  return `${cause.replace('{thing}', thing)} ${spotifyDeadEnd ? SPOTIFY_REMEDY : remedy}`;
}
