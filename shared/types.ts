// Single source of truth for cross-process domain types (and the runtime const
// arrays their unions derive from). Imported by both electron/ (main) and src/
// (renderer); the boundary whitelists in ipc.ts reuse these consts, so an enum
// added here can never drift from its validator.

export const SOURCE_PLATFORMS = [
  'youtube',
  'spotify',
  'soundcloud',
  'instagram',
  'tiktok',
  'facebook',
  'direct',
  'unknown',
] as const;
export type SourcePlatform = (typeof SOURCE_PLATFORMS)[number];

/** The four UI tabs media is grouped under. */
export type UiSource = 'youtube' | 'spotify' | 'soundcloud' | 'reels';

export type ContentType =
  | 'track'
  | 'video'
  | 'playlist'
  | 'album'
  | 'artist'
  | 'unknown';

export interface UrlDetection {
  url: string;
  platform: SourcePlatform;
  contentType: ContentType;
  /** Extracted identifier (video id, track id, playlist id, …). */
  id?: string;
  /** True for playlists/albums — a collection rather than a single item. */
  isCollection: boolean;
  /** Human label for the UI (e.g. "YouTube playlist"). */
  label: string;
}

export type DownloadStatus =
  | 'queued'
  | 'fetching_info'
  | 'downloading'
  | 'converting'
  | 'embedding'
  | 'done'
  | 'paused'
  | 'retrying'
  | 'failed'
  | 'cancelled';

export interface DownloadProgress {
  status: DownloadStatus;
  percent: number;
  /** bytes/sec */
  speed: number;
  /** seconds remaining */
  eta: number;
  downloaded: number;
  total: number;
  filename: string;
  error: string;
  playlistIndex: number;
  playlistTotal: number;
}

export type AudioFormat = 'mp3' | 'aac' | 'm4a' | 'ogg' | 'opus' | 'flac' | 'wav' | 'alac';

/** Audio codecs that carry no bitrate — quality is omitted for these (§5). */
export const LOSSLESS_FORMATS = ['flac', 'wav', 'alac'] as const;

export const VIDEO_QUALITIES = ['360p', '480p', '720p', '1080p', '2160p', 'best'] as const;
export type VideoQuality = (typeof VIDEO_QUALITIES)[number];

export interface DownloadTask {
  taskId: string;
  url: string;
  source: SourcePlatform;
  outputDir: string;
  format: string;
  /** Audio bitrate (e.g. "320") or "lossless". */
  quality: string;
  videoQuality: VideoQuality;
  isAudioOnly: boolean;
  isPlaylist: boolean;
  playlistName: string;
  /** Cap on items taken from a collection (basic tier); absent = no cap. */
  playlistLimit?: number;
  embedThumbnail: boolean;
  embedMetadata: boolean;
  skipExisting: boolean;
  title: string;
  thumbnailUrl: string;
  /** seconds */
  duration: number;
  uploader: string;
  progress: DownloadProgress;
  retryCount?: number;
  originalTaskId?: string;
  /** True when this task was cut off by an app exit and restored on boot —
   *  the Downloads tab asks the user to resume or discard it. */
  interrupted?: boolean;
  /** Epoch ms when the task was created / when it reached a terminal state.
   *  Optional because history persisted before these existed has neither —
   *  every consumer must tolerate their absence rather than showing "1970". */
  createdAt?: number;
  completedAt?: number;
  /** Scraped Spotify track (Spotify tasks only) — drives the ytsearch query + tagging (§4). */
  track?: Track;
  /** Spotify bridge (§11): scorer's confidence in the chosen ytsearch match (0..1). */
  matchConfidence?: number;
  /** Spotify bridge (§11): the direct video URL the scorer picked to download. */
  matchedUrl?: string;
}

export interface Track {
  id: string;
  name: string;
  artist: string;
  artists: string[];
  album: string;
  albumYear?: string;
  trackNumber: number;
  durationMs: number;
  thumbnailUrl: string;
  spotifyUrl?: string;
}

export interface Playlist {
  id: string;
  name: string;
  description: string;
  owner: string;
  tracks: Track[];
  thumbnailUrl: string;
  url: string;
  trackCount: number;
}

export interface SearchResult {
  id: string;
  title: string;
  uploader: string;
  duration: number;
  thumbnailUrl: string;
  url: string;
  source: SourcePlatform;
}

/** Normalized metadata returned by url:fetchMetadata; collections carry their entries. */
export interface MediaMetadata {
  title: string;
  uploader: string;
  duration: number;
  thumbnailUrl: string;
  url: string;
  isCollection: boolean;
  entries: SearchResult[];
  // Short-form preview fields (REDESIGN-PLAN §4.2). All optional and best-effort:
  // yt-dlp only reports them for some extractors, so every consumer must tolerate
  // an absent value. Nothing existing changes shape.
  description?: string;
  width?: number;
  height?: number;
  viewCount?: number;
  likeCount?: number;
  /** yt-dlp upload_date, 'YYYYMMDD' as reported. */
  uploadDate?: string;
  /** Which extractor actually resolved it — the short-form tab hosts several. */
  extractor?: string;
}

export type ThemeName = 'light' | 'dark' | 'system';

