//! Compares the ahg:// protocol links, the clipboard watcher, Discord presence
//! and the feedback, hotkey and tray decisions with electron/protocol.ts,
//! clipboard-watch.ts, presence.ts, feedback.ts, hotkey.ts and tray.ts (golden
//! file written by scripts/rust-golden/shell.mjs).
use std::collections::VecDeque;

use ahg_engine::clipboard_watch::{ClipboardWatcher, POLL_MS, Tick};
use ahg_engine::presence::{Activity, Effect, FLUSH_MS, Frame, FrameReader, Presence, RECONNECT_MS, pipe_path};
use ahg_engine::protocol::{PROTOCOL_SCHEME, Registration, extract_protocol_url, registration, resolve_protocol_link};
use ahg_engine::shell_model::{
    MenuEntry, TRAY_TOOLTIP, TaskbarProgress, TrayAction, accelerator_given, completion_toast, register_outcome, tray_menu,
};
use ahg_engine::types::DownloadStatus;
use serde_json::{Value, json};

fn golden() -> Value {
    serde_json::from_str(include_str!("golden/shell.json")).unwrap()
}

fn list<'a>(v: &'a Value, what: &str) -> &'a Vec<Value> {
    let l = v.as_array().unwrap_or_else(|| panic!("{what} is not a list"));
    assert!(!l.is_empty(), "{what} is empty");
    l
}

/// A number the generator could not write in JSON is written as its name.
fn num(v: &Value) -> f64 {
    match v.as_str() {
        Some("NaN") => f64::NAN,
        Some("Infinity") => f64::INFINITY,
        Some("-Infinity") => f64::NEG_INFINITY,
        _ => v.as_f64().unwrap(),
    }
}

fn same(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b
}

