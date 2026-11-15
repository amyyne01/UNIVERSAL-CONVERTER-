//! Finding the tools, reading metadata and search results, and the Spotify
//! bridge, against the scripted stand-in for yt-dlp. Nothing touches the network.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use ahg_engine::queue::{TaskInit, create_task};
use ahg_engine::types::{SourcePlatform, Track};
use ahg_runtime::download::{Downloader, Report};
use ahg_runtime::ytdlp::{TIMEOUT_MESSAGE, Tools, YtDlp};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_ytdlp");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ahg-yt-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A yt-dlp whose metadata calls play `json_script` and whose downloads play `script`.
fn ytdlp(dir: &Path, json_script: &str, script: &str) -> YtDlp {
    fs::write(dir.join("json.txt"), json_script).unwrap();
    fs::write(dir.join("script.txt"), script).unwrap();
    let mut y = YtDlp::new(PathBuf::from(FAKE), None);
    y.env("FAKE_YTDLP_SCRIPT_JSON", dir.join("json.txt").to_str().unwrap());
    y.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
    y.env("FAKE_YTDLP_ARGS", dir.join("args.txt").to_str().unwrap());
    y
}

fn runs(file: &Path) -> usize {
    fs::read_to_string(file).map(|t| t.lines().count()).unwrap_or(0)
}

// ── Finding the tools ────────────────────────────────────────────────────────

#[test]
fn the_first_folder_holding_a_working_yt_dlp_wins() {
    let dir = scratch("tools");
    let (empty, second, third) = (dir.join("empty"), dir.join("second"), dir.join("third"));
    for d in [&empty, &second, &third] {
        fs::create_dir_all(d).unwrap();
    }
    fs::write(second.join("yt-dlp.exe"), "not a program").unwrap(); // exists, but does not run
    fs::copy(FAKE, third.join("yt-dlp.exe")).unwrap();
    fs::write(third.join("ffmpeg.exe"), "").unwrap();

    let tools = Tools::new(vec![empty, second, third.clone()]);
    assert_eq!(tools.ytdlp(), Some(third.join("yt-dlp.exe")));
    assert_eq!(tools.ffmpeg(), Some(third.join("ffmpeg.exe").to_string_lossy().into_owned()));
    assert_eq!(tools.ytdlp_version(), "2026.01.01");
}

#[test]
fn ffmpeg_is_optional_and_a_missing_yt_dlp_is_reported_as_missing() {
    let dir = scratch("notools");
    let tools = Tools::new(vec![dir.clone()]);
    assert_eq!(tools.ffmpeg(), None);
    // Nothing in the folder; the bare `yt-dlp` on PATH is the last resort and is not installed here.
    if tools.ytdlp().is_none() {
        assert_eq!(tools.ytdlp_version(), "");
    }
}

#[test]
fn a_replaced_yt_dlp_is_found_after_the_cache_is_reset() {
    let dir = scratch("reset");
    let (old, new) = (dir.join("old"), dir.join("new"));
    fs::create_dir_all(&old).unwrap();
    fs::create_dir_all(&new).unwrap();
    fs::copy(FAKE, old.join("yt-dlp.exe")).unwrap();
    let tools = Tools::new(vec![new.clone(), old.clone()]);
    assert_eq!(tools.ytdlp(), Some(old.join("yt-dlp.exe")));
    fs::copy(FAKE, new.join("yt-dlp.exe")).unwrap();
    assert_eq!(tools.ytdlp(), Some(old.join("yt-dlp.exe")), "the answer is cached");
    tools.reset_cache();
    assert_eq!(tools.ytdlp(), Some(new.join("yt-dlp.exe")));
}

// ── Metadata and search ──────────────────────────────────────────────────────

#[test]
fn records_are_read_one_per_line_and_other_output_is_ignored() {
    let dir = scratch("json");
    let y = ytdlp(&dir, "out WARNING: noise\nout {\"id\":\"a\",\"title\":\"A\"}\nout\nout {\"id\":\"b\"}\nexit 0\n", "");
    let items = y.run_json(&["--dump-json", "x"]).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["title"], "A");
    let args = fs::read_to_string(dir.join("args.txt.json")).unwrap();
    assert!(args.starts_with("--socket-timeout\n20\n--retries\n3\n--dump-json"), "{args}");
}

