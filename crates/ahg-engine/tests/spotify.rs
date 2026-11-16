//! Compares the Spotify pipeline with electron/spotify.ts (tests/golden/spotify.json,
//! written by scripts/rust-golden/spotify.mjs). Each scenario is a list of steps on one
//! handler. The golden file holds, per step, the requests the TypeScript made and the
//! response each got; here the same responses are replayed, and the requests and the
//! result must come out identical.
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use ahg_engine::spotify::{Request, Spotify, Token, Transport};
use ahg_engine::url_detector::parse_spotify_url;
use serde_json::{Value, json};

struct Replay {
    calls: Rc<RefCell<VecDeque<Value>>>,
    clock: Arc<Mutex<f64>>,
    at: Rc<RefCell<String>>,
}

fn request_json(r: &Request) -> Value {
    assert!(r.timeout_ms > 0, "every request carries a timeout");
    json!({
        "url": r.url,
        "method": r.method,
        "headers": r.headers.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
        "body": r.body,
        "signal": true,
    })
}

impl Transport for Replay {
    fn fetch(&mut self, req: &Request) -> Option<String> {
        let at = self.at.borrow().clone();
        let Some(call) = self.calls.borrow_mut().pop_front() else { panic!("{at}: unexpected request {}", req.url) };
        assert_eq!(request_json(req), call["req"], "{at}");
        *self.clock.lock().unwrap() += call["advance"].as_f64().unwrap();
        call["resp"]["text"].as_str().map(str::to_owned)
    }
}

fn to_json<T: serde::Serialize>(v: Result<T, ahg_engine::spotify::Thrown>) -> Result<Value, ()> {
    v.map(|v| serde_json::to_value(v).unwrap()).map_err(|_| ())
}

fn run(sp: &mut Spotify<Replay>, step: &Value) -> Result<Value, ()> {
    let arg = step["arg"].as_str().unwrap_or("");
    match step["op"].as_str().unwrap() {
        "parseUrl" => Ok(match parse_spotify_url(arg) {
            Some((kind, id)) => json!({ "type": kind, "id": id }),
            None => json!({ "type": null, "id": null }),
        }),
        "resolveId" => Ok(json!(ahg_engine::spotify::resolve_id(arg))),
        "token" => Ok(json!(sp.token())),
        "seedToken" => {
            sp.state.token = Some(Token { value: step["value"].as_str().unwrap().to_owned(), expires_at: step["expiresAt"].as_f64().unwrap() });
            Ok(Value::Null)
        }
        "heal" => Ok(json!(sp.heal(arg))),
        "state" => Ok(sp.state.snapshot()),
        "fetchTrack" => to_json(sp.fetch_track(arg)),
        "fetchPlaylist" => to_json(sp.fetch_playlist(arg)),
        "fetchAlbum" => to_json(sp.fetch_album(arg)),
        "search" => Ok(serde_json::to_value(sp.search(arg)).unwrap()),
        op => panic!("unknown op {op}"),
    }
}

#[test]
fn spotify_matches_electron() {
    let golden: Vec<Value> = serde_json::from_str(include_str!("golden/spotify.json")).unwrap();
    assert!(!golden.is_empty(), "the golden file has no scenarios");
    let mut requests = 0;
    for scenario in &golden {
        let name = scenario["name"].as_str().unwrap();
        let clock = Arc::new(Mutex::new(1_700_000_000_000.0));
        let calls = Rc::new(RefCell::new(VecDeque::new()));
        let at = Rc::new(RefCell::new(String::new()));
        let transport = Replay { calls: calls.clone(), clock: clock.clone(), at: at.clone() };
        let mut sp = Spotify::new(transport, Box::new({
            let clock = clock.clone();
            move || *clock.lock().unwrap()
        }));
        // D45: Rust starts `searchDesktop` from a verified hash; the golden file was recorded
        // from Electron's placeholder, so replay from that one.
        sp.state.hashes.insert("searchDesktop".into(), "919c08e0c4c5b0a8e8a3b1e6f2e0a6d0c4c5b0a8e8a3b1e6f2e0a6d0c4c5b0a8".into());
        for (i, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
            let label = format!("{name} / step {i} ({} {})", step["op"], step["arg"]);
            *at.borrow_mut() = label.clone();
            if let Some(t) = step["at"].as_f64() {
                *clock.lock().unwrap() = t;
            }
            *calls.borrow_mut() = step["calls"].as_array().unwrap().iter().cloned().collect();
            requests += calls.borrow().len();
            let got = run(&mut sp, step);
            assert!(calls.borrow().is_empty(), "{label}: the TypeScript made {} more request(s)", calls.borrow().len());
            if step["throws"].as_bool() == Some(true) {
                assert_eq!(got, Err(()), "{label}: the TypeScript threw");
            } else {
                let want = &step["result"];
                let got = got.unwrap_or_else(|_| panic!("{label}: threw, the TypeScript returned {want}"));
                assert_eq!(&got, want, "{label}");
            }
        }
    }
    assert!(requests > 0);
}

