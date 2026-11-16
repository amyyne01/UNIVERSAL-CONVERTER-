//! Finding the tools, reading metadata and search results, and the Spotify
//! bridge, against the scripted stand-in for yt-dlp. Nothing touches the network.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use ahg_engine::queue::{TaskInit, create_task};
use ahg_engine::types::{SourcePlatform, Track};
use ahg_runtime::download::{Downloader, Report};
use ahg_runtime::ytdlp::{CANCELLED_MESSAGE, TIMEOUT_MESSAGE, Tools, YtDlp};

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
        explicit: None,
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

/// D10, an approved difference from Electron 1.6.0: the proxy from Settings
/// also covers link lookups, search and the Spotify bridge's search. Electron
/// passes it to the download only.
#[test]
fn the_proxy_also_covers_lookups_and_searches() {
    let dir = scratch("proxy");
    let y = ytdlp(&dir, "out {\"id\":\"a\",\"title\":\"A\"}\nexit 0\n", "");
    let args_of = |dir: &Path| fs::read_to_string(dir.join("args.txt.json")).unwrap();

    y.fetch_metadata("https://youtu.be/dQw4w9WgXcQ").unwrap();
    assert!(!args_of(&dir).contains("--proxy"), "no proxy is set yet");

    y.set_proxy(Some("socks5h://127.0.0.1:9050"));
    y.fetch_metadata("https://youtu.be/dQw4w9WgXcQ").unwrap();
    assert!(args_of(&dir).starts_with("--socket-timeout\n20\n--retries\n3\n--proxy\nsocks5h://127.0.0.1:9050\n--dump-json"), "{}", args_of(&dir));
    y.search_youtube("some song").unwrap();
    assert!(args_of(&dir).contains("--proxy\nsocks5h://127.0.0.1:9050\n"));

    // An empty setting means no proxy.
    y.set_proxy(Some(""));
    y.search_soundcloud("some song").unwrap();
    assert!(!args_of(&dir).contains("--proxy"));
}

/// A shipped build never falls back to whatever `yt-dlp` is on the system path.
#[test]
fn a_shipped_build_does_not_run_a_yt_dlp_found_on_the_system_path() {
    let dir = scratch("nopath");
    // The folder holding the stand-in is put on the path of a strict search that has no tool folders.
    let tools = Tools::strict(vec![dir.clone()]);
    assert_eq!(tools.ytdlp(), None, "nothing in the app's folders means no yt-dlp, whatever the system has");
}

// ── The Spotify bridge with YouTube Music's song search (D34) ───────────────

/// What the stand-in plays for the download itself.
const DOWNLOADS: &str = "out [FILEPATH]C:\\Music\\a.mp3\nexit 0\n";

/// A song-search response holding these rows: (video id, title, "Artist • Album • m:ss").
fn song_search(rows: &[(&str, &str, &str)]) -> String {
    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|(id, title, second)| {
            let column = |text: &str| serde_json::json!({ "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": text }] } } });
            serde_json::json!({ "musicResponsiveListItemRenderer": { "flexColumns": [column(title), column(second)], "playlistItemData": { "videoId": id } } })
        })
        .collect();
    serde_json::json!({ "contents": { "musicShelfRenderer": { "contents": items } } }).to_string()
}

/// Runs the Spotify task with `search` answering the song search (`None`: it is unreachable).
fn run_song_bridge(y: YtDlp, search: Option<String>) -> (Vec<Report>, Vec<String>) {
    let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut d = Downloader::with(Arc::new(y));
    let seen = asked.clone();
    d.set_song_search(Arc::new(move |query: &str| {
        seen.lock().unwrap().push(query.to_owned());
        search.clone()
    }));
    let (tx, rx) = channel();
    d.start(spotify_task(), 3, tx);
    let mut reports = Vec::new();
    while let Ok(report) = rx.recv_timeout(Duration::from_secs(20)) {
        let ended = matches!(report, Report::Done { .. } | Report::Error { .. });
        reports.push(report);
        if ended {
            break;
        }
    }
    let asked = asked.lock().unwrap().clone();
    (reports, asked)
}

