//! The prepared downloads every direction of the design gate shows: one of
//! each kind the Downloads screen has to handle, with plausible numbers.
//! Real engine types, so the direction that wins needs no translation later.
use std::path::PathBuf;

use ahg_engine::queue::{TaskInit, create_task};
use ahg_engine::stats::{DownloadStats, MonthBucket};
use ahg_engine::types::{
    DownloadStatus, DownloadTask, ErrorCode, MediaMetadata, Playlist, SearchResult, SourcePlatform, Track, VideoQuality,
};
use serde_json::json;

const MIB: f64 = 1024.0 * 1024.0;
/// 15 November 2026, 14:00 UTC, in milliseconds.
pub const NOW: f64 = 1_794_751_200_000.0;
/// The samples' download folder: an invented user's.
pub const OUTPUT_DIR: &str = "C:\\Users\\Alex\\Music";
/// An invented machine fingerprint.
pub const MACHINE_ID: &str = "0f3a9c2e71b4d856";

/// The saved settings: Electron's defaults, saving into [`OUTPUT_DIR`].
pub fn settings() -> serde_json::Map<String, serde_json::Value> {
    let mut settings = ahg_engine::config::default_config();
    settings.insert("outputDir".into(), OUTPUT_DIR.into());
    settings
}

/// The download engine as Settings first shows it.
pub fn engine() -> ahg_engine::ytdlp_updater::YtdlpStatus {
    ahg_engine::ytdlp_updater::YtdlpStatus { current: "2026.10.22".into(), latest: String::new(), state: "idle", message: None }
}

/// The app updater before anything was checked.
pub fn update() -> ahg_engine::updater::UpdateState {
    ahg_engine::updater::UpdateState { phase: "idle", version: None, percent: None, message: None }
}

/// The last half minute of the speed trace, in bytes per second.
pub fn speed() -> std::collections::VecDeque<f32> {
    (0..30).map(|i| ((12.0 + 3.0 * (i as f64 * 0.7).sin()) * MIB) as f32).collect()
}

struct Sample {
    id: &'static str,
    source: SourcePlatform,
    title: &'static str,
    uploader: &'static str,
    /// "mp3", "flac", "mp4" ...
    format: &'static str,
    audio: bool,
    /// Seconds.
    duration: f64,
    minutes_ago: f64,
}

fn base(s: &Sample) -> DownloadTask {
    let init = TaskInit {
        url: Some(format!("https://example.invalid/{}", s.id)),
        source: Some(s.source),
        output_dir: Some(OUTPUT_DIR.into()),
        format: Some(s.format.into()),
        quality: Some("320".into()),
        video_quality: Some(VideoQuality::P1080),
        is_audio_only: Some(s.audio),
        title: Some(s.title.into()),
        uploader: Some(s.uploader.into()),
        duration: Some(s.duration),
        created_at: Some(NOW - s.minutes_ago * 60_000.0),
        ..Default::default()
    };
    create_task(&init, s.id.into(), (NOW - s.minutes_ago * 60_000.0) as u64)
}

fn spotify(mut task: DownloadTask, name: &str, artist: &str, album: &str, confidence: f64) -> DownloadTask {
    task.track = Some(Track {
        id: task.task_id.clone(),
        name: name.into(),
        artist: artist.into(),
        artists: vec![artist.into()],
        album: album.into(),
        album_year: None,
        track_number: 1.0,
        duration_ms: task.duration * 1000.0,
        thumbnail_url: String::new(),
        spotify_url: None,
        explicit: None,
    });
    task.match_confidence = Some(confidence);
    task.matched_url = Some("https://music.youtube.com/watch?v=swJOIjjW69U".into());
    task
}

fn running(mut task: DownloadTask, status: DownloadStatus, percent: f64, total_mib: f64, mib_per_second: f64) -> DownloadTask {
    let p = &mut task.progress;
    p.status = status;
    p.percent = percent;
    p.total = total_mib * MIB;
    p.downloaded = p.total * percent / 100.0;
    p.speed = mib_per_second * MIB;
    p.eta = if mib_per_second > 0.0 { (p.total - p.downloaded) / p.speed } else { 0.0 };
    task
}

