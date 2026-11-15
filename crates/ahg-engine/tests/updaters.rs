//! Compares the two self-updaters with electron/updater.ts and
//! electron/ytdlp-updater.ts (golden file written by scripts/rust-golden/updaters.mjs).
//!
//! The scenario drivers below play the part of the future network and process
//! layers: they fetch what the engine asks for from the scripted routes, hash
//! nothing (the sha comes from the golden file) and write the files.
use std::fs;
use std::path::{Path, PathBuf};

use ahg_engine::updater::{self, Event, Request, Updater, USER_AGENT};
use ahg_engine::ytdlp_updater::{self as yt, CheckBegin, UpdateBegin, YtdlpStatus, YtdlpUpdater};
use serde_json::{Map, Value, json};

fn golden() -> Value {
    serde_json::from_str(include_str!("golden/updaters.json")).unwrap()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ahg-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn cases<'a>(g: &'a Value, key: &str, min: usize) -> &'a [Value] {
    let list = g[key].as_array().unwrap();
    assert!(list.len() >= min, "{key}: only {} cases", list.len());
    list
}

fn short(v: &Value) -> String {
    v.to_string().chars().take(300).collect()
}

fn report(what: &str, total: usize, wrong: Vec<String>) {
    assert!(wrong.is_empty(), "{what}: {} of {total} differ:\n{}", wrong.len(), wrong.iter().take(25).cloned().collect::<Vec<_>>().join("\n"));
}

// ── pure functions ───────────────────────────────────────────────────────────

#[test]
fn constants_and_file_names_agree() {
    let g = golden();
    let c = &g["constants"];
    assert_eq!(json!(updater::RELEASE_API), c["api"]);
    assert_eq!(json!(updater::SUMS_ASSET), c["sumsAsset"]);
    assert_eq!(json!(USER_AGENT), c["userAgent"]);
    assert_eq!(json!(yt::RELEASE_API), c["ytdlpApi"]);
    assert_eq!(json!(yt::SUMS_ASSET), c["ytdlpSumsAsset"]);
    assert_eq!(json!(yt::ASSET_NAME), c["ytdlpAsset"]);
}

#[test]
fn the_host_gate_agrees() {
    let g = golden();
    let list = cases(&g, "trusted", 200);
    let mut wrong = Vec::new();
    for c in list {
        let url = c["url"].as_str().unwrap();
        let got = match c.get("base") {
            Some(base) => updater::is_trusted_release_host_with_base(url, base.as_str().unwrap()),
            None => updater::is_trusted_release_host(url),
        };
        if json!(got) != c["out"] {
            wrong.push(format!("{} -> rust {got}", short(c)));
        }
    }
    report("is_trusted_release_host", list.len(), wrong);
    assert!(list.iter().any(|c| c["out"] == json!(true)) && list.iter().any(|c| c["out"] == json!(false)));
}

#[test]
fn version_comparison_agrees() {
    let g = golden();
    let list = cases(&g, "newer", 3000);
    let mut wrong = Vec::new();
    for c in list {
        let got = updater::is_newer_version(c["a"].as_str().unwrap(), c["b"].as_str().unwrap());
        if json!(got) != c["out"] {
            wrong.push(format!("{} -> rust {got}", short(c)));
        }
    }
    report("is_newer_version", list.len(), wrong);
    assert!(list.iter().any(|c| c["out"] == json!(true)));
}

#[test]
fn checksum_lines_agree() {
    let g = golden();
    let list = cases(&g, "checksum", 800);
    let mut wrong = Vec::new();
    for c in list {
        let got = updater::parse_checksum(c["sums"].as_str().unwrap(), c["name"].as_str().unwrap());
        if json!(got) != c["out"] {
            wrong.push(format!("{} -> rust {got:?}", short(c)));
        }
    }
    report("parse_checksum", list.len(), wrong);
    assert!(list.iter().any(|c| c["out"].is_string()) && list.iter().any(|c| c["out"].is_null()));
}

#[test]
fn the_apply_script_agrees() {
    let g = golden();
    let list = cases(&g, "swap", 150);
    let mut wrong = Vec::new();
    for c in list {
        let got = updater::swap_script(c["staged"].as_str().unwrap(), c["target"].as_str().unwrap());
        if json!(got) != c["out"] {
            wrong.push(format!("{} -> rust {got:?}", short(c)));
        }
    }
    report("swap_script", list.len(), wrong);
}

