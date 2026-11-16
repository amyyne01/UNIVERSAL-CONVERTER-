//! The engine thread. It owns the queue and carries out what the queue
//! decides: it starts and stops downloads, wakes when a timer falls due, and
//! passes messages on to whoever is listening (the interface, the tray, the
//! statistics).
//!
//! Everything reaches the queue through one channel, so the queue is only ever
//! touched from this one thread.
use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ahg_engine::log::log_line;
use ahg_engine::queue::{Effect, Event, Queue};
use ahg_engine::types::DownloadTask;

use crate::download::{Downloader, Report};
use crate::{lock, now_ms};

/// What the engine tells the outside.
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// A message for the interface.
    Event(Event),
    /// The queue's activity changed: tray, presence and the shutdown watcher re-read it.
    Changed,
    /// A download finished and wrote these files.
    Completed { task: Box<DownloadTask>, filepaths: Vec<String> },
    /// The raw error text of a failed download.
    Failure(String),
}

type Job = Box<dyn FnOnce(&mut Queue, u64) -> Vec<Effect> + Send>;

enum Msg {
    /// Something to do to the queue, on the engine thread.
    Job(Job),
    Report(Report),
    Shutdown,
}

pub struct Engine {
    tx: Sender<Msg>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Engine {
    /// Start the engine thread. Saved history is loaded from `history_file`,
    /// and unfinished tasks from the last run come back parked.
    pub fn start(downloader: Downloader, history_file: &Path, notices: Sender<Notice>) -> Self {
        let (tx, rx) = channel();
        let mut queue = Queue::new();
        queue.init_history(history_file, now_ms());
        let reports = forward_reports(tx.clone());
        let thread = thread::spawn(move || run(queue, downloader, rx, reports, notices));
        Self { tx, thread: Mutex::new(Some(thread)) }
    }

    /// Run `job` on the engine thread and wait for its answer.
    fn ask<T: Send + 'static>(&self, job: impl FnOnce(&mut Queue, u64) -> (T, Vec<Effect>) + Send + 'static) -> Option<T> {
        let (reply, answer) = channel();
        let job: Job = Box::new(move |queue, now| {
            let (value, effects) = job(queue, now);
            let _ = reply.send(value);
            effects
        });
        self.tx.send(Msg::Job(job)).ok()?;
        answer.recv().ok()
    }

    /// Run `job` on the engine thread without waiting.
    fn tell(&self, job: impl FnOnce(&mut Queue, u64) -> Vec<Effect> + Send + 'static) {
        let _ = self.tx.send(Msg::Job(Box::new(job)));
    }

    pub fn set_max_slots(&self, slots: f64) {
        self.tell(move |q, now| q.set_max_slots(slots, now));
    }

    pub fn enqueue(&self, task: DownloadTask) {
        self.tell(move |q, now| q.enqueue(task, now));
    }

    pub fn pause(&self, id: &str) -> bool {
        let id = id.to_owned();
        self.ask(move |q, now| q.pause_task(&id, now)).unwrap_or(false)
    }

    pub fn resume(&self, id: &str) -> bool {
        let id = id.to_owned();
        self.ask(move |q, now| q.resume_task(&id, now)).unwrap_or(false)
    }

    pub fn cancel(&self, id: &str) -> bool {
        let id = id.to_owned();
        self.ask(move |q, now| q.cancel_task(&id, now)).unwrap_or(false)
    }

    pub fn promote(&self, id: &str) -> bool {
        let id = id.to_owned();
        self.ask(move |q, now| q.promote_task(&id, now)).unwrap_or(false)
    }

    pub fn pause_all(&self) {
        self.tell(|q, now| q.pause_all(now));
    }

    pub fn resume_all(&self) {
        self.tell(|q, now| q.resume_all(now));
    }

    pub fn cancel_all(&self) {
        self.tell(|q, now| q.cancel_all(now));
    }

    pub fn remove(&self, ids: Vec<String>) {
        self.tell(move |q, now| q.remove_tasks(&ids, now));
    }

    pub fn reorder(&self, ids: Vec<String>) {
        self.tell(move |q, now| q.reorder(&ids, now));
    }

