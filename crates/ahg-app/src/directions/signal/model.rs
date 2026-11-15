//! What the two screens read off the downloads: groups, counts, the reason a
//! row failed, and the formats that are not wording (timecode, link label).
//! Rules from `docs/design-gate/CONTENT.md` sections 3 and 4.
use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode, SourcePlatform};

use crate::copy;

pub fn is_active(t: &DownloadTask) -> bool {
    !matches!(t.progress.status, St::Done | St::Failed | St::Cancelled)
}

/// Transferring now: fetching_info, downloading, converting, embedding, retrying.
pub fn is_live(t: &DownloadTask) -> bool {
    matches!(t.progress.status, St::FetchingInfo | St::Downloading | St::Converting | St::Embedding | St::Retrying)
}

pub fn active_count(tasks: &[DownloadTask]) -> usize {
    tasks.iter().filter(|t| is_active(t)).count()
}

/// At least one row is paused and none is live.
pub fn all_paused(tasks: &[DownloadTask]) -> bool {
    tasks.iter().any(|t| t.progress.status == St::Paused) && !tasks.iter().any(is_live)
}

/// Summed speed of every row in `downloading`, bytes per second.
pub fn total_speed(tasks: &[DownloadTask]) -> f64 {
    tasks.iter().filter(|t| t.progress.status == St::Downloading).map(|t| t.progress.speed.max(0.0)).sum()
}

/// The source name printed on a row's detail line.
pub fn source_name(source: SourcePlatform) -> &'static str {
    use SourcePlatform::*;
    match source {
        Youtube => copy::SOURCE_YOUTUBE,
        Spotify => copy::SOURCE_SPOTIFY,
        Soundcloud => copy::SOURCE_SOUNDCLOUD,
        Instagram => copy::SOURCE_INSTAGRAM,
        Tiktok => copy::SOURCE_TIKTOK,
        Facebook => copy::SOURCE_FACEBOOK,
        Twitter => copy::SOURCE_TWITTER,
        Vimeo => copy::SOURCE_VIMEO,
        Twitch => copy::SOURCE_TWITCH,
        Bandcamp => copy::SOURCE_BANDCAMP,
        Reddit => copy::SOURCE_REDDIT,
        Dailymotion => copy::SOURCE_DAILYMOTION,
        Mixcloud => copy::SOURCE_MIXCLOUD,
        Rumble => copy::SOURCE_RUMBLE,
        Direct => copy::SOURCE_DIRECT,
        Generic => copy::SOURCE_GENERIC,
        Unknown => copy::SOURCE_UNKNOWN,
    }
}

pub fn is_permanent(code: Option<ErrorCode>) -> bool {
    matches!(code, Some(ErrorCode::Removed | ErrorCode::Unsupported | ErrorCode::Drm | ErrorCode::GeoBlocked))
}

/// The readable reason on a failed or cancelled row (3.7), or the raw text when the row has no kind.
pub fn reason(t: &DownloadTask) -> Option<String> {
    let st = t.progress.status;
    if !matches!(st, St::Failed | St::Cancelled) {
        return None;
    }
    let Some(code) = t.error_code.filter(|_| st == St::Failed) else {
        let raw = t.progress.error.trim();
        return (!raw.is_empty()).then(|| raw.to_string());
    };
    use ErrorCode::*;
    if t.source == SourcePlatform::Generic && code == Removed {
        return Some(copy::ERR_GENERIC_DEAD_END.to_string());
    }
    let (cause, mut remedy) = match code {
        Private => (copy::ERR_PRIVATE_CAUSE, copy::ERR_PRIVATE_REMEDY),
        Removed => (copy::ERR_REMOVED_CAUSE, copy::ERR_REMOVED_REMEDY),
        AgeRestricted => (copy::ERR_AGE_RESTRICTED_CAUSE, copy::ERR_AGE_RESTRICTED_REMEDY),
        LoginRequired => (copy::ERR_LOGIN_REQUIRED_CAUSE, copy::ERR_LOGIN_REQUIRED_REMEDY),
        GeoBlocked => (copy::ERR_GEO_BLOCKED_CAUSE, copy::ERR_GEO_BLOCKED_REMEDY),
        Drm => (copy::ERR_DRM_CAUSE, copy::ERR_DRM_REMEDY),
        RateLimited => (copy::ERR_RATE_LIMITED_CAUSE, copy::ERR_RATE_LIMITED_REMEDY),
        Network => (copy::ERR_NETWORK_CAUSE, copy::ERR_NETWORK_REMEDY),
        Unsupported => (copy::ERR_UNSUPPORTED_CAUSE, copy::ERR_UNSUPPORTED_REMEDY),
        FfmpegMissing => (copy::ERR_FFMPEG_MISSING_CAUSE, copy::ERR_FFMPEG_MISSING_REMEDY),
        DiskFull => (copy::ERR_DISK_FULL_CAUSE, copy::ERR_DISK_FULL_REMEDY),
        Unknown => (copy::ERR_UNKNOWN_CAUSE, copy::ERR_UNKNOWN_REMEDY),
    };
    if t.source == SourcePlatform::Spotify && matches!(code, Unknown | Removed | Unsupported) {
        remedy = copy::ERR_SPOTIFY_REMEDY;
    }
    let video = matches!(t.source, SourcePlatform::Youtube | SourcePlatform::Vimeo | SourcePlatform::Dailymotion | SourcePlatform::Rumble | SourcePlatform::Twitch);
    Some(copy::error_text(cause, remedy, video))
}

