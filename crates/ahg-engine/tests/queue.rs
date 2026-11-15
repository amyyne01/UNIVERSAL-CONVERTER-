//! Replays every scenario in tests/golden/queue.json against the Rust queue.
//! The file was recorded by scripts/rust-golden/queue.mjs from the real
//! electron/queue.ts with its clock, timers, downloader and window replaced by
//! recorders. After each step the messages sent, the downloader calls, the
//! listener calls, the pending timers, the task list and the history file must
//! all match.
use std::fs;
use std::path::PathBuf;

use ahg_engine::queue::{Effect, Event, Queue, TaskInit, create_task};
use ahg_engine::types::DownloadProgress;
use serde_json::{Value, json};

const START_MS: u64 = 1_763_200_000_000;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("ahg-test-{}-queue", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("song.mp3"), vec![0u8; 4321]).unwrap();
        Self(dir)
    }
    fn real(&self, v: &Value) -> Value {
        let tmp = self.0.to_str().unwrap();
        match v {
            Value::String(s) => Value::String(s.replace("{TMP}", tmp)),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.real(x)).collect()),
            Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), self.real(x))).collect()),
            other => other.clone(),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// What a step produced, in the shape the recorder used.
#[derive(Default)]
struct Recorded {
    sends: Vec<Value>,
    changes: u64,
    failures: Vec<Value>,
    dl: Vec<Value>,
    completed: Vec<Value>,
}

struct Replay {
    queue: Queue,
    now: u64,
    ids: Vec<String>,
    /// (task id, attempt) for each download the queue started, in order.
    calls: Vec<(String, u64)>,
}

impl Replay {
    fn record(&mut self, effects: Vec<Effect>) -> Recorded {
        let mut rec = Recorded::default();
        for effect in effects {
            match effect {
                Effect::Send(event) => rec.sends.push(match event {
                    Event::Queued { id, task } => json!(["download:queued", id, task]),
                    Event::Progress { id, progress } => json!(["download:progress", id, progress]),
                    Event::Done { id, progress } => json!(["download:done", id, progress]),
                    Event::Error { id, message } => json!(["download:error", id, message]),
                    Event::Cancelled { id } => json!(["download:cancelled", id]),
                }),
                Effect::Changed => rec.changes += 1,
                Effect::Failure(text) => rec.failures.push(json!(text)),
                Effect::Download { task, attempt } => {
                    self.calls.push((task.task_id.clone(), attempt));
                    let track = task.track.clone();
                    rec.dl.push(json!(["download", task, track]));
                }
                Effect::Cancel(id) => rec.dl.push(json!(["cancel", id])),
                Effect::CancelAll => rec.dl.push(json!(["cancelAll"])),
                Effect::Completed { task, filepaths } => rec.completed.push(json!([task.task_id, filepaths])),
                Effect::Log { .. } => {}
            }
        }
        rec
    }

    fn id(&self, i: &Value) -> String {
        let i = i.as_u64().unwrap() as usize;
        self.ids.get(i).cloned().unwrap_or_else(|| format!("missing_{i}"))
    }
}

/// The `yt(n)` helper of the generator, for the bulk-enqueue step.
fn yt(n: usize) -> Value {
    json!({ "url": format!("https://youtu.be/{n:a>11}"), "source": "youtube", "outputDir": "C:\\Music", "title": format!("Song {n}") })
}

