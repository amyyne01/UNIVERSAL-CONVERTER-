// UI-only constants for the renderer. Domain types live in shared/.
// Format/quality lists here drive the selectors; the main process owns the
// authoritative download logic — these are presentation choices only.
import type { AppIcon } from '@/components/ui/icons';
import {
  Home, Download, YouTube, Spotify, SoundCloud, ShortForm, Settings,
  Resolution, Batch, Collection, Audio, Scheduled,
} from '@/components/ui/icons';
import type { AudioFormat, VideoQuality, UiSource } from '@shared/types';
import { BASIC_LIMITS, LOSSLESS_FORMATS, MAX_CONCURRENT_DOWNLOADS, VIDEO_QUALITIES as VIDEO_QUALITY_VALUES } from '@shared/types';

/** The renderer's navigable tabs (left rail + Ctrl+number order). */
export type TabKey = 'home' | 'youtube' | 'spotify' | 'soundcloud' | 'reels' | 'queue' | 'settings';
export type PlatformKey = UiSource; // 'youtube' | 'spotify' | 'soundcloud' | 'reels'

export interface AudioFormatOption {
  value: AudioFormat;
  label: string;
  lossless: boolean;
}

// Presentation labels; the lossless flag derives from the shared LOSSLESS_FORMATS
// set so the codec classification lives in exactly one place (shared/types.ts).
const AUDIO_FORMAT_LABELS: readonly { value: AudioFormat; label: string }[] = [
  { value: 'mp3', label: 'MP3' },
  { value: 'aac', label: 'AAC' },
  { value: 'm4a', label: 'M4A' },
  { value: 'ogg', label: 'OGG' },
  { value: 'opus', label: 'Opus' },
  { value: 'flac', label: 'FLAC' },
  { value: 'wav', label: 'WAV' },
  { value: 'alac', label: 'ALAC' },
];

export const AUDIO_FORMATS: readonly AudioFormatOption[] = AUDIO_FORMAT_LABELS.map((f) => ({
  ...f,
  lossless: (LOSSLESS_FORMATS as readonly string[]).includes(f.value),
}));

export interface Option {
  value: string;
  label: string;
}

export const AUDIO_QUALITIES: readonly Option[] = [
  { value: '128', label: '128 kbps' },
  { value: '192', label: '192 kbps' },
  { value: '256', label: '256 kbps' },
  { value: '320', label: '320 kbps' },
  { value: 'lossless', label: 'Lossless' },
];

// Values come from the shared VIDEO_QUALITIES const (single source of truth for
// the union); only the display labels that differ from the raw value live here.
const VIDEO_QUALITY_LABELS: Partial<Record<VideoQuality, string>> = {
  '720p': '720p HD',
  '1080p': '1080p Full HD',
  '2160p': '2160p 4K',
  best: 'Best available',
};

export const VIDEO_QUALITIES: readonly { value: VideoQuality; label: string }[] =
  VIDEO_QUALITY_VALUES.map((value) => ({ value, label: VIDEO_QUALITY_LABELS[value] ?? value }));

// ── Download extras (§5) ─────────────────────────────────────────────────────
// Presentation labels for the shared unions. The unions themselves live in
// shared/types.ts; only the wording is a renderer choice.
export const SUBTITLE_MODE_OPTIONS: readonly Option[] = [
  { value: 'off',   label: 'Off' },
  { value: 'embed', label: 'Embed in the file' },
  { value: 'file',  label: 'Separate .srt' },
  { value: 'both',  label: 'Both' },
];

export const SPONSORBLOCK_OPTIONS: readonly Option[] = [
  { value: 'off',    label: 'Off' },
  { value: 'mark',   label: 'Mark as chapters' },
  { value: 'remove', label: 'Cut them out' },
];

// Which segments SponsorBlock acts on. Labels are the community's category
// names in plain English; the values must match SPONSORBLOCK_CATEGORIES.
export const SPONSORBLOCK_CATEGORY_OPTIONS: readonly Option[] = [
  { value: 'sponsor',        label: 'Sponsor' },
  { value: 'intro',          label: 'Intro' },
  { value: 'outro',          label: 'Outro' },
  { value: 'selfpromo',      label: 'Self-promo' },
  { value: 'interaction',    label: 'Subscribe reminder' },
  { value: 'preview',        label: 'Recap' },
  { value: 'music_offtopic', label: 'Non-music' },
];

