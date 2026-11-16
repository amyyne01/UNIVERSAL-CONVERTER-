//! The request a retry or "download again" sends (D22): the fields the Electron
//! downloads tab passes to `download.start`, and nothing else.
use ahg_engine::api::retry_request;
use ahg_engine::queue::{Effect, Queue, TaskInit, create_task};
use ahg_engine::types::{DownloadTask, SourcePlatform, Track};
use serde_json::json;

const T0: u64 = 1_763_200_000_000;

const FIELDS: [&str; 17] = [
    "url",
    "source",
    "format",
    "quality",
    "videoQuality",
    "isAudioOnly",
    "isPlaylist",
    "playlistName",
    "embedThumbnail",
    "embedMetadata",
    "skipExisting",
    "title",
    "thumbnailUrl",
    "duration",
    "uploader",
    "track",
    "playlistItems",
];

fn sample() -> DownloadTask {
    let track = Track {
        id: "t1".into(),
        name: "Song".into(),
        artist: "Artist".into(),
        artists: vec!["Artist".into()],
        album: "Album".into(),
        album_year: None,
        track_number: 1.0,
        duration_ms: 212_000.0,
        thumbnail_url: String::new(),
        spotify_url: None,
        explicit: None,
    };
    let init = TaskInit {
        url: Some("https://youtu.be/dQw4w9WgXcQ".into()),
        source: Some(SourcePlatform::Youtube),
        output_dir: Some("out".into()),
        format: Some("mp3".into()),
        quality: Some("320".into()),
        is_audio_only: Some(true),
        playlist_name: Some("List".into()),
        skip_existing: Some(true),
        title: Some("A title".into()),
        thumbnail_url: Some("https://i.example/t.jpg".into()),
        duration: Some(212.0),
        uploader: Some("Someone".into()),
        track: Some(track),
        playlist_items: Some("1,3-5".into()),
        retry_count: Some(2.0),
        original_task_id: Some("task_orig".into()),
        ..TaskInit::default()
    };
    create_task(&init, "task_a".into(), T0)
}

#[test]
fn the_request_holds_exactly_the_fields_electron_sends() {
    assert!(!FIELDS.is_empty());
    let req = retry_request(&sample(), None);
    let mut keys: Vec<&str> = req.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut want = FIELDS.to_vec();
    want.sort_unstable();
    assert_eq!(keys, want);
    for left_out in ["outputDir", "retryCount", "originalTaskId"] {
        assert!(req.get(left_out).is_none(), "{left_out}");
    }
}

#[test]
fn the_values_come_from_the_task() {
    let req = retry_request(&sample(), None);
    assert_eq!(req["url"], "https://youtu.be/dQw4w9WgXcQ");
    assert_eq!(req["source"], "youtube");
    assert_eq!((req["format"].as_str(), req["quality"].as_str()), (Some("mp3"), Some("320")));
    assert_eq!(req["isAudioOnly"], true);
    assert_eq!(req["skipExisting"], true);
    assert_eq!(req["title"], "A title");
    assert_eq!(req["duration"], json!(212));
    assert_eq!(req["playlistItems"], "1,3-5");
    assert_eq!(req["track"]["name"], "Song");
}

#[test]
fn skip_existing_can_be_overridden() {
    assert_eq!(retry_request(&sample(), Some(false))["skipExisting"], false);
    assert_eq!(retry_request(&sample(), Some(true))["skipExisting"], true);
    assert_eq!(retry_request(&sample(), None)["skipExisting"], true);
}

#[test]
fn a_task_without_track_or_playlist_items_sends_neither() {
    let init = TaskInit { url: Some("https://youtu.be/dQw4w9WgXcQ".into()), source: Some(SourcePlatform::Youtube), ..TaskInit::default() };
    let req = retry_request(&create_task(&init, "task_b".into(), T0), None);
    assert!(req.get("track").is_none() && req.get("playlistItems").is_none());
}

#[test]
fn the_pending_automatic_retry_is_due_five_seconds_after_the_failure() {
    let mut queue = Queue::new();
    queue.set_max_slots(1.0, T0);
    let init = TaskInit { url: Some("https://youtu.be/dQw4w9WgXcQ".into()), source: Some(SourcePlatform::Youtube), ..TaskInit::default() };
    let effects = queue.enqueue(create_task(&init, "task_a".into(), T0), T0);
    let attempt = effects
        .iter()
        .find_map(|e| match e {
            Effect::Download { attempt, .. } => Some(*attempt),
            _ => None,
        })
        .expect("the task starts at once");
    assert_eq!(queue.retry_due("task_a"), None);
    queue.on_error("task_a", attempt, "ERROR: unable to download video data: The read operation timed out", T0 + 100);
    assert_eq!(queue.retry_due("task_a"), Some(T0 + 100 + 5000));
    assert_eq!(queue.retry_due("nope"), None);
}
