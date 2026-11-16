//! The app's self-update (electron/updater.ts), minus the network call, the
//! process spawn and Electron itself. The owner fetches bytes, hashes them and
//! runs a command; every decision in between is made here, on a plain struct.
//!
//! The owner's loop, mirroring `Updater.check` / `download` / `install`:
//!   check:    `begin_check()`, fetch `check_request()` through `guard_request` /
//!             `guard_response`, then `on_release_json(json, current)`; a failing
//!             step is `check_failed(message)`.
//!   download: `download_begin()`, fetch the sums, `download_sums(text)`, fetch the
//!             exe, `download_head(content-length)`, `download_chunk(len)` per
//!             chunk, SHA-256 the bytes, `download_finish(sha)`, `write_staged`,
//!             `download_written`. A failing step is `download_failed(message)`.
//!   install:  `install(target)`, `write_install_script`, spawn `plan.program`
//!             with `plan.args` (detached, stdio ignored, window hidden, unref),
//!             log `plan.log`, quit the app.
//! Every method that emits returns the events for the window, in order; the
//! state is updated whether or not a window exists to receive them.
use std::fs;
use std::io;
use std::path::Path;

use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use url::Url;

use crate::downloader::progress::js_number;
use crate::text::{JS_SPACE, js_trim};

pub const RELEASE_API: &str = "https://api.github.com/repos/amyyne01/UNIVERSAL-CONVERTER-/releases/latest";
/// Published beside the exe by scripts/release-assets.mjs; the update is refused without it.
pub const SUMS_ASSET: &str = "SHA256SUMS.txt";
pub const USER_AGENT: &str = "AHG-Universal-Converter";
const TRUSTED_HOSTS: [&str; 4] = ["github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com", "api.github.com"];
const GITHUB_BASE: &str = "https://github.com/";
const SCRIPT_NAME: &str = "apply-update.cmd";
const PREFIX: &str = "Update";

/// A GET the owner must make: the URL and a `User-Agent` header, nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub url: String,
    pub user_agent: &'static str,
}

impl Request {
    pub fn new(url: &str) -> Self {
        Self { url: url.to_string(), user_agent: USER_AGENT }
    }
}

/// One `webContents.send(channel, payload)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub channel: &'static str,
    pub payload: Value,
}

pub(crate) fn event(channel: &'static str, payload: Value) -> Event {
    Event { channel, payload }
}

/// `UpdateState` of shared/types.ts.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpdateState {
    pub phase: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// A number, or null when JavaScript's `Infinity` went through JSON.
    pub percent: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl UpdateState {
    fn phase(phase: &'static str) -> Self {
        Self { phase, version: None, percent: None, message: None }
    }
}

// ── pure functions ───────────────────────────────────────────────────────────

/// `isTrustedReleaseHost(url)`: https on an allowlisted host; a relative URL resolves onto github.com.
pub fn is_trusted_release_host(url: &str) -> bool {
    is_trusted_release_host_with_base(url, GITHUB_BASE)
}

pub fn is_trusted_release_host_with_base(url: &str, base: &str) -> bool {
    let Ok(base) = Url::parse(base) else { return false };
    let Ok(parsed) = Url::options().base_url(Some(&base)).parse(url) else { return false };
    parsed.scheme() == "https" && parsed.host_str().is_some_and(|h| TRUSTED_HOSTS.contains(&h))
}

/// `parseInt(n, 10) || 0`.
fn int_part(n: &str) -> f64 {
    let t = js_trim(n);
    let (sign, rest) = match t.as_bytes().first() {
        Some(b'-') => (-1.0, &t[1..]),
        Some(b'+') => (1.0, &t[1..]),
        _ => (1.0, t),
    };
    let digits = &rest[..rest.bytes().take_while(u8::is_ascii_digit).count()];
    if digits.is_empty() {
        return 0.0;
    }
    let v = sign * digits.parse::<f64>().unwrap_or(f64::NAN);
    if v.is_nan() || v == 0.0 { 0.0 } else { v }
}

fn version_parts(v: &str) -> Vec<f64> {
    v.strip_prefix('v').unwrap_or(v).split('.').map(int_part).collect()
}

/// `isNewerVersion(candidate, current)`: numeric segments; junk counts as 0.
pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    let (a, b) = (version_parts(candidate), version_parts(current));
    for i in 0..a.len().max(b.len()) {
        let d = a.get(i).copied().unwrap_or(0.0) - b.get(i).copied().unwrap_or(0.0);
        if d != 0.0 {
            return d > 0.0;
        }
    }
    false
}

