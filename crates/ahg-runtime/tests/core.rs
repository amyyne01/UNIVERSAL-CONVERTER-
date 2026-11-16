//! The app core end to end, with the scripted stand-in for yt-dlp: settings,
//! plan, queue and statistics working together. Nothing touches the network.
mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use ahg_engine::queue::Event;
use ahg_engine::tier::Plan;
use ahg_engine::types::{DownloadStatus, VideoQuality};
use ahg_runtime::core::{CookiesImport, Core, CoreEvent, Outside, Paths, Shown, TrackMatch};
use ahg_runtime::ytdlp::{CANCELLED_MESSAGE, YtDlp};
use common::{pid_is_running, runs, wait_for};
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

#[test]
fn an_upgrade_raises_the_number_of_simultaneous_downloads_without_a_restart() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let dir = scratch("upgrade");
    fs::write(dir.join("script.txt"), "err [PROGRESS]1|100|NA|1|9\nsleep 3000\nexit 0\n").unwrap();
    let mut ytdlp = YtDlp::new(PathBuf::from(FAKE), None);
    ytdlp.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
    let premium = Arc::new(AtomicBool::new(false));
    let plan = { let premium = premium.clone(); move || if premium.load(Ordering::SeqCst) { Plan::Premium } else { Plan::Basic } };
    let (tx, rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let core = Core::start(paths, Arc::new(ytdlp), Box::new(plan), tx);
    let running = |core: &Core| core.tasks().iter().filter(|t| !matches!(t.progress.status, DownloadStatus::Queued | DownloadStatus::Done)).count();

    core.update_settings(json!({ "maxConcurrentDownloads": 4 }).as_object().unwrap());
    for _ in 0..4 {
        core.start_download(&request()).unwrap();
    }
    wait_for(&rx, |e| matches!(e, CoreEvent::Download(Event::Progress { progress, .. }) if progress.status == DownloadStatus::Downloading));
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(running(&core), 2, "the free tier runs two");

    premium.store(true, Ordering::SeqCst);
    core.plan_changed();
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(running(&core), 4, "the key was just activated: all four run");
    core.shutdown();
}

// ── Matching a Spotify track for the playlist preview (D34) ─────────────────

#[test]
fn the_preview_can_ask_for_one_tracks_match() {
    use ahg_engine::types::Track;
    let dir = scratch("preview");
    fs::write(dir.join("script.txt"), "").unwrap();
    let ytdlp = YtDlp::new(PathBuf::from(FAKE), None);
    let column = |text: &str| json!({ "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": text }] } } });
    let response = json!({ "contents": [{ "musicResponsiveListItemRenderer": {
        "flexColumns": [column("The Song"), column("Artist • The Album • 3:20")], "playlistItemData": { "videoId": "right000001" } } }] })
    .to_string();
    let search: ahg_runtime::download::SongSearch = Arc::new(move |query: &str| (!query.contains("Offline")).then(|| response.clone()));
    let (tx, _rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let core = Core::start_with_search(paths, Arc::new(ytdlp), Box::new(|| Plan::Premium), tx, search);

    let track: Track = serde_json::from_value(json!({ "id": "t1", "name": "The Song", "artist": "Artist", "artists": ["Artist"], "album": "The Album", "trackNumber": 1, "durationMs": 200000, "thumbnailUrl": "" })).unwrap();
    let TrackMatch::Found(found) = core.match_track(&track) else { panic!("the song agrees with the track") };
    assert_eq!(found.url, "https://music.youtube.com/watch?v=right000001");
    assert_eq!((found.title.as_str(), found.artist.as_str(), found.album.as_str(), found.duration), ("The Song", "Artist", "The Album", Some(200.0)));
    assert_eq!(found.confidence, 1.0);

    // No song agrees: the preview says so. The download still tries its fallbacks.
    let other = Track { name: "Another Song".into(), ..track.clone() };
    assert_eq!(core.match_track(&other), TrackMatch::NoSong);
    // The search could not be reached: that is not "no match", and is worth trying again.
    let offline = Track { name: "Offline".into(), ..track.clone() };
    assert_eq!(core.match_track(&offline), TrackMatch::Unreachable);
    core.shutdown();
}

/// The whole Spotify journey on the real services: the track is read from
/// Spotify, matched on YouTube Music, and downloaded as an MP3 by the real
/// yt-dlp. A few megabytes of audio. Network; run with --ignored.
#[test]
#[ignore = "downloads one real song"]
fn a_real_spotify_track_is_matched_and_downloaded() {
    use ahg_engine::spotify::{Spotify, system_now_ms};
    use ahg_runtime::http::SpotifyHttp;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let (bin, ffmpeg) = (root.join("bin").join("yt-dlp.exe"), root.join("bin").join("ffmpeg.exe"));
    assert!(bin.exists() && ffmpeg.exists(), "bin/yt-dlp.exe and bin/ffmpeg.exe are needed");
    let dir = scratch("realspotify");
    let (tx, rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let core = Core::start(paths, Arc::new(YtDlp::new(bin, Some(ffmpeg.to_string_lossy().into_owned()))), Box::new(|| Plan::Premium), tx);

    // Fleetwood Mac, "Dreams".
    let mut spotify = Spotify::new(SpotifyHttp::new(|| None), Box::new(system_now_ms));
    let track = spotify.fetch_track("https://open.spotify.com/track/0ofHAoxe9vBkTCp2UQIavz").ok().flatten().expect("the track should load");
    println!("spotify: {} - {} ({} s, explicit {:?})", track.artist, track.name, track.duration_ms / 1000.0, track.explicit);
    let TrackMatch::Found(found) = core.match_track(&track) else { panic!("YouTube Music should have this song") };
    println!("match: {} / {} / {} ({:?} s), confidence {}, {}", found.title, found.artist, found.album, found.duration, found.confidence, found.url);
    assert_eq!(found.confidence, 1.0);

    let request = json!({ "url": track.spotify_url, "source": "spotify", "isAudioOnly": true, "format": "mp3", "quality": "128",
        "track": track, "matchedUrl": found.url, "matchConfidence": found.confidence });
    let task = core.start_download(&request).unwrap();
    let seen = wait_for_long(&rx, |e| matches!(e, CoreEvent::Download(Event::Done { .. }) | CoreEvent::DownloadFailed(_)));
    assert!(matches!(seen.last(), Some(CoreEvent::Download(Event::Done { .. }))), "the download failed: {:?}", seen.last());
    let done = core.tasks().into_iter().find(|t| t.task_id == task.task_id).unwrap();
    let file = PathBuf::from(&done.progress.filename);
    println!("file: {} ({} bytes)", done.progress.filename, fs::metadata(&file).unwrap().len());
    assert_eq!(file.extension().and_then(|e| e.to_str()), Some("mp3"));
    assert!(fs::metadata(&file).unwrap().len() > 1_000_000, "about four minutes of audio");
    assert_eq!(done.matched_url.as_deref(), Some(found.url.as_str()));
    // The real file is the track's length, so the match is still confirmed after the check.
    assert_eq!(done.match_confidence, Some(1.0));
    // Named and tagged from Spotify's data, not from the video's title.
    assert_eq!(file.file_name().and_then(|n| n.to_str()), Some("Fleetwood Mac - Dreams - 2004 Remaster.mp3"));
    let probe = std::process::Command::new(root.join("bin").join("ffmpeg.exe")).args(["-hide_banner", "-i"]).arg(&file).output().unwrap();
    let probe = String::from_utf8_lossy(&probe.stderr).into_owned();
    let tag = |name: &str| probe.lines().find_map(|l| l.trim().strip_prefix(name).map(|rest| rest.trim_start().trim_start_matches(':').trim().to_owned()));
    println!("tags: title {:?}, artist {:?}, album {:?}, track {:?}, date {:?}", tag("title"), tag("artist"), tag("album"), tag("track"), tag("date"));
    assert_eq!(tag("title").as_deref(), Some(track.name.as_str()));
    assert_eq!(tag("artist").as_deref(), Some("Fleetwood Mac"));
    // The file says where it came from.
    let comment = tag("comment").unwrap_or_default();
    println!("comment: {comment}");
    let video_id = found.url.rsplit('=').next().unwrap();
    assert!(comment.starts_with("Source: ") && comment.contains(video_id), "{comment}");
    // A single track's page names no album, so that tag is left to YouTube's own.
    assert!(track.album.is_empty() || tag("album").as_deref() == Some(track.album.as_str()));
    assert!(tag("album").is_some_and(|album| !album.is_empty()));
    core.shutdown();
    let _ = fs::remove_dir_all(&dir);
}

fn wait_for_long(rx: &Receiver<CoreEvent>, mut want: impl FnMut(&CoreEvent) -> bool) -> Vec<CoreEvent> {
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(240);
    while let Ok(event) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        let hit = want(&event);
        seen.push(event);
        if hit {
            break;
        }
    }
    seen
}

