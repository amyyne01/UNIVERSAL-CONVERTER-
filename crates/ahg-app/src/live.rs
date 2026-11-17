//! Live mode (`--live <folder>`, decision 11): the runtime `Core`, started on a
//! scratch folder with the licence feature off, fills `State`. Sample mode stays
//! for `--scene`.
//!
//! No thread here runs on a timer. A relay turns each burst of `CoreEvent`s into
//! one fresh read of the core; one command thread runs the actions' calls in
//! order; each lookup has a thread of its own; a startup thread waits out the
//! three launch delays (K22) on a channel. Each sends a [`Job`] and wakes Rune;
//! [`poll`] applies the jobs and says whether `State` changed.
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, OnceLock, Weak};
use std::thread;
use std::time::Instant;

use ahg_engine::api::{build_request, retry_request};
use ahg_engine::queue::{TaskInit, create_task};
use ahg_engine::text::js_len;
use ahg_engine::tier::Plan;
use ahg_engine::types::{ContentType, MediaMetadata, Playlist, SearchResult, SourcePlatform, Track, UrlDetection};
use ahg_runtime::core::{APP_UPDATE_DELAY, Core, CoreEvent, ENGINE_UPDATE_DELAY, Paths, PlanSource, TrackMatch, WARM_UP_DELAY};
use ahg_runtime::unpack::tool_dirs;
use ahg_runtime::ytdlp::{Tools, YtDlp};
use rune::{Tx, Waker};
use serde_json::{Value, json};

use crate::actions;
use crate::copy::{self, found as fc};
use crate::gate::{Band, Card, Found, Loading, Matched, PRESETS, State, Tone};

/// The names the Electron app's data folder has had under `%APPDATA%`. The
/// Rust app shares that folder, so `--live` refuses all of them.
pub const REFUSED_NAMES: [&str; 3] = ["ahg-universal-converter", "ahg-converter", "AHG Converter"];

/// Without the licence feature the plan is Basic, as on an unactivated copy.
const PLAN: Plan = Plan::Basic;

/// The folder `--live` named, checked in `main` before the window opens.
pub static FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// Something for the UI thread to do to `State`; true when it changed what the view reads.
pub type Job = Box<dyn FnOnce(&mut State, &mut Tx) -> bool + Send>;
type Command = Box<dyn FnOnce(&Core) -> Option<Job> + Send>;

/// The running core, as `State::live` holds it.
pub struct Handle {
    core: Arc<Core>,
    inbox: Receiver<Job>,
    outbox: Sender<Job>,
    wake: Waker,
    commands: Option<Sender<Command>>,
    /// Dropped first at shutdown: the startup thread stops waiting.
    stop: Option<Sender<()>>,
    /// Bumped by every cancel; a lookup answer from before it is dropped (LESSONS 17b).
    generation: u64,
    /// The preset and the pin as last saved; `None` until `settings-rust.json` was read.
    saved: Option<(usize, bool)>,
}

impl Drop for Handle {
    /// Stop, then let go: a lookup thread may still hold the core, so it is shut
    /// down here rather than by whichever thread drops it last (LESSONS 17a).
    fn drop(&mut self) {
        drop((self.stop.take(), self.commands.take()));
        self.core.cancel_lookups();
        self.core.shutdown();
    }
}

// ── The folder

/// `--live <folder>` from the command line: `None` without the flag, else the
/// checked folder or why it is refused. It never defaults.
pub fn folder_from(args: &[String]) -> Option<Result<PathBuf, String>> {
    let at = args.iter().position(|a| a == "--live")?;
    Some(match args.get(at + 1).filter(|v| !v.starts_with("--")) {
        None => Err("--live needs a folder".into()),
        Some(_) if args.iter().any(|a| a == "--scene") => Err("--live and --scene do not mix".into()),
        Some(folder) => match std::env::current_dir() {
            Ok(cwd) => check(Path::new(folder), std::env::var_os("APPDATA").map(PathBuf::from).as_deref(), &cwd),
            Err(_) => Err("no working folder to resolve it from".into()),
        },
    })
}

