//! The app core: the one object the interface talks to. It owns the settings,
//! the engine thread and the statistics, and keeps them consistent with each
//! other and with the plan the user is on.
use std::path::PathBuf;
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
use ahg_engine::types::{DownloadTask, MediaMetadata, SearchResult};
use ahg_engine::validate::media_url_arg;
use serde_json::{Map, Value, json};

use crate::download::Downloader;
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
    /// The scheduled time came and the queue was resumed. The interface shows
    /// "Scheduled downloads started." when notifications are on.
    ScheduleFired,
    /// A scheduled run that asked for it has finished: shut the PC down.
    ShutdownRequested,
}

/// After a scheduled run with nothing to resume, no queue change will ever
/// come; look once after downloads have had this long to start.
const SHUTDOWN_CHECK: Duration = Duration::from_secs(10);

/// D11: the day the schedule last fired, kept across restarts.
fn schedule_state_file(user_data: &std::path::Path) -> PathBuf {
    user_data.join("scheduler-state.json")
}

fn last_fired_date(file: &std::path::Path) -> String {
    let saved = std::fs::read_to_string(file).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
    saved.as_ref().and_then(|v| v["lastFiredDate"].as_str()).unwrap_or_default().to_owned()
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
}

impl Core {
    /// Load the settings, the history and the statistics, and start the engine.
    pub fn start(paths: Paths, ytdlp: Arc<YtDlp>, plan: PlanSource, events: Sender<CoreEvent>) -> Self {
        init_log(&paths.user_data);
        let config = Arc::new(Mutex::new(ConfigManager::new(&paths.user_data, paths.downloads.clone())));
        let mut stats = StatsStore::default();
        stats.init(&stats_file_path(&paths.user_data));
        let stats = Arc::new(Mutex::new(stats));

        let (notices, incoming) = channel();
        let engine = Arc::new(Engine::start(Downloader::with(ytdlp.clone()), &paths.user_data.join("downloads-history.json"), notices));
        let plan: Arc<dyn Fn() -> Plan + Send + Sync> = Arc::from(plan);
        let (stop, stopped) = channel();
        let dispatcher = {
            let (config, stats, events, engine, plan) = (config.clone(), stats.clone(), events.clone(), engine.clone(), plan.clone());
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
                        Ok(Notice::Failure(text)) => drop(events.send(CoreEvent::DownloadFailed(text))),
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
        let core = Self { paths, config, stats, ytdlp, plan, engine, events, stop, dispatcher: Some(dispatcher) };
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
