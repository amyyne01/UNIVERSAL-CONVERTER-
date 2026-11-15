//! Compares the progress parsing and the output handling of a download with
//! electron/downloader.ts (tests/golden/progress.json, written by
//! scripts/rust-golden/progress.mjs).
use ahg_engine::downloader::progress::{OutputScan, Outcome, parse_eta, parse_line, parse_size, parse_speed};
use ahg_engine::types::{DownloadProgress, DownloadStatus};
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
    exact(serde_json::from_str(include_str!("golden/progress.json")).unwrap())
}

fn rows(g: &Value, key: &str) -> Vec<Value> {
    let rows = g[key].as_array().unwrap().clone();
    assert!(!rows.is_empty(), "{key} has no cases");
    rows
}

fn short(v: &Value) -> String {
    v.to_string().chars().take(300).collect()
}

/// A number as the golden file writes it: non-finite ones travel as their JavaScript names.
fn enc(n: f64) -> Value {
    if n.is_nan() {
        json!("NaN")
    } else if n.is_infinite() {
        json!(if n > 0.0 { "Infinity" } else { "-Infinity" })
    } else if n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 {
        json!(n as i64)
    } else {
        json!(n)
    }
}

fn progress_json(p: &DownloadProgress) -> Value {
    serde_json::to_value(p).unwrap()
}

fn start(status: &str, percent: f64) -> DownloadProgress {
    DownloadProgress { status: serde_json::from_value(json!(status)).unwrap(), percent, ..Default::default() }
}

/// One line through `parse_line`, in the shape the golden file uses.
fn feed(line: &str, progress: &mut DownloadProgress, last: f64) -> (Value, f64) {
    let r = parse_line(line, progress, last);
    let events: Vec<Value> = if r.emit { vec![progress_json(progress)] } else { vec![] };
    (json!({ "progress": progress_json(progress), "last": enc(r.last_percent), "events": events }), r.last_percent)
}

#[test]
fn parse_line_matches_electron_for_every_line() {
    for c in rows(&golden(), "parseLine") {
        let init = &c["init"];
        let mut p = start(init["status"].as_str().unwrap(), init["percent"].as_f64().unwrap());
        let (out, _) = feed(c["line"].as_str().unwrap(), &mut p, init["last"].as_f64().unwrap());
        assert_eq!(out, c["out"], "{}", short(&c));
    }
}

#[test]
fn parse_line_carries_state_across_a_sequence() {
    for c in rows(&golden(), "sequences") {
        let mut p = DownloadProgress { status: DownloadStatus::Downloading, ..Default::default() };
        let mut last = 0.0;
        let lines = c["lines"].as_array().unwrap();
        let steps = c["steps"].as_array().unwrap();
        assert_eq!(lines.len(), steps.len());
        for (line, want) in lines.iter().zip(steps) {
            let (out, l) = feed(line.as_str().unwrap(), &mut p, last);
            last = l;
            assert_eq!(&out, want, "line {}", short(line));
        }
    }
}

#[test]
fn parse_size_matches_electron() {
    for c in rows(&golden(), "size") {
        assert_eq!(enc(parse_size(c["s"].as_str().unwrap())), c["out"], "{}", short(&c));
    }
}

#[test]
fn parse_speed_matches_electron() {
    for c in rows(&golden(), "speed") {
        assert_eq!(enc(parse_speed(c["s"].as_str().unwrap())), c["out"], "{}", short(&c));
    }
}

#[test]
fn parse_eta_matches_electron() {
    for c in rows(&golden(), "eta") {
        assert_eq!(enc(parse_eta(c["s"].as_str().unwrap())), c["out"], "{}", short(&c));
    }
}

/// The output of a whole download, chunk by chunk, through `OutputScan`.
#[test]
fn output_scan_matches_electron_for_every_chunking() {
    for c in rows(&golden(), "scenarios") {
        let mut scan = OutputScan::new();
        let mut events: Vec<Value> = Vec::new();
        for step in c["steps"].as_array().unwrap() {
            let (stream, text) = (step[0].as_str().unwrap(), step[1].as_str().unwrap());
            let got = if stream == "stderr" { scan.push_stderr(text) } else { scan.push_stdout(text) };
            events.extend(got.iter().map(progress_json));
        }
        let (tail, outcome) = scan.finish(Some(c["code"].as_i64().unwrap() as i32));
        events.extend(tail.iter().map(progress_json));
        let mut out = json!({ "events": events });
        match outcome {
            Outcome::Done { path, filepaths, already_on_disk } => {
                out["done"] = json!({ "path": path, "filepaths": filepaths, "alreadyOnDisk": already_on_disk });
            }
            Outcome::Failed(error) => out["error"] = json!(error),
        }
        assert_eq!(out, c["out"], "{}", short(&c));
    }
}

#[test]
fn a_process_without_an_exit_code_fails_like_electron() {
    // `${code}` of null: Electron reports "yt-dlp exited with code null".
    let (_, outcome) = OutputScan::new().finish(None);
    assert_eq!(outcome, Outcome::Failed("yt-dlp exited with code null".into()));
}
