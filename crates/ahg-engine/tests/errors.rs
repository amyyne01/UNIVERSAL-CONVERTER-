//! Compares the error handling with electron/downloader.ts
//! (tests/golden/errors.json, written by scripts/rust-golden/errors.mjs).
use ahg_engine::downloader::errors::{ErrorClass, classify_error, error_code_of, explain_error, needs_client_fallback};
use serde_json::{Value, json};

fn class_name(c: ErrorClass) -> &'static str {
    match c {
        ErrorClass::Retryable => "retryable",
        ErrorClass::Permanent => "permanent",
        ErrorClass::Auth => "auth",
        ErrorClass::Geo => "geo",
    }
}

fn cases() -> Vec<Value> {
    let cases: Vec<Value> = serde_json::from_str(include_str!("golden/errors.json")).unwrap();
    assert!(!cases.is_empty(), "errors.json has no cases");
    cases
}

/// D30 (Rust-only fix): loose words the Electron classifier matched anywhere, with the precise
/// phrases that replaced them (kept in step with the tables in src/downloader/errors.rs).
const D30_LOOSE: &[(&str, &[&str])] = &[
    ("geo", &["geo restriction", "geo restricted", "use a vpn or a proxy server"]),
    ("region", &["not available in your region"]),
    ("private", &["private video. ", "playlist is private", "account is private", "either private or"]),
    ("removed", &["has been removed"]),
    ("deleted", &["has been deleted"]),
    ("copyright", &[]),
    ("drm", &["drm protected"]),
    ("not found", &["\" not found", "space not found"]),
    ("network", &["network error"]),
    ("timeout", &[]),
    ("temporar", &[]),
];

/// An input the Electron result may legitimately differ on: it holds a removed loose word and
/// none of the precise phrases that replaced it. Only ever consulted when Rust and Electron disagree.
fn d30_exempt(input: &str) -> bool {
    let t = input.to_lowercase();
    D30_LOOSE.iter().any(|(loose, precise)| t.contains(loose) && !precise.iter().any(|p| t.contains(p)))
}

/// Inputs on which any of classify / code / explain differs from Electron.
fn differing(cases: &[Value]) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, c) in cases.iter().enumerate() {
        let input = c["input"].as_str().unwrap();
        let same = json!(class_name(classify_error(input))) == c["classify"]
            && serde_json::to_value(error_code_of(input)).unwrap() == c["code"]
            && json!(explain_error(input)) == c["explain"];
        if !same {
            out.push(i);
        }
    }
    out
}

fn shown(c: &Value) -> String {
    c["input"].to_string().chars().take(160).collect()
}

#[test]
fn classify_error_agrees_with_electron() {
    for c in cases() {
        let got = class_name(classify_error(c["input"].as_str().unwrap()));
        if json!(got) != c["classify"] {
            assert!(d30_exempt(c["input"].as_str().unwrap()), "{} -> {got}, Electron {}", shown(&c), c["classify"]);
        }
    }
}

#[test]
fn needs_client_fallback_agrees_with_electron() {
    for c in cases() {
        assert_eq!(json!(needs_client_fallback(c["input"].as_str().unwrap())), c["fallback"], "{}", shown(&c));
    }
}

#[test]
fn error_code_of_agrees_with_electron() {
    for c in cases() {
        let got = serde_json::to_value(error_code_of(c["input"].as_str().unwrap())).unwrap();
        if got != c["code"] {
            assert!(d30_exempt(c["input"].as_str().unwrap()), "{} -> {got}, Electron {}", shown(&c), c["code"]);
        }
    }
}

#[test]
fn explain_error_agrees_with_electron() {
    for c in cases() {
        let got = explain_error(c["input"].as_str().unwrap());
        if json!(got) != c["explain"] {
            assert!(d30_exempt(c["input"].as_str().unwrap()), "{} -> {got:?}, Electron {}", shown(&c), c["explain"]);
        }
    }
}

/// The D30 differences are explicit and counted: some inputs differ, every one is exempt, and
/// the count is printed (run with --nocapture). Raise this only for a reviewed reason.
#[test]
fn d30_exempt_inputs_are_counted() {
    let cases = cases();
    let diff = differing(&cases);
    assert!(!diff.is_empty(), "D30 should change at least one golden input");
    assert!(diff.iter().all(|&i| d30_exempt(cases[i]["input"].as_str().unwrap())));
    println!("D30: {} of {} golden inputs differ from Electron", diff.len(), cases.len());
    for (loose, precise) in D30_LOOSE {
        let n = diff
            .iter()
            .filter(|&&i| {
                let t = cases[i]["input"].as_str().unwrap().to_lowercase();
                t.contains(loose) && !precise.iter().any(|p| t.contains(p))
            })
            .count();
        println!("  uncovered {loose:?}: {n}");
    }
}
