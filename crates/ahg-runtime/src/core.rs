//! The app core: the one object the interface talks to. It owns the settings,
//! the engine thread and the statistics, and keeps them consistent with each
//! other and with the plan the user is on.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ahg_engine::api::build_task;
use ahg_engine::atomic_write::write_file_atomic;
use ahg_engine::clock::LocalTime;
use ahg_engine::config::ConfigManager;
use ahg_engine::log::init_log;
use ahg_engine::queue::{Event, new_task_id};
use ahg_engine::scheduler::{Fire, POLL_MS, ScheduleConfig, Scheduler};
use ahg_engine::stats::{DownloadStats, StatsStore, stats_file_path};
use ahg_engine::tier::{Plan, clamp_concurrency};
use ahg_engine::ytdlp_updater::YtdlpStatus;
use ahg_engine::downloader::bridge::{CONFIRMED, SongMatch, match_song, song_query, song_rows, song_url};
use ahg_engine::types::{DownloadTask, MediaMetadata, SearchResult, Track};
use ahg_engine::validate::media_url_arg;
use serde_json::{Map, Value, json};

use crate::download::{Downloader, SongSearch};
use crate::engine_update::{EngineUpdater, Fetch};
use crate::engine::{Engine, Notice};
use crate::ytdlp::YtDlp;

/// Where the app keeps its data and where downloads go by default.
pub struct Paths {
    /// The per-user data folder, shared with the Electron app.
    pub user_data: PathBuf,
    /// The Downloads folder, used when no output folder is set.
    pub downloads: String,
}

/// What the core tells the interface.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreEvent {
    /// A message about one download.
    Download(Event),
    /// The queue's activity changed.
    QueueChanged,
    SettingsChanged,
    /// The raw error text of a failed download.
    DownloadFailed(String),
    /// The yt-dlp updater's state changed: `current`, `latest`, `state`
    /// ("idle", "checking", "available", "uptodate", "downloading", "updated",
    /// "error") and, on an error, `message`.
    EngineStatus(Value),
    /// The scheduled time came and the queue was resumed. The interface shows
    /// "Scheduled downloads started." when notifications are on.
    ScheduleFired,
    /// A scheduled run that asked for it has finished: shut the PC down.
    ShutdownRequested,
}

/// After a scheduled run with nothing to resume, no queue change will ever
/// come; look once after downloads have had this long to start.
const SHUTDOWN_CHECK: Duration = Duration::from_secs(10);

/// Choices only the Rust app has. They live in their own file, so the
/// settings file stays exactly what Electron 1.x writes and reads.
fn own_settings_file(user_data: &std::path::Path) -> PathBuf {
    user_data.join("settings-rust.json")
}

/// D34: the matches the user chose, by Spotify track id.
fn remembered_file(user_data: &std::path::Path) -> PathBuf {
    user_data.join("spotify-matches.json")
}

/// The saved choices. The file is outside input: an entry is kept only if it
/// is a whole match whose address is YouTube's.
fn load_remembered(file: &std::path::Path) -> HashMap<String, SongMatch> {
    let saved = std::fs::read_to_string(file).ok().and_then(|text| serde_json::from_str::<Map<String, Value>>(&text).ok()).unwrap_or_default();
    saved.into_iter().filter_map(|(id, entry)| Some((id, serde_json::from_value::<SongMatch>(entry).ok()?))).filter(|(_, m)| song_url(&m.url).is_some()).collect()
}

/// D11: the day the schedule last fired, kept across restarts.
fn schedule_state_file(user_data: &std::path::Path) -> PathBuf {
    user_data.join("scheduler-state.json")
}

fn last_fired_date(file: &std::path::Path) -> String {
    let saved = std::fs::read_to_string(file).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
    saved.as_ref().and_then(|v| v["lastFiredDate"].as_str()).unwrap_or_default().to_owned()
}