#[test]
fn the_needs_review_choice_is_off_at_first_and_survives_a_restart() {
    let dir = scratch("holdsetting");
    let (core, _rx) = start(&dir, "", Plan::Premium);
    assert!(!core.hold_unconfirmed(), "a new install downloads best guesses");
    core.set_hold_unconfirmed(true);
    assert!(core.hold_unconfirmed());
    core.shutdown();

    let (core, _rx) = start(&dir, "", Plan::Premium);
    assert!(core.hold_unconfirmed());
    core.shutdown();
    // It has its own file: the settings file stays exactly what Electron writes.
    assert!(!fs::read_to_string(dir.join("userData").join("config.json")).unwrap_or_default().contains("nconfirmed"));
}

// ── A match the user chose is remembered ────────────────────────────────────

#[test]
fn a_chosen_match_is_remembered_for_that_track_and_used_from_then_on() {
    use ahg_engine::downloader::bridge::SongMatch;
    use ahg_engine::types::Track;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let dir = scratch("remember");
    let searches = Arc::new(AtomicUsize::new(0));
    let begin = |dir: &Path| {
        fs::write(dir.join("script.txt"), "sleep 3000\nexit 0\n").unwrap();
        let mut ytdlp = YtDlp::new(PathBuf::from(FAKE), None);
        ytdlp.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
        let count = searches.clone();
        let search: ahg_runtime::download::SongSearch = Arc::new(move |_: &str| {
            count.fetch_add(1, Ordering::SeqCst);
            None
        });
        let (tx, rx) = channel();
        let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
        (Core::start_with_search(paths, Arc::new(ytdlp), Box::new(|| Plan::Premium), tx, search), rx)
    };
    let track: Track = serde_json::from_value(json!({ "id": "t1", "name": "The Song", "artist": "Artist", "artists": ["Artist"], "album": "", "trackNumber": 1, "durationMs": 200000, "thumbnailUrl": "" })).unwrap();
    let chosen = SongMatch { url: "https://www.youtube.com/watch?v=mine0000001".into(), title: "The Song (the one I wanted)".into(), artist: "Artist".into(), album: String::new(), duration: Some(201.0), explicit: false, confidence: 0.4 };

    let (core, _rx) = begin(&dir);
    assert_eq!(core.match_track(&track), TrackMatch::Unreachable);
    assert!(core.remember_match("t1", &chosen));
    // An address that is not YouTube's is refused: it would be handed to yt-dlp.
    assert!(!core.remember_match("t2", &SongMatch { url: "https://evil.example/x".into(), ..chosen.clone() }));
    assert!(!core.remember_match("", &chosen));
    core.shutdown();

    let (core, _rx) = begin(&dir);
    let before = searches.load(Ordering::SeqCst);
    let TrackMatch::Found(found) = core.match_track(&track) else { panic!("the choice was saved") };
    assert_eq!((found.url.as_str(), found.title.as_str()), (chosen.url.as_str(), chosen.title.as_str()));
    assert_eq!(found.confidence, 1.0, "the user's own choice is a confirmed match");
    assert_eq!(searches.load(Ordering::SeqCst), before, "a remembered match needs no search");

    // A download of that track with no match named (the hotkey, say) uses it too.
    let task = core.start_download(&json!({ "url": "https://open.spotify.com/track/t1", "source": "spotify", "track": track })).unwrap();
    assert_eq!(task.matched_url.as_deref(), Some(chosen.url.as_str()));
    assert_eq!(task.match_confidence, Some(1.0));
    // A match named by the request still wins over the remembered one.
    let named = core.start_download(&json!({ "url": "https://open.spotify.com/track/t1", "source": "spotify", "track": track, "matchedUrl": "https://music.youtube.com/watch?v=other000001" })).unwrap();
    assert_eq!(named.matched_url.as_deref(), Some("https://music.youtube.com/watch?v=other000001"));

    core.forget_match("t1");
    assert_eq!(core.match_track(&track), TrackMatch::Unreachable);
    core.shutdown();
    let (core, _rx) = begin(&dir);
    assert_eq!(core.match_track(&track), TrackMatch::Unreachable, "forgetting is saved too");
    core.shutdown();
}