export const VIDEO_CONTAINER_OPTIONS: readonly Option[] = [
  { value: 'mp4',  label: 'MP4 (most compatible)' },
  { value: 'mkv',  label: 'MKV (keeps everything)' },
  { value: 'webm', label: 'WebM' },
];

export const VIDEO_CODEC_OPTIONS: readonly Option[] = [
  { value: 'any',  label: 'Whatever is best' },
  { value: 'h264', label: 'H.264 (plays anywhere)' },
  { value: 'vp9',  label: 'VP9 (smaller)' },
  { value: 'av1',  label: 'AV1 (smallest)' },
];

/** 1..MAX slots. Built from the shared ceiling so the list can never offer a
 *  number the engine would clamp away. */
export const CONCURRENCY_OPTIONS: readonly Option[] = Array.from(
  { length: MAX_CONCURRENT_DOWNLOADS },
  (_, i) => ({ value: String(i + 1), label: i === 0 ? '1 at a time' : `${i + 1} at a time` }),
);

export const COOKIE_BROWSER_OPTIONS: readonly Option[] = [
  { value: '',         label: "Don't use cookies" },
  { value: 'chrome',   label: 'Chrome' },
  { value: 'edge',     label: 'Edge' },
  { value: 'firefox',  label: 'Firefox' },
  { value: 'brave',    label: 'Brave' },
  { value: 'chromium', label: 'Chromium' },
  { value: 'opera',    label: 'Opera' },
  { value: 'vivaldi',  label: 'Vivaldi' },
];

export interface PlatformDef {
  key: PlatformKey;
  label: string;
  /** Color token name → CSS var(--color-<accent>). */
  accent: PlatformKey;
  icon: AppIcon;
}

export const PLATFORMS: readonly PlatformDef[] = [
  { key: 'youtube', label: 'YouTube', accent: 'youtube', icon: YouTube },
  { key: 'spotify', label: 'Spotify', accent: 'spotify', icon: Spotify },
  { key: 'soundcloud', label: 'SoundCloud', accent: 'soundcloud', icon: SoundCloud },
  { key: 'reels', label: 'Reels & Shorts', accent: 'reels', icon: ShortForm },
];

export interface NavItem {
  key: TabKey;
  label: string;
  icon: AppIcon;
  group: 'Library' | 'Sources' | 'System';
}

export const NAV: readonly NavItem[] = [
  { key: 'home', label: 'Home', icon: Home, group: 'Library' },
  { key: 'queue', label: 'Downloads', icon: Download, group: 'Library' },
  { key: 'youtube', label: 'YouTube', icon: YouTube, group: 'Sources' },
  { key: 'spotify', label: 'Spotify', icon: Spotify, group: 'Sources' },
  { key: 'soundcloud', label: 'SoundCloud', icon: SoundCloud, group: 'Sources' },
  { key: 'reels', label: 'Reels & Shorts', icon: ShortForm, group: 'Sources' },
  { key: 'settings', label: 'Settings', icon: Settings, group: 'System' },
];

/** Ctrl+1 … Ctrl+n select these tabs in order. */
export const TAB_ORDER: readonly TabKey[] = NAV.map((n) => n.key);

// ── Tier comparison ───────────────────────────────────────────────────────────
// Presentation only: the numbers come from BASIC_LIMITS (@shared/types), which is
// what the main process actually enforces, so the sheet can never advertise a
// ceiling the engine doesn't apply. Every row maps to real, shipped behaviour.
export interface PlanFeature {
  key: string;
  label: string;
  icon: AppIcon;
  basic: string;
  premium: string;
}

export const PLAN_FEATURES: readonly PlanFeature[] = [
  {
    key: 'video',
    label: 'Video quality',
    icon: Resolution,
    basic: `Up to ${BASIC_LIMITS.maxVideoQuality}`,
    premium: 'Up to 4K, or best available',
  },
  {
    key: 'batch',
    label: 'Batch paste',
    icon: Batch,
    basic: `${BASIC_LIMITS.maxBatchLinks} links at a time`,
    premium: 'As many links as you paste',
  },
  {
    key: 'collections',
    label: 'Playlists & albums',
    icon: Collection,
    basic: `First ${BASIC_LIMITS.maxCollectionTracks} tracks`,
    premium: 'Every track',
  },
  {
    key: 'lossless',
    label: 'Lossless audio',
    icon: Audio,
    basic: 'Lossy formats only',
    premium: 'FLAC, WAV and ALAC',
  },
  {
    key: 'scheduler',
    label: 'Scheduled downloads',
    icon: Scheduled,
    basic: 'Not included',
    premium: 'Run unattended, on your days',
  },
];
