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
use ahg_engine::downloader::progress::{Outcome, OutputScan};
use ahg_engine::types::{DownloadProgress, DownloadTask};

use crate::proc::{Stream, Utf8Chunks, hidden, kill_tree, read_output};

/// What a running download tells the queue. `attempt` is the one the queue
/// handed out with the task.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    Progress { id: String, attempt: u64, progress: DownloadProgress },
    Done { id: String, attempt: u64, path: String, filepaths: Vec<String>, already_on_disk: bool },
    Error { id: String, attempt: u64, message: String },
}

/// One download in flight.
struct Running {
    attempt: u64,
    /// Set by a cancel. A killed download reports nothing more, so no error
    /// row appears after the user cancelled.
    killed: Arc<AtomicBool>,
    /// The process id, once the process has started.
    pid: Option<u32>,
}

type Active = Arc<Mutex<HashMap<String, Running>>>;

fn lock(active: &Active) -> std::sync::MutexGuard<'_, HashMap<String, Running>> {
    active.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Downloader {
    ytdlp: PathBuf,
    ffmpeg: Option<String>,
    env: Vec<(String, String)>,
    active: Active,
}

impl Downloader {
    /// `ytdlp` is the resolved yt-dlp binary; `ffmpeg` the resolved ffmpeg, if found.
    pub fn new(ytdlp: PathBuf, ffmpeg: Option<String>) -> Self {
        Self { ytdlp, ffmpeg, env: Vec::new(), active: Arc::default() }
    }

    /// An extra environment variable for every process this downloader starts.
    pub fn env(&mut self, key: &str, value: &str) {
        self.env.push((key.to_owned(), value.to_owned()));
    }

    /// How many downloads are in flight.
    pub fn running(&self) -> usize {
        lock(&self.active).len()
    }

    /// Start a download on its own thread. Reports arrive on `reports`.
    pub fn start(&self, task: DownloadTask, attempt: u64, reports: Sender<Report>) {
        let killed = Arc::new(AtomicBool::new(false));
        lock(&self.active).insert(task.task_id.clone(), Running { attempt, killed: killed.clone(), pid: None });
        let args = build_ytdlp_args(&task, self.ffmpeg.as_deref());
        let mut cmd = hidden(&self.ytdlp);
        cmd.args(args).envs(self.env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        let active = self.active.clone();
        thread::spawn(move || {
            let id = task.task_id;
            let send = |report: Report| {
                if !killed.load(Ordering::SeqCst) {
                    let _ = reports.send(report);
                }
            };
            let outcome = run(cmd, &id, attempt, &active, &killed, &send);
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

/// Runs the process to its end, sending progress as it goes.
fn run(
    mut cmd: std::process::Command,
    id: &str,
    attempt: u64,
    active: &Active,
    killed: &AtomicBool,
    send: &dyn Fn(Report),
) -> Outcome {
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
