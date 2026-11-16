//! The download queue: up to N jobs at once, oldest first, with pause, resume,
//! cancel, reorder, retry with backoff, and a saved history.
//!
//! It does no waiting and starts no process itself. Every call takes the
//! current time and returns the `Effect`s the caller must carry out: messages
//! for the interface, downloads to start or stop. Timers are kept here as
//! deadlines; the caller asks for `next_deadline` and calls `advance` when it
//! passes. The downloader reports back through `on_progress`, `on_done` and
//! `on_error`.
//!
//! Tasks are held by identity, not by id. A retry reuses the task id with a
//! fresh task object while callbacks from the killed process still refer to
//! the old one, so a callback is accepted only if the object it belongs to is
//! the one currently holding the slot.
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::atomic_write::write_file_atomic;
use crate::downloader::errors::{ErrorClass, classify_error, error_code_of, explain_error, needs_client_fallback};
use crate::log::{host_of, scrub_urls, tail};
use crate::state_machine::can_transition;
use crate::types::{
    DownloadProgress, DownloadStatus, DownloadTask, PartialExtras, RETRY_BACKOFF_MS, SourcePlatform, Track, VideoQuality,
};

const RECENT_CAP: usize = 8;
const PROGRESS_INTERVAL_MS: u64 = 250;
const MAX_RETRIES: f64 = RETRY_BACKOFF_MS.len() as f64;
/// A brief deliberate pause between one job ending and the next starting.
const NEXT_PAUSE_MS: u64 = 500;
/// How long a killed process's error is expected and suppressed.
const CANCEL_GRACE_MS: u64 = 3000;
const HISTORY_CAP: usize = 200;
const PERSIST_MS: u64 = 500;

fn is_terminal(status: DownloadStatus) -> bool {
    matches!(status, DownloadStatus::Done | DownloadStatus::Failed | DownloadStatus::Cancelled)
}

/// What a new task may be created from. Anything missing takes its default.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskInit {
    pub url: Option<String>,
    pub source: Option<SourcePlatform>,
    pub output_dir: Option<String>,
    pub format: Option<String>,
    pub quality: Option<String>,
    pub video_quality: Option<VideoQuality>,
    pub is_audio_only: Option<bool>,
    pub is_playlist: Option<bool>,
    pub playlist_name: Option<String>,
    pub embed_thumbnail: Option<bool>,
    pub embed_metadata: Option<bool>,
    pub skip_existing: Option<bool>,
    pub title: Option<String>,
    pub thumbnail_url: Option<String>,
    pub duration: Option<f64>,
    pub uploader: Option<String>,
    pub created_at: Option<f64>,
    pub track: Option<Track>,
    pub retry_count: Option<f64>,
    pub original_task_id: Option<String>,
    pub playlist_limit: Option<f64>,
    pub playlist_items: Option<String>,
    pub fallback_profile: Option<bool>,
    pub extras: Option<PartialExtras>,
}

/// A queued task with sensible defaults. `id` must be unique.
pub fn create_task(init: &TaskInit, id: String, now: u64) -> DownloadTask {
    let i = init.clone();
    DownloadTask {
        task_id: id,
        url: i.url.unwrap_or_default(),
        source: i.source.unwrap_or(SourcePlatform::Unknown),
        output_dir: i.output_dir.unwrap_or_default(),
        format: i.format.unwrap_or_else(|| "mp3".to_owned()),
        quality: i.quality.unwrap_or_else(|| "320".to_owned()),
        video_quality: i.video_quality.unwrap_or(VideoQuality::Best),
        is_audio_only: i.is_audio_only.unwrap_or(true),
        is_playlist: i.is_playlist.unwrap_or(false),
        playlist_name: i.playlist_name.unwrap_or_default(),
        playlist_limit: i.playlist_limit,
        playlist_items: i.playlist_items,
        error_code: None,
        embed_thumbnail: i.embed_thumbnail.unwrap_or(true),
        embed_metadata: i.embed_metadata.unwrap_or(true),
        skip_existing: i.skip_existing.unwrap_or(true),
        title: i.title.unwrap_or_default(),
        thumbnail_url: i.thumbnail_url.unwrap_or_default(),
        duration: i.duration.unwrap_or(0.0),
        uploader: i.uploader.unwrap_or_default(),
        progress: DownloadProgress::default(),
        retry_count: i.retry_count,
        original_task_id: i.original_task_id,
        interrupted: None,
        // Stamped at creation so it survives into the saved history.
        created_at: Some(i.created_at.unwrap_or(now as f64)),
        completed_at: None,
        track: i.track,
        match_confidence: None,
        matched_url: None,
        fallback_profile: i.fallback_profile,
        extras: i.extras,
        other: Map::new(),
    }
}

