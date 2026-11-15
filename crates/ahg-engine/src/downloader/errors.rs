//! Turning yt-dlp's complaint into a decision and a sentence: whether to
//! retry, why it failed, what to tell the user. Ports classifyError,
//! needsClientFallback, errorCodeOf and explainError from electron/downloader.ts.
use crate::text::{is_js_whitespace, js_trim, utf16_prefix};
use crate::types::ErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    Retryable,
    Permanent,
    Auth,
    Geo,
}

const TIMEOUT_MESSAGE: &str = "Timed out reading that link \u{2014} the site did not answer in time.";

// D30 (Rust-only behaviour change, owner-approved): the owner-named loose words "geo",
// "region" and "private", which matched anywhere in the text (paths, playlist names), are
// now phrases quoted from yt-dlp's source (master). "drm" is tightened the same way. The other
// loose-looking needles (removed, deleted, copyright, not found, network, timeout, temporar)
// were deliberately left as they were: no quoted replacement keeps every real message.
//   yt_dlp/extractor/common.py          raise_geo_restricted default message, "Video is geo restricted",
//                                       report_drm "This video is DRM protected"
//   yt_dlp/extractor/unsupported.py     "The requested site is known to use DRM protection"
//   yt_dlp/YoutubeDL.py                 GeoRestrictedError hint "use a VPN or a proxy server"
//   yt_dlp/extractor/youtube/_video.py  "The uploader has not made this video available in your country";
//                                       reason "Private video" (+ ". " + subreason)
//   yt_dlp/extractor/youtube/_tab.py    "not available in your region", "This playlist is private"
//   yt_dlp/extractor/tiktok.py          "account is private", "either private or"
// A stray word now falls through to the default: Permanent / Unknown, shown with yt-dlp's own text.

const AUTH: &[&str] = &[
    "sign in",
    "login required",
    "members-only",
    "members only",
    "age-restricted",
    "age restricted",
    "confirm your age",
    "logged-in",
    "logged in",
    "authentication is required",
];
const PRIVATE: &[&str] = &["private video. ", "playlist is private", "account is private", "either private or"];
const GEO: &[&str] = &[
    "geo restriction",
    "geo restricted",
    "not available from your location",
    "not available in your country",
    "made this video available in your country",
    "not available in your region",
    "use a vpn or a proxy server",
];
const PERMANENT: &[&str] = &[
    "is unavailable",
    "video unavailable",
    "no longer available",
    "removed",
    "deleted",
    "does not exist",
    "this video is not available",
    "copyright",
    "drm protect",
    "is not a valid url",
    "unsupported url",
    "http error 404",
    "http error 410",
    "live event will begin",
    "premieres in",
    "this live stream recording is not available",
];
const RETRYABLE: &[&str] = &[
    "timed out",
    "timeout",
    "temporar",
    "http error 5",
    "too many requests",
    "connection reset",
    "reset by peer",
    "connection refused",
    "network",
    "read error",
    "unable to download",
];
/// Text meaning the player client's streams will not serve to us (the PO-token gate).
const GATED_SIGNS: &[&str] = &["403: forbidden", "requested format is not available", "page needs to be reloaded"];

/// Needles per code, first match wins.
const ERROR_CODE_NEEDLES: &[(ErrorCode, &[&str])] = &[
    (
        ErrorCode::FfmpegMissing,
        &["ffmpeg is not installed", "ffmpeg not found", "ffprobe and ffmpeg not found", "ffmpeg-location"],
    ),
    (ErrorCode::DiskFull, &["no space left", "disk full", "enospc", "not enough space"]),
    (ErrorCode::Drm, &["drm protect"]),
    (ErrorCode::AgeRestricted, &["age-restricted", "age restricted", "confirm your age"]),
    (ErrorCode::Private, PRIVATE),
    (
        ErrorCode::LoginRequired,
        &[
            "sign in",
            "login required",
            "members-only",
            "members only",
            "logged-in",
            "logged in",
            "authentication is required",
        ],
    ),
    (ErrorCode::GeoBlocked, GEO),
    (ErrorCode::RateLimited, &["too many requests", "http error 429", "rate-limit", "rate limit"]),
    (ErrorCode::Unsupported, &["unsupported url", "is not a valid url"]),
    (
        ErrorCode::Removed,
        &[
            "http error 404",
            "http error 410",
            "removed",
            "deleted",
            "does not exist",
            "no longer available",
            "video unavailable",
            "is unavailable",
            "this video is not available",
            "copyright",
            "not found",
        ],
    ),
];