#[test]
fn stale_engine_errors_agree() {
    let g = golden();
    let list = cases(&g, "stale", 100);
    let mut wrong = Vec::new();
    for c in list {
        let got = yt::is_engine_stale(c["text"].as_str().unwrap());
        if json!(got) != c["out"] {
            wrong.push(format!("{} -> rust {got}", short(c)));
        }
    }
    report("is_engine_stale", list.len(), wrong);
    assert!(list.iter().any(|c| c["out"] == json!(true)) && list.iter().any(|c| c["out"] == json!(false)));
}

#[test]
fn windows_path_joins_agree() {
    let g = golden();
    let list = cases(&g, "joins", 25);
    let mut wrong = Vec::new();
    for c in list {
        let got = updater::win_join(c["a"].as_str().unwrap(), c["b"].as_str().unwrap());
        if json!(got) != c["out"] {
            wrong.push(format!("{} -> rust {got:?}", short(c)));
        }
    }
    report("win_join", list.len(), wrong);
}

// ── the scripted network ─────────────────────────────────────────────────────

struct Net {
    routes: Map<String, Value>,
    requests: Vec<Value>,
}

struct Response<'a> {
    spec: &'a Value,
}

impl Net {
    fn new(routes: &Value) -> Self {
        Self { routes: routes.as_object().cloned().unwrap_or_default(), requests: Vec::new() }
    }

    /// `request()` of both updaters: the host guard, the fetch, the status and redirect guards.
    fn get(&mut self, url: &str, prefix: &str) -> Result<Response<'_>, String> {
        let req = updater::guard_request(url, prefix)?;
        self.requests.push(json!({ "url": req.url, "headers": { "User-Agent": req.user_agent } }));
        let spec = self.routes.get(url).ok_or_else(|| format!("no route: {url}"))?;
        if let Some(m) = spec.get("throws") {
            return Err(m.as_str().unwrap().to_string());
        }
        let ok = spec.get("ok").and_then(Value::as_bool).unwrap_or(true);
        let status = spec.get("status").and_then(Value::as_u64).map_or(if ok { 200 } else { 500 }, |s| s as u16);
        let url = spec.get("url").and_then(Value::as_str);
        updater::guard_response(ok, status, url, prefix)?;
        Ok(Response { spec })
    }
}

impl Response<'_> {
    fn bytes(&self) -> &str {
        self.spec.get("bytes").and_then(Value::as_str).unwrap_or("")
    }
}

/// The recorded requests, with a numeric url (a non-string URL in the feed) as its text.
fn want_requests(v: &Value) -> Value {
    Value::Array(
        v.as_array()
            .unwrap()
            .iter()
            .map(|r| {
                let mut r = r.clone();
                if let Some(n) = r["url"].as_number().cloned() {
                    r["url"] = json!(n.to_string());
                }
                r
            })
            .collect(),
    )
}

fn events_json(events: &[Event]) -> Vec<Value> {
    events.iter().map(|e| json!({ "channel": e.channel, "payload": e.payload })).collect()
}

fn with_tmp(v: &Value, tmp: &str) -> Value {
    match v {
        Value::String(s) => Value::String(s.replace("{TMP}", tmp)),
        Value::Array(a) => Value::Array(a.iter().map(|x| with_tmp(x, tmp)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.replace("{TMP}", tmp), with_tmp(x, tmp))).collect()),
        other => other.clone(),
    }
}

fn snapshot(dir: &Path) -> Value {
    fn walk(root: &Path, d: &Path, m: &mut Map<String, Value>) {
        let mut entries: Vec<_> = fs::read_dir(d).unwrap().map(|e| e.unwrap().path()).collect();
        entries.sort();
        for p in entries {
            let rel = p.strip_prefix(root).unwrap().to_str().unwrap().to_string();
            if p.is_dir() {
                m.insert(format!("{rel}\\"), Value::Null);
                walk(root, &p, m);
            } else {
                m.insert(rel, Value::String(fs::read_to_string(&p).unwrap()));
            }
        }
    }
    if !dir.exists() {
        return Value::Null;
    }
    let mut m = Map::new();
    walk(dir, dir, &mut m);
    Value::Object(m)
}

fn fs_message(e: &std::io::Error, syscall: &str, path: &str) -> String {
    updater::fs_error_message(e, syscall, path)
}

