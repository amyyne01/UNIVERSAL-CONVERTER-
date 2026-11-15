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
