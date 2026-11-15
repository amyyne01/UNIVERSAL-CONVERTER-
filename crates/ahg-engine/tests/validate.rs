//! Compares the outside-input validators with electron/security.ts
//! (tests/golden/validate.json, written by scripts/rust-golden.mjs).
use ahg_engine::validate::{
    MAX_COOKIES_BYTES, confine_to_root, is_netscape_cookies, media_url_arg, playlist_items_arg, url_arg,
};
use serde_json::{Value, json};

fn golden() -> Value {
    serde_json::from_str(include_str!("golden/validate.json")).unwrap()
}

fn rows(g: &Value, key: &str) -> Vec<Value> {
    let rows = g[key].as_array().unwrap().clone();
    assert!(!rows.is_empty(), "{key} has no cases");
    rows
}

/// A validator's result in the shape the golden file uses.
fn shape(r: Result<&str, String>) -> Value {
    match r {
        Ok(v) => json!({ "ok": v }),
        Err(e) => json!({ "err": e }),
    }
}

fn short(v: &Value) -> String {
    v.to_string().chars().take(160).collect()
}

#[test]
fn url_arg_accepts_and_rejects_what_electron_does() {
    for c in rows(&golden(), "urlArg") {
        assert_eq!(shape(url_arg(c["v"].as_str().unwrap())), c["out"], "{}", short(&c));
    }
}

#[test]
fn media_url_arg_accepts_and_rejects_what_electron_does() {
    for c in rows(&golden(), "mediaUrlArg") {
        assert_eq!(shape(media_url_arg(c["v"].as_str().unwrap())), c["out"], "{}", short(&c));
    }
}

#[test]
fn playlist_items_arg_accepts_and_rejects_what_electron_does() {
    for c in rows(&golden(), "playlistItemsArg") {
        assert_eq!(shape(playlist_items_arg(c["v"].as_str().unwrap())), c["out"], "{}", short(&c));
    }
}

#[test]
fn is_netscape_cookies_agrees_with_electron() {
    let g = golden();
    assert_eq!(json!(MAX_COOKIES_BYTES), g["maxCookiesBytes"]);
    for c in rows(&g, "isNetscapeCookies") {
        assert_eq!(json!(is_netscape_cookies(c["v"].as_str().unwrap())), c["out"], "{}", short(&c));
    }
}

#[cfg(windows)]
#[test]
fn confine_to_root_agrees_with_electron() {
    for c in rows(&golden(), "confineToRoot") {
        let got = confine_to_root(c["candidate"].as_str().unwrap(), c["root"].as_str().unwrap());
        assert_eq!(json!(got), c["out"], "{}", short(&c));
    }
}