/// `parseChecksum(sums, name)`: the lowercase hash on the line `<sha256>  <name>` (`*name` tolerated).
pub fn parse_checksum(sums: &str, name: &str) -> Option<String> {
    let line_re = Regex::new(&format!(r"^([a-fA-F0-9]{{64}})[{JS_SPACE}]+\*?([^\n\r\x{{2028}}\x{{2029}}]+)$")).unwrap();
    let split = Regex::new(r"\r?\n").unwrap();
    for line in split.split(sums) {
        if let Some(m) = line_re.captures(js_trim(line))
            && js_trim(&m[2]) == name
        {
            return Some(m[1].to_ascii_lowercase());
        }
    }
    None
}

/// `swapScript(staged, target)`: the text of apply-update.cmd. The paths go in
/// raw, exactly as the TypeScript does (no escaping of `%` or `"`).
pub fn swap_script(staged: &str, target: &str) -> String {
    [
        "@echo off",
        "setlocal",
        &format!("set \"SRC={staged}\""),
        &format!("set \"DST={target}\""),
        "for /l %%i in (1,1,30) do (",
        "  move /y \"%SRC%\" \"%DST%\" >nul 2>&1 && goto done",
        "  ping -n 3 127.0.0.1 >nul",
        ")",
        "goto cleanup",
        ":done",
        "start \"\" \"%DST%\"",
        ":cleanup",
        "(goto) 2>nul & del \"%~f0\"",
        "",
    ]
    .join("\r\n")
}

/// The request guard of `request()` in both updaters, before the fetch.
/// `prefix` is "Update" or "Engine update".
pub fn guard_request(url: &str, prefix: &str) -> Result<Request, String> {
    if is_trusted_release_host(url) { Ok(Request::new(url)) } else { Err(format!("{prefix} refused: untrusted host \"{url}\"")) }
}

/// The guard after the fetch: HTTP status, then the host that actually answered
/// (`res.url`; an empty or missing one is not checked).
pub fn guard_response(ok: bool, status: u16, final_url: Option<&str>, prefix: &str) -> Result<(), String> {
    if !ok {
        return Err(format!("{prefix} failed: HTTP {status}"));
    }
    match final_url {
        Some(u) if !u.is_empty() && !is_trusted_release_host(u) => Err(format!("{prefix} refused: redirected to untrusted host \"{u}\"")),
        _ => Ok(()),
    }
}

/// The text Node gives an fs error, which ends up in the `update:error` message.
/// Only a missing path is spelled the Node way; anything else is Rust's own text.
pub fn fs_error_message(err: &io::Error, syscall: &str, path: &str) -> String {
    // 123 is Windows' ERROR_INVALID_NAME, which Node also reports as a missing path.
    if err.kind() == io::ErrorKind::NotFound || err.raw_os_error() == Some(123) {
        format!("ENOENT: no such file or directory, {syscall} '{path}'")
    } else {
        err.to_string()
    }
}

// ── JavaScript value helpers for the release JSON ────────────────────────────

fn js_number_string(n: f64) -> String {
    if n.is_nan() {
        "NaN".into()
    } else if n == 0.0 {
        "0".into()
    } else if n.abs() >= 1e21 || n.abs() < 1e-6 {
        let s = format!("{n:e}");
        match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
            _ => s,
        }
    } else {
        format!("{n}")
    }
}

/// `String(v)`.
pub(crate) fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => js_number_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
        Value::Array(a) => a.iter().map(|x| if x.is_null() { String::new() } else { js_string(x) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

pub(crate) fn js_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// `value.key ?? undefined`: reading a property of null is a TypeError.
pub(crate) fn prop<'a>(v: &'a Value, key: &str) -> Result<Option<&'a Value>, String> {
    if v.is_null() {
        return Err(format!("Cannot read properties of null (reading '{key}')"));
    }
    Ok(v.get(key).filter(|x| !x.is_null()))
}

/// `json.assets ?? []` followed by a call to `.find`: a non-array is a TypeError.
pub(crate) fn assets_list(json: &Value) -> Result<&[Value], String> {
    match prop(json, "assets")? {
        None => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        Some(_) => Err("assets.find is not a function".into()),
    }
}

