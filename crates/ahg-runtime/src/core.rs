//! The app core: the one object the interface talks to. It owns the settings,
//! the engine thread and the statistics, and keeps them consistent with each
//! other and with the plan the user is on.
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ahg_engine::api::{build_task, retry_request};
use ahg_engine::atomic_write::write_file_atomic;
use ahg_engine::clock::LocalTime;
use ahg_engine::config::ConfigManager;
use ahg_engine::log::init_log;
use ahg_engine::queue::{Event, new_task_id};
use ahg_engine::scheduler::{Fire, POLL_MS, ScheduleConfig, Scheduler};
use ahg_engine::spotify::{self, Spotify, Thrown};
use ahg_engine::stats::{DownloadStats, StatsStore, stats_file_path};
use ahg_engine::text::{js_len, utf16_prefix};
use ahg_engine::tier::{BASIC_LIMITS, Plan, clamp_concurrency};
use ahg_engine::updater::UpdateState;
use ahg_engine::ytdlp_updater::YtdlpStatus;
use ahg_engine::downloader::bridge::{CONFIRMED, SongMatch, match_song, song_query, song_rows, song_url};
use ahg_engine::types::{DownloadStatus, DownloadTask, MediaMetadata, Playlist, SearchResult, Track};
use ahg_engine::validate::{MAX_COOKIES_BYTES, is_netscape_cookies, media_url_arg, openable, revealable};
use serde_json::{Map, Value, json};

use crate::app_update::AppUpdater;
use crate::{lock, now_ms};
use crate::desktop::Desktop;
use crate::download::{Downloader, SongSearch};
use crate::engine_update::{EngineUpdater, Fetch, Fetched};
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
    /// The app's own updater has news, as the engine names it: `update:available`
    /// (`version`), `update:not-available`, `update:progress` (`percent`),
    /// `update:downloaded` (`version`) or `update:error` (`message`).
    Update { channel: &'static str, payload: Value },
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

/// After launch, Electron waits this long, then resolves the tools
/// ([`Core::warm_up`]); the caller waits it out, the core never sleeps.
pub const WARM_UP_DELAY: Duration = Duration::from_millis(1500);
/// ... then this long, counted from launch, before the app update check.
pub const APP_UPDATE_DELAY: Duration = Duration::from_secs(6);
/// ... and this long before [`Core::auto_update_engine`].
pub const ENGINE_UPDATE_DELAY: Duration = Duration::from_secs(8);

/// The most characters (UTF-16 units, as in JavaScript) of a search text; more is cut.
pub const MAX_QUERY: usize = 256;
/// The most ids `reorder` takes; more refuses the whole call.
pub const MAX_REORDER_IDS: usize = 500;
/// The longest id `reorder` and `promote` take; a longer one refuses the call.
pub const MAX_TASK_ID: usize = 64;
/// The longest id `pause`, `resume`, `cancel` and `remove` take; a longer one refuses the call.
pub const MAX_PATH: usize = 4096;

/// Whether every id is short enough, counting as JavaScript does.
fn short_enough(ids: &[String], max: usize) -> bool {
    ids.iter().all(|id| js_len(id) <= max)
}

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

/// `Spotify` wants one transport type; this is the one the core holds, whatever
/// is behind it.
struct Dyn(Box<dyn spotify::Transport + Send>);

impl spotify::Transport for Dyn {
    fn fetch(&mut self, req: &spotify::Request) -> Option<String> {
        self.0.fetch(req)
    }

    fn fetch_many(&mut self, reqs: &[spotify::Request]) -> Vec<Option<String>> {
        self.0.fetch_many(reqs)
    }
}

/// One cover-art request: the address and the proxy setting as it is right now.
pub type CoverFetch = Box<dyn Fn(&str, Option<&str>) -> Option<Fetched> + Send + Sync>;

/// The most bytes of a cover image `Core::fetch_cover` takes.
pub const MAX_COVER_BYTES: u64 = 5 * 1024 * 1024;

/// The most redirects `Core::fetch_cover` follows; the next one is refused.
pub const MAX_COVER_REDIRECTS: u32 = 3;

/// Where a redirect from `base` (an https address) points: `location` may be
/// absolute, start with `//` or `/`, or be relative to `base`'s folder.
fn resolve_location(base: &str, location: &str) -> String {
    let location = location.trim();
    if location.split_once("://").is_some_and(|(scheme, _)| !scheme.contains(['/', '?', '#'])) {
        return location.to_owned();
    }
    if let Some(rest) = location.strip_prefix("//") {
        return format!("https://{rest}");
    }
    let rest = &base["https://".len()..];
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    if location.starts_with('/') {
        return format!("https://{}{location}", &rest[..authority_end]);
    }
    let path = rest[..rest.find(['?', '#']).unwrap_or(rest.len())].to_owned();
    let folder = if authority_end == path.len() { format!("{path}/") } else { path[..=path.rfind('/').unwrap_or(0)].to_owned() };
    format!("https://{folder}{location}")
}

/// Everything of the core that reaches outside the app's own files and
/// processes. The real ones, or stand-ins for tests.
pub struct Outside {
    /// Cover-art images.
    pub cover: CoverFetch,
    /// YouTube Music's song search.
    pub song_search: SongSearch,
    /// Spotify's pages and API.
    pub spotify: Box<dyn spotify::Transport + Send>,
    /// GitHub's release pages, for both updaters.
    pub releases: Arc<dyn Fetch>,
    /// Showing and opening files.
    pub desktop: Box<dyn Desktop>,
}

/// The answer to "show this file" or "open this file" (`RevealResult`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    Ok,
    /// The file is no longer where the download left it.
    Missing,
    /// No path was given: the download recorded none.
    NoPath,
}