fn the_match(reports: &[Report]) -> Option<(String, f64)> {
    reports.iter().find_map(|r| if let Report::Match { url, confidence, .. } = r { Some((url.clone(), *confidence)) } else { None })
}

#[test]
fn a_song_that_agrees_in_artist_name_and_length_is_downloaded_without_any_lookup() {
    let dir = scratch("song");
    let search = song_search(&[
        ("live0000001", "The Song (Live)", "Artist • Live Album • 4:10"),
        ("other000001", "The Song", "Someone Else • Album • 3:20"),
        ("right000001", "The Song", "Artist • Album • 3:21"),
    ]);
    let (reports, asked) = run_song_bridge(ytdlp(&dir, "exit 1\n", DOWNLOADS), Some(search));
    assert_eq!(asked, ["Artist - The Song"]);
    assert_eq!(the_match(&reports), Some(("https://music.youtube.com/watch?v=right000001".to_owned(), 1.0)));
    assert!(matches!(reports.last().unwrap(), Report::Done { .. }));
    assert!(fs::read_to_string(dir.join("args.txt")).unwrap().ends_with("https://music.youtube.com/watch?v=right000001"));
    assert!(!dir.join("args.txt.json").exists(), "a row that agrees needs no yt-dlp search or lookup");
}

#[test]
fn when_no_song_agrees_the_plain_search_is_scored_and_never_counts_as_confirmed() {
    let dir = scratch("songnone");
    let search = song_search(&[("other000001", "Another Song", "Someone Else • Album • 3:20")]);
    let candidates = "out {\"id\":\"topic000001\",\"title\":\"The Song\",\"uploader\":\"Artist - Topic\",\"duration\":200,\"url\":\"https://www.youtube.com/watch?v=topic000001\"}\nexit 0\n";
    let (reports, _) = run_song_bridge(ytdlp(&dir, candidates, DOWNLOADS), Some(search));
    let (url, confidence) = the_match(&reports).expect("the scored match is reported");
    assert_eq!(url, "https://www.youtube.com/watch?v=topic000001");
    assert!(confidence < 1.0, "a scored guess is not a confirmed match: {confidence}");
    assert!(fs::read_to_string(dir.join("args.txt.json")).unwrap().ends_with("ytsearch5:Artist - The Song"));
}

#[test]
fn when_the_song_search_is_unreachable_yt_dlp_finds_the_song_and_a_lookup_confirms_it() {
    let dir = scratch("songbackup");
    // The stand-in answers the song search and the lookup alike: a song with its music metadata.
    let song = "out {\"id\":\"right000001\",\"title\":\"The Song\",\"track\":\"The Song\",\"artist\":\"Artist\",\"duration\":201}\nexit 0\n";
    let (reports, _) = run_song_bridge(ytdlp(&dir, song, DOWNLOADS), None);
    assert_eq!(the_match(&reports), Some(("https://music.youtube.com/watch?v=right000001".to_owned(), 1.0)));
    assert!(fs::read_to_string(dir.join("args.txt")).unwrap().ends_with("https://music.youtube.com/watch?v=right000001"));
}

#[test]
fn a_song_the_lookup_contradicts_is_not_downloaded_as_a_match() {
    let dir = scratch("songwrong");
    // Same title, but four minutes longer: a different version.
    let song = "out {\"id\":\"long0000001\",\"title\":\"The Song\",\"track\":\"The Song\",\"artist\":\"Artist\",\"duration\":440,\"uploader\":\"Fan\",\"url\":\"https://www.youtube.com/watch?v=long0000001\"}\nexit 0\n";
    let (reports, _) = run_song_bridge(ytdlp(&dir, song, DOWNLOADS), None);
    assert!(the_match(&reports).is_none_or(|(url, _)| !url.contains("long0000001")), "{reports:?}");
}