/// The folder as the file system names it, or why it is refused: it must
/// exist, and it may not be, hold or lie inside the app's real data folder, in
/// any spelling: case, trailing dots or spaces, `.` and `..`, a relative path,
/// a symlink or junction. Nothing is created before the check.
pub fn check(folder: &Path, appdata: Option<&Path>, cwd: &Path) -> Result<PathBuf, String> {
    let appdata = appdata.filter(|a| a.is_absolute()).ok_or("%APPDATA% is not set, so the real data folder cannot be ruled out")?;
    // `join` keeps an absolute folder as it is.
    let folder = cwd.join(folder);
    let real = std::fs::canonicalize(&folder).ok().filter(|p| p.is_dir()).ok_or("not an existing folder")?;
    let names: Vec<String> = REFUSED_NAMES.iter().map(|n| fold(n)).collect();
    let mut bases = vec![parts(appdata)];
    bases.extend(std::fs::canonicalize(appdata).ok().map(|a| parts(&a)));
    // The real folders, where they exist, as the file system resolves them (a junction followed).
    let targets: Vec<Vec<String>> =
        REFUSED_NAMES.iter().filter_map(|n| std::fs::canonicalize(appdata.join(n)).ok()).map(|p| parts(&p)).collect();
    for p in [parts(&folder), parts(&real)] {
        let named = p.iter().any(|c| names.contains(c));
        let above = bases.iter().any(|b| b.starts_with(&p));
        let around = targets.iter().any(|t| p.starts_with(t) || t.starts_with(&p));
        if named || above || around {
            return Err("that is, holds or lies inside the app's real data folder".into());
        }
    }
    Ok(real)
}

/// The path's components, compared as Windows compares names: `\\?\C:` is `c:`,
/// case is ignored, trailing dots and spaces are dropped, `.` and `..` are applied.
fn parts(p: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for c in p.components() {
        match c {
            Component::Prefix(x) => out.push(match x.kind() {
                Prefix::Disk(d) | Prefix::VerbatimDisk(d) => format!("{}:", char::from(d).to_ascii_lowercase()),
                Prefix::UNC(host, share) | Prefix::VerbatimUNC(host, share) => {
                    format!(r"\\{}\{}", fold(&host.to_string_lossy()), fold(&share.to_string_lossy()))
                }
                _ => fold(&x.as_os_str().to_string_lossy()),
            }),
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                if out.len() > 1 {
                    out.pop();
                }
            }
            Component::Normal(n) => out.push(fold(&n.to_string_lossy())),
        }
    }
    out
}

fn fold(name: &str) -> String {
    name.trim_end_matches(['.', ' ']).to_lowercase()
}

// ── Starting and stopping

/// Starts the core on `folder` with the real tools (the self-updated yt-dlp,
/// the unpacked bundle, and in a development build `bin` beside the program or
/// in the working folder) and fills `s` from it.
pub fn start(s: &mut State, folder: &Path, wake: Waker) {
    let tools = Arc::new(Tools::new(tool_dirs(folder, &folder.join("tools"))));
    attach(s, folder, wake, move |paths, plan, events| Core::start(paths, Arc::new(YtDlp::from_tools(tools)), plan, events));
}

/// [`start`], with the core made by `make` (the tests give stand-ins). Downloads
/// go to `Downloads` inside the folder, never the user's own.
pub fn attach(s: &mut State, folder: &Path, wake: Waker, make: impl FnOnce(Paths, PlanSource, Sender<CoreEvent>) -> Core) {
    let downloads = folder.join("Downloads");
    let _ = std::fs::create_dir_all(&downloads);
    let (events_tx, events) = channel();
    let paths = Paths { user_data: folder.to_owned(), downloads: downloads.to_string_lossy().into_owned() };
    let core = Arc::new(make(paths, Box::new(|| PLAN), events_tx));
    let (outbox, inbox) = channel();
    let (commands, queue) = channel();
    let (stop, stopped) = channel();
    relay(Arc::downgrade(&core), events, outbox.clone(), wake.clone());
    serve(Arc::downgrade(&core), queue, outbox.clone(), wake.clone());
    startup(Arc::downgrade(&core), stopped);
    // Nothing of the samples stays; the first read of the core fills the rest.
    s.tasks.clear();
    s.stats = None;
    s.covers.clear();
    s.speed.clear();
    s.selected.clear();
    s.picked.clear();
    s.found = Found::Nothing;
    s.notice = None;
    s.plan = PLAN;
    s.loading = Loading::Booting;
    s.settings = core.settings();
    s.engine = core.engine_status();
    s.update = core.update_status();
    s.live = Some(Handle { core, inbox, outbox, wake, commands: Some(commands), stop: Some(stop), generation: 0, saved: None });
    s.sync();
}