fn done(mut task: DownloadTask, file: &str, size_mib: f64) -> DownloadTask {
    let p = &mut task.progress;
    p.status = DownloadStatus::Done;
    p.percent = 100.0;
    p.total = size_mib * MIB;
    p.downloaded = p.total;
    p.filename = format!("C:\\Users\\Alex\\Music\\{file}");
    task.completed_at = task.created_at.map(|at| at + 40_000.0);
    task
}

fn failed(mut task: DownloadTask, code: ErrorCode, raw: &str) -> DownloadTask {
    task.progress.status = DownloadStatus::Failed;
    task.progress.error = ahg_engine::downloader::errors::explain_error(raw);
    task.error_code = Some(code);
    task
}

// ── The downloads ───────────────────────────────────────────────────────────

fn get_lucky() -> DownloadTask {
    let t = base(&Sample {
        id: "get-lucky",
        source: SourcePlatform::Spotify,
        title: "Get Lucky (feat. Pharrell Williams and Nile Rodgers)",
        uploader: "Daft Punk",
        format: "mp3",
        audio: true,
        duration: 369.0,
        minutes_ago: 0.4,
    });
    running(
        spotify(t, "Get Lucky (feat. Pharrell Williams and Nile Rodgers)", "Daft Punk", "Random Access Memories", 1.0),
        DownloadStatus::Downloading,
        62.0,
        8.4,
        3.1,
    )
}

fn bunny() -> DownloadTask {
    let t = base(&Sample {
        id: "big-buck-bunny",
        source: SourcePlatform::Youtube,
        title: "Big Buck Bunny 60fps 4K - Official Blender Foundation Short Film",
        uploader: "Blender",
        format: "mp4",
        audio: false,
        duration: 635.0,
        minutes_ago: 0.7,
    });
    running(t, DownloadStatus::Downloading, 34.0, 412.0, 11.6)
}

fn converting() -> DownloadTask {
    let t = base(&Sample {
        id: "tiny-desk",
        source: SourcePlatform::Youtube,
        title: "Khruangbin: Tiny Desk Concert",
        uploader: "NPR Music",
        format: "flac",
        audio: true,
        duration: 1034.0,
        minutes_ago: 1.6,
    });
    running(t, DownloadStatus::Converting, 100.0, 96.0, 0.0)
}

fn queued() -> DownloadTask {
    base(&Sample {
        id: "soundcloud-set",
        source: SourcePlatform::Soundcloud,
        title: "Four Tet - Boiler Room London DJ Set",
        uploader: "Boiler Room",
        format: "mp3",
        audio: true,
        duration: 3540.0,
        minutes_ago: 0.2,
    })
}

fn retrying() -> DownloadTask {
    let mut t = base(&Sample {
        id: "vimeo-film",
        source: SourcePlatform::Vimeo,
        title: "The Last Ice Merchant",
        uploader: "Sandy Patch",
        format: "mp4",
        audio: false,
        duration: 842.0,
        minutes_ago: 2.1,
    });
    t.progress.status = DownloadStatus::Retrying;
    t.progress.percent = 18.0;
    t.retry_count = Some(1.0);
    t
}

fn paused() -> DownloadTask {
    let mut t = running(
        base(&Sample {
            id: "lecture",
            source: SourcePlatform::Youtube,
            title: "Feynman: The Character of Physical Law - Lecture 1",
            uploader: "Cornell University",
            format: "mp4",
            audio: false,
            duration: 3342.0,
            minutes_ago: 26.0,
        }),
        DownloadStatus::Paused,
        47.0,
        640.0,
        0.0,
    );
    t.interrupted = Some(true);
    t
}

fn dreams() -> DownloadTask {
    let t = base(&Sample {
        id: "dreams",
        source: SourcePlatform::Spotify,
        title: "Dreams - 2004 Remaster",
        uploader: "Fleetwood Mac",
        format: "mp3",
        audio: true,
        duration: 258.0,
        minutes_ago: 9.0,
    });
    done(spotify(t, "Dreams - 2004 Remaster", "Fleetwood Mac", "Rumours", 1.0), "Fleetwood Mac - Dreams - 2004 Remaster.mp3", 4.2)
}

