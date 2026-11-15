//! The Spotify bridge's matching rules against their reference,
//! tools/match-model/match.mjs (tests/golden/bridge.json, written by
//! tools/match-model/golden.mjs from collected songs). Feature values are
//! compared exactly: the ranker's weights were fitted on the reference's numbers.
use ahg_engine::downloader::bridge::{Candidate, Lookup, Verdict, features, norm, verify};
use ahg_engine::types::Track;
use serde_json::{Value, json};

fn golden() -> Value {
    serde_json::from_str(include_str!("golden/bridge.json")).unwrap()
}

fn track(v: &Value) -> Track {
    let mut full = json!({ "id": "", "album": "", "trackNumber": 0, "thumbnailUrl": "" });
    full.as_object_mut().unwrap().extend(v.as_object().unwrap().clone());
    serde_json::from_value(full).unwrap()
}

#[test]
fn text_is_normalised_as_in_the_reference() {
    let golden = golden();
    let cases = golden["norm"].as_array().unwrap();
    assert!(cases.len() > 100, "bridge.json has too few texts");
    for c in cases {
        assert_eq!(norm(c["input"].as_str().unwrap()), c["output"].as_str().unwrap(), "norm of {}", c["input"]);
    }
}

#[test]
fn a_looked_up_video_gets_the_same_verdict_as_in_the_reference() {
    let golden = golden();
    let cases = golden["verdicts"].as_array().unwrap();
    assert!(cases.len() > 500, "bridge.json has too few verdicts");
    let mut seen = [0usize; 3];
    for c in cases {
        let look: Lookup = serde_json::from_value(c["look"].clone()).unwrap();
        let got = verify(&track(&c["track"]), &look);
        let name = match got {
            Verdict::Confirmed => "confirmed",
            Verdict::Rejected => "rejected",
            Verdict::Unknown => "unknown",
        };
        assert_eq!(name, c["verdict"].as_str().unwrap(), "{} against {}", c["track"], c["look"]);
        seen[got as usize] += 1;
    }
    assert!(seen.iter().all(|n| *n > 50), "every verdict should be well represented: {seen:?}");
}

#[test]
fn a_candidate_is_described_by_the_same_numbers_as_in_the_reference() {
    let golden = golden();
    let cases = golden["features"].as_array().unwrap();
    assert!(cases.len() > 200, "bridge.json has too few feature rows");
    for c in cases {
        let cand = Candidate {
            title: c["cand"]["title"].as_str().unwrap(),
            uploader: c["cand"]["uploader"].as_str().unwrap_or(""),
            duration: c["cand"]["duration"].as_f64(),
        };
        let got = features(&track(&c["track"]), &cand, c["rank"].as_u64().unwrap() as usize, c["fromSongs"].as_bool().unwrap());
        let want: Vec<f64> = c["x"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        assert_eq!(got.to_vec(), want, "{} against {}", c["track"], c["cand"]);
    }
}

/// The few-seconds rule, spelled out: the same recording may differ by a few
/// seconds between the two services, but a version note YouTube adds on its
/// own means a different cut.
#[test]
fn a_few_seconds_apart_is_confirmed_only_without_a_version_note_of_its_own() {
    let wanted = track(&json!({ "name": "I Wanna Rock", "artist": "Twisted Sister", "artists": ["Twisted Sister"], "durationMs": 180_000 }));
    let look = |title: &str, duration: f64| Lookup { duration: Some(duration), track: Some(title.into()), artist: Some("Twisted Sister".into()), creator: None };
    assert_eq!(verify(&wanted, &look("I Wanna Rock", 182.0)), Verdict::Confirmed);
    assert_eq!(verify(&wanted, &look("I Wanna Rock", 184.0)), Verdict::Confirmed);
    assert_eq!(verify(&wanted, &look("I Wanna Rock (2016 Remaster)", 184.0)), Verdict::Unknown);
    assert_eq!(verify(&wanted, &look("I Wanna Rock", 187.0)), Verdict::Unknown);
    assert_eq!(verify(&wanted, &look("I Wanna Rock", 200.0)), Verdict::Rejected);
    assert_eq!(verify(&wanted, &look("We're Not Gonna Take It", 219.0)), Verdict::Rejected);
    // No music metadata: nothing to judge by.
    assert_eq!(verify(&wanted, &Lookup { duration: Some(180.0), ..Default::default() }), Verdict::Unknown);
}