fn status(v: &Value) -> DownloadStatus {
    serde_json::from_value(v.clone()).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn short(v: &Value) -> String {
    v.to_string().chars().take(300).collect()
}

fn check(wrong: Vec<String>) {
    assert!(wrong.is_empty(), "{} mismatches:\n{}", wrong.len(), wrong.join("\n"));
}

// ── protocol.ts ──────────────────────────────────────────────────────────────

#[test]
fn protocol_links_resolve_as_electron_does() {
    let g = golden();
    assert_eq!(PROTOCOL_SCHEME, "ahg");
    let mut wrong = Vec::new();
    for c in list(&g["protocol"]["resolve"], "protocol.resolve") {
        let got = resolve_protocol_link(c["raw"].as_str().unwrap()).map(|d| serde_json::to_value(d).unwrap());
        if got.unwrap_or(Value::Null) != c["out"] {
            wrong.push(short(c));
        }
    }
    check(wrong);
}

#[test]
fn the_first_protocol_argument_is_found_as_electron_finds_it() {
    let g = golden();
    let mut wrong = Vec::new();
    for c in list(&g["protocol"]["extract"], "protocol.extract") {
        let argv: Vec<String> = c["argv"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_string()).collect();
        if json!(extract_protocol_url(&argv)) != c["out"] {
            wrong.push(short(c));
        }
    }
    check(wrong);
}

#[test]
fn protocol_registration_matches_electron() {
    let g = golden();
    for c in list(&g["protocol"]["registration"], "protocol.registration") {
        let got = match registration(c["defaultApp"].as_bool().unwrap(), c["argvLen"].as_u64().unwrap() as usize) {
            Registration::Skip => "none",
            Registration::Plain => "plain",
            Registration::ViaScript => "script",
        };
        assert_eq!(json!(got), c["call"], "{c}");
        if got != "none" {
            assert_eq!(c["scheme"], json!(PROTOCOL_SCHEME));
        }
    }
}

// ── clipboard-watch.ts ───────────────────────────────────────────────────────

#[test]
fn the_clipboard_watcher_offers_as_electron_does() {
    let g = golden();
    assert_eq!(g["clipboard"]["pollMs"], json!(POLL_MS));
    let mut wrong = Vec::new();
    for (n, scenario) in list(&g["clipboard"]["scenarios"], "clipboard.scenarios").iter().enumerate() {
        let mut w = ClipboardWatcher::default();
        for step in list(scenario, "clipboard scenario") {
            let clip = step["clip"].as_str().map(str::to_string);
            let mut reads = 0;
            let mut read = || {
                reads += 1;
                clip.clone()
            };
            let (mut ran, mut started, mut offer, mut errors) = (true, false, Value::Null, 0);
            match step["op"].as_str().unwrap() {
                "start" => started = w.start(&mut read),
                "stop" => w.stop(),
                _ => match w.tick(step["enabled"].as_bool().unwrap(), &mut read) {
                    Tick::Stopped => ran = false,
                    Tick::Idle => {}
                    Tick::Offer(d) => offer = serde_json::to_value(d).unwrap(),
                    Tick::ReadFailed => errors = 1,
                },
            }
            let ms = if started { json!(POLL_MS) } else { Value::Null };
            let ok = json!(ran) == step["ran"]
                && ms == step["ms"]
                && json!(reads) == step["reads"]
                && json!(errors) == step["errors"]
                && offer == step["offer"];
            if !ok {
                wrong.push(format!(
                    "scenario {n}: {}\n  rust: ran {ran}, ms {ms}, reads {reads}, errors {errors}, offer {}",
                    short(step),
                    short(&offer)
                ));
            }
        }
    }
    check(wrong);
}

// ── presence.ts ──────────────────────────────────────────────────────────────

/// Does what the app layer would with each effect, and says what it did in the
/// shape the generator recorded. A failing write or connect is fed back the way
/// the app layer must feed it back.
fn carry_out(p: &mut Presence, effects: Vec<Effect>, windows: bool, xdg: Option<&str>, tmp: Option<&str>, fail: &mut (bool, bool)) -> Vec<Value> {
    let mut queue: VecDeque<Effect> = effects.into();
    let mut events = Vec::new();
    while let Some(effect) = queue.pop_front() {
        let mut follow = Vec::new();
        match effect {
            Effect::Connect(i) => {
                events.push(json!({"op": "connect", "path": pipe_path(i, windows, xdg, tmp)}));
                if std::mem::take(&mut fail.0) {
                    follow = p.connect_failed();
                }
            }
            Effect::Write(bytes) => {
                events.push(json!({"op": "write", "hex": hex(&bytes)}));
                if std::mem::take(&mut fail.1) {
                    follow = p.on_disconnect();
                    queue.retain(|e| !matches!(e, Effect::Write(_)));
                }
            }
            Effect::DestroyProbe | Effect::CloseSocket => events.push(json!({"op": "destroy"})),
            Effect::ScheduleFlush => events.push(json!({"op": "set", "ms": FLUSH_MS})),
            Effect::ScheduleReconnect => events.push(json!({"op": "set", "ms": RECONNECT_MS})),
            Effect::CancelFlush => events.push(json!({"op": "clear", "ms": FLUSH_MS})),
            Effect::CancelReconnect => events.push(json!({"op": "clear", "ms": RECONNECT_MS})),
        }
        for e in follow.into_iter().rev() {
            queue.push_front(e);
        }
    }
    events
}

#[test]
fn presence_connects_writes_and_reconnects_as_electron_does() {
    let g = golden();
    assert_eq!(g["presence"]["flushMs"], json!(FLUSH_MS));
    assert_eq!(g["presence"]["reconnectMs"], json!(RECONNECT_MS));
    let pid = g["presence"]["pid"].as_u64().unwrap() as u32;
    let mut wrong = Vec::new();
    for (n, scenario) in list(&g["presence"]["scenarios"], "presence.scenarios").iter().enumerate() {
        let env = &scenario["env"];
        let windows = env["platform"] == "win32";
        let (xdg, tmp) = (env["xdg"].as_str(), env["tmp"].as_str());
        let mut p = Presence::new(pid);
        let mut fail = (false, false);
        for step in list(&scenario["ops"], "presence ops") {
            let effects = match step["op"].as_str().unwrap() {
                "start" => p.start(step["id"].as_str().unwrap()),
                "activity" => p.set_activity(Activity {
                    downloading: step["downloading"].as_u64().unwrap() as u32,
                    queued: step["queued"].as_u64().unwrap() as u32,
                }),
                "stop" => p.stop(),
                "failNextConnect" => {
                    fail.0 = true;
                    vec![]
                }
                "failNextWrite" => {
                    fail.1 = true;
                    vec![]
                }
                "fire" if step["fired"] == true => {
                    if step["ms"] == json!(FLUSH_MS) { p.flush_timer_fired() } else { p.reconnect_timer_fired() }
                }
                "fire" => vec![],
                _ => {
                    let h = &step["handlers"];
                    let has = |k: &str| h[k].as_u64().unwrap() > 0;
                    let pipe = step["pipe"].as_u64().unwrap() as u32;
                    match step["ev"].as_str().unwrap() {
                        "connect" if has("connect") => p.on_connect(),
                        "error" if has("close") => p.on_disconnect(),
                        "error" if has("error") => p.on_probe_error(pipe),
                        "close" if has("close") => p.on_disconnect(),
                        _ => vec![],
                    }
                }
            };
            let events = carry_out(&mut p, effects, windows, xdg, tmp, &mut fail);
            if json!(events) != step["events"] {
                wrong.push(format!("scenario {n}: {}\n  rust: {}", short(step), short(&json!(events))));
            }
        }
    }
    check(wrong);
}

#[test]
fn presence_frames_decode_as_the_reference_reader_does() {
    let g = golden();
    let mut wrong = Vec::new();
    for c in list(&g["presence"]["decode"], "presence.decode") {
        let mut reader = FrameReader::default();
        let got: Vec<Vec<Value>> = c["chunks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|chunk| {
                reader
                    .feed(&unhex(chunk.as_str().unwrap()))
                    .into_iter()
                    .map(|Frame { op, payload }| json!({"op": op, "payload": payload}))
                    .collect()
            })
            .collect();
        if json!(got) != c["out"] {
            wrong.push(format!("{}\n  rust: {}", short(c), short(&json!(got))));
        }
    }
    check(wrong);
}

