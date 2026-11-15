//! The credential-free Spotify metadata pipeline of electron/spotify.ts, without
//! the network call itself.
//!
//!   Tier 1  the embed page's `__NEXT_DATA__` block yields an anonymous token and the entity.
//!   Tier 2  with that token, Spotify's persisted-query GraphQL pages through the full track
//!           list; a stale query hash is healed from the live web-player bundle, behind a cooldown.
//!   Tier 3  the condensed track list embedded in the page is the fallback.
//!
//! Everything except sending a request is here. The pieces are pure functions that
//! build a [`Request`] or parse a response text, plus [`Spotify`], which strings them
//! together over a [`Transport`]: the one thing the network layer has to supply is
//! "perform this request, give back the response text, or `None` if it failed". Status
//! codes are never looked at, as in the original.
//!
//! Responses are read as JavaScript read them (`serde_json::Value` navigated with `?.`
//! and `??` semantics), so a missing key, a `null` and a value of the wrong type behave
//! as they did there.
//!
//! ponytail: not modelled. (1) Fields whose JSON type is wrong for the output (a numeric
//! `name`, a string `trackNumber`, a non-string cover `url`, a string `totalCount`) come out
//! as `""`/`0`/ignored, where JavaScript passed the odd value through into the `Track`.
//! (2) `Object.values` order for integer-like keys (JavaScript lists those first) in the
//! deep scan. (3) Concurrent heals of one operation sharing a single attempt: here they
//! cannot overlap. (4) `JSON.parse` and `serde_json` differ on lone surrogate escapes,
//! numbers out of range and nesting over 128 levels.
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value, json};

use crate::downloader::progress::js_number;
use crate::text::{js_trim, utf16_prefix};
use crate::types::{Playlist, Track};
use crate::url_detector::parse_spotify_url;

pub const EMBED: &str = "https://open.spotify.com/embed";
pub const GQL_URL: &str = "https://api-partner.spotify.com/pathfinder/v1/query";
pub const HOME: &str = "https://open.spotify.com/";
pub const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

/// The anonymous token is cached for 25 minutes, whatever expiry the page states.
pub const TOKEN_TTL_MS: f64 = 25.0 * 60.0 * 1000.0;
/// After a heal attempt (good or bad) the same operation is not healed again for a minute.
pub const HEAL_COOLDOWN_MS: f64 = 60.0 * 1000.0;
/// Budget for each scraper request; the network layer applies it.
pub const FETCH_TIMEOUT_MS: u64 = 10_000;
/// One budget for the whole heal (home page and every bundle together).
pub const HEAL_TIMEOUT_MS: u64 = 15_000;
/// How many web-player bundles a heal scans.
pub const MAX_BUNDLES: usize = 8;
/// A well-known public track, fetched only to mint a token when `search` has none.
pub const TOKEN_BOOTSTRAP_TRACK_ID: &str = "7tFiyTwD0nx5a1eklYtX2J";

pub const PLAYLIST_PAGE: u32 = 100;
pub const PLAYLIST_CAP: u32 = 10_000;
pub const ALBUM_PAGE: u32 = 50;
pub const ALBUM_CAP: u32 = 2_000;

/// Starting persisted-query hashes. `searchDesktop` is a placeholder the service
/// rejects, so a first search always heals first.
pub const DEFAULT_HASHES: [(&str, &str); 3] = [
    ("fetchPlaylist", "73a3b3470804983e4d55d83cd6cc99715019228fd999d51429cc69473a18789d"),
    ("getAlbum", "46ae954ef809572f5e21cea2bedf2ea9ff1c5a73af2b90d8a52a96d0e6e98018"),
    ("searchDesktop", "919c08e0c4c5b0a8e8a3b1e6f2e0a6d0c4c5b0a8e8a3b1e6f2e0a6d0c4c5b0a8"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Playlist,
    Album,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Playlist => "playlist",
            Kind::Album => "album",
        }
    }

    /// The GraphQL operation name.
    pub fn operation(self) -> &'static str {
        match self {
            Kind::Playlist => "fetchPlaylist",
            Kind::Album => "getAlbum",
        }
    }

    pub fn page_size(self) -> u32 {
        match self {
            Kind::Playlist => PLAYLIST_PAGE,
            Kind::Album => ALBUM_PAGE,
        }
    }

    pub fn cap(self) -> u32 {
        match self {
            Kind::Playlist => PLAYLIST_CAP,
            Kind::Album => ALBUM_CAP,
        }
    }
}

