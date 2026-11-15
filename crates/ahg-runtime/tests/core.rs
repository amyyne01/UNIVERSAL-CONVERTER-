//! The app core end to end, with the scripted stand-in for yt-dlp: settings,
//! plan, queue and statistics working together. Nothing touches the network.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use ahg_engine::queue::Event;
use ahg_engine::tier::Plan;
use ahg_engine::types::{DownloadStatus, VideoQuality};
use ahg_runtime::core::{Core, CoreEvent, Paths};
use ahg_runtime::ytdlp::YtDlp;
use serde_json::{Value, json};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_ytdlp");
const WAIT: Duration = Duration::from_secs(20);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ahg-core-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("userData")).unwrap();
    fs::create_dir_all(dir.join("Downloads")).unwrap();
    dir
}

fn start(dir: &Path, script: &str, plan: Plan) -> (Core, Receiver<CoreEvent>) {
    fs::write(dir.join("script.txt"), script).unwrap();
    let mut ytdlp = YtDlp::new(PathBuf::from(FAKE), None);
    ytdlp.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
    ytdlp.env("FAKE_YTDLP_ARGS", dir.join("args.txt").to_str().unwrap());
    let (tx, rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    (Core::start(paths, Arc::new(ytdlp), Box::new(move || plan), tx), rx)
}

fn wait_for(rx: &Receiver<CoreEvent>, mut want: impl FnMut(&CoreEvent) -> bool) -> Vec<CoreEvent> {
    let mut seen = Vec::new();
    let deadline = Instant::now() + WAIT;
    while let Ok(event) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        let hit = want(&event);
        seen.push(event);
        if hit {
            return seen;
        }
    }
    panic!("never saw the expected event; saw {}", seen.len());
}

fn done(e: &CoreEvent) -> bool {
    matches!(e, CoreEvent::Download(Event::Done { .. }))
}

fn request() -> Value {
    json!({ "url": "https://youtu.be/dQw4w9WgXcQ", "source": "youtube", "isAudioOnly": false, "videoQuality": "2160p", "format": "flac", "quality": "lossless" })
}

#[test]
fn a_request_is_checked_clamped_to_the_plan_and_downloaded() {
    let dir = scratch("basic");
    let media = dir.join("Downloads").join("a.mp3");
    fs::write(&media, vec![0u8; 2048]).unwrap();
    let script = format!("out [FILEPATH]{}\nexit 0\n", media.display());
    let (core, rx) = start(&dir, &script, Plan::Basic);

    let task = core.start_download(&request()).unwrap();
    // The free tier's ceilings apply, and the folder comes from Settings.
    assert_eq!((task.format.as_str(), task.quality.as_str(), task.video_quality), ("mp3", "320", VideoQuality::P1080));
    assert_eq!(task.output_dir, dir.join("Downloads").to_string_lossy());
    assert!(task.task_id.starts_with("task_"));
    wait_for(&rx, done);

    // The finished file is counted in the month's statistics.
    let stats = core.stats();
    assert_eq!((stats.current.files, stats.current.bytes), (1.0, 2048.0));
    assert_eq!(stats.current.by_platform["youtube"], 1);
    assert_eq!(core.tasks()[0].progress.status, DownloadStatus::Done);
    core.shutdown();
}

#[test]
fn a_premium_request_keeps_what_it_asked_for() {
    let dir = scratch("premium");
    let (core, rx) = start(&dir, "exit 0\n", Plan::Premium);
    let task = core.start_download(&request()).unwrap();
    assert_eq!((task.format.as_str(), task.quality.as_str(), task.video_quality), ("flac", "lossless", VideoQuality::P2160));
    wait_for(&rx, done);
    core.shutdown();
}

#[test]
fn a_bad_request_is_refused_and_nothing_is_queued() {
    let dir = scratch("refused");
    let (core, _rx) = start(&dir, "exit 0\n", Plan::Premium);
    assert_eq!(core.start_download(&json!({ "url": "-rf" })).unwrap_err(), "Invalid input: URL must not start with \"-\"");
    assert_eq!(core.start_download(&json!(5)).unwrap_err(), "Invalid input: expected a task object");
    assert!(core.tasks().is_empty());
    core.shutdown();
}

#[test]
fn settings_are_checked_saved_and_announced() {
    let dir = scratch("settings");
    let (core, rx) = start(&dir, "exit 0\n", Plan::Premium);
    assert_eq!(core.settings()["defaultFormat"], "mp3");

    // One good value and one the engine refuses: the good one is kept, the bad one reported.
    let refused = core.update_settings(json!({ "defaultFormat": "opus", "rateLimit": "2 MB" }).as_object().unwrap());
    assert_eq!(refused, vec!["rateLimit".to_owned()]);
    assert_eq!(core.settings()["defaultFormat"], "opus");
    assert_eq!(core.settings()["rateLimit"], "");
    wait_for(&rx, |e| matches!(e, CoreEvent::SettingsChanged));

    core.shutdown();
    let saved: Value = serde_json::from_slice(&fs::read(dir.join("userData").join("config.json")).unwrap()).unwrap();
    assert_eq!(saved["defaultFormat"], "opus");
}