fn send(out: &Sender<Job>, wake: &Waker, job: Job) {
    if out.send(job).is_ok() {
        wake();
    }
}

/// The settings-rust.json choices first, then a fresh read on every burst of events.
fn relay(core: Weak<Core>, events: Receiver<CoreEvent>, out: Sender<Job>, wake: Waker) {
    thread::spawn(move || {
        if let Some(core) = core.upgrade() {
            send(&out, &wake, own(&core));
        }
        loop {
            // Never held while waiting: the core must be free to go at shutdown.
            let Some(held) = core.upgrade() else { break };
            send(&out, &wake, fresh(&held));
            drop(held);
            if events.recv().is_err() {
                break;
            }
            while events.try_recv().is_ok() {}
        }
    });
}

/// The actions' calls, one at a time and in order, each followed by a fresh
/// read, so a refusal puts the list back as the core has it.
fn serve(core: Weak<Core>, commands: Receiver<Command>, out: Sender<Job>, wake: Waker) {
    thread::spawn(move || {
        for command in commands {
            let Some(core) = core.upgrade() else { break };
            if let Some(job) = command(&core) {
                send(&out, &wake, job);
            }
            send(&out, &wake, fresh(&core));
        }
    });
}

/// K22: after launch, the tools are resolved, then the app update check runs,
/// then yt-dlp's own update, each at its delay from launch.
fn startup(core: Weak<Core>, stop: Receiver<()>) {
    thread::spawn(move || {
        let launch = Instant::now();
        for (step, at) in [WARM_UP_DELAY, APP_UPDATE_DELAY, ENGINE_UPDATE_DELAY].into_iter().enumerate() {
            if stop.recv_timeout(at.saturating_sub(launch.elapsed())) != Err(RecvTimeoutError::Timeout) {
                return;
            }
            let Some(core) = core.upgrade() else { return };
            match step {
                0 => core.warm_up(),
                1 => drop(core.check_update()),
                _ => core.auto_update_engine(),
            }
        }
    });
}

/// The core as it is now, for `State` to follow.
fn fresh(core: &Core) -> Job {
    let mut tasks = core.tasks();
    tasks.reverse();
    let stats = Some(core.stats()).filter(|st| st.current.files > 0.0 || st.previous.is_some());
    let (settings, engine, update) = (core.settings(), core.engine_status(), core.update_status());
    Box::new(move |s, _| {
        let changed = [
            put(&mut s.tasks, tasks),
            put(&mut s.stats, stats),
            put(&mut s.settings, settings),
            put(&mut s.engine, engine),
            put(&mut s.update, update),
            put(&mut s.loading, Loading::Ready),
        ];
        changed.contains(&true)
    })
}

fn put<T: PartialEq>(slot: &mut T, value: T) -> bool {
    let changed = *slot != value;
    if changed {
        *slot = value;
    }
    changed
}

/// The Rust-only choices (K52), read once at start.
fn own(core: &Core) -> Job {
    let preset = core.own_setting("preset").and_then(|v| v.as_u64()).map(|p| p as usize).filter(|p| *p < PRESETS.len());
    let pinned = core.own_setting("navPinned").and_then(|v| v.as_bool());
    let hold = core.hold_unconfirmed();
    Box::new(move |s, tx| {
        if let Some(preset) = preset {
            actions::set_preset(s, preset, tx);
        }
        s.nav_pinned = pinned.unwrap_or(s.nav_pinned);
        s.own.hold_unconfirmed = hold;
        s.sync();
        let now = (s.own.preset, s.own.nav_pinned);
        if let Some(live) = s.live.as_mut() {
            live.saved = Some(now);
        }
        true
    })
}

// ── The UI thread's side

/// Rune's poll: applies what arrived and fires due deadlines. True only when `State` changed.
pub fn poll(s: &mut State, clock: f64, tx: &mut Tx) -> bool {
    drain(s, clock, tx).1
}

/// [`poll`], also counting the jobs applied.
fn drain(s: &mut State, clock: f64, tx: &mut Tx) -> (usize, bool) {
    let Some(live) = &s.live else { return (0, false) };
    let jobs: Vec<Job> = live.inbox.try_iter().collect();
    let mut changed = actions::tick(s, clock);
    let count = jobs.len();
    for job in jobs {
        changed |= job(s, tx);
    }
    if changed {
        s.sync();
    }
    save_own(s);
    (count, changed)
}

