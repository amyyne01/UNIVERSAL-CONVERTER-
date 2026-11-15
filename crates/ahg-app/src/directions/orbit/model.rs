//! What a task says, in the app's words: status groups, the source name and colour, the
//! label, the reason a download failed, the spec line. Pure functions over engine types.
use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode, SourcePlatform as Src};
use rune::Color;

use super::look::Pal;
use crate::copy;

/// Counts as active: queued, starting, downloading, converting, tagging, paused, retrying.
pub fn is_active(t: &DownloadTask) -> bool {
    !matches!(t.progress.status, St::Done | St::Failed | St::Cancelled)
}

/// Live: something is happening right now (not waiting, not paused).
pub fn is_live(t: &DownloadTask) -> bool {
    matches!(t.progress.status, St::FetchingInfo | St::Downloading | St::Converting | St::Embedding | St::Retrying)
}

pub fn is_finished(t: &DownloadTask) -> bool {
    matches!(t.progress.status, St::Done | St::Failed | St::Cancelled)
}

pub fn source(p: &Pal, src: Src) -> (&'static str, Color) {
    match src {
        Src::Youtube => (copy::SOURCE_YOUTUBE, p.youtube),
        Src::Spotify => (copy::SOURCE_SPOTIFY, p.spotify),
        Src::Soundcloud => (copy::SOURCE_SOUNDCLOUD, p.soundcloud),
        Src::Instagram => (copy::SOURCE_INSTAGRAM, p.reels),
        Src::Tiktok => (copy::SOURCE_TIKTOK, p.reels),
        Src::Facebook => (copy::SOURCE_FACEBOOK, p.reels),
        Src::Twitter => (copy::SOURCE_TWITTER, p.more(240.0, 0.020)),
        Src::Vimeo => (copy::SOURCE_VIMEO, p.more(if p.dark { 230.0 } else { 250.0 }, 0.130)),
        Src::Twitch => (copy::SOURCE_TWITCH, p.more(275.0, 0.130)),
        Src::Bandcamp => (copy::SOURCE_BANDCAMP, p.more(20.0, 0.130)),
        Src::Reddit => (copy::SOURCE_REDDIT, p.more(115.0, 0.130)),
        Src::Dailymotion => (copy::SOURCE_DAILYMOTION, p.more(330.0, 0.130)),
        Src::Mixcloud => (copy::SOURCE_MIXCLOUD, p.more(70.0, 0.130)),
        Src::Rumble => (copy::SOURCE_RUMBLE, p.more(170.0, 0.130)),
        Src::Direct => (copy::SOURCE_DIRECT, p.accent),
        Src::Generic => (copy::SOURCE_GENERIC, p.accent),
        Src::Unknown => (copy::SOURCE_UNKNOWN, p.accent),
    }
}

/// The part of a path after its last separator.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// `linkLabel`: the stand-in title of a task with none (CONTENT 4.7).
pub fn link_label(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else { return url.to_string() };
    let (host_path, query) = rest.split_once('?').map_or((rest, ""), |(a, b)| (a, b));
    let host_path = host_path.split('#').next().unwrap_or(host_path);
    let (host, path) = host_path.split_once('/').map_or((host_path, ""), |(h, p)| (h, p));
    let host = host.strip_prefix("www.").or_else(|| host.strip_prefix("m.")).unwrap_or(host);
    if let Some(v) = query.split('&').find_map(|kv| kv.strip_prefix("v=")).filter(|v| !v.is_empty()) {
        return format!("{host} · {v}");
    }
    let joined = format!("{host}/{path}");
    let joined = joined.trim_end_matches('/');
    if joined.chars().count() > 48 { format!("{}…", joined.chars().take(47).collect::<String>()) } else { joined.to_string() }
}

/// The title, else the saved file's name, else the link.
pub fn label(t: &DownloadTask) -> String {
    if !t.title.is_empty() {
        t.title.clone()
    } else if !t.progress.filename.is_empty() {
        file_name(&t.progress.filename).to_string()
    } else {
        link_label(&t.url)
    }
}

/// A kind that can never succeed on retry.
pub fn is_permanent(code: Option<ErrorCode>) -> bool {
    matches!(code, Some(ErrorCode::Removed | ErrorCode::Unsupported | ErrorCode::Drm | ErrorCode::GeoBlocked))
}

