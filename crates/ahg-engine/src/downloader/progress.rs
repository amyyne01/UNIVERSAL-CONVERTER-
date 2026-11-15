//! Reading yt-dlp's output while a download runs: the progress lines, the
//! file paths, and the line buffering of both streams. This is everything in
//! `parseLine`, `parseSize`, `parseSpeed`, `parseEta` and `spawnDownload` of
//! electron/downloader.ts that does not touch a process.
//!
//! JavaScript's number parsing differs from Rust's (`Number`, `parseFloat`,
//! `parseInt` each accept different text), so those three live here once, for
//! the metadata module too.
use std::sync::LazyLock;

use regex::Regex;

use crate::text::{is_js_whitespace, js_trim};
use crate::types::{DownloadProgress, DownloadStatus};

const PROGRESS_MARKER: &str = "[PROGRESS]";
const FILEPATH_MARKER: &str = "[FILEPATH]";
/// What JavaScript's `\s` matches. The regex crate's own `\s` differs (U+FEFF, U+0085).
const WS: &str = r"\t\n\x0B\x0C\r \u{A0}\u{1680}\u{2000}-\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}\u{FEFF}";

fn re(pattern: String) -> Regex {
    Regex::new(&pattern).unwrap()
}

// ── JavaScript number parsing ────────────────────────────────────────────────

/// Length of the longest prefix of `b` that is an unsigned decimal literal
/// (`12`, `1.`, `.5`, `1.5e-3`); 0 when there is none.
fn decimal_len(b: &[u8]) -> usize {
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int = digits(0);
    let mut end = int;
    if b.get(end) == Some(&b'.') {
        let frac = digits(end + 1);
        if int == 0 && frac == 0 {
            return 0;
        }
        end += 1 + frac;
    } else if int == 0 {
        return 0;
    }
    if matches!(b.get(end), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(b.get(end + 1), Some(b'+' | b'-')));
        let exp = digits(end + 1 + sign);
        if exp > 0 {
            end += 1 + sign + exp;
        }
    }
    end
}

fn signed(negative: bool, v: f64) -> f64 {
    if negative { -v } else { v }
}

