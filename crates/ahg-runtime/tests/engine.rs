//! The engine thread end to end: a task goes in, the scripted stand-in for
//! yt-dlp runs, and the interface hears about it. Nothing touches the network.
mod common;

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use ahg_engine::queue::{Event, TaskInit, create_task};
use ahg_engine::types::DownloadStatus;
use ahg_runtime::download::Downloader;
use ahg_runtime::engine::{Engine, Notice};
use common::{pid_is_running, scratch, task, wait_for};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_ytdlp");
fn engine(dir: &std::path::Path, script: &str, slots: f64) -> (Engine, Receiver<Notice>) {
    let file = dir.join("script.txt");
    fs::write(&file, script).unwrap();
    let mut downloader = Downloader::new(PathBuf::from(FAKE), None);
    downloader.env("FAKE_YTDLP_SCRIPT", file.to_str().unwrap());
    let (tx, rx) = channel();
    let engine = Engine::start(downloader, &dir.join("downloads-history.json"), tx);
    engine.set_max_slots(slots);
    (engine, rx)
}

fn is_done(n: &Notice, id: &str) -> bool {
    matches!(n, Notice::Event(Event::Done { id: done, .. }) if done == id)
}

#[test]
fn a_task_runs_to_done_and_is_saved_to_history() {
    let dir = scratch("done");
    let (engine, rx) = engine(&dir, "err [PROGRESS]50|100|NA|10|5\nout [FILEPATH]C:\\Music\\a.mp3\nexit 0\n", 1.0);
    engine.enqueue(task("t1"));
    let seen = wait_for(&rx, |n| is_done(n, "t1"));
    assert!(seen.iter().any(|n| matches!(n, Notice::Event(Event::Queued { id, .. }) if id == "t1")));
    assert!(seen.iter().any(|n| matches!(n, Notice::Completed { filepaths, .. } if filepaths == &["C:\\Music\\a.mp3".to_owned()])));

    let tasks = engine.tasks();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].progress.status, DownloadStatus::Done);
    assert_eq!(tasks[0].progress.filename, "C:\\Music\\a.mp3");

    engine.shutdown();
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("downloads-history.json")).unwrap()).unwrap();
    assert_eq!(saved[0]["taskId"], "t1");
    assert_eq!(saved[0]["progress"]["status"], "done");
}

#[test]
fn one_slot_runs_two_tasks_one_after_the_other() {
    let dir = scratch("serial");
    let (engine, rx) = engine(&dir, "sleep 200\nexit 0\n", 1.0);
    engine.enqueue(task("a"));
    engine.enqueue(task("b"));
    let seen = wait_for(&rx, |n| is_done(n, "b"));
    let done_a = seen.iter().position(|n| is_done(n, "a")).expect("a finished first");
    let started_b = seen
        .iter()
        .position(|n| matches!(n, Notice::Event(Event::Progress { id, progress }) if id == "b" && progress.status == DownloadStatus::FetchingInfo))
        .expect("b started");
    assert!(done_a < started_b, "b must not start until a's slot is free");
    engine.shutdown();
}

#[test]
fn a_failed_download_is_reported_with_an_explanation() {
    let dir = scratch("failed");
    let (engine, rx) = engine(&dir, "err ERROR: [youtube] abc: Video unavailable\nexit 1\n", 1.0);
    engine.enqueue(task("t2"));
    let seen = wait_for(&rx, |n| matches!(n, Notice::Event(Event::Error { .. })));
    let Some(Notice::Event(Event::Error { message, .. })) = seen.last() else { unreachable!() };
    assert!(!message.is_empty() && !message.contains("[youtube]"), "the row shows an explanation, not raw output: {message}");
    assert_eq!(engine.tasks()[0].progress.status, DownloadStatus::Failed);
    engine.shutdown();
}

#[test]
fn pausing_a_running_task_stops_its_process_and_resuming_restarts_it() {
    let dir = scratch("pause");
    let (engine, rx) = engine(&dir, "err [PROGRESS]1|100|NA|1|9\nsleep 1500\nout [FILEPATH]C:\\Music\\a.mp3\nexit 0\n", 1.0);
    engine.enqueue(task("t3"));
    wait_for(&rx, |n| matches!(n, Notice::Event(Event::Progress { progress, .. }) if progress.status == DownloadStatus::Downloading));
    assert!(engine.pause("t3"));
    wait_for(&rx, |n| matches!(n, Notice::Event(Event::Progress { progress, .. }) if progress.status == DownloadStatus::Paused));
    assert_eq!(engine.tasks()[0].progress.status, DownloadStatus::Paused);
    assert!(engine.resume("t3"));
    wait_for(&rx, |n| is_done(n, "t3"));
    engine.shutdown();
}