/// A song row carrying YouTube Music's explicit badge.
fn with_badge(response: &str, id: &str) -> String {
    let mut v: serde_json::Value = serde_json::from_str(response).unwrap();
    for item in v["contents"]["musicShelfRenderer"]["contents"].as_array_mut().unwrap() {
        if item["musicResponsiveListItemRenderer"]["playlistItemData"]["videoId"] == id {
            item["musicResponsiveListItemRenderer"]["badges"] = serde_json::json!([{ "musicInlineBadgeRenderer": { "icon": { "iconType": "MUSIC_EXPLICIT_BADGE" } } }]);
        }
    }
    v.to_string()
}

fn run_explicit_bridge(dir: &Path, search: String) -> Vec<Report> {
    let mut d = Downloader::with(Arc::new(ytdlp(dir, "exit 1\n", DOWNLOADS)));
    d.set_song_search(Arc::new(move |_: &str| Some(search.clone())));
    let mut task = spotify_task();
    task.track.as_mut().unwrap().explicit = Some(true);
    let (tx, rx) = channel();
    d.start(task, 3, tx);
    let mut reports = Vec::new();
    while let Ok(report) = rx.recv_timeout(Duration::from_secs(20)) {
        let ended = matches!(report, Report::Done { .. } | Report::Error { .. });
        reports.push(report);
        if ended {
            break;
        }
    }
    reports
}

#[test]
fn an_explicit_track_takes_the_explicit_song_over_a_clean_one_listed_first() {
    let dir = scratch("explicit");
    let rows = song_search(&[("clean000001", "The Song", "Artist • Album • 3:20"), ("dirty000001", "The Song", "Artist • Album • 3:20")]);
    let reports = run_explicit_bridge(&dir, with_badge(&rows, "dirty000001"));
    assert_eq!(the_match(&reports), Some(("https://music.youtube.com/watch?v=dirty000001".to_owned(), 1.0)));
}

#[test]
fn an_explicit_track_with_only_a_clean_song_is_a_best_guess_not_a_confirmed_match() {
    let dir = scratch("explicitonly");
    let reports = run_explicit_bridge(&dir, song_search(&[("clean000001", "The Song", "Artist • Album • 3:20")]));
    let (url, confidence) = the_match(&reports).unwrap();
    assert_eq!(url, "https://music.youtube.com/watch?v=clean000001");
    assert!(confidence < 1.0, "a different explicit flag is not a confirmed match: {confidence}");
}

#[test]
fn a_match_already_chosen_is_downloaded_without_searching_again() {
    let dir = scratch("chosen");
    let asked = Arc::new(std::sync::Mutex::new(0));
    let mut d = Downloader::with(Arc::new(ytdlp(&dir, "exit 1\n", DOWNLOADS)));
    let count = asked.clone();
    d.set_song_search(Arc::new(move |_: &str| {
        *count.lock().unwrap() += 1;
        None
    }));
    let mut task = spotify_task();
    task.matched_url = Some("https://music.youtube.com/watch?v=chosen00001".into());
    task.match_confidence = Some(0.9);
    let (tx, rx) = channel();
    d.start(task, 3, tx);
    let mut reports = Vec::new();
    while let Ok(report) = rx.recv_timeout(Duration::from_secs(20)) {
        let ended = matches!(report, Report::Done { .. } | Report::Error { .. });
        reports.push(report);
        if ended {
            break;
        }
    }
    assert!(matches!(reports.last().unwrap(), Report::Done { .. }), "{reports:?}");
    assert_eq!(*asked.lock().unwrap(), 0, "the chosen match needs no search");
    assert!(!dir.join("args.txt.json").exists(), "and no yt-dlp search either");
    assert!(fs::read_to_string(dir.join("args.txt")).unwrap().ends_with("https://music.youtube.com/watch?v=chosen00001"));
    assert_eq!(the_match(&reports), Some(("https://music.youtube.com/watch?v=chosen00001".to_owned(), 0.9)));
}

