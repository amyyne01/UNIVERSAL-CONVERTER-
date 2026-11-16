//! D37: a "%" in a title or playlist name stays a "%" in the file name
//! (yt-dlp reads "%%" as one "%"). Rust only: Electron differs on purpose.
use ahg_engine::downloader::args::build_ytdlp_args;
use ahg_engine::types::DownloadTask;
use serde_json::{Value, json};

fn build(over: &Value) -> Vec<String> {
    let golden: Value = serde_json::from_str(include_str!("golden/args.json")).unwrap();
    let mut task = golden["base"].clone();
    for (k, v) in over.as_object().unwrap() {
        task[k] = v.clone();
    }
    let task: DownloadTask = serde_json::from_value(task).unwrap();
    build_ytdlp_args(&task, None)
}

fn output_args(over: &Value) -> Vec<String> {
    let args = build(over);
    args.iter().enumerate().filter(|(_, a)| *a == "-o").map(|(i, _)| args[i + 1].clone()).collect()
}

#[test]
fn percent_is_doubled_in_the_literal_pieces_only() {
    let cases = [
        (json!({"title": "100%"}), vec!["C:\\Downloads\\100%%.%(ext)s"]),
        (json!({"title": "100%(id)s"}), vec!["C:\\Downloads\\100%%(id)s.%(ext)s"]),
        (json!({"title": "a % b"}), vec!["C:\\Downloads\\a %% b.%(ext)s"]),
        // Already doubled by the user: doubled again, so it stays "%%" on disk.
        (json!({"title": "%%"}), vec!["C:\\Downloads\\%%%%.%(ext)s"]),
        (json!({"title": "Test Song"}), vec!["C:\\Downloads\\Test Song.%(ext)s"]),
        (
            json!({"isPlaylist": true, "playlistName": "50% (mix)"}),
            vec!["C:\\Downloads\\50%% (mix)\\%(title)s.%(ext)s"],
        ),
        (
            json!({"isPlaylist": true, "playlistName": "50%", "extras": {"splitChapters": true}}),
            vec![
                "C:\\Downloads\\50%%\\%(title)s.%(ext)s",
                "chapter:C:\\Downloads\\50%%\\%(title)s - %(section_number)03d %(section_title)s.%(ext)s",
            ],
        ),
        // The user's own template is yt-dlp's language: untouched.
        (
            json!({"title": "100%", "extras": {"outputTemplate": "%(title)s - 5%"}}),
            vec!["C:\\Downloads\\%(title)s - 5%.%(ext)s"],
        ),
        (
            json!({"isPlaylist": true, "playlistName": "50%", "extras": {"outputTemplate": "%(title)s"}}),
            vec!["C:\\Downloads\\50%%\\%(title)s.%(ext)s"],
        ),
    ];
    assert!(!cases.is_empty());
    for (over, want) in cases {
        assert_eq!(output_args(&over), want, "{over}");
    }
}

#[test]
fn the_download_archive_path_is_a_plain_path_not_a_template() {
    let args = build(&json!({"isPlaylist": true, "playlistName": "50%"}));
    let at = args.iter().position(|a| a == "--download-archive").unwrap();
    assert_eq!(args[at + 1], "C:\\Downloads\\50%\\.download-archive.txt");
}

#[test]
fn a_spotify_name_with_percent_is_replaced_once_and_not_doubled_twice() {
    use ahg_engine::downloader::bridge::name_and_tag_from_spotify;
    use ahg_engine::queue::{TaskInit, create_task};
    use ahg_engine::types::{SourcePlatform, Track};
    let track: Track = serde_json::from_value(json!({
        "id": "", "album": "", "trackNumber": 0, "thumbnailUrl": "", "durationMs": 1000,
        "name": "100% Pure", "artist": "Band", "artists": ["Band"]
    }))
    .unwrap();
    // The task title is the track's bare name (what a playlist preview sends), or empty.
    for title in [Some("100% Pure"), None] {
        let init = TaskInit {
            url: Some("https://open.spotify.com/track/abc".into()),
            source: Some(SourcePlatform::Spotify),
            output_dir: Some("C:\\Music".into()),
            format: Some("mp3".into()),
            is_audio_only: Some(true),
            title: title.map(str::to_owned),
            track: Some(track.clone()),
            ..Default::default()
        };
        let task = create_task(&init, "task_1".into(), 0);
        let mut args = build_ytdlp_args(&task, None);
        name_and_tag_from_spotify(&task, &mut args);
        let at = args.iter().position(|a| a == "-o").unwrap();
        assert_eq!(args[at + 1], "C:\\Music\\Band - 100%% Pure.%(ext)s", "{title:?}");
    }
}
