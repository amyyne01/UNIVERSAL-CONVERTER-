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
    for case in cases {
        let mut task = golden["base"].clone();
        for (k, v) in case["over"].as_object().unwrap() {
            task[k] = v.clone();
        }
        let task: DownloadTask = serde_json::from_value(task).unwrap();
        let want: Vec<&str> = case["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        assert_eq!(build_ytdlp_args(&task, case["ffmpeg"].as_str()), want, "{}", case["over"]);
    }
}
