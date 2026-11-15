//! Running downloads. `Downloader` carries out the queue's `Download`,
//! `Cancel` and `CancelAll` effects and sends back `Report`s, which the caller
//! feeds into the queue's `on_progress`, `on_done`, `on_error` and `on_match`.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use ahg_engine::downloader::args::build_ytdlp_args;
use ahg_engine::downloader::metadata::search_results;
use ahg_engine::downloader::progress::{Outcome, OutputScan};
use ahg_engine::downloader::score::score_candidates;
use ahg_engine::text::js_trim;
use ahg_engine::types::{DownloadProgress, DownloadTask, SourcePlatform, Track};

use crate::proc::{Stream, Utf8Chunks, kill_tree, read_output};
use crate::ytdlp::YtDlp;

/// What a running download tells the queue. `attempt` is the one the queue
/// handed out with the task.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    Progress { id: String, attempt: u64, progress: DownloadProgress },
    Done { id: String, attempt: u64, path: String, filepaths: Vec<String>, already_on_disk: bool },
    Error { id: String, attempt: u64, message: String },
    /// The Spotify bridge chose this video; `confidence` is between 0 and 1.
    Match { id: String, attempt: u64, url: String, confidence: f64 },
}

/// One download in flight.
struct Running {
    attempt: u64,
    /// Set by a cancel. A killed download reports nothing more, so no error
    /// row appears after the user cancelled.
    killed: Arc<AtomicBool>,
    /// The process id, once the download process has started. While a Spotify
    /// search is still running there is none, and a cancel calls the download off.
    pid: Option<u32>,
}

type Active = Arc<Mutex<HashMap<String, Running>>>;

fn lock(active: &Active) -> std::sync::MutexGuard<'_, HashMap<String, Running>> {
    active.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Downloader {
    ytdlp: Arc<YtDlp>,
    active: Active,
}

impl Downloader {
    /// `ytdlp` is the resolved yt-dlp binary; `ffmpeg` the resolved ffmpeg, if found.
    pub fn new(ytdlp: PathBuf, ffmpeg: Option<String>) -> Self {
        Self::with(Arc::new(YtDlp::new(ytdlp, ffmpeg)))
    }

    pub fn with(ytdlp: Arc<YtDlp>) -> Self {
        Self { ytdlp, active: Arc::default() }
    }

    /// An extra environment variable for every process this downloader starts.
    /// Only before the first download.
    pub fn env(&mut self, key: &str, value: &str) {
        if let Some(ytdlp) = Arc::get_mut(&mut self.ytdlp) {
            ytdlp.env(key, value);
        }
    }

    /// How many downloads are in flight.
    pub fn running(&self) -> usize {
        lock(&self.active).len()
    }

    /// Start a download on its own thread. Reports arrive on `reports`.
    pub fn start(&self, task: DownloadTask, attempt: u64, reports: Sender<Report>) {
        let killed = Arc::new(AtomicBool::new(false));
        lock(&self.active).insert(task.task_id.clone(), Running { attempt, killed: killed.clone(), pid: None });
        let (ytdlp, active) = (self.ytdlp.clone(), self.active.clone());
        thread::spawn(move || {
            let id = task.task_id.clone();
            let send = |report: Report| {
                if !killed.load(Ordering::SeqCst) {
                    let _ = reports.send(report);
                }
            };
            let outcome = run(&ytdlp, &task, attempt, &active, &killed, &send);
            // Only this attempt's entry is removed: a pause then a quick resume
            // reuses the task id for a newer download.
            let mut map = lock(&active);
            if map.get(&id).is_some_and(|r| r.attempt == attempt) {
                map.remove(&id);
            }
            drop(map);
            match outcome {
                Outcome::Done { path, filepaths, already_on_disk } => send(Report::Done { id, attempt, path, filepaths, already_on_disk }),
                Outcome::Failed(message) => send(Report::Error { id, attempt, message }),
            }
        });
    }

    /// Stop one task's download and everything it started.
    pub fn cancel(&self, id: &str) {
        let removed = lock(&self.active).remove(id);
        if let Some(running) = removed {
            stop(&running);
        }
    }

