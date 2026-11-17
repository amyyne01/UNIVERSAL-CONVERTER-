//! How numbers and names are written, and the facts read off a task or the
//! statistics, the same in all six presets. The outputs are the Electron app's
//! (`docs/design-gate/CONTENT.md` section 4, `src/lib/format.ts`, `errors.ts`,
//! `DownloadsTab.tsx`); the tests below are that section's tables.
use ahg_engine::downloader::errors;
use ahg_engine::stats::DownloadStats;
use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode, RETRY_BACKOFF_MS, SourcePlatform};

use crate::copy;

/// `formatBytes`: 1024-based, no decimals for bytes and one from KB up, `—` for nothing.
pub fn bytes(n: f64) -> String {
    if n <= 0.0 {
        return "—".into();
    }
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let (mut value, mut unit) = (n, 0);
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        return format!("{value} B");
    }
    // JavaScript's `toFixed(1)` rounds an exact tie up (1.25 is 1.3); Rust rounds it to even.
    let tie = (value * 4.0).fract() == 0.0 && (value * 2.0).fract() != 0.0;
    format!("{:.1} {}", if tie { value + 0.01 } else { value }, UNITS[unit])
}

/// `fmtSpeed`: `2.4 MB/s`; empty when nothing is moving.
pub fn speed(bytes_per_second: f64) -> String {
    if bytes_per_second > 0.0 { format!("{}/s", bytes(bytes_per_second)) } else { String::new() }
}

/// `formatDuration`: `m:ss`, or `h:mm:ss` from an hour; empty for zero or NaN; fractions are floored.
pub fn duration(seconds: f64) -> String {
    if seconds == 0.0 || seconds.is_nan() {
        return String::new();
    }
    let h = (seconds / 3600.0).floor() as i64;
    let m = ((seconds % 3600.0) / 60.0).floor() as i64;
    let s = (seconds % 60.0).floor() as i64;
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// `fmtEta`: time left; empty when unknown.
pub fn eta(seconds: f64) -> String {
    if seconds.is_finite() && seconds > 0.0 { duration(seconds) } else { String::new() }
}

/// Speed and time left joined with a middle dot; `–` when neither is known.
pub fn rate(bytes_per_second: f64, eta_seconds: f64) -> String {
    let parts: Vec<String> = [speed(bytes_per_second), eta(eta_seconds)].into_iter().filter(|p| !p.is_empty()).collect();
    if parts.is_empty() { copy::RATE_UNKNOWN.into() } else { parts.join(" · ") }
}

/// A whole percent, 0 to 100. A finished download is 100.
pub fn percent(t: &DownloadTask) -> u32 {
    if t.progress.status == St::Done { 100 } else { round_half_up(t.progress.percent.clamp(0.0, 100.0)) }
}

/// JavaScript's `Math.round` for a value that is not negative.
pub fn round_half_up(value: f64) -> u32 {
    (value + 0.5).floor() as u32
}

/// `2026-11` to `November`; anything else comes back unchanged.
pub fn month_name(year_month: &str) -> String {
    const NAMES: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    year_month
        .split_once('-')
        .and_then(|(_, m)| m.parse::<usize>().ok())
        .and_then(|m| NAMES.get(m.wrapping_sub(1)))
        .map_or_else(|| year_month.to_string(), |n| (*n).to_string())
}

/// `linkLabel`: the stand-in title for a download that has none yet. What `new URL()` gives in
/// Electron: trimmed, no fragment, no user or port, lower-case host without `www.`/`m.`, the first
/// `v` parameter if it is not empty, else host and path without trailing slashes; not a URL, unchanged.
// ponytail: no percent-decoding of `v` and no IDNA; add if a real link needs it.
pub fn link_label(url: &str) -> String {
    let Some((scheme, rest)) = url.trim().split_once("://") else { return url.to_string() };
    if !scheme.starts_with(|c: char| c.is_ascii_alphabetic()) || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
        return url.to_string();
    }
    let rest = rest.split('#').next().unwrap_or(rest);
    let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let host = host.rsplit_once(':').filter(|(_, port)| port.bytes().all(|b| b.is_ascii_digit())).map_or(host, |(h, _)| h).to_lowercase();
    if host.is_empty() {
        return url.to_string();
    }
    let host = host.strip_prefix("www.").or_else(|| host.strip_prefix("m.")).unwrap_or(&host);
    let v = query.split('&').map(|pair| pair.split_once('=').unwrap_or((pair, ""))).find(|(name, _)| *name == "v").map_or("", |(_, v)| v);
    if !v.is_empty() {
        return format!("{host} · {v}");
    }
    let label = format!("{host}{}", path.trim_end_matches('/'));
    if label.chars().count() > 48 { format!("{}…", label.chars().take(47).collect::<String>()) } else { label }
}