/// The readable sentence for a failed task's kind (CONTENT 3.7), with its two overrides.
fn sentence(code: ErrorCode, src: Src) -> String {
    use ErrorCode as E;
    if src == Src::Generic && code == E::Removed {
        return copy::ERR_GENERIC_DEAD_END.to_string();
    }
    let (cause, remedy) = match code {
        E::Private => (copy::ERR_PRIVATE_CAUSE, copy::ERR_PRIVATE_REMEDY),
        E::Removed => (copy::ERR_REMOVED_CAUSE, copy::ERR_REMOVED_REMEDY),
        E::AgeRestricted => (copy::ERR_AGE_RESTRICTED_CAUSE, copy::ERR_AGE_RESTRICTED_REMEDY),
        E::LoginRequired => (copy::ERR_LOGIN_REQUIRED_CAUSE, copy::ERR_LOGIN_REQUIRED_REMEDY),
        E::GeoBlocked => (copy::ERR_GEO_BLOCKED_CAUSE, copy::ERR_GEO_BLOCKED_REMEDY),
        E::Drm => (copy::ERR_DRM_CAUSE, copy::ERR_DRM_REMEDY),
        E::RateLimited => (copy::ERR_RATE_LIMITED_CAUSE, copy::ERR_RATE_LIMITED_REMEDY),
        E::Network => (copy::ERR_NETWORK_CAUSE, copy::ERR_NETWORK_REMEDY),
        E::Unsupported => (copy::ERR_UNSUPPORTED_CAUSE, copy::ERR_UNSUPPORTED_REMEDY),
        E::FfmpegMissing => (copy::ERR_FFMPEG_MISSING_CAUSE, copy::ERR_FFMPEG_MISSING_REMEDY),
        E::DiskFull => (copy::ERR_DISK_FULL_CAUSE, copy::ERR_DISK_FULL_REMEDY),
        E::Unknown => (copy::ERR_UNKNOWN_CAUSE, copy::ERR_UNKNOWN_REMEDY),
    };
    let remedy = if src == Src::Spotify && matches!(code, E::Unknown | E::Removed | E::Unsupported) { copy::ERR_SPOTIFY_REMEDY } else { remedy };
    let video = matches!(src, Src::Youtube | Src::Vimeo | Src::Dailymotion | Src::Rumble | Src::Twitch);
    copy::error_text(cause, remedy, video)
}

/// Why a failed or canceled task stopped, when it says.
pub fn reason(t: &DownloadTask) -> Option<String> {
    if !matches!(t.progress.status, St::Failed | St::Cancelled) {
        return None;
    }
    match t.error_code {
        Some(code) if t.progress.status == St::Failed => Some(sentence(code, t.source)),
        _ => Some(t.progress.error.clone()).filter(|e| !e.is_empty()),
    }
}

/// `[position] · [duration] · [size] · [format]`, the empty parts left out.
pub fn specs(t: &DownloadTask) -> String {
    let p = &t.progress;
    let size = if p.total > 0.0 { p.total } else { p.downloaded };
    let parts = [
        if t.is_playlist && p.playlist_total > 0.0 { format!("{}/{}", p.playlist_index, p.playlist_total) } else { String::new() },
        copy::format_duration(t.duration),
        if size > 0.0 { copy::format_bytes(size) } else { String::new() },
        t.format.clone(),
    ];
    parts.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
}

pub fn percent(t: &DownloadTask) -> f32 {
    if t.progress.status == St::Done { 1.0 } else { (t.progress.percent.clamp(0.0, 100.0) / 100.0) as f32 }
}

/// Full month name of `YYYY-MM`; anything else as given.
pub fn month_name(ym: &str) -> String {
    const NAMES: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    ym.split_once('-').and_then(|(_, m)| m.parse::<usize>().ok()).and_then(|m| NAMES.get(m.wrapping_sub(1))).map_or_else(|| ym.to_string(), |n| n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_labels_match_electron() {
        assert_eq!(link_label("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), "youtube.com · dQw4w9WgXcQ");
        assert_eq!(link_label("https://m.youtube.com/watch?v=abc"), "youtube.com · abc");
        assert_eq!(link_label("https://soundcloud.com/artist/track/"), "soundcloud.com/artist/track");
        assert_eq!(link_label("not a url"), "not a url");
        assert_eq!(month_name("2026-08"), "August");
        assert_eq!(month_name("abc"), "abc");
    }
}