/// The error JavaScript would have thrown: a track node whose `uri` is neither
/// missing nor a string (`uri.startsWith` is not a function). `search` swallows it
/// into an empty list; the fetches pass it on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thrown;

type Res<T> = Result<T, Thrown>;

// ── Requests and transport ─────────────────────────────────────────────────────

/// A request exactly as the original sent it. There is never a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub url: String,
    pub method: &'static str,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    /// Abort the request after this long.
    pub timeout_ms: u64,
}

fn get_request(url: String, headers: Vec<(&str, String)>, timeout_ms: u64) -> Request {
    Request { url, method: "GET", headers: headers.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(), body: None, timeout_ms }
}

fn ua_request(url: String, timeout_ms: u64) -> Request {
    get_request(url, vec![("user-agent", UA.to_owned())], timeout_ms)
}

/// The network layer. `None` means the request failed or was aborted; any HTTP
/// status is a success, and the response text is handed back as it is.
pub trait Transport {
    fn fetch(&mut self, req: &Request) -> Option<String>;

    /// Several requests at once (the heal fetches its bundles in parallel). Results
    /// come back in request order. The default runs them one after another.
    fn fetch_many(&mut self, reqs: &[Request]) -> Vec<Option<String>> {
        reqs.iter().map(|r| self.fetch(r)).collect()
    }
}

/// `encodeURIComponent`.
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => out.push(b as char),
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

/// Tier 1: the embed page of `type_` (`track`, `playlist`, `album`) and `id`.
pub fn embed_request(type_: &str, id: &str) -> Request {
    ua_request(format!("{EMBED}/{type_}/{id}"), FETCH_TIMEOUT_MS)
}

/// One persisted-query GraphQL call. The variables' key order is part of the URL.
pub fn gql_request(op: &str, variables: &Value, hash: &str, token: &str) -> Request {
    let extensions = json!({ "persistedQuery": { "version": 1, "sha256Hash": hash } });
    let url = format!(
        "{GQL_URL}?operationName={}&variables={}&extensions={}",
        encode_uri_component(op),
        encode_uri_component(&variables.to_string()),
        encode_uri_component(&extensions.to_string()),
    );
    get_request(
        url,
        vec![
            ("authorization", format!("Bearer {token}")),
            ("app-platform", "WebPlayer".to_owned()),
            ("accept", "application/json".to_owned()),
            ("user-agent", UA.to_owned()),
        ],
        FETCH_TIMEOUT_MS,
    )
}

/// Variables of a playlist or album page.
pub fn collection_variables(kind: Kind, id: &str, offset: u32) -> Value {
    json!({ "uri": format!("spotify:{}:{id}", kind.as_str()), "offset": offset, "limit": kind.page_size() })
}

pub fn search_variables(query: &str) -> Value {
    json!({ "searchTerm": query, "offset": 0, "limit": 20, "numberOfTopResults": 20, "includeAudiobooks": false })
}

pub fn home_request() -> Request {
    ua_request(HOME.to_owned(), HEAL_TIMEOUT_MS)
}

/// A bundle is fetched at the `src` found in the home page, verbatim: a relative
/// one is not resolved and simply fails.
pub fn bundle_request(src: &str) -> Request {
    ua_request(src.to_owned(), HEAL_TIMEOUT_MS)
}

// ── JavaScript reading of JSON ─────────────────────────────────────────────────

/// `v?.key`: only an object has keys here.
fn g<'a>(v: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    v?.as_object()?.get(key)
}

fn path<'a>(v: Option<&'a Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().fold(v, |v, k| g(v, k))
}

/// The left side of `??`: missing and `null` are the same.
fn nn(v: Option<&Value>) -> Option<&Value> {
    v.filter(|v| !v.is_null())
}

/// `a ?? b ?? c`.
fn first<'a>(candidates: &[Option<&'a Value>]) -> Option<&'a Value> {
    candidates.iter().copied().find_map(nn)
}