/// What the playlist preview learns about one Spotify track (D34).
#[derive(Debug, Clone, PartialEq)]
pub enum TrackMatch {
    /// This YouTube Music song is the track; its confidence says how surely.
    Found(SongMatch),
    /// The search answered and no song agrees. The download will still try
    /// ordinary videos.
    NoSong,
    /// The search could not be reached or gave nothing readable: not "no
    /// match", and worth asking again.
    Unreachable,
}

/// Which plan the user is on right now. Asked on every request, so an
/// activation or a revocation takes effect at once.
pub type PlanSource = Box<dyn Fn() -> Plan + Send + Sync>;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Core {
    paths: Paths,
    config: Arc<Mutex<ConfigManager>>,
    stats: Arc<Mutex<StatsStore>>,
    ytdlp: Arc<YtDlp>,
    plan: Arc<dyn Fn() -> Plan + Send + Sync>,
    engine: Arc<Engine>,
    events: Sender<CoreEvent>,
    stop: Sender<()>,
    dispatcher: Option<JoinHandle<()>>,
    song_search: SongSearch,
    hold_unconfirmed: Arc<AtomicBool>,
    remembered: Mutex<HashMap<String, SongMatch>>,
    engine_updater: Arc<EngineUpdater>,
}

impl Core {
    /// Load the settings, the history and the statistics, and start the engine.
    pub fn start(paths: Paths, ytdlp: Arc<YtDlp>, plan: PlanSource, events: Sender<CoreEvent>) -> Self {
        let proxy = {
            let ytdlp = ytdlp.clone();
            move || ytdlp.proxy()
        };
        Self::start_with_search(paths, ytdlp, plan, events, crate::http::song_search(proxy))
    }

    /// `start`, with the YouTube Music song search given: the real one, or a
    /// stand-in for tests.
    pub fn start_with_search(paths: Paths, ytdlp: Arc<YtDlp>, plan: PlanSource, events: Sender<CoreEvent>, song_search: SongSearch) -> Self {
        Self::start_with(paths, ytdlp, plan, events, song_search, Box::new(crate::http::Releases))
    }