// ── the app updater ──────────────────────────────────────────────────────────

struct Run<'a> {
    u: Updater,
    net: Net,
    current: String,
    sc: &'a Value,
}

impl Run<'_> {
    fn check(&mut self) -> Vec<Event> {
        self.u.begin_check();
        let req = Updater::check_request();
        let current = self.current.clone();
        match self.net.get(&req.url, "Update") {
            Err(m) => self.u.check_failed(&m),
            Ok(r) => {
                let json = r.spec["json"].clone();
                self.u.on_release_json(&json, &current)
            }
        }
    }

    /// `download()`; `Err` is the rejected promise.
    fn download(&mut self) -> Result<Vec<Event>, String> {
        let sums_req = self.u.download_begin()?;
        let mut events = Vec::new();
        macro_rules! get {
            ($req:expr) => {
                match self.net.get(&$req.url, "Update") {
                    Ok(r) => r,
                    Err(m) => {
                        events.extend(self.u.download_failed(&m));
                        return Ok(events);
                    }
                }
            };
        }
        let text = get!(sums_req).spec.get("text").and_then(Value::as_str).unwrap_or("").to_string();
        let asset_req = match self.u.download_sums(&text) {
            Ok(r) => r,
            Err(ev) => return Ok(ev),
        };
        let r = get!(asset_req);
        let (bytes, spec) = (r.bytes().to_string(), r.spec.clone());
        let content_length = match spec.get("contentLength") {
            None => Some(bytes.len().to_string()),
            Some(Value::Null) => None,
            Some(v) => Some(v.as_str().unwrap().to_string()),
        };
        self.u.download_head(content_length.as_deref());
        let streamed = spec.get("bytes").is_some() && spec.get("noBody").is_none();
        if streamed {
            let chunks: Vec<u64> = spec["chunks"].as_array().unwrap().iter().map(|c| c.as_u64().unwrap()).collect();
            for i in 0..=chunks.len() {
                if let Some(e) = spec.get("streamError").filter(|e| e["after"] == json!(i)) {
                    events.extend(self.u.download_failed(e["message"].as_str().unwrap()));
                    return Ok(events);
                }
                if i == chunks.len() {
                    break;
                }
                events.extend(self.u.download_chunk(chunks[i] as usize));
            }
        }
        let stage = match self.u.download_finish(spec["sha"].as_str().unwrap()) {
            Ok(s) => s,
            Err(ev) => {
                events.extend(ev);
                return Ok(events);
            }
        };
        match updater::write_staged(&stage, bytes.as_bytes()) {
            Ok(()) => events.extend(self.u.download_written(&stage)),
            Err(e) => events.extend(self.u.download_failed(&fs_message(&e, "open", &stage.file))),
        }
        Ok(events)
    }

    /// `install()`: the spawn and quit it asks for, or the error it throws.
    fn install(&mut self) -> Result<(Value, u64), String> {
        let target = self.sc.get("target").map(Value::as_str);
        let exe = updater::resolve_target_exe(target, self.sc.get("env").and_then(Value::as_str));
        let plan = self.u.install(exe.as_deref())?;
        updater::write_install_script(&plan).map_err(|e| fs_message(&e, "open", &plan.script_path))?;
        assert_eq!(plan.log, "quitting to apply an update");
        Ok((json!({ "cmd": plan.program, "args": plan.args, "opts": { "detached": true, "stdio": "ignore", "windowsHide": true } }), 1))
    }
}

