//! D34: the Spotify reader keeps a track's explicit flag, which the bridge
//! uses to prefer the matching version on YouTube Music. Electron drops it.
use ahg_engine::spotify::{track_from_embed, track_from_graphql};
use serde_json::json;

#[test]
fn a_track_page_says_whether_the_track_is_explicit() {
    let page = |flag: serde_json::Value| json!({ "uri": "spotify:track:abc", "name": "Song", "artists": [{ "name": "Artist" }], "duration": 200000, "isExplicit": flag });
    let read = |v: serde_json::Value| track_from_embed(Some(&v), "", "abc").ok().flatten().unwrap().explicit;
    assert_eq!(read(page(json!(true))), Some(true));
    assert_eq!(read(page(json!(false))), Some(false));
    // Not stated, or not a yes/no: unknown, never guessed.
    assert_eq!(read(json!({ "uri": "spotify:track:abc", "name": "Song", "duration": 200000 })), None);
    assert_eq!(read(page(json!("yes"))), None);
}

#[test]
fn a_playlist_or_album_row_says_whether_the_track_is_explicit() {
    let row = |rating: serde_json::Value| json!({ "uri": "spotify:track:abc", "name": "Song", "artists": { "items": [{ "profile": { "name": "Artist" } }] }, "contentRating": rating });
    let read = |v: serde_json::Value| track_from_graphql(Some(&v), None).ok().flatten().unwrap().explicit;
    assert_eq!(read(row(json!({ "label": "EXPLICIT" }))), Some(true));
    assert_eq!(read(row(json!({ "label": "NONE" }))), Some(false));
    assert_eq!(read(json!({ "uri": "spotify:track:abc", "name": "Song" })), None);
}

#[test]
fn an_unknown_flag_is_left_out_of_the_saved_track_so_electron_reads_the_same_file() {
    let track = track_from_embed(Some(&json!({ "uri": "spotify:track:abc", "name": "Song", "duration": 1 })), "", "abc").ok().flatten().unwrap();
    assert!(serde_json::to_value(&track).unwrap().get("explicit").is_none());
}