/// A new id in the shape the Electron app uses: `task_<ms>_<9 base-36 characters>`.
pub fn new_task_id(now: u64) -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    // RandomState is seeded from the OS; hashing the time gives an unpredictable value.
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(now);
    let mut n = hasher.finish();
    let mut suffix = String::with_capacity(9);
    for _ in 0..9 {
        suffix.push(char::from_digit((n % 36) as u32, 36).unwrap_or('0'));
        n /= 36;
    }
    format!("task_{now}_{suffix}")
}

/// A message for the interface.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Queued { id: String, task: Box<DownloadTask> },
    Progress { id: String, progress: DownloadProgress },
    Done { id: String, progress: DownloadProgress },
    Error { id: String, message: String },
    Cancelled { id: String },
}

/// Something the caller must do after a call into the queue, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Deliver to the interface.
    Send(Event),
    /// The queue's activity changed: tray, presence and the shutdown watcher re-read it.
    Changed,
    /// The raw error text of a failed download, before any retry decision.
    Failure(String),
    /// Start this download. Report back with the same `attempt`.
    Download { task: DownloadTask, attempt: u64 },
    /// Stop this task's process.
    Cancel(String),
    /// Stop every process.
    CancelAll,
    /// A download finished and wrote these files. Counted by the statistics.
    Completed { task: DownloadTask, filepaths: Vec<String> },
    Log { level: &'static str, message: String },
}

/// Identity of one task object.
type Key = u64;

#[derive(Debug, Clone, PartialEq)]
enum TimerKind {
    Pump,
    Persist,
    Tick { id: String, key: Key },
    Retry { id: String, key: Key },
    Grace { id: String },
}

#[derive(Debug, Clone)]
struct Timer {
    due: u64,
    seq: u64,
    kind: TimerKind,
}