#[test]
fn the_app_updater_behaves_as_electron_does() {
    let g = golden();
    let list = cases(&g, "updater", 400);
    let root = scratch("updater");
    let tmp = root.to_str().unwrap().to_string();
    let mut wrong = Vec::new();
    let mut steps = 0;
    for sc in list {
        let sandbox = root.join(sc["sandbox"].as_str().unwrap());
        fs::create_dir_all(&sandbox).unwrap();
        let sandbox_s = sandbox.to_str().unwrap();
        let update_dir = updater::win_join(sandbox_s, sc.get("dir").and_then(Value::as_str).unwrap_or("upd"));
        let mut run = Run { u: Updater::new(&update_dir), net: Net::new(&sc["routes"]), current: sc["current"].as_str().unwrap().to_string(), sc };
        let no_window = sc.get("noWindow").is_some();
        for want in sc["out"].as_array().unwrap() {
            steps += 1;
            let want = with_tmp(want, &tmp);
            let step = &want["step"];
            if let Some(c) = step.get("current") {
                run.current = c.as_str().unwrap().to_string();
            }
            if let Some(r) = step.get("routes") {
                run.net.routes.extend(r.as_object().unwrap().clone());
            }
            run.net.requests.clear();
            let (mut events, mut threw, mut spawns, mut quits) = (Vec::new(), None, Vec::new(), 0);
            match step["op"].as_str().unwrap() {
                "check" => events = run.check(),
                "download" => match run.download() {
                    Ok(ev) => events = ev,
                    Err(m) => threw = Some(m),
                },
                "install" => match run.install() {
                    Ok((spawn, q)) => (spawns, quits) = (vec![spawn], q),
                    Err(m) => threw = Some(m),
                },
                "cleanup" => run.u.cleanup().unwrap(),
                _ => {}
            }
            if no_window {
                events.clear();
            }
            let got = json!({
                "step": step, "requests": run.net.requests, "events": events_json(&events), "state": serde_json::to_value(run.u.get_state()).unwrap(),
                "threw": threw, "files": snapshot(&sandbox), "spawns": spawns, "quits": quits,
            });
            let mut expect = want.clone();
            expect["requests"] = want_requests(&want["requests"]);
            if got != expect {
                let field = ["requests", "events", "state", "threw", "files", "spawns", "quits"].into_iter().find(|f| got[*f] != expect[*f]).unwrap();
                wrong.push(format!("{} / {step}\n  {field}: rust {}\n  {field}: electron {}", sc["name"], short(&got[field]), short(&expect[field])));
            }
        }
    }
    report("the app updater", steps, wrong);
    assert!(steps > 900);
}

// ── the yt-dlp updater ───────────────────────────────────────────────────────

struct YRun {
    y: YtdlpUpdater,
    net: Net,
    events: Vec<Event>,
    no_window: bool,
    auto: bool,
    current: String,
    now: f64,
    resets: u64,
    fired: bool,
}

/// An operation that has started and is waiting for its fetches (a held step), or finished at once.
enum Started {
    Done(Option<YtdlpStatus>),
    Check,
    Update,
}

impl YRun {
    fn push(&mut self, ev: Vec<Event>) {
        if !self.no_window {
            self.events.extend(ev);
        }
    }

    fn start(&mut self, step: &Value) -> Started {
        match step["op"].as_str().unwrap() {
            "check" => {
                let force = step.get("force").and_then(Value::as_bool).unwrap_or(false);
                match self.y.check_begin(force, self.now, &self.current) {
                    CheckBegin::Return(st) => Started::Done(Some(st)),
                    CheckBegin::Fetch { events, .. } => {
                        self.push(events);
                        Started::Check
                    }
                }
            }
            "update" => self.start_update(),
            "failure" => {
                if self.y.handle_download_failure(step["error"].as_str().unwrap(), self.auto) {
                    // `void this.update()`: it runs to its end on its own, or returns at once when busy.
                    let started = self.start_update();
                    self.finish(started);
                }
                Started::Done(None)
            }
            _ => Started::Done(Some(self.y.status().clone())),
        }
    }

    fn start_update(&mut self) -> Started {
        self.fired = true;
        match self.y.update_begin() {
            UpdateBegin::Busy(st) => Started::Done(Some(st)),
            UpdateBegin::Go { events, .. } => {
                self.push(events);
                Started::Update
            }
        }
    }

    fn finish(&mut self, s: Started) -> Option<YtdlpStatus> {
        match s {
            Started::Done(st) => st,
            Started::Check => {
                let latest = self.latest();
                let (st, ev) = self.y.check_end(latest, self.now, &self.current);
                self.push(ev);
                Some(st)
            }
            Started::Update => {
                let (st, ev) = self.finish_update();
                self.push(ev);
                Some(st)
            }
        }
    }

    fn latest(&mut self) -> Result<yt::Latest, String> {
        let r = self.net.get(&yt::YtdlpUpdater::api_request().url, "Engine update")?;
        yt::latest_release(&r.spec["json"])
    }

