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
use ahg_runtime::core::{Core, CoreEvent, Paths, TrackMatch};
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
    use ahg_runtime::http::{Http, SpotifyHttp};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let (bin, ffmpeg) = (root.join("bin").join("yt-dlp.exe"), root.join("bin").join("ffmpeg.exe"));
    assert!(bin.exists() && ffmpeg.exists(), "bin/yt-dlp.exe and bin/ffmpeg.exe are needed");
    let dir = scratch("realspotify");
    let (tx, rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let core = Core::start(paths, Arc::new(YtDlp::new(bin, Some(ffmpeg.to_string_lossy().into_owned()))), Box::new(|| Plan::Premium), tx);

    // Fleetwood Mac, "Dreams".
    let mut spotify = Spotify::new(SpotifyHttp::new(Http::new(None)), Box::new(system_now_ms));
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
        Some(ahg_runtime::engine_update::Fetched { status: 200, final_url: request.url.clone(), body: Box::new(std::io::Cursor::new(body)) })
    }
}

fn start_with_release(dir: &Path, script: &str) -> (Core, Receiver<CoreEvent>) {
    fs::write(dir.join("script.txt"), script).unwrap();
    let mut ytdlp = YtDlp::new(PathBuf::from(FAKE), None);
    ytdlp.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
    let (tx, rx) = channel();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.join("Downloads").to_string_lossy().into_owned() };
    let core = Core::start_with(paths, Arc::new(ytdlp), Box::new(|| Plan::Premium), tx, Arc::new(|_: &str| None), Box::new(OneRelease { binary: b"MZ a newer yt-dlp".to_vec() }));
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
