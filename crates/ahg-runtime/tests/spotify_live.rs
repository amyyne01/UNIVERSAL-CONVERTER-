//! D45 smoke test against the real service. Not run by default:
//! `cargo test -p ahg-runtime --test spotify_live -- --ignored`
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::channel;

use ahg_engine::tier::Plan;
use ahg_runtime::core::{Core, Paths};
use ahg_runtime::ytdlp::YtDlp;

#[test]
#[ignore = "needs the network"]
fn spotify_search_returns_tracks_from_the_live_service() {
    let dir = std::env::temp_dir().join(format!("ahg-spotify-live-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("userData")).unwrap();
    let paths = Paths { user_data: dir.join("userData"), downloads: dir.to_string_lossy().into_owned() };
    let (tx, _rx) = channel();
    let core = Core::start(paths, Arc::new(YtDlp::new(PathBuf::from("yt-dlp-not-used"), None)), Box::new(|| Plan::Premium), tx);
    let tracks = core.spotify_search("daft punk");
    println!("tracks: {}", tracks.len());
    assert!(tracks.len() > 5, "got {} tracks", tracks.len());
}
