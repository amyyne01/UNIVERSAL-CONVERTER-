//! Small rotating diagnostic log in the app's data folder: `ahg.log` and
//! `ahg.log.1`, about 512 KB each. Everything is redacted on the way in, since
//! a log a user may attach to a bug report must never carry a token, a cookie
//! or an auth header.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use fancy_regex::Regex;
use url::Url;

use crate::clock::iso_now;
use crate::text::{JS_SPACE, ascii_ci_regex, js_len, utf16_suffix};

pub const LOG_CAP_BYTES: usize = 512 * 1024;
const TAIL_CHARS: usize = 2000;

/// `\b` before a word character, with JavaScript's ASCII idea of a word.
const WORD_START: &str = "(?<![A-Za-z0-9_])";

static GITHUB_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("{WORD_START}(?:gh[pousr]_|github_pat_)[A-Za-z0-9_]{{10,}}")).expect("constant pattern")
});
static AUTH_SCHEME: LazyLock<Regex> = LazyLock::new(|| {
    ascii_ci_regex(&format!("{WORD_START}(Bearer|Basic)[{JS_SPACE}]+[A-Za-z0-9._~+/=-]+"))
});
static HEADER: LazyLock<Regex> = LazyLock::new(|| {
    ascii_ci_regex(&format!(r"{WORD_START}((?:set-)?cookie|authorization)[{JS_SPACE}]*[:=][{JS_SPACE}]*[^\r\n]*"))
});
static SECRET_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    ascii_ci_regex(&format!(
        r#"((?:access_|refresh_|api_)?(?:token|secret|key|sig|password)["']?[{JS_SPACE}]*[=:][{JS_SPACE}]*["']?)[^&{JS_SPACE}"',}}]+"#
    ))
});
static URL: LazyLock<Regex> =
    LazyLock::new(|| ascii_ci_regex(&format!(r#"{WORD_START}[a-zA-Z][a-zA-Z0-9+.-]*://[^{JS_SPACE}"'<>]+"#)));

/// Masks credentials: GitHub tokens, Bearer and Basic auth, cookie headers,
/// and token-like query or JSON values.
pub fn redact(text: &str) -> String {
    let text = GITHUB_TOKEN.replace_all(text, "[redacted]");
    let text = AUTH_SCHEME.replace_all(&text, "${1} [redacted]");
    let text = HEADER.replace_all(&text, "${1}: [redacted]");
    SECRET_VALUE.replace_all(&text, "${1}[redacted]").into_owned()
}

/// The last part of a long error such as yt-dlp's stderr, where the reason sits.
pub fn tail(s: &str) -> String {
    if js_len(s) > TAIL_CHARS { format!("...{}", utf16_suffix(s, TAIL_CHARS)) } else { s.to_owned() }
}

/// Host and port only. A log line never carries a full URL, its path or its query.
pub fn host_of(u: &str) -> String {
    let Ok(url) = Url::parse(u) else { return String::new() };
    match (url.host_str(), url.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_owned(),
        (None, _) => String::new(),
    }
}

pub fn scrub_urls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut copied = 0;
    for m in URL.find_iter(s).flatten() {
        out.push_str(&s[copied..m.start()]);
        out.push_str(&host_of(m.as_str()));
        copied = m.end();
    }
    out.push_str(&s[copied..]);
    out
}

static LOG_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Sets where `log_line` writes. Until this is called, logging does nothing.
pub fn init_log(dir: &Path) {
    *LOG_FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir.join("ahg.log"));
}

pub fn log_line(level: &str, msg: &str) {
    let file = LOG_FILE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(file) = file {
        log_line_to(&file, level, msg, &iso_now());
    }
}

/// Appends one redacted line, rotating first when the file has reached its
/// cap. Logging must never fail the caller, so every error is swallowed.
pub fn log_line_to(file: &Path, level: &str, msg: &str, timestamp: &str) {
    if fs::metadata(file).is_ok_and(|m| m.len() >= LOG_CAP_BYTES as u64) {
        let mut rotated = file.as_os_str().to_owned();
        rotated.push(".1");
        let _ = fs::remove_file(&rotated);
        let _ = fs::rename(file, &rotated);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{timestamp} [{level}] {}", redact(msg));
    }
}