// ── feedback.ts ──────────────────────────────────────────────────────────────

#[test]
fn feedback_toasts_and_taskbar_progress_match_electron() {
    let g = golden();
    let mut wrong = Vec::new();
    for (n, steps) in list(&g["feedback"], "feedback").iter().enumerate() {
        let mut bar = TaskbarProgress::default();
        for step in list(steps, "feedback steps") {
            let (mut notified, mut progress) = (Vec::new(), Vec::new());
            if step["op"] == "completed" {
                let title = step["title"].as_str().unwrap();
                notified.extend(completion_toast(status(&step["status"]), title, step["enabled"].as_bool().unwrap()));
            } else {
                let tasks: Vec<(DownloadStatus, f64)> =
                    step["tasks"].as_array().unwrap().iter().map(|t| (status(&t["status"]), num(&t["percent"]))).collect();
                progress.extend(bar.update(&tasks));
            }
            let ok = json!(notified) == step["notified"]
                && progress.len() == step["progress"].as_array().unwrap().len()
                && progress.iter().zip(step["progress"].as_array().unwrap()).all(|(a, b)| same(*a, num(b)));
            if !ok {
                wrong.push(format!("sequence {n}: {}\n  rust: notified {notified:?}, progress {progress:?}", short(step)));
            }
        }
    }
    check(wrong);
}

// ── hotkey.ts ────────────────────────────────────────────────────────────────

#[test]
fn hotkey_decisions_match_electron() {
    let g = golden();
    assert_eq!(list(&g["hotkeyAll"], "hotkeyAll"), &vec![json!("unregisterAll")]);
    for c in list(&g["hotkey"], "hotkey") {
        let accel = c["accelerator"].as_str().unwrap();
        let given = accelerator_given(accel);
        // Calls to Electron happen only for a given accelerator.
        assert_eq!(given, !c["registerCalls"].as_array().unwrap().is_empty(), "{c}");
        assert_eq!(given, !c["unregisterCalls"].as_array().unwrap().is_empty(), "{c}");
        let electron = match c["electronReturns"].as_str().unwrap() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        };
        let result = given && register_outcome(electron);
        assert_eq!(json!(result), c["result"], "{c}");
    }
}

// ── tray.ts ──────────────────────────────────────────────────────────────────

#[test]
fn the_tray_menu_matches_electron() {
    let g = golden();
    assert_eq!(json!(TRAY_TOOLTIP), g["tray"]["tooltip"]);
    assert_eq!(g["tray"]["tooltipsAreAllEqual"], json!(true));
    let mut wrong = Vec::new();
    for c in list(&g["tray"]["cases"], "tray.cases") {
        let recent: Vec<(String, DownloadStatus)> = c["recent"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r["title"].as_str().unwrap().to_string(), status(&r["status"])))
            .collect();
        let got: Vec<Value> = tray_menu(&recent)
            .into_iter()
            .map(|e| match e {
                MenuEntry::Separator => json!({"type": "separator"}),
                MenuEntry::Item { label, enabled, action } => {
                    let action = action.map(|a| match a {
                        TrayAction::PauseAll => "pauseAll",
                        TrayAction::ResumeAll => "resumeAll",
                        TrayAction::ToggleWindow => "toggleWindow",
                        TrayAction::PasteAndDownload => "pasteAndDownload",
                        TrayAction::Quit => "quit",
                    });
                    json!({"label": label, "enabled": enabled, "action": action})
                }
            })
            .collect();
        if json!(got) != c["menu"] {
            wrong.push(format!("{}\n  rust: {}", short(c), short(&json!(got))));
        }
    }
    check(wrong);
}
