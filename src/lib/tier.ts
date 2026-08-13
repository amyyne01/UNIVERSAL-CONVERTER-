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
  BASIC_LIMITS, clampAudioQuality, clampFormat, clampVideoQuality,
  isPremiumFormat, isPremiumVideoQuality,
} from '@shared/types';
import { useAppStore } from '@/store';

const PREMIUM_VIDEO_VALUES = VIDEO_QUALITIES.filter((q) => isPremiumVideoQuality(q.value)).map((q) => q.value);
// Via isPremiumFormat, not the lossless flag directly, so the lock follows the
// same definition the engine clamps with.
const PREMIUM_FORMAT_VALUES = AUDIO_FORMATS.filter((f) => isPremiumFormat(f.value)).map((f) => f.value);
const NONE: readonly string[] = [];

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
      batchLimit: isPremium ? null : BASIC_LIMITS.maxBatchLinks,
      collectionLimit: isPremium ? null : BASIC_LIMITS.maxCollectionTracks,
      effectiveVideoQuality: (q: VideoQuality) => clampVideoQuality(q, plan),
      effectiveFormat: (f: string) => clampFormat(f, plan),
      effectiveAudioQuality: (q: string) => clampAudioQuality(q, plan),
    }),
    [plan, isPremium, setUpgradeOpen, showPremiumNudge],
  );
}
