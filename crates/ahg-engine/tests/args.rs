//! Compares the yt-dlp argument builder with Downloader.buildYtDlpArgs
//! (tests/golden/args.json, written by scripts/rust-golden/args.mjs).
use ahg_engine::downloader::args::build_ytdlp_args;
use ahg_engine::types::DownloadTask;
use serde_json::Value;

#[test]
fn builds_the_arguments_electron_builds() {
    let golden: Value = serde_json::from_str(include_str!("golden/args.json")).unwrap();
    let cases = golden["cases"].as_array().unwrap();
    assert!(!cases.is_empty(), "args.json has no cases");
    let mut exempt = 0;
    for case in cases {
        let mut task = golden["base"].clone();
        for (k, v) in case["over"].as_object().unwrap() {
            task[k] = v.clone();
        }
        let task: DownloadTask = serde_json::from_value(task).unwrap();
        let want: Vec<&str> = case["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        let got = build_ytdlp_args(&task, case["ffmpeg"].as_str());
        if got != want && has_percent(&golden["base"], &case["over"]) {
            exempt += 1;
            continue;
        }
        assert_eq!(got, want, "{}", case["over"]);
    }
    assert_eq!(exempt, EXEMPT_BY_D37, "cases that differ from Electron because of D37");
}

/// D37 (Rust only): a "%" in the title or playlist name is written as "%%" so it
/// stays a "%" in the file name. The only cases allowed to differ from Electron
/// are those with a "%" in one of the two, and there are exactly this many.
const EXEMPT_BY_D37: usize = 64;

fn has_percent(base: &Value, over: &Value) -> bool {
    ["title", "playlistName"].iter().any(|k| over.get(k).unwrap_or(&base[k]).as_str().is_some_and(|s| s.contains('%')))
}
