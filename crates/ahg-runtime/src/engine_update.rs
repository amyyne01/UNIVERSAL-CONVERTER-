//! Keeping yt-dlp current: the decisions are the engine's
//! (`ahg_engine::ytdlp_updater`); this fetches what it asks for, hashes what
//! arrives and puts a verified binary in place.
//!
//! The new binary lands in `<userData>/bin`, the first place a shipped build
//! runs yt-dlp from. So nothing is written there unless it came from a trusted
//! release host and its SHA-256 is the one that release published.
use std::io::Read;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use ahg_engine::updater::{Event, Request, fs_error_message, guard_request, guard_response};
use ahg_engine::ytdlp_updater::{CheckBegin, UpdateBegin, YtdlpStatus, YtdlpUpdater, latest_release, write_swap};
use sha2::{Digest, Sha256};

const PREFIX: &str = "Engine update";
/// The release feed and the checksum list are small; the binary is about 18 MB.
/// A reply beyond these is not what was asked for.
const TEXT_LIMIT: u64 = 5 * 1024 * 1024;
const BINARY_LIMIT: u64 = 150 * 1024 * 1024;

/// The answer to one request.
pub struct Fetched {
    pub status: u16,
    /// Where the answer finally came from, after any redirects.
    pub final_url: String,
    /// The `content-length` header as sent, if any.
    pub content_length: Option<String>,
    pub body: Box<dyn Read + Send>,
}

/// Makes the requests the updaters ask for. `None` when a request fails
/// outright. The real one is `http::Releases`; tests script their own.
pub trait Fetch: Send + Sync {
    fn get(&self, request: &Request) -> Option<Fetched>;
}

/// One transport may serve both updaters.
impl<T: Fetch + ?Sized> Fetch for std::sync::Arc<T> {
    fn get(&self, request: &Request) -> Option<Fetched> {
        (**self).get(request)
    }
}

type Version = Box<dyn Fn() -> String + Send + Sync>;
type Swapped = Box<dyn Fn() + Send + Sync>;
type Notify = Box<dyn Fn(Event) + Send + Sync>;

pub struct EngineUpdater {
    state: Mutex<YtdlpUpdater>,
    fetch: Box<dyn Fetch>,
    /// The installed yt-dlp version, "" when there is none.
    current: Version,
    /// Called once a new binary is in place, so it is found on the next run.
    swapped: Swapped,
    notify: Notify,
}

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_millis() as f64)
}

impl EngineUpdater {
    /// `bin_dir` is `<userData>/bin`.
    pub fn new(bin_dir: &str, fetch: Box<dyn Fetch>, current: Version, swapped: Swapped, notify: Notify) -> Self {
        Self { state: Mutex::new(YtdlpUpdater::new(bin_dir)), fetch, current, swapped, notify }
    }

    fn state(&self) -> MutexGuard<'_, YtdlpUpdater> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn tell(&self, events: Vec<Event>) {
        events.into_iter().for_each(|event| (self.notify)(event));
    }

    pub fn status(&self) -> YtdlpStatus {
        self.state().status().clone()
    }

    /// Ask the release feed what is available. The answer is kept for a day
    /// unless `force`. Blocking: call it from a worker thread.
    pub fn check(&self, force: bool) -> YtdlpStatus {
        let begin = self.state().check_begin(force, now(), &(self.current)());
        let (request, events) = match begin {
            CheckBegin::Return(status) => return status,
            CheckBegin::Fetch { request, events } => (request, events),
        };
        self.tell(events);
        let latest = self.json(&request.url).and_then(|feed| latest_release(&feed));
        let (status, events) = self.state().check_end(latest, now(), &(self.current)());
        self.tell(events);
        status
    }

    /// Download the latest yt-dlp, verify it against the published checksum
    /// and put it in place. Blocking: call it from a worker thread.
    pub fn update(&self) -> YtdlpStatus {
        let begin = self.state().update_begin();
        let (request, events) = match begin {
            UpdateBegin::Busy(status) => return status,
            UpdateBegin::Go { request, events } => (request, events),
        };
        self.tell(events);
        let (status, events) = match self.fetch_verify_swap(&request.url) {
            Ok(tag) => {
                (self.swapped)();
                self.state().update_done(&tag, &(self.current)(), now())
            }
            Err(message) => self.state().update_fail(&message),
        };
        self.tell(events);
        status
    }

    /// A download failed with `error`. True when that looks like an outdated
    /// yt-dlp and the caller should now run `update` (once per session, and
    /// only while automatic engine updates are on).
    pub fn on_download_failure(&self, error: &str, auto_update_enabled: bool) -> bool {
        self.state().handle_download_failure(error, auto_update_enabled)
    }

    /// The tag of the release now installed, or why nothing was installed.
    fn fetch_verify_swap(&self, feed_url: &str) -> Result<String, String> {
        let latest = latest_release(&self.json(feed_url)?)?;
        let binary = self.bytes(&latest.asset_request().url, BINARY_LIMIT)?;
        let sums = String::from_utf8_lossy(&self.bytes(&latest.sums_request().url, TEXT_LIMIT)?).into_owned();
        let sha: String = Sha256::digest(&binary).iter().map(|b| format!("{b:02x}")).collect();
        let plan = self.state().update_verify(&sums, &sha)?;
        write_swap(&plan, &binary).map_err(|e| fs_error_message(&e, "rename", &plan.target))?;
        Ok(latest.tag)
    }

    fn json(&self, url: &str) -> Result<serde_json::Value, String> {
        serde_json::from_slice(&self.bytes(url, TEXT_LIMIT)?).map_err(|_| format!("{PREFIX} failed: unreadable release feed"))
    }

    /// One guarded request: a trusted host before, a good status and a
    /// trusted final host after, and no more bytes than `limit`.
    fn bytes(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let request = guard_request(url, PREFIX)?;
        let reply = self.fetch.get(&request).ok_or("fetch failed")?;
        guard_response((200..300).contains(&reply.status), reply.status, Some(&reply.final_url), PREFIX)?;
        let mut body = Vec::new();
        reply.body.take(limit + 1).read_to_end(&mut body).map_err(|_| "fetch failed".to_owned())?;
        if body.len() as u64 > limit {
            return Err(format!("{PREFIX} refused: the reply is too large"));
        }
        Ok(body)
    }
}