/// A Spotify track whose only match carried the other explicit flag: a best guess.
fn humble() -> DownloadTask {
    let t = base(&Sample {
        id: "humble",
        source: SourcePlatform::Spotify,
        title: "HUMBLE.",
        uploader: "Kendrick Lamar",
        format: "mp3",
        audio: true,
        duration: 177.0,
        minutes_ago: 14.0,
    });
    done(spotify(t, "HUMBLE.", "Kendrick Lamar", "DAMN.", 0.9), "Kendrick Lamar - HUMBLE..mp3", 6.8)
}

fn reel() -> DownloadTask {
    done(
        base(&Sample {
            id: "reel",
            source: SourcePlatform::Instagram,
            title: "Sourdough, day three",
            uploader: "breadbythebay",
            format: "mp4",
            audio: false,
            duration: 41.0,
            minutes_ago: 55.0,
        }),
        "Sourdough, day three.mp4",
        9.4,
    )
}

fn nasa() -> DownloadTask {
    done(
        base(&Sample {
            id: "artemis",
            source: SourcePlatform::Youtube,
            title: "Artemis II: Around the Moon and Back",
            uploader: "NASA",
            format: "mp4",
            audio: false,
            duration: 1287.0,
            minutes_ago: 190.0,
        }),
        "Artemis II Around the Moon and Back [1080p].mp4",
        386.0,
    )
}

fn private_post() -> DownloadTask {
    let t = base(&Sample {
        id: "private-reel",
        source: SourcePlatform::Instagram,
        title: "Reel by studio.notes",
        uploader: "studio.notes",
        format: "mp4",
        audio: false,
        duration: 0.0,
        minutes_ago: 4.0,
    });
    failed(
        t,
        ErrorCode::LoginRequired,
        "ERROR: [Instagram] C0ffee: Requested content is not available, rate-limit reached or login required. Use --cookies to authenticate",
    )
}

fn removed() -> DownloadTask {
    let t = base(&Sample {
        id: "removed",
        source: SourcePlatform::Youtube,
        title: "Live at the Roundhouse (full set)",
        uploader: "",
        format: "mp3",
        audio: true,
        duration: 0.0,
        minutes_ago: 31.0,
    });
    failed(t, ErrorCode::Removed, "ERROR: [youtube] dQw4w9WgXcQ: Video unavailable. This video has been removed by the uploader")
}

fn no_match() -> DownloadTask {
    let t = base(&Sample {
        id: "no-match",
        source: SourcePlatform::Spotify,
        title: "Untitled (Demo, 1997)",
        uploader: "The Lighthouse Keepers",
        format: "mp3",
        audio: true,
        duration: 203.0,
        minutes_ago: 6.0,
    });
    failed(
        spotify(t, "Untitled (Demo, 1997)", "The Lighthouse Keepers", "", 0.0),
        ErrorCode::Unknown,
        ahg_engine::downloader::bridge::NO_MATCH_FOUND,
    )
}

fn cancelled() -> DownloadTask {
    let mut t = base(&Sample {
        id: "cancelled",
        source: SourcePlatform::Tiktok,
        title: "How a mechanical watch works",
        uploader: "horology.daily",
        format: "mp4",
        audio: false,
        duration: 58.0,
        minutes_ago: 12.0,
    });
    t.progress.status = DownloadStatus::Cancelled;
    t
}

/// Downloads in progress: two running, one converting, one waiting, one
/// retrying, one paused after a restart.
pub fn active() -> Vec<DownloadTask> {
    vec![get_lucky(), bunny(), converting(), queued(), retrying(), paused()]
}

/// Finished files, including a best-guess Spotify match.
pub fn finished() -> Vec<DownloadTask> {
    vec![dreams(), humble(), reel(), nasa()]
}

/// What can go wrong: a post that needs a sign-in, a removed video, a
/// Spotify track with no match, and a cancelled download.
pub fn troubled() -> Vec<DownloadTask> {
    vec![private_post(), no_match(), removed(), cancelled()]
}

/// A realistic day: a little of everything, newest first.
pub fn everything() -> Vec<DownloadTask> {
    let mut all: Vec<DownloadTask> = active().into_iter().chain(finished()).chain(troubled()).collect();
    all.sort_by(|a, b| b.created_at.partial_cmp(&a.created_at).unwrap_or(std::cmp::Ordering::Equal));
    all
}