// ── "Needs review": nothing but a confirmed match is downloaded ─────────────

/// Runs the Spotify task with unconfirmed matches held back. Returns the
/// reports and how many times the download itself started.
fn run_held(dir: &Path, search: Option<String>, explicit: Option<bool>) -> (Vec<Report>, usize) {
    let ran = dir.join("ran");
    let candidates = "out {\"id\":\"topic000001\",\"title\":\"The Song\",\"uploader\":\"Artist - Topic\",\"duration\":200,\"url\":\"https://www.youtube.com/watch?v=topic000001\"}\nexit 0\n";
    let mut d = Downloader::with(Arc::new(ytdlp(dir, candidates, &format!("count {}\n{DOWNLOADS}", ran.display()))));
    d.set_song_search(Arc::new(move |_: &str| search.clone()));
    d.set_hold_unconfirmed(Arc::new(std::sync::atomic::AtomicBool::new(true)));
    let mut task = spotify_task();
    task.track.as_mut().unwrap().explicit = explicit;
    let (tx, rx) = channel();
    d.start(task, 3, tx);
    let mut reports = Vec::new();
    while let Ok(report) = rx.recv_timeout(Duration::from_secs(20)) {
        let ended = matches!(report, Report::Done { .. } | Report::Error { .. });
        reports.push(report);
        if ended {
            break;
        }
    }
    (reports, runs(&ran))
}

fn refused(reports: &[Report]) -> bool {
    matches!(reports.last(), Some(Report::Error { message, .. }) if message == ahg_engine::downloader::bridge::NO_CONFIRMED_MATCH)
}

#[test]
fn needs_review_downloads_a_confirmed_match() {
    let dir = scratch("heldok");
    let (reports, ran) = run_held(&dir, Some(song_search(&[("right000001", "The Song", "Artist • Album • 3:20")])), None);
    assert!(matches!(reports.last().unwrap(), Report::Done { .. }), "{reports:?}");
    assert_eq!(ran, 1);
}

#[test]
fn needs_review_holds_a_best_guess_and_still_says_which_song_it_was() {
    let dir = scratch("heldguess");
    // The track is explicit and the only song is the clean one.
    let (reports, ran) = run_held(&dir, Some(song_search(&[("clean000001", "The Song", "Artist • Album • 3:20")])), Some(true));
    assert!(refused(&reports), "{reports:?}");
    assert_eq!(ran, 0, "nothing may be downloaded");
    assert_eq!(the_match(&reports).map(|(url, _)| url).as_deref(), Some("https://music.youtube.com/watch?v=clean000001"), "the guess is shown so it can be accepted or swapped");
}

#[test]
fn needs_review_holds_a_scored_guess_and_a_track_with_no_match_at_all() {
    let dir = scratch("heldscored");
    let nothing_agrees = song_search(&[("other000001", "Another Song", "Someone Else • Album • 3:20")]);
    let (reports, ran) = run_held(&dir, Some(nothing_agrees), None);
    assert!(refused(&reports) && ran == 0, "{reports:?}");
    assert!(the_match(&reports).is_some_and(|(url, confidence)| url.contains("topic000001") && confidence < 1.0));
}

#[test]
fn needs_review_downloads_a_match_the_user_accepted() {
    let dir = scratch("heldaccepted");
    let ran = dir.join("ran");
    let mut d = Downloader::with(Arc::new(ytdlp(&dir, "exit 1\n", &format!("count {}\n{DOWNLOADS}", ran.display()))));
    d.set_song_search(Arc::new(|_: &str| None));
    d.set_hold_unconfirmed(Arc::new(std::sync::atomic::AtomicBool::new(true)));
    let mut task = spotify_task();
    task.matched_url = Some("https://music.youtube.com/watch?v=chosen00001".into());
    task.match_confidence = Some(1.0);
    let (tx, rx) = channel();
    d.start(task, 3, tx);
    let last = std::iter::from_fn(|| rx.recv_timeout(Duration::from_secs(20)).ok()).find(|r| matches!(r, Report::Done { .. } | Report::Error { .. }));
    assert!(matches!(last, Some(Report::Done { .. })), "{last:?}");
    assert_eq!(runs(&ran), 1);
}

