//! `buildDownloadRequest` (src/lib/download.ts), cases transcribed by hand from
//! src/test/lib/download.test.ts: the renderer holds it, so the golden generator
//! cannot load it. `toMatchObject` means "these keys, at least".
use ahg_engine::api::{build_request, build_task};
use ahg_engine::tier::Plan;
use serde_json::{Map, Value, json};

fn config() -> Value {
    json!({
        "outputDir": "", "defaultFormat": "mp3", "defaultQuality": "320", "defaultVideoQuality": "1080p",
        "concurrentDownloads": 1, "embedThumbnail": true, "embedMetadata": true, "skipExisting": true,
        "theme": "dark", "clipboardWatch": false, "showNotifications": true, "showVisualizer": false,
        "showAmbientParticles": false, "rateLimit": "", "proxy": "", "minimizeToTray": false,
        "globalHotkey": "", "discordRichPresence": false, "scheduleEnabled": false, "scheduleTime": "",
        "scheduleDays": [], "scheduleShutdown": false,
    })
}

fn over(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn assert_matches(got: &Value, expected: Value) {
    let expected = expected.as_object().unwrap();
    assert!(!expected.is_empty());
    for (key, want) in expected {
        assert_eq!(&got[key], want, "key {key} in {got}");
    }
}

#[test]
fn takes_format_quality_embed_and_skip_existing_defaults_from_config() {
    let req = build_request(Some(&config()), &over(json!({ "url": "https://x", "source": "youtube" })));
    assert_matches(
        &req,
        json!({
            "format": "mp3", "quality": "320", "videoQuality": "1080p", "embedThumbnail": true,
            "embedMetadata": true, "skipExisting": true, "url": "https://x", "source": "youtube",
        }),
    );
}

#[test]
fn falls_back_to_hardcoded_defaults_when_config_is_missing() {
    let o = over(json!({ "url": "https://x", "source": "youtube" }));
    for missing in [None, Some(&Value::Null)] {
        assert_matches(
            &build_request(missing, &o),
            json!({
                "format": "mp3", "quality": "320", "videoQuality": "best", "embedThumbnail": true,
                "embedMetadata": true, "skipExisting": true,
            }),
        );
    }
}

#[test]
fn overrides_win_over_config_defaults() {
    let req = build_request(Some(&config()), &over(json!({ "format": "flac", "skipExisting": false })));
    assert_eq!(req["format"], "flac");
    assert_eq!(req["skipExisting"], false);
}

#[test]
fn the_request_passes_the_engines_own_validation() {
    let req = build_request(Some(&config()), &over(json!({ "url": "https://youtu.be/dQw4w9WgXcQ", "source": "youtube" })));
    let settings = config();
    let task = build_task(&req, settings.as_object().unwrap(), Plan::Premium, None, "t".into(), 0).unwrap();
    assert_eq!(task.url, "https://youtu.be/dQw4w9WgXcQ");
}