fn has_any(t: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| t.contains(n))
}

/// yt-dlp prints YouTube's bare reason "Private video" when there is no subreason. A plain
/// substring would also match "Private Videos" in a path, so it must end a line.
fn ends_with_private_video(t: &str) -> bool {
    t.lines().any(|l| l.trim_end().ends_with("private video"))
}

/// Whether to retry (§8). Conservative: auth, then geo, then permanent, then
/// retryable (which includes the gated signs); anything unlisted is permanent.
pub fn classify_error(text: &str) -> ErrorClass {
    let t = text.to_lowercase();
    if has_any(&t, AUTH) || has_any(&t, PRIVATE) || ends_with_private_video(&t) {
        ErrorClass::Auth
    } else if has_any(&t, GEO) {
        ErrorClass::Geo
    } else if has_any(&t, PERMANENT) {
        ErrorClass::Permanent
    } else if has_any(&t, RETRYABLE) || has_any(&t, GATED_SIGNS) {
        ErrorClass::Retryable
    } else {
        ErrorClass::Permanent
    }
}

/// True when the text is the PO-token gate, which only a different player client fixes.
pub fn needs_client_fallback(text: &str) -> bool {
    has_any(&text.to_lowercase(), GATED_SIGNS)
}

/// Why a download failed, for the UI to branch on.
pub fn error_code_of(text: &str) -> ErrorCode {
    let t = text.to_lowercase();
    for (code, needles) in ERROR_CODE_NEEDLES {
        if has_any(&t, needles) || (*code == ErrorCode::Private && ends_with_private_video(&t)) {
            return *code;
        }
    }
    if classify_error(&t) == ErrorClass::Retryable { ErrorCode::Network } else { ErrorCode::Unknown }
}

/// One sentence a user can act on, from yt-dlp's stderr.
pub fn explain_error(text: &str) -> String {
    let raw = js_trim(text);
    if raw.is_empty() {
        return "That link could not be read.".to_owned();
    }
    if raw.starts_with(TIMEOUT_MESSAGE) {
        return TIMEOUT_MESSAGE.to_owned();
    }
    let t = raw.to_lowercase();
    if needs_client_fallback(&t) {
        return "The site refused to serve this file \u{2014} try again later, or import cookies in Settings.".to_owned();
    }
    match classify_error(&t) {
        ErrorClass::Auth => "That post needs a sign-in \u{2014} it may be private, age-restricted or members-only.".to_owned(),
        ErrorClass::Geo => "That post is not available in your region.".to_owned(),
        ErrorClass::Retryable => "The site did not answer \u{2014} check your connection and try again.".to_owned(),
        ErrorClass::Permanent => {
            if t.contains("live event will begin") || t.contains("premieres in") {
                return "That video has not premiered yet \u{2014} try again once it is live.".to_owned();
            }
            match error_code_of(&t) {
                ErrorCode::Removed => {
                    "Nothing was found at that link \u{2014} it may have been removed, or the address is wrong.".to_owned()
                }
                ErrorCode::Unknown => {
                    let last = raw.split('\n').rfind(|l| !js_trim(l).is_empty()).unwrap_or(raw);
                    // /^\s*ERROR:\s*/
                    let start = last.trim_start_matches(is_js_whitespace);
                    let detail = start.strip_prefix("ERROR:").map_or(last, |r| r.trim_start_matches(is_js_whitespace));
                    format!("The download stopped with an error: {}", utf16_prefix(js_trim(detail), 200))
                }
                _ => "That post could not be read \u{2014} it may be private, removed, or need a sign-in.".to_owned(),
            }
        }
    }
}