// ── Nothing is downloaded blind, and a weak guess is no guess ───────────────

fn no_match_found(reports: &[Report]) -> bool {
    matches!(reports.last(), Some(Report::Error { message, .. }) if message == ahg_engine::downloader::bridge::NO_MATCH_FOUND)
}

#[test]
fn with_no_song_and_no_usable_video_nothing_is_downloaded() {
    let dir = scratch("nomatch");
    let ran = dir.join("ran");
    let nothing_agrees = song_search(&[("other000001", "Another Song", "Someone Else • Album • 3:20")]);
    // The plain search fails too. Electron would download the first hit of a blind search here.
    let (reports, _) = run_song_bridge(ytdlp(&dir, "err ERROR: Video unavailable\nexit 1\n", &format!("count {}\n{DOWNLOADS}", ran.display())), Some(nothing_agrees));
    assert!(no_match_found(&reports), "{reports:?}");
    assert!(the_match(&reports).is_none());
    assert_eq!(runs(&ran), 0, "nothing may be downloaded");
}

#[test]
fn a_video_that_barely_resembles_the_track_is_not_taken_as_a_guess() {
    let dir = scratch("weakguess");
    let ran = dir.join("ran");
    let nothing_agrees = song_search(&[("other000001", "Another Song", "Someone Else • Album • 3:20")]);
    // One word in common with "Artist - The Song", right length, a stranger's upload.
    let weak = "out {\"id\":\"weak0000001\",\"title\":\"Best song mix 2024\",\"uploader\":\"Fan\",\"duration\":200,\"url\":\"https://www.youtube.com/watch?v=weak0000001\"}\nexit 0\n";
    let (reports, _) = run_song_bridge(ytdlp(&dir, weak, &format!("count {}\n{DOWNLOADS}", ran.display())), Some(nothing_agrees));
    assert!(no_match_found(&reports), "{reports:?}");
    assert_eq!(runs(&ran), 0);
}

// ── The finished file's length is checked against Spotify's ─────────────────

/// Runs the Spotify task (a 200-second track) to a confirmed match; the
/// stand-in also plays ffmpeg, reporting `length` for the finished file.
fn run_with_file_length(dir: &Path, length: &str, extra_setup: impl FnOnce(&mut ahg_engine::types::DownloadTask)) -> Vec<Report> {
    let script = format!("err   Duration: {length}, start: 0.000000, bitrate: 128 kb/s\n{DOWNLOADS}");
    fs::write(dir.join("script.txt"), script).unwrap();
    fs::write(dir.join("json.txt"), "exit 1\n").unwrap();
    let mut y = YtDlp::new(PathBuf::from(FAKE), Some(FAKE.to_owned()));
    y.env("FAKE_YTDLP_SCRIPT_JSON", dir.join("json.txt").to_str().unwrap());
    y.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
    y.env("FAKE_YTDLP_ARGS", dir.join("args.txt").to_str().unwrap());
    let mut d = Downloader::with(Arc::new(y));
    let search = song_search(&[("right000001", "The Song", "Artist • Album • 3:20")]);
    d.set_song_search(Arc::new(move |_: &str| Some(search.clone())));
    let mut task = spotify_task();
    extra_setup(&mut task);
    let (tx, rx) = channel();
    d.start(task, 3, tx);
    let mut reports = Vec::new();
    while let Ok(report) = rx.recv_timeout(Duration::from_secs(20)) {
        let ended = matches!(report, Report::Done { .. } | Report::Error { .. });
        reports.push(report);
        if ended {
            break;
        }
    }
    reports
}

