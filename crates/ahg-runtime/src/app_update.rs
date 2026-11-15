//! The app's own updates: the decisions are the engine's
//! (`ahg_engine::updater`); this fetches, hashes, stages and swaps.
//!
//! D33: the swap needs no script. Windows lets a running program be renamed,
//! so the app renames its own EXE aside, moves the verified new one into its
//! place, and the new version removes the spare when it starts. No install
//! path is ever pasted into a command line, so no folder name can break it.
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use ahg_engine::updater::{Event, UpdateState, Updater, guard_request, guard_response};
use sha2::{Digest, Sha256};

use crate::engine_update::Fetch;

const PREFIX: &str = "Update";
const TEXT_LIMIT: u64 = 5 * 1024 * 1024;
/// The portable EXE is about 120 MB today; far beyond that is not a release.
const EXE_LIMIT: u64 = 600 * 1024 * 1024;
/// The download while it is arriving, inside the updates folder.
const PART: &str = "download.part";

type Notify = Box<dyn Fn(Event) + Send + Sync>;

pub struct AppUpdater {
    state: Mutex<Updater>,
    update_dir: PathBuf,
    fetch: Box<dyn Fetch>,
    /// The running app's version, such as "2.0.0".
    current: String,
    notify: Notify,
}

impl AppUpdater {
    /// `update_dir` is `<userData>/updates`.
    pub fn new(update_dir: &Path, current: &str, fetch: Box<dyn Fetch>, notify: Notify) -> Self {
        Self { state: Mutex::new(Updater::new(&update_dir.to_string_lossy())), update_dir: update_dir.to_owned(), fetch, current: current.to_owned(), notify }
    }

    fn state(&self) -> MutexGuard<'_, Updater> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn tell(&self, events: Vec<Event>) {
        events.into_iter().for_each(|event| (self.notify)(event));
    }

    pub fn status(&self) -> UpdateState {
        self.state().get_state().clone()
    }

    /// Drop what an earlier run staged: if its swap worked the file is gone
    /// already, and a stale half-download is worse than fetching again.
    pub fn cleanup(&self) {
        let _ = self.state().cleanup();
    }

    /// Ask GitHub whether a newer version exists. Blocking: call it from a worker thread.
    pub fn check(&self) -> UpdateState {
        self.state().begin_check();
        let events = match self.bytes(&Updater::check_request().url, TEXT_LIMIT).and_then(|body| serde_json::from_slice(&body).map_err(|_| format!("{PREFIX} failed: unreadable release feed"))) {
            Ok(feed) => self.state().on_release_json(&feed, &self.current),
            Err(message) => self.state().check_failed(&message),
        };
        self.tell(events);
        self.status()
    }

    /// Download the available update and verify it. Progress and the outcome
    /// arrive as events. `Err` only when there is nothing to download.
    /// Blocking: call it from a worker thread.
    pub fn download(&self) -> Result<UpdateState, String> {
        let sums = self.state().download_begin()?;
        if let Err(events) = self.fetch_and_stage(&sums.url) {
            self.tell(events);
        }
        Ok(self.status())
    }

    /// Everything after `download_begin`. `Err` carries the events that report the failure.
    fn fetch_and_stage(&self, sums_url: &str) -> Result<(), Vec<Event>> {
        let failed = |message: String| self.state().download_failed(&message);
        let sums = self.bytes(sums_url, TEXT_LIMIT).map_err(failed)?;
        let asset = self.state().download_sums(&String::from_utf8_lossy(&sums))?;

        let request = guard_request(&asset.url, PREFIX).map_err(failed)?;
        let reply = self.fetch.get(&request).ok_or_else(|| failed("fetch failed".into()))?;
        guard_response((200..300).contains(&reply.status), reply.status, Some(&reply.final_url), PREFIX).map_err(failed)?;
        self.state().download_head(reply.content_length.as_deref());

        let part = self.update_dir.join(PART);
        let sha = self.stream_to(reply.body, &part).map_err(|message| {
            let _ = fs::remove_file(&part);
            failed(message)
        })?;
        let stage = self.state().download_finish(&sha).inspect_err(|_| drop(fs::remove_file(&part)))?;
        fs::rename(&part, &stage.file).map_err(|e| {
            let _ = fs::remove_file(&part);
            failed(e.to_string())
        })?;
        let events = self.state().download_written(&stage);
        self.tell(events);
        Ok(())
    }

    /// Write `body` to `file`, reporting progress, and return its SHA-256.
    fn stream_to(&self, mut body: Box<dyn Read + Send>, file: &Path) -> Result<String, String> {
        fs::create_dir_all(&self.update_dir).map_err(|e| e.to_string())?;
        let mut out = fs::File::create(file).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut chunk = vec![0u8; 64 * 1024];
        let mut received = 0u64;
        loop {
            let n = body.read(&mut chunk).map_err(|_| "fetch failed".to_owned())?;
            if n == 0 {
                break;
            }
            received += n as u64;
            if received > EXE_LIMIT {
                return Err(format!("{PREFIX} refused: the download is too large"));
            }
            hasher.update(&chunk[..n]);
            out.write_all(&chunk[..n]).map_err(|e| e.to_string())?;
            let events = self.state().download_chunk(n);
            self.tell(events);
        }
        out.flush().map_err(|e| e.to_string())?;
        Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
    }

    /// Put the downloaded version in place of `target_exe`, the running app.
    /// Afterwards the caller shuts down, starts `target_exe` (now the new
    /// version) and exits. On failure the old EXE is where it was.
    pub fn install(&self, target_exe: &Path) -> Result<(), String> {
        let staged = self.state().staged().map(PathBuf::from).ok_or("Update refused: nothing downloaded to install")?;
        swap_in(&staged, target_exe).map_err(|e| format!("{PREFIX} failed: {e}"))
    }

    /// One guarded request, as in the yt-dlp updater.
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

/// Where the replaced EXE waits until the new version removes it.
fn spare_of(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(".old");
    PathBuf::from(name)
}

/// Replace `target` (which may be running) with `staged`. The old file is
/// kept beside it as `<name>.old`; if the new one cannot be put in place, the
/// old one is put back.
pub fn swap_in(staged: &Path, target: &Path) -> io::Result<()> {
    if !staged.is_file() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "the downloaded update is missing"));
    }
    let spare = spare_of(target);
    // Left by an update before this one, if the new version never got to remove it.
    let _ = fs::remove_file(&spare);
    fs::rename(target, &spare)?;
    // A rename cannot cross drives; the updates folder may be on another one.
    let placed = fs::rename(staged, target).or_else(|_| fs::copy(staged, target).map(|_| drop(fs::remove_file(staged))));
    if let Err(error) = placed {
        let _ = fs::remove_file(target);
        let _ = fs::rename(&spare, target);
        return Err(error);
    }
    Ok(())
}

/// At startup: remove the EXE the last update replaced. It could not be
/// deleted while it was still running. Nothing happens when there is none.
pub fn remove_spare(target: &Path) {
    let _ = fs::remove_file(spare_of(target));
}