    /// `start`, with everything that reaches the network given.
    pub fn start_with(paths: Paths, ytdlp: Arc<YtDlp>, plan: PlanSource, events: Sender<CoreEvent>, song_search: SongSearch, releases: Box<dyn Fetch>) -> Self {
        let engine_updater = {
            let (version, found, tell) = (ytdlp.clone(), ytdlp.clone(), events.clone());
            Arc::new(EngineUpdater::new(
                &paths.user_data.join("bin").to_string_lossy(),
                releases,
                Box::new(move || version.version()),
                Box::new(move || found.reset_cache()),
                Box::new(move |event| drop(tell.send(CoreEvent::EngineStatus(event.payload)))),
            ))
        };
        init_log(&paths.user_data);
        let config = Arc::new(Mutex::new(ConfigManager::new(&paths.user_data, paths.downloads.clone())));
        let mut stats = StatsStore::default();
        stats.init(&stats_file_path(&paths.user_data));
        let stats = Arc::new(Mutex::new(stats));

        let (notices, incoming) = channel();
        let remembered = Mutex::new(load_remembered(&remembered_file(&paths.user_data)));
        let saved = std::fs::read_to_string(own_settings_file(&paths.user_data)).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
        let hold_unconfirmed = Arc::new(AtomicBool::new(saved.is_some_and(|v| v["holdUnconfirmed"] == true)));
        let mut downloader = Downloader::with(ytdlp.clone());
        downloader.set_song_search(song_search.clone());
        downloader.set_hold_unconfirmed(hold_unconfirmed.clone());
        let engine = Arc::new(Engine::start(downloader, &paths.user_data.join("downloads-history.json"), notices));
        let plan: Arc<dyn Fn() -> Plan + Send + Sync> = Arc::from(plan);
        let (stop, stopped) = channel();
        let dispatcher = {
            let (config, stats, events, engine, plan) = (config.clone(), stats.clone(), events.clone(), engine.clone(), plan.clone());
            let engine_updater = engine_updater.clone();
            let state_file = schedule_state_file(&paths.user_data);
            let mut scheduler = Scheduler::restored(&last_fired_date(&state_file));
            // One look at the schedule. When it is due: resume the queue, remember
            // the day, and answer whether the PC should be shut down afterwards.
            let mut check_schedule = {
                let (config, events, engine) = (config.clone(), events.clone(), engine.clone());
                move || -> Option<bool> {
                    let settings = serde_json::from_value::<ScheduleConfig>(Value::Object(lock(&config).get_all().clone())).ok()?;
                    let Fire::Trigger { shutdown } = scheduler.poll(&settings, plan() == Plan::Premium, &LocalTime::now()) else { return None };
                    let _ = write_file_atomic(&state_file, &json!({ "lastFiredDate": scheduler.last_fired_date() }).to_string());
                    engine.resume_all();
                    let _ = events.send(CoreEvent::ScheduleFired);
                    Some(shutdown)
                }
            };
            // The first look is now, before anything else can happen: it catches a
            // run missed while the app was closed.
            let missed = check_schedule();
            thread::spawn(move || {
                let mut next_poll = Instant::now() + Duration::from_millis(POLL_MS);
                // Set by a scheduled run that asked for a shutdown, until the queue is empty.
                let mut shutdown_armed = missed == Some(true);
                let mut shutdown_check = shutdown_armed.then(|| Instant::now() + SHUTDOWN_CHECK);
                loop {
                    let mut queue_changed = false;
                    match incoming.recv_timeout(Duration::from_millis(100)) {
                        Ok(Notice::Event(event)) => drop(events.send(CoreEvent::Download(event))),
                        Ok(Notice::Changed) => {
                            queue_changed = true;
                            let _ = events.send(CoreEvent::QueueChanged);
                        }
                        Ok(Notice::Failure(text)) => {
                            // A failure that reads like an outdated yt-dlp updates it, once, if allowed.
                            let allowed = lock(&config).get("autoUpdateEngine").is_none_or(|v| v != &Value::Bool(false));
                            if engine_updater.on_download_failure(&text, allowed) {
                                let updater = engine_updater.clone();
                                thread::spawn(move || drop(updater.update()));
                            }
                            let _ = events.send(CoreEvent::DownloadFailed(text));
                        }
                        // Counted here, the one place a download is truly finished.
                        Ok(Notice::Completed { task, filepaths }) => lock(&stats).record(task.source, &filepaths, task.progress.total),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                    // Settings are saved a moment after the last change.
                    lock(&config).poll();

                    let now = Instant::now();
                    if now >= next_poll {
                        next_poll = now + Duration::from_millis(POLL_MS);
                        if check_schedule() == Some(true) {
                            shutdown_armed = true;
                            shutdown_check = Some(now + SHUTDOWN_CHECK);
                        }
                    }
                    let check_due = shutdown_check.is_some_and(|at| now >= at);
                    if check_due {
                        shutdown_check = None;
                    }
                    if shutdown_armed && (queue_changed || check_due) && engine.is_drained() {
                        shutdown_armed = false;
                        let _ = events.send(CoreEvent::ShutdownRequested);
                    }
                    if stopped.try_recv().is_ok() {
                        break;
                    }
                }
            })
        };
        let core = Self { paths, config, stats, ytdlp, plan, engine, events, stop, dispatcher: Some(dispatcher), song_search, hold_unconfirmed, remembered, engine_updater };
        core.sync();
        core
    }

    fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Push what depends on the settings and the plan to where it is used.
    fn sync(&self) {
        let config = lock(&self.config);
        let wanted = config.get("maxConcurrentDownloads").and_then(Value::as_f64).unwrap_or(f64::NAN);
        self.engine().set_max_slots(f64::from(clamp_concurrency(wanted, (self.plan)())));
        self.ytdlp.set_proxy(config.get("proxy").and_then(Value::as_str));
    }

    /// Call when the plan may have changed (an activation, a release), so the
    /// number of simultaneous downloads follows it without a restart.
    pub fn plan_changed(&self) {
        self.sync();
    }

    // ── Settings ────────────────────────────────────────────────────────────

    pub fn settings(&self) -> Map<String, Value> {
        lock(&self.config).get_all().clone()
    }

    /// Apply each setting that is acceptable. Returns the names of the ones
    /// refused, whose old values stay in force, so the interface can show them
    /// as invalid.
    pub fn update_settings(&self, changes: &Map<String, Value>) -> Vec<String> {
        let refused: Vec<String> = {
            let mut config = lock(&self.config);
            changes.iter().filter(|(key, value)| !config.set(key, (*value).clone())).map(|(key, _)| key.clone()).collect()
        };
        self.sync();
        let _ = self.events.send(CoreEvent::SettingsChanged);
        refused
    }

    // ── Downloads ───────────────────────────────────────────────────────────

    /// Check a download request, clamp it to the plan and queue it.
    pub fn start_download(&self, request: &Value) -> Result<DownloadTask, String> {
        let now = now_ms();
        let cookies = self.paths.user_data.join("cookies.txt");
        let cookies = cookies.exists().then(|| cookies.to_string_lossy().into_owned());
        // A Spotify track the user chose a match for downloads that match,
        // unless the request names one itself.
        let chosen = request["track"]["id"].as_str().filter(|_| request["source"] == "spotify" && request.get("matchedUrl").is_none_or(Value::is_null));
        let chosen = chosen.and_then(|id| lock(&self.remembered).get(id).map(|m| m.url.clone()));
        let with_choice = chosen.map(|url| {
            let mut request = request.clone();
            request["matchedUrl"] = Value::String(url);
            request["matchConfidence"] = json!(CONFIRMED);
            request
        });
        let request = with_choice.as_ref().unwrap_or(request);
        let task = build_task(request, lock(&self.config).get_all(), (self.plan)(), cookies.as_deref(), new_task_id(now), now)?;
        self.engine().enqueue(task.clone());
        Ok(task)
    }

    pub fn pause(&self, id: &str) -> bool {
        self.engine().pause(id)
    }

    pub fn resume(&self, id: &str) -> bool {
        self.engine().resume(id)
    }

    pub fn cancel(&self, id: &str) -> bool {
        self.engine().cancel(id)
    }

    pub fn promote(&self, id: &str) -> bool {
        self.engine().promote(id)
    }

    pub fn pause_all(&self) {
        self.engine().pause_all();
    }

    pub fn resume_all(&self) {
        self.engine().resume_all();
    }

    pub fn cancel_all(&self) {
        self.engine().cancel_all();
    }

    pub fn remove(&self, ids: Vec<String>) {
        self.engine().remove(ids);
    }

    pub fn reorder(&self, ids: Vec<String>) {
        self.engine().reorder(ids);
    }

    /// Every task in history, oldest first.
    pub fn tasks(&self) -> Vec<DownloadTask> {
        self.engine().tasks()
    }

    pub fn has_unfinished_work(&self) -> bool {
        self.engine().has_unfinished_work()
    }

    // ── Looking things up ───────────────────────────────────────────────────

    /// What a link holds. Slow: call it from a worker thread.
    pub fn fetch_metadata(&self, url: &str) -> Result<MediaMetadata, String> {
        self.ytdlp.fetch_metadata(media_url_arg(url)?)
    }

    pub fn search_youtube(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        self.ytdlp.search_youtube(query)
    }

    pub fn search_soundcloud(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        self.ytdlp.search_soundcloud(query)
    }

    /// D34, for the playlist preview: the YouTube Music song that is this
    /// Spotify track, if the song search has one that agrees. About half a
    /// second on the network: call it from a worker thread.
    pub fn match_track(&self, track: &Track) -> TrackMatch {
        // What the user chose for this track before stands, and is theirs to change.
        if let Some(chosen) = lock(&self.remembered).get(&track.id) {
            return TrackMatch::Found(SongMatch { confidence: CONFIRMED, ..chosen.clone() });
        }
        let rows = (self.song_search)(&song_query(track)).map(|response| song_rows(&response)).unwrap_or_default();
        if rows.is_empty() {
            return TrackMatch::Unreachable;
        }
        match_song(track, &rows).map_or(TrackMatch::NoSong, TrackMatch::Found)
    }

    /// D34: the user picked this song for the Spotify track `track_id`. It is
    /// used for that track from now on, by the preview and by downloads, and is
    /// kept across restarts. False when the track id is empty or the address is
    /// not YouTube's.
    // ponytail: the list is never trimmed; one short entry per corrected track.
    pub fn remember_match(&self, track_id: &str, chosen: &SongMatch) -> bool {
        if track_id.is_empty() || song_url(&chosen.url).is_none() {
            return false;
        }
        let mut remembered = lock(&self.remembered);
        remembered.insert(track_id.to_owned(), chosen.clone());
        self.save_remembered(&remembered);
        true
    }

    /// Drop the remembered choice for a track: it is matched afresh next time.
    pub fn forget_match(&self, track_id: &str) {
        let mut remembered = lock(&self.remembered);
        if remembered.remove(track_id).is_some() {
            self.save_remembered(&remembered);
        }
    }

    fn save_remembered(&self, remembered: &HashMap<String, SongMatch>) {
        if let Ok(text) = serde_json::to_string(remembered) {
            let _ = write_file_atomic(&remembered_file(&self.paths.user_data), &text);
        }
    }

    /// D34, "needs review": whether a Spotify track is downloaded only on a
    /// confirmed match. Off on a new install: a best guess downloads, marked.
    pub fn hold_unconfirmed(&self) -> bool {
        self.hold_unconfirmed.load(Ordering::SeqCst)
    }

    /// Takes effect for downloads that start from now on, and is remembered.
    pub fn set_hold_unconfirmed(&self, hold: bool) {
        self.hold_unconfirmed.store(hold, Ordering::SeqCst);
        let _ = write_file_atomic(&own_settings_file(&self.paths.user_data), &json!({ "holdUnconfirmed": hold }).to_string());
        let _ = self.events.send(CoreEvent::SettingsChanged);
    }

    // ── Keeping yt-dlp current ──────────────────────────────────────────────

    /// What is known about the installed and the latest yt-dlp.
    pub fn engine_status(&self) -> YtdlpStatus {
        self.engine_updater.status()
    }

    /// Ask whether a newer yt-dlp exists; the answer is kept for a day unless
    /// `force`. Slow: call it from a worker thread.
    pub fn check_engine(&self, force: bool) -> YtdlpStatus {
        self.engine_updater.check(force)
    }

    /// Download, verify and install the latest yt-dlp. Slow: call it from a
    /// worker thread.
    pub fn update_engine(&self) -> YtdlpStatus {
        self.engine_updater.update()
    }

    /// This month's counters and the last active month before it.
    pub fn stats(&self) -> DownloadStats {
        lock(&self.stats).get()
    }

    // ── Shutdown ────────────────────────────────────────────────────────────

    /// Stop every download, and save the history and the settings.
    pub fn shutdown(mut self) {
        self.stop_all();
    }

    fn stop_all(&mut self) {
        self.engine.shutdown();
        let _ = self.stop.send(());
        if let Some(dispatcher) = self.dispatcher.take() {
            let _ = dispatcher.join();
        }
        lock(&self.config).save_now();
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        self.stop_all();
    }
}