fn confidences(reports: &[Report]) -> Vec<f64> {
    reports.iter().filter_map(|r| if let Report::Match { confidence, .. } = r { Some(*confidence) } else { None }).collect()
}

#[test]
fn a_file_of_the_right_length_stays_a_confirmed_match() {
    let dir = scratch("lengthok");
    let reports = run_with_file_length(&dir, "00:03:21.40", |_| {});
    assert!(matches!(reports.last().unwrap(), Report::Done { .. }), "{reports:?}");
    assert_eq!(confidences(&reports), [1.0]);
}

#[test]
fn a_file_of_the_wrong_length_is_kept_but_no_longer_counts_as_confirmed() {
    let dir = scratch("lengthoff");
    // The search said 3:20 and the track is 200 s, but what was saved runs 7:20.
    let reports = run_with_file_length(&dir, "00:07:20.00", |_| {});
    assert!(matches!(reports.last().unwrap(), Report::Done { .. }), "the file is kept: {reports:?}");
    let seen = confidences(&reports);
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert!(seen[0] == 1.0 && seen[1] < 1.0, "{seen:?}");
    // The lowered confidence arrives before the download is reported as done.
    assert!(matches!(reports[reports.len() - 2], Report::Match { .. }) || reports.iter().rposition(|r| matches!(r, Report::Match { .. })) < reports.iter().rposition(|r| matches!(r, Report::Done { .. })));
}

#[test]
fn a_length_that_cannot_be_read_changes_nothing() {
    let dir = scratch("lengthunknown");
    assert_eq!(confidences(&run_with_file_length(&dir, "N/A", |_| {})), [1.0]);
}

#[test]
fn removing_sponsor_segments_shortens_the_file_so_its_length_is_not_judged() {
    use ahg_engine::types::{PartialExtras, SponsorBlockMode};
    let dir = scratch("lengthsponsor");
    let reports = run_with_file_length(&dir, "00:02:00.00", |task| {
        task.extras = Some(PartialExtras { sponsor_block: Some(SponsorBlockMode::Remove), sponsor_block_categories: Some(vec!["sponsor".into()]), ..Default::default() });
    });
    assert_eq!(confidences(&reports), [1.0]);
}

// ── Stopping a lookup ────────────────────────────────────────────────────────

fn pid_is_running(pid: &str) -> bool {
    let out = std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).contains(pid)
}

#[test]
fn cancelling_a_search_ends_its_process_within_two_seconds_and_is_not_retried() {
    let dir = scratch("cancelsearch");
    let (pids, count) = (dir.join("pids"), dir.join("count"));
    let y = Arc::new(ytdlp(&dir, &format!("count {}\npid {}\nsleep 60000\n", count.display(), pids.display()), ""));
    let flag = Arc::new(AtomicBool::new(false));
    let worker = std::thread::spawn({
        let (y, flag) = (y.clone(), flag.clone());
        move || y.run_json_until(&["--dump-json", "x"], &flag)
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while fs::read_to_string(&pids).is_ok_and(|t| !t.ends_with('\n')) || !pids.exists() {
        assert!(Instant::now() < deadline, "the search never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    let pid = fs::read_to_string(&pids).unwrap().trim().to_owned();
    assert!(pid_is_running(&pid), "the search should be running before the cancel");

    let started = Instant::now();
    flag.store(true, Ordering::SeqCst);
    assert_eq!(worker.join().unwrap().unwrap_err(), CANCELLED_MESSAGE);
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    assert!(!pid_is_running(&pid), "the search process must be gone");
    assert_eq!(runs(&count), 1, "a cancelled search is not tried again");
    assert_eq!(y.run_json_until(&["--dump-json", "x"], &flag).unwrap_err(), CANCELLED_MESSAGE, "a flag already set starts nothing");
    assert_eq!(runs(&count), 1);
}
