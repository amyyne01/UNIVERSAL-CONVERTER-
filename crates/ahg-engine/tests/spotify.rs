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