/// K52: the preset and the pin, written to `settings-rust.json` when they change.
fn save_own(s: &mut State) {
    let now = (s.own.preset, s.own.nav_pinned);
    let Some(live) = s.live.as_mut().filter(|l| l.saved.is_some_and(|saved| saved != now)) else { return };
    live.saved = Some(now);
    command(s, move |core| {
        core.set_own_setting("preset", json!(now.0));
        core.set_own_setting("navPinned", json!(now.1));
        None
    });
}

/// Runs a call into the core on the command thread. Nothing when not live.
pub fn command(s: &State, work: impl FnOnce(&Core) -> Option<Job> + Send + 'static) {
    if let Some(commands) = s.live.as_ref().and_then(|l| l.commands.as_ref()) {
        let _ = commands.send(Box::new(work));
    }
}

/// [`command`] for a call whose answer changes nothing on screen.
pub fn tell(s: &State, work: impl FnOnce(&Core) + Send + 'static) {
    command(s, move |core| {
        work(core);
        None
    });
}

/// A slow call (the network) on a thread of its own, so the command thread is
/// never held up; what it changes arrives as the core's events.
pub fn spawn(s: &State, work: impl FnOnce(&Core) + Send + 'static) {
    if let Some(core) = s.live.as_ref().map(|l| l.core.clone()) {
        thread::spawn(move || work(&core));
    }
}

/// A notice from a worker's answer.
fn notice(tone: Tone, text: String) -> Job {
    Box::new(move |s, _| {
        actions::notify(s, tone, text, None);
        true
    })
}

// ── Lookups

/// Runs a lookup on a thread of its own. `work` may send several answers; each
/// is dropped if a cancel came after the lookup began.
fn lookup(s: &State, work: impl FnOnce(&Core, &dyn Fn(Job)) + Send + 'static) {
    let Some(live) = &s.live else { return };
    let (core, out, wake, generation) = (live.core.clone(), live.outbox.clone(), live.wake.clone(), live.generation);
    thread::spawn(move || {
        let answer = |job: Job| {
            let current: Job = Box::new(move |s, tx| s.live.as_ref().is_some_and(|l| l.generation == generation) && job(s, tx));
            send(&out, &wake, current);
        };
        work(&core, &answer);
    });
}

/// Stops the lookups running and makes their late answers count for nothing.
pub fn cancel_lookups(s: &mut State) {
    if let Some(live) = s.live.as_mut() {
        live.generation += 1;
        live.core.cancel_lookups();
    }
}

/// One pasted link, read as Electron's platform tab read it.
pub fn look_up(s: &mut State, det: UrlDetection) {
    cancel_lookups(s);
    lookup(s, move |core, answer| {
        let (found, card) = find(core, &det);
        let spotify =
            matches!(&found, Found::Playlist { playlist, .. } if det.platform == SourcePlatform::Spotify && !playlist.tracks.is_empty());
        let tracks = if let Found::Playlist { playlist, .. } = &found { playlist.tracks.clone() } else { Vec::new() };
        answer(Box::new(move |s, _| {
            s.found = found;
            s.card = card;
            true
        }));
        // D34: each Spotify track's match, while the list is already on screen.
        for (i, track) in tracks.iter().enumerate().filter(|_| spotify) {
            let matched = match core.match_track(track) {
                TrackMatch::Found(m) if m.confidence >= 1.0 => Matched::Confirmed,
                TrackMatch::Found(m) => Matched::BestGuess(m.confidence),
                // ponytail: Matched has no "unreachable"; a failed search reads as no match until it does.
                TrackMatch::NoSong | TrackMatch::Unreachable => Matched::NoMatch,
            };
            answer(Box::new(move |s, _| match &mut s.found {
                Found::Playlist { matched: row, .. } if i < row.len() => {
                    row[i] = matched;
                    true
                }
                _ => false,
            }));
        }
    });
}