/// `x?.[0]`.
fn index0(v: Option<&Value>) -> Option<&Value> {
    match v? {
        Value::Array(a) => a.first(),
        Value::Object(m) => m.get("0"),
        _ => None,
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0 && !n.is_nan()),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// A string where the output wants one: anything else reads as empty.
fn text_of(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or("").to_owned()
}

fn number_of(v: Option<&Value>) -> f64 {
    v.and_then(Value::as_f64).unwrap_or(0.0)
}

/// `String(v)`.
fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            let x = n.as_f64().unwrap_or(f64::NAN);
            if x == 0.0 {
                "0".into()
            } else if x.fract() == 0.0 && x.abs() < 1e21 {
                format!("{x:.0}")
            } else {
                x.to_string()
            }
        }
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(|x| if x.is_null() { String::new() } else { js_string(x) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// `Number(v)`.
fn to_number(v: &Value) -> f64 {
    match v {
        Value::Null => 0.0,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => js_number(s),
        Value::Array(_) => js_number(&js_string(v)),
        Value::Object(_) => f64::NAN,
    }
}

// ── Tier 1: the embed page ─────────────────────────────────────────────────────

/// What the embed page held.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Embed {
    pub entity: Option<Value>,
    pub token: Option<String>,
}

static NEXT_DATA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<script id="__NEXT_DATA__"[^>]*>([\s\S]*?)</script>"#).unwrap());

/// The first `__NEXT_DATA__` block, parsed. An unparseable or falsy one is none.
pub fn extract_next_data(html: &str) -> Option<Value> {
    let data: Value = serde_json::from_str(NEXT_DATA.captures(html)?.get(1)?.as_str()).ok()?;
    truthy(&data).then_some(data)
}

/// The first value for `key` that is not null anywhere in the document: this level
/// first, then each child in order.
fn deep_find<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => nn(m.get(key)).or_else(|| m.values().find_map(|c| deep_find(c, key))),
        Value::Array(a) => a.iter().find_map(|c| deep_find(c, key)),
        _ => None,
    }
}

