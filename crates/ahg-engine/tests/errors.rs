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

fn shown(c: &Value) -> String {
    c["input"].to_string().chars().take(160).collect()
}

#[test]
fn classify_error_agrees_with_electron() {
    for c in cases() {
        let got = class_name(classify_error(c["input"].as_str().unwrap()));
        assert_eq!(json!(got), c["classify"], "{}", shown(&c));
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
        assert_eq!(got, c["code"], "{}", shown(&c));
    }
}

#[test]
fn explain_error_agrees_with_electron() {
    for c in cases() {
        assert_eq!(json!(explain_error(c["input"].as_str().unwrap())), c["explain"], "{}", shown(&c));
    }
}