/// `Number(s)`: NaN when the whole text is not a number.
pub fn js_number(s: &str) -> f64 {
    let t = js_trim(s);
    if t.is_empty() {
        return 0.0;
    }
    let b = t.as_bytes();
    if b.len() > 2 && b[0] == b'0' {
        let radix = match b[1] {
            b'x' | b'X' => 16,
            b'o' | b'O' => 8,
            b'b' | b'B' => 2,
            _ => 0,
        };
        if radix != 0 {
            // ponytail: above 2^128 the digits are summed as floats, which can be
            // off in the last bit where JavaScript rounds the exact value.
            let (mut exact, mut approx): (Option<u128>, f64) = (Some(0), 0.0);
            for c in t[2..].chars() {
                let Some(d) = c.to_digit(radix).filter(|_| c.is_ascii()) else { return f64::NAN };
                exact = exact.and_then(|v| v.checked_mul(radix as u128)?.checked_add(d as u128));
                approx = approx * radix as f64 + d as f64;
            }
            return exact.map_or(approx, |v| v as f64);
        }
    }
    let (negative, rest) = match b[0] {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    if rest == "Infinity" {
        return signed(negative, f64::INFINITY);
    }
    if decimal_len(rest.as_bytes()) == rest.len() {
        return signed(negative, rest.parse().unwrap_or(f64::NAN));
    }
    f64::NAN
}

/// `Number(s)` kept only when finite, else 0: the `toNum` of electron/downloader.ts.
pub fn to_num_str(s: Option<&str>) -> f64 {
    let n = s.map_or(f64::NAN, js_number);
    if n.is_finite() { n } else { 0.0 }
}

/// `parseFloat(s)`: the longest numeric prefix, NaN when there is none.
pub fn parse_float(s: &str) -> f64 {
    let t = s.trim_start_matches(is_js_whitespace);
    let (negative, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    if rest.starts_with("Infinity") {
        return signed(negative, f64::INFINITY);
    }
    match decimal_len(rest.as_bytes()) {
        0 => f64::NAN,
        n => signed(negative, rest[..n].parse().unwrap_or(f64::NAN)),
    }
}

/// `parseInt(s, 10)`.
fn parse_int(s: &str) -> f64 {
    let t = s.trim_start_matches(is_js_whitespace);
    let (negative, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    match rest.bytes().take_while(u8::is_ascii_digit).count() {
        0 => f64::NAN,
        n => signed(negative, rest[..n].parse().unwrap_or(f64::NAN)),
    }
}

/// `Math.round`: halves go up, also below zero.
fn js_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let floor = x.floor();
    if x - floor >= 0.5 { floor + 1.0 } else { floor }
}

// ── Size, speed, eta ─────────────────────────────────────────────────────────

static SIZE: LazyLock<Regex> = LazyLock::new(|| re(format!(r"([0-9.]+)[{WS}]*([KkMmGgTt][Ii]?[Bb]|[Bb])")));

/// "1.2MiB" to bytes. Binary units use 1024, decimal ones 1000; unparseable is 0.
pub fn parse_size(s: &str) -> f64 {
    let Some(m) = SIZE.captures(s) else { return 0.0 };
    let mut value = parse_float(&m[1]);
    if !value.is_finite() {
        return 0.0;
    }
    let unit = m[2].to_ascii_uppercase();
    let base = if unit.contains('I') { 1024.0 } else { 1000.0 };
    let exp = match unit.as_bytes()[0] {
        b'K' => 1,
        b'M' => 2,
        b'G' => 3,
        b'T' => 4,
        _ => 0,
    };
    for _ in 0..exp {
        value *= base;
    }
    value
}

/// "1.2MiB/s" to bytes per second.
pub fn parse_speed(s: &str) -> f64 {
    let b = s.as_bytes();
    let n = b.len();
    let cut = n >= 2 && b[n - 2] == b'/' && matches!(b[n - 1], b's' | b'S');
    parse_size(if cut { &s[..n - 2] } else { s })
}

/// "00:12" or "1:02:03" to seconds; unparseable is 0.
pub fn parse_eta(s: &str) -> f64 {
    let lower = s.to_ascii_lowercase();
    if s.is_empty() || lower.contains("unknown") || lower.contains("na") {
        return 0.0;
    }
    let parts: Vec<f64> = s.split(':').map(parse_int).collect();
    if parts.iter().any(|p| p.is_nan()) {
        return 0.0;
    }
    parts.iter().fold(0.0, |acc, p| acc * 60.0 + p)
}

// ── One line ─────────────────────────────────────────────────────────────────

/// What `parseLine` did besides editing the progress it was given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParsedLine {
    /// The percent to carry into the next line.
    pub last_percent: f64,
    /// True when Electron called `onProgress`: the caller must send the progress on.
    pub emit: bool,
}

static HUMAN: LazyLock<Regex> = LazyLock::new(|| {
    re(format!(
        r"([0-9]+(?:\.[0-9]+)?)%[{WS}]+of[{WS}]+~?[{WS}]*([0-9.]+[{WS}]*[A-Za-z0-9_]+)[{WS}]+at[{WS}]+([^{WS}]+)[{WS}]+ETA[{WS}]+([^{WS}]+)"
    ))
});

fn after_post_phase(status: DownloadStatus) -> bool {
    matches!(status, DownloadStatus::Converting | DownloadStatus::Embedding)
}

/// Reads one line of yt-dlp output: updates `progress`, and says whether to emit it.
pub fn parse_line(line: &str, progress: &mut DownloadProgress, last_percent: f64) -> ParsedLine {
    let keep = |emit| ParsedLine { last_percent, emit };
    // Status only moves forward within one item; the next collection item starts over.
    if line.strip_prefix("[download] Downloading item ").is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit())) {
        progress.status = DownloadStatus::Downloading;
        return keep(false);
    }
    if line.starts_with("[ExtractAudio]") || line.starts_with("[Merger]") {
        progress.status = DownloadStatus::Converting;
        return keep(true);
    }
    if ["Metadata", "EmbedThumbnail", "ThumbnailsConvertor", "EmbedSubtitle"].iter().any(|n| line.starts_with(&format!("[{n}]"))) {
        progress.status = DownloadStatus::Embedding;
        return keep(true);
    }

    if let Some(rest) = line.strip_prefix(PROGRESS_MARKER) {
        let mut f = rest.split('|');
        let (dl, total, est, speed, eta) = (f.next(), f.next(), f.next(), f.next(), f.next());
        let downloaded = to_num_str(dl);
        let total_bytes = match to_num_str(total) {
            t if t != 0.0 => t,
            _ => to_num_str(est),
        };
        let mut last = last_percent;
        if total_bytes > 0.0 {
            last = (downloaded / total_bytes * 100.0).min(100.0);
        }
        if !after_post_phase(progress.status) {
            progress.status = DownloadStatus::Downloading;
        }
        progress.downloaded = downloaded;
        progress.total = total_bytes;
        progress.speed = to_num_str(speed);
        progress.eta = to_num_str(eta);
        progress.percent = last;
        return ParsedLine { last_percent: last, emit: true };
    }

    // Fallback: yt-dlp's human-readable progress line.
    let Some(m) = HUMAN.captures(line) else { return keep(false) };
    let parsed = parse_float(&m[1]);
    let last = (if parsed.is_nan() { last_percent } else { parsed }).min(100.0);
    if !after_post_phase(progress.status) {
        progress.status = DownloadStatus::Downloading;
    }
    progress.percent = last;
    progress.total = parse_size(&m[2]);
    progress.downloaded = js_round(progress.total * last / 100.0);
    progress.speed = parse_speed(&m[3]);
    progress.eta = parse_eta(&m[4]);
    ParsedLine { last_percent: last, emit: true }
}

