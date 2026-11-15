//! The data model must read what Electron wrote and write it back unchanged,
//! because both apps share `downloads-history.json`.
#![recursion_limit = "256"] // the JSON literals below are large
use ahg_engine::types::{DownloadTask, MediaMetadata, Playlist};
use serde_json::{Value, json};

/// Reads `v` as `T` and writes it back; the JSON must come out identical,
/// including which optional keys are present and how numbers are printed.
fn round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(v: Value) {
    let parsed: T = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_string(&parsed).unwrap(), serde_json::to_string(&v).unwrap());
}

/// A task exactly as `createTask` in electron/queue.ts builds it.
fn fresh_task() -> Value {
    json!({
        "taskId": "task_1763200000000_ab12cd34e",
        "url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "source": "youtube",
        "outputDir": "C:\\Users\\x\\Music",
        "format": "mp3",
        "quality": "320",
        "videoQuality": "best",
        "isAudioOnly": true,
        "isPlaylist": false,
        "playlistName": "",
        "embedThumbnail": true,
        "embedMetadata": true,
        "skipExisting": true,
        "title": "",
        "thumbnailUrl": "",
        "duration": 0,
        "uploader": "",
        "progress": {
            "status": "queued", "percent": 0, "speed": 0, "eta": 0, "downloaded": 0, "total": 0,
            "filename": "", "error": "", "playlistIndex": 0, "playlistTotal": 0
        },
        "createdAt": 1763200000000u64
    })
}

#[test]
fn a_freshly_created_task_round_trips() {
    round_trip::<DownloadTask>(fresh_task());
}

#[test]
fn a_finished_spotify_task_with_every_optional_field_round_trips() {
    round_trip::<DownloadTask>(json!({
        "taskId": "task_1763200000001_zz",
        "url": "ytsearch:Artist - Song",
        "source": "spotify",
        "outputDir": "D:\\",
        "format": "flac",
        "quality": "lossless",
        "videoQuality": "1080p",
        "isAudioOnly": true,
        "isPlaylist": true,
        "playlistName": "Mix",
        "playlistLimit": 20,
        "playlistItems": "1,3-5",
        "errorCode": "geo_blocked",
        "embedThumbnail": false,
        "embedMetadata": true,
        "skipExisting": false,
        "title": "Song",
        "thumbnailUrl": "https://i.scdn.co/image/x",
        "duration": 213.5,
        "uploader": "Artist",
        "progress": {
            "status": "done", "percent": 100, "speed": 1536000.25, "eta": 0, "downloaded": 8123456, "total": 8123456,
            "filename": "D:\\Song.flac", "error": "", "playlistIndex": 3, "playlistTotal": 12, "alreadyOnDisk": true
        },
        "retryCount": 2,
        "originalTaskId": "task_1763200000000_aa",
        "interrupted": true,
        "createdAt": 1763200000001u64,
        "completedAt": 1763200009999u64,
        "track": {
            "id": "4cOdK2wGLETKBW3PvgPWqT", "name": "Song", "artist": "Artist", "artists": ["Artist", "Guest"],
            "album": "Album", "albumYear": "2019", "trackNumber": 7, "durationMs": 213500,
            "thumbnailUrl": "https://i.scdn.co/image/x", "spotifyUrl": "https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT"
        },
        "matchConfidence": 0.82,
        "matchedUrl": "https://www.youtube.com/watch?v=abcdefghijk",
        "fallbackProfile": true,
        "extras": {
            "subtitleMode": "both", "subtitleLangs": "en,fr", "subtitleAuto": true, "embedChapters": true,
            "splitChapters": false, "sponsorBlock": "remove", "sponsorBlockCategories": ["sponsor", "intro"],
            "writeThumbnail": true, "writeInfoJson": false, "writeDescription": false, "videoContainer": "mkv",
            "videoCodec": "av1", "cookieBrowser": "", "cookiesFile": "", "outputTemplate": "", "rateLimit": "2M", "proxy": ""
        }
    }));
}

#[test]
fn a_row_saved_before_timestamps_and_extras_existed_round_trips() {
    let mut old = fresh_task();
    old.as_object_mut().unwrap().remove("createdAt");
    round_trip::<DownloadTask>(old);
}

#[test]
fn partial_extras_on_a_task_keep_only_the_keys_they_had() {
    let mut task = fresh_task();
    task["extras"] = json!({ "subtitleMode": "embed", "proxy": "socks5://127.0.0.1:9050" });
    round_trip::<DownloadTask>(task);
}

#[test]
fn metadata_and_playlists_round_trip() {
    let entry = json!({
        "id": "abcdefghijk", "title": "T", "uploader": "U", "duration": 61, "thumbnailUrl": "", "url": "https://youtu.be/abcdefghijk",
        "source": "youtube"
    });
    round_trip::<MediaMetadata>(json!({
        "title": "List", "uploader": "U", "duration": 0, "thumbnailUrl": "", "url": "https://x", "isCollection": true,
        "entries": [entry]
    }));
    round_trip::<MediaMetadata>(json!({
        "title": "Clip", "uploader": "U", "duration": 12.5, "thumbnailUrl": "t", "url": "https://x", "isCollection": false,
        "entries": [], "description": "d", "width": 1080, "height": 1920, "viewCount": 12345678901u64, "likeCount": 9,
        "uploadDate": "20261115", "extractor": "TikTok"
    }));
    round_trip::<Playlist>(json!({
        "id": "p", "name": "N", "description": "", "owner": "O", "tracks": [], "thumbnailUrl": "", "url": "u", "trackCount": 0
    }));
}
