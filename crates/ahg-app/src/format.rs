//! How numbers and names are written, the same in all six presets. The
//! outputs are the Electron app's (`docs/design-gate/CONTENT.md` section 4);
//! the tests below are that section's tables.
use ahg_engine::types::{DownloadStatus as St, DownloadTask};

use crate::copy;

/// 1024-based, one decimal from KB up, `—` for nothing.
pub fn bytes(n: f64) -> String {
    if n.is_nan() || n <= 0.0 {
        return "—".into();
    }
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let (mut value, mut unit) = (n, 0);
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{value} B") } else { format!("{value:.1} {}", UNITS[unit]) }
}

/// `2.4 MB/s`; empty when nothing is moving.
pub fn speed(bytes_per_second: f64) -> String {
    if bytes_per_second > 0.0 { format!("{}/s", bytes(bytes_per_second)) } else { String::new() }
}

/// `m:ss`, or `h:mm:ss` from an hour; empty for zero.
pub fn duration(seconds: f64) -> String {
    if seconds.is_nan() || seconds < 1.0 {
        return String::new();
    }
    let total = seconds.floor() as u64;
    let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Time left; empty when unknown.
pub fn eta(seconds: f64) -> String {
    if seconds.is_finite() && seconds > 0.0 { duration(seconds) } else { String::new() }
}

/// Speed and time left joined with a middle dot; `–` when neither is known.
pub fn rate(bytes_per_second: f64, eta_seconds: f64) -> String {
    let parts: Vec<String> = [speed(bytes_per_second), eta(eta_seconds)].into_iter().filter(|p| !p.is_empty()).collect();
    if parts.is_empty() { "–".into() } else { parts.join(" · ") }
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
    const NAMES: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    year_month.split_once('-').and_then(|(_, m)| m.parse::<usize>().ok()).and_then(|m| NAMES.get(m.wrapping_sub(1))).map_or_else(|| year_month.to_string(), |n| (*n).to_string())
}

/// The stand-in title for a download that has none yet.
pub fn link_label(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else { return url.to_string() };
    let (before_query, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (host, path) = before_query.split_once('/').unwrap_or((before_query, ""));
    let host = host.strip_prefix("www.").or_else(|| host.strip_prefix("m.")).unwrap_or(host);
    if let Some(v) = query.split('&').find_map(|pair| pair.strip_prefix("v=")) {
        return format!("{host} · {v}");
    }
    let path = path.trim_end_matches('/');
    let label = if path.is_empty() { host.to_string() } else { format!("{host}/{path}") };
    if label.chars().count() > 48 { format!("{}…", label.chars().take(47).collect::<String>()) } else { label }
}

/// The last part of a Windows or POSIX path.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// A download's name: its title, else its file's name, else the link label.
pub fn label(t: &DownloadTask) -> String {
    if !t.title.trim().is_empty() {
        return t.title.clone();
    }
    let file = file_name(&t.progress.filename);
    if file.is_empty() { link_label(&t.url) } else { file.to_string() }
}

/// Not finished one way or another: still counted as active.
pub fn is_active(t: &DownloadTask) -> bool {
    !matches!(t.progress.status, St::Done | St::Failed | St::Cancelled)
}

/// Transferring now.
pub fn is_live(t: &DownloadTask) -> bool {
    matches!(t.progress.status, St::FetchingInfo | St::Downloading | St::Converting | St::Embedding | St::Retrying)
}

/// The speed of everything downloading, summed, in bytes per second.
pub fn total_speed(tasks: &[DownloadTask]) -> f64 {
    tasks.iter().filter(|t| t.progress.status == St::Downloading).map(|t| t.progress.speed.max(0.0)).sum()
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
        let table = [(0.0, "—"), (-5.0, "—"), (1.0, "1 B"), (512.0, "512 B"), (1023.0, "1023 B"), (1024.0, "1.0 KB"), (1536.0, "1.5 KB"), (1_048_575.0, "1024.0 KB"), (1_048_576.0, "1.0 MB"), (3_565_158.0, "3.4 MB"), (13_002_342.0, "12.4 MB"), (1_073_741_824.0, "1.0 GB"), (1_649_267_441_664.0, "1.5 TB"), (512.5, "512.5 B")];
        for (input, want) in table {
            assert_eq!(bytes(input), want, "bytes({input})");
        }
    }

    #[test]
    fn speeds_times_and_rates_match_the_old_app() {
        assert_eq!(speed(0.0), "");
        assert_eq!(speed(2_500_000.0), "2.4 MB/s");
        assert_eq!(speed(524_288.0), "512.0 KB/s");
        for (input, want) in [(0.0, ""), (5.0, "0:05"), (59.9, "0:59"), (187.0, "3:07"), (600.0, "10:00"), (3599.0, "59:59"), (3600.0, "1:00:00"), (3723.0, "1:02:03"), (36000.0, "10:00:00")] {
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
        for (input, want) in [(47.4, 47), (47.5, 48), (99.5, 100), (0.0, 0)] {
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
}