    /// Stop every download.
    pub fn cancel_all(&self) {
        let all: Vec<Running> = lock(&self.active).drain().map(|(_, r)| r).collect();
        for running in &all {
            stop(running);
        }
    }
}

fn stop(running: &Running) {
    running.killed.store(true, Ordering::SeqCst);
    if let Some(pid) = running.pid {
        kill_tree(pid);
    }
}

/// Spotify audio cannot be downloaded, so the track is found on YouTube: a
/// five-result search is scored against the track and the best hit replaces
/// the link. If the search yields nothing, the first hit of a plain search is
/// downloaded instead. Returns the chosen video and the confidence in it.
fn spotify_bridge(ytdlp: &YtDlp, track: &Track, args: &mut [String]) -> Option<(String, f64)> {
    // The arguments reach yt-dlp as they are, with no shell, so quotes are removed.
    let clean = |s: &str| js_trim(&s.replace('"', "")).to_owned();
    let artist = if !track.artist.is_empty() { track.artist.clone() } else { track.artists.first().cloned().unwrap_or_default() };
    let query = format!("{} - {}", clean(&artist), clean(&track.name));
    let last = args.last_mut()?;
    *last = format!("ytsearch1:{query}");

    let items = ytdlp.run_json(&["--dump-json", "--no-warnings", "--flat-playlist", &format!("ytsearch5:{query}")]).ok()?;
    let best = score_candidates(track, &search_results(&items, SourcePlatform::Youtube)).into_iter().next()?;
    if best.candidate.url.is_empty() {
        return None;
    }
    *last = best.candidate.url.clone();
    Some((best.candidate.url, best.score.clamp(0.0, 1.0)))
}

/// Runs one download to its end, sending progress as it goes.
fn run(ytdlp: &YtDlp, task: &DownloadTask, attempt: u64, active: &Active, killed: &AtomicBool, send: &dyn Fn(Report)) -> Outcome {
    let id = task.task_id.as_str();
    let Some(bin) = ytdlp.bin() else {
        return Outcome::Failed("yt-dlp not found. Please ensure yt-dlp is installed or bundled in bin/.".to_owned());
    };
    let mut args = build_ytdlp_args(task, ytdlp.ffmpeg().as_deref());
    if task.source == SourcePlatform::Spotify
        && let Some(track) = &task.track
        && let Some((url, confidence)) = spotify_bridge(ytdlp, track, &mut args)
    {
        send(Report::Match { id: id.to_owned(), attempt, url, confidence });
    }
    // Cancelled while the search ran: the download must not start.
    if killed.load(Ordering::SeqCst) {
        return Outcome::Failed(String::new());
    }

    let mut cmd = ytdlp.command(&bin);
    cmd.args(args);
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => return Outcome::Failed(e.to_string()),
    };
    // Record the process id, unless a cancel arrived while it was starting.
    {
        let mut map = lock(active);
        match map.get_mut(id).filter(|r| r.attempt == attempt) {
            Some(running) => running.pid = Some(child.id()),
            None => killed.store(true, Ordering::SeqCst),
        }
    }
    if killed.load(Ordering::SeqCst) {
        kill_tree(child.id());
    }

    let mut scan = OutputScan::new().with_item_counts();
    let (mut out, mut err) = (Utf8Chunks::default(), Utf8Chunks::default());
    let progress = |events: Vec<DownloadProgress>| {
        for progress in events {
            send(Report::Progress { id: id.to_owned(), attempt, progress });
        }
    };
    for (stream, bytes) in read_output(&mut child) {
        // Output still draining from a killed process must not be attributed
        // to the download that replaced it.
        if killed.load(Ordering::SeqCst) {
            continue;
        }
        progress(match stream {
            Stream::Stdout => scan.push_stdout(&out.push(&bytes)),
            Stream::Stderr => scan.push_stderr(&err.push(&bytes)),
        });
    }
    let code = child.wait().ok().and_then(|status| status.code());
    progress(scan.push_stdout(&out.finish()));
    progress(scan.push_stderr(&err.finish()));
    let (events, outcome) = scan.finish(code);
    progress(events);
    outcome
}
