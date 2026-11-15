//! The yt-dlp engine self-update, "Binary Doctor" (electron/ytdlp-updater.ts),
//! minus the network call and the hashing. It reuses the app updater's host
//! gate, checksum parser, `Request`, `Event` and path join.
//!
//! The owner's loop (every method takes the clock and the installed yt-dlp
//! version as arguments, so nothing here reads either):
//!   check:  `check_begin(force, now, current)`; on `Fetch`, fetch `api_request()`
//!           through `guard_request` / `guard_response`, `latest_release(json)`,
//!           then `check_end(result, now, current)`.
//!   update: `update_begin()`; on `Go`, fetch the API, then `latest.asset_request()`,
//!           then `latest.sums_request()` (in that order), SHA-256 the binary,
//!           `update_verify(sums_text, sha)`, `write_swap(plan, bytes)`, reset the
//!           downloader's binary cache, `update_done(tag, current, now)`.
//!           Any failing step is `update_fail(message)`.
//!   failure: `handle_download_failure(stderr, auto_update_enabled)`; when it
//!           returns true, run `update`.
//! Validation of the new binary (`--version`) is the downloader's job at its next
//! spawn; this module only checks the checksum.
use std::fs;
use std::io;

use serde::Serialize;
use serde_json::Value;

use crate::text::js_trim;
use crate::updater::{
    Event, Request, assets_list, event, find_named, is_trusted_release_host, js_string, js_truthy, parse_checksum, prop, win_join,
};

pub const RELEASE_API: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
pub const SUMS_ASSET: &str = "SHA2-256SUMS";
pub const ASSET_NAME: &str = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };
/// Checks are cached for 24 hours.
pub const CHECK_TTL_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;
const PREFIX: &str = "Engine update";

/// yt-dlp stderr that means the ENGINE is stale (the site changed, an extractor broke).
pub fn is_engine_stale(text: &str) -> bool {
    let t = text.to_lowercase();
    [
        "unable to extract",
        "failed to extract",
        "nsig extraction failed",
        "update to the latest version",
        "yt-dlp is out of date",
        "please report this issue",
    ]
    .iter()
    .any(|p| t.contains(p))
}

/// `YtdlpUpdateStatus` of shared/types.ts, sent on `ytdlp:status`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YtdlpStatus {
    pub current: String,
    pub latest: String,
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Latest {
    pub tag: String,
    asset_url: String,
    sums_url: String,
}

impl Latest {
    pub fn asset_request(&self) -> Request {
        Request::new(&self.asset_url)
    }

    pub fn sums_request(&self) -> Request {
        Request::new(&self.sums_url)
    }
}

/// `latestRelease()` after the fetch.
pub fn latest_release(json: &Value) -> Result<Latest, String> {
    let tag = js_trim(&prop(json, "tag_name")?.map_or_else(String::new, js_string)).to_string();
    let assets = assets_list(json)?;
    let url_of = |name: &str| -> Result<Option<&Value>, String> {
        Ok(find_named(assets, name)?.and_then(|a| a.get("browser_download_url")).filter(|u| !u.is_null()))
    };
    let (asset_url, sums_url) = (url_of(ASSET_NAME)?, url_of(SUMS_ASSET)?);
    let present = |u: Option<&Value>| u.filter(|u| js_truthy(u)).map(js_string);
    let (Some(asset_url), Some(sums_url), false) = (present(asset_url), present(sums_url), tag.is_empty()) else {
        return Err(format!("{PREFIX} refused: incomplete release feed"));
    };
    for url in [&asset_url, &sums_url] {
        if !is_trusted_release_host(url) {
            return Err(format!("{PREFIX} refused: untrusted host \"{url}\""));
        }
    }
    Ok(Latest { tag, asset_url, sums_url })
}

/// Where a verified binary goes: written to `tmp`, renamed over `target`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapPlan {
    pub dir: String,
    pub target: String,
    pub tmp: String,
}

pub enum CheckBegin {
    /// Cached, or another call is in flight: the status as it is, no events.
    Return(YtdlpStatus),
    Fetch { request: Request, events: Vec<Event> },
}

pub enum UpdateBegin {
    /// Another call is in flight: the status as it is, no events.
    Busy(YtdlpStatus),
    Go { request: Request, events: Vec<Event> },
}

pub struct YtdlpUpdater {
    bin_dir: String,
    status: YtdlpStatus,
    last_check: f64,
    busy: bool,
    reactive_tried: bool,
}

impl YtdlpUpdater {
    /// `bin_dir` is `<userData>/bin`, searched before the bundled copy.
    pub fn new(bin_dir: &str) -> Self {
        Self {
            bin_dir: bin_dir.to_string(),
            status: YtdlpStatus { current: String::new(), latest: String::new(), state: "idle", message: None },
            last_check: 0.0,
            busy: false,
            reactive_tried: false,
        }
    }