/// The last part of a Windows or POSIX path.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// The folder part of a path, without the file.
pub fn folder(path: &str) -> &str {
    path.rfind(['\\', '/']).map_or("", |at| &path[..at])
}

/// A download's name: its title, else its file's name, else the link label.
pub fn label(t: &DownloadTask) -> String {
    if !t.title.is_empty() {
        return t.title.clone();
    }
    let file = file_name(&t.progress.filename);
    if file.is_empty() { link_label(&t.url) } else { file.to_string() }
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

/// A failure kind no retry can fix (the engine's `isPermanentError`).
pub fn is_permanent(code: Option<ErrorCode>) -> bool {
    code.is_some_and(errors::is_permanent)
}

/// The readable reason on a failed or canceled row (3.7), or the raw text when the row has no kind.
// TODO(engine-export): `describeError`'s sentences live in copy.rs only; the engine has no table.
pub fn reason(t: &DownloadTask) -> Option<String> {
    use ErrorCode::*;
    let status = t.progress.status;
    if !matches!(status, St::Failed | St::Cancelled) {
        return None;
    }
    let Some(code) = t.error_code.filter(|_| status == St::Failed) else {
        return Some(t.progress.error.clone()).filter(|e| !e.is_empty());
    };
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
    let video = matches!(
        t.source,
        SourcePlatform::Youtube | SourcePlatform::Vimeo | SourcePlatform::Dailymotion | SourcePlatform::Rumble | SourcePlatform::Twitch
    );
    Some(copy::error_text(cause, remedy, video))
}

/// The one-click way out of a failed or canceled row's failure, if it has one (`fixFor`).
pub fn fix(t: &DownloadTask) -> Option<errors::Fix> {
    matches!(t.progress.status, St::Failed | St::Cancelled).then_some(t.error_code).flatten().and_then(errors::fix_for)
}

/// `[playlist position] · [duration] · [size] · [format]`, the empty parts left out.
pub fn specs(t: &DownloadTask) -> String {
    let p = &t.progress;
    let size = if p.total > 0.0 { p.total } else { p.downloaded };
    [
        if t.is_playlist && p.playlist_total > 0.0 { format!("{}/{}", p.playlist_index, p.playlist_total) } else { String::new() },
        duration(t.duration),
        if size > 0.0 { bytes(size) } else { String::new() },
        t.format.clone(),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" · ")
}

/// Whole seconds a retrying row waits before its next try: the queue's backoff for its retry count.
pub fn retry_wait(t: &DownloadTask) -> u64 {
    let at = (t.retry_count.unwrap_or(0.0).max(0.0) as usize).min(RETRY_BACKOFF_MS.len() - 1);
    RETRY_BACKOFF_MS[at].div_ceil(1000)
}

/// Not finished one way or another: still counted as active.
pub fn is_active(t: &DownloadTask) -> bool {
    !matches!(t.progress.status, St::Done | St::Failed | St::Cancelled)
}

/// Transferring now.
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

/// The live rows and their speeds summed, in bytes per second (`laneSpeed`).
pub fn in_flight(tasks: &[DownloadTask]) -> (usize, f64) {
    tasks.iter().filter(|t| is_live(t)).fold((0, 0.0), |(n, speed), t| (n + 1, speed + t.progress.speed))
}

/// The summed speed of the live rows, in bytes per second.
pub fn total_speed(tasks: &[DownloadTask]) -> f64 {
    in_flight(tasks).1
}

/// The counts on the four filters: all, active, done, failed (failed holds canceled too).
pub fn filter_counts(tasks: &[DownloadTask]) -> [usize; 4] {
    let count = |wanted: &dyn Fn(St) -> bool| tasks.iter().filter(|t| wanted(t.progress.status)).count();
    [tasks.len(), active_count(tasks), count(&|s| s == St::Done), count(&|s| matches!(s, St::Failed | St::Cancelled))]
}

/// The next of 1, 2, 5 times a power of ten at or above `v`; 1 for anything up to 1.
pub fn axis_ceiling(v: f64) -> f64 {
    if v.is_nan() || v <= 1.0 {
        return 1.0;
    }
    let mut step = 10f64.powi(v.log10().floor() as i32);
    loop {
        for k in [1.0, 2.0, 5.0] {
            if k * step >= v {
                return k * step;
            }
        }
        step *= 10.0;
    }
}

/// The four groups on the rail plus the rest, as Home's "where it came from" shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Youtube,
    Spotify,
    Soundcloud,
    Reels,
    Other,
}