#[test]
fn a_failure_with_no_output_is_the_error_text_and_one_with_output_still_counts() {
    let dir = scratch("jsonfail");
    let y = ytdlp(&dir, "err ERROR: Video unavailable\nexit 1\n", "");
    assert_eq!(y.run_json(&["--dump-json", "x"]).unwrap_err(), "ERROR: Video unavailable");
    let y = ytdlp(&dir, "exit 5\n", "");
    assert_eq!(y.run_json(&["--dump-json", "x"]).unwrap_err(), "yt-dlp exited with code 5");
    let y = ytdlp(&dir, "out {\"id\":\"a\"}\nerr ERROR: later item failed\nexit 1\n", "");
    assert_eq!(y.run_json(&["--dump-json", "x"]).unwrap().len(), 1);
}

#[test]
fn a_network_failure_is_tried_once_more_and_a_permanent_one_is_not() {
    let dir = scratch("retry");
    let (marker, count) = (dir.join("marker"), dir.join("count"));
    let y = ytdlp(&dir, &format!("count {}\nfailonce {}\nout {{\"id\":\"a\"}}\nexit 0\n", count.display(), marker.display()), "");
    assert_eq!(y.run_json(&["--dump-json", "x"]).unwrap().len(), 1);
    assert_eq!(runs(&count), 2, "one failure, one retry");

    let count = dir.join("count2");
    let y = ytdlp(&dir, &format!("count {}\nerr ERROR: Video unavailable\nexit 1\n", count.display()), "");
    assert!(y.run_json(&["--dump-json", "x"]).is_err());
    assert_eq!(runs(&count), 1, "a removed video answers the same twice; no retry");
}

#[test]
fn a_call_that_never_answers_is_stopped_and_not_retried() {
    let dir = scratch("watchdog");
    let count = dir.join("count");
    let mut y = ytdlp(&dir, &format!("count {}\nsleep 60000\n", count.display()), "");
    y.set_metadata_timeout(Duration::from_millis(400));
    let started = Instant::now();
    assert_eq!(y.run_json(&["--dump-json", "x"]).unwrap_err(), TIMEOUT_MESSAGE);
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(runs(&count), 1, "a timeout has already cost the user the full wait");
}

#[test]
fn metadata_for_a_single_video_asks_for_no_playlist_and_explains_a_failure() {
    let dir = scratch("meta");
    let record = r#"{"id":"dQw4w9WgXcQ","title":"Song","uploader":"Artist","duration":212,"webpage_url":"https://www.youtube.com/watch?v=dQw4w9WgXcQ"}"#;
    let y = ytdlp(&dir, &format!("out {record}\nexit 0\n"), "");
    let meta = y.fetch_metadata("https://youtu.be/dQw4w9WgXcQ").unwrap();
    assert_eq!((meta.title.as_str(), meta.duration, meta.is_collection), ("Song", 212.0, false));
    let args = fs::read_to_string(dir.join("args.txt.json")).unwrap();
    assert!(args.ends_with("--flat-playlist\n--no-playlist\n--\nhttps://youtu.be/dQw4w9WgXcQ"), "{args}");

    let y = ytdlp(&dir, "err ERROR: [youtube] x: Video unavailable\nexit 1\n", "");
    let error = y.fetch_metadata("https://youtu.be/dQw4w9WgXcQ").unwrap_err();
    assert!(!error.contains("[youtube]"), "the interface gets an explanation: {error}");

    let y = ytdlp(&dir, "exit 0\n", "");
    assert_eq!(y.fetch_metadata("https://youtu.be/dQw4w9WgXcQ").unwrap_err(), "That link returned nothing to download.");
}

#[test]
fn a_playlist_link_is_listed_whole() {
    let dir = scratch("playlist");
    let y = ytdlp(&dir, "out {\"id\":\"a\",\"title\":\"A\",\"playlist_title\":\"Mix\"}\nout {\"id\":\"b\",\"title\":\"B\"}\nexit 0\n", "");
    let meta = y.fetch_metadata("https://www.youtube.com/playlist?list=PL123").unwrap();
    assert!(meta.is_collection);
    assert_eq!((meta.title.as_str(), meta.entries.len()), ("Mix", 2));
    let args = fs::read_to_string(dir.join("args.txt.json")).unwrap();
    assert!(!args.contains("--no-playlist"), "{args}");
}

#[test]
fn searches_ask_for_twelve_results_on_the_right_site() {
    let dir = scratch("search");
    let y = ytdlp(&dir, "out {\"id\":\"a\",\"title\":\"A\",\"url\":\"https://youtu.be/a\"}\nexit 0\n", "");
    let results = y.search_youtube("some song").unwrap();
    assert_eq!((results.len(), results[0].source), (1, SourcePlatform::Youtube));
    assert!(fs::read_to_string(dir.join("args.txt.json")).unwrap().ends_with("ytsearch12:some song"));
    let results = y.search_soundcloud("some song").unwrap();
    assert_eq!(results[0].source, SourcePlatform::Soundcloud);
    assert!(fs::read_to_string(dir.join("args.txt.json")).unwrap().ends_with("scsearch12:some song"));
}