pub fn extract_token(data: &Value) -> Option<String> {
    let known = path(Some(data), &["props", "pageProps", "state", "data", "session", "accessToken"]);
    if let Some(Value::String(s)) = known.filter(|s| s.as_str().is_some_and(|s| !s.is_empty())) {
        return Some(s.clone());
    }
    match deep_find(data, "accessToken") {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

pub fn extract_entity(data: &Value) -> Option<Value> {
    nn(path(Some(data), &["props", "pageProps", "state", "data", "entity"])).or_else(|| deep_find(data, "entity")).cloned()
}

/// Token and entity from an embed page. Nothing when the page has no usable data.
pub fn parse_embed(html: &str) -> Embed {
    match extract_next_data(html) {
        Some(data) => Embed { token: extract_token(&data), entity: extract_entity(&data) },
        None => Embed::default(),
    }
}

// ── Tiers 2 and 3: mapping ─────────────────────────────────────────────────────

/// Names from a GraphQL `{ items: [...] }`, a bare array, or an array of strings.
fn artist_names(a: Option<&Value>) -> Vec<String> {
    let Some(a) = a else { return vec![] };
    let Value::Array(items) = nn(g(Some(a), "items")).unwrap_or(a) else { return vec![] };
    items
        .iter()
        .filter_map(|x| {
            let name = if x.is_string() { Some(x) } else { first(&[g(g(Some(x), "profile"), "name"), g(Some(x), "name")]) };
            name.and_then(Value::as_str).filter(|n| !n.is_empty()).map(str::to_owned)
        })
        .collect()
}

/// The last source's url of a `{ sources: [...] }` node (or a bare array), or `""`.
fn cover_url(node: Option<&Value>) -> String {
    let sources = nn(g(node, "sources")).or(node);
    match sources {
        Some(Value::Array(a)) => text_of(g(a.last(), "url")),
        _ => String::new(),
    }
}

/// Four leading ASCII digits of the date, as a string.
fn year_of(date: Option<&Value>) -> Option<String> {
    let date = date.filter(|d| truthy(d))?;
    if let Some(y) = g(Some(date), "year").filter(|y| truthy(y)) {
        return Some(js_string(y));
    }
    let iso = match nn(g(Some(date), "isoString")) {
        Some(v) => js_string(v),
        None => date.as_str().unwrap_or("").to_owned(),
    };
    let year: String = iso.chars().take(4).collect();
    (year.len() == 4 && year.bytes().all(|b| b.is_ascii_digit())).then_some(year)
}

fn spotify_url(id: &str) -> Option<String> {
    Some(format!("https://open.spotify.com/track/{id}"))
}

/// A GraphQL track node (playlist `itemV2.data`, album `track`, search item) → [`Track`].
/// Nodes that are not tracks (episodes, local files, nothing) are `None`.
pub fn track_from_graphql(d: Option<&Value>, album: Option<&Value>) -> Res<Option<Track>> {
    let uri = match nn(g(d, "uri")) {
        None => "",
        Some(Value::String(s)) => s.as_str(),
        Some(_) => return Err(Thrown),
    };
    let Some(id) = uri.strip_prefix("spotify:track:") else { return Ok(None) };
    let artists = artist_names(g(d, "artists"));
    Ok(Some(Track {
        id: id.to_owned(),
        name: text_of(nn(g(d, "name"))),
        artist: artists.first().cloned().unwrap_or_default(),
        artists,
        album: text_of(nn(g(album, "name"))),
        album_year: year_of(g(album, "date")),
        track_number: number_of(nn(g(d, "trackNumber"))),
        duration_ms: number_of(first(&[path(d, &["trackDuration", "totalMilliseconds"]), path(d, &["duration", "totalMilliseconds"])])),
        thumbnail_url: cover_url(g(album, "coverArt")),
        spotify_url: spotify_url(id),
        // D34: "EXPLICIT" or "NONE"; anything else is unknown.
        explicit: path(d, &["contentRating", "label"]).and_then(Value::as_str).map(|label| label == "EXPLICIT"),
    }))
}

/// An embed entity (or one item of its condensed `trackList`) → [`Track`]. `fallback_id`
/// is used when the node has no uri; `fallback_cover` when it has no cover of its own.
pub fn track_from_embed(e: Option<&Value>, fallback_cover: &str, fallback_id: &str) -> Res<Option<Track>> {
    let id = match nn(g(e, "uri")) {
        Some(u) if truthy(u) => match u {
            // A collection's list also carries episodes and local files.
            Value::String(s) => match s.strip_prefix("spotify:track:") {
                Some(id) => id.to_owned(),
                None => return Ok(None),
            },
            _ => return Err(Thrown),
        },
        _ => fallback_id.to_owned(),
    };
    if id.is_empty() {
        return Ok(None);
    }
    let artists = match nn(g(e, "artists")).filter(|a| truthy(a)) {
        Some(a) => artist_names(Some(a)),
        None => nn(g(e, "subtitle")).filter(|s| truthy(s)).map(|s| vec![js_string(s)]).unwrap_or_default(),
    };
    let duration = first(&[g(e, "duration"), g(e, "duration_ms")]).map_or(0.0, to_number);
    let cover = cover_url(g(e, "coverArt"));
    Ok(Some(Track {
        name: text_of(first(&[g(e, "name"), g(e, "title")])),
        artist: artists.first().cloned().unwrap_or_default(),
        artists,
        album: text_of(nn(path(e, &["album", "name"]))),
        album_year: year_of(first(&[g(e, "releaseDate"), path(e, &["album", "date"])])),
        track_number: number_of(nn(g(e, "trackNumber"))),
        duration_ms: if duration.is_nan() { 0.0 } else { duration },
        thumbnail_url: if cover.is_empty() { fallback_cover.to_owned() } else { cover },
        spotify_url: spotify_url(&id),
        explicit: g(e, "isExplicit").and_then(Value::as_bool),
        id,
    }))
}

/// Name, owner, cover and counts of a playlist or album. `meta` is the first GraphQL
/// page's root node, when Tier 2 ran; `entity` the embed entity.
pub fn build_collection(kind: Kind, id: &str, entity: Option<&Value>, meta: Option<&Value>, total: f64, tracks: Vec<Track>) -> Playlist {
    let owner = match kind {
        Kind::Album => artist_names(first(&[g(meta, "artists"), g(entity, "artists")])).into_iter().next().unwrap_or_default(),
        Kind::Playlist => text_of(first(&[path(meta, &["ownerV2", "data", "name"]), path(entity, &["owner", "name"])])),
    };
    let thumbnail_url = [
        cover_url(index0(path(meta, &["images", "items"]))),
        cover_url(g(meta, "coverArt")),
        cover_url(g(entity, "coverArt")),
    ]
    .into_iter()
    .find(|c| !c.is_empty())
    .unwrap_or_default();
    Playlist {
        id: id.to_owned(),
        name: text_of(first(&[g(meta, "name"), g(entity, "name"), g(entity, "title")])),
        description: first(&[g(meta, "description"), g(entity, "subtitle")]).map(js_string).unwrap_or_default(),
        owner,
        thumbnail_url,
        url: format!("https://open.spotify.com/{}/{id}", kind.as_str()),
        track_count: if truthy(&json!(total)) { total } else { tracks.len() as f64 },
        tracks,
    }
}

/// The tracks of a search response.
pub fn parse_search(json: &Value) -> Res<Vec<Track>> {
    let sv = path(Some(json), &["data", "searchV2"]);
    let Some(Value::Array(items)) = first(&[path(sv, &["tracksV2", "items"]), path(sv, &["tracks", "items"])]) else { return Ok(vec![]) };
    let mut tracks = vec![];
    for it in items {
        let data = first(&[path(Some(it), &["item", "data"]), g(Some(it), "data")]);
        tracks.extend(track_from_graphql(data, g(data, "albumOfTrack"))?);
    }
    Ok(tracks)
}

/// A bare base62 id, or the id inside any Spotify link or URI.
pub fn resolve_id(id_or_url: &str) -> Option<String> {
    if let Some((_, id)) = parse_spotify_url(id_or_url) {
        return Some(id);
    }
    let t = js_trim(id_or_url);
    (!t.is_empty() && t.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| t.to_owned())
}

// ── GraphQL responses ──────────────────────────────────────────────────────────

/// `res.json()`: the parsed body, or none when it is not JSON.
pub fn parse_gql(text: &str) -> Option<Value> {
    serde_json::from_str(text).ok()
}

/// A response that means "this hash is not known, heal and retry": no body, a falsy
/// body, or a non-empty `errors` array.
pub fn is_stale(json: Option<&Value>) -> bool {
    match json {
        None => true,
        Some(j) => !truthy(j) || g(Some(j), "errors").and_then(Value::as_array).is_some_and(|e| !e.is_empty()),
    }
}

/// What `gql` hands on: a truthy body with no truthy `errors` (an empty `errors`
/// array is truthy, so it is dropped even though it is not stale).
pub fn accept(json: Option<Value>) -> Option<Value> {
    json.filter(|j| truthy(j) && !g(Some(j), "errors").is_some_and(truthy))
}

/// One page of a playlist or album, folded into what has been collected so far.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Collected {
    pub tracks: Vec<Track>,
    /// The first page's root node.
    pub meta: Option<Value>,
    pub total: f64,
    /// True when the list ended (a short page, or `total` reached), as opposed to a failure or the cap.
    pub complete: bool,
}

/// Folds one page into `acc`. Returns false when collecting should stop.
pub fn collect_page(kind: Kind, json: &Value, acc: &mut Collected) -> Res<bool> {
    let root = path(Some(json), &["data", if kind == Kind::Playlist { "playlistV2" } else { "albumUnion" }]).filter(|r| truthy(r));
    let Some(root) = root else { return Ok(false) };
    if acc.meta.is_none() {
        acc.meta = Some(root.clone());
    }
    let content = g(Some(root), if kind == Kind::Playlist { "content" } else { "tracksV2" });
    if let Some(t) = nn(g(content, "totalCount")).and_then(Value::as_f64) {
        acc.total = t;
    }
    let Some(Value::Array(items)) = g(content, "items").filter(|i| i.as_array().is_some_and(|a| !a.is_empty())) else { return Ok(false) };
    for it in items {
        let (data, album) = match kind {
            Kind::Playlist => {
                let d = path(Some(it), &["itemV2", "data"]);
                (d, g(d, "albumOfTrack"))
            }
            Kind::Album => (g(Some(it), "track"), Some(root)),
        };
        acc.tracks.extend(track_from_graphql(data, album)?);
    }
    if items.len() < kind.page_size() as usize || (acc.total != 0.0 && !acc.total.is_nan() && acc.tracks.len() as f64 >= acc.total) {
        acc.complete = true;
        return Ok(false);
    }
    Ok(true)
}

// ── Self-heal parsing ──────────────────────────────────────────────────────────

static SCRIPT_SRC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<script[^>]+src="([^"]+\.js)""#).unwrap());
static HASH64: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-fA-F0-9]{64}").unwrap());