#[test]
fn the_queue_behaves_as_electron_does_step_by_step() {
    let scenarios: Vec<Value> = serde_json::from_str(include_str!("golden/queue.json")).unwrap();
    assert!(scenarios.len() >= 30);
    let scratch = Scratch::new();
    let mut wrong = Vec::new();

    for (n, scenario) in scenarios.iter().enumerate() {
        let name = scenario["name"].as_str().unwrap();
        let dir = scratch.0.join(n.to_string());
        fs::create_dir_all(&dir).unwrap();
        let history = dir.join("downloads-history.json");
        if let Some(file) = scenario["file"].as_str() {
            fs::write(&history, file).unwrap();
        }
        let mut r = Replay { queue: Queue::new(), now: START_MS, ids: Vec::new(), calls: Vec::new() };

        for expected in scenario["out"].as_array().unwrap() {
            let step = &expected["step"];
            let (a, b, c, d) = (&step[1], &step[2], &step[3], &step[4]);
            let now = r.now;
            let mut ret = Value::Null;
            let effects = match step[0].as_str().unwrap() {
                "init" => {
                    r.queue.init_history(&history, now);
                    Vec::new()
                }
                "slots" => r.queue.set_max_slots(a.as_f64().unwrap(), now),
                "enqueue" => {
                    let init: TaskInit = serde_json::from_value(scratch.real(a)).unwrap();
                    let id = expected["ret"]["taskId"].as_str().unwrap().to_owned();
                    let task = create_task(&init, id.clone(), now);
                    ret = serde_json::to_value(&task).unwrap();
                    r.ids.push(id);
                    r.queue.enqueue(task, now)
                }
                "enqueueMany" => {
                    let mut effects = Vec::new();
                    for (i, id) in expected["ret"].as_array().unwrap().iter().enumerate() {
                        let init: TaskInit = serde_json::from_value(yt(1000 + i)).unwrap();
                        let id = id.as_str().unwrap().to_owned();
                        r.ids.push(id.clone());
                        effects.extend(r.queue.enqueue(create_task(&init, id, now), now));
                    }
                    ret = expected["ret"].clone();
                    effects
                }
                op @ ("pause" | "resume" | "resumeId" | "cancel" | "promote") => {
                    let id = if op == "resumeId" { a.as_str().unwrap().to_owned() } else { r.id(a) };
                    let (ok, effects) = match op {
                        "pause" => r.queue.pause_task(&id, now),
                        "resume" | "resumeId" => r.queue.resume_task(&id, now),
                        "cancel" => r.queue.cancel_task(&id, now),
                        _ => r.queue.promote_task(&id, now),
                    };
                    ret = json!(ok);
                    effects
                }
                "pauseAll" => r.queue.pause_all(now),
                "resumeAll" => r.queue.resume_all(now),
                "cancelAll" => r.queue.cancel_all(now),
                op @ ("remove" | "reorder") => {
                    let ids: Vec<String> = a.as_array().unwrap().iter().map(|i| r.id(i)).collect();
                    ret = json!(true);
                    if op == "remove" { r.queue.remove_tasks(&ids, now) } else { r.queue.reorder(&ids, now) }
                }
                "progress" => {
                    let (id, attempt) = r.calls[a.as_u64().unwrap() as usize].clone();
                    let progress: DownloadProgress = serde_json::from_value(b.clone()).unwrap();
                    r.queue.on_progress(&id, attempt, progress, now)
                }
                "done" => {
                    let (id, attempt) = r.calls[a.as_u64().unwrap() as usize].clone();
                    let filepath = scratch.real(b).as_str().unwrap().to_owned();
                    let filepaths: Vec<String> = serde_json::from_value(scratch.real(c)).unwrap();
                    r.queue.on_done(&id, attempt, &filepath, &filepaths, d.as_bool().unwrap(), now)
                }
                "error" => {
                    let (id, attempt) = r.calls[a.as_u64().unwrap() as usize].clone();
                    r.queue.on_error(&id, attempt, b.as_str().unwrap(), now)
                }
                "advance" => {
                    r.now += a.as_u64().unwrap();
                    r.queue.advance(r.now)
                }
                "flush" => {
                    r.queue.flush_history();
                    Vec::new()
                }
                other => panic!("unknown step {other}"),
            };
            let rec = r.record(effects);
            let disk = fs::read(&history).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok());
            let got = json!({
                "ret": ret,
                "sends": rec.sends,
                "changes": rec.changes,
                "failures": rec.failures,
                "dl": rec.dl,
                "completed": rec.completed,
                "state": {
                    "all": r.queue.all_tasks(),
                    "recent": r.queue.recent_downloads().iter().map(|(title, status)| json!({ "title": title, "status": status })).collect::<Vec<_>>(),
                    "unfinished": r.queue.has_unfinished_work(),
                    "disk": disk,
                    "timers": r.queue.pending_timers(r.now),
                },
            });
            for key in ["ret", "sends", "changes", "failures", "dl", "completed"] {
                if got[key] != scratch.real(&expected[key]) {
                    wrong.push(format!("[{name}] step {step}: {key}\n  electron: {}\n  rust:     {}", expected[key], got[key]));
                }
            }
            for key in ["all", "recent", "unfinished", "disk", "timers"] {
                if got["state"][key] != scratch.real(&expected["state"][key]) {
                    wrong.push(format!(
                        "[{name}] step {step}: state.{key}\n  electron: {}\n  rust:     {}",
                        expected["state"][key], got["state"][key]
                    ));
                }
            }
            if wrong.len() > 12 {
                break;
            }
        }
        if wrong.len() > 12 {
            break;
        }
    }
    let shown: Vec<String> = wrong.iter().map(|w| w.chars().take(1500).collect()).collect();
    assert!(wrong.is_empty(), "{} mismatches (first shown):\n{}", wrong.len(), shown.join("\n"));
}
