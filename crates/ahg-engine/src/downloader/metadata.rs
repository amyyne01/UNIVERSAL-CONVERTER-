//! Turning yt-dlp's JSON records into the app's metadata and search results:
//! `toNum`, `compact`, `thumb`, `toSearchResult`, `platformOf`, and the mapping
//! inside `fetchMetadata` / `searchYouTube` / `searchSoundCloud` of
//! electron/downloader.ts. Records are read the way JavaScript reads them, so a
//! number given as text, a missing key and a `null` behave the same here.
use serde_json::Value;

use super::progress::js_number;
use crate::text::truthy;
use crate::types::{MediaMetadata, SearchResult, SourcePlatform};
use crate::url_detector::detect_url;

/// `value[key]` where JavaScript would read a property: absent on anything but an object.
fn field<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.get(key).filter(|_| v.is_object())
}

/// `a ?? b ?? ''` over a record's keys: the first one that is neither missing nor `null`.
fn first<'a>(j: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|k| field(j, k).filter(|v| !v.is_null()))
}

/// `Number.prototype.toString`.
fn number_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let sign = if x < 0.0 { "-" } else { "" };
    // Rust prints the shortest digits that read back as the same number, as JavaScript does.
    let sci = format!("{:e}", x.abs());
    let (mantissa, exp) = sci.split_once('e').unwrap();
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp.parse::<i32>().unwrap() + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign_e = if e < 0 { '-' } else { '+' };
        if k == 1 { format!("{digits}e{sign_e}{}", e.abs()) } else { format!("{}.{}e{sign_e}{}", &digits[..1], &digits[1..], e.abs()) }
    };
    format!("{sign}{body}")
}

/// `String(v)`; inside an array `null` is empty, as in `Array.prototype.join`.
fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(|e| if e.is_null() { String::new() } else { js_string(e) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// `String(v ?? '')`.
fn string_or_empty(v: Option<&Value>) -> String {
    v.filter(|v| !v.is_null()).map_or_else(String::new, js_string)
}

/// `toNum`: `Number(v)` when that is finite, else 0. A missing value is 0.
pub fn to_num(v: Option<&Value>) -> f64 {
    let n = match v {
        None | Some(Value::Null) => 0.0,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => js_number(s),
        Some(a @ Value::Array(_)) => js_number(&js_string(a)),
        Some(Value::Object(_)) => f64::NAN,
    };
    if n.is_finite() { n } else { 0.0 }
}

/// `thumb`: `thumbnail` when truthy, else the last of `thumbnails`.
pub fn thumb(j: &Value) -> String {
    if let Some(t) = field(j, "thumbnail").filter(|t| truthy(t)) {
        return js_string(t);
    }
    match field(j, "thumbnails") {
        Some(Value::Array(list)) if !list.is_empty() => string_or_empty(list.last().and_then(|l| field(l, "url"))),
        _ => String::new(),
    }
}

/// `toSearchResult`.
pub fn to_search_result(j: &Value, source: SourcePlatform) -> SearchResult {
    SearchResult {
        id: string_or_empty(field(j, "id")),
        title: string_or_empty(field(j, "title")),
        uploader: string_or_empty(first(j, &["uploader", "channel", "uploader_id"])),
        duration: to_num(field(j, "duration")),
        thumbnail_url: thumb(j),
        url: string_or_empty(first(j, &["webpage_url", "url"])),
        source,
    }
}

/// What `searchYouTube` and `searchSoundCloud` do with yt-dlp's records.
pub fn search_results(items: &[Value], source: SourcePlatform) -> Vec<SearchResult> {
    items.iter().map(|j| to_search_result(j, source)).collect()
}

/// `platformOf`: only these four are told apart; every other site is `Unknown`.
pub fn platform_of(url: &str) -> SourcePlatform {
    let u = url.to_lowercase();
    if u.contains("soundcloud") {
        SourcePlatform::Soundcloud
    } else if u.contains("spotify") {
        SourcePlatform::Spotify
    } else if u.contains("youtu") {
        SourcePlatform::Youtube
    } else {
        SourcePlatform::Unknown
    }
}

/// `compact` for a text field: empty means the site did not report it.
fn text(v: Option<&Value>) -> Option<String> {
    Some(string_or_empty(v)).filter(|s| !s.is_empty())
}

/// `compact` for a number field: 0 means the site did not report it.
fn count(v: Option<&Value>) -> Option<f64> {
    Some(to_num(v)).filter(|n| *n != 0.0)
}

/// What `fetchMetadata` makes of yt-dlp's records for `url`. The first record is
/// the header; `Err` is the message Electron throws when there is none. Running
/// yt-dlp, and the error text for a failed run, are the caller's.
pub fn metadata_from_items(items: &[Value], url: &str) -> Result<MediaMetadata, String> {
    let detected = detect_url(url);
    let Some(head) = items.first() else {
        return Err("That link returned nothing to download.".into());
    };
    // Count is not the test, detection is; but a playlist holding one track dumps one record.
    let is_collection = detected.is_collection || items.len() > 1;
    let source = platform_of(url);
    Ok(MediaMetadata {
        title: string_or_empty(first(head, &["playlist_title", "title"])),
        uploader: string_or_empty(first(head, &["uploader", "channel"])),
        duration: to_num(field(head, "duration")),
        thumbnail_url: thumb(head),
        url: first(head, &["webpage_url"]).map_or_else(|| url.to_owned(), js_string),
        is_collection,
        entries: if is_collection { search_results(items, source) } else { Vec::new() },
        description: text(field(head, "description")),
        width: count(field(head, "width")),
        height: count(field(head, "height")),
        view_count: count(field(head, "view_count")),
        like_count: count(field(head, "like_count")),
        upload_date: text(field(head, "upload_date")),
        extractor: text(first(head, &["extractor_key", "extractor"])),
    })
}