impl Bucket {
    pub fn name(self) -> &'static str {
        match self {
            Bucket::Youtube => copy::SPLIT_YOUTUBE,
            Bucket::Spotify => copy::SPLIT_SPOTIFY,
            Bucket::Soundcloud => copy::SPLIT_SOUNDCLOUD,
            Bucket::Reels => copy::SPLIT_REELS,
            Bucket::Other => copy::SPLIT_OTHER,
        }
    }
}

/// This month's files by group, biggest first, ties in the order each group first appeared (`splitRows`).
pub fn source_split(stats: &DownloadStats) -> Vec<(Bucket, u64)> {
    let mut rows: Vec<(Bucket, u64)> = Vec::new();
    for (key, n) in &stats.current.by_platform {
        let n = n.as_f64().unwrap_or(0.0).max(0.0) as u64;
        if n == 0 {
            continue;
        }
        let bucket = match key.as_str() {
            "youtube" => Bucket::Youtube,
            "spotify" => Bucket::Spotify,
            "soundcloud" => Bucket::Soundcloud,
            "instagram" | "tiktok" | "facebook" => Bucket::Reels,
            _ => Bucket::Other,
        };
        match rows.iter_mut().find(|row| row.0 == bucket) {
            Some(row) => row.1 += n,
            None => rows.push((bucket, n)),
        }
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.1));
    rows
}

/// The sentence under the month's figure: the comparison with the last active month, or, when this
/// month is empty, what last month kept.
pub fn month_sentence(stats: &DownloadStats) -> String {
    let files = stats.current.files.max(0.0) as u64;
    match (files, stats.previous.as_ref().filter(|p| p.files > 0.0)) {
        (0, Some(p)) => copy::month_last_month_kept(p.files as u64, &bytes(p.bytes)),
        (0, None) => copy::MONTH_NOTHING_YET_NO_PREV.to_string(),
        (_, None) => copy::MONTH_FIRST_ON_RECORD.to_string(),
        (n, Some(p)) => {
            let (before, name) = (p.files as u64, month_name(&p.month));
            if n == before { copy::month_same_count(&name) } else { copy::month_diff(n as i64 - before as i64, &name) }
        }
    }
}