/// Every `<script ... src="....js">` of the home page, in order.
pub fn bundle_srcs(html: &str) -> Vec<String> {
    SCRIPT_SRC.captures_iter(html).map(|c| c[1].to_owned()).collect()
}

/// The 64-hex hash within 400 UTF-16 units of the FIRST occurrence of `op`.
pub fn find_hash(js: &str, op: &str) -> Option<String> {
    let i = js.find(op)?;
    HASH64.find(utf16_prefix(&js[i..], 400)).map(|m| m.as_str().to_owned())
}

// ── State ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub value: String,
    /// Epoch milliseconds; valid while the clock is strictly before it.
    pub expires_at: f64,
}

/// Token, hashes and heal cooldowns. Time is passed in, never read.
#[derive(Debug, Clone)]
pub struct State {
    pub token: Option<Token>,
    pub hashes: HashMap<String, String>,
    /// When each operation's last heal attempt finished.
    pub last_heal_at: HashMap<String, f64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            token: None,
            hashes: DEFAULT_HASHES.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
            last_heal_at: HashMap::new(),
        }
    }
}

fn num(v: f64) -> Value {
    if v.fract() == 0.0 { json!(v as i64) } else { json!(v) }
}

impl State {
    /// The cached token, or none once it has expired (which also forgets it).
    pub fn get_token(&mut self, now: f64) -> Option<String> {
        if let Some(t) = &self.token
            && now < t.expires_at
        {
            return Some(t.value.clone());
        }
        self.token = None;
        None
    }