#[test]
fn shutting_down_stops_running_downloads_and_leaves_them_to_be_resumed() {
    let dir = scratch("quit");
    let pid_file = dir.join("child.pid");
    let script = format!("child {}\nerr [PROGRESS]1|100|NA|1|9\nsleep 60000\nexit 0\n", pid_file.display());
    let (engine, rx) = engine(&dir, &script, 1.0);
    engine.enqueue(task("t4"));
    wait_for(&rx, |n| matches!(n, Notice::Event(Event::Progress { progress, .. }) if progress.status == DownloadStatus::Downloading));
    let child_pid = fs::read_to_string(&pid_file).unwrap();

    engine.shutdown();
    assert!(!pid_is_running(&child_pid), "no process may be left running after quit");

    // The next launch finds the task unfinished and parks it for the user to resume.
    let (tx, _rx) = channel();
    let next = Engine::start(Downloader::new(PathBuf::from(FAKE), None), &dir.join("downloads-history.json"), tx);
    let tasks = next.tasks();
    assert_eq!(tasks[0].progress.status, DownloadStatus::Paused);
    assert_eq!(tasks[0].interrupted, Some(true));
    next.shutdown();
}

/// The real yt-dlp and ffmpeg, on a 19-second clip (well under the 150 MB cap
/// for test downloads). Needs the network, so it only runs when asked:
/// `cargo test -p ahg-runtime --test engine -- --ignored`.
#[test]
#[ignore = "downloads a short real clip; needs the network and bin/yt-dlp.exe"]
fn the_real_yt_dlp_downloads_a_short_clip_as_mp3() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let (ytdlp, ffmpeg) = (root.join("bin").join("yt-dlp.exe"), root.join("bin").join("ffmpeg.exe"));
    assert!(ytdlp.exists() && ffmpeg.exists(), "bin/yt-dlp.exe and bin/ffmpeg.exe are needed");
    let dir = scratch("real");
    let (tx, rx) = channel();
    let downloader = Downloader::new(ytdlp, Some(ffmpeg.to_string_lossy().into_owned()));
    let engine = Engine::start(downloader, &dir.join("downloads-history.json"), tx);
    engine.set_max_slots(1.0);
    let init = TaskInit {
        url: Some("https://www.youtube.com/watch?v=jNQXAC9IVRw".into()),
        output_dir: Some(dir.to_string_lossy().into_owned()),
        format: Some("mp3".into()),
        quality: Some("128".into()),
        ..Default::default()
    };
    engine.enqueue(create_task(&init, "real".into(), 0));

    let deadline = Instant::now() + Duration::from_secs(180);
    let mut saw_progress = false;
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).expect("the download should finish in time") {
            Notice::Event(Event::Progress { progress, .. }) if progress.percent > 0.0 => saw_progress = true,
            Notice::Event(Event::Error { message, .. }) => panic!("the download failed: {message}"),
            Notice::Event(Event::Done { progress, .. }) => {
                let file = PathBuf::from(&progress.filename);
                assert!(file.exists(), "the reported file should be on disk: {}", progress.filename);
                assert_eq!(file.extension().and_then(|e| e.to_str()), Some("mp3"));
                assert!(fs::metadata(&file).unwrap().len() > 50_000, "the file should hold real audio");
                assert_eq!(progress.total, fs::metadata(&file).unwrap().len() as f64);
                break;
            }
            _ => {}
        }
    }
    assert!(saw_progress, "progress should have been reported on the way");
    engine.shutdown();
    let _ = fs::remove_dir_all(&dir);
}

/// A download that cannot even start must not hold its slot: the next task
/// gets its turn, and both end as failed rather than waiting for ever.
#[test]
fn a_download_that_cannot_start_frees_its_slot_for_the_next() {
    let dir = scratch("nostart");
    let (tx, rx) = channel();
    let downloader = Downloader::new(dir.join("no-such-yt-dlp.exe"), None);
    let engine = Engine::start(downloader, &dir.join("downloads-history.json"), tx);
    engine.set_max_slots(1.0);
    engine.enqueue(task("a"));
    engine.enqueue(task("b"));

    let mut failed = Vec::new();
    wait_for(&rx, |n| {
        if let Notice::Event(Event::Error { id, .. }) = n {
            failed.push(id.clone());
        }
        failed.len() == 2
    });
    assert_eq!(failed, ["a", "b"], "the second task started only after the first gave up its slot");
    assert!(engine.tasks().iter().all(|t| t.progress.status == DownloadStatus::Failed));
    assert!(!engine.has_unfinished_work());
    engine.shutdown();
}

/// A task whose process cannot be spawned (a NUL byte in its link makes the
/// start fail) must not wedge the queue: all three end and the third runs.
#[test]
fn a_middle_task_that_cannot_start_does_not_stop_the_third() {
    let dir = scratch("middle");
    let (engine, rx) = engine(&dir, "sleep 100\nexit 0\n", 1.0);
    let mut broken = task("b");
    broken.url.push('\0');
    engine.enqueue(task("a"));
    engine.enqueue(broken);
    engine.enqueue(task("c"));

    let mut ended = Vec::new();
    wait_for(&rx, |n| {
        if let Notice::Event(Event::Done { id, .. } | Event::Error { id, .. }) = n {
            ended.push(id.clone());
        }
        ended.len() == 3
    });
    assert_eq!(ended, ["a", "b", "c"]);
    let tasks = engine.tasks();
    let status = |id: &str| tasks.iter().find(|t| t.task_id == id).unwrap().progress.status;
    assert_eq!(status("a"), DownloadStatus::Done);
    assert_eq!(status("b"), DownloadStatus::Failed);
    assert_eq!(status("c"), DownloadStatus::Done);
    assert!(!engine.has_unfinished_work());
    engine.shutdown();
}