// ── The Spotify bridge ───────────────────────────────────────────────────────

fn spotify_task() -> ahg_engine::types::DownloadTask {
    let track = Track {
        id: "t1".into(),
        name: "The \"Song\"".into(),
        artist: "Artist".into(),
        artists: vec!["Artist".into()],
        album: "Album".into(),
        album_year: None,
        track_number: 1.0,
        duration_ms: 200_000.0,
        thumbnail_url: String::new(),
        spotify_url: None,
    };
    let init = TaskInit {
        url: Some("https://open.spotify.com/track/t1".into()),
        source: Some(SourcePlatform::Spotify),
        output_dir: Some("C:\\Music".into()),
        track: Some(track),
        ..Default::default()
    };
    create_task(&init, "sp".into(), 0)
}

fn run_bridge(y: YtDlp) -> Vec<Report> {
    let d = Downloader::with(Arc::new(y));
    let (tx, rx) = channel();
    d.start(spotify_task(), 3, tx);
    let mut seen = Vec::new();
    while let Ok(report) = rx.recv_timeout(Duration::from_secs(20)) {
        let ended = matches!(report, Report::Done { .. } | Report::Error { .. });
        seen.push(report);
        if ended {
            break;
        }
    }
    seen
}

#[test]
fn a_spotify_track_downloads_the_best_scored_search_result() {
    let dir = scratch("bridge");
    let candidates = concat!(
        "out {\"id\":\"live0000001\",\"title\":\"Artist - The Song (Live)\",\"uploader\":\"Fan\",\"duration\":200,\"url\":\"https://www.youtube.com/watch?v=live0000001\"}\n",
        "out {\"id\":\"topic000001\",\"title\":\"The Song\",\"uploader\":\"Artist - Topic\",\"duration\":200,\"url\":\"https://www.youtube.com/watch?v=topic000001\"}\n",
        "exit 0\n"
    );
    let reports = run_bridge(ytdlp(&dir, candidates, "out [FILEPATH]C:\\Music\\a.mp3\nexit 0\n"));
    let matched = reports.iter().find_map(|r| match r {
        Report::Match { id, attempt, url, confidence } => Some((id.clone(), *attempt, url.clone(), *confidence)),
        _ => None,
    });
    let (id, attempt, url, confidence) = matched.expect("the chosen match is reported");
    assert_eq!((id.as_str(), attempt, url.as_str()), ("sp", 3, "https://www.youtube.com/watch?v=topic000001"));
    assert!((0.0..=1.0).contains(&confidence));
    assert!(matches!(reports.last().unwrap(), Report::Done { .. }));
    // The search is for the track with quotes removed, and the download is of the chosen video.
    assert!(fs::read_to_string(dir.join("args.txt.json")).unwrap().ends_with("ytsearch5:Artist - The Song"));
    assert!(fs::read_to_string(dir.join("args.txt")).unwrap().ends_with("https://www.youtube.com/watch?v=topic000001"));
}

#[test]
fn when_the_search_fails_the_download_falls_back_to_the_first_search_hit() {
    let dir = scratch("bridgefail");
    let reports = run_bridge(ytdlp(&dir, "err ERROR: Video unavailable\nexit 1\n", "out [FILEPATH]C:\\Music\\a.mp3\nexit 0\n"));
    assert!(!reports.iter().any(|r| matches!(r, Report::Match { .. })));
    assert!(matches!(reports.last().unwrap(), Report::Done { .. }));
    assert!(fs::read_to_string(dir.join("args.txt")).unwrap().ends_with("ytsearch1:Artist - The Song"));
}

#[test]
fn a_cancel_during_the_search_calls_the_download_off() {
    let dir = scratch("bridgecancel");
    let ran = dir.join("ran");
    let y = ytdlp(&dir, "sleep 1500\nexit 0\n", &format!("count {}\nexit 0\n", ran.display()));
    let d = Downloader::with(Arc::new(y));
    let (tx, rx) = channel();
    d.start(spotify_task(), 1, tx);
    std::thread::sleep(Duration::from_millis(300));
    d.cancel("sp");
    assert!(rx.recv_timeout(Duration::from_secs(4)).is_err(), "a cancelled download reports nothing");
    assert_eq!(runs(&ran), 0, "the download itself must never have started");
}
