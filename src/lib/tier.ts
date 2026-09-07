// Renderer-side view of the tier: what the UI shows as locked, so a control
// never silently does less than it says.
//
// Division of labour, precisely: the PER-REQUEST limits (video quality, lossless
// format/bitrate, and a collection URL's --playlist-end) are clamped in the main
// process by electron/ipc.ts, which is authoritative. The COUNT limits — 5 links
// per batch paste, 20 tracks out of a Spotify collection — are enforced here,
// because each of those items is an ordinary single download:start call that main
// cannot tell apart from a user deliberately queueing their 21st track one at a
// time. A count guard there would reject legitimate downloads, which is a worse
// bug than the ceiling it closes: this is the user's own machine, and bypassing
// it is the same accepted limit as the shipped token being extractable.
import { useMemo } from 'react';
import { AUDIO_FORMATS, VIDEO_QUALITIES } from '@/constants';
import type { Plan, VideoQuality } from '@shared/types';
import {
  BASIC_LIMITS, COOKIE_BROWSERS, MAX_CONCURRENT_DOWNLOADS,
  clampAudioQuality, clampFormat, clampVideoQuality,
  isPremiumFormat, isPremiumVideoQuality,
} from '@shared/types';
import { useAppStore } from '@/store';

const PREMIUM_VIDEO_VALUES = VIDEO_QUALITIES.filter((q) => isPremiumVideoQuality(q.value)).map((q) => q.value);
// Via isPremiumFormat, not the lossless flag directly, so the lock follows the
// same definition the engine clamps with.
const PREMIUM_FORMAT_VALUES = AUDIO_FORMATS.filter((f) => isPremiumFormat(f.value)).map((f) => f.value);
// Every browser except '' — picking "don't use cookies" is not a premium act.
const PREMIUM_COOKIE_VALUES = COOKIE_BROWSERS.filter(Boolean);
// Slot counts above the free cap, as the Select's string values.
const PREMIUM_CONCURRENCY_VALUES = Array.from(
  { length: MAX_CONCURRENT_DOWNLOADS - BASIC_LIMITS.maxConcurrent },
  (_, i) => String(BASIC_LIMITS.maxConcurrent + 1 + i),
);
const NONE: readonly string[] = [];

// What a locked control says. One home for all of it, because the same sentence
// was being retyped across four files and had already started to drift.
//
// ONE shape, every time: "<thing> is a Premium feature." The user is mid-task and
// did not come here to read; they need to know instantly that they hit a paywall
// and where to go. Explaining bitrates or pixel counts makes them decode a
// sentence to learn something they already understood from the lock icon.
// The detail belongs in the sheet, next to the price — not in a toast.
// Two shapes, because there are two kinds of ceiling. A LOCKED capability is
// binary — the control was never yours, so it just says so. A COUNTED limit is
// one you were already using and have now run out of, so it names the number you
// hit; "why did only 5 of my 8 links queue?" has to be answerable on the spot.
export const premiumCopy = {
  losslessFormat: (label: string) => `${label} is a Premium feature.`,
  losslessQuality: () => 'Lossless audio is a Premium feature.',
  videoQuality: (quality: string) => `${quality} video is a Premium feature.`,
  scheduler: () => 'Scheduled downloads are a Premium feature.',
  subtitleLanguages: () => 'Multiple subtitle languages are a Premium feature.',
  subtitleAuto: () => 'Auto-generated captions are a Premium feature.',
  splitChapters: () => 'Splitting by chapter is a Premium feature.',
  concurrency: () =>
    `More than ${BASIC_LIMITS.maxConcurrent} downloads at once is a Premium feature.`,
  cookies: () => 'Signing in with your browser is a Premium feature.',
  batch: () =>
    `Free limit reached — ${BASIC_LIMITS.maxBatchLinks} links at a time. Upgrade for unlimited.`,
  collection: () =>
    `Free limit reached — ${BASIC_LIMITS.maxCollectionTracks} tracks per playlist. Upgrade for unlimited.`,
} as const;