    /// Every task in history, oldest first.
    pub fn tasks(&self) -> Vec<DownloadTask> {
        self.ask(|q, _| (q.all_tasks().into_iter().cloned().collect(), Vec::new())).unwrap_or_default()
    }

    /// One task in history.
    pub fn task(&self, id: &str) -> Option<DownloadTask> {
        let id = id.to_owned();
        self.ask(move |q, _| (q.all_tasks().into_iter().find(|t| t.task_id == id).cloned(), Vec::new())).flatten()
    }

    /// When the task's pending automatic retry starts, if it has one.
    pub fn retry_due(&self, id: &str) -> Option<u64> {
        let id = id.to_owned();
        self.ask(move |q, _| (q.retry_due(&id), Vec::new())).flatten()
    }

    /// True while anything is running or still queued.
    pub fn has_unfinished_work(&self) -> bool {
        self.ask(|q, _| (q.has_unfinished_work(), Vec::new())).unwrap_or(false)
    }

    /// True when the engine is running and has nothing left to do. A stopped
    /// engine is not "drained": nothing may be concluded from it.
    pub(crate) fn is_drained(&self) -> bool {
        self.ask(|q, _| (q.has_unfinished_work(), Vec::new())).is_some_and(|busy| !busy)
    }

    /// Stop every running download, save the history and end the thread.
    /// Unfinished tasks stay in history and come back parked at the next start.
    pub fn shutdown(&self) {
        let _ = self.tx.send(Msg::Shutdown);
        let thread = lock(&self.thread).take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Downloads report on their own channel; this passes each report on to the engine's.
fn forward_reports(tx: Sender<Msg>) -> Sender<Report> {
    let (reports, incoming) = channel();
    thread::spawn(move || {
        for report in incoming {
            if tx.send(Msg::Report(report)).is_err() {
                break;
            }
        }
    });
    reports
}

fn run(mut queue: Queue, downloader: Downloader, rx: Receiver<Msg>, reports: Sender<Report>, notices: Sender<Notice>) {
    loop {
        // Sleep until a message arrives or the next timer falls due.
        let wait = queue.next_deadline().map(|due| Duration::from_millis(due.saturating_sub(now_ms())));
        let msg = match wait {
            Some(wait) => match rx.recv_timeout(wait) {
                Ok(msg) => Some(msg),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => Some(Msg::Shutdown),
            },
            None => Some(rx.recv().unwrap_or(Msg::Shutdown)),
        };
        let now = now_ms();
        let effects = match msg {
            None => queue.advance(now),
            Some(Msg::Job(job)) => {
                let mut effects = job(&mut queue, now);
                effects.extend(queue.advance(now));
                effects
            }
            Some(Msg::Report(report)) => match report {
                Report::Progress { id, attempt, progress } => queue.on_progress(&id, attempt, progress, now),
                Report::Done { id, attempt, path, filepaths, already_on_disk } => {
                    queue.on_done(&id, attempt, &path, &filepaths, already_on_disk, now)
                }
                Report::Error { id, attempt, message } => queue.on_error(&id, attempt, &message, now),
                Report::Match { id, attempt, url, confidence } => {
                    queue.on_match(&id, attempt, &url, confidence);
                    Vec::new()
                }
            },
            Some(Msg::Shutdown) => {
                // Quit: no process may be left running. Partial files stay, and
                // the tasks are saved unfinished so the next start parks them.
                downloader.cancel_all();
                downloader.wait_for_searches();
                queue.flush_history();
                return;
            }
        };
        for effect in effects {
            match effect {
                Effect::Send(event) => drop(notices.send(Notice::Event(event))),
                Effect::Changed => drop(notices.send(Notice::Changed)),
                Effect::Failure(text) => drop(notices.send(Notice::Failure(text))),
                Effect::Completed { task, filepaths } => drop(notices.send(Notice::Completed { task: Box::new(task), filepaths })),
                Effect::Download { task, attempt } => downloader.start(task, attempt, reports.clone()),
                Effect::Cancel(id) => downloader.cancel(&id),
                Effect::CancelAll => downloader.cancel_all(),
                Effect::Log { level, message } => log_line(level, &message),
            }
        }
    }
}