    fn finish_update(&mut self) -> (YtdlpStatus, Vec<Event>) {
        let latest = match self.latest() {
            Ok(l) => l,
            Err(m) => return self.y.update_fail(&m),
        };
        let asset = match self.net.get(&latest.asset_request().url, "Engine update") {
            Ok(r) => r.bytes().to_string(),
            Err(m) => return self.y.update_fail(&m),
        };
        let sums = match self.net.get(&latest.sums_request().url, "Engine update") {
            Ok(r) => r.spec.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
            Err(m) => return self.y.update_fail(&m),
        };
        let actual = self.net.routes[&latest.asset_request().url]["sha"].as_str().unwrap().to_string();
        let swap = match self.y.update_verify(&sums, &actual) {
            Ok(s) => s,
            Err(m) => return self.y.update_fail(&m),
        };
        if let Err(e) = yt::write_swap(&swap, asset.as_bytes()) {
            return self.y.update_fail(&fs_message(&e, "open", &swap.tmp));
        }
        self.resets += 1;
        self.y.update_done(&latest.tag, &self.current, self.now)
    }
}

#[test]
fn the_engine_updater_behaves_as_electron_does() {
    let g = golden();
    let list = cases(&g, "ytdlp", 250);
    let root = scratch("ytdlp");
    let tmp = root.to_str().unwrap().to_string();
    let mut wrong = Vec::new();
    let mut steps = 0;
    for sc in list {
        let sandbox = root.join(sc["sandbox"].as_str().unwrap());
        fs::create_dir_all(&sandbox).unwrap();
        let bin_dir = updater::win_join(sandbox.to_str().unwrap(), "bin");
        let mut run = YRun {
            y: YtdlpUpdater::new(&bin_dir),
            net: Net::new(&sc["routes"]),
            events: Vec::new(),
            no_window: sc.get("noWindow").is_some(),
            auto: sc.get("auto").and_then(Value::as_bool).unwrap_or(true),
            current: sc["current"].as_str().unwrap().to_string(),
            now: 0.0,
            resets: 0,
            fired: false,
        };
        for want in sc["out"].as_array().unwrap() {
            steps += 1;
            let want = with_tmp(want, &tmp);
            let step = &want["step"];
            if let Some(c) = step.get("current") {
                run.current = c.as_str().unwrap().to_string();
            }
            if let Some(r) = step.get("routes") {
                run.net.routes.extend(r.as_object().unwrap().clone());
            }
            if let Some(n) = step.get("now") {
                run.now = n.as_f64().unwrap();
            }
            if let Some(a) = step.get("auto") {
                run.auto = a.as_bool().unwrap();
            }
            run.net.requests.clear();
            run.events.clear();
            (run.resets, run.fired) = (0, false);
            let status_json = |s: Option<YtdlpStatus>| s.map_or(Value::Null, |s| serde_json::to_value(s).unwrap());
            let mut during = Vec::new();
            let result = if step.get("hold").is_some() {
                let held = run.start(step);
                for inner in step.get("during").and_then(Value::as_array).into_iter().flatten() {
                    let inner = if inner.is_string() { json!({ "op": inner }) } else { inner.clone() };
                    let started = run.start(&inner);
                    let r = run.finish(started);
                    during.push(json!({ "step": inner, "result": status_json(r), "fired": run.fired }));
                }
                status_json(run.finish(held))
            } else {
                let started = run.start(step);
                status_json(run.finish(started))
            };
            let got = json!({
                "step": step, "result": result, "during": during, "requests": run.net.requests, "events": events_json(&run.events),
                "status": serde_json::to_value(run.y.status()).unwrap(), "files": snapshot(&sandbox), "resets": run.resets, "fired": run.fired,
            });
            let mut expect = want.clone();
            expect["requests"] = want_requests(&want["requests"]);
            if got != expect {
                let field = ["result", "during", "requests", "events", "status", "files", "resets", "fired"].into_iter().find(|f| got[*f] != expect[*f]).unwrap();
                wrong.push(format!("{} / {step}\n  {field}: rust {}\n  {field}: electron {}", sc["name"], short(&got[field]), short(&expect[field])));
            }
        }
    }
    report("the engine updater", steps, wrong);
    assert!(steps > 400);
}

#[test]
fn requests_carry_only_the_user_agent() {
    let r = Request::new("https://github.com/x");
    assert_eq!((r.url.as_str(), r.user_agent), ("https://github.com/x", "AHG-Universal-Converter"));
}