// What Basic GIVES you, stated where the work happens.
//
// The nudges above only fire when someone hits a ceiling, which means the tier
// only ever speaks to a user at the moment it says no. That teaches "the free
// version is the crippled one". These lines are the other half: on each surface,
// a quiet statement of what is included at no cost — phrased as a capability, not
// as a limit. "Free: video up to 1080p" is the same fact as "Basic caps you at
// 1080p" and lands in the opposite place. A user who feels well-treated by the
// free tier is the one who believes Premium is worth paying for; a user who feels
// nickel-and-dimed just resents the lock.
//
// Numbers come from BASIC_LIMITS, so these can never promise a ceiling the engine
// does not actually apply.
export const basicCopy = {
  video: `Free: video up to ${BASIC_LIMITS.maxVideoQuality}`,
  audio: 'Free: MP3, AAC, M4A, OGG and Opus, up to 320 kbps',
  batch: `Free: ${BASIC_LIMITS.maxBatchLinks} links at a time`,
  concurrency: `Free: ${BASIC_LIMITS.maxConcurrent} downloads at once`,
  collection: `Free: the first ${BASIC_LIMITS.maxCollectionTracks} tracks of any playlist`,
  /** The full one-line summary, for the plan card in Settings. */
  summary: `Included free: ${BASIC_LIMITS.maxVideoQuality} video · 320 kbps audio · ${BASIC_LIMITS.maxBatchLinks}-link batches · ${BASIC_LIMITS.maxCollectionTracks} tracks per playlist`,
} as const;

export interface TierLocks {
  isPremium: boolean;
  /** Opens the full upgrade sheet (pricing cards). */
  openUpgrade: () => void;
  /** Raises the highlighted popup naming the limit that was just hit. Locked
   *  controls call this rather than throwing the whole sheet at the user. */
  nudge: (reason: string) => void;
  lockedVideoQualities: readonly string[];
  lockedAudioQualities: readonly string[];
  lockedFormats: readonly string[];
  /** Every real browser, when cookies are premium — '' (no cookies) stays open. */
  lockedCookieBrowsers: readonly string[];
  /** Slot counts above the free cap, as Select values. */
  lockedConcurrency: readonly string[];
  /** Max links one batch paste may start, or null when unlimited. */
  batchLimit: number | null;
  /** Max tracks selectable from a collection, or null when unlimited. */
  collectionLimit: number | null;
  /** What a saved default actually becomes on this tier — what selectors display. */
  effectiveVideoQuality: (q: VideoQuality) => VideoQuality;
  effectiveFormat: (format: string) => string;
  effectiveAudioQuality: (quality: string) => string;
}

export function useTierLocks(): TierLocks {
  const plan: Plan = useAppStore((s) => s.plan);
  const isPremium = plan === 'premium';
  const setUpgradeOpen = useAppStore((s) => s.setUpgradeOpen);
  const showPremiumNudge = useAppStore((s) => s.showPremiumNudge);

  return useMemo(
    () => ({
      isPremium,
      openUpgrade: () => setUpgradeOpen(true),
      nudge: showPremiumNudge,
      lockedVideoQualities: isPremium ? NONE : PREMIUM_VIDEO_VALUES,
      lockedAudioQualities: isPremium ? NONE : ['lossless'],
      lockedFormats: isPremium ? NONE : PREMIUM_FORMAT_VALUES,
      lockedCookieBrowsers: isPremium || BASIC_LIMITS.cookies ? NONE : PREMIUM_COOKIE_VALUES,
      lockedConcurrency: isPremium ? NONE : PREMIUM_CONCURRENCY_VALUES,
      batchLimit: isPremium ? null : BASIC_LIMITS.maxBatchLinks,
      collectionLimit: isPremium ? null : BASIC_LIMITS.maxCollectionTracks,
      effectiveVideoQuality: (q: VideoQuality) => clampVideoQuality(q, plan),
      effectiveFormat: (f: string) => clampFormat(f, plan),
      effectiveAudioQuality: (q: string) => clampAudioQuality(q, plan),
    }),
    [plan, isPremium, setUpgradeOpen, showPremiumNudge],
  );
}
