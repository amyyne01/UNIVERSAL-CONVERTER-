//! Tiers. `Basic` is the free default every install starts on; `Premium` is the
//! one-time purchase. `BASIC_LIMITS` is the one definition of what free allows,
//! and the clamps say what a request actually becomes on a given tier. They
//! degrade a request instead of rejecting it.
use serde::{Deserialize, Serialize};

use crate::types::{CookieBrowser, DownloadExtras, LOSSLESS_FORMATS, PartialExtras, VideoQuality, merge_extras};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Plan {
    Basic,
    Premium,
}

/// Hard app ceiling on concurrent downloads, which is the premium cap. Above
/// about five the bottleneck is the connection, not the app.
pub const MAX_CONCURRENT_DOWNLOADS: u32 = 5;

pub struct BasicLimits {
    /// Highest video quality basic may download; anything above is clamped to it.
    pub max_video_quality: VideoQuality,
    /// Links a single batch paste may start.
    pub max_batch_links: u32,
    /// Items taken from any playlist or album.
    pub max_collection_tracks: u32,
    /// Concurrent download slots on the free tier.
    pub max_concurrent: u32,
    /// Lossless codecs (FLAC/WAV/ALAC) are premium-only.
    pub lossless: bool,
    /// Scheduled and missed-run downloads are premium-only.
    pub scheduler: bool,
    /// Subtitle languages a single download may request.
    pub max_subtitle_langs: usize,
    /// Machine-generated captions are premium-only (human ones are free).
    pub subtitle_auto: bool,
    /// Cookies (age-gated, private, members-only) are premium-only.
    pub cookies: bool,
    /// One file per chapter is premium-only; embedding chapter marks is free.
    pub split_chapters: bool,
}

pub const BASIC_LIMITS: BasicLimits = BasicLimits {
    max_video_quality: VideoQuality::P1080,
    max_batch_links: 5,
    max_collection_tracks: 20,
    max_concurrent: 2,
    lossless: false,
    scheduler: false,
    max_subtitle_langs: 1,
    subtitle_auto: false,
    cookies: false,
    split_chapters: false,
};

/// True when `q` sits above the basic ceiling.
pub fn is_premium_video_quality(q: VideoQuality) -> bool {
    q > BASIC_LIMITS.max_video_quality
}

/// True for codecs basic may not use: the lossless set, unless basic is granted it.
pub fn is_premium_format(format: &str) -> bool {
    !BASIC_LIMITS.lossless && LOSSLESS_FORMATS.contains(&format)
}

pub fn clamp_video_quality(q: VideoQuality, plan: Plan) -> VideoQuality {
    if plan == Plan::Premium || !is_premium_video_quality(q) { q } else { BASIC_LIMITS.max_video_quality }
}

pub fn clamp_format(format: &str, plan: Plan) -> &str {
    if plan == Plan::Premium || !is_premium_format(format) { format } else { "mp3" }
}

pub fn clamp_audio_quality(quality: &str, plan: Plan) -> &str {
    if plan == Plan::Premium || quality != "lossless" { quality } else { "320" }
}

/// Slots a plan may actually run. A hand-edited config asking for 20 gets the
/// tier ceiling, and junk (NaN, 0, -3) gets 1: never a queue that starts
/// nothing or everything.
pub fn clamp_concurrency(n: f64, plan: Plan) -> u32 {
    let cap = if plan == Plan::Premium { MAX_CONCURRENT_DOWNLOADS } else { BASIC_LIMITS.max_concurrent };
    // NaN and zero both count as "no usable number" and become 1.
    let n = if n.is_nan() || n == 0.0 { 1.0 } else { n.floor() };
    // The cast saturates, so infinities and huge values land on the bounds.
    (n.max(1.0) as u32).min(cap)
}

/// Cut a set of extras down to what `plan` is allowed to run. Basic asking for
/// five subtitle languages gets the first one, not an error.
pub fn clamp_extras(extras: Option<&PartialExtras>, plan: Plan) -> DownloadExtras {
    let full = merge_extras(extras);
    if plan == Plan::Premium {
        return full;
    }
    let langs = full
        .subtitle_langs
        .split(',')
        .map(|l| l.trim_matches(crate::text::is_js_whitespace))
        .filter(|l| !l.is_empty())
        .take(BASIC_LIMITS.max_subtitle_langs)
        .collect::<Vec<_>>()
        .join(",");
    DownloadExtras {
        subtitle_langs: if langs.is_empty() { DownloadExtras::default().subtitle_langs } else { langs },
        subtitle_auto: BASIC_LIMITS.subtitle_auto && full.subtitle_auto,
        split_chapters: BASIC_LIMITS.split_chapters && full.split_chapters,
        cookie_browser: if BASIC_LIMITS.cookies { full.cookie_browser } else { CookieBrowser::None },
        cookies_file: if BASIC_LIMITS.cookies { full.cookies_file } else { String::new() },
        ..full
    }
}