/// `assets.find(a => a.name === name)`, the way both updaters look up a published file.
pub(crate) fn find_named<'a>(assets: &'a [Value], name: &str) -> Result<Option<&'a Value>, String> {
    for a in assets {
        if prop(a, "name")?.and_then(Value::as_str) == Some(name) {
            return Ok(Some(a));
        }
    }
    Ok(None)
}

// ── Windows paths (path.win32.join, which Node uses on Windows) ──────────────

fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}

/// `path.win32.normalize` for drive-letter, rooted and relative paths.
/// ponytail: UNC (`\\server\share`) is not recognised; add when userData can sit on one.
pub fn win_normalize(path: &str) -> String {
    let chars: Vec<char> = path.chars().collect();
    match chars.len() {
        0 => return ".".into(),
        1 => return if is_sep(chars[0]) { "\\".into() } else { path.into() },
        _ => {}
    }
    let (mut root_end, mut abs, mut device) = (0, false, String::new());
    if is_sep(chars[0]) {
        abs = true;
        root_end = 1;
    } else if chars[0].is_ascii_alphabetic() && chars[1] == ':' {
        device = chars[..2].iter().collect();
        root_end = 2;
        if chars.len() > 2 && is_sep(chars[2]) {
            abs = true;
            root_end = 3;
        }
    }
    let rest: String = chars[root_end..].iter().collect();
    let mut stack: Vec<&str> = Vec::new();
    for seg in rest.split(is_sep) {
        match seg {
            "" | "." => {}
            ".." => match stack.last() {
                Some(&last) if last != ".." => {
                    stack.pop();
                }
                _ if !abs => stack.push(".."),
                _ => {}
            },
            s => stack.push(s),
        }
    }
    let mut tail = stack.join("\\");
    if tail.is_empty() && !abs {
        tail = ".".into();
    }
    if !tail.is_empty() && is_sep(chars[chars.len() - 1]) {
        tail.push('\\');
    }
    format!("{device}{}{tail}", if abs { "\\" } else { "" })
}

/// `path.join(a, b)` with Windows semantics.
pub fn win_join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, true) => ".".into(),
        (true, false) => win_normalize(b),
        (false, true) => win_normalize(a),
        _ => win_normalize(&format!("{a}\\{b}")),
    }
}

// ── the updater ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Available {
    pub version: String,
    pub asset_name: String,
    pub asset_url: String,
    pub sums_url: String,
}

/// Where a verified download goes: `file` inside `dir`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    pub dir: String,
    pub file: String,
    version: String,
}

/// What `install` hands the process layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    pub script_path: String,
    /// Written as UTF-8, no BOM.
    pub script: String,
    /// Spawn this detached, stdio ignored, window hidden, then unref.
    pub program: &'static str,
    pub args: Vec<String>,
    /// `logLine('info', ..)` before quitting; then quit the app.
    pub log: &'static str,
}

const NO_UPDATE: &str = "Update refused: no available update to download";

struct Job {
    expected: String,
    version: String,
    asset: String,
    total: f64,
    received: f64,
    last_sent: f64,
}

pub struct Updater {
    update_dir: String,
    available: Option<Available>,
    /// The release a download began with; a later check cannot take it away.
    target: Option<Available>,
    downloaded: Option<String>,
    state: UpdateState,
    job: Option<Job>,
}

/// The exe to replace: the injected one when there is one, else
/// `PORTABLE_EXECUTABLE_FILE` (`||`, so an empty variable counts as unset).
pub fn resolve_target_exe(injected: Option<Option<&str>>, env_portable: Option<&str>) -> Option<String> {
    match injected {
        Some(t) => t.map(str::to_string),
        None => env_portable.filter(|s| !s.is_empty()).map(str::to_string),
    }
}

impl Updater {
    /// `update_dir` is `<userData>/updates`.
    pub fn new(update_dir: &str) -> Self {
        Self { update_dir: update_dir.to_string(), available: None, target: None, downloaded: None, state: UpdateState::phase("idle"), job: None }
    }

    pub fn get_state(&self) -> &UpdateState {
        &self.state
    }

    pub fn available(&self) -> Option<&Available> {
        self.available.as_ref()
    }

    fn emit(&mut self, state: UpdateState, channel: &'static str, payload: Value) -> Event {
        self.state = state;
        event(channel, payload)
    }

    fn error(&mut self, message: String) -> Vec<Event> {
        let state = UpdateState { message: Some(message.clone()), ..UpdateState::phase("error") };
        vec![self.emit(state, "update:error", json!({ "message": message }))]
    }