#[test]
fn a_tampered_list_of_remembered_matches_cannot_send_yt_dlp_anywhere() {
    use ahg_engine::types::Track;
    let dir = scratch("remembertamper");
    fs::write(dir.join("userData").join("spotify-matches.json"), r#"{"t1":{"url":"--exec=calc","title":"x","artist":"","album":"","duration":null,"explicit":false,"confidence":1},"t2":"nonsense"}"#).unwrap();
    // A song search that is never reachable: this test must not touch the network.
    fs::write(dir.join("script.txt"), "exit 1
").unwrap();
    let mut ytdlp = YtDlp::new(PathBuf::from(FAKE), None);
    ytdlp.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
    let (tx, _rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let core = Core::start_with_search(paths, Arc::new(ytdlp), Box::new(|| Plan::Premium), tx, Arc::new(|_: &str| None));
    let track: Track = serde_json::from_value(json!({ "id": "t1", "name": "No Such Song zzqq", "artist": "Nobody zzqq", "artists": ["Nobody zzqq"], "album": "", "trackNumber": 1, "durationMs": 200000, "thumbnailUrl": "" })).unwrap();
    let task = core.start_download(&json!({ "url": "https://open.spotify.com/track/t1", "source": "spotify", "track": track })).unwrap();
    assert_eq!(task.matched_url, None);
    core.shutdown();
}

// ── Keeping yt-dlp current, through the core ────────────────────────────────

/// A release feed that always answers with the same new yt-dlp.
struct OneRelease {
    binary: Vec<u8>,
}

impl ahg_runtime::engine_update::Fetch for OneRelease {
    fn get(&self, request: &ahg_engine::updater::Request) -> Option<ahg_runtime::engine_update::Fetched> {
        use sha2::{Digest, Sha256};
        let base = "https://github.com/yt-dlp/yt-dlp/releases/download/2099.01.01";
        let body = if request.url.ends_with("/releases/latest") {
            json!({ "tag_name": "2099.01.01", "assets": [
                { "name": "yt-dlp.exe", "browser_download_url": format!("{base}/yt-dlp.exe") },
                { "name": "SHA2-256SUMS", "browser_download_url": format!("{base}/SHA2-256SUMS") } ] })
            .to_string()
            .into_bytes()
        } else if request.url.ends_with("/yt-dlp.exe") {
            self.binary.clone()
        } else {
            let sha: String = Sha256::digest(&self.binary).iter().map(|b| format!("{b:02x}")).collect();
            format!("{sha}  yt-dlp.exe\n").into_bytes()
        };
        Some(ahg_runtime::engine_update::Fetched { status: 200, final_url: request.url.clone(), content_length: None, location: None, body: Box::new(std::io::Cursor::new(body)) })
    }
}

fn start_with_release(dir: &Path, script: &str) -> (Core, Receiver<CoreEvent>) {
    fs::write(dir.join("script.txt"), script).unwrap();
    let mut ytdlp = YtDlp::new(PathBuf::from(FAKE), None);
    ytdlp.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
    let (tx, rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let core = Core::start_with(paths, Arc::new(ytdlp), Box::new(|| Plan::Premium), tx, Outside { song_search: Arc::new(|_: &str| None), releases: Arc::new(OneRelease { binary: b"MZ a newer yt-dlp".to_vec() }), desktop: Box::new(ahg_runtime::desktop::Windows), spotify: offline(), cover: Box::new(|_, _| None) });
    (core, rx)
}

fn engine_states(seen: &[CoreEvent]) -> Vec<String> {
    seen.iter().filter_map(|e| if let CoreEvent::EngineStatus(status) = e { status["state"].as_str().map(str::to_owned) } else { None }).collect()
}

#[test]
fn the_core_checks_for_and_installs_a_newer_yt_dlp_and_says_so() {
    let dir = scratch("engineupdate");
    let (core, rx) = start_with_release(&dir, "");
    assert_eq!(core.engine_status().state, "idle");
    assert_eq!(core.check_engine(false).latest, "2099.01.01");
    let done = core.update_engine();
    assert_eq!(done.state, "updated", "{:?}", done.message);
    assert_eq!(fs::read(dir.join("userData").join("bin").join("yt-dlp.exe")).unwrap(), b"MZ a newer yt-dlp");
    let seen: Vec<CoreEvent> = rx.try_iter().collect();
    assert_eq!(engine_states(&seen), ["checking", "available", "downloading", "updated"]);
    core.shutdown();
}

#[test]
fn a_download_that_fails_like_an_outdated_yt_dlp_updates_it_unless_that_is_switched_off() {
    let stale = "err ERROR: [youtube] abc: Unable to extract player response; please report this issue on https://github.com/yt-dlp/yt-dlp/issues , confirm you are on the latest version using yt-dlp -U\nexit 1\n";
    let updated = |e: &CoreEvent| matches!(e, CoreEvent::EngineStatus(status) if status["state"] == "updated");

    let dir = scratch("enginestale");
    let (core, rx) = start_with_release(&dir, stale);
    core.start_download(&request()).unwrap();
    wait_for(&rx, updated);
    assert!(dir.join("userData").join("bin").join("yt-dlp.exe").exists());
    core.shutdown();

    let dir = scratch("enginestaleoff");
    let (core, rx) = start_with_release(&dir, stale);
    core.update_settings(json!({ "autoUpdateEngine": false }).as_object().unwrap());
    core.start_download(&request()).unwrap();
    wait_for(&rx, |e| matches!(e, CoreEvent::DownloadFailed(_)));
    never_within(&rx, Duration::from_millis(800), |e| matches!(e, CoreEvent::EngineStatus(_)));
    assert!(!dir.join("userData").join("bin").join("yt-dlp.exe").exists());
    core.shutdown();
}

// ── The app's own updates, through the core ─────────────────────────────────

#[test]
fn the_core_finds_downloads_and_installs_a_newer_version_of_the_app() {
    const FEED: &str = "https://api.github.com/repos/amyyne01/UNIVERSAL-CONVERTER-/releases/latest";
    const EXE: &str = "https://github.com/amyyne01/UNIVERSAL-CONVERTER-/releases/download/v9.9.9/AHG-Converter.exe";
    const SUMS: &str = "https://github.com/amyyne01/UNIVERSAL-CONVERTER-/releases/download/v9.9.9/SHA256SUMS.txt";
    let new_exe = b"MZ version 9.9.9 of the app".to_vec();
    let feed = json!({ "tag_name": "v9.9.9", "assets": [{ "name": "AHG-Converter.exe", "browser_download_url": EXE }, { "name": "SHA256SUMS.txt", "browser_download_url": SUMS }] }).to_string();
    let net = common::Net::default().with(FEED, 200, feed.as_bytes()).with(EXE, 200, &new_exe).with(SUMS, 200, format!("{}  AHG-Converter.exe\n", common::sha(&new_exe)).as_bytes());

    let dir = scratch("appupdate");
    // Something an earlier run staged: it is dropped when the core starts.
    fs::create_dir_all(dir.join("userData").join("updates")).unwrap();
    fs::write(dir.join("userData").join("updates").join("stale.exe"), b"x").unwrap();
    fs::write(dir.join("script.txt"), "").unwrap();
    let (tx, rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let core = Core::start_with(paths, Arc::new(YtDlp::new(PathBuf::from(FAKE), None)), Box::new(|| Plan::Premium), tx, Outside { song_search: Arc::new(|_: &str| None), releases: Arc::new(net), desktop: Box::new(ahg_runtime::desktop::Windows), spotify: offline(), cover: Box::new(|_, _| None) });
    assert!(!dir.join("userData").join("updates").join("stale.exe").exists());

    assert_eq!(core.update_status().phase, "idle");
    assert_eq!(core.install_update(&dir.join("AHG-Converter.exe")).unwrap_err(), "Update refused: nothing downloaded to install");
    let found = core.check_update();
    assert_eq!((found.phase, found.version.as_deref()), ("available", Some("9.9.9")));
    assert_eq!(core.download_update().unwrap().phase, "ready");

    // The app's own EXE, standing in a scratch folder.
    let target = dir.join("AHG-Converter.exe");
    fs::write(&target, b"MZ the version that is running").unwrap();
    core.install_update(&target).unwrap();
    assert_eq!(fs::read(&target).unwrap(), new_exe);

    let channels: Vec<&str> = rx.try_iter().filter_map(|e| if let CoreEvent::Update { channel, .. } = e { Some(channel) } else { None }).collect();
    assert_eq!(channels.first(), Some(&"update:available"));
    assert_eq!(channels.last(), Some(&"update:downloaded"));
    assert!(channels.contains(&"update:progress"));
    core.shutdown();
}

// ── Showing and opening finished files ──────────────────────────────────────

/// A desktop that only writes down what it was asked to do.
#[derive(Clone, Default)]
struct Recorded(Arc<std::sync::Mutex<Vec<String>>>);

impl ahg_runtime::desktop::Desktop for Recorded {
    fn reveal(&self, path: &str) {
        self.0.lock().unwrap().push(format!("reveal {path}"));
    }

    fn open(&self, path: &str) -> bool {
        self.0.lock().unwrap().push(format!("open {path}"));
        !path.ends_with("broken.mp3")
    }
}

#[test]
fn only_media_inside_the_download_folder_is_shown_or_opened() {
    let dir = scratch("openreveal");
    fs::write(dir.join("script.txt"), "").unwrap();
    let desktop = Recorded::default();
    let (tx, _rx) = channel();
    let downloads = dir.join("Downloads");
    let paths = Paths { user_data: dir.join("userData"), downloads: downloads.to_string_lossy().into_owned() };
    let outside = Outside { song_search: Arc::new(|_: &str| None), releases: Arc::new(common::Net::default()), desktop: Box::new(desktop.clone()), spotify: offline(), cover: Box::new(|_, _| None) };
    let core = Core::start_with(paths, Arc::new(YtDlp::new(PathBuf::from(FAKE), None)), Box::new(|| Plan::Premium), tx, outside);
    let at = |name: &str| downloads.join(name).to_string_lossy().into_owned();
    for name in ["song.mp3", "broken.mp3", "tool.exe"] {
        fs::write(downloads.join(name), b"x").unwrap();
    }

    assert_eq!(core.show_in_folder(&at("song.mp3")), Ok(Shown::Ok));
    assert_eq!(core.open_file(&at("song.mp3")), Ok(Shown::Ok));
    // Gone since it was downloaded: said so, and Windows is not asked.
    assert_eq!(core.show_in_folder(&at("gone.mp3")), Ok(Shown::Missing));
    assert_eq!(core.open_file(&at("gone.mp3")), Ok(Shown::Missing));
    assert_eq!(core.open_file(""), Ok(Shown::NoPath));
    // Windows could not open it.
    assert_eq!(core.open_file(&at("broken.mp3")), Ok(Shown::Missing));
    // A program in the download folder may be shown, never run.
    assert_eq!(core.show_in_folder(&at("tool.exe")), Ok(Shown::Ok));
    assert!(core.open_file(&at("tool.exe")).is_err());
    // Outside the download folder: neither.
    let outside_file = dir.join("userData").join("config.json").to_string_lossy().into_owned();
    assert!(core.show_in_folder(&outside_file).is_err() && core.open_file(&outside_file).is_err());

    let asked = desktop.0.lock().unwrap().clone();
    assert_eq!(asked, [format!("reveal {}", at("song.mp3")), format!("open {}", at("song.mp3")), format!("open {}", at("broken.mp3")), format!("reveal {}", at("tool.exe"))]);
    core.shutdown();
}

// ── Importing a cookies file (`cookies:import`, ipc.test.ts :481-:501) ──────

const COOKIE_ROW: &str = ".youtube.com\tTRUE\t/\tTRUE\t1893456000\tSID\tsecret-value";

#[test]
fn a_cookies_file_is_copied_into_the_apps_data_and_used_for_downloads() {
    let dir = scratch("cookies");
    let (core, _rx) = start(&dir, "", Plan::Premium);
    assert!(core.may_import_cookies());
    let good = dir.join("c.txt");
    fs::write(&good, format!("# Netscape HTTP Cookie File\n{COOKIE_ROW}\n")).unwrap();
    assert_eq!(core.import_cookies(Some(&good)), CookiesImport::Ok);
    let saved = dir.join("userData").join("cookies.txt");
    assert!(fs::read_to_string(&saved).unwrap().contains(COOKIE_ROW));
    // A download now passes the file to yt-dlp.
    let task = core.start_download(&request()).unwrap();
    assert_eq!(task.extras.as_ref().and_then(|x| x.cookies_file.clone()).as_deref(), Some(saved.to_string_lossy().as_ref()));
    core.shutdown();
}

#[test]
fn a_file_that_is_not_a_cookies_file_is_refused_without_saying_why() {
    let dir = scratch("cookiesbad");
    let (core, _rx) = start(&dir, "", Plan::Premium);
    let not_cookies = dir.join("notes.txt");
    fs::write(&not_cookies, "just some notes\nnothing like cookies\n").unwrap();
    let huge = dir.join("huge.txt");
    fs::write(&huge, format!("# Netscape HTTP Cookie File\n{}", format!("{COOKIE_ROW}\n").repeat(40_000))).unwrap();
    assert!(fs::metadata(&huge).unwrap().len() > 2 * 1024 * 1024);
    for bad in [dir.join("nope.txt"), dir.clone(), not_cookies, huge] {
        assert_eq!(core.import_cookies(Some(&bad)), CookiesImport::Invalid, "{}", bad.display());
    }
    // The dialog was closed without choosing.
    assert_eq!(core.import_cookies(None), CookiesImport::Cancelled);
    assert!(!dir.join("userData").join("cookies.txt").exists());
    core.shutdown();
}

#[test]
fn the_basic_plan_cannot_import_cookies() {
    let dir = scratch("cookiesbasic");
    let (core, _rx) = start(&dir, "", Plan::Basic);
    assert!(!core.may_import_cookies(), "the interface offers the upgrade instead of a file dialog");
    let good = dir.join("c.txt");
    fs::write(&good, format!("# Netscape HTTP Cookie File\n{COOKIE_ROW}\n")).unwrap();
    assert_eq!(core.import_cookies(Some(&good)), CookiesImport::Premium);
    assert!(!dir.join("userData").join("cookies.txt").exists());
    core.shutdown();
}

// ── The queue's controls, through the core ──────────────────────────────────

/// End a stand-in's leftover child, so a test leaves nothing running.
fn kill_pid(pid: &str) {
    let _ = std::process::Command::new("taskkill").args(["/F", "/T", "/PID", pid]).output();
}

fn until(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !cond() {
        assert!(Instant::now() < deadline, "never happened: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn status_of(core: &Core, id: &str) -> DownloadStatus {
    core.tasks().into_iter().find(|t| t.task_id == id).unwrap().progress.status
}

fn one_at_a_time(core: &Core) {
    assert_eq!(core.update_settings(json!({ "maxConcurrentDownloads": 1 }).as_object().unwrap()), Vec::<String>::new());
}

/// The id of the next task the queue starts, ignoring those already seen.
fn next_started(rx: &Receiver<CoreEvent>, seen: &[&str]) -> String {
    let events = wait_for(rx, |e| matches!(e, CoreEvent::Download(Event::Progress { id, progress }) if progress.status == DownloadStatus::FetchingInfo && !seen.contains(&id.as_str())));
    match events.last() {
        Some(CoreEvent::Download(Event::Progress { id, .. })) => id.clone(),
        other => panic!("{other:?}"),
    }
}

/// A script that records its start, starts a child and stays busy for `busy` ms.
fn busy_script(dir: &Path, busy: u32) -> String {
    format!("child {}\ncount {}\nerr [PROGRESS]1|100|NA|1|9\nsleep {busy}\nexit 0\n", dir.join("child.pid").display(), dir.join("runs.txt").display())
}

#[test]
fn pausing_a_running_download_stops_its_process_and_resuming_starts_it_again() {
    let dir = scratch("pause");
    let (core, rx) = start(&dir, &busy_script(&dir, 1500), Plan::Premium);
    let id = core.start_download(&request()).unwrap().task_id;
    until("the first run", || runs(&dir.join("runs.txt")) == 1);
    let first = fs::read_to_string(dir.join("child.pid")).unwrap();
    assert!(pid_is_running(&first));

    assert!(core.pause(&id));
    assert_eq!(status_of(&core, &id), DownloadStatus::Paused);
    until("the first process is gone", || !pid_is_running(&first));
    assert_eq!(runs(&dir.join("runs.txt")), 1, "a paused download must not start again by itself");

    assert!(core.resume(&id));
    until("the second run", || runs(&dir.join("runs.txt")) == 2);
    let second = fs::read_to_string(dir.join("child.pid")).unwrap();
    assert_ne!(first, second, "a new process was started");
    // The stand-in's child holds yt-dlp's pipes open; ending it lets the exit be seen.
    kill_pid(&second);
    wait_for(&rx, done);
    assert_eq!(status_of(&core, &id), DownloadStatus::Done);
    core.shutdown();
}

#[test]
fn cancelling_a_waiting_task_and_a_running_task_ends_both_as_cancelled() {
    let dir = scratch("cancel");
    let (core, _rx) = start(&dir, &busy_script(&dir, 60_000), Plan::Premium);
    one_at_a_time(&core);
    let running = core.start_download(&request()).unwrap().task_id;
    let waiting = core.start_download(&request()).unwrap().task_id;
    until("the first run", || runs(&dir.join("runs.txt")) == 1);
    let pid = fs::read_to_string(dir.join("child.pid")).unwrap();

    assert!(core.cancel(&waiting));
    assert_eq!(status_of(&core, &waiting), DownloadStatus::Cancelled);
    assert_ne!(status_of(&core, &running), DownloadStatus::Cancelled, "the running one is untouched");
    assert!(!core.cancel(&waiting), "it has ended: nothing left to cancel");
    assert!(!core.cancel("no-such-task"));

    assert!(core.cancel(&running));
    assert_eq!(status_of(&core, &running), DownloadStatus::Cancelled);
    until("the process is gone", || !pid_is_running(&pid));
    assert_eq!(runs(&dir.join("runs.txt")), 1, "the cancelled waiting task never started");
    core.shutdown();
}

#[test]
fn promote_and_reorder_decide_which_waiting_task_starts_next() {
    let dir = scratch("order");
    let (core, rx) = start(&dir, "sleep 60000\n", Plan::Premium);
    one_at_a_time(&core);
    let a = core.start_download(&request()).unwrap().task_id;
    assert_eq!(next_started(&rx, &[]), a);
    let [b, c, d] = [(); 3].map(|()| core.start_download(&request()).unwrap().task_id);
    assert!(!core.promote(&a), "a running task is not waiting");

    // Waiting: b c d. "Do this next" puts d in front.
    assert!(core.promote(&d));
    assert!(core.cancel(&a));
    assert_eq!(next_started(&rx, &[&a]), d);

    // Waiting: b c. Naming c first moves it ahead of b.
    core.reorder(vec![c.clone(), b.clone()]);
    assert!(core.cancel(&d));
    assert_eq!(next_started(&rx, &[&a, &d]), c);
    assert!(core.cancel(&c));
    assert_eq!(next_started(&rx, &[&a, &d, &c]), b);
    core.shutdown();
}

#[test]
fn finished_rows_can_be_removed_but_a_running_one_stays() {
    let dir = scratch("remove");
    let (core, rx) = start(&dir, "exit 0\n", Plan::Premium);
    let finished = core.start_download(&request()).unwrap().task_id;
    wait_for(&rx, done);
    core.remove(vec![finished.clone()]);
    assert!(core.tasks().iter().all(|t| t.task_id != finished), "the finished row is gone");
    core.shutdown();

    let dir = scratch("remove-running");
    let (core, rx) = start(&dir, "err [PROGRESS]1|100|NA|1|9\nsleep 60000\n", Plan::Premium);
    let running = core.start_download(&request()).unwrap().task_id;
    wait_for(&rx, |e| matches!(e, CoreEvent::Download(Event::Progress { progress, .. }) if progress.status == DownloadStatus::Downloading));
    core.remove(vec![running.clone()]);
    assert_eq!(status_of(&core, &running), DownloadStatus::Downloading, "a running row is skipped");
    assert!(core.has_unfinished_work());
    core.shutdown();
}

#[test]
fn pause_all_resume_all_and_cancel_all_move_every_task_together() {
    let dir = scratch("all");
    let (core, _rx) = start(&dir, &busy_script(&dir, 60_000), Plan::Premium);
    one_at_a_time(&core);
    assert!(!core.has_unfinished_work(), "an empty queue has none");
    let running = core.start_download(&request()).unwrap().task_id;
    let waiting = core.start_download(&request()).unwrap().task_id;
    until("the first run", || runs(&dir.join("runs.txt")) == 1);
    assert!(core.has_unfinished_work());
    let first = fs::read_to_string(dir.join("child.pid")).unwrap();

    core.pause_all();
    assert_eq!(status_of(&core, &running), DownloadStatus::Paused);
    assert_eq!(status_of(&core, &waiting), DownloadStatus::Queued);
    assert!(core.has_unfinished_work(), "paused work is still work: the PC must not shut down");
    until("the process is gone", || !pid_is_running(&first));
    assert_eq!(runs(&dir.join("runs.txt")), 1, "nothing starts while paused");

    core.resume_all();
    until("the paused task runs again", || runs(&dir.join("runs.txt")) == 2);
    assert_eq!(status_of(&core, &waiting), DownloadStatus::Queued, "one slot: the other still waits");
    let second = fs::read_to_string(dir.join("child.pid")).unwrap();

    core.cancel_all();
    assert_eq!((status_of(&core, &running), status_of(&core, &waiting)), (DownloadStatus::Cancelled, DownloadStatus::Cancelled));
    assert!(!core.has_unfinished_work());
    until("the process is gone", || !pid_is_running(&second));
    core.shutdown();
}

#[test]
fn a_finished_download_leaves_no_unfinished_work() {
    let dir = scratch("unfinished");
    let (core, rx) = start(&dir, "exit 0\n", Plan::Premium);
    core.start_download(&request()).unwrap();
    wait_for(&rx, done);
    until("the queue is idle", || !core.has_unfinished_work());
    core.shutdown();
}

#[test]
fn the_searches_pass_yt_dlp_the_arguments_electron_does() {
    let dir = scratch("search");
    let (core, _rx) = start(&dir, "out {\"id\":\"a\",\"title\":\"A\"}\nexit 0\n", Plan::Premium);
    // runJsonOnce puts a socket timeout and retries in front of every lookup.
    let args = |rest: &str| format!("--socket-timeout\n20\n--retries\n3\n--dump-json\n--no-warnings\n--flat-playlist\n{rest}");
    let seen = || fs::read_to_string(dir.join("args.txt.json")).unwrap();
    core.search_youtube("lofi beats").unwrap();
    assert_eq!(seen(), args("ytsearch12:lofi beats"));
    core.search_soundcloud("lofi beats").unwrap();
    assert_eq!(seen(), args("scsearch12:lofi beats"));
    core.fetch_metadata("https://youtu.be/dQw4w9WgXcQ").unwrap();
    assert_eq!(seen(), args("--no-playlist\n--\nhttps://youtu.be/dQw4w9WgXcQ"));
    core.shutdown();
}

#[test]
fn a_link_that_looks_like_an_option_is_refused_before_any_process_starts() {
    let dir = scratch("optionlink");
    let (core, _rx) = start(&dir, "out {}\nexit 0\n", Plan::Premium);
    assert_eq!(core.fetch_metadata("--rm-rf").unwrap_err(), "Invalid input: URL must not start with \"-\"");
    assert!(!dir.join("args.txt").exists() && !dir.join("args.txt.json").exists(), "yt-dlp must not have been started");
    core.shutdown();
}

// ── Stopping lookups, and leaving no process at quit ────────────────────────

/// A core whose lookups (`--dump-json`) play `json_script` and whose downloads
/// play `script`. The song search finds nothing, so yt-dlp matches a Spotify track.
fn start_split(dir: &Path, json_script: &str, script: &str) -> (Core, Receiver<CoreEvent>) {
    fs::write(dir.join("json.txt"), json_script).unwrap();
    fs::write(dir.join("script.txt"), script).unwrap();
    let mut ytdlp = YtDlp::new(PathBuf::from(FAKE), None);
    ytdlp.env("FAKE_YTDLP_SCRIPT_JSON", dir.join("json.txt").to_str().unwrap());
    ytdlp.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
    let search: ahg_runtime::download::SongSearch = Arc::new(|_: &str| None);
    let (tx, rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    (Core::start_with_search(paths, Arc::new(ytdlp), Box::new(|| Plan::Premium), tx, search), rx)
}

fn spotify_request() -> Value {
    let track = json!({ "id": "t1", "name": "The Song", "artist": "Artist", "artists": ["Artist"], "album": "", "trackNumber": 1, "durationMs": 200000, "thumbnailUrl": "" });
    json!({ "url": "https://open.spotify.com/track/t1", "source": "spotify", "isAudioOnly": true, "format": "mp3", "quality": "128", "track": track })
}

/// The process ids a stand-in's `pid` lines wrote.
#[cfg(windows)]
fn pids_in(file: &Path) -> Vec<String> {
    fs::read_to_string(file).map_or_else(|_| Vec::new(), |text| text.lines().map(str::to_owned).collect())
}

#[test]
fn cancel_lookups_stops_a_hung_lookup_but_not_a_downloads_own_search() {
    let dir = scratch("cancellookups");
    let media = dir.join("Downloads").join("a.mp3");
    fs::write(&media, vec![0u8; 2048]).unwrap();
    let (count, record) = (dir.join("count"), r#"{"id":"abc00000001","track":"The Song","artist":"Artist","duration":200}"#);
    // Every lookup answers after 2.5 s: the interface's, and the bridge's two.
    let (core, rx) = start_split(&dir, &format!("count {}\nsleep 2500\nout {record}\nexit 0\n", count.display()), &format!("out [FILEPATH]{}\nexit 0\n", media.display()));
    let core = Arc::new(core);
    let id = core.start_download(&spotify_request()).unwrap().task_id;
    until("the bridge's search started", || runs(&count) == 1);
    let lookup = std::thread::spawn({
        let core = core.clone();
        move || core.fetch_metadata("https://youtu.be/dQw4w9WgXcQ")
    });
    until("the interface's lookup started", || runs(&count) == 2);

    let cancelled = Instant::now();
    core.cancel_lookups();
    assert_eq!(lookup.join().unwrap().unwrap_err(), CANCELLED_MESSAGE);
    assert!(cancelled.elapsed() < Duration::from_secs(2), "{:?}", cancelled.elapsed());
    // The download's own search ran on and found the song.
    wait_for(&rx, done);
    assert_eq!(status_of(&core, &id), DownloadStatus::Done);
    assert!(runs(&count) >= 3, "the bridge looked the song up after the other lookup was stopped");
    // Only lookups running at that moment are stopped, not later ones.
    assert!(core.fetch_metadata("https://youtu.be/dQw4w9WgXcQ").is_ok());
}

#[cfg(windows)]
#[test]
fn cancelling_a_spotify_download_that_is_still_searching_ends_its_search() {
    let dir = scratch("cancelsearching");
    let pids = dir.join("pids");
    let (core, _rx) = start_split(&dir, &format!("pid {}\nsleep 60000\n", pids.display()), "exit 0\n");
    let id = core.start_download(&spotify_request()).unwrap().task_id;
    until("the search started", || pids_in(&pids).len() == 1);
    let pid = pids_in(&pids).remove(0);
    assert!(pid_is_running(&pid));

    let cancelled = Instant::now();
    assert!(core.cancel(&id));
    until("the search process is gone", || !pid_is_running(&pid));
    assert!(cancelled.elapsed() < Duration::from_secs(2), "{:?}", cancelled.elapsed());
    assert_eq!(status_of(&core, &id), DownloadStatus::Cancelled);
    kill_pid(&pid);
    // Shutdown waits for the bridge to end, so a second search could not appear later.
    core.shutdown();
    assert_eq!(pids_in(&pids), [pid], "the bridge started no further search");
}

#[test]
fn cancel_all_returns_at_once_even_while_a_search_cannot_be_stopped() {
    let dir = scratch("cancelallprompt");
    fs::write(dir.join("script.txt"), "exit 0\n").unwrap();
    fs::write(dir.join("json.txt"), "").unwrap();
    let mut ytdlp = YtDlp::new(PathBuf::from(FAKE), None);
    ytdlp.env("FAKE_YTDLP_SCRIPT_JSON", dir.join("json.txt").to_str().unwrap());
    ytdlp.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
    // The song search stands in for the HTTP call, which no flag can stop: it holds the bridge until released.
    let (entered, entered_rx) = channel::<()>();
    let (release, release_rx) = channel::<()>();
    let (entered, release_rx) = (std::sync::Mutex::new(entered), std::sync::Mutex::new(release_rx));
    let search: ahg_runtime::download::SongSearch = Arc::new(move |_: &str| {
        let _ = entered.lock().unwrap().send(());
        let _ = release_rx.lock().unwrap().recv();
        None
    });
    let (tx, _rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let core = Core::start_with_search(paths, Arc::new(ytdlp), Box::new(|| Plan::Premium), tx, search);
    let id = core.start_download(&spotify_request()).unwrap().task_id;
    entered_rx.recv_timeout(WAIT).expect("the bridge never reached the song search");

    // The queue's thread answers `tasks` only after it has carried out the cancel.
    let cancelled = Instant::now();
    core.cancel_all();
    assert_eq!(status_of(&core, &id), DownloadStatus::Cancelled);
    assert!(cancelled.elapsed() < Duration::from_secs(2), "{:?}", cancelled.elapsed());
    let _ = release.send(());
    core.shutdown();
}

#[cfg(windows)]
#[test]
fn quitting_with_four_downloads_a_search_and_a_hung_lookup_leaves_no_process() {
    let dir = scratch("quit");
    let (downloads, lookups) = (dir.join("downloads.pids"), dir.join("lookups.pids"));
    // Three YouTube downloads and one Spotify track still searching, which fill four slots.
    let (core, _rx) = start_split(&dir, &format!("pid {}\nsleep 60000\n", lookups.display()), &format!("pid {}\nerr [PROGRESS]1|100|NA|1|9\nsleep 60000\n", downloads.display()));
    core.update_settings(json!({ "maxConcurrentDownloads": 4 }).as_object().unwrap());
    for _ in 0..3 {
        core.start_download(&request()).unwrap();
    }
    core.start_download(&spotify_request()).unwrap();
    let core = Arc::new(core);
    let lookup = std::thread::spawn({
        let core = core.clone();
        move || core.fetch_metadata("https://youtu.be/dQw4w9WgXcQ")
    });
    until("every process started", || pids_in(&downloads).len() == 3 && pids_in(&lookups).len() == 2);

    // The interface stops its lookups, then lets go of the core: that is quitting.
    let quitting = Instant::now();
    core.cancel_lookups();
    assert_eq!(lookup.join().unwrap().unwrap_err(), CANCELLED_MESSAGE);
    let Ok(core) = Arc::try_unwrap(core) else { panic!("the lookup has ended, so nothing else holds the core") };
    core.shutdown();
    assert!(quitting.elapsed() < Duration::from_secs(8), "{:?}", quitting.elapsed());
    for pid in pids_in(&downloads).into_iter().chain(pids_in(&lookups)) {
        if pid_is_running(&pid) {
            kill_pid(&pid);
            panic!("process {pid} was still running after shutdown");
        }
    }
}

// ── Spotify through the core ────────────────────────────────────────────────

type Seen = Arc<std::sync::Mutex<Vec<String>>>;

/// Spotify's side, scripted: writes down every address asked for and answers
/// from `pages`. No network.
struct Scripted {
    seen: Seen,
    pages: fn(&str) -> Option<String>,
}

impl ahg_engine::spotify::Transport for Scripted {
    fn fetch(&mut self, req: &ahg_engine::spotify::Request) -> Option<String> {
        self.seen.lock().unwrap().push(req.url.clone());
        (self.pages)(&req.url)
    }
}

/// An embed page that mints a (made-up) token and carries `entity`.
fn embed(entity: Value) -> String {
    let data = json!({ "props": { "pageProps": { "state": { "data": { "session": { "accessToken": "made-up" }, "entity": entity } } } } });
    format!(r#"<script id="__NEXT_DATA__" type="application/json">{data}</script>"#)
}

/// Spotify's embed pages for one track and one playlist; everything else (the
/// GraphQL API, the home page) answers nothing.
fn spotify_pages(url: &str) -> Option<String> {
    if url.contains("/embed/track/") {
        Some(embed(json!({ "uri": "spotify:track:t1", "name": "Song", "artists": [{ "name": "Artist" }], "duration": 215000, "album": { "name": "Album" } })))
    } else if url.contains("/embed/playlist/") {
        let list = json!([{ "uri": "spotify:track:a1", "title": "One", "subtitle": "Artist A", "duration": 1000 }, { "uri": "spotify:track:a2", "title": "Two", "subtitle": "Artist B", "duration": 2000 }]);
        Some(embed(json!({ "uri": "spotify:playlist:p1", "name": "Mix", "trackList": list })))
    } else {
        None
    }
}

fn offline() -> Box<dyn ahg_engine::spotify::Transport + Send> {
    Box::new(Scripted { seen: Seen::default(), pages: |_| None })
}

fn start_spotify(name: &str) -> (Core, Seen) {
    let dir = scratch(name);
    let seen = Seen::default();
    let (tx, _rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let spotify = Box::new(Scripted { seen: seen.clone(), pages: spotify_pages });
    let outside = Outside { song_search: Arc::new(|_: &str| None), releases: Arc::new(common::Net::default()), desktop: Box::new(ahg_runtime::desktop::Windows), spotify, cover: Box::new(|_, _| None) };
    (Core::start_with(paths, Arc::new(YtDlp::new(PathBuf::from(FAKE), None)), Box::new(|| Plan::Premium), tx, outside), seen)
}

#[test]
fn the_core_reads_a_spotify_track() {
    let (core, _) = start_spotify("spotifytrack");
    let track = core.spotify_track("https://open.spotify.com/track/t1").unwrap().unwrap();
    assert_eq!((track.id.as_str(), track.name.as_str(), track.artist.as_str(), track.album.as_str(), track.duration_ms), ("t1", "Song", "Artist", "Album", 215000.0));
}

#[test]
fn the_core_reads_a_spotify_playlist() {
    let (core, _) = start_spotify("spotifyplaylist");
    let playlist = core.spotify_playlist("https://open.spotify.com/playlist/p1").unwrap().unwrap();
    assert_eq!((playlist.name.as_str(), playlist.track_count), ("Mix", 2.0));
    let tracks: Vec<_> = playlist.tracks.iter().map(|t| (t.id.as_str(), t.name.as_str(), t.artist.as_str())).collect();
    assert_eq!(tracks, [("a1", "One", "Artist A"), ("a2", "Two", "Artist B")]);
}

#[test]
fn an_id_spotify_cannot_read_gives_nothing_and_asks_nothing() {
    let (core, seen) = start_spotify("spotifybadid");
    assert_eq!(core.spotify_track("not a link!").unwrap(), None);
    assert_eq!(core.spotify_playlist("").unwrap(), None);
    assert_eq!(core.spotify_album("https://example.com/x y").unwrap(), None);
    assert!(seen.lock().unwrap().is_empty());
}

/// As in Electron, whose search answered nothing on 2026-11-15 (QUIRKS Q6): the
/// call is made faithfully (a token, then the search query) and gives nothing.
#[test]
fn spotify_search_makes_the_faithful_call_and_the_stand_in_gives_nothing() {
    let (core, seen) = start_spotify("spotifysearch");
    assert!(core.spotify_search("artist song").is_empty());
    let asked = seen.lock().unwrap().clone();
    assert!(asked[0].contains("/embed/track/7tFiyTwD0nx5a1eklYtX2J"), "{asked:?}");
    assert!(asked.iter().any(|u| u.contains("operationName=searchDesktop")), "{asked:?}");
}

#[test]
fn two_lookups_at_once_share_one_heal() {
    let (core, seen) = start_spotify("spotifyheal");
    let start = std::sync::Barrier::new(2);
    std::thread::scope(|s| {
        for _ in 0..2 {
            s.spawn(|| {
                start.wait();
                assert!(core.spotify_playlist("p1").unwrap().is_some());
            });
        }
    });
    // The GraphQL answer is missing both times, which asks for a heal: the
    // home page is read once, and the second lookup finds the cooldown running.
    let homes = seen.lock().unwrap().iter().filter(|u| u.as_str() == ahg_engine::spotify::HOME).count();
    assert_eq!(homes, 1);
}

const GONE: &str = "err ERROR: Video unavailable\nexit 1\n";

fn failed(e: &CoreEvent) -> bool {
    matches!(e, CoreEvent::DownloadFailed(_))
}

#[test]
fn retrying_a_failed_task_gives_a_new_id_and_removes_the_old_row() {
    let dir = scratch("retry-failed");
    let (core, rx) = start(&dir, GONE, Plan::Premium);
    let old = core.start_download(&request()).unwrap().task_id;
    wait_for(&rx, failed);
    assert_eq!(status_of(&core, &old), DownloadStatus::Failed);

    let new = core.retry(&old, None).unwrap();
    assert_ne!(new.task_id, old);
    assert_eq!(new.url, request()["url"]);
    wait_for(&rx, failed);
    until("the old row is gone", || core.task(&old).is_none());
    assert!(core.task(&new.task_id).is_some());
    assert_eq!(core.tasks().len(), 1);
    core.shutdown();
}

#[test]
fn a_refused_retry_keeps_the_old_row() {
    let dir = scratch("retry-refused");
    let row = json!([{ "taskId": "task_old", "url": "-rf", "progress": { "status": "failed" } }]);
    fs::write(dir.join("userData").join("downloads-history.json"), row.to_string()).unwrap();
    let (core, _rx) = start(&dir, GONE, Plan::Premium);
    assert_eq!(status_of(&core, "task_old"), DownloadStatus::Failed);

    assert!(core.retry("task_old", None).is_err());
    assert_eq!(core.tasks().len(), 1);
    assert_eq!(status_of(&core, "task_old"), DownloadStatus::Failed);
    core.shutdown();
}

#[test]
fn a_cancelled_task_can_be_retried() {
    let dir = scratch("retry-cancelled");
    let (core, _rx) = start(&dir, &busy_script(&dir, 60_000), Plan::Premium);
    one_at_a_time(&core);
    let running = core.start_download(&request()).unwrap().task_id;
    let waiting = core.start_download(&request()).unwrap().task_id;
    until("the first run", || runs(&dir.join("runs.txt")) == 1);
    let pid = fs::read_to_string(dir.join("child.pid")).unwrap();
    assert!(core.cancel(&waiting));

    let new = core.retry(&waiting, None).unwrap();
    assert_ne!(new.task_id, waiting);
    until("the cancelled row is gone", || core.task(&waiting).is_none());
    assert!(core.cancel(&running));
    until("the process is gone", || !pid_is_running(&pid));
    core.shutdown();
}

#[test]
fn a_running_task_and_an_unknown_id_cannot_be_retried() {
    let dir = scratch("retry-running");
    let (core, _rx) = start(&dir, &busy_script(&dir, 60_000), Plan::Premium);
    let running = core.start_download(&request()).unwrap().task_id;
    until("the first run", || runs(&dir.join("runs.txt")) == 1);
    let pid = fs::read_to_string(dir.join("child.pid")).unwrap();

    assert!(core.retry(&running, None).is_err());
    assert!(core.retry("no-such-task", None).is_err());
    assert_eq!(core.tasks().len(), 1);
    assert!(core.cancel(&running));
    until("the process is gone", || !pid_is_running(&pid));
    core.shutdown();
}

#[test]
fn retry_due_is_the_automatic_retrys_deadline_and_none_otherwise() {
    let dir = scratch("retry-due");
    let (core, rx) = start(&dir, "err ERROR: unable to download video data: The read operation timed out\nexit 1\n", Plan::Premium);
    let id = core.start_download(&request()).unwrap().task_id;
    assert_eq!(core.retry_due("no-such-task"), None);
    until("the first failure is waiting out its delay", || core.retry_due(&id).is_some());
    let due = core.retry_due(&id).unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    assert!(due > now && due <= now + 5000, "due {due} now {now}");
    drop(rx);
    core.shutdown();
}

// ── Startup checks, input limits and honest saves ───────────────────────────

#[test]
fn the_startup_delays_are_electrons() {
    use ahg_runtime::core::{APP_UPDATE_DELAY, ENGINE_UPDATE_DELAY, WARM_UP_DELAY};
    assert_eq!((WARM_UP_DELAY, APP_UPDATE_DELAY, ENGINE_UPDATE_DELAY), (Duration::from_millis(1500), Duration::from_secs(6), Duration::from_secs(8)));
}

#[test]
fn warm_up_resolves_the_tools_without_starting_a_download() {
    let dir = scratch("warmup");
    let (core, _rx) = start(&dir, "", Plan::Premium);
    core.warm_up();
    assert!(core.tasks().is_empty());
    core.shutdown();
}

#[test]
fn auto_update_engine_installs_the_newer_yt_dlp_when_on_and_asks_nothing_when_off() {
    let new_exe = |dir: &Path| dir.join("userData").join("bin").join("yt-dlp.exe");

    let dir = scratch("autoengine-on");
    let (core, rx) = start_with_release(&dir, "");
    core.update_settings(json!({ "autoUpdateEngine": true }).as_object().unwrap());
    core.auto_update_engine();
    assert_eq!(fs::read(new_exe(&dir)).unwrap(), b"MZ a newer yt-dlp");
    assert_eq!(engine_states(&rx.try_iter().collect::<Vec<_>>()), ["checking", "available", "downloading", "updated"]);
    core.shutdown();

    let dir = scratch("autoengine-off");
    let (core, rx) = start_with_release(&dir, "");
    core.update_settings(json!({ "autoUpdateEngine": false }).as_object().unwrap());
    core.auto_update_engine();
    assert!(!new_exe(&dir).exists());
    assert!(engine_states(&rx.try_iter().collect::<Vec<_>>()).is_empty(), "no request means no status change");
    assert_eq!(core.engine_status().state, "idle");
    core.shutdown();
}

#[test]
fn a_search_query_is_cut_to_256_characters_before_yt_dlp_sees_it() {
    let dir = scratch("longquery");
    let (core, _rx) = start(&dir, "out {\"id\":\"a\",\"title\":\"A\"}\nexit 0\n", Plan::Premium);
    let seen = || fs::read_to_string(dir.join("args.txt.json")).unwrap();
    core.search_youtube(&"a".repeat(5000)).unwrap();
    assert!(seen().ends_with(&format!("ytsearch12:{}", "a".repeat(256))));
    // JavaScript counts UTF-16 units: 300 emoji are 600, and 128 of them are 256.
    core.search_soundcloud(&"😀".repeat(300)).unwrap();
    assert!(seen().ends_with(&format!("scsearch12:{}", "😀".repeat(128))));
    core.shutdown();
}

#[test]
fn over_long_ids_and_too_many_ids_change_nothing() {
    let dir = scratch("limits");
    let (core, rx) = start(&dir, "sleep 60000\n", Plan::Premium);
    one_at_a_time(&core);
    let a = core.start_download(&request()).unwrap().task_id;
    assert_eq!(next_started(&rx, &[]), a);
    let [b, c, d] = [(); 3].map(|()| core.start_download(&request()).unwrap().task_id);

    // 501 ids, the first three would put d first; refused as a whole.
    let mut many = vec![d.clone(), c.clone(), b.clone()];
    many.extend((0..498).map(|i| format!("x{i}")));
    assert_eq!(many.len(), 501);
    core.reorder(many);
    // One id of 65 characters refuses the whole list too.
    core.reorder(vec![d.clone(), "x".repeat(65)]);
    assert!(!core.promote(&format!("{d}{}", "x".repeat(64))));
    assert!(!core.promote(&"x".repeat(65)));
    assert!(!core.cancel(&"x".repeat(4097)));
    assert!(!core.pause(&"x".repeat(4097)));
    assert!(!core.resume(&"x".repeat(4097)));
    core.remove(vec![b.clone(), "x".repeat(4097)]);
    assert!(core.tasks().iter().any(|t| t.task_id == b), "one over-long id refuses the whole removal");

    assert!(core.cancel(&a));
    assert_eq!(next_started(&rx, &[&a]), b, "the order is as it was");
    assert!(core.cancel(&c) && core.cancel(&d) && core.cancel(&b));
    core.shutdown();
}

#[test]
fn a_save_answers_whether_the_file_was_written() {
    use ahg_engine::downloader::bridge::SongMatch;
    let dir = scratch("honestsave");
    let user = dir.join("userData");
    let chosen = SongMatch { url: "https://www.youtube.com/watch?v=mine0000001".into(), title: "T".into(), artist: "A".into(), album: String::new(), duration: Some(1.0), explicit: false, confidence: 0.4 };
    let (core, _rx) = start(&dir, "", Plan::Premium);
    assert!(core.set_hold_unconfirmed(true));
    assert!(core.remember_match("t1", &chosen));
    assert!(core.forget_match("t1"));
    assert!(core.forget_match("never-remembered"), "nothing to save is not a failed save");
    assert!(core.remember_match("t1", &chosen));

    // A folder where each file should be: the atomic write cannot replace it.
    fs::remove_file(user.join("settings-rust.json")).unwrap();
    fs::create_dir(user.join("settings-rust.json")).unwrap();
    fs::remove_file(user.join("spotify-matches.json")).unwrap();
    fs::create_dir(user.join("spotify-matches.json")).unwrap();
    assert!(!core.set_hold_unconfirmed(false));
    assert!(!core.remember_match("t2", &chosen));
    assert!(!core.forget_match("t1"));
    core.shutdown();
}

// ── Ready for the interface: shutdown through an Arc, own settings, covers ──

#[test]
fn shutdown_through_an_arc_returns_quickly_and_a_second_call_does_nothing() {
    let dir = scratch("arcshutdown");
    let (core, _rx) = start(&dir, "", Plan::Premium);
    let core = Arc::new(core);
    let other = core.clone();
    let began = Instant::now();
    core.shutdown();
    assert!(began.elapsed() < Duration::from_secs(8), "{:?}", began.elapsed());

    // A change made after the first call would be saved by a second one that did anything.
    let config = dir.join("userData").join("config.json");
    let before = fs::read_to_string(&config).unwrap_or_default();
    core.update_settings(json!({ "maxConcurrentDownloads": 3 }).as_object().unwrap());
    let again = Instant::now();
    other.shutdown();
    assert!(again.elapsed() < Duration::from_millis(500), "{:?}", again.elapsed());
    assert_eq!(fs::read_to_string(&config).unwrap_or_default(), before);
}

#[test]
fn shutdown_stops_a_lookup_that_is_still_running() {
    let dir = scratch("shutdownlookup");
    let count = dir.join("count");
    let (core, _rx) = start_split(&dir, &format!("count {}\nsleep 60000\n", count.display()), "");
    let core = Arc::new(core);
    let lookup = std::thread::spawn({
        let core = core.clone();
        move || core.fetch_metadata("https://youtu.be/dQw4w9WgXcQ")
    });
    until("the lookup started", || runs(&count) == 1);
    let began = Instant::now();
    core.shutdown();
    assert_eq!(lookup.join().unwrap().unwrap_err(), CANCELLED_MESSAGE);
    assert!(began.elapsed() < Duration::from_secs(8), "{:?}", began.elapsed());
}

fn own_file(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("userData").join("settings-rust.json")).unwrap()).unwrap()
}

#[test]
fn two_own_settings_both_survive_a_restart() {
    let dir = scratch("ownsettings");
    let (core, _rx) = start(&dir, "", Plan::Premium);
    assert_eq!(core.own_setting("preset"), None);
    assert!(core.set_own_setting("preset", json!("orbit")));
    assert!(core.set_own_setting("sidebarPinned", json!(true)));
    core.shutdown();

    let (core, _rx) = start(&dir, "", Plan::Premium);
    assert_eq!(core.own_setting("preset"), Some(json!("orbit")));
    assert_eq!(core.own_setting("sidebarPinned"), Some(json!(true)));
    core.shutdown();
}

#[test]
fn needs_review_keeps_the_other_keys_of_the_file() {
    let dir = scratch("holdkeepsothers");
    fs::write(dir.join("userData").join("settings-rust.json"), r#"{"preset":"shelf","mystery":{"a":1}}"#).unwrap();
    let (core, _rx) = start(&dir, "", Plan::Premium);
    assert!(core.set_hold_unconfirmed(true));
    assert!(core.set_own_setting("sidebarPinned", json!(false)));
    core.shutdown();
    assert_eq!(own_file(&dir), json!({ "preset": "shelf", "mystery": { "a": 1 }, "holdUnconfirmed": true, "sidebarPinned": false }));

    let (core, _rx) = start(&dir, "", Plan::Premium);
    assert!(core.hold_unconfirmed());
    core.shutdown();
}

type Asked = Arc<std::sync::Mutex<Vec<(String, Option<String>)>>>;

/// A core whose cover transport answers with `body`, and writes down every
/// (address, proxy) it was asked for. `/missing` is a 404, `/downgrade` ends on http.
fn start_cover(name: &str, body: Vec<u8>) -> (Core, Asked) {
    let dir = scratch(name);
    let asked = Asked::default();
    let cover: ahg_runtime::core::CoverFetch = Box::new({
        let asked = asked.clone();
        move |url: &str, proxy: Option<&str>| {
            asked.lock().unwrap().push((url.to_owned(), proxy.map(str::to_owned)));
            let status = if url.ends_with("/missing") { 404 } else { 200 };
            let final_url = if url.ends_with("/downgrade") { "http://cdn.example/a.jpg".to_owned() } else { url.to_owned() };
            Some(ahg_runtime::engine_update::Fetched { status, final_url, content_length: Some(body.len().to_string()), location: None, body: Box::new(std::io::Cursor::new(body.clone())) })
        }
    });
    let (tx, _rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let outside = Outside { cover, ..Outside::standin() };
    (Core::start_with(paths, Arc::new(YtDlp::new(PathBuf::from(FAKE), None)), Box::new(|| Plan::Premium), tx, outside), asked)
}

#[test]
fn a_cover_comes_back_as_its_bytes() {
    let (core, asked) = start_cover("cover", b"jpeg-bytes".to_vec());
    assert_eq!(core.fetch_cover("https://i.example/a.jpg").unwrap(), b"jpeg-bytes");
    assert_eq!(asked.lock().unwrap().len(), 1);
    core.shutdown();
}

#[test]
fn a_cover_over_five_mib_is_refused_and_five_mib_is_not() {
    const MIB: usize = 1024 * 1024;
    let (core, _) = start_cover("coverbig", vec![7u8; 6 * MIB]);
    assert!(core.fetch_cover("https://i.example/a.jpg").is_err());
    core.shutdown();
    let (core, _) = start_cover("coveredge", vec![7u8; 5 * MIB]);
    assert_eq!(core.fetch_cover("https://i.example/a.jpg").unwrap().len(), 5 * MIB);
    core.shutdown();
}

#[test]
fn only_https_covers_are_fetched_and_a_bad_answer_is_an_error() {
    let (core, asked) = start_cover("coverrefused", b"x".to_vec());
    for url in ["http://i.example/a.jpg", "file:///c:/a.jpg", "ftp://i.example/a.jpg", "", "i.example/a.jpg", "https:///a.jpg", "https://127.0.0.1/a.jpg", "https://[::1]/a.jpg", "https://localhost/a.jpg", "https://u@i.example/a.jpg"] {
        assert!(core.fetch_cover(url).is_err(), "{url}");
    }
    assert!(asked.lock().unwrap().is_empty(), "a refused address is never requested");
    assert!(core.fetch_cover("https://i.example/missing").is_err());
    assert!(core.fetch_cover("https://i.example/downgrade").is_err(), "an answer from plain http is refused");
    core.shutdown();
}

#[test]
fn a_changed_proxy_reaches_the_next_cover_request() {
    let (core, asked) = start_cover("coverproxy", b"x".to_vec());
    core.fetch_cover("https://i.example/a.jpg").unwrap();
    core.update_settings(json!({ "proxy": "socks5h://127.0.0.1:9050" }).as_object().unwrap());
    core.fetch_cover("https://i.example/a.jpg").unwrap();
    let asked = asked.lock().unwrap();
    assert_eq!(asked[0].1, None);
    assert_eq!(asked[1].1.as_deref(), Some("socks5h://127.0.0.1:9050"));
    core.shutdown();
}

#[test]
fn two_threads_shutting_down_at_once_both_return_after_the_settings_are_saved() {
    let dir = scratch("twoshutdowns");
    let (core, _rx) = start(&dir, "", Plan::Premium);
    let core = Arc::new(core);
    core.update_settings(json!({ "maxConcurrentDownloads": 3 }).as_object().unwrap());
    let config = dir.join("userData").join("config.json");
    let both: Vec<_> = (0..2)
        .map(|_| {
            let (core, config) = (core.clone(), config.clone());
            std::thread::spawn(move || {
                core.shutdown();
                fs::read_to_string(config).unwrap_or_default().contains("maxConcurrentDownloads")
            })
        })
        .collect();
    for caller in both {
        assert!(caller.join().unwrap(), "a caller came back before the settings were saved");
    }
}

#[test]
fn a_lookup_started_after_shutdown_is_cancelled_and_starts_no_process() {
    let dir = scratch("lookupafter");
    let count = dir.join("count");
    let (core, _rx) = start_split(&dir, &format!("count {}\nsleep 60000\n", count.display()), "");
    core.shutdown();
    let began = Instant::now();
    assert_eq!(core.fetch_metadata("https://youtu.be/dQw4w9WgXcQ").unwrap_err(), CANCELLED_MESSAGE);
    assert!(began.elapsed() < Duration::from_secs(2), "{:?}", began.elapsed());
    assert_eq!(runs(&count), 0, "no process was started");
    assert!(core.start_download(&request()).is_err(), "no download is queued after shutdown");
}

/// A core whose cover transport answers from `routes` (address, status, `Location`);
/// an address not listed is a 200 with the bytes `cover`. Every address asked for is written down.
fn start_hops(name: &str, routes: &'static [(&'static str, u16, &'static str)]) -> (Core, Asked) {
    let dir = scratch(name);
    let asked = Asked::default();
    let cover: ahg_runtime::core::CoverFetch = Box::new({
        let asked = asked.clone();
        move |url: &str, proxy: Option<&str>| {
            asked.lock().unwrap().push((url.to_owned(), proxy.map(str::to_owned)));
            let (status, location) = routes.iter().find(|r| r.0 == url).map_or((200, None), |r| (r.1, Some(r.2.to_owned())));
            let body = if status == 200 { b"cover".to_vec() } else { Vec::new() };
            Some(ahg_runtime::engine_update::Fetched { status, final_url: url.to_owned(), content_length: None, location, body: Box::new(std::io::Cursor::new(body)) })
        }
    });
    let (tx, _rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let outside = Outside { cover, ..Outside::standin() };
    (Core::start_with(paths, Arc::new(YtDlp::new(PathBuf::from(FAKE), None)), Box::new(|| Plan::Premium), tx, outside), asked)
}

#[test]
fn a_redirect_to_an_address_that_is_not_allowed_is_refused_before_it_is_requested() {
    const ROUTES: &[(&str, u16, &str)] = &[
        ("https://i.example/ip", 302, "https://127.0.0.1/x"),
        ("https://i.example/localhost", 301, "https://localhost/x"),
        ("https://i.example/http", 307, "http://i.example/x"),
        ("https://i.example/userinfo", 308, "https://u@i.example/x"),
        ("https://i.example/v6", 303, "https://[::1]/x"),
        ("https://i.example/slashes", 302, "//127.0.0.1/x"),
        ("https://i.example/nowhere", 302, ""),
    ];
    assert!(!ROUTES.is_empty());
    for (from, _, to) in ROUTES {
        let (core, asked) = start_hops("coverbadhop", ROUTES);
        assert!(core.fetch_cover(from).is_err(), "{from} -> {to}");
        assert_eq!(asked.lock().unwrap().len(), 1, "the second request was never made: {from} -> {to}");
        core.shutdown();
    }
}

#[test]
fn a_relative_redirect_to_an_allowed_host_is_followed() {
    const ROUTES: &[(&str, u16, &str)] = &[
        ("https://i.example/a/b.jpg", 302, "c.jpg"),
        ("https://i.example/a/c.jpg", 301, "/d/e.jpg"),
        ("https://i.example/d/e.jpg", 308, "//cdn.example/f.jpg"),
    ];
    let (core, asked) = start_hops("coverrelative", ROUTES);
    assert_eq!(core.fetch_cover("https://i.example/a/b.jpg").unwrap(), b"cover");
    let seen: Vec<String> = asked.lock().unwrap().iter().map(|a| a.0.clone()).collect();
    assert_eq!(seen, ["https://i.example/a/b.jpg", "https://i.example/a/c.jpg", "https://i.example/d/e.jpg", "https://cdn.example/f.jpg"]);
    core.shutdown();
}

#[test]
fn a_fourth_redirect_is_refused_and_three_are_followed() {
    const ROUTES: &[(&str, u16, &str)] = &[
        ("https://i.example/1", 302, "https://i.example/2"),
        ("https://i.example/2", 302, "https://i.example/3"),
        ("https://i.example/3", 302, "https://i.example/4"),
        ("https://i.example/4", 302, "https://i.example/5"),
    ];
    assert!(!ROUTES.is_empty());
    let (core, asked) = start_hops("coverchain", ROUTES);
    let err = core.fetch_cover("https://i.example/1").unwrap_err();
    assert!(err.contains("too many redirects"), "{err}");
    assert_eq!(asked.lock().unwrap().len(), 4, "the fourth redirect is never followed");
    assert_eq!(core.fetch_cover("https://i.example/2").unwrap(), b"cover", "three redirects are followed");
    core.shutdown();
}