/// November so far, and October to compare with.
pub fn stats() -> DownloadStats {
    let bucket = |month: &str, files: f64, gib: f64, by: serde_json::Value| MonthBucket {
        month: month.into(),
        files,
        bytes: gib * 1024.0 * MIB,
        by_platform: by.as_object().cloned().unwrap_or_default(),
    };
    DownloadStats {
        current: bucket("2026-11", 86.0, 3.9, json!({ "youtube": 41, "spotify": 32, "soundcloud": 7, "instagram": 4, "tiktok": 2 })),
        previous: Some(bucket("2026-10", 61.0, 2.7, json!({ "youtube": 30, "spotify": 24, "soundcloud": 5, "instagram": 2 }))),
    }
}

/// Cover art for the samples, by task id: every `<task id>.jpg` in
/// `docs/design-gate/covers`. That folder is not in git (the pictures are
/// other people's), so a checkout without it simply shows placeholders.
pub fn covers() -> Vec<(String, Vec<u8>)> {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("docs").join("design-gate").join("covers");
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jpg" || x == "png" || x == "webp"))
        .filter_map(|e| Some((e.path().file_stem()?.to_string_lossy().into_owned(), std::fs::read(e.path()).ok()?)))
        .collect()
}

// ── After a paste: what "Look up" found ──────────────────────────────────────

fn result(id: &str, source: SourcePlatform, title: &str, uploader: &str, duration: f64) -> SearchResult {
    SearchResult {
        id: id.into(),
        title: title.into(),
        uploader: uploader.into(),
        duration,
        thumbnail_url: String::new(),
        url: format!("https://example.invalid/{id}"),
        source,
    }
}

/// The words typed for [`search`].
pub const QUERY: &str = "khruangbin";

/// A search typed into the field: YouTube, SoundCloud and Spotify answers, in that order.
pub fn search() -> [Vec<SearchResult>; 3] {
    use SourcePlatform::{Soundcloud, Spotify, Youtube};
    [
        vec![
            result("yt-1", Youtube, "Khruangbin: Tiny Desk Concert", "NPR Music", 1034.0),
            result("yt-2", Youtube, "Khruangbin - Live at Villain | Pitchfork Live", "Pitchfork", 3128.0),
            result("yt-3", Youtube, "Khruangbin - So We Won't Forget (Official Video)", "Khruangbin", 300.0),
            result("yt-4", Youtube, "Khruangbin @ Lightning in a Bottle 2019 (Full Set)", "Do LaB", 4380.0),
            result("yt-5", Youtube, "Khruangbin - Maria Tambien (Official Video)", "Khruangbin", 195.0),
            result("yt-6", Youtube, "Khruangbin & Leon Bridges - Texas Sun (Official Video)", "Khruangbin", 253.0),
            result("yt-7", Youtube, "Khruangbin - Time (You and I)", "Khruangbin", 342.0),
        ],
        vec![
            result("sc-1", Soundcloud, "White Gloves", "Khruangbin", 236.0),
            result("sc-2", Soundcloud, "People Everywhere (Still Alive)", "Khruangbin", 212.0),
            result("sc-3", Soundcloud, "Khruangbin Boiler Room mix", "Boiler Room", 3660.0),
        ],
        vec![
            result("sp-1", Spotify, "Texas Sun", "Khruangbin, Leon Bridges", 252.0),
            result("sp-2", Spotify, "So We Won't Forget", "Khruangbin", 300.0),
            result("sp-3", Spotify, "Friday Morning", "Khruangbin", 407.0),
            result("sp-4", Spotify, "August 10", "Khruangbin", 259.0),
        ],
    ]
}

