// ONE copy map for every failure the user can see: what happened, and what to do
// next. Keyed by the shared ErrorCode so main's classifier and this stay in step
// (the compiler flags a code with no entry). Plain words, no jargon; every remedy
// is an action.
import type { ErrorCode } from '@shared/types';

export interface ErrorCopy { cause: string; remedy: string }

export const ERROR_COPY: Record<ErrorCode, ErrorCopy> = {
  private:        { cause: 'This video is private.', remedy: 'Ask the owner for access, or sign in with your browser in Settings and try again.' },
  removed:        { cause: 'This video has been removed.', remedy: 'Check the link, or paste a different one.' },
  age_restricted: { cause: 'This video is age-restricted.', remedy: 'Sign in with your browser in Settings, then retry.' },
  login_required: { cause: 'This site wants you to sign in first.', remedy: 'Sign in with your browser in Settings, then retry.' },
  geo_blocked:    { cause: 'This video is not available in your country.', remedy: 'Try a different link, or use a VPN you trust.' },
  drm:            { cause: 'This content is copy-protected.', remedy: 'It cannot be downloaded. Try a different link.' },
  rate_limited:   { cause: 'The site is asking us to slow down.', remedy: 'Wait a few minutes, then retry.' },
  network:        { cause: 'The connection dropped.', remedy: 'Check your internet, then retry.' },
  unsupported:    { cause: 'This link is not something we can download.', remedy: 'Check the link, or try one from YouTube, Spotify or SoundCloud.' },
  ffmpeg_missing: { cause: 'A part of the app needed for conversion is missing.', remedy: 'Reinstall the app, or restore the files it came with.' },
  disk_full:      { cause: 'There is no space left on the drive.', remedy: 'Free up some space or pick another folder in Settings, then retry.' },
  unknown:        { cause: 'Something went wrong.', remedy: 'Check your connection and try again.' },
};

/** One readable sentence pair. Unknown or missing codes fall back to `unknown`. */
export function describeError(code?: ErrorCode): string {
  const { cause, remedy } = ERROR_COPY[code ?? 'unknown'] ?? ERROR_COPY.unknown;
  return `${cause} ${remedy}`;
}
