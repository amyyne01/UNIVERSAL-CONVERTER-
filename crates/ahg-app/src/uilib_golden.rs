//! Electron's renderer formatters, replayed: `tests/golden/uilib.jsonl` holds what Electron's own
//! `src/lib` functions returned for edge cases and traps recorded by rust-anything (470 cases). Every
//! mismatch inside the real input domain (`real_input`) is listed and fails the test.

use serde_json::Value;

use crate::{format, spark};

const CASES: &str = include_str!("../tests/golden/uilib.jsonl");

/// What real inputs are: sizes and counts are finite and whole-ish, durations are seconds. Outside
/// it (infinity, NaN, 1e308, negative or fractional counts) our formatters answer in their own way
/// (`0`, a plain decimal) where JavaScript prints `Infinity`, `1e-7` or `-2.5`; nothing sends those.
fn real_input(name: &str, args: &[Value]) -> bool {
    let Some(x) = args.first().and_then(num) else { return true };
    match name {
        "formatBytes" => x.is_finite() && (x <= 0.0 || (1.0..1e18).contains(&x)),
        "formatDuration" => x.is_finite() && x.abs() < 1e15,
        "formatCount" => x.is_finite() && x >= 0.0 && x.fract() == 0.0 && x < 1e15,
        _ => true,
    }
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Object(o) => match o.get("$num")?.as_str()? {
            "NaN" => Some(f64::NAN),
            "Infinity" => Some(f64::INFINITY),
            "-Infinity" => Some(f64::NEG_INFINITY),
            "-0" => Some(-0.0),
            other => other.parse().ok(),
        },
        _ => None,
    }
}

fn same(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b
}

fn run(name: &str, args: &[Value]) -> Option<Result<Value, f64>> {
    let n = |i: usize| args.get(i).and_then(num);
    let s = |i: usize| args.get(i).and_then(Value::as_str).map(str::to_owned);
    Some(match name {
        "formatBytes" => Ok(Value::String(format::bytes(n(0)?))),
        "formatDuration" => Ok(Value::String(format::duration(n(0)?))),
        "formatCount" => Ok(Value::String(format::short_count(n(0)?))),
        "linkLabel" => Ok(Value::String(format::link_label(&s(0)?))),
        "niceCeil" => Err(spark::nice_ceil(n(0)?)),
        "stepCeiling" => Err(spark::step_ceiling(n(0)?, n(1)?, n(2).unwrap_or(0.3))),
        "sparkPath" => {
            let samples: Option<Vec<f64>> = args.first()?.as_array()?.iter().map(num).collect();
            Ok(Value::String(spark::spark_path(&samples?, n(1)?, n(2)?)))
        }
        _ => return None,
    })
}

#[test]
fn electrons_formatters_and_sparkline_give_the_recorded_answers() {
    let mut checked = 0;
    let mut wrong: Vec<String> = Vec::new();
    for line in CASES.lines().filter(|l| !l.trim().is_empty()) {
        let case: Value = serde_json::from_str(line).expect("a case line is JSON");
        let name = case["fn"].as_str().expect("fn");
        let args = case["args"].as_array().expect("args");
        let expected = &case["ok"];
        if !real_input(name, args) {
            continue;
        }
        let Some(got) = run(name, args) else { continue };
        checked += 1;
        let ok = match (&got, num(expected)) {
            (Ok(v), _) => v == expected,
            (Err(g), Some(e)) => same(*g, e),
            (Err(_), None) => false,
        };
        let shown = match &got {
            Ok(v) => v.to_string(),
            Err(g) => format!("{g:?}"),
        };
        let key = case["args"].to_string();
        if !ok {
            wrong.push(format!("{name}({key}) expected {expected}, got {shown}"));
        }
    }
    assert!(checked >= 400, "the case list shrank to {checked}");
    assert!(wrong.is_empty(), "{} of {checked} differ:\n{}", wrong.len(), wrong.iter().take(60).cloned().collect::<Vec<_>>().join("\n"));
}