    // check ────────────────────────────────────────────────────────────────

    /// `check()` starting: the state becomes `checking`, with no event.
    pub fn begin_check(&mut self) {
        self.state = UpdateState::phase("checking");
    }

    pub fn check_request() -> Request {
        Request::new(RELEASE_API)
    }

    /// The fetch or one of its guards failed (a network error, `guard_response`, bad JSON).
    pub fn check_failed(&mut self, message: &str) -> Vec<Event> {
        self.error(message.to_string())
    }

    /// The feed answered with `json`.
    pub fn on_release_json(&mut self, json: &Value, current: &str) -> Vec<Event> {
        match latest_release(json) {
            Err(message) => self.error(message),
            Ok(Some(found)) if is_newer_version(&found.version, current) => {
                let version = found.version.clone();
                self.available = Some(found);
                let state = UpdateState { version: Some(version.clone()), ..UpdateState::phase("available") };
                vec![self.emit(state, "update:available", json!({ "version": version }))]
            }
            Ok(_) => {
                self.available = None;
                vec![self.emit(UpdateState::phase("uptodate"), "update:not-available", json!({}))]
            }
        }
    }

    // download ─────────────────────────────────────────────────────────────

    /// `download()` starting. `Err` is the rejected promise (no event, no state change);
    /// `Ok` is the checksum file to fetch.
    pub fn download_begin(&mut self) -> Result<Request, String> {
        let target = self.available.clone().ok_or(NO_UPDATE)?;
        // The URLs passed the host gate when the release was read, and the gate is a pure function.
        let sums = Request::new(&target.sums_url);
        self.target = Some(target);
        self.job = None;
        Ok(sums)
    }

    /// Any step of the download failed after `download_begin`.
    pub fn download_failed(&mut self, message: &str) -> Vec<Event> {
        self.job = None;
        self.error(message.to_string())
    }

    /// The checksum file arrived. `Ok` is the exe to fetch; `Err` carries the events.
    pub fn download_sums(&mut self, text: &str) -> Result<Request, Vec<Event>> {
        let Some(target) = self.target.clone() else { return Err(self.error(NO_UPDATE.into())) };
        match parse_checksum(text, &target.asset_name) {
            None => Err(self.error("Update refused: no checksum published for this release".into())),
            Some(expected) => {
                self.job = Some(Job { expected, version: target.version, asset: target.asset_name, total: 0.0, received: 0.0, last_sent: -1.0 });
                Ok(Request::new(&target.asset_url))
            }
        }
    }

    /// The exe's response headers: `content-length` as sent (none when absent).
    /// A response with no body is read whole, with no progress: skip `download_chunk`.
    pub fn download_head(&mut self, content_length: Option<&str>) {
        let n = content_length.map_or(0.0, js_number);
        if let Some(job) = &mut self.job {
            job.total = if n.is_nan() { 0.0 } else { n };
        }
    }

    /// A chunk of `len` bytes arrived: a progress event when the whole percent changed.
    pub fn download_chunk(&mut self, len: usize) -> Vec<Event> {
        let version = self.available.as_ref().map(|a| a.version.clone());
        let Some(job) = &mut self.job else { return vec![] };
        job.received += len as f64;
        if job.total > 0.0 {
            let percent = (job.received / job.total * 100.0).floor();
            if percent != job.last_sent {
                job.last_sent = percent;
                let p = if percent.is_finite() { json!(percent as u64) } else { Value::Null };
                let state = UpdateState { percent: Some(p.clone()), version, ..UpdateState::phase("downloading") };
                return vec![self.emit(state, "update:progress", json!({ "percent": p }))];
            }
        }
        vec![]
    }

    /// All bytes arrived; `actual_sha` is their SHA-256 as lowercase hex.
    /// `Ok` is where to write them (`write_staged`).
    pub fn download_finish(&mut self, actual_sha: &str) -> Result<Stage, Vec<Event>> {
        let Some(job) = self.job.take() else { return Err(self.error(NO_UPDATE.into())) };
        if actual_sha != job.expected.to_ascii_lowercase() {
            return Err(self.error("Update refused: the download did not match its published checksum".into()));
        }
        Ok(Stage { dir: self.update_dir.clone(), file: win_join(&self.update_dir, &job.asset), version: job.version })
    }

