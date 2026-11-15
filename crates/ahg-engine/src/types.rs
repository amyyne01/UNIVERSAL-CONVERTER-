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
