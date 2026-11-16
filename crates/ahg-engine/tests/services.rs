//! Compares the redacting log, the monthly counters and the scheduler with
//! electron/log.ts, stats.ts and scheduler.ts (golden files written by
//! scripts/rust-golden/services.mjs).
use std::fs;
use std::path::PathBuf;

use ahg_engine::clock::LocalTime;
use ahg_engine::log::{LOG_CAP_BYTES, host_of, log_line_to, redact, scrub_urls, tail};
use ahg_engine::scheduler::{Fire, ScheduleConfig, Scheduler};
use ahg_engine::stats::{StatsStore, month_key, stats_file_path};
use ahg_engine::text::js_len;
use ahg_engine::types::SourcePlatform;
use serde_json::{Value, json};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ahg-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// `[year, month, day, hour?, minute?]` as the generator writes local times.
fn local(at: &Value, weekday: u32) -> LocalTime {
    let n = |i: usize, default: i64| at.get(i).and_then(Value::as_i64).unwrap_or(default);
    LocalTime { year: n(0, 0) as i32, month: n(1, 1) as u32, day: n(2, 1) as u32, hour: n(3, 12) as u32, minute: n(4, 0) as u32, weekday }
}

fn short(v: &Value) -> String {
    v.to_string().chars().take(200).collect()
}

// ── log.ts ───────────────────────────────────────────────────────────────────

#[test]
fn log_text_functions_agree_with_electron() {
    let g: Value = serde_json::from_str(include_str!("golden/log.json")).unwrap();
    assert_eq!(json!(LOG_CAP_BYTES), g["capBytes"]);
    let cases = g["cases"].as_array().unwrap();
    assert!(cases.len() > 300);
    let mut wrong = Vec::new();
    for c in cases {
        let t = c["t"].as_str().unwrap();
        let tail_ok = match c["tail"].as_str() {
            Some(expected) => tail(t) == expected,
            // Electron cut an emoji in half here; Rust drops it whole, one unit shorter.
            None => js_len(&tail(t)) as u64 == c["tailLen"].as_u64().unwrap() - 1,
        };
        if json!(redact(t)) != c["redact"] || json!(host_of(t)) != c["host"] || json!(scrub_urls(t)) != c["scrub"] || !tail_ok {
            wrong.push(format!(
                "{}\n  redact {:?}\n  host {:?}\n  scrub {:?}\n  tail ok: {tail_ok}",
                short(c),
                redact(t),
                host_of(t),
                scrub_urls(t)
            ));
        }
    }
    assert!(wrong.is_empty(), "{} mismatches:\n{}", wrong.len(), wrong.join("\n"));
}

#[test]
fn log_lines_are_stamped_redacted_and_appended() {
    let file = scratch("log1").join("ahg.log");
    log_line_to(&file, "info", "token=abc started", "2026-01-15T03:04:05.678Z");
    log_line_to(&file, "error", "second", "2026-01-15T03:04:06.000Z");
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "2026-01-15T03:04:05.678Z [info] token=[redacted] started\n2026-01-15T03:04:06.000Z [error] second\n"
    );
}

#[test]
fn a_full_log_is_rotated_to_dot_one_replacing_the_previous_one() {
    let dir = scratch("log2");
    let file = dir.join("ahg.log");
    fs::write(&file, "x".repeat(LOG_CAP_BYTES - 1)).unwrap();
    log_line_to(&file, "info", "still fits", "T");
    assert!(!dir.join("ahg.log.1").exists(), "rotated below the cap");
    // Now at or over the cap: the next line starts a new file.
    fs::write(dir.join("ahg.log.1"), "older").unwrap();
    log_line_to(&file, "info", "new file", "T");
    assert_eq!(fs::read_to_string(&file).unwrap(), "T [info] new file\n");
    assert!(fs::read_to_string(dir.join("ahg.log.1")).unwrap().starts_with("xxxx"));
}

