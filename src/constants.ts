// UI-only constants for the renderer. Domain types live in shared/.
// Format/quality lists here drive the selectors; the main process owns the
// authoritative download logic — these are presentation choices only.
import type { LucideIcon } from 'lucide-react';
import {
  Home, Download, Youtube, Music2, AudioLines, Clapperboard, Settings,
  MonitorPlay, Rows3, ListMusic, AudioWaveform, CalendarClock,
} from 'lucide-react';
import type { AudioFormat, VideoQuality, UiSource } from '@shared/types';
import { BASIC_LIMITS, LOSSLESS_FORMATS, VIDEO_QUALITIES as VIDEO_QUALITY_VALUES } from '@shared/types';

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

export interface PlatformDef {
  key: PlatformKey;
  label: string;
  /** Color token name → CSS var(--color-<accent>). */
  accent: PlatformKey;
  icon: LucideIcon;
}

export const PLATFORMS: readonly PlatformDef[] = [
  { key: 'youtube', label: 'YouTube', accent: 'youtube', icon: Youtube },
  { key: 'spotify', label: 'Spotify', accent: 'spotify', icon: Music2 },
  { key: 'soundcloud', label: 'SoundCloud', accent: 'soundcloud', icon: AudioLines },
  { key: 'reels', label: 'Reels & Shorts', accent: 'reels', icon: Clapperboard },
];

export interface NavItem {
  key: TabKey;
  label: string;
  icon: LucideIcon;
  group: 'Library' | 'Sources' | 'System';
}

export const NAV: readonly NavItem[] = [
  { key: 'home', label: 'Home', icon: Home, group: 'Library' },
  { key: 'queue', label: 'Downloads', icon: Download, group: 'Library' },
  { key: 'youtube', label: 'YouTube', icon: Youtube, group: 'Sources' },
  { key: 'spotify', label: 'Spotify', icon: Music2, group: 'Sources' },
  { key: 'soundcloud', label: 'SoundCloud', icon: AudioLines, group: 'Sources' },
  { key: 'reels', label: 'Reels & Shorts', icon: Clapperboard, group: 'Sources' },
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
  icon: LucideIcon;
  basic: string;
  premium: string;
}

export const PLAN_FEATURES: readonly PlanFeature[] = [
  {
    key: 'video',
    label: 'Video quality',
    icon: MonitorPlay,
    basic: `Up to ${BASIC_LIMITS.maxVideoQuality}`,
    premium: 'Up to 4K, or best available',
  },
  {
    key: 'batch',
    label: 'Batch paste',
    icon: Rows3,
    basic: `${BASIC_LIMITS.maxBatchLinks} links at a time`,
    premium: 'As many links as you paste',
  },
  {
    key: 'collections',
    label: 'Playlists & albums',
    icon: ListMusic,
    basic: `First ${BASIC_LIMITS.maxCollectionTracks} tracks`,
    premium: 'Every track',
  },
  {
    key: 'lossless',
    label: 'Lossless audio',
    icon: AudioWaveform,
    basic: 'Lossy formats only',
    premium: 'FLAC, WAV and ALAC',
  },
  {
    key: 'scheduler',
    label: 'Scheduled downloads',
    icon: CalendarClock,
    basic: 'Not included',
    premium: 'Run unattended, on your days',
  },
];