    /// The staged file is on disk.
    pub fn download_written(&mut self, stage: &Stage) -> Vec<Event> {
        self.downloaded = Some(stage.file.clone());
        let state = UpdateState { version: Some(stage.version.clone()), ..UpdateState::phase("ready") };
        vec![self.emit(state, "update:downloaded", json!({ "version": stage.version }))]
    }

    // install ──────────────────────────────────────────────────────────────

    /// D33: the verified download waiting to be installed, if there is one.
    /// The Rust app swaps it in itself; `install` below is Electron's script.
    pub fn staged(&self) -> Option<&str> {
        self.downloaded.as_deref()
    }

    /// `install()`: `Err` is the thrown error. `target_exe` from `resolve_target_exe`.
    pub fn install(&self, target_exe: Option<&str>) -> Result<InstallPlan, String> {
        let staged = self.downloaded.as_deref().ok_or("Update refused: nothing downloaded to install")?;
        let exe = target_exe
            .filter(|e| !e.is_empty())
            .ok_or("Update refused: this build cannot replace itself \u{2014} download the new version instead")?;
        let script_path = win_join(&self.update_dir, SCRIPT_NAME);
        Ok(InstallPlan {
            args: vec!["/c".into(), script_path.clone()],
            script_path,
            script: swap_script(staged, exe),
            program: "cmd.exe",
            log: "quitting to apply an update",
        })
    }

    /// `cleanup()`: drop a staged download a previous run already applied.
    pub fn cleanup(&self) -> io::Result<()> {
        let dir = Path::new(&self.update_dir);
        if !dir.exists() {
            return Ok(());
        }
        if dir.is_dir() { fs::remove_dir_all(dir) } else { fs::remove_file(dir) }
    }
}

/// `mkdirSync(dir, {recursive})` then `writeFileSync(file, bytes)`.
pub fn write_staged(stage: &Stage, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(&stage.dir)?;
    fs::write(&stage.file, bytes)
}

/// `writeFileSync(script, text, 'utf-8')`. The folder is not created.
pub fn write_install_script(plan: &InstallPlan) -> io::Result<()> {
    fs::write(&plan.script_path, plan.script.as_bytes())
}

/// `latestRelease()` after the fetch: `Ok(None)` when the feed lacks what an update needs.
fn latest_release(json: &Value) -> Result<Option<Available>, String> {
    let tag = prop(json, "tag_name")?.map_or_else(String::new, js_string);
    let version = js_trim(tag.strip_prefix('v').unwrap_or(&tag)).to_string();
    let assets = assets_list(json)?;
    let mut exe = None;
    for a in assets {
        let name = match prop(a, "name")? {
            None => None,
            Some(Value::String(s)) => Some(s),
            Some(_) => return Err("(a.name ?? \"\").toLowerCase is not a function".into()),
        };
        if name.is_some_and(|n| n.to_lowercase().ends_with(".exe")) {
            exe = Some(a);
            break;
        }
    }
    let sums = find_named(assets, SUMS_ASSET)?;
    fn url_of(a: Option<&Value>) -> Option<&Value> {
        a.and_then(|a| a.get("browser_download_url")).filter(|u| js_truthy(u))
    }
    let (Some(exe_url), Some(sums_url)) = (url_of(exe), url_of(sums)) else { return Ok(None) };
    let exe_name = exe.and_then(|a| a.get("name")).and_then(Value::as_str).unwrap_or("");
    if version.is_empty() || exe_name.is_empty() {
        return Ok(None);
    }
    let (exe_url, sums_url) = (js_string(exe_url), js_string(sums_url));
    for url in [&exe_url, &sums_url] {
        if !is_trusted_release_host(url) {
            return Err(format!("{PREFIX} refused: untrusted host \"{url}\""));
        }
    }
    // D32: the download is saved under this name, so it must be a plain file name.
    if !is_plain_file_name(exe_name) {
        return Err(format!("{PREFIX} refused: unsafe file name \"{exe_name}\""));
    }
    Ok(Some(Available { version, asset_name: exe_name.into(), asset_url: exe_url, sums_url }))
}

/// True for a name that is only a file name: no folder, drive or stream part,
/// no control character, nothing Windows would silently rename (a trailing dot
/// or space) and no reserved device name.
pub fn is_plain_file_name(name: &str) -> bool {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3",
        "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = name.split('.').next().unwrap_or("");
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.chars().any(|c| r#"/\:*?"<>|"#.contains(c) || c.is_control())
        && !name.ends_with(['.', ' '])
        && !RESERVED.iter().any(|r| stem.eq_ignore_ascii_case(r))
}
