//! Compares the Spotify match scoring with electron/downloader.ts
//! (tests/golden/score.json, written by scripts/rust-golden/score.mjs).
//! Scores are compared bit for bit, and so is the order.
use ahg_engine::downloader::score::score_candidates;
use ahg_engine::types::{SearchResult, Track};
use serde_json::Value;

fn from_bits(v: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(v.as_str().unwrap(), 16).unwrap())
}

#[test]
fn score_candidates_agrees_with_electron() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("golden/score.json")).unwrap();
    assert!(!cases.is_empty(), "score.json has no cases");
    let mut ranked = 0;
    for (n, c) in cases.iter().enumerate() {
        let mut track: Track = serde_json::from_value(c["track"].clone()).unwrap();
        track.duration_ms = from_bits(&c["trackBits"]);
        let mut candidates: Vec<SearchResult> = serde_json::from_value(c["candidates"].clone()).unwrap();
        for (cand, b) in candidates.iter_mut().zip(c["candidateBits"].as_array().unwrap()) {
            cand.duration = from_bits(b);
        }
        let got = score_candidates(&track, &candidates);
        let want = c["out"].as_array().unwrap();
        let got_ids: Vec<&str> = got.iter().map(|s| s.candidate.id.as_str()).collect();
        let want_ids: Vec<&str> = want.iter().map(|w| w["id"].as_str().unwrap()).collect();
        assert_eq!(got_ids, want_ids, "case {n}: order, track {}", c["track"]["name"]);
        for (g, w) in got.iter().zip(want) {
            assert_eq!(
                g.score.to_bits(),
                from_bits(&w["bits"]).to_bits(),
                "case {n}: {} scored {} want {}",
                g.candidate.id,
                g.score,
                w["score"]
            );
        }
        ranked += want.len();
    }
    assert!(ranked > 100, "corpus ranks too few candidates");
}
