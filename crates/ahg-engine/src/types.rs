//! Domain types shared by the whole engine (the Rust home of shared/types.ts).
use serde::{Deserialize, Serialize};

/// A download job's state. The serialised names are the ones Electron writes
/// to `downloads-history.json`, so both apps can read the same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Queued,
    FetchingInfo,
    Downloading,
    Converting,
    Embedding,
    Paused,
    Retrying,
    Failed,
    Done,
    Cancelled,
}

/// Where a link comes from. Names match `SOURCE_PLATFORMS` in shared/types.ts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourcePlatform {
    Youtube,
    Spotify,
    Soundcloud,
    Instagram,
    Tiktok,
    Facebook,
    Direct,
    Twitter,
    Vimeo,
    Twitch,
    Bandcamp,
    Reddit,
    Dailymotion,
    Mixcloud,
    Rumble,
    Generic,
    Unknown,
}

impl SourcePlatform {
    /// Display name, as in `PLATFORM_LABEL` / `MORE_SOURCE_LABELS`.
    pub fn label(self) -> &'static str {
        use SourcePlatform::*;
        match self {
            Youtube => "YouTube",
            Spotify => "Spotify",
            Soundcloud => "SoundCloud",
            Instagram => "Instagram",
            Tiktok => "TikTok",
            Facebook => "Facebook",
            Twitter => "X / Twitter",
            Vimeo => "Vimeo",
            Twitch => "Twitch",
            Bandcamp => "Bandcamp",
            Reddit => "Reddit",
            Dailymotion => "Dailymotion",
            Mixcloud => "Mixcloud",
            Rumble => "Rumble",
            Direct => "Direct",
            Generic => "Link",
            Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    Track,
    Video,
    Playlist,
    Album,
    Artist,
    Unknown,
}

impl ContentType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Video => "video",
            Self::Playlist => "playlist",
            Self::Album => "album",
            Self::Artist => "artist",
            Self::Unknown => "unknown",
        }
    }
}

/// What a pasted link was recognised as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlDetection {
    pub url: String,
    pub platform: SourcePlatform,
    pub content_type: ContentType,
    /// Extracted identifier (video id, track id, playlist id, hostname for a generic link).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub is_collection: bool,
    pub label: String,
}

/// Audio codecs that carry no bitrate; quality is omitted for these.
pub const LOSSLESS_FORMATS: [&str; 3] = ["flac", "wav", "alac"];

/// Retry backoff (5 s, 15 s, 45 s). Its length is the retry cap.
pub const RETRY_BACKOFF_MS: [u64; 3] = [5000, 15000, 45000];

/// Declared in ascending order, so comparing two values compares quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum VideoQuality {
    #[serde(rename = "360p")]
    P360,
    #[serde(rename = "480p")]
    P480,
    #[serde(rename = "720p")]
    P720,
    #[serde(rename = "1080p")]
    P1080,
    #[serde(rename = "2160p")]
    P2160,
    #[serde(rename = "best")]
    Best,
}

/// `File` writes a .srt beside the media, `Embed` muxes it in, `Both` does both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleMode {
    Off,
    Embed,
    File,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SponsorBlockMode {
    Off,
    Mark,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoContainer {
    Mp4,
    Mkv,
    Webm,
}

/// `Any` lets yt-dlp pick; the rest express a preference with a fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoCodec {
    Any,
    H264,
    Vp9,
    Av1,
}

/// `None` (the empty string on disk) means "don't touch cookies". The rest are
/// yt-dlp's `--cookies-from-browser` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CookieBrowser {
    #[serde(rename = "")]
    None,
    Chrome,
    Edge,
    Firefox,
    Brave,
    Chromium,
    Opera,
    Vivaldi,
}