    pub fn set_token(&mut self, value: String, now: f64) {
        self.token = Some(Token { value, expires_at: now + TOKEN_TTL_MS });
    }

    /// Cooldown test. An operation never healed counts from time zero.
    pub fn heal_blocked(&self, op: &str, now: f64) -> bool {
        now - self.last_heal_at.get(op).copied().unwrap_or(0.0) < HEAL_COOLDOWN_MS
    }

    pub fn snapshot(&self) -> Value {
        json!({
            "hashes": self.hashes.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<Map<_, _>>(),
            "lastHealAt": self.last_heal_at.iter().map(|(k, v)| (k.clone(), num(*v))).collect::<Map<_, _>>(),
            "token": self.token.as_ref().map(|t| json!({ "value": t.value, "expiresAt": num(t.expires_at) })),
        })
    }
}

// ── The pipeline ───────────────────────────────────────────────────────────────

/// Milliseconds since the Unix epoch, for the production clock.
pub fn system_now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_millis() as f64)
}

pub struct Spotify<T: Transport> {
    pub transport: T,
    pub state: State,
    now: Box<dyn Fn() -> f64>,
}

impl<T: Transport> Spotify<T> {
    /// `now` returns the current time in epoch milliseconds ([`system_now_ms`]).
    pub fn new(transport: T, now: Box<dyn Fn() -> f64>) -> Self {
        Self { transport, state: State::default(), now }
    }

    /// The cached token, if it has not expired. Never fetches.
    pub fn token(&mut self) -> Option<String> {
        self.state.get_token((self.now)())
    }

    /// A single track, read from its own embed (Tier 1).
    pub fn fetch_track(&mut self, id_or_url: &str) -> Res<Option<Track>> {
        let Some(id) = resolve_id(id_or_url) else { return Ok(None) };
        let Some(entity) = self.fetch_embed("track", &id).entity.filter(truthy) else { return Ok(None) };
        track_from_embed(Some(&entity), "", &id)
    }

    pub fn fetch_playlist(&mut self, id_or_url: &str) -> Res<Option<Playlist>> {
        self.fetch_collection(Kind::Playlist, id_or_url)
    }