#[test]
fn the_number_of_simultaneous_downloads_follows_the_setting_and_the_plan() {
    let dir = scratch("slots");
    let script = "err [PROGRESS]1|100|NA|1|9\nsleep 1200\nexit 0\n";
    // Settings ask for four at once; the free tier allows two.
    let (core, rx) = start(&dir, script, Plan::Basic);
    core.update_settings(json!({ "maxConcurrentDownloads": 4 }).as_object().unwrap());
    for _ in 0..3 {
        core.start_download(&request()).unwrap();
    }
    wait_for(&rx, |e| matches!(e, CoreEvent::Download(Event::Progress { progress, .. }) if progress.status == DownloadStatus::Downloading));
    std::thread::sleep(Duration::from_millis(400));
    let running = core.tasks().iter().filter(|t| !matches!(t.progress.status, DownloadStatus::Queued | DownloadStatus::Done)).count();
    assert_eq!(running, 2, "two slots on the free tier, whatever the setting says");
    core.shutdown();
}

#[test]
fn the_proxy_setting_reaches_link_lookups() {
    let dir = scratch("proxy");
    let (core, _rx) = start(&dir, "out {\"id\":\"a\",\"title\":\"A\"}\nexit 0\n", Plan::Premium);
    core.update_settings(json!({ "proxy": "socks5h://127.0.0.1:9050" }).as_object().unwrap());
    core.fetch_metadata("https://youtu.be/dQw4w9WgXcQ").unwrap();
    let args = fs::read_to_string(dir.join("args.txt.json")).unwrap();
    assert!(args.contains("--proxy\nsocks5h://127.0.0.1:9050\n"), "{args}");
    core.shutdown();
}

// ── The schedule ────────────────────────────────────────────────────────────

/// Save a schedule that is due this very minute, then stop: the next start
/// finds a run it has to catch up on.
fn with_a_schedule_due_now(dir: &Path, shutdown: bool) {
    let now = ahg_engine::clock::LocalTime::now();
    let (core, _rx) = start(dir, "", Plan::Premium);
    let due = json!({ "scheduleEnabled": true, "scheduleTime": format!("{:02}:{:02}", now.hour, now.minute), "scheduleDays": [], "scheduleShutdown": shutdown });
    assert_eq!(core.update_settings(due.as_object().unwrap()), Vec::<String>::new());
    core.shutdown();
}

fn never_within(rx: &Receiver<CoreEvent>, time: Duration, unwanted: impl Fn(&CoreEvent) -> bool) {
    let deadline = Instant::now() + time;
    while let Ok(event) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        assert!(!unwanted(&event), "saw {event:?}");
    }
}

#[test]
fn a_schedule_that_is_due_fires_at_startup_and_only_once_that_day() {
    let dir = scratch("schedule");
    with_a_schedule_due_now(&dir, false);

    let (core, rx) = start(&dir, "", Plan::Premium);
    wait_for(&rx, |e| *e == CoreEvent::ScheduleFired);
    never_within(&rx, Duration::from_millis(500), |e| *e == CoreEvent::ShutdownRequested);
    core.shutdown();

    // D11: the day it fired is on disk, so a restart does not fire again.
    let saved = fs::read_to_string(dir.join("userData").join("scheduler-state.json")).unwrap();
    assert!(saved.contains("lastFiredDate"), "{saved}");
    let (core, rx) = start(&dir, "", Plan::Premium);
    never_within(&rx, Duration::from_millis(1500), |e| *e == CoreEvent::ScheduleFired);
    core.shutdown();
}

#[test]
fn the_schedule_does_not_fire_on_the_basic_plan() {
    let dir = scratch("schedule-basic");
    with_a_schedule_due_now(&dir, false);
    let (core, rx) = start(&dir, "", Plan::Basic);
    never_within(&rx, Duration::from_millis(1500), |e| *e == CoreEvent::ScheduleFired);
    core.shutdown();
}

#[test]
fn shutdown_after_a_scheduled_run_waits_for_the_queue_to_empty() {
    let dir = scratch("schedule-shutdown");
    with_a_schedule_due_now(&dir, true);
    let media = dir.join("Downloads").join("a.mp3");
    fs::write(&media, vec![0u8; 2048]).unwrap();
    let script = format!("sleep 2000\nout [FILEPATH]{}\nexit 0\n", media.display());
    let (core, rx) = start(&dir, &script, Plan::Premium);
    let began = Instant::now();
    core.start_download(&request()).unwrap();
    wait_for(&rx, |e| *e == CoreEvent::ShutdownRequested);
    assert!(began.elapsed() >= Duration::from_secs(2), "shutdown was requested while the download was still running");
    assert_eq!(core.tasks()[0].progress.status, DownloadStatus::Done);
    assert!(began.elapsed() < Duration::from_secs(8), "the request should follow the queue emptying, not the fallback timer");
    core.shutdown();
}

#[test]
fn shutdown_after_a_scheduled_run_with_nothing_to_download_comes_after_a_short_wait() {
    let dir = scratch("schedule-empty");
    with_a_schedule_due_now(&dir, true);
    let (core, rx) = start(&dir, "", Plan::Premium);
    let began = Instant::now();
    wait_for(&rx, |e| *e == CoreEvent::ShutdownRequested);
    assert!(began.elapsed() >= Duration::from_secs(9), "downloads get time to start before the PC is shut down");
    core.shutdown();
}