/// Everything yt-dlp can do that isn't format or quality, as one object. The
/// macro also declares the all-optional twin used for partial input, so the
/// two field lists cannot drift apart.
macro_rules! extras {
    ($($(#[$doc:meta])* $field:ident: $ty:ty = $default:expr,)*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct DownloadExtras {
            $($(#[$doc])* pub $field: $ty,)*
        }

        impl Default for DownloadExtras {
            fn default() -> Self {
                Self { $($field: $default,)* }
            }
        }

        /// A possibly incomplete set of extras, as read from a persisted row.
        #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", default)]
        pub struct PartialExtras {
            $(#[serde(skip_serializing_if = "Option::is_none")] pub $field: Option<$ty>,)*
        }

        /// Complete a possibly partial set of extras from the defaults. Only
        /// known fields are read, so unknown keys never reach the result, and a
        /// missing value keeps its default.
        pub fn merge_extras(extras: Option<&PartialExtras>) -> DownloadExtras {
            let mut out = DownloadExtras::default();
            if let Some(e) = extras {
                $(if let Some(v) = &e.$field { out.$field = v.clone(); })*
            }
            out
        }
    };
}

extras! {
    subtitle_mode: SubtitleMode = SubtitleMode::Off,
    /// Comma-separated yt-dlp language list, e.g. "en,fr".
    subtitle_langs: String = "en".to_owned(),
    /// Include machine-generated captions when no human ones exist.
    subtitle_auto: bool = false,
    embed_chapters: bool = true,
    /// Write one file per chapter instead of one file.
    split_chapters: bool = false,
    sponsor_block: SponsorBlockMode = SponsorBlockMode::Off,
    sponsor_block_categories: Vec<String> = vec!["sponsor".to_owned()],
    /// Cover art as a separate image file, in addition to any embedded copy.
    write_thumbnail: bool = false,
    write_info_json: bool = false,
    write_description: bool = false,
    video_container: VideoContainer = VideoContainer::Mp4,
    video_codec: VideoCodec = VideoCodec::Any,
    cookie_browser: CookieBrowser = CookieBrowser::None,
    /// Absolute path of an imported cookies.txt; empty means none. It wins over
    /// `cookie_browser` when present.
    cookies_file: String = String::new(),
    /// yt-dlp output template; empty keeps the built-in title-based naming.
    output_template: String = String::new(),
    /// Bandwidth cap passed to `--limit-rate`, e.g. "2M"; empty means unlimited.
    rate_limit: String = String::new(),
    proxy: String = String::new(),
}

/// Numbers that come from yt-dlp or the network are `f64`, as in JavaScript.
/// Whole values are written without a fraction (`5`, not `5.0`), so the files
/// both apps share look the same whichever app wrote them.
pub mod num {
    use serde::Serializer;

    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        if v.fract() == 0.0 && v.abs() < 9_007_199_254_740_992.0 {
            s.serialize_i64(*v as i64)
        } else {
            s.serialize_f64(*v)
        }
    }

    pub fn option<S: Serializer>(v: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(v) => serialize(v, s),
            None => s.serialize_none(),
        }
    }
}

/// Why a download failed, in a form the UI can branch on. Set on failed tasks
/// beside the human-readable message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Private,
    Removed,
    AgeRestricted,
    LoginRequired,
    GeoBlocked,
    Drm,
    RateLimited,
    Network,
    Unsupported,
    FfmpegMissing,
    DiskFull,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub status: DownloadStatus,
    #[serde(serialize_with = "num::serialize")]
    pub percent: f64,
    /// Bytes per second.
    #[serde(serialize_with = "num::serialize")]
    pub speed: f64,
    /// Seconds remaining.
    #[serde(serialize_with = "num::serialize")]
    pub eta: f64,
    #[serde(serialize_with = "num::serialize")]
    pub downloaded: f64,
    #[serde(serialize_with = "num::serialize")]
    pub total: f64,
    pub filename: String,
    pub error: String,
    #[serde(serialize_with = "num::serialize")]
    pub playlist_index: f64,
    #[serde(serialize_with = "num::serialize")]
    pub playlist_total: f64,
    /// Done because the file was already on disk: nothing was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub already_on_disk: Option<bool>,
}

impl Default for DownloadProgress {
    fn default() -> Self {
        Self {
            status: DownloadStatus::Queued,
            percent: 0.0,
            speed: 0.0,
            eta: 0.0,
            downloaded: 0.0,
            total: 0.0,
            filename: String::new(),
            error: String::new(),
            playlist_index: 0.0,
            playlist_total: 0.0,
            already_on_disk: None,
        }
    }
}

/// A scraped Spotify track. It drives the search query and the tagging.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub artists: Vec<String>,
    pub album: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album_year: Option<String>,
    #[serde(serialize_with = "num::serialize")]
    pub track_number: f64,
    #[serde(serialize_with = "num::serialize")]
    pub duration_ms: f64,
    pub thumbnail_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spotify_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner: String,
    pub tracks: Vec<Track>,
    pub thumbnail_url: String,
    pub url: String,
    #[serde(serialize_with = "num::serialize")]
    pub track_count: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub uploader: String,
    #[serde(serialize_with = "num::serialize")]
    pub duration: f64,
    pub thumbnail_url: String,
    pub url: String,
    pub source: SourcePlatform,
}