#[test]
fn logging_into_a_missing_folder_does_nothing_and_does_not_panic() {
    log_line_to(&scratch("log3").join("nope").join("ahg.log"), "info", "x", "T");
}

// ── stats.ts ─────────────────────────────────────────────────────────────────

#[test]
fn month_keys_and_the_file_name_match_electron() {
    let g: Value = serde_json::from_str(include_str!("golden/stats.json")).unwrap();
    assert_eq!(stats_file_path("x").file_name().unwrap().to_str().unwrap(), g["fileName"].as_str().unwrap());
    let cases = g["monthKey"].as_array().unwrap();
    assert!(!cases.is_empty(), "stats.json has no monthKey cases");
    for c in cases {
        assert_eq!(json!(month_key(&local(&c["at"], 0))), c["key"], "{c}");
    }
}

#[test]
fn the_counters_count_roll_and_save_as_electron_does() {
    let g: Value = serde_json::from_str(include_str!("golden/stats.json")).unwrap();
    let tmp = scratch("stats");
    for (name, size) in [("a.bin", 10), ("b.bin", 2500), ("empty.bin", 0)] {
        fs::write(tmp.join(name), vec![0u8; size]).unwrap();
    }
    let scenarios = g["scenarios"].as_array().unwrap();
    assert!(scenarios.len() >= 12);
    // D38: these scenarios cross an idle month, where Electron's `previous` becomes
    // an empty bucket and Rust keeps the last active month. Pinned by the hand test below.
    const IDLE_MONTH: [usize; 1] = [0];
    assert_eq!(IDLE_MONTH.len(), 1);
    for (i, scenario) in scenarios.iter().enumerate() {
        if IDLE_MONTH.contains(&i) {
            let crosses = scenario["out"]
                .as_array()
                .unwrap()
                .iter()
                .any(|st| st["disk"].as_str().is_some_and(|d| d.contains("\"previous\":{\"month\":\"2026-06\",\"files\":0,")));
            assert!(crosses, "scenario {i} no longer crosses an idle month");
            continue;
        }
        let dir = tmp.join(i.to_string());
        fs::create_dir_all(&dir).unwrap();
        let target = stats_file_path(&dir);
        if let Some(file) = scenario["file"].as_str() {
            fs::write(&target, file).unwrap();
        }
        let disk = || fs::read_to_string(&target).ok();
        let mut store = StatsStore::default();
        for step in scenario["out"].as_array().unwrap() {
            let s = &step["step"];
            let month = month_key(&local(&s[1], 0));
            let result = match s[0].as_str().unwrap() {
                "init" => {
                    store.init_at(&target, &month);
                    Value::Null
                }
                "get" => serde_json::to_value(store.get_at(&month)).unwrap(),
                _ => {
                    let platform: SourcePlatform = serde_json::from_value(s[2].clone()).unwrap();
                    let files: Vec<String> =
                        s[3].as_array().unwrap().iter().map(|f| f.as_str().unwrap().replace("{TMP}", tmp.to_str().unwrap())).collect();
                    store.record_at(platform, &files, s[4].as_f64().unwrap(), &month);
                    Value::Null
                }
            };
            assert_eq!(result, step.get("result").cloned().unwrap_or(Value::Null), "scenario {i} step {s}");
            assert_eq!(json!(disk()), step["disk"], "scenario {i} step {s}: file on disk");
        }
    }
}

