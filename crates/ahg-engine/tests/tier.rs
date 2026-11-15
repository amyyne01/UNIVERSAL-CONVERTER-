//! Compares the tier clamps and extras merging with what shared/types.ts
//! returns for the same inputs (tests/golden/tier.json, written by
//! scripts/rust-golden.mjs).
use ahg_engine::tier::{
    BASIC_LIMITS, MAX_CONCURRENT_DOWNLOADS, Plan, clamp_audio_quality, clamp_concurrency, clamp_extras, clamp_format,
    clamp_video_quality, is_premium_format, is_premium_video_quality,
};
use ahg_engine::types::{DownloadExtras, PartialExtras, RETRY_BACKOFF_MS, VideoQuality, merge_extras};
use serde_json::{Value, json};

fn golden() -> Value {
    serde_json::from_str(include_str!("golden/tier.json")).unwrap()
}

fn plan(v: &Value) -> Plan {
    serde_json::from_value(v.clone()).unwrap()
}

fn rows(g: &Value, key: &str) -> Vec<Value> {
    let rows = g[key].as_array().unwrap().clone();
    assert!(!rows.is_empty(), "{key} has no cases");
    rows
}

#[test]
fn constants_match_electron() {
    let g = golden();
    assert_eq!(serde_json::to_value(DownloadExtras::default()).unwrap(), g["defaultExtras"]);
    assert_eq!(json!(RETRY_BACKOFF_MS), g["retryBackoffMs"]);
    assert_eq!(json!(MAX_CONCURRENT_DOWNLOADS), g["maxConcurrentDownloads"]);
    assert_eq!(
        json!({
            "maxVideoQuality": BASIC_LIMITS.max_video_quality,
            "maxBatchLinks": BASIC_LIMITS.max_batch_links,
            "maxCollectionTracks": BASIC_LIMITS.max_collection_tracks,
            "maxConcurrent": BASIC_LIMITS.max_concurrent,
            "lossless": BASIC_LIMITS.lossless,
            "scheduler": BASIC_LIMITS.scheduler,
            "maxSubtitleLangs": BASIC_LIMITS.max_subtitle_langs,
            "subtitleAuto": BASIC_LIMITS.subtitle_auto,
            "cookies": BASIC_LIMITS.cookies,
            "splitChapters": BASIC_LIMITS.split_chapters,
        }),
        g["basicLimits"]
    );
}

#[test]
fn video_quality_is_clamped_as_in_electron() {
    for c in rows(&golden(), "videoQuality") {
        let q: VideoQuality = serde_json::from_value(c["q"].clone()).unwrap();
        assert_eq!(json!(is_premium_video_quality(q)), c["premium"], "{c}");
        assert_eq!(json!(clamp_video_quality(q, plan(&c["plan"]))), c["out"], "{c}");
    }
}

#[test]
fn format_is_clamped_as_in_electron() {
    for c in rows(&golden(), "format") {
        let f = c["f"].as_str().unwrap();
        assert_eq!(json!(is_premium_format(f)), c["premium"], "{c}");
        assert_eq!(json!(clamp_format(f, plan(&c["plan"]))), c["out"], "{c}");
    }
}

#[test]
fn audio_quality_is_clamped_as_in_electron() {
    for c in rows(&golden(), "audioQuality") {
        assert_eq!(json!(clamp_audio_quality(c["q"].as_str().unwrap(), plan(&c["plan"]))), c["out"], "{c}");
    }
}

#[test]
fn concurrency_is_clamped_as_in_electron() {
    for c in rows(&golden(), "concurrency") {
        let n: f64 = c["n"].as_str().unwrap().parse().unwrap();
        assert_eq!(json!(clamp_concurrency(n, plan(&c["plan"]))), c["out"], "{c}");
    }
}

#[test]
fn extras_are_merged_and_clamped_as_in_electron() {
    for c in rows(&golden(), "extras") {
        let partial: Option<PartialExtras> = serde_json::from_value(c["partial"].clone()).unwrap();
        let got = if c["plan"].is_null() {
            merge_extras(partial.as_ref())
        } else {
            clamp_extras(partial.as_ref(), plan(&c["plan"]))
        };
        assert_eq!(serde_json::to_value(got).unwrap(), c["out"], "{c}");
    }
}