/// Normalised metadata for a link; collections carry their entries. The
/// optional fields are best-effort, since yt-dlp reports them only for some sites.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadata {
    pub title: String,
    pub uploader: String,
    #[serde(serialize_with = "num::serialize")]
    pub duration: f64,
    pub thumbnail_url: String,
    pub url: String,
    pub is_collection: bool,
    pub entries: Vec<SearchResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "num::option")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "num::option")]
    pub height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "num::option")]
    pub view_count: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "num::option")]
    pub like_count: Option<f64>,
    /// yt-dlp's `upload_date`, "YYYYMMDD" as reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_date: Option<String>,
    /// Which extractor resolved it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extractor: Option<String>,
}

/// One download job, as queued, shown and saved in `downloads-history.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub task_id: String,
    pub url: String,
    pub source: SourcePlatform,
    pub output_dir: String,
    pub format: String,
    /// Audio bitrate (e.g. "320") or "lossless".
    pub quality: String,
    pub video_quality: VideoQuality,
    pub is_audio_only: bool,
    pub is_playlist: bool,
    pub playlist_name: String,
    /// Cap on items taken from a collection (basic tier); absent means no cap.
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "num::option")]
    pub playlist_limit: Option<f64>,
    /// yt-dlp `--playlist-items` spec, e.g. "1,3-5".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playlist_items: Option<String>,
    /// Set when the task fails; the raw message stays in `progress.error`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ErrorCode>,
    pub embed_thumbnail: bool,
    pub embed_metadata: bool,
    pub skip_existing: bool,
    pub title: String,
    pub thumbnail_url: String,
    /// Seconds.
    #[serde(serialize_with = "num::serialize")]
    pub duration: f64,
    pub uploader: String,
    pub progress: DownloadProgress,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "num::option")]
    pub retry_count: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_task_id: Option<String>,
    /// True when the task was cut off by an app exit and restored at launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupted: Option<bool>,
    /// Epoch milliseconds. Absent on rows saved before these existed.
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "num::option")]
    pub created_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "num::option")]
    pub completed_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<Track>,
    /// Spotify bridge: confidence in the chosen search match, 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "num::option")]
    pub match_confidence: Option<f64>,
    /// Spotify bridge: the video URL picked to download.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_url: Option<String>,
    /// Set by the queue on a retry after YouTube's token gate, never by the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_profile: Option<bool>,
    /// Subtitles, chapters, SponsorBlock, container, cookies and naming. Absent
    /// on rows saved before extras existed, which means "no extras".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<PartialExtras>,
    /// Keys this version does not know. They are kept and written back, so a
    /// history row never loses data by passing through this app.
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}