/// What a link holds (PlatformTab's `handleSubmit`).
fn find(core: &Core, det: &UrlDetection) -> (Found, Card) {
    use SourcePlatform::*;
    let failed = |text: String| (Found::LookupFailed(text), Card::default());
    if det.platform == Spotify {
        let collection = matches!(det.content_type, ContentType::Playlist | ContentType::Album);
        if collection {
            let read =
                if det.content_type == ContentType::Playlist { core.spotify_playlist(&det.url) } else { core.spotify_album(&det.url) };
            return match read {
                Ok(Some(playlist)) => (playlist_found(playlist, false), Card::default()),
                Ok(None) => failed(fc::ERROR_SPOTIFY_COLLECTION.into()),
                Err(_) => failed(unknown()),
            };
        }
        return match core.spotify_track(&det.url) {
            Ok(Some(t)) => (Found::Media(track_media(&t, &det.url)), Card::default()),
            Ok(None) => failed(fc::ERROR_SPOTIFY_TRACK.into()),
            Err(_) => failed(unknown()),
        };
    }
    let meta = core.fetch_metadata(&det.url);
    if det.is_collection {
        return match meta {
            Ok(m) if det.platform == Youtube && !m.entries.is_empty() => (playlist_found(youtube_playlist(det, m), true), Card::default()),
            Ok(m) if m.entries.is_empty() => failed(fc::EMPTY_COLLECTION.into()),
            Ok(m) => {
                let mut bands = [Band::Empty, Band::Empty, Band::Empty];
                bands[actions::band_of(det.platform)] = Band::Results(m.entries);
                (Found::Search { query: m.title, bands }, Card::default())
            }
            Err(e) => failed(reason(&e)),
        };
    }
    let short = matches!(det.platform, Instagram | Tiktok | Facebook);
    match meta {
        Ok(m) if short && m.thumbnail_url.is_empty() => failed(fc::ERROR_SHORT_NO_PREVIEW.into()),
        Ok(m) => (Found::Media(m), if short { Card::Short { label: det.label.clone() } } else { Card::default() }),
        Err(e) if short && !fits(&e) => failed(fc::ERROR_SHORT_NO_PREVIEW.into()),
        Err(e) => failed(reason(&e)),
    }
}

/// `ipcReason`: the engine's sentence when it is 3 to 199 characters, else the generic one.
fn fits(text: &str) -> bool {
    (3..200).contains(&js_len(text))
}

fn reason(text: &str) -> String {
    if fits(text) { text.to_owned() } else { unknown() }
}

fn unknown() -> String {
    copy::error_text(copy::ERR_UNKNOWN_CAUSE, copy::ERR_UNKNOWN_REMEDY, false)
}

/// A playlist to pick from: YouTube's start all ticked, Spotify's none (Electron's store).
fn playlist_found(playlist: Playlist, all: bool) -> Found {
    let n = playlist.tracks.len();
    let matched = vec![if all { Matched::Confirmed } else { Matched::Matching }; n];
    Found::Playlist { playlist, matched, selected: vec![all; n] }
}

fn youtube_playlist(det: &UrlDetection, m: MediaMetadata) -> Playlist {
    let tracks = m
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| Track {
            id: e.id.clone(),
            name: e.title.clone(),
            artist: e.uploader.clone(),
            artists: vec![e.uploader.clone()],
            album: String::new(),
            album_year: None,
            track_number: (i + 1) as f64,
            duration_ms: e.duration * 1000.0,
            thumbnail_url: e.thumbnail_url.clone(),
            spotify_url: None,
            explicit: None,
        })
        .collect::<Vec<_>>();
    Playlist {
        id: det.id.clone().unwrap_or_else(|| det.url.clone()),
        name: m.title,
        description: String::new(),
        owner: m.uploader,
        track_count: tracks.len() as f64,
        tracks,
        thumbnail_url: m.thumbnail_url,
        url: det.url.clone(),
    }
}

/// A Spotify track as the single card shows it (`trackToItem`).
fn track_media(t: &Track, url: &str) -> MediaMetadata {
    let card = json!({
        "title": t.name, "uploader": t.artist, "duration": (t.duration_ms / 1000.0).round(), "thumbnailUrl": t.thumbnail_url,
        "url": t.spotify_url.clone().unwrap_or_else(|| url.to_owned()), "isCollection": false, "entries": [],
    });
    serde_json::from_value(card).unwrap_or_else(|_| unreachable!("a MediaMetadata's required fields are all given"))
}

fn track_row(t: &Track) -> SearchResult {
    SearchResult {
        id: t.id.clone(),
        title: t.name.clone(),
        uploader: t.artist.clone(),
        duration: (t.duration_ms / 1000.0).round(),
        thumbnail_url: t.thumbnail_url.clone(),
        url: t.spotify_url.clone().unwrap_or_default(),
        source: SourcePlatform::Spotify,
    }
}