/// The three groups of the list, in the order they are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// `3 downloading`, `1 retrying` and so on for the summary line: fixed order, zeros left out.
pub fn summary_parts(tasks: &[DownloadTask]) -> Vec<String> {
    let count = |wanted: &dyn Fn(St) -> bool| tasks.iter().filter(|t| wanted(t.progress.status)).count();
    let counts = [
        count(&|s| matches!(s, St::FetchingInfo | St::Downloading | St::Converting | St::Embedding)),
        count(&|s| s == St::Retrying),
        count(&|s| s == St::Queued),
        count(&|s| s == St::Paused),
        count(&|s| s == St::Done),
        count(&|s| s == St::Failed),
        count(&|s| s == St::Cancelled),
    ];
    counts.iter().zip(copy::SUMMARY_WORDS).filter(|(n, _)| **n > 0).map(|(n, word)| format!("{n} {word}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_match_the_old_app() {
        let table = [
            (0.0, "—"),
            (-5.0, "—"),
            (1.0, "1 B"),
            (512.0, "512 B"),
            (1023.0, "1023 B"),
            (1024.0, "1.0 KB"),
            (1536.0, "1.5 KB"),
            (1_048_575.0, "1024.0 KB"),
            (1_048_576.0, "1.0 MB"),
            (3_565_158.0, "3.4 MB"),
            (13_002_342.0, "12.4 MB"),
            (1_073_741_824.0, "1.0 GB"),
            (1_649_267_441_664.0, "1.5 TB"),
            (512.5, "512.5 B"),
            (f64::NAN, "NaN B"),
            (1280.0, "1.3 KB"),
            (1_310_720.0, "1.3 MB"),
        ];
        assert!(!table.is_empty());
        for (input, want) in table {
            assert_eq!(bytes(input), want, "bytes({input})");
        }
    }

    #[test]
    fn speeds_times_and_rates_match_the_old_app() {
        assert_eq!(speed(0.0), "");
        assert_eq!(speed(2_500_000.0), "2.4 MB/s");
        assert_eq!(speed(524_288.0), "512.0 KB/s");
        let durations = [
            (0.0, ""),
            (5.0, "0:05"),
            (0.5, "0:00"),
            (59.9, "0:59"),
            (187.0, "3:07"),
            (600.0, "10:00"),
            (3599.0, "59:59"),
            (3600.0, "1:00:00"),
            (3723.0, "1:02:03"),
            (36000.0, "10:00:00"),
        ];
        assert!(!durations.is_empty());
        for (input, want) in durations {
            assert_eq!(duration(input), want, "duration({input})");
        }
        assert_eq!(duration(f64::NAN), "");
        for input in [0.0, -1.0, f64::INFINITY] {
            assert_eq!(eta(input), "");
        }
        assert_eq!(eta(3725.0), "1:02:05");
        assert_eq!(rate(2_500_000.0, 75.0), "2.4 MB/s · 1:15");
        assert_eq!(rate(2_500_000.0, 0.0), "2.4 MB/s");
        assert_eq!(rate(0.0, 75.0), "1:15");
        assert_eq!(rate(0.0, 0.0), "–");
    }

    #[test]
    fn percents_months_and_link_labels_match_the_old_app() {
        let rounds = [(47.4, 47), (47.5, 48), (99.5, 100), (0.0, 0)];
        assert!(!rounds.is_empty());
        for (input, want) in rounds {
            assert_eq!(round_half_up(input), want);
        }
        assert_eq!(month_name("2026-08"), "August");
        assert_eq!(month_name("2026-11"), "November");
        assert_eq!(month_name("abc"), "abc");
        assert_eq!(link_label("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), "youtube.com · dQw4w9WgXcQ");
        assert_eq!(link_label("https://m.youtube.com/watch?v=abc"), "youtube.com · abc");
        assert_eq!(link_label("https://soundcloud.com/artist/track/"), "soundcloud.com/artist/track");
        assert_eq!(link_label("not a url"), "not a url");
        let long = link_label("https://example.com/a/very/long/path/that/goes/on/and/on/for/ever");
        assert_eq!(long.chars().count(), 48);
        assert!(long.ends_with('…'));
    }

    use ahg_engine::queue::{TaskInit, create_task};
    use ahg_engine::stats::{DownloadStats, MonthBucket};
    use ahg_engine::types::{ErrorCode, SourcePlatform};

    fn task(status: St) -> DownloadTask {
        let mut t = create_task(&TaskInit { url: Some("https://example.com/a".into()), ..Default::default() }, "id".into(), 0);
        t.progress.status = status;
        t
    }

    fn month(name: &str, files: f64, bytes: f64, by: &[(&str, u64)]) -> MonthBucket {
        MonthBucket { month: name.into(), files, bytes, by_platform: by.iter().map(|(k, n)| (k.to_string(), (*n).into())).collect() }
    }

    #[test]
    fn link_labels_follow_the_url_parser() {
        let cases = [
            ("https://www.youtube.com/watch?v=abc#t=5", "youtube.com · abc"),
            ("https://example.com/a/b/#frag", "example.com/a/b"),
            ("https://example.com/a?x=1#v=zz", "example.com/a"),
            ("https://example.com/a/b?v=", "example.com/a/b"),
            ("https://example.com/a?v&v=2", "example.com/a"),
            ("  https://example.com/a/  ", "example.com/a"),
            ("https://EXAMPLE.com:8080/a", "example.com/a"),
            ("https://user:pw@example.com/a", "example.com/a"),
            ("https://example.com", "example.com"),
            ("https://example.com/", "example.com"),
            ("https://example.com/watch?x=1&v=q", "example.com · q"),
        ];
        assert!(!cases.is_empty());
        for (input, want) in cases {
            assert_eq!(link_label(input), want, "link_label({input:?})");
        }
        assert_eq!(link_label("no scheme/x"), "no scheme/x");
        assert_eq!(link_label("  not a url "), "  not a url ");
    }

    #[test]
    fn a_label_is_the_title_then_the_file_then_the_link() {
        let mut t = task(St::Done);
        assert_eq!(label(&t), "example.com/a");
        t.progress.filename = "C:\\Music\\song.mp3".into();
        assert_eq!(label(&t), "song.mp3");
        assert_eq!(folder(&t.progress.filename), "C:\\Music");
        t.title = "  ".into();
        assert_eq!(label(&t), "  ", "Electron keeps a blank but non-empty title");
        t.title = "Song".into();
        assert_eq!(label(&t), "Song");
    }

    #[test]
    fn reasons_come_from_the_kind_with_two_overrides() {
        let mut t = task(St::Failed);
        t.error_code = Some(ErrorCode::Private);
        t.source = SourcePlatform::Youtube;
        assert_eq!(reason(&t).unwrap(), format!("{} {}", copy::ERR_PRIVATE_CAUSE.replace("{thing}", "video"), copy::ERR_PRIVATE_REMEDY));
        t.source = SourcePlatform::Soundcloud;
        assert!(reason(&t).unwrap().contains("This link is private."));
        t.source = SourcePlatform::Generic;
        t.error_code = Some(ErrorCode::Removed);
        assert_eq!(reason(&t).unwrap(), copy::ERR_GENERIC_DEAD_END);
        t.source = SourcePlatform::Spotify;
        assert!(reason(&t).unwrap().ends_with(copy::ERR_SPOTIFY_REMEDY));
        t.error_code = Some(ErrorCode::Network);
        assert!(!reason(&t).unwrap().contains(copy::ERR_SPOTIFY_REMEDY));
        t.error_code = None;
        t.progress.error = "raw text".into();
        assert_eq!(reason(&t).unwrap(), "raw text");
        t.progress.error = String::new();
        assert_eq!(reason(&t), None);
        t.progress.status = St::Cancelled;
        t.error_code = Some(ErrorCode::Private);
        t.progress.error = "raw".into();
        assert_eq!(reason(&t).unwrap(), "raw", "a canceled row shows the raw text");
        assert_eq!(reason(&task(St::Done)), None);
    }

    #[test]
    fn permanent_kinds_are_the_engines() {
        let all = [
            ErrorCode::Private,
            ErrorCode::Removed,
            ErrorCode::AgeRestricted,
            ErrorCode::LoginRequired,
            ErrorCode::GeoBlocked,
            ErrorCode::Drm,
            ErrorCode::RateLimited,
            ErrorCode::Network,
            ErrorCode::Unsupported,
            ErrorCode::FfmpegMissing,
            ErrorCode::DiskFull,
            ErrorCode::Unknown,
        ];
        let permanent: Vec<_> = all.into_iter().filter(|c| is_permanent(Some(*c))).collect();
        assert_eq!(permanent, [ErrorCode::Removed, ErrorCode::GeoBlocked, ErrorCode::Drm, ErrorCode::Unsupported]);
        assert!(!is_permanent(None));
        let mut t = task(St::Failed);
        let fixes: Vec<_> = [ErrorCode::DiskFull, ErrorCode::Unknown, ErrorCode::Network]
            .map(|c| {
                t.error_code = Some(c);
                fix(&t)
            })
            .to_vec();
        assert_eq!(fixes, [Some(errors::Fix::Settings), Some(errors::Fix::Engine), None]);
        t.error_code = Some(ErrorCode::Unknown);
        t.progress.status = St::Done;
        assert_eq!(fix(&t), None);
    }

    #[test]
    fn specs_join_the_parts_that_exist() {
        let mut t = task(St::Done);
        t.format = "mp3".into();
        assert_eq!(specs(&t), "mp3");
        t.duration = 187.0;
        t.progress.downloaded = 3_565_158.0;
        assert_eq!(specs(&t), "3:07 · 3.4 MB · mp3");
        t.progress.total = 13_002_342.0;
        t.is_playlist = true;
        t.progress.playlist_index = 2.0;
        t.progress.playlist_total = 9.0;
        assert_eq!(specs(&t), "2/9 · 3:07 · 12.4 MB · mp3");
        t.is_playlist = false;
        assert_eq!(specs(&t), "3:07 · 12.4 MB · mp3");
    }

    #[test]
    fn source_names_are_the_copy_names() {
        assert_eq!(source_name(SourcePlatform::Instagram), "Reels");
        assert_eq!(source_name(SourcePlatform::Twitter), "X / Twitter");
        assert_eq!(source_name(SourcePlatform::Generic), "Link");
    }

    #[test]
    fn retry_waits_are_the_queues_backoff() {
        let mut t = task(St::Retrying);
        let waits: Vec<u64> = [None, Some(0.0), Some(1.0), Some(2.0), Some(7.0)]
            .into_iter()
            .map(|n| {
                t.retry_count = n;
                retry_wait(&t)
            })
            .collect();
        assert_eq!(waits, [5, 5, 15, 45, 45]);
    }

    #[test]
    fn counts_and_totals_read_the_rows() {
        let mut live = task(St::Downloading);
        live.progress.speed = 2_000_000.0;
        let mut converting = task(St::Converting);
        converting.progress.speed = 500_000.0;
        let mut paused = task(St::Paused);
        paused.progress.speed = 9_000_000.0;
        let tasks = [live, converting, paused, task(St::Queued), task(St::Done), task(St::Failed), task(St::Cancelled)];
        assert_eq!(active_count(&tasks), 4);
        assert_eq!(in_flight(&tasks), (2, 2_500_000.0));
        assert_eq!(total_speed(&tasks), 2_500_000.0);
        assert_eq!(filter_counts(&tasks), [7, 4, 1, 2]);
        assert!(!all_paused(&tasks));
        assert!(all_paused(&[task(St::Paused), task(St::Queued)]));
        assert!(!all_paused(&[task(St::Queued)]));
        assert_eq!(in_flight(&[]), (0, 0.0));
    }

    #[test]
    fn the_axis_ceiling_is_the_next_one_two_five() {
        let table = [(0.0, 1.0), (1.0, 1.0), (1.5, 2.0), (2.0, 2.0), (3.0, 5.0), (5.1, 10.0), (1000.0, 1000.0), (2_500_000.0, 5_000_000.0)];
        assert!(!table.is_empty());
        for (input, want) in table {
            assert_eq!(axis_ceiling(input), want, "axis_ceiling({input})");
        }
        assert_eq!(axis_ceiling(f64::NAN), 1.0);
    }

    #[test]
    fn the_source_split_groups_ranks_and_keeps_first_seen_on_ties() {
        let stats = DownloadStats {
            current: month("2026-11", 8.0, 0.0, &[("tiktok", 2), ("vimeo", 2), ("youtube", 3), ("instagram", 1), ("spotify", 0)]),
            previous: None,
        };
        assert_eq!(source_split(&stats), [(Bucket::Reels, 3), (Bucket::Youtube, 3), (Bucket::Other, 2)]);
        let ties = DownloadStats { current: month("2026-11", 6.0, 0.0, &[("youtube", 2), ("vimeo", 2), ("tiktok", 2)]), previous: None };
        assert_eq!(source_split(&ties), [(Bucket::Youtube, 2), (Bucket::Other, 2), (Bucket::Reels, 2)]);
        assert_eq!(Bucket::Reels.name(), "Reels & Shorts");
        assert!(source_split(&DownloadStats { current: month("2026-11", 0.0, 0.0, &[]), previous: None }).is_empty());
    }

    #[test]
    fn the_month_sentence_compares_with_the_last_active_month() {
        let sentence = |cur: MonthBucket, prev: Option<MonthBucket>| month_sentence(&DownloadStats { current: cur, previous: prev });
        let prev = |files| Some(month("2026-10", files, 5_000_000.0, &[]));
        assert_eq!(sentence(month("2026-11", 0.0, 0.0, &[]), None), copy::MONTH_NOTHING_YET_NO_PREV);
        assert_eq!(sentence(month("2026-11", 0.0, 0.0, &[]), prev(3.0)), copy::month_last_month_kept(3, "4.8 MB"));
        assert_eq!(sentence(month("2026-11", 4.0, 1.0, &[]), None), copy::MONTH_FIRST_ON_RECORD);
        assert_eq!(sentence(month("2026-11", 4.0, 1.0, &[]), prev(0.0)), copy::MONTH_FIRST_ON_RECORD);
        assert_eq!(sentence(month("2026-11", 4.0, 1.0, &[]), prev(4.0)), copy::month_same_count("October"));
        assert_eq!(sentence(month("2026-11", 4.0, 1.0, &[]), prev(1.0)), copy::month_diff(3, "October"));
        assert_eq!(sentence(month("2026-11", 1.0, 1.0, &[]), prev(4.0)), copy::month_diff(-3, "October"));
    }
}