    pub fn status(&self) -> &YtdlpStatus {
        &self.status
    }

    pub fn api_request() -> Request {
        Request::new(RELEASE_API)
    }

    fn emit(&mut self, status: YtdlpStatus) -> (YtdlpStatus, Vec<Event>) {
        self.status = status.clone();
        let payload = serde_json::to_value(&status).expect("a status is plain data");
        (status, vec![event("ytdlp:status", payload)])
    }

    fn error(&mut self, message: &str) -> (YtdlpStatus, Vec<Event>) {
        self.busy = false;
        self.emit(YtdlpStatus { state: "error", message: Some(message.to_string()), ..self.status.clone() })
    }

    // check ────────────────────────────────────────────────────────────────

    /// `check(force)` starting. `now` is `Date.now()`, `current` the installed yt-dlp version.
    pub fn check_begin(&mut self, force: bool, now: f64, current: &str) -> CheckBegin {
        let cached = self.last_check != 0.0 && !self.last_check.is_nan() && now - self.last_check < CHECK_TTL_MS;
        if (!force && cached) || self.busy {
            return CheckBegin::Return(self.status.clone());
        }
        self.busy = true;
        let (_, events) = self.emit(YtdlpStatus { current: current.to_string(), state: "checking", ..self.status.clone() });
        CheckBegin::Fetch { request: Self::api_request(), events }
    }

    /// The feed was fetched and read (`latest_release`), or a step failed with a message.
    pub fn check_end(&mut self, latest: Result<Latest, String>, now: f64, current: &str) -> (YtdlpStatus, Vec<Event>) {
        match latest {
            Err(message) => self.error(&message),
            Ok(latest) => {
                self.last_check = now;
                self.busy = false;
                let up_to_date = !current.is_empty() && !latest.tag.is_empty() && current == latest.tag;
                let state = if up_to_date { "uptodate" } else { "available" };
                self.emit(YtdlpStatus { current: current.to_string(), latest: latest.tag, state, message: None })
            }
        }
    }

    // update ───────────────────────────────────────────────────────────────

    /// `update()` starting.
    pub fn update_begin(&mut self) -> UpdateBegin {
        if self.busy {
            return UpdateBegin::Busy(self.status.clone());
        }
        self.busy = true;
        let (_, events) = self.emit(YtdlpStatus { state: "downloading", ..self.status.clone() });
        UpdateBegin::Go { request: Self::api_request(), events }
    }

    /// Any step of the update failed.
    pub fn update_fail(&mut self, message: &str) -> (YtdlpStatus, Vec<Event>) {
        self.error(message)
    }

    /// The binary and the checksum file are both fetched; `actual_sha` is the
    /// SHA-256 of the binary as lowercase hex. `Err` is the message to `update_fail` with.
    pub fn update_verify(&self, sums_text: &str, actual_sha: &str) -> Result<SwapPlan, String> {
        let expected = parse_checksum(sums_text, ASSET_NAME).ok_or("Engine update refused: no published checksum")?;
        if actual_sha != expected {
            return Err("Engine update refused: checksum mismatch".into());
        }
        let target = win_join(&self.bin_dir, ASSET_NAME);
        Ok(SwapPlan { dir: self.bin_dir.clone(), tmp: format!("{target}.tmp"), target })
    }

    /// The binary is swapped in and the downloader's cache reset.
    pub fn update_done(&mut self, tag: &str, current: &str, now: f64) -> (YtdlpStatus, Vec<Event>) {
        self.last_check = now;
        self.busy = false;
        self.emit(YtdlpStatus { current: current.to_string(), latest: tag.to_string(), state: "updated", message: None })
    }

    /// The reactive trigger: true when the owner should now run `update` (at most
    /// once per session, only for a stale-extractor error, only with auto-update on).
    pub fn handle_download_failure(&mut self, error: &str, auto_update_enabled: bool) -> bool {
        if self.reactive_tried || !is_engine_stale(error) || !auto_update_enabled {
            return false;
        }
        self.reactive_tried = true;
        true
    }
}

/// `mkdirSync(dir, {recursive})`, write `<target>.tmp`, rename over the target;
/// a failed rename removes the tmp file and returns the error.
pub fn write_swap(plan: &SwapPlan, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(&plan.dir)?;
    fs::write(&plan.tmp, bytes)?;
    let renamed = fs::rename(&plan.tmp, &plan.target);
    if renamed.is_err() {
        let _ = fs::remove_file(&plan.tmp);
    }
    renamed
}