/// A Spotify album link looked up: its tracks, to pick from before downloading.
pub fn playlist() -> Playlist {
    let names: [(&str, f64); 10] = [
        ("Speedo", 187.0),
        ("Shadowboxin'", 198.0),
        ("Rhymes Like Dimes", 259.0),
        ("The Finest", 240.0),
        ("Back in the Days (Skit)", 49.0),
        ("Go With the Flow", 216.0),
        ("Tick, Tick...", 247.0),
        ("Red and Gold", 284.0),
        ("The Hands of Doom (Skit)", 111.0),
        ("Who You Think I Am?", 204.0),
    ];
    let tracks: Vec<Track> = names
        .iter()
        .enumerate()
        .map(|(i, (name, seconds))| Track {
            id: format!("op-{i}"),
            name: (*name).into(),
            artist: "MF DOOM".into(),
            artists: vec!["MF DOOM".into()],
            album: "Operation: Doomsday".into(),
            album_year: None,
            track_number: i as f64 + 1.0,
            duration_ms: seconds * 1000.0,
            thumbnail_url: String::new(),
            spotify_url: None,
            explicit: Some(i % 3 != 1),
        })
        .collect();
    Playlist {
        id: "operation-doomsday".into(),
        name: "Operation: Doomsday".into(),
        description: String::new(),
        owner: "MF DOOM".into(),
        track_count: 19.0,
        tracks,
        thumbnail_url: String::new(),
        url: "https://open.spotify.com/album/example".into(),
    }
}

/// One YouTube video looked up: the step where format and quality are chosen.
pub fn media() -> MediaMetadata {
    MediaMetadata {
        title: "Cosmos: A Spacetime Odyssey - Standing Up in the Milky Way".into(),
        uploader: "National Geographic".into(),
        duration: 2640.0,
        thumbnail_url: String::new(),
        url: "https://www.youtube.com/watch?v=example".into(),
        is_collection: false,
        entries: Vec::new(),
        description: None,
        width: Some(3840.0),
        height: Some(2160.0),
        view_count: Some(4_812_339.0),
        like_count: None,
        upload_date: Some("20240309".into()),
        extractor: Some("youtube".into()),
    }
}

/// The Reels link of [`short`].
pub const SHORT_URL: &str = "https://www.instagram.com/reel/C0ffee/";

/// A Reels link looked up: the short-form poster.
pub fn short() -> MediaMetadata {
    MediaMetadata {
        title: "Sourdough, day three".into(),
        uploader: "breadbythebay".into(),
        duration: 41.0,
        thumbnail_url: String::new(),
        url: SHORT_URL.into(),
        is_collection: false,
        entries: Vec::new(),
        description: Some("Day three of the starter. It finally doubled overnight.".into()),
        width: Some(1080.0),
        height: Some(1920.0),
        view_count: Some(48_210.0),
        like_count: Some(3_904.0),
        upload_date: Some("20261112".into()),
        extractor: Some("Instagram".into()),
    }
}

/// A YouTube playlist link looked up: 48 videos, the first ten listed.
pub fn youtube_playlist() -> Playlist {
    let names: [(&str, f64); 10] = [
        ("Lecture 1: The Law of Gravitation", 3342.0),
        ("Lecture 2: The Relation of Mathematics and Physics", 3318.0),
        ("Lecture 3: The Great Conservation Principles", 3371.0),
        ("Lecture 4: Symmetry in Physical Law", 3423.0),
        ("Lecture 5: The Distinction of Past and Future", 2760.0),
        ("Lecture 6: Probability and Uncertainty", 3398.0),
        ("Lecture 7: Seeking New Laws", 3454.0),
        ("Questions and answers, part 1", 1260.0),
        ("Questions and answers, part 2", 1385.0),
        ("Closing remarks", 610.0),
    ];
    let tracks: Vec<Track> = names
        .iter()
        .enumerate()
        .map(|(i, (name, seconds))| Track {
            id: format!("yt-pl-{i}"),
            name: (*name).into(),
            artist: "Cornell University".into(),
            artists: vec!["Cornell University".into()],
            album: String::new(),
            album_year: None,
            track_number: i as f64 + 1.0,
            duration_ms: seconds * 1000.0,
            thumbnail_url: String::new(),
            spotify_url: None,
            explicit: None,
        })
        .collect();
    Playlist {
        id: "character-of-physical-law".into(),
        name: "The Character of Physical Law".into(),
        description: String::new(),
        owner: "Cornell University".into(),
        track_count: 48.0,
        tracks,
        thumbnail_url: String::new(),
        url: "https://www.youtube.com/playlist?list=example".into(),
    }
}