pub struct Queue {
    tasks: HashMap<Key, DownloadTask>,
    next_key: Key,
    /// Waiting tasks, front first.
    queue: Vec<Key>,
    /// Tasks holding a slot, in the order they started.
    active: IndexMap<String, Key>,
    max_slots: f64,
    paused: bool,
    /// Recently seen tasks, most recent first.
    recent: Vec<Key>,
    /// Every task this install has seen, oldest first, softly capped.
    history: IndexMap<String, Key>,
    history_file: Option<PathBuf>,
    timers: Vec<Timer>,
    next_seq: u64,
    persist_timer: Option<u64>,
    tick_at: HashMap<String, u64>,
    tick_timers: HashMap<String, u64>,
    retry_timers: IndexMap<String, u64>,
    recently_cancelled: HashMap<String, u64>,
    /// Which task object each started download belongs to.
    attempts: HashMap<u64, Key>,
    /// The attempt each task object is currently running under.
    current: HashMap<Key, u64>,
    next_attempt: u64,
    now: u64,
    out: Vec<Effect>,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            next_key: 0,
            queue: Vec::new(),
            active: IndexMap::new(),
            max_slots: 1.0,
            paused: false,
            recent: Vec::new(),
            history: IndexMap::new(),
            history_file: None,
            timers: Vec::new(),
            next_seq: 0,
            persist_timer: None,
            tick_at: HashMap::new(),
            tick_timers: HashMap::new(),
            retry_timers: IndexMap::new(),
            recently_cancelled: HashMap::new(),
            attempts: HashMap::new(),
            current: HashMap::new(),
            next_attempt: 0,
            now: 0,
            out: Vec::new(),
        }
    }

    // ── Entry and exit of every public call ─────────────────────────────────

    fn begin(&mut self, now: u64) {
        self.now = now;
    }

    fn finish(&mut self) -> Vec<Effect> {
        self.collect_garbage();
        std::mem::take(&mut self.out)
    }

    /// Forget task objects nothing refers to any more.
    fn collect_garbage(&mut self) {
        if self.tasks.len() <= self.queue.len() + self.active.len() + self.history.len() + self.recent.len() {
            return;
        }
        let mut live: Vec<Key> = self.queue.clone();
        live.extend(self.active.values());
        live.extend(self.history.values());
        live.extend(&self.recent);
        live.extend(self.timers.iter().filter_map(|t| match &t.kind {
            TimerKind::Tick { key, .. } | TimerKind::Retry { key, .. } => Some(*key),
            _ => None,
        }));
        self.tasks.retain(|k, _| live.contains(k));
        let tasks = &self.tasks;
        self.attempts.retain(|_, k| tasks.contains_key(k));
        self.current.retain(|k, _| tasks.contains_key(k));
    }

    fn insert(&mut self, task: DownloadTask) -> Key {
        self.next_key += 1;
        self.tasks.insert(self.next_key, task);
        self.next_key
    }

    fn task(&self, key: Key) -> &DownloadTask {
        &self.tasks[&key]
    }

    fn task_mut(&mut self, key: Key) -> &mut DownloadTask {
        self.tasks.get_mut(&key).expect("a referenced task object exists")
    }

    fn status(&self, key: Key) -> DownloadStatus {
        self.task(key).progress.status
    }

    fn id(&self, key: Key) -> String {
        self.task(key).task_id.clone()
    }

    // ── Timers ──────────────────────────────────────────────────────────────

    fn later(&mut self, kind: TimerKind, ms: u64) -> u64 {
        self.next_seq += 1;
        self.timers.push(Timer { due: self.now + ms, seq: self.next_seq, kind });
        self.next_seq
    }

    fn clear_timer(&mut self, seq: u64) {
        self.timers.retain(|t| t.seq != seq);
    }

    /// When the earliest pending timer falls due.
    pub fn next_deadline(&self) -> Option<u64> {
        self.timers.iter().map(|t| t.due).min()
    }

    /// When a task's pending automatic retry starts, or None when it has none.
    pub fn retry_due(&self, id: &str) -> Option<u64> {
        let seq = self.retry_timers.get(id)?;
        self.timers.iter().find(|t| t.seq == *seq).map(|t| t.due)
    }

    /// Milliseconds until each pending timer, soonest first.
    pub fn pending_timers(&self, now: u64) -> Vec<u64> {
        let mut waits: Vec<u64> = self.timers.iter().map(|t| t.due.saturating_sub(now)).collect();
        waits.sort_unstable();
        waits
    }

    /// Fire every timer due by `now`, earliest first.
    pub fn advance(&mut self, now: u64) -> Vec<Effect> {
        loop {
            let due = self.timers.iter().filter(|t| t.due <= now).min_by_key(|t| (t.due, t.seq)).cloned();
            let Some(timer) = due else { break };
            self.clear_timer(timer.seq);
            self.now = self.now.max(timer.due);
            match timer.kind {
                TimerKind::Pump => self.process_next(),
                TimerKind::Persist => {
                    self.persist_timer = None;
                    self.flush_history();
                }
                TimerKind::Tick { id, key } => {
                    self.tick_timers.remove(&id);
                    if self.active.get(&id) == Some(&key) {
                        self.send_tick(key);
                    }
                }
                TimerKind::Retry { id, key } => {
                    self.retry_timers.shift_remove(&id);
                    // The front of the queue, not a reserved slot: if every slot
                    // is busy the retry waits there, never pushing past the limit.
                    self.queue.insert(0, key);
                    let task = self.task(key).clone();
                    self.emit(Event::Queued { id, task: Box::new(task) });
                    self.process_next();
                }
                TimerKind::Grace { id } => {
                    self.recently_cancelled.remove(&id);
                }
            }
        }
        self.now = now;
        self.finish()
    }

    // ── History ─────────────────────────────────────────────────────────────

    /// Load saved history and park interrupted (unfinished) tasks as paused,
    /// back in the queue. yt-dlp continues their partial files on resume.
    pub fn init_history(&mut self, file: &Path, now: u64) {
        self.begin(now);
        self.history_file = Some(file.to_owned());
        let Some(Value::Array(saved)) = fs::read(file).ok().and_then(|b| serde_json::from_slice(&b).ok()) else { return };
        // Defaults are laid first and the saved values win on top, so a row
        // written by an older build still has every field the interface reads.
        let Ok(Value::Object(base)) = serde_json::to_value(create_task(&TaskInit::default(), String::new(), now)) else {
            return;
        };
        for raw in saved {
            let Value::Object(raw) = raw else { continue };
            if raw.get("taskId").and_then(Value::as_str).is_none_or(str::is_empty) {
                continue;
            }
            let Some(Value::Object(raw_progress)) = raw.get("progress").cloned() else { continue };
            let mut row = base.clone();
            let mut progress = match row.get("progress") {
                Some(Value::Object(p)) => p.clone(),
                _ => Map::new(),
            };
            progress.extend(raw_progress);
            row.extend(raw);
            let terminal = matches!(progress.get("status").and_then(Value::as_str), Some("done" | "failed" | "cancelled"));
            if !terminal {
                progress.insert("status".into(), "paused".into());
                progress.insert("speed".into(), 0.into());
                progress.insert("eta".into(), 0.into());
                progress.insert("error".into(), "".into());
                row.insert("interrupted".into(), true.into()); // the interface asks: finish it or erase it
            }
            row.insert("progress".into(), Value::Object(progress));
            // A row whose fields have the wrong types cannot be represented and is left out.
            let Ok(task) = serde_json::from_value::<DownloadTask>(Value::Object(row)) else { continue };
            let id = task.task_id.clone();
            let key = self.insert(task);
            if !terminal {
                self.queue.push(key);
            }
            self.history.insert(id, key);
        }
    }

    /// Every task in history, oldest first.
    pub fn all_tasks(&self) -> Vec<&DownloadTask> {
        self.history.values().map(|k| self.task(*k)).collect()
    }

    /// Drop tasks from history and from the waiting queue. Running jobs are skipped.
    pub fn remove_tasks(&mut self, ids: &[String], now: u64) -> Vec<Effect> {
        self.begin(now);
        for id in ids {
            if self.active.contains_key(id) {
                continue;
            }
            self.history.shift_remove(id);
            if let Some(idx) = self.queue.iter().position(|k| self.tasks[k].task_id == *id) {
                self.queue.remove(idx);
            }
            // An uncleared retry timer would re-queue a task whose history row was just deleted.
            if let Some(seq) = self.retry_timers.shift_remove(id) {
                self.clear_timer(seq);
            }
        }
        // Removing the last parked row by hand must not leave the queue paused for good.
        self.clear_pause_if_nothing_parked();
        self.schedule_persist();
        self.finish()
    }

    /// One immediate save, for shutdown.
    pub fn flush_history(&mut self) {
        let Some(file) = self.history_file.clone() else { return };
        if let Some(seq) = self.persist_timer.take() {
            self.clear_timer(seq);
        }
        if let Ok(text) = serde_json::to_string(&self.all_tasks()) {
            let _ = write_file_atomic(&file, &text);
        }
    }

    fn schedule_persist(&mut self) {
        if self.history_file.is_none() || self.persist_timer.is_some() {
            return;
        }
        self.persist_timer = Some(self.later(TimerKind::Persist, PERSIST_MS));
    }

    fn record_history(&mut self, key: Key) {
        self.history.insert(self.id(key), key);
        if self.history.len() <= HISTORY_CAP {
            return;
        }
        // Oldest first, finished rows only: finished work is the cheapest to forget.
        // ponytail: a soft cap. If nothing finished is left, every row is work in
        // flight, and an oversized history is the better failure than dropping it.
        let finished: Vec<String> =
            self.history.iter().filter(|(_, k)| is_terminal(self.status(**k))).map(|(id, _)| id.clone()).collect();
        for id in finished {
            if self.history.len() <= HISTORY_CAP {
                break;
            }
            self.history.shift_remove(&id);
        }
    }

    // ── Slots and enqueueing ────────────────────────────────────────────────

    /// Set how many downloads may run at once. The caller owns the tier clamp.
    /// Raising it fills the new slots at once; lowering it never kills a
    /// running job, it only blocks new starts.
    pub fn set_max_slots(&mut self, n: f64, now: u64) -> Vec<Effect> {
        self.begin(now);
        self.max_slots = n.floor().max(1.0);
        self.process_next();
        self.finish()
    }

    pub fn enqueue(&mut self, task: DownloadTask, now: u64) -> Vec<Effect> {
        self.begin(now);
        let key = self.insert(task);
        // Enqueued while everything is paused: park it, since "queued" would
        // promise it is about to start.
        if self.paused {
            self.transition(key, DownloadStatus::Paused);
        }
        self.queue.push(key);
        self.record_recent(key);
        self.record_history(key);
        let task = self.task(key).clone();
        self.emit(Event::Queued { id: task.task_id.clone(), task: Box::new(task) });
        self.schedule_persist();
        self.process_next();
        self.finish()
    }

    // ── Pause and resume, one task ──────────────────────────────────────────

    /// Pause one task. A running job is stopped (its partial file stays for
    /// later), parked at the front of the queue and its slot freed. A waiting
    /// job is paused in place.
    pub fn pause_task(&mut self, id: &str, now: u64) -> (bool, Vec<Effect>) {
        self.begin(now);
        let ok = if let Some(&key) = self.active.get(id) {
            self.out.push(Effect::Cancel(id.to_owned()));
            self.mark_recently_cancelled(id);
            self.transition(key, DownloadStatus::Paused);
            let progress = &mut self.task_mut(key).progress;
            progress.speed = 0.0;
            progress.eta = 0.0;
            self.queue.insert(0, key);
            self.active.shift_remove(id);
            self.emit_progress(key);
            self.later(TimerKind::Pump, NEXT_PAUSE_MS);
            true
        } else if let Some(key) = self.find_waiting(id)
            && self.transition(key, DownloadStatus::Paused)
        {
            self.emit_progress(key);
            true
        } else {
            false
        };
        (ok, self.finish())
    }

    /// Resume a paused task: back to queued, then let the queue pick it up.
    pub fn resume_task(&mut self, id: &str, now: u64) -> (bool, Vec<Effect>) {
        self.begin(now);
        let ok = match self.find_waiting(id) {
            Some(key) if self.status(key) == DownloadStatus::Paused && self.transition(key, DownloadStatus::Queued) => {
                self.task_mut(key).interrupted = Some(false); // the user chose to finish it
                self.emit_progress(key);
                self.process_next();
                true
            }
            _ => false,
        };
        (ok, self.finish())
    }

    fn find_waiting(&self, id: &str) -> Option<Key> {
        self.queue.iter().copied().find(|k| self.tasks[k].task_id == id)
    }

    // ── Reordering waiting work ─────────────────────────────────────────────

    /// Put the queued tasks into the order `ids` gives. Only waiting work can
    /// move. Paused tasks keep the exact places they sit in. Ids that are
    /// unknown, repeated or no longer queued are skipped, and queued tasks not
    /// named keep their order behind the named ones.
    pub fn reorder(&mut self, ids: &[String], now: u64) -> Vec<Effect> {
        self.begin(now);
        self.reorder_queued(ids);
        self.finish()
    }

    fn reorder_queued(&mut self, ids: &[String]) {
        let slots: Vec<usize> =
            (0..self.queue.len()).filter(|&i| self.status(self.queue[i]) == DownloadStatus::Queued).collect();
        if slots.len() < 2 {
            return;
        }
        let pending: Vec<Key> = slots.iter().map(|&i| self.queue[i]).collect();
        let mut named: Vec<Key> = Vec::new();
        for id in ids {
            if let Some(&key) = pending.iter().find(|k| self.tasks[k].task_id == *id)
                && !named.contains(&key)
            {
                named.push(key);
            }
        }
        let rest: Vec<Key> = pending.iter().copied().filter(|k| !named.contains(k)).collect();
        for (slot, key) in slots.into_iter().zip(named.into_iter().chain(rest)) {
            self.queue[slot] = key;
        }
    }

    /// "Do this next": move one queued task to the front of the waiting ones.
    /// False when it is not waiting any more.
    pub fn promote_task(&mut self, id: &str, now: u64) -> (bool, Vec<Effect>) {
        self.begin(now);
        let ok = self.find_waiting(id).is_some_and(|k| self.status(k) == DownloadStatus::Queued);
        if ok {
            self.reorder_queued(&[id.to_owned()]);
        }
        (ok, self.finish())
    }

    // ── What the tray, presence and the shutdown watcher read ───────────────

    /// Most recent first: each task's title (or its link) and current status.
    pub fn recent_downloads(&self) -> Vec<(String, DownloadStatus)> {
        self.recent
            .iter()
            .map(|k| {
                let t = self.task(*k);
                (if t.title.is_empty() { t.url.clone() } else { t.title.clone() }, t.progress.status)
            })
            .collect()
    }

    /// True while anything is running or still queued, paused tasks included,
    /// so the PC is never shut down with paused downloads waiting.
    pub fn has_unfinished_work(&self) -> bool {
        !self.active.is_empty() || !self.queue.is_empty()
    }

    // ── Pause and resume, everything ────────────────────────────────────────

    /// Stop every running job, park them at the front in the order they
    /// started, and block new starts.
    pub fn pause_all(&mut self, now: u64) -> Vec<Effect> {
        self.begin(now);
        if !self.paused {
            self.paused = true;
            let parked: Vec<(String, Key)> = self.active.iter().map(|(id, k)| (id.clone(), *k)).collect();
            for (id, key) in &parked {
                self.out.push(Effect::Cancel(id.clone()));
                self.mark_recently_cancelled(id);
                self.transition(*key, DownloadStatus::Paused);
                self.emit_progress(*key);
            }
            self.queue.splice(0..0, parked.iter().map(|(_, k)| *k));
            self.active.clear();
        }
        self.finish()
    }

    /// The global pause exists to hold parked work. Once none remains it must
    /// clear, or every newly added link would sit at "queued" until a restart.
    fn clear_pause_if_nothing_parked(&mut self) {
        if self.paused && !self.queue.iter().any(|k| self.status(*k) == DownloadStatus::Paused) {
            self.paused = false;
        }
    }

    /// Flip paused jobs back to queued and start processing again.
    pub fn resume_all(&mut self, now: u64) -> Vec<Effect> {
        self.begin(now);
        if self.paused {
            self.paused = false;
            for key in self.queue.clone() {
                if self.status(key) == DownloadStatus::Paused {
                    self.transition(key, DownloadStatus::Queued);
                    self.emit_progress(key);
                }
            }
            self.process_next();
        }
        self.finish()
    }

    // ── Cancelling ──────────────────────────────────────────────────────────

    pub fn cancel_task(&mut self, id: &str, now: u64) -> (bool, Vec<Effect>) {
        self.begin(now);
        let ok = if let Some(seq) = self.retry_timers.shift_remove(id) {
            // A retry that has not started yet: clear its timer, and end the
            // fresh task that replaced the history row so it cannot come back.
            self.clear_timer(seq);
            if let Some(&retried) = self.history.get(id) {
                self.transition(retried, DownloadStatus::Cancelled);
            }
            self.emit(Event::Cancelled { id: id.to_owned() });
            true
        } else if let Some(&running) = self.active.get(id) {
            // A running job: stop it and expect its dying error for a while.
            self.out.push(Effect::Cancel(id.to_owned()));
            self.mark_recently_cancelled(id);
            self.transition(running, DownloadStatus::Cancelled);
            self.emit(Event::Cancelled { id: id.to_owned() });
            self.finish_task(id);
            true
        } else if let Some(idx) = self.queue.iter().position(|k| self.tasks[k].task_id == id) {
            let key = self.queue.remove(idx);
            self.transition(key, DownloadStatus::Cancelled);
            self.emit(Event::Cancelled { id: id.to_owned() });
            true
        } else {
            false
        };
        (ok, self.finish())
    }

    pub fn cancel_all(&mut self, now: u64) -> Vec<Effect> {
        self.begin(now);
        // Tasks waiting out a retry delay must end too, or they would be saved
        // as "queued" and come back at the next launch.
        for (id, seq) in std::mem::take(&mut self.retry_timers) {
            self.clear_timer(seq);
            if let Some(&retried) = self.history.get(&id) {
                self.transition(retried, DownloadStatus::Cancelled);
            }
            self.emit(Event::Cancelled { id });
        }
        for key in std::mem::take(&mut self.queue) {
            self.transition(key, DownloadStatus::Cancelled);
            self.emit(Event::Cancelled { id: self.id(key) });
        }
        self.clear_pause_if_nothing_parked();
        // Mark every running id before the kill, so none of the dying
        // processes paints an error row after the user cancelled.
        for (id, key) in std::mem::take(&mut self.active) {
            self.mark_recently_cancelled(&id);
            self.transition(key, DownloadStatus::Cancelled);
            self.emit(Event::Cancelled { id });
        }
        self.out.push(Effect::CancelAll);
        self.schedule_persist();
        self.finish()
    }

    // ── Processing: up to `max_slots` at a time, oldest first ───────────────

    fn process_next(&mut self) {
        // Occupancy is re-read on every pass and never counted in advance: two
        // jobs finishing close together queue two pumps, and both must see the truth.
        while !self.paused && (self.active.len() as f64) < self.max_slots {
            // Individually paused tasks stay parked; take the first live one.
            let Some(idx) = self.queue.iter().position(|k| self.status(*k) != DownloadStatus::Paused) else { return };
            let key = self.queue.remove(idx);
            let id = self.id(key);
            self.active.insert(id, key);
            self.transition(key, DownloadStatus::FetchingInfo);
            self.emit_progress(key);
            // Each attempt starts with no error code; a failure stamps a new one.
            self.task_mut(key).error_code = None;
            self.next_attempt += 1;
            self.attempts.insert(self.next_attempt, key);
            self.current.insert(key, self.next_attempt);
            self.out.push(Effect::Download { task: self.task(key).clone(), attempt: self.next_attempt });
        }
    }

    /// The task object a callback belongs to, if that object is the one
    /// holding the slot and `attempt` is its current attempt. A late callback
    /// from a killed or finished process is refused. Pause then resume restarts
    /// the same object, so the object alone would not tell the two attempts apart.
    fn admitted(&self, id: &str, attempt: u64) -> Option<Key> {
        let key = *self.attempts.get(&attempt)?;
        (self.active.get(id) == Some(&key) && self.current.get(&key) == Some(&attempt)).then_some(key)
    }

    /// The Spotify bridge picked the video to download for this attempt.
    /// `confidence` is between 0 and 1.
    pub fn on_match(&mut self, id: &str, attempt: u64, matched_url: &str, confidence: f64) {
        if let Some(key) = self.admitted(id, attempt) {
            let task = self.task_mut(key);
            task.matched_url = Some(matched_url.to_owned());
            task.match_confidence = Some(confidence);
        }
    }

    pub fn on_progress(&mut self, id: &str, attempt: u64, p: DownloadProgress, now: u64) -> Vec<Effect> {
        self.begin(now);
        if let Some(key) = self.admitted(id, attempt) {
            // An impossible status change keeps the current status but still
            // takes the new numbers.
            let before = self.status(key);
            let status = if self.transition(key, p.status) { p.status } else { before };
            let percent = p.percent;
            self.task_mut(key).progress = DownloadProgress { status, ..p };
            if status != before || status != DownloadStatus::Downloading || percent >= 100.0 {
                // A status change or the final tick goes out at once.
                self.clear_tick(id);
                self.emit_progress(key);
            } else {
                // A plain tick: about four a second per task, with a trailing
                // send so the last tick of a burst is never lost.
                let since = now.saturating_sub(self.tick_at.get(id).copied().unwrap_or(0));
                if since >= PROGRESS_INTERVAL_MS {
                    self.send_tick(key);
                } else if !self.tick_timers.contains_key(id) {
                    let seq = self.later(TimerKind::Tick { id: id.to_owned(), key }, PROGRESS_INTERVAL_MS - since);
                    self.tick_timers.insert(id.to_owned(), seq);
                }
            }
        }
        self.finish()
    }

    /// A progress message for the interface only; nothing else is woken.
    fn send_tick(&mut self, key: Key) {
        let task = self.task(key);
        let (id, progress) = (task.task_id.clone(), task.progress.clone());
        self.tick_at.insert(id.clone(), self.now);
        self.out.push(Effect::Send(Event::Progress { id, progress }));
    }

    fn clear_tick(&mut self, id: &str) {
        if let Some(seq) = self.tick_timers.remove(id) {
            self.clear_timer(seq);
        }
        self.tick_at.insert(id.to_owned(), self.now);
    }

    pub fn on_done(
        &mut self,
        id: &str,
        attempt: u64,
        filepath: &str,
        filepaths: &[String],
        already_on_disk: bool,
        now: u64,
    ) -> Vec<Effect> {
        self.begin(now);
        if let Some(key) = self.admitted(id, attempt) {
            // A clean exit is authoritative: the task is done whatever status was last seen.
            let task = self.task_mut(key);
            task.progress.status = DownloadStatus::Done;
            task.progress.percent = 100.0;
            if !filepath.is_empty() {
                task.progress.filename = filepath.to_owned();
            }
            if already_on_disk {
                task.progress.already_on_disk = Some(true);
            }
            // The final size from disk, since an instant finish reports no progress.
            // Only a real absolute path is measured; a bare title is not a file.
            if is_absolute_win32(&task.progress.filename)
                && let Ok(meta) = fs::metadata(&task.progress.filename)
            {
                task.progress.downloaded = meta.len() as f64;
                task.progress.total = meta.len() as f64;
            }
            task.completed_at = Some(now as f64);
            let written: Vec<String> = if !filepaths.is_empty() {
                filepaths.to_vec()
            } else if !task.progress.filename.is_empty() {
                vec![task.progress.filename.clone()]
            } else {
                Vec::new()
            };
            let task = task.clone();
            // A file that was already there wrote nothing and is not counted.
            if !already_on_disk {
                self.out.push(Effect::Completed { task: task.clone(), filepaths: written });
            }
            self.emit(Event::Done { id: id.to_owned(), progress: task.progress });
            self.schedule_persist();
            self.finish_task(id);
        }
        self.finish()
    }

    pub fn on_error(&mut self, id: &str, attempt: u64, error: &str, now: u64) -> Vec<Effect> {
        self.begin(now);
        // The failing attempt's task is stamped with a typed code whether or
        // not the failure is then acted on.
        if let Some(task) = self.attempts.get(&attempt).and_then(|k| self.tasks.get_mut(k)) {
            task.error_code = Some(error_code_of(error));
        }
        if let Some(key) = self.admitted(id, attempt) {
            if self.recently_cancelled.contains_key(id) {
                self.finish_task(id); // the expected error of a process we killed
            } else {
                self.out.push(Effect::Failure(error.to_owned()));
                let task = self.task(key);
                let retry_count = task.retry_count.unwrap_or(0.0);
                let retry = classify_error(error) == ErrorClass::Retryable && retry_count < MAX_RETRIES;
                let code = task
                    .error_code
                    .and_then(|c| serde_json::to_value(c).ok())
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_else(|| "unknown".to_owned());
                self.out.push(Effect::Log {
                    level: "error",
                    message: format!(
                        "download {} {id} host={} code={code}: {}",
                        if retry { "retrying" } else { "failed" },
                        host_of(&task.url),
                        scrub_urls(&tail(error))
                    ),
                });
                if retry {
                    self.schedule_retry(key, error);
                } else {
                    self.transition(key, DownloadStatus::Failed);
                    // The row shows an explanation the user can act on, not raw output.
                    let shown = explain_error(error);
                    let progress = &mut self.task_mut(key).progress;
                    progress.status = DownloadStatus::Failed;
                    progress.error = shown.clone();
                    self.emit(Event::Error { id: id.to_owned(), message: shown });
                }
                self.finish_task(id);
            }
        }
        self.finish()
    }

    // ── Retry with growing waits, re-queued at the front ────────────────────

    fn schedule_retry(&mut self, key: Key, error: &str) {
        let prev = self.task(key).retry_count.unwrap_or(0.0);
        let delay = if prev >= 0.0 && prev.fract() == 0.0 && (prev as usize) < RETRY_BACKOFF_MS.len() {
            RETRY_BACKOFF_MS[prev as usize]
        } else {
            RETRY_BACKOFF_MS[RETRY_BACKOFF_MS.len() - 1]
        };
        self.transition(key, DownloadStatus::Retrying);
        let progress = &mut self.task_mut(key).progress;
        progress.status = DownloadStatus::Retrying;
        progress.error = error.to_owned();
        self.emit_progress(key);

        // Retry in place: the same task id, a bumped count and fresh progress,
        // so the retry ends under the original history row.
        let old = self.task(key).clone();
        let init = TaskInit {
            url: Some(old.url),
            source: Some(old.source),
            output_dir: Some(old.output_dir),
            format: Some(old.format),
            quality: Some(old.quality),
            video_quality: Some(old.video_quality),
            is_audio_only: Some(old.is_audio_only),
            is_playlist: Some(old.is_playlist),
            playlist_name: Some(old.playlist_name),
            embed_thumbnail: Some(old.embed_thumbnail),
            embed_metadata: Some(old.embed_metadata),
            skip_existing: Some(old.skip_existing),
            title: Some(old.title),
            thumbnail_url: Some(old.thumbnail_url),
            duration: Some(old.duration),
            uploader: Some(old.uploader),
            created_at: old.created_at,
            track: old.track,
            retry_count: Some(prev + 1.0),
            original_task_id: Some(old.original_task_id.unwrap_or_else(|| old.task_id.clone())),
            playlist_limit: old.playlist_limit,
            playlist_items: old.playlist_items,
            // Repeating the same command is only useful after a network
            // failure. YouTube's token gate reproduces exactly, so the retry
            // switches to the fallback clients. Only ever set to true.
            fallback_profile: (old.fallback_profile == Some(true) || needs_client_fallback(error)).then_some(true),
            extras: old.extras,
        };
        let id = old.task_id;
        let retried = self.insert(create_task(&init, id.clone(), self.now));
        self.history.insert(id.clone(), retried);
        // The recent list holds the live task, so swap in the fresh one.
        if let Some(slot) = self.recent.iter_mut().find(|k| **k == key) {
            *slot = retried;
        }
        let seq = self.later(TimerKind::Retry { id: id.clone(), key: retried }, delay);
        self.retry_timers.insert(id, seq);
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

    /// Free one slot. The slot is released before anyone is told, because the
    /// shutdown watcher reads `has_unfinished_work` when it hears of a change.
    fn finish_task(&mut self, id: &str) {
        self.active.shift_remove(id);
        self.clear_tick(id);
        self.tick_at.remove(id);
        self.out.push(Effect::Changed);
        // Deferred, never immediate: the next job starts after a brief pause.
        self.later(TimerKind::Pump, NEXT_PAUSE_MS);
    }

    /// Apply a status change if the state machine allows it. False, with the
    /// task untouched, for an impossible one; an unchanged status succeeds.
    fn transition(&mut self, key: Key, next: DownloadStatus) -> bool {
        let cur = self.status(key);
        if cur == next {
            return true;
        }
        if !can_transition(cur, next) {
            return false;
        }
        let now = self.now;
        let task = self.task_mut(key);
        task.progress.status = next;
        if is_terminal(next) {
            task.completed_at = Some(now as f64);
        }
        self.schedule_persist(); // a real status change is what history must keep
        true
    }

    fn mark_recently_cancelled(&mut self, id: &str) {
        if let Some(seq) = self.recently_cancelled.remove(id) {
            self.clear_timer(seq);
        }
        let seq = self.later(TimerKind::Grace { id: id.to_owned() }, CANCEL_GRACE_MS);
        self.recently_cancelled.insert(id.to_owned(), seq);
    }

    /// A message for the interface that also counts as a change in activity.
    fn emit(&mut self, event: Event) {
        self.out.push(Effect::Send(event));
        self.out.push(Effect::Changed);
    }

    fn emit_progress(&mut self, key: Key) {
        let task = self.task(key);
        self.emit(Event::Progress { id: task.task_id.clone(), progress: task.progress.clone() });
    }

    fn record_recent(&mut self, key: Key) {
        self.recent.retain(|k| *k != key);
        self.recent.insert(0, key);
        self.recent.truncate(RECENT_CAP);
    }
}

/// Node's `path.win32.isAbsolute`: a leading separator, or a drive letter,
/// a colon and a separator.
fn is_absolute_win32(p: &str) -> bool {
    let sep = |c: char| c == '\\' || c == '/';
    let mut chars = p.chars();
    match chars.next() {
        Some(c) if sep(c) => true,
        Some(c) if c.is_ascii_alphabetic() => chars.next() == Some(':') && chars.next().is_some_and(sep),
        _ => false,
    }
}