/// Words typed: each band asked searches its source (0 YouTube, 1 SoundCloud, 2 Spotify).
pub fn search(s: &mut State, query: &str, asked: [bool; 3]) {
    cancel_lookups(s);
    for (band, _) in asked.iter().enumerate().filter(|(_, ask)| **ask) {
        search_band(s, query, band);
    }
}

/// One band's search, asked again by its Retry too.
pub fn search_band(s: &State, query: &str, band: usize) {
    let query = query.to_owned();
    lookup(s, move |core, answer| {
        let rows = match band {
            0 => core.search_youtube(&query),
            1 => core.search_soundcloud(&query),
            _ => Ok(core.spotify_search(&query).iter().map(track_row).collect()),
        };
        let result = match rows {
            Ok(rows) if rows.is_empty() => Band::Empty,
            Ok(rows) => Band::Results(rows),
            Err(_) => Band::Failed,
        };
        answer(Box::new(move |s, _| match &mut s.found {
            Found::Search { bands, .. } => {
                bands[band] = result;
                true
            }
            _ => false,
        }));
    });
}

// ── Downloads

/// Queues `init` in the core, as Electron's `buildDownloadRequest` builds it. False when not live.
pub fn start_download(s: &State, init: &TaskInit) -> bool {
    if s.live.is_none() {
        return false;
    }
    let Value::Object(mut overrides) = retry_request(&create_task(init, String::new(), 0), None) else { return true };
    // An absent field, not a null one, as the object spread sent it.
    overrides.retain(|_, v| !v.is_null());
    let request = build_request(Some(&Value::Object(s.settings.clone())), &overrides);
    command(s, move |core| core.start_download(&request).err().map(|_| notice(Tone::Error, fc::ERROR_START_FAILED.into())));
    true
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, Instant};

    use ahg_engine::types::DownloadStatus as St;
    use ahg_runtime::core::Outside;
    use rune::{App, Env, Runtime};

    use super::*;

    const LINK: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    const RECORD: &str = r#"{"id":"dQw4w9WgXcQ","title":"A Song","uploader":"Someone","duration":212,"webpage_url":"https://www.youtube.com/watch?v=dQw4w9WgXcQ"}"#;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ahg-app-live-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Downloads")).unwrap();
        dir
    }

    /// The stand-in yt-dlp, built by `cargo build -p ahg-runtime --bin fake_ytdlp` beside this test's folder.
    fn fake() -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        let fake = exe.parent().and_then(Path::parent).unwrap().join("fake_ytdlp.exe");
        assert!(fake.exists(), "build the stand-in first: cargo build -p ahg-runtime --bin fake_ytdlp ({})", fake.display());
        fake
    }

    /// The real spec, headless, live on `dir` with the stand-ins: lookups play `json`, downloads `script`.
    fn live(dir: &Path, json: &str, script: &str) -> Runtime<State> {
        fs::write(dir.join("json.txt"), json).unwrap();
        fs::write(dir.join("script.txt"), script).unwrap();
        let mut ytdlp = YtDlp::new(fake(), None);
        ytdlp.env("FAKE_YTDLP_SCRIPT_JSON", dir.join("json.txt").to_str().unwrap());
        ytdlp.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
        ytdlp.env("FAKE_YTDLP_ARGS", dir.join("args.txt").to_str().unwrap());
        let ytdlp = Arc::new(ytdlp);
        let mut rt = App::new(crate::spec()).headless_runtime((1240.0, 820.0), Env { system_dark: true, scale: 1.0 });
        rt.frame(0.0);
        attach(&mut rt.state, dir, Arc::new(|| {}), move |paths, plan, events| {
            Core::start_with(paths, ytdlp, plan, events, Outside::standin())
        });
        until(&mut rt, "the first read of the core", |s| s.loading == Loading::Ready);
        rt
    }

    fn until(rt: &mut Runtime<State>, what: &str, done: impl Fn(&State) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            rt.frame(1.0 / 60.0);
            if done(&rt.state) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("never saw {what}");
    }

    #[test]
    fn a_link_submitted_live_reaches_done() {
        let dir = scratch("done");
        let media = dir.join("Downloads").join("a.mp3");
        fs::write(&media, vec![0u8; 2048]).unwrap();
        let mut rt = live(&dir, &format!("out {RECORD}\nexit 0\n"), &format!("out [FILEPATH]{}\nexit 0\n", media.display()));
        assert!(rt.state.tasks.is_empty(), "no sample task is left");
        rt.update(|s, tx| {
            s.set_field(LINK);
            actions::submit(s, tx);
        });
        until(&mut rt, "the lookup's answer", |s| matches!(&s.found, Found::Media(m) if m.title == "A Song"));
        rt.update(|s, _| actions::download_found(s));
        until(&mut rt, "the download to finish", |s| s.tasks.iter().any(|t| t.progress.status == St::Done));
        assert_eq!(rt.state.tasks.len(), 1);
        assert_eq!(rt.state.tasks[0].title, "A Song");
        assert!(rt.state.tasks[0].task_id.starts_with("task_"), "the row is the core's own");
        rt.state.live = None;
    }

    #[test]
    fn poll_is_true_only_when_state_changed() {
        // Sample mode: nothing to drain.
        let mut sample = State::sample(0);
        assert!(!poll(&mut sample, 1.0, &mut Tx::default()));

        let dir = scratch("poll");
        let mut rt = live(&dir, "exit 0\n", "exit 0\n");
        let core = rt.state.live.as_ref().unwrap().core.clone();
        let mut tx = Tx::default();
        // An echo: the event comes and the fresh read equals State.
        core.update_settings(&serde_json::Map::new());
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = 0;
        while seen == 0 && Instant::now() < deadline {
            let (count, changed) = drain(&mut rt.state, 0.0, &mut tx);
            assert!(!changed, "an unchanged read must not count as a change");
            seen += count;
        }
        assert!(seen > 0, "the echo never arrived");
        // A real change.
        let flip = !rt.state.settings["showNotifications"].as_bool().unwrap();
        core.update_settings(&serde_json::Map::from_iter([("showNotifications".to_owned(), json!(flip))]));
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut changed = false;
        while !changed && Instant::now() < deadline {
            changed = drain(&mut rt.state, 0.0, &mut tx).1;
        }
        assert!(changed && rt.state.settings["showNotifications"] == json!(flip));
        drop(core);
        rt.state.live = None;
    }

    #[test]
    fn a_late_lookup_answer_after_a_cancel_is_dropped() {
        let dir = scratch("late");
        let mut rt = live(&dir, &format!("sleep 1500\nout {RECORD}\nexit 0\n"), "exit 0\n");
        rt.update(|s, tx| {
            s.set_field(LINK);
            actions::submit(s, tx);
        });
        assert!(matches!(rt.state.found, Found::LookingUp));
        rt.update(|s, _| actions::dismiss_found(s));
        // The cancelled lookup answers "Cancelled." within moments; it must not show.
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            rt.frame(1.0 / 60.0);
            assert!(matches!(rt.state.found, Found::Nothing), "a cancelled lookup's answer was shown");
            thread::sleep(Duration::from_millis(20));
        }
        rt.state.live = None;
    }

    #[test]
    fn shutdown_cancels_the_lookups_before_letting_go_of_the_core() {
        let dir = scratch("shutdown");
        // A lookup that would hang for a minute, and holds the core meanwhile.
        let mut rt = live(&dir, &format!("count {}\nsleep 60000\nexit 0\n", dir.join("count").display()), "exit 0\n");
        rt.update(|s, tx| {
            s.set_field(LINK);
            actions::submit(s, tx);
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        while !dir.join("count").exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(dir.join("count").exists(), "the lookup never started");
        let began = Instant::now();
        rt.state.live = None;
        assert!(began.elapsed() < Duration::from_secs(10), "shutdown waited for the hung lookup");
        assert!(dir.join("config.json").exists(), "the settings were saved at shutdown");
    }

    #[test]
    fn the_preset_and_the_pin_come_from_and_go_to_settings_rust_json() {
        let dir = scratch("own");
        fs::write(dir.join("settings-rust.json"), r#"{"preset":4,"navPinned":true,"holdUnconfirmed":true}"#).unwrap();
        let mut rt = live(&dir, "exit 0\n", "exit 0\n");
        until(&mut rt, "the saved preset", |s| crate::gate::preset(s) == 4 && s.nav_pinned);
        assert!(rt.state.own.hold_unconfirmed);
        rt.update(|s, _| actions::toggle_pin(s));
        let file = dir.join("settings-rust.json");
        let read = || serde_json::from_str::<Value>(&fs::read_to_string(&file).unwrap_or_default()).unwrap_or_default();
        until(&mut rt, "the pin written back", |_| read()["navPinned"] == json!(false));
        assert_eq!(read(), json!({ "preset": 4, "navPinned": false, "holdUnconfirmed": true }), "other keys are kept");
        rt.state.live = None;
    }

    // ── The refused folders. The real %APPDATA% names appear only as strings.

    fn refused(r: Result<PathBuf, String>) -> bool {
        r.is_err_and(|why| why.contains("real data folder"))
    }

    #[test]
    fn every_name_of_the_real_data_folder_is_refused_as_a_string() {
        let appdata = Path::new(r"C:\Users\u\AppData\Roaming");
        let names: Vec<String> = REFUSED_NAMES.iter().map(|n| fold(n)).collect();
        assert_eq!(names.len(), 3);
        for name in REFUSED_NAMES {
            for spelling in [name.to_owned(), name.to_uppercase(), format!("{name}."), format!("{name} "), format!(r"{name}\sub")] {
                let p = parts(&appdata.join(&spelling));
                assert!(p.iter().any(|c| names.contains(c)), "{spelling} is not seen as the data folder");
            }
        }
        // `..` and `.` are applied before comparing; the verbatim prefix is the drive.
        let p = parts(Path::new(r"\\?\C:\Users\u\AppData\Roaming\x\..\.\AHG Converter"));
        assert_eq!(p, parts(&appdata.join("ahg converter")));
        // %APPDATA% and everything above it hold the data folder.
        let base = parts(appdata);
        for above in [r"C:\", r"C:\Users", r"c:\users\U\appdata", r"C:\Users\u\AppData\Roaming\"] {
            assert!(base.starts_with(&parts(Path::new(above))), "{above}");
        }
        assert!(!base.starts_with(&parts(Path::new(r"C:\Users\u\AppData\Local\Temp"))));
    }

    #[test]
    fn the_folder_check_refuses_every_form_on_disk() {
        let root = scratch("refuse");
        let appdata = root.join("AppData").join("Roaming");
        let real = appdata.join("ahg-converter");
        fs::create_dir_all(real.join("sub")).unwrap();
        let elsewhere = root.join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let cwd = std::env::current_dir().unwrap();
        let ok = |p: &Path| check(p, Some(&appdata), &cwd);

        assert!(ok(&elsewhere).is_ok(), "a plain scratch folder is accepted");
        assert!(ok(&root.join("missing")).is_err(), "a missing folder is refused, and not created");
        assert!(!root.join("missing").exists());
        assert!(check(&elsewhere, None, &cwd).is_err(), "no %APPDATA%: refused");
        for p in [
            real.clone(),
            real.join("sub"),
            appdata.join("AHG-CONVERTER"),
            PathBuf::from(format!("{}\\", real.display())),
            appdata.join("x").join("..").join("ahg-converter"),
            appdata.clone(),
            appdata.parent().unwrap().to_owned(),
        ] {
            assert!(refused(ok(&p)), "{} was accepted", p.display());
        }
        // A relative path, from the working folder.
        let up = cwd.components().count() - 1;
        let relative: PathBuf = std::iter::repeat_n("..", up).collect::<PathBuf>().join(real.components().skip(1).collect::<PathBuf>());
        assert!(refused(ok(&relative)), "{} was accepted", relative.display());
        // A junction to the real folder, and a real folder that is itself a junction.
        let link = root.join("link");
        let made = std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&link).arg(&real).output().unwrap();
        assert!(made.status.success(), "mklink failed");
        assert!(refused(ok(&link)), "a junction to the real folder was accepted");
        assert!(refused(ok(&link.join("sub"))));
        let appdata2 = root.join("AppData2");
        fs::create_dir_all(&appdata2).unwrap();
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(appdata2.join("AHG Converter"))
            .arg(&elsewhere)
            .output()
            .unwrap();
        assert!(made.status.success(), "mklink failed");
        assert!(refused(check(&elsewhere, Some(&appdata2), &cwd)), "the folder a junctioned data folder points to was accepted");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_flag_needs_a_folder_and_does_not_mix_with_scenes() {
        let args = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<String>>();
        assert!(folder_from(&args(&["app"])).is_none());
        assert!(folder_from(&args(&["app", "--live"])).unwrap().is_err());
        assert!(folder_from(&args(&["app", "--live", "--snap", "x.png"])).unwrap().is_err());
        let dir = scratch("flag");
        let d = dir.to_string_lossy().into_owned();
        assert!(folder_from(&args(&["app", "--live", &d, "--scene", "home-first"])).unwrap().is_err());
        assert!(folder_from(&args(&["app", "--live", &d])).unwrap().is_ok());
    }
}