export interface AppConfig {
  /** Config-file shape version, used by the load() migration harness (electron/config.ts). */
  schemaVersion: number;
  outputDir: string;
  defaultFormat: string;
  defaultQuality: string;
  defaultVideoQuality: VideoQuality;
  embedThumbnail: boolean;
  embedMetadata: boolean;
  skipExisting: boolean;
  theme: ThemeName;
  rememberLastDir: boolean;
  autoPaste: boolean;
  showNotifications: boolean;
  /** Optional bandwidth cap, e.g. "2M"; empty = unlimited. */
  rateLimit: string;
  proxy: string;
  ffmpegPath: string;
  globalHotkey: string;
  discordRichPresence: boolean;
  // Scheduler
  scheduleEnabled: boolean;
  scheduleTime: string;
  scheduleDays: number[];
  scheduleShutdown: boolean;
  /** #26: allow the yt-dlp engine to self-update (weekly + on an extractor failure). */
  autoUpdateEngine: boolean;
}

// #26 yt-dlp engine self-update. `current`/`latest` are yt-dlp release tags
// ('2026.06.30'); empty when unknown (never checked / offline / not installed).
export interface YtdlpUpdateStatus {
  current: string;
  latest: string;
  state: 'idle' | 'checking' | 'available' | 'downloading' | 'updated' | 'uptodate' | 'error';
  message?: string;
}

// §12 app self-update. The events (update:available/progress/…) are the live feed;
// this is the same state readable at any moment. Both exist because the startup
// check fires before the lazily-mounted Settings tab has subscribed — without a
// readable state, an update found at boot would be invisible until the user
// happened to press Check again.
export interface UpdateState {
  phase: 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'uptodate' | 'error';
  /** The available/downloaded release, when one is known. */
  version?: string;
  /** Download progress, 0-100, while phase is 'downloading'. */
  percent?: number;
  message?: string;
}

// ── Tiers ─────────────────────────────────────────────────────────────────────
// 'basic' is the free default every install starts on; 'premium' is the one-time
// purchase. BASIC_LIMITS is the ONE definition of what free allows — the renderer
// reads it to lock controls, the main process reads it to clamp what actually runs.
export type Plan = 'basic' | 'premium';

export const BASIC_LIMITS = {
  /** Highest video quality basic may download; anything above is clamped to it. */
  maxVideoQuality: '1080p',
  /** Links a single batch paste may start. */
  maxBatchLinks: 5,
  /** Items taken from any playlist/album. */
  maxCollectionTracks: 20,
  /** Lossless codecs (FLAC/WAV/ALAC) are premium-only. */
  lossless: false,
  /** Scheduled/missed-run downloads are premium-only. */
  scheduler: false,
} as const;

/** True when `q` sits above the basic ceiling in the VIDEO_QUALITIES order. */
export function isPremiumVideoQuality(q: VideoQuality): boolean {
  return VIDEO_QUALITIES.indexOf(q) > VIDEO_QUALITIES.indexOf(BASIC_LIMITS.maxVideoQuality);
}

/** True for codecs basic may not use — the lossless set, unless basic is granted it. */
export function isPremiumFormat(format: string): boolean {
  return !BASIC_LIMITS.lossless && (LOSSLESS_FORMATS as readonly string[]).includes(format);
}

// What a request actually becomes on a given tier. The main process clamps with
// these before spawning; the renderer displays with them, so a saved default of
// "best" shows as 1080p on basic instead of promising what won't happen.
export function clampVideoQuality(q: VideoQuality, plan: Plan): VideoQuality {
  return plan === 'premium' || !isPremiumVideoQuality(q) ? q : BASIC_LIMITS.maxVideoQuality;
}

export function clampFormat(format: string, plan: Plan): string {
  return plan === 'premium' || !isPremiumFormat(format) ? format : 'mp3';
}

export function clampAudioQuality(quality: string, plan: Plan): string {
  return plan === 'premium' || quality !== 'lossless' ? quality : '320';
}

// ── Download statistics ───────────────────────────────────────────────────────
// Counted per calendar month in the main process, NOT derived from the download
// history — that history is capped and evicts, so a total built on it would start
// shrinking as the user kept downloading. `files` counts files actually written
// (a collection writes many), and byPlatform is by file count too, so one large
// video can't distort the split.
export interface MonthBucket {
  /** 'YYYY-MM', local time. */
  month: string;
  files: number;
  bytes: number;
  byPlatform: Partial<Record<SourcePlatform, number>>;
}

export interface DownloadStats {
  current: MonthBucket;
  /** The last month with activity, for the comparison — null on a first run. */
  previous: MonthBucket | null;
}

/** Outcome of revealing a downloaded file — 'missing' means it is no longer on disk. */
export interface RevealResult {
  ok: boolean;
  reason?: 'missing' | 'no-path';
}

export interface ActivationResult {
  success: boolean;
  error?: 'invalid' | 'already_activated' | 'network';
  message?: string;
  /** Tier the accepted key granted (absent when activation failed). */
  plan?: Plan;
}

export interface LicenseCheck {
  activated: boolean;
  /** Always present — an install with no key is a valid 'basic' user, not a locked one. */
  plan: Plan;
  machineId?: string;
  integrityFailed?: boolean;
  machineMismatch?: boolean;
}

// #21 self-service machine transfer. releaseLicense() frees the gist claim so the
// key can be re-activated elsewhere. 'not_bound' = this machine isn't the bound
// one (or has no key); 'cooldown' = within the 7-day transfer window (eligibleAt).
export interface ReleaseResult {
  success: boolean;
  error?: 'network' | 'not_bound' | 'cooldown';
  eligibleAt?: string;
}