/// How importing a cookies file went (`CookiesImportResult`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookiesImport {
    Ok,
    /// The plan does not include cookies.
    Premium,
    /// No file was chosen.
    Cancelled,
    /// Not a readable cookies file. Deliberately no more detail than that.
    Invalid,
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

pub struct Core {
    paths: Paths,
    config: Arc<Mutex<ConfigManager>>,
    stats: Arc<Mutex<StatsStore>>,
    ytdlp: Arc<YtDlp>,
    plan: Arc<dyn Fn() -> Plan + Send + Sync>,
    engine: Arc<Engine>,
    events: Sender<CoreEvent>,
    stop: Sender<()>,
    /// Taken once, by the first `shutdown`.
    dispatcher: Mutex<Option<JoinHandle<()>>>,
    /// Set by the first `shutdown` (and read by the later ones and by `Drop`).
    down: AtomicBool,
    /// Held while `settings-rust.json` is read, changed and written, so that two
    /// callers never save over each other.
    own_settings: Mutex<()>,
    /// Held for the whole of `shutdown`, so a second caller waits for the first
    /// to finish (the settings saved) instead of returning early.
    finish: Mutex<()>,
    cover: CoverFetch,
    song_search: SongSearch,
    /// One reader for every Spotify lookup, so that two lookups that both need
    /// a token or hash refresh share one attempt.
    spotify: Mutex<Spotify<Dyn>>,
    hold_unconfirmed: Arc<AtomicBool>,
    remembered: Mutex<HashMap<String, SongMatch>>,
    /// The cancel flag of each lookup the interface has running (D46).
    lookups: Mutex<Vec<Arc<AtomicBool>>>,
    engine_updater: Arc<EngineUpdater>,
    app_updater: AppUpdater,
    desktop: Box<dyn Desktop>,
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
        let spotify = {
            let ytdlp = ytdlp.clone();
            Box::new(crate::http::SpotifyHttp::new(move || ytdlp.proxy()))
        };
        Self::start_with(paths, ytdlp, plan, events, Outside { song_search, spotify, releases: Arc::new(crate::http::Releases), desktop: Box::new(crate::desktop::Windows), cover: Box::new(crate::http::cover_get) })
    }

    /// `start`, with everything that reaches the network given.
    pub fn start_with(paths: Paths, ytdlp: Arc<YtDlp>, plan: PlanSource, events: Sender<CoreEvent>, outside: Outside) -> Self {
        let Outside { song_search, spotify, releases, desktop, cover } = outside;
        let spotify = Mutex::new(Spotify::new(Dyn(spotify), Box::new(spotify::system_now_ms)));
        let app_updater = {
            let tell = events.clone();
            AppUpdater::new(&paths.user_data.join("updates"), env!("CARGO_PKG_VERSION"), Box::new(releases.clone()), Box::new(move |event| drop(tell.send(CoreEvent::Update { channel: event.channel, payload: event.payload }))))
        };
        // Whatever an earlier run staged is stale now, applied or not.
        app_updater.cleanup();
        let engine_updater = {
            let (version, found, tell) = (ytdlp.clone(), ytdlp.clone(), events.clone());
            Arc::new(EngineUpdater::new(
                &paths.user_data.join("bin").to_string_lossy(),
                Box::new(releases),
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
        let core = Self { paths, config, stats, ytdlp, plan, engine, events, stop, dispatcher: Mutex::new(Some(dispatcher)), down: AtomicBool::new(false), own_settings: Mutex::new(()), finish: Mutex::new(()), cover, song_search, spotify, hold_unconfirmed, remembered, lookups: Mutex::default(), engine_updater, app_updater, desktop };
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
        if self.down.load(Ordering::SeqCst) {
            return Err("The app is closing".into());
        }
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
        js_len(id) <= MAX_PATH && self.engine().pause(id)
    }

    pub fn resume(&self, id: &str) -> bool {
        js_len(id) <= MAX_PATH && self.engine().resume(id)
    }

    pub fn cancel(&self, id: &str) -> bool {
        js_len(id) <= MAX_PATH && self.engine().cancel(id)
    }

    pub fn promote(&self, id: &str) -> bool {
        js_len(id) <= MAX_TASK_ID && self.engine().promote(id)
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

    /// An id over `MAX_PATH` characters refuses the whole call, as in Electron.
    pub fn remove(&self, ids: Vec<String>) {
        if short_enough(&ids, MAX_PATH) {
            self.engine().remove(ids);
        }
    }

    /// More than `MAX_REORDER_IDS` ids, or an id over `MAX_TASK_ID` characters,
    /// refuses the whole call, as in Electron.
    pub fn reorder(&self, ids: Vec<String>) {
        if ids.len() <= MAX_REORDER_IDS && short_enough(&ids, MAX_TASK_ID) {
            self.engine().reorder(ids);
        }
    }

    /// Every task in history, oldest first.
    pub fn tasks(&self) -> Vec<DownloadTask> {
        self.engine().tasks()
    }

    pub fn has_unfinished_work(&self) -> bool {
        self.engine().has_unfinished_work()
    }

    /// One task in history.
    pub fn task(&self, id: &str) -> Option<DownloadTask> {
        self.engine().task(id)
    }

    /// When the task's pending automatic retry starts (epoch ms), if it has one.
    pub fn retry_due(&self, id: &str) -> Option<u64> {
        self.engine().retry_due(id)
    }

    /// Download a failed, cancelled or finished task again as a new task, then
    /// drop the old row. A refused request keeps the old row.
    pub fn retry(&self, id: &str, skip_existing: Option<bool>) -> Result<DownloadTask, String> {
        let old = self.task(id).ok_or("No such task")?;
        if !matches!(old.progress.status, DownloadStatus::Failed | DownloadStatus::Cancelled | DownloadStatus::Done) {
            return Err("Only a failed, cancelled or finished download can be retried".into());
        }
        let task = self.start_download(&retry_request(&old, skip_existing))?;
        self.remove(vec![old.task_id]);
        Ok(task)
    }

    // ── Looking things up ───────────────────────────────────────────────────

    /// What a link holds. Slow: call it from a worker thread.
    pub fn fetch_metadata(&self, url: &str) -> Result<MediaMetadata, String> {
        let url = media_url_arg(url)?;
        self.lookup(|cancel| self.ytdlp.fetch_metadata_until(url, cancel))
    }

    /// The search text is cut to `MAX_QUERY` characters, not refused.
    pub fn search_youtube(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let query = utf16_prefix(query, MAX_QUERY);
        self.lookup(|cancel| self.ytdlp.search_youtube_until(query, cancel))
    }

    pub fn search_soundcloud(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let query = utf16_prefix(query, MAX_QUERY);
        self.lookup(|cancel| self.ytdlp.search_soundcloud_until(query, cancel))
    }

    /// D46: stop the lookups the interface has running right now, each ending
    /// with [`CANCELLED_MESSAGE`](crate::ytdlp::CANCELLED_MESSAGE). A download's
    /// own searches are not touched (cancelling the download stops those), and
    /// a lookup started afterwards is not affected.
    pub fn cancel_lookups(&self) {
        for flag in lock(&self.lookups).iter() {
            flag.store(true, Ordering::SeqCst);
        }
    }

    /// Runs one lookup under a flag of its own, which `cancel_lookups` can set
    /// while it runs. A flag per call, rather than one shared flag, so that
    /// cancelling never has to be undone for the next lookup.
    fn lookup<T>(&self, run: impl FnOnce(&AtomicBool) -> T) -> T {
        let flag = Arc::new(AtomicBool::new(false));
        lock(&self.lookups).push(flag.clone());
        // Pushed first, then checked: a shutdown that began meanwhile either sees the flag or is seen here.
        if self.down.load(Ordering::SeqCst) {
            flag.store(true, Ordering::SeqCst);
        }
        let result = run(&flag);
        lock(&self.lookups).retain(|f| !Arc::ptr_eq(f, &flag));
        result
    }

    /// Spotify, without an account. Each call blocks on the network, so call it
    /// from a worker thread; calls take turns, which is what lets two of them
    /// share one token or hash refresh. `Ok(None)`: not a Spotify id, or the
    /// page had nothing. `Err(Thrown)`: Spotify sent data the reader cannot
    /// take, where Electron's reader threw. Cancelling lookups does not reach
    /// these: the transport has no stop hook.
    pub fn spotify_track(&self, id_or_url: &str) -> Result<Option<Track>, Thrown> {
        lock(&self.spotify).fetch_track(id_or_url)
    }

    pub fn spotify_playlist(&self, id_or_url: &str) -> Result<Option<Playlist>, Thrown> {
        lock(&self.spotify).fetch_playlist(id_or_url)
    }

    pub fn spotify_album(&self, id_or_url: &str) -> Result<Option<Playlist>, Thrown> {
        lock(&self.spotify).fetch_album(id_or_url)
    }

    /// As in Electron, which found nothing on 2026-11-15 (QUIRKS Q6): faithful,
    /// and empty whenever Spotify says no.
    pub fn spotify_search(&self, query: &str) -> Vec<Track> {
        lock(&self.spotify).search(utf16_prefix(query, MAX_QUERY))
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
        self.save_remembered(&remembered)
    }

    /// Drop the remembered choice for a track: it is matched afresh next time.
    /// False when the file could not be saved (nothing to forget is not a failure).
    pub fn forget_match(&self, track_id: &str) -> bool {
        let mut remembered = lock(&self.remembered);
        remembered.remove(track_id).is_none() || self.save_remembered(&remembered)
    }

    fn save_remembered(&self, remembered: &HashMap<String, SongMatch>) -> bool {
        serde_json::to_string(remembered).is_ok_and(|text| write_file_atomic(&remembered_file(&self.paths.user_data), &text).is_ok())
    }

    /// D34, "needs review": whether a Spotify track is downloaded only on a
    /// confirmed match. Off on a new install: a best guess downloads, marked.
    pub fn hold_unconfirmed(&self) -> bool {
        self.hold_unconfirmed.load(Ordering::SeqCst)
    }

    /// Takes effect for downloads that start from now on, and is remembered.
    /// False when the file could not be saved: the choice holds until quit only.
    pub fn set_hold_unconfirmed(&self, hold: bool) -> bool {
        self.hold_unconfirmed.store(hold, Ordering::SeqCst);
        let saved = self.set_own_setting("holdUnconfirmed", Value::Bool(hold));
        let _ = self.events.send(CoreEvent::SettingsChanged);
        saved
    }

    /// The file of the Rust-only settings, as the object it holds (empty when it
    /// is missing or unreadable).
    fn read_own_settings(&self) -> Map<String, Value> {
        let text = std::fs::read_to_string(own_settings_file(&self.paths.user_data)).ok();
        text.and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }

    /// One choice only the Rust app has (the look, the sidebar pin ...), from
    /// `settings-rust.json`. `None` when it was never set.
    pub fn own_setting(&self, key: &str) -> Option<Value> {
        let _file = lock(&self.own_settings);
        self.read_own_settings().remove(key)
    }

    /// Remember one such choice, keeping every other key of the file, including
    /// ones this version does not know. False when the file could not be saved.
    pub fn set_own_setting(&self, key: &str, value: Value) -> bool {
        let _file = lock(&self.own_settings);
        let mut all = self.read_own_settings();
        all.insert(key.to_owned(), value);
        write_file_atomic(&own_settings_file(&self.paths.user_data), &Value::Object(all).to_string()).is_ok()
    }

    /// A cover image, from an `https` address only. Slow: call it from a worker
    /// thread. The proxy setting is read for each call (D10). At most
    /// `MAX_COVER_BYTES`; a bigger body is refused, not cut.
    /// Electron's CSP lets pages show images from any https host (`img-src
    /// https:`), so there is no host list here either.
    pub fn fetch_cover(&self, url: &str) -> Result<Vec<u8>, String> {
        // Redirects are followed here, one hop at a time, so that every address is
        // checked before it is requested.
        let mut target = url.to_owned();
        let mut hops = 0;
        let reply = loop {
            if !cover_url_allowed(&target) {
                return Err("Cover refused: not an https address of a named host".into());
            }
            let reply = (self.cover)(&target, self.ytdlp.proxy().as_deref()).ok_or("Cover fetch failed")?;
            if !matches!(reply.status, 301 | 302 | 303 | 307 | 308) {
                break reply;
            }
            hops += 1;
            if hops > MAX_COVER_REDIRECTS {
                return Err("Cover refused: too many redirects".into());
            }
            target = resolve_location(&target, reply.location.as_deref().filter(|l| !l.trim().is_empty()).ok_or("Cover fetch failed: redirect without a place")?);
        };
        if !(200..300).contains(&reply.status) || !(reply.final_url.is_empty() || reply.final_url.starts_with("https://")) {
            return Err(format!("Cover fetch failed: HTTP {}", reply.status));
        }
        // ponytail: the same read as engine_update::guarded_bytes, which also insists on GitHub's hosts
        let mut body = Vec::new();
        reply.body.take(MAX_COVER_BYTES + 1).read_to_end(&mut body).map_err(|_| "Cover fetch failed".to_owned())?;
        if body.len() as u64 > MAX_COVER_BYTES {
            return Err("Cover refused: the image is too large".into());
        }
        Ok(body)
    }

    // ── Cookies ─────────────────────────────────────────────────────────────

    /// Whether the plan includes importing cookies. The interface asks before
    /// it shows a file dialog, and offers the upgrade instead when it does not.
    pub fn may_import_cookies(&self) -> bool {
        BASIC_LIMITS.cookies || (self.plan)() == Plan::Premium
    }

    /// Copy the cookies file the user chose (`None`: they chose none) into
    /// the app's data, where downloads pick it up. It holds login cookies, so
    /// neither its content nor its path is ever logged or reported.
    pub fn import_cookies(&self, chosen: Option<&std::path::Path>) -> CookiesImport {
        if !self.may_import_cookies() {
            return CookiesImport::Premium;
        }
        let Some(file) = chosen else { return CookiesImport::Cancelled };
        let small_file = std::fs::metadata(file).is_ok_and(|m| m.is_file() && m.len() <= MAX_COOKIES_BYTES as u64);
        let text = small_file.then(|| std::fs::read_to_string(file).ok()).flatten().filter(|text| is_netscape_cookies(text));
        let Some(text) = text else { return CookiesImport::Invalid };
        let saved = std::fs::create_dir_all(&self.paths.user_data).and_then(|()| write_file_atomic(&self.paths.user_data.join("cookies.txt"), &text));
        if saved.is_ok() { CookiesImport::Ok } else { CookiesImport::Invalid }
    }

    // ── Finished files ──────────────────────────────────────────────────────

    /// Show a downloaded file in its folder. `Err` when the path is not
    /// inside the download folder.
    pub fn show_in_folder(&self, path: &str) -> Result<Shown, String> {
        let Some(file) = revealable(path, &self.output_dir())? else { return Ok(Shown::NoPath) };
        if !std::path::Path::new(&file).exists() {
            return Ok(Shown::Missing);
        }
        self.desktop.reveal(&file);
        Ok(Shown::Ok)
    }

    /// Open a downloaded file in its default program. `Err` when the path is
    /// not a media file inside the download folder.
    pub fn open_file(&self, path: &str) -> Result<Shown, String> {
        let Some(file) = openable(path, &self.output_dir())? else { return Ok(Shown::NoPath) };
        if !std::path::Path::new(&file).exists() {
            return Ok(Shown::Missing);
        }
        Ok(if self.desktop.open(&file) { Shown::Ok } else { Shown::Missing })
    }

    fn output_dir(&self) -> String {
        lock(&self.config).get("outputDir").and_then(Value::as_str).unwrap_or_default().to_owned()
    }

    // ── The app's own updates ───────────────────────────────────────────────

    /// Where the app's own update stands.
    pub fn update_status(&self) -> UpdateState {
        self.app_updater.status()
    }

    /// Ask whether a newer version of the app exists. Slow: call it from a
    /// worker thread.
    pub fn check_update(&self) -> UpdateState {
        self.app_updater.check()
    }

    /// Download the available version and verify it; progress arrives as
    /// events. `Err` when there is nothing to download. Slow: call it from a
    /// worker thread.
    pub fn download_update(&self) -> Result<UpdateState, String> {
        self.app_updater.download()
    }

    /// Put the downloaded version in place of `target_exe`, the running app
    /// (`std::env::current_exe()` in the real app). The caller then shuts the
    /// core down, starts `target_exe` again and exits; at its next start it
    /// calls `app_update::remove_spare`. The core never ends the process.
    pub fn install_update(&self, target_exe: &std::path::Path) -> Result<(), String> {
        self.app_updater.install(target_exe)
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

    /// What the startup does for yt-dlp: nothing unless the `autoUpdateEngine`
    /// setting is on, else check, and install when a newer one is found. Slow:
    /// call it from a worker thread, `ENGINE_UPDATE_DELAY` after launch.
    pub fn auto_update_engine(&self) {
        if lock(&self.config).get("autoUpdateEngine").and_then(Value::as_bool) == Some(true) && self.check_engine(false).state == "available" {
            self.update_engine();
        }
    }

    /// Resolve yt-dlp and ffmpeg now, so a first search does not pay for it.
    /// Call it `WARM_UP_DELAY` after launch, from a worker thread.
    pub fn warm_up(&self) {
        self.ytdlp.bin();
        self.ytdlp.ffmpeg();
    }

    /// This month's counters and the last active month before it.
    pub fn stats(&self) -> DownloadStats {
        lock(&self.stats).get()
    }

    // ── Shutdown ────────────────────────────────────────────────────────────

    /// Stop the lookups and every download, and save the history and the
    /// settings. Callable through a shared handle (`Arc<Core>`) and any number
    /// of times: the first call does the work, the others wait until it is
    /// done and then return. Make no calls after it: settings changes are no
    /// longer saved and `start_download` is refused.
    pub fn shutdown(&self) {
        let _finish = lock(&self.finish);
        if self.down.swap(true, Ordering::SeqCst) {
            return;
        }
        // A lookup holds a handle of its own now, so it would outlive the quit.
        self.cancel_lookups();
        self.engine.shutdown();
        let _ = self.stop.send(());
        let dispatcher = lock(&self.dispatcher).take();
        if let Some(dispatcher) = dispatcher {
            let _ = dispatcher.join();
        }
        lock(&self.config).save_now();
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// The interface holds the core behind an `Arc` on several threads.
const _: fn() = || {
    fn is<T: Send + Sync>() {}
    is::<Core>();
};

/// Whether `url` may be fetched as cover art: https, with a real host name. No
/// IP literal (IPv4 in any spelling, or IPv6 in brackets), no `localhost` or
/// `*.localhost`, no user name in the address. Pure: nothing is resolved.
// ponytail: a name that resolves to a private address (DNS rebinding) still passes;
// upgrade path: resolve, pin the address for the request, refuse private ranges.
pub fn cover_url_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let authority = rest.split(['/', '?', '#', '\\']).next().unwrap_or_default();
    if authority.contains('@') || authority.starts_with('[') {
        return false;
    }
    let host = authority.split(':').next().unwrap_or_default().trim_end_matches('.').to_ascii_lowercase();
    let last = host.rsplit('.').next().unwrap_or_default();
    let numeric = !last.is_empty() && (last.bytes().all(|b| b.is_ascii_digit()) || last.starts_with("0x"));
    !host.is_empty() && host != "localhost" && !host.ends_with(".localhost") && !numeric
}

#[cfg(test)]
mod cover_host_tests {
    use super::cover_url_allowed;

    #[test]
    fn hosts_are_accepted_or_refused_without_resolving_anything() {
        let accepted = ["https://i.scdn.co/image/a", "https://CDN.Example.com:8443/a.jpg?x=1", "https://i.ytimg.com", "https://a.b.c.example/#f", "https://example.com./a"];
        let refused = [
            "http://i.example/a", "file:///c:/a", "", "i.example/a", "https:///a", "https://",
            "https://127.0.0.1/a", "https://10.0.0.5:8080/a", "https://2130706433/a", "https://0x7f.1/a", "https://[::1]/a", "https://[2001:db8::1]:443/a",
            "https://localhost/a", "https://LOCALHOST:80/a", "https://app.localhost/a", "https://localhost./a",
            "https://user@i.example/a", "https://user:pw@i.example/a", "https://i.example@evil.example/a", "https://@i.example/a", "https://127.0.0.1\\.example.com/a",
        ];
        assert!(!accepted.is_empty() && !refused.is_empty());
        for url in accepted {
            assert!(cover_url_allowed(url), "should be accepted: {url}");
        }
        for url in refused {
            assert!(!cover_url_allowed(url), "should be refused: {url}");
        }
    }
}