    pub fn fetch_album(&mut self, id_or_url: &str) -> Res<Option<Playlist>> {
        self.fetch_collection(Kind::Album, id_or_url)
    }

    /// Best-effort catalogue search; empty when it cannot.
    pub fn search(&mut self, query: &str) -> Vec<Track> {
        if self.token().is_none() {
            self.fetch_embed("track", TOKEN_BOOTSTRAP_TRACK_ID);
        }
        match self.gql("searchDesktop", &search_variables(query)) {
            Some(json) => parse_search(&json).unwrap_or_default(),
            None => vec![],
        }
    }

    fn fetch_embed(&mut self, type_: &str, id: &str) -> Embed {
        let Some(html) = self.transport.fetch(&embed_request(type_, id)) else { return Embed::default() };
        let embed = parse_embed(&html);
        if let Some(t) = &embed.token {
            let now = (self.now)();
            self.state.set_token(t.clone(), now);
        }
        embed
    }

    fn fetch_collection(&mut self, kind: Kind, id_or_url: &str) -> Res<Option<Playlist>> {
        let Some(id) = resolve_id(id_or_url) else { return Ok(None) };
        let Embed { entity, token } = self.fetch_embed(kind.as_str(), &id);

        // Tier 2: gated on HAVING a token, not on this page having minted one.
        if token.is_some() || self.token().is_some() {
            let c = self.collect_tracks(kind, &id)?;
            if !c.tracks.is_empty() {
                let total = if c.complete { c.total } else { c.tracks.len() as f64 };
                return Ok(Some(build_collection(kind, &id, entity.as_ref(), c.meta.as_ref(), total, c.tracks)));
            }
        }

        // Tier 3: the condensed list in the page.
        let Some(entity) = entity.filter(truthy) else { return Ok(None) };
        let cover = cover_url(g(Some(&entity), "coverArt"));
        let mut tracks = vec![];
        if let Some(Value::Array(list)) = g(Some(&entity), "trackList") {
            for t in list {
                tracks.extend(track_from_embed(Some(t), &cover, "")?);
            }
        }
        let total = tracks.len() as f64;
        Ok(Some(build_collection(kind, &id, Some(&entity), None, total, tracks)))
    }

    fn collect_tracks(&mut self, kind: Kind, id: &str) -> Res<Collected> {
        let mut acc = Collected::default();
        let mut offset = 0;
        while offset < kind.cap() {
            let Some(json) = self.gql(kind.operation(), &collection_variables(kind, id, offset)) else { break };
            if !collect_page(kind, &json, &mut acc)? {
                break;
            }
            offset += kind.page_size();
        }
        Ok(acc)
    }

    /// One query, with a single heal and retry when the hash is stale.
    fn gql(&mut self, op: &str, variables: &Value) -> Option<Value> {
        let token = self.token()?;
        let hash = self.state.hashes.get(op).cloned().unwrap_or_default();
        let mut json = self.gql_once(op, &hash, variables, &token);
        if is_stale(json.as_ref())
            && let Some(fresh) = self.heal(op)
        {
            json = self.gql_once(op, &fresh, variables, &token);
        }
        accept(json)
    }

    fn gql_once(&mut self, op: &str, hash: &str, variables: &Value, token: &str) -> Option<Value> {
        parse_gql(&self.transport.fetch(&gql_request(op, variables, hash, token))?)
    }

    /// Reads the current hash of `op` from the live web-player bundles. Behind the
    /// cooldown, which starts when the attempt ends, whether it worked or not.
    pub fn heal(&mut self, op: &str) -> Option<String> {
        if self.state.heal_blocked(op, (self.now)()) {
            return None;
        }
        let found = self.scan_bundles(op);
        let done = (self.now)();
        self.state.last_heal_at.insert(op.to_owned(), done);
        found
    }

    fn scan_bundles(&mut self, op: &str) -> Option<String> {
        let html = self.transport.fetch(&home_request())?;
        let reqs: Vec<Request> = bundle_srcs(&html).iter().take(MAX_BUNDLES).map(|s| bundle_request(s)).collect();
        let hash = self.transport.fetch_many(&reqs).into_iter().flatten().find_map(|js| find_hash(&js, op))?;
        self.state.hashes.insert(op.to_owned(), hash.clone());
        Some(hash)
    }
}