// ── A whole download's output ────────────────────────────────────────────────

static ALREADY: LazyLock<Regex> =
    LazyLock::new(|| re(r"^\[download\] ([^\n\r\u{2028}\u{2029}]+) has already been downloaded".to_owned()));
static DRIVE_PATH: LazyLock<Regex> = LazyLock::new(|| re(r"^[a-zA-Z]:[\\/]".to_owned()));

/// How a download ended, as `spawnDownload` reports it on close.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// `filepaths` leaves out files yt-dlp found already on disk;
    /// `already_on_disk` means it skipped every one.
    Done { path: String, filepaths: Vec<String>, already_on_disk: bool },
    Failed(String),
}

/// Everything `spawnDownload` does with yt-dlp's two output streams, minus the
/// process itself: split into lines, keep a partial last line for the next
/// chunk, track progress, file paths and errors. The caller feeds it decoded
/// text and sends on the progress it returns.
#[derive(Debug, Clone)]
pub struct OutputScan {
    progress: DownloadProgress,
    last_percent: f64,
    final_path: String,
    filepaths: Vec<String>,
    skipped: Vec<String>,
    error_out: String,
    stderr_buf: String,
    stdout_buf: String,
    count_items: bool,
}

/// The item number and total from yt-dlp's "[download] Downloading item 3 of 12".
pub fn playlist_item(line: &str) -> Option<(f64, f64)> {
    let rest = line.strip_prefix("[download] Downloading item ")?;
    let (index, total) = rest.trim_end_matches('\r').split_once(" of ")?;
    let number = |s: &str| (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<f64>().ok()).flatten();
    Some((number(index)?, number(total)?))
}

impl Default for OutputScan {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputScan {
    pub fn new() -> Self {
        Self {
            progress: DownloadProgress { status: DownloadStatus::Downloading, ..Default::default() },
            last_percent: 0.0,
            final_path: String::new(),
            filepaths: Vec::new(),
            skipped: Vec::new(),
            error_out: String::new(),
            stderr_buf: String::new(),
            stdout_buf: String::new(),
            count_items: false,
        }
    }

    /// D31: record which item of a collection is downloading, and emit the
    /// change. Electron 1.x never fills these in; the app turns this on.
    pub fn with_item_counts(mut self) -> Self {
        self.count_items = true;
        self
    }

