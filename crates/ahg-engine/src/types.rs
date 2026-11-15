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
/// second macro arm declares the all-optional twin used for partial input, so
/// the two field lists cannot drift apart.
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
            $(pub $field: Option<$ty>,)*
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
