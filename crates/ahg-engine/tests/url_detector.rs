//! Compares `detect_url` and `parse_spotify_url` with what the Electron app's
//! own functions return for the same inputs (tests/golden/url_detector.json,
//! written by scripts/rust-golden.mjs from electron/url-detector.ts).
use ahg_engine::url_detector::{detect_url, parse_spotify_url};
use serde_json::{Value, json};

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!("golden/url_detector.json")).unwrap()
}

#[test]
fn detect_url_matches_electron_for_every_golden_input() {
    let mut wrong = Vec::new();
    for c in cases() {
        let input = c["input"].as_str().unwrap();
        let got = serde_json::to_value(detect_url(input)).unwrap();
        if got != c["detect"] {
            wrong.push(format!("{input:?}\n  electron: {}\n  rust:     {got}", c["detect"]));
        }
    }
    assert!(wrong.is_empty(), "{} mismatches:\n{}", wrong.len(), wrong.join("\n"));
}

#[test]
fn parse_spotify_url_matches_electron_for_every_golden_input() {
    for c in cases() {
        let input = c["input"].as_str().unwrap();
        let got = match parse_spotify_url(input) {
            Some((kind, id)) => json!({ "type": kind, "id": id }),
            None => json!({ "type": null, "id": null }),
        };
        assert_eq!(got, c["spotify"], "{input:?}");
    }
}

#[test]
fn the_golden_file_covers_every_platform() {
    let seen: std::collections::BTreeSet<String> =
        cases().iter().map(|c| c["detect"]["platform"].as_str().unwrap().to_owned()).collect();
    assert_eq!(seen.len(), 17, "{seen:?}");
}
