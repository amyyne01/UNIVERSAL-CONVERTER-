//! Compares how a download request becomes a task with the `download:start`
//! handler in electron/ipc.ts (tests/golden/api.json, written by
//! scripts/rust-golden/api.mjs through the real handler registration).
use ahg_engine::api::build_task;
use ahg_engine::tier::Plan;
use serde_json::{Value, json};

#[test]
fn a_request_becomes_the_same_task_or_the_same_refusal_as_in_electron() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("golden/api.json")).unwrap();
    assert_eq!(cases.len(), 6, "three settings files, two plans each");
    let mut wrong = Vec::new();
    let mut checked = 0;
    for case in &cases {
        let config = case["config"].as_object().unwrap();
        let plan: Plan = serde_json::from_value(case["plan"].clone()).unwrap();
        let cookies = case["cookiesFile"].as_str();
        for o in case["out"].as_array().unwrap() {
            let request = if o["request"] == "__undefined__" { Value::Null } else { o["request"].clone() };
            let id = o["task"]["taskId"].as_str().unwrap_or("task_unused").to_owned();
            let got = match build_task(&request, config, plan, cookies, id, o["now"].as_u64().unwrap()) {
                Ok(task) => json!({ "task": task }),
                Err(error) => json!({ "error": error }),
            };
            let expected = match o.get("error") {
                Some(error) => json!({ "error": error }),
                None => json!({ "task": o["task"] }),
            };
            if got != expected {
                let show = |v: &Value| v.to_string().chars().take(900).collect::<String>();
                wrong.push(format!(
                    "[{} / {}] {}\n  electron: {}\n  rust:     {}",
                    case["settings"],
                    case["plan"],
                    show(&o["request"]),
                    show(&expected),
                    show(&got)
                ));
            }
            checked += 1;
        }
    }
    assert!(checked > 600, "only {checked} requests checked");
    assert!(wrong.is_empty(), "{} mismatches:\n{}", wrong.len(), wrong.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

/// D34: the playlist preview has already matched the track, so the request
/// names the video. It reaches yt-dlp, so it must be a YouTube address.
#[test]
fn a_spotify_request_may_carry_the_match_the_preview_found() {
    let config = json!({ "outputDir": "C:/Music", "defaultFormat": "mp3", "defaultQuality": "320" });
    let config = config.as_object().unwrap();
    let request = |matched: Value, confidence: Value| json!({ "url": "https://open.spotify.com/track/abc", "source": "spotify", "matchedUrl": matched, "matchConfidence": confidence });
    let build = |r: Value| build_task(&r, config, Plan::Premium, None, "task_1".into(), 0);

    let task = build(request(json!("https://music.youtube.com/watch?v=right000001"), json!(0.9))).unwrap();
    assert_eq!(task.matched_url.as_deref(), Some("https://music.youtube.com/watch?v=right000001"));
    assert_eq!(task.match_confidence, Some(0.9));
    // No confidence given: a chosen match is not a confirmed one.
    assert!(build(request(json!("https://www.youtube.com/watch?v=right000001"), Value::Null)).unwrap().match_confidence.is_some_and(|c| c < 1.0));

    for bad in ["https://evil.example/watch?v=right000001", "http://music.youtube.com/watch?v=x", "--exec=calc", "ytsearch1:anything", "https://music.youtube.com.evil.example/watch?v=x"] {
        assert!(build(request(json!(bad), json!(1))).is_err(), "{bad} should be refused");
    }
    assert!(build(request(json!(42), json!(1))).is_err());
    // Only the Spotify bridge uses a match: on any other source it is dropped.
    let other = build(json!({ "url": "https://youtu.be/dQw4w9WgXcQ", "source": "youtube", "matchedUrl": "https://music.youtube.com/watch?v=right000001" })).unwrap();
    assert_eq!(other.matched_url, None);
}
