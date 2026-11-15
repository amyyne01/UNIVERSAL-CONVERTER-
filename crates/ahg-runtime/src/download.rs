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
use ahg_engine::downloader::bridge::{CONFIRMED, GUESS_CEILING, Lookup, Verdict, match_song, song_query, song_rows, song_search_page, song_url, verify};
use ahg_engine::downloader::metadata::search_results;
use ahg_engine::downloader::progress::{Outcome, OutputScan};
use ahg_engine::downloader::score::score_candidates;
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

/// Asks YouTube Music's own song search for `query` and returns the response
/// text, or `None` when it cannot be reached.
pub type SongSearch = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// How many songs the yt-dlp backup looks up before giving up.
const LOOKUPS: usize = 3;

pub struct Downloader {
    ytdlp: Arc<YtDlp>,
    active: Active,
    song_search: Option<SongSearch>,
}

impl Downloader {
    /// `ytdlp` is the resolved yt-dlp binary; `ffmpeg` the resolved ffmpeg, if found.
    pub fn new(ytdlp: PathBuf, ffmpeg: Option<String>) -> Self {
        Self::with(Arc::new(YtDlp::new(ytdlp, ffmpeg)))
    }

    pub fn with(ytdlp: Arc<YtDlp>) -> Self {
        Self { ytdlp, active: Arc::default(), song_search: None }
    }

    /// An extra environment variable for every process this downloader starts.
    /// Only before the first download.
    pub fn env(&mut self, key: &str, value: &str) {
        if let Some(ytdlp) = Arc::get_mut(&mut self.ytdlp) {
            ytdlp.env(key, value);
        }
    }

    /// D34: match Spotify tracks through YouTube Music's song search. Without
    /// it the bridge is the plain scored search, as in Electron.
    pub fn set_song_search(&mut self, search: SongSearch) {
        self.song_search = Some(search);
    }

    /// How many downloads are in flight.
    pub fn running(&self) -> usize {
        lock(&self.active).len()
    }

    /// Start a download on its own thread. Reports arrive on `reports`.
    pub fn start(&self, task: DownloadTask, attempt: u64, reports: Sender<Report>) {
        let killed = Arc::new(AtomicBool::new(false));
        lock(&self.active).insert(task.task_id.clone(), Running { attempt, killed: killed.clone(), pid: None });
        let (ytdlp, active, song_search) = (self.ytdlp.clone(), self.active.clone(), self.song_search.clone());
        thread::spawn(move || {
            let id = task.task_id.clone();
            let send = |report: Report| {
                if !killed.load(Ordering::SeqCst) {
                    let _ = reports.send(report);
                }
            };
            let outcome = run(&ytdlp, &task, attempt, &active, &killed, song_search.as_ref(), &send);
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

/// Spotify audio cannot be downloaded, so the track is found on YouTube.
/// Returns the chosen video and the confidence in it: 1.0 for a confirmed
/// match, less for a guess. With nothing chosen, the first hit of a plain
/// search is downloaded.
///
/// D34, with a song search: YouTube Music's songs come first, and a row whose
/// artist, name and length agree with the track is taken as it is. If that
/// search cannot be reached, yt-dlp reads the same song list and a full lookup
/// must confirm the song. Only then is the plain search scored, as in
/// Electron, and what it finds is never reported as confirmed.
fn spotify_bridge(ytdlp: &YtDlp, task: &DownloadTask, track: &Track, args: &mut [String], song_search: Option<&SongSearch>) -> Option<(String, f64)> {
    let query = song_query(track);
    let last = args.last_mut()?;
    *last = format!("ytsearch1:{query}");

    if let Some(search) = song_search {
        // Already chosen, by the playlist preview or by an earlier attempt:
        // no second search. The address may come from a saved file, so it is checked.
        let chosen = task.matched_url.as_deref().and_then(song_url).map(|url| (url.to_owned(), task.match_confidence.unwrap_or(GUESS_CEILING)));
        let song = chosen.or_else(|| match search(&query).map(|response| song_rows(&response)).filter(|rows| !rows.is_empty()) {
            Some(rows) => match_song(track, &rows).map(|found| (found.url, found.confidence)),
            None => confirmed_by_lookup(ytdlp, track, &query).map(|url| (url, CONFIRMED)),
        });
        if let Some((url, confidence)) = song {
            *last = url.clone();
            return Some((url, confidence));
        }
    }

    let items = ytdlp.run_json(&["--dump-json", "--no-warnings", "--flat-playlist", &format!("ytsearch5:{query}")]).ok()?;
    let best = score_candidates(track, &search_results(&items, SourcePlatform::Youtube)).into_iter().next()?;
    if best.candidate.url.is_empty() {
        return None;
    }
    *last = best.candidate.url.clone();
    let confidence = best.score.clamp(0.0, 1.0);
    Some((best.candidate.url, if song_search.is_some() { confidence.min(GUESS_CEILING) } else { confidence }))
}

/// The backup for an unreachable song search: yt-dlp lists the songs, which
/// then carry only an id and a title, so each is looked up in full until the
/// music metadata of one confirms it.
fn confirmed_by_lookup(ytdlp: &YtDlp, track: &Track, query: &str) -> Option<String> {
    let songs = ytdlp.run_json(&["--dump-json", "--no-warnings", "--flat-playlist", "--playlist-end", &LOOKUPS.to_string(), &song_search_page(query)]).ok()?;
    songs.iter().take(LOOKUPS).filter_map(|song| song["id"].as_str()).map(|id| format!("https://music.youtube.com/watch?v={id}")).find(|url| {
        let found = ytdlp.run_json(&["--dump-json", "--no-warnings", "--no-playlist", "--", url]).ok();
        let look = found.and_then(|items| items.into_iter().next()).and_then(|item| serde_json::from_value::<Lookup>(item).ok());
        look.is_some_and(|look| verify(track, &look) == Verdict::Confirmed)
    })
}

/// Runs one download to its end, sending progress as it goes.
fn run(ytdlp: &YtDlp, task: &DownloadTask, attempt: u64, active: &Active, killed: &AtomicBool, song_search: Option<&SongSearch>, send: &dyn Fn(Report)) -> Outcome {
    let id = task.task_id.as_str();
    let Some(bin) = ytdlp.bin() else {
        return Outcome::Failed("yt-dlp not found. Please ensure yt-dlp is installed or bundled in bin/.".to_owned());
    };
    let mut args = build_ytdlp_args(task, ytdlp.ffmpeg().as_deref());
    if task.source == SourcePlatform::Spotify
        && let Some(track) = &task.track
        && let Some((url, confidence)) = spotify_bridge(ytdlp, task, track, &mut args, song_search)
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
