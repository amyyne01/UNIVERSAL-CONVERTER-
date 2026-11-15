//! Compares the normalisation of yt-dlp's JSON with electron/downloader.ts
//! (tests/golden/metadata.json, written by scripts/rust-golden/metadata.mjs).
use ahg_engine::downloader::metadata::{metadata_from_items, search_results};
use ahg_engine::types::SourcePlatform;
use serde_json::{Value, json};

/// The generator writes every number that is not a safe integer as `{"$f": "<text>"}`,
/// because serde_json can misread a float by one digit; Rust's own parser does not.
fn exact(v: Value) -> Value {
    match v {
        Value::Object(m) if m.len() == 1 && m.contains_key("$f") => json!(m["$f"].as_str().unwrap().parse::<f64>().unwrap()),
        Value::Object(m) => Value::Object(m.into_iter().map(|(k, v)| (k, exact(v))).collect()),
        Value::Array(a) => Value::Array(a.into_iter().map(exact).collect()),
        v => v,
    }
}

fn golden() -> Value {
    exact(serde_json::from_str(include_str!("golden/metadata.json")).unwrap())
}

fn rows(g: &Value, key: &str) -> Vec<Value> {
    let rows = g[key].as_array().unwrap().clone();
    assert!(!rows.is_empty(), "{key} has no cases");
    rows
}

fn short(v: &Value) -> String {
    v.to_string().chars().take(300).collect()
}

fn items(c: &Value) -> Vec<Value> {
    c["items"].as_array().unwrap().clone()
}

#[test]
fn search_results_match_electron() {
    for c in rows(&golden(), "search") {
        let items = items(&c);
        let youtube = json!({ "ok": serde_json::to_value(search_results(&items, SourcePlatform::Youtube)).unwrap() });
        let soundcloud = json!({ "ok": serde_json::to_value(search_results(&items, SourcePlatform::Soundcloud)).unwrap() });
        assert_eq!(youtube, c["youtube"], "youtube: {}", short(&c));
        assert_eq!(soundcloud, c["soundcloud"], "soundcloud: {}", short(&c));
    }
}

#[test]
fn fetch_metadata_matches_electron() {
    for c in rows(&golden(), "metadata") {
        let got = match metadata_from_items(&items(&c), c["url"].as_str().unwrap()) {
            Ok(m) => json!({ "ok": serde_json::to_value(m).unwrap() }),
            Err(e) => json!({ "err": e }),
        };
        assert_eq!(got, c["out"], "{}", short(&c));
    }
}