    pub fn progress(&self) -> &DownloadProgress {
        &self.progress
    }

    fn flush(&mut self, line: &str, events: &mut Vec<DownloadProgress>) {
        if let Some(m) = ALREADY.captures(line) {
            self.skipped.push(js_trim(&m[1]).to_owned());
        }
        let r = parse_line(line, &mut self.progress, self.last_percent);
        self.last_percent = r.last_percent;
        let item = if self.count_items { playlist_item(line) } else { None };
        if let Some((index, total)) = item {
            self.progress.playlist_index = index;
            self.progress.playlist_total = total;
        }
        if r.emit || item.is_some() {
            events.push(self.progress.clone());
        }
        // Only yt-dlp's own ERROR: lines: the word alone also appears in titles.
        if line.trim_start_matches(is_js_whitespace).starts_with("ERROR:") {
            self.error_out.push_str(line);
            self.error_out.push('\n');
        }
    }

    /// With `--no-quiet` everything arrives on stdout: only `[FILEPATH]` lines are
    /// paths, and a path must start with a drive letter (a UNC path would make
    /// the later stat leak credentials). Anything else is handled as a progress line.
    fn flush_stdout(&mut self, line: &str, events: &mut Vec<DownloadProgress>) {
        let t = line.strip_suffix('\r').unwrap_or(line);
        match t.strip_prefix(FILEPATH_MARKER) {
            Some(p) => {
                let p = js_trim(p);
                if DRIVE_PATH.is_match(p) {
                    self.filepaths.push(p.to_owned());
                    self.final_path = p.to_owned();
                }
            }
            None => self.flush(t, events),
        }
    }

    pub fn push_stderr(&mut self, chunk: &str) -> Vec<DownloadProgress> {
        let mut events = Vec::new();
        self.stderr_buf.push_str(chunk);
        let buf = std::mem::take(&mut self.stderr_buf);
        let mut lines: Vec<&str> = buf.split('\n').collect();
        self.stderr_buf = lines.pop().unwrap_or_default().to_owned(); // the partial last line
        for line in lines {
            self.flush(line, &mut events);
        }
        events
    }

    pub fn push_stdout(&mut self, chunk: &str) -> Vec<DownloadProgress> {
        let mut events = Vec::new();
        self.stdout_buf.push_str(chunk);
        let buf = std::mem::take(&mut self.stdout_buf);
        let mut lines: Vec<&str> = buf.split('\n').collect();
        self.stdout_buf = lines.pop().unwrap_or_default().to_owned();
        for line in lines {
            self.flush_stdout(line, &mut events);
        }
        events
    }

    /// The process has closed: handle what is left in the buffers, then report the
    /// outcome. `code` is the exit code, `None` when a signal ended it. The events
    /// come back in order, the last one being the `done` update on success.
    pub fn finish(&mut self, code: Option<i32>) -> (Vec<DownloadProgress>, Outcome) {
        let mut events = Vec::new();
        let (err, out) = (std::mem::take(&mut self.stderr_buf), std::mem::take(&mut self.stdout_buf));
        if !err.is_empty() {
            self.flush(&err, &mut events);
        }
        if !out.is_empty() {
            self.flush_stdout(&out, &mut events);
        }
        if code == Some(0) {
            self.progress.status = DownloadStatus::Done;
            self.progress.percent = 100.0;
            events.push(self.progress.clone());
            let fresh: Vec<String> = self.filepaths.iter().filter(|p| !self.skipped.contains(p)).cloned().collect();
            let already_on_disk = !self.filepaths.is_empty() && fresh.is_empty();
            return (events, Outcome::Done { path: self.final_path.clone(), filepaths: fresh, already_on_disk });
        }
        let error = match js_trim(&self.error_out) {
            "" => format!("yt-dlp exited with code {}", code.map_or("null".to_owned(), |c| c.to_string())),
            e => e.to_owned(),
        };
        self.progress.error.clone_from(&error);
        (events, Outcome::Failed(error))
    }
}