/// `linkLabel(url)` (4.7): host without `www.`/`m.`, the `v` parameter when there is one, else host + path.
pub fn link_label(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else { return url.to_string() };
    let (before_query, query) = rest.split_once('?').map_or((rest, ""), |(a, q)| (a, q));
    let (host, path) = before_query.split_once('/').map_or((before_query, ""), |(h, p)| (h, p));
    let host = host.strip_prefix("www.").or_else(|| host.strip_prefix("m.")).unwrap_or(host);
    if let Some(v) = query.split('&').find_map(|kv| kv.strip_prefix("v=")) {
        return format!("{host} · {v}");
    }
    let path = path.trim_end_matches('/');
    let label = if path.is_empty() { host.to_string() } else { format!("{host}/{path}") };
    if label.chars().count() > 48 { format!("{}…", label.chars().take(47).collect::<String>()) } else { label }
}

/// The row's name: the title, else the saved file's name, else the link label.
pub fn label(t: &DownloadTask) -> String {
    if !t.title.trim().is_empty() {
        return t.title.clone();
    }
    let file = file_name(&t.progress.filename);
    if !file.is_empty() { file.to_string() } else { link_label(&t.url) }
}

/// The last part of a Windows or POSIX path.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// The folder part of a path, without the file.
pub fn folder(path: &str) -> &str {
    path.rfind(['\\', '/']).map_or("", |at| &path[..at])
}

/// Signal's one signature: the tape counter. Media time carried across so far
/// (`duration × percent`), as `hh:mm:ss`. Only in-flight rows on Downloads show it.
pub fn timecode(seconds: f64) -> String {
    let s = if seconds.is_finite() { seconds.max(0.0) as u64 } else { 0 };
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// Full month name for `YYYY-MM`; an unparsable value is shown as is (4.6).
/// The shared `copy` has no helper for this, so the English names live here.
pub fn month_name(ym: &str) -> String {
    const NAMES: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    ym.split_once('-')
        .and_then(|(_, m)| m.parse::<usize>().ok())
        .and_then(|m| NAMES.get(m.wrapping_sub(1)))
        .map_or_else(|| ym.to_string(), |n| n.to_string())
}

/// The three list groups, in order (3.4).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    InProgress,
    Attention,
    Completed,
}

pub fn group_of(t: &DownloadTask) -> Group {
    match t.progress.status {
        St::Failed | St::Cancelled => Group::Attention,
        St::Done => Group::Completed,
        _ => Group::InProgress,
    }
}

/// `{n} {word}` parts for the summary line, fixed order, zeros left out.
pub fn summary_parts(tasks: &[DownloadTask]) -> Vec<String> {
    let count = |f: &dyn Fn(St) -> bool| tasks.iter().filter(|t| f(t.progress.status)).count();
    let counts = [
        count(&|s| matches!(s, St::FetchingInfo | St::Downloading | St::Converting | St::Embedding)),
        count(&|s| s == St::Retrying),
        count(&|s| s == St::Queued),
        count(&|s| s == St::Paused),
        count(&|s| s == St::Done),
        count(&|s| s == St::Failed),
        count(&|s| s == St::Cancelled),
    ];
    counts.iter().zip(copy::SUMMARY_WORDS).filter(|(n, _)| **n > 0).map(|(n, w)| format!("{n} {w}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(link_label("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), "youtube.com · dQw4w9WgXcQ");
        assert_eq!(link_label("https://soundcloud.com/artist/track/"), "soundcloud.com/artist/track");
        assert_eq!(link_label("not a url"), "not a url");
        assert_eq!(timecode(228.6), "00:03:48");
        assert_eq!(month_name("2026-11"), "November");
        assert_eq!(month_name("abc"), "abc");
        assert_eq!(folder("C:\\Users\\x\\Music\\a.mp3"), "C:\\Users\\x\\Music");
    }
}