// ── D45: search heal reaches the lazy route chunk (hand tests; Electron lacks this) ──

use ahg_engine::spotify::{chunk_urls, find_hash};

const BASE: &str = "https://cdn.test/build/";
const NEW_HASH: &str = "abababababababababababababababababababababababababababababababab";

/// A trimmed webpack runtime: a name map, a hash map, and a css pair that must be ignored.
fn runtime_js() -> String {
    format!(
        r#"u.p="{BASE}";u.u=e=>""+(({{4406:"xpui-routes-search",9908:"dwp-panel-section"}})[e]||e)+"."+({{4406:"bdb477e8",9908:"11223344"}})[e]+".js";u.miniCssF=e=>""+(({{4406:"xpui-routes-search"}})[e]||e)+"."+({{4406:"deadbeef"}})[e]+".css""#
    )
}

#[test]
fn chunk_urls_lists_the_js_chunks_with_full_urls() {
    let got = chunk_urls(&runtime_js(), BASE);
    let want = vec![
        ("xpui-routes-search".to_owned(), format!("{BASE}xpui-routes-search.bdb477e8.js")),
        ("dwp-panel-section".to_owned(), format!("{BASE}dwp-panel-section.11223344.js")),
    ];
    assert_eq!(got, want);
    assert!(chunk_urls("no runtime here", BASE).is_empty());
}

#[test]
fn find_hash_prefers_the_definition_form() {
    let cases = [
        (format!(r#"new _.l("searchDesktop","query","{NEW_HASH}",null)"#), Some(NEW_HASH)),
        (format!(r#"new _.l("searchDesktop","mutation","{NEW_HASH}",null)"#), Some(NEW_HASH)),
        // the name appears first with no hash nearby, then as a definition
        (format!(r#"x="searchDesktop";{}new _.l("searchDesktop","query","{NEW_HASH}",null)"#, "y;".repeat(300)), Some(NEW_HASH)),
        // the name appears first next to another hash; the definition still wins
        (format!(r#"f("searchDesktop","{}");new _.l("searchDesktop","query","{NEW_HASH}",null)"#, "cd".repeat(32)), Some(NEW_HASH)),
        ("nothing".to_owned(), None),
    ];
    assert!(!cases.is_empty());
    for (js, want) in &cases {
        assert_eq!(find_hash(js, "searchDesktop").as_deref(), *want, "{js}");
    }
}

struct Scripted {
    log: Rc<RefCell<Vec<String>>>,
    chunk: Option<String>,
}

impl Transport for Scripted {
    fn fetch(&mut self, req: &Request) -> Option<String> {
        self.log.borrow_mut().push(req.url.clone());
        let u = &req.url;
        if u.contains("operationName=searchDesktop") {
            return Some(if u.contains(NEW_HASH) {
                r#"{"data":{"searchV2":{"tracksV2":{"items":[{"item":{"data":{"uri":"spotify:track:AAA","name":"One","artists":{"items":[{"profile":{"name":"Ann"}}]},"albumOfTrack":{"name":"Al"},"duration":{"totalMilliseconds":1000}}}}]}}}}"#.to_owned()
            } else {
                r#"{"errors":[{"message":"PersistedQueryNotFound"}]}"#.to_owned()
            });
        }
        if u == "https://open.spotify.com/" {
            return Some(r#"<script src="https://cdn.test/build/web-player.js"></script>"#.to_owned());
        }
        if u == "https://cdn.test/build/web-player.js" {
            return Some(runtime_js());
        }
        if u.ends_with("xpui-routes-search.bdb477e8.js") {
            return self.chunk.clone();
        }
        panic!("unexpected request {u}");
    }
}

fn searcher(chunk: Option<String>) -> (Spotify<Scripted>, Rc<RefCell<Vec<String>>>) {
    let log = Rc::new(RefCell::new(vec![]));
    let mut sp = Spotify::new(Scripted { log: log.clone(), chunk }, Box::new(|| 1_700_000_000_000.0));
    sp.state.token = Some(Token { value: "t".to_owned(), expires_at: 1.8e12 });
    (sp, log)
}

#[test]
fn search_heals_through_the_lazy_chunk_and_retries() {
    let chunk = format!(r#"new _.l("searchDesktop","query","{NEW_HASH}",null)"#);
    let (mut sp, log) = searcher(Some(chunk));
    let tracks = sp.search("ann");
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].id, "AAA");
    assert_eq!(sp.state.hashes["searchDesktop"], NEW_HASH);
    let log = log.borrow();
    // stale answer, home page, web-player script, exactly one lazy chunk, retry
    assert_eq!(log.len(), 5, "{log:?}");
    assert!(log[4].contains(NEW_HASH));
}

#[test]
fn search_returns_empty_when_the_heal_finds_nothing() {
    let (mut sp, log) = searcher(Some("nothing here".to_owned()));
    assert!(sp.search("ann").is_empty());
    assert_eq!(log.borrow().len(), 4, "no retry without a new hash");
}