/// D38, an approved difference from Electron 1.6.1: an idle month never
/// replaces `previous`, so it stays the last month that had downloads.
#[test]
fn an_idle_month_does_not_replace_the_previous_month() {
    // (months visited in order, a download in each, expected previous month after the last visit)
    let cases: [(&[&str], &[bool], Option<&str>); 5] = [
        (&["2026-01", "2026-02", "2026-03"], &[true, false, false], Some("2026-01")),
        (&["2026-01", "2026-02"], &[true, false], Some("2026-01")),
        (&["2026-01", "2026-02", "2026-03"], &[true, true, false], Some("2026-02")),
        (&["2026-01", "2026-02", "2026-03", "2026-04"], &[true, false, false, false], Some("2026-01")),
        (&["2026-01", "2026-02"], &[false, false], None),
    ];
    assert!(!cases.is_empty());
    let tmp = scratch("stats-d38");
    for (i, (months, active, previous)) in cases.iter().enumerate() {
        let dir = tmp.join(i.to_string());
        fs::create_dir_all(&dir).unwrap();
        let target = stats_file_path(&dir);
        let mut store = StatsStore::default();
        store.init_at(&target, months[0]);
        for (month, active) in months.iter().zip(active.iter()) {
            if *active {
                store.record_at(SourcePlatform::Youtube, &[], 5.0, month);
            } else {
                store.get_at(month);
            }
        }
        let last = months.last().unwrap();
        let got = store.get_at(last);
        assert_eq!(got.previous.as_ref().map(|p| p.month.as_str()), *previous, "case {i}");
        // A restart reads the same thing back from the file.
        let mut again = StatsStore::default();
        again.init_at(&target, last);
        assert_eq!(again.get_at(last), got, "case {i}: file round trip");
    }
}

// ── scheduler.ts ─────────────────────────────────────────────────────────────

#[test]
fn the_scheduler_fires_when_electron_does() {
    let g: Value = serde_json::from_str(include_str!("golden/scheduler.json")).unwrap();
    let sequences = g.as_array().unwrap();
    assert!(sequences.len() > 30);
    let mut wrong = Vec::new();
    for sequence in sequences {
        let mut scheduler = Scheduler::default();
        for step in sequence.as_array().unwrap() {
            let cfg: ScheduleConfig = serde_json::from_value(step["cfg"].clone()).unwrap();
            let now = local(&step["at"], step["weekday"].as_u64().unwrap() as u32);
            let fire = scheduler.poll(&cfg, step["premium"].as_bool().unwrap(), &now);
            let got = match fire {
                Fire::None => (0, 0),
                Fire::Trigger { shutdown } => (1, shutdown as u64),
            };
            if json!(got.0) != step["triggered"] || json!(got.1) != step["shutdown"] {
                wrong.push(format!("{step}\n  rust: {fire:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{} mismatches:\n{}", wrong.len(), wrong.join("\n"));
}

#[test]
fn the_real_clock_gives_a_plausible_local_time() {
    let now = LocalTime::now();
    assert!(now.year >= 2026 && (1..=12).contains(&now.month) && (1..=31).contains(&now.day));
    assert!(now.hour < 24 && now.minute < 60 && now.weekday < 7);
}

/// D11, an approved difference from Electron 1.6.0: the scheduler remembers
/// the day it last fired across a restart, so it cannot fire twice in one day.
#[test]
fn a_restart_inside_the_window_does_not_fire_the_schedule_again() {
    let cfg = ScheduleConfig { enabled: true, time: "03:00".into(), days: Vec::new(), shutdown: false };
    let at = |hour, minute, day| LocalTime { year: 2026, month: 1, day, hour, minute, weekday: 4 };

    let mut first_run = Scheduler::default();
    assert_eq!(first_run.poll(&cfg, true, &at(3, 0, 15)), Fire::Trigger { shutdown: false });
    assert_eq!(first_run.last_fired_date(), "2026-01-15");

    // The app is restarted twenty minutes later, still inside the hour after 03:00.
    let mut after_restart = Scheduler::restored(first_run.last_fired_date());
    assert_eq!(after_restart.poll(&cfg, true, &at(3, 20, 15)), Fire::None);
    // The next day it fires as usual.
    assert_eq!(after_restart.poll(&cfg, true, &at(3, 0, 16)), Fire::Trigger { shutdown: false });

    // Without the remembered day (what Electron does) it would have fired again.
    assert_eq!(Scheduler::default().poll(&cfg, true, &at(3, 20, 15)), Fire::Trigger { shutdown: false });
    assert_eq!(Scheduler::default().last_fired_date(), "");
}
