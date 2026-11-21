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
use ahg_engine::config::PALETTE_IDS;
use ahg_engine::queue::{TaskInit, create_task};
use ahg_engine::text::js_len;
use ahg_engine::tier::Plan;
use ahg_engine::types::{ContentType, MediaMetadata, Playlist, SearchResult, SourcePlatform, Track, UrlDetection};
use ahg_runtime::core::{APP_UPDATE_DELAY, Core, CoreEvent, ENGINE_UPDATE_DELAY, Paths, PlanSource, TrackMatch, WARM_UP_DELAY};
use ahg_runtime::unpack::tool_dirs;
use ahg_runtime::ytdlp::{Tools, YtDlp};
use rune::{ImageCache, Tx, Waker};
use serde_json::{Value, json};

use crate::copy::{self, found as fc};
use crate::gate::{Band, Card, Found, Loading, Matched, Nav, PRESETS, State, Tone};
use crate::timeline::{self, Due};
use crate::{actions, covers, motion};

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
    /// The preset, the pin and the own-palette choice as last saved; `None` until `settings-rust.json` was read.
    saved: Option<(usize, bool, bool)>,
    /// The causes the jobs named since the last poll, for the cause table.
    causes: Vec<Change>,
    /// Decodes the fetched covers, on the cover workers.
    images: Arc<ImageCache>,
}

impl Handle {
    /// `settings-rust.json` was read and applied (the splash's settings stage).
    pub fn own_read(&self) -> bool {
        self.saved.is_some()
    }
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
    s.thumbs = Default::default();
    s.speed.clear();
    s.selected.clear();
    s.picked.clear();
    s.found = Found::Nothing;
    s.notices.clear();
    s.plan = PLAN;
    s.loading = Loading::Booting;
    // A core starting with no splash up (a restart) loads behind a splash of its own.
    if s.splash.is_none() {
        s.splash = Some(crate::splash::Splash::new(crate::splash::STAGES));
    }
    s.settings = core.settings();
    s.engine = core.engine_status();
    s.update = core.update_status();
    let images = Arc::new(ImageCache::new(wake.clone()));
    s.live = Some(Handle {
        core,
        inbox,
        outbox,
        wake,
        commands: Some(commands),
        stop: Some(stop),
        generation: 0,
        saved: None,
        causes: Vec::new(),
        images,
    });
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
    let waiting = core.waiting_ids();
    let stats = Some(core.stats()).filter(|st| st.current.files > 0.0 || st.previous.is_some());
    let (settings, engine, update) = (core.settings(), core.engine_status(), core.update_status());
    Box::new(move |s, _| {
        let new_stats = put(&mut s.stats, stats);
        if new_stats {
            mark(s, Change::Stats);
        }
        let new_tasks = put(&mut s.tasks, tasks);
        if new_tasks {
            // A row the core let go of (a retry's old row, a removal) is no longer ticked.
            let ids: std::collections::HashSet<&str> = s.tasks.iter().map(|t| t.task_id.as_str()).collect();
            s.selected.retain(|id| ids.contains(id.as_str()));
        }
        let changed = [
            new_stats,
            new_tasks,
            put(&mut s.waiting, waiting),
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

/// The theme and palette the folder was left in, as `State` holds them, read before the first
/// frame so the splash paints in them (Electron's `theme-init.js`): two small files, no core.
pub fn peek_look(folder: &Path) -> (Option<Value>, usize) {
    let read = |name: &str| std::fs::read_to_string(folder.join(name)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let (config, own) = (read("config.json").unwrap_or_default(), read("settings-rust.json").unwrap_or_default());
    let saved = config.get("palette").and_then(Value::as_str);
    let palette = if own["ownPalette"] == true { 0 } else { PALETTE_IDS.iter().position(|id| Some(*id) == saved).map_or(0, |at| at + 1) };
    (config.get("theme").cloned(), palette)
}

/// The Rust-only choices (K52), read once at start.
fn own(core: &Core) -> Job {
    let preset = core.own_setting("preset").and_then(|v| v.as_u64()).map(|p| p as usize).filter(|p| *p < PRESETS.len());
    let pinned = core.own_setting("navPinned").and_then(|v| v.as_bool());
    let own_palette = core.own_setting("ownPalette").and_then(|v| v.as_bool()).unwrap_or(false);
    let hold = core.hold_unconfirmed();
    // Electron's `pickFloor`: the full floor once, the flag set as it is picked.
    let seen = core.own_setting("splashSeen") == Some(json!(true));
    if !seen {
        core.set_own_setting("splashSeen", json!(true));
    }
    Box::new(move |s, tx| {
        if let Some(splash) = s.splash.as_mut() {
            splash.floor = Some(if seen { motion::SPLASH_FLOOR_WARM } else { motion::SPLASH_FLOOR });
        }
        if let Some(preset) = preset {
            actions::set_preset(s, preset, tx);
        }
        s.nav_pinned = pinned.unwrap_or(s.nav_pinned);
        // Electron's `palette` names 1 to 7; the look's own mood (0) is only in `settings-rust.json`.
        let saved = s.settings.get("palette").and_then(Value::as_str);
        s.palette = if own_palette { 0 } else { PALETTE_IDS.iter().position(|id| Some(*id) == saved).map_or(0, |at| at + 1) };
        s.own.hold_unconfirmed = hold;
        s.sync();
        let now = (s.own.preset, s.own.nav_pinned, s.own.own_palette);
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

/// [`poll`], also counting the jobs applied and saying whether the commit is animated. The
/// one place live mode syncs the rings (MOTION-SYSTEM 5.2), and the one place it asks Rune
/// for a wake: at the timeline's next deadline, nothing else.
fn drain(s: &mut State, clock: f64, tx: &mut Tx) -> (usize, bool, bool) {
    let Some(live) = &s.live else { return (0, false, false) };
    let jobs: Vec<Job> = live.inbox.try_iter().collect();
    let count = jobs.len();
    let booting = s.loading == Loading::Booting;
    let mut causes = actions::fire(s, clock);
    // A stall falls due by the clock alone.
    let mut changed = !causes.is_empty() | sync_rings(s, &mut causes);
    for job in jobs {
        changed |= job(s, tx);
        // After each job: one that calls `State::sync` itself must not swallow the finish before it.
        changed |= sync_rings(s, &mut causes);
    }
    if changed {
        s.sync();
        covers::sync(s);
    }
    save_own(s);
    if let Some(live) = s.live.as_mut() {
        causes.append(&mut live.causes);
    }
    let home = s.nav == Nav::Home;
    let animated = changed && causes.iter().any(|c| deserves_motion(*c, booting, home));
    motion::data_commit(tx, animated, s.timeline.next_deadline());
    (count, changed, animated)
}

/// Brings the rings to the tasks: the stall deadline moves to the new `next_stall` (5.3), and a
/// finish or a fail starts its chain (5.6). True when a pass changed.
fn sync_rings(s: &mut State, causes: &mut Vec<Change>) -> bool {
    let synced = s.rings.sync(&s.tasks, s.now);
    s.timeline.cancel(&Due::Stall);
    if let Some(at) = synced.next_stall {
        s.timeline.at(at, Due::Stall);
    }
    let (chained, lands) = timeline::chain(s.now, &synced.finished, &synced.failed);
    for (at, due) in lands {
        s.timeline.at(at, due);
    }
    if chained {
        causes.push(Change::Chain);
    } else if synced.pass_changed {
        causes.push(Change::Pass);
    }
    synced.pass_changed
}

/// What a poll changed, as the cause table names it. A change not listed (a values-only report,
/// the settings, the engine's status, the first read) has no row: it lands, as Rune lands every
/// commit no input caused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// A ring is new or entered another kind of pass: a row arrives, the handoff, a stall (5.1).
    Pass,
    /// A task finished or failed: the finish or the fail chain's commit (5.6).
    Chain,
    /// A finished ring held `RING_HOLD` and landed; its row changes group (5.6).
    Landed,
    /// A look-up, a search band or a match answered (5.4).
    Answer,
    /// Bones held `LOADING_MIN` and give way to content (5.4).
    Freed,
    /// A wait reached `LOADING_DELAY`: bones go up.
    Bones,
    /// A slow wait's line changed its words (MOTION 2.4: a crossfade, never a cut).
    WaitLine,
    /// A notice entered.
    Notice,
    /// The month's statistics changed.
    Stats,
    /// A fetched cover arrived: it fades in over its placeholder.
    Cover,
}

/// The cause table: whether a change is committed with `tx.animated()`. Nothing animates while
/// the first read boots; the statistics only while Home shows them.
pub fn deserves_motion(change: Change, booting: bool, home: bool) -> bool {
    !booting
        && match change {
            Change::Pass
            | Change::Chain
            | Change::Landed
            | Change::Answer
            | Change::Freed
            | Change::WaitLine
            | Change::Notice
            | Change::Cover => true,
            Change::Stats => home,
            Change::Bones => false,
        }
}

/// Names a cause for the next poll's commit, from a job.
fn mark(s: &mut State, change: Change) {
    if let Some(live) = s.live.as_mut() {
        live.causes.push(change);
    }
}

/// K52: the preset, the pin and the own-palette choice, written to `settings-rust.json` when they change.
fn save_own(s: &mut State) {
    let now = (s.own.preset, s.own.nav_pinned, s.own.own_palette);
    let Some(live) = s.live.as_mut().filter(|l| l.saved.is_some_and(|saved| saved != now)) else { return };
    live.saved = Some(now);
    command(s, move |core| {
        core.set_own_setting("preset", json!(now.0));
        core.set_own_setting("navPinned", json!(now.1));
        core.set_own_setting("ownPalette", json!(now.2));
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

/// Fetches one cover through the core's strict client on a thread of its own and decodes it
/// there; the answer goes to [`covers::arrived`]. The worker holds the core only while it fetches,
/// and nothing lands once the handle is gone (its inbox goes with it).
// ponytail: `Core::cancel_lookups` does not reach a cover fetch; one in flight runs to its own timeout.
pub fn fetch_cover(s: &State, url: String) {
    let Some(live) = &s.live else { return };
    let (core, out, wake, images) = (Arc::downgrade(&live.core), live.outbox.clone(), live.wake.clone(), live.images.clone());
    thread::spawn(move || {
        let bytes = core.upgrade().and_then(|core| core.fetch_cover(&url).inspect_err(|e| debug_note(e)).ok());
        let image = bytes.and_then(|b| {
            covers::decode(&images, b).or_else(|| {
                debug_note("cover refused: format or size");
                None
            })
        });
        send(
            &out,
            &wake,
            Box::new(move |s, _| {
                let came = covers::arrived(s, url, image);
                if came {
                    mark(s, Change::Cover);
                }
                came
            }),
        );
    });
}

/// Says why a cover did not come, in a debug build's console only (the error carries no address).
fn debug_note(why: &str) {
    if cfg!(debug_assertions) {
        eprintln!("rune: {why}");
    }
}

/// A notice from a worker's answer.
fn notice(tone: Tone, text: String) -> Job {
    Box::new(move |s, _| {
        actions::notify(s, tone, text, None);
        mark(s, Change::Notice);
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
        let began = Instant::now();
        let answer = |job: Job| {
            // Bones that went up stay `LOADING_MIN` (MOTION-SYSTEM 5.4): an answer inside that
            // window waits out the rest here, so the shimmer never flashes.
            // ponytail: held on the worker; the timeline's waits (`actions::begin_wait`) take over when the found views ask `timeline::shows`.
            let (shown, gone) = (motion::LOADING_DELAY, motion::LOADING_DELAY + motion::LOADING_MIN);
            let at = began.elapsed().as_secs_f64();
            if (shown..gone).contains(&at) {
                thread::sleep(std::time::Duration::from_secs_f64(gone - at));
            }
            let current: Job = Box::new(move |s, tx| {
                let shown = s.live.as_ref().is_some_and(|l| l.generation == generation) && job(s, tx);
                if shown {
                    mark(s, Change::Answer);
                }
                shown
            });
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
pub(crate) mod tests {
    use std::fs;
    use std::time::{Duration, Instant};

    use ahg_engine::types::DownloadStatus as St;
    use ahg_runtime::core::Outside;
    use rune::{App, Env, Runtime};

    use super::*;
    use crate::motion::{RING_HOLD, STALL_AFTER};
    use crate::ring::Pass;

    pub(crate) const LINK: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    pub(crate) const RECORD: &str = r#"{"id":"dQw4w9WgXcQ","title":"A Song","uploader":"Someone","duration":212,"webpage_url":"https://www.youtube.com/watch?v=dQw4w9WgXcQ"}"#;

    pub(crate) fn scratch(name: &str) -> PathBuf {
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
    pub(crate) fn live(dir: &Path, json: &str, script: &str) -> Runtime<State> {
        live_with(dir, json, script, Outside::standin())
    }

    /// [`live`], with `outside` for the core's network.
    fn live_with(dir: &Path, json: &str, script: &str, outside: Outside) -> Runtime<State> {
        fs::write(dir.join("json.txt"), json).unwrap();
        fs::write(dir.join("script.txt"), script).unwrap();
        let mut ytdlp = YtDlp::new(fake(), None);
        ytdlp.env("FAKE_YTDLP_SCRIPT_JSON", dir.join("json.txt").to_str().unwrap());
        ytdlp.env("FAKE_YTDLP_SCRIPT", dir.join("script.txt").to_str().unwrap());
        ytdlp.env("FAKE_YTDLP_ARGS", dir.join("args.txt").to_str().unwrap());
        let ytdlp = Arc::new(ytdlp);
        let mut rt = App::new(crate::spec()).headless_runtime((1240.0, 820.0), Env { system_dark: true, scale: 1.0 });
        rt.frame(0.0);
        attach(&mut rt.state, dir, Arc::new(|| {}), move |paths, plan, events| Core::start_with(paths, ytdlp, plan, events, outside));
        crate::splash::skip(&mut rt.state); // the launch splash has its own tests (splash/tests.rs)
        until(&mut rt, "the first read of the core", |s| s.loading == Loading::Ready);
        rt
    }

    pub(crate) fn until(rt: &mut Runtime<State>, what: &str, done: impl Fn(&State) -> bool) {
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

    /// Whether `words` is drawn somewhere.
    fn drawn(rt: &Runtime<State>, words: &str) -> bool {
        rt.elements().iter().any(|e| !e.exiting && e.text.as_deref().is_some_and(|t| t.contains(words)))
    }

    /// The owner's report (2026-11-18): after Download the new download shows on Home (its
    /// row's `Starting`, or the classic looks' `1 in flight`, CONTENT.md 2.1) and its row says
    /// `Starting` on Downloads, before yt-dlp's first percent, in every preset.
    #[test]
    fn a_new_download_shows_its_row_starting_before_the_first_percent_in_every_preset() {
        let mut failed = Vec::new();
        for (i, (name, ..)) in crate::gate::PRESETS.iter().enumerate() {
            let dir = scratch(&format!("start-{i}"));
            let media = dir.join("Downloads").join("a.mp3");
            fs::write(&media, vec![0u8; 2048]).unwrap();
            // A wide FetchingInfo window, so a busy machine (the whole suite in parallel) still sees it.
            let script = format!("sleep 4000\n{P25}\nsleep 300\nout [FILEPATH]{}\nexit 0\n", media.display());
            let mut rt = live(&dir, &format!("out {RECORD}\nexit 0\n"), &script);
            rt.update(move |s, tx| actions::set_preset(s, i, tx));
            rt.update(|s, tx| {
                s.set_field(LINK);
                actions::submit(s, tx);
            });
            until(&mut rt, "the lookup's answer", |s| matches!(&s.found, Found::Media(_)));
            let pressed = Instant::now();
            rt.update(|s, _| actions::download_found(s));
            until(&mut rt, "the new row", |s| !s.tasks.is_empty());
            let row = pressed.elapsed().as_secs_f64();
            until(&mut rt, "the row starting", |s| s.tasks[0].progress.status == St::FetchingInfo);
            let starting = pressed.elapsed().as_secs_f64();
            for _ in 0..20 {
                rt.frame(1.0 / 60.0);
            }
            let id = rt.state.tasks[0].task_id.clone();
            let still = rt.state.tasks[0].progress.status == St::FetchingInfo;
            let ring = rt.state.rings.pass(&id);
            let home = drawn(&rt, copy::STATE_STARTING) || drawn(&rt, copy::HOME_IN_FLIGHT_SUFFIX.trim());
            rt.update(|s, _| actions::go(s, crate::gate::Nav::Downloads));
            for _ in 0..20 {
                rt.frame(1.0 / 60.0);
            }
            let downloads = drawn(&rt, copy::STATE_STARTING);
            until(&mut rt, "the first percent", |s| s.tasks[0].progress.percent > 0.0);
            let percent = pressed.elapsed().as_secs_f64();
            eprintln!(
                "preset {name}: row {:.0} ms, FetchingInfo {:.0} ms, first percent {:.0} ms after Download",
                row * 1e3,
                starting * 1e3,
                percent * 1e3
            );
            if !(still && ring == Some(Pass::OnThePad) && home && downloads) {
                failed
                    .push(format!("preset {name}: still starting {still}, ring {ring:?}, shown on Home {home}, on Downloads {downloads}"));
            }
            rt.state.live = None;
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }

    #[test]
    fn a_found_cover_shows_its_placeholder_first_then_fades_in_in_every_preset() {
        use ahg_runtime::engine_update::Fetched;
        let dir = scratch("covers");
        let png = crate::covers::tests::png(8, 6);
        let asked = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let log = asked.clone();
        // The real strict client, with only the wire a stand-in: an https answer of a tiny PNG.
        let outside = Outside {
            cover: Box::new(move |url: &str, _: Option<&str>, pinned: &[std::net::SocketAddr]| {
                assert!(!pinned.is_empty(), "the host was resolved and pinned first");
                log.lock().unwrap().push(url.to_owned());
                Some(Fetched {
                    status: 200,
                    final_url: url.to_owned(),
                    content_length: None,
                    location: None,
                    body: Box::new(std::io::Cursor::new(png.clone())),
                })
            }),
            ..Outside::standin()
        };
        let mut rt = live_with(&dir, "exit 0\n", "exit 0\n", outside);
        let art = |rt: &Runtime<State>| {
            rt.elements()
                .iter()
                .filter(|e| e.key.as_deref() == Some(crate::covers::ART) && !e.exiting)
                .map(|e| e.opacity)
                .collect::<Vec<_>>()
        };
        // What the field finds, three ways: one video, a search row, a collection's cover.
        let found = |kind: usize, url: String| -> Found {
            match kind {
                0 => Found::Media(MediaMetadata { thumbnail_url: url, ..crate::sample::media() }),
                1 => {
                    let [mut rows, _, _] = crate::sample::search();
                    rows.truncate(1);
                    rows[0].thumbnail_url = url;
                    Found::Search { query: "q".into(), bands: [Band::Results(rows), Band::Empty, Band::Empty] }
                }
                _ => playlist_found(Playlist { thumbnail_url: url, ..crate::sample::playlist() }, false),
            }
        };
        let mut urls = Vec::new();
        for (i, (name, ..)) in PRESETS.iter().enumerate() {
            rt.update(move |s, tx| actions::set_preset(s, i, tx));
            for kind in 0..3 {
                let case = format!("{name} / {}", ["media", "search row", "playlist"][kind]);
                let url = format!("https://img.example.com/cover-{i}-{kind}.png");
                urls.push(url.clone());
                // Out of the last case, then the answer arrives as a lookup's does: a job through the inbox.
                rt.update(|s, _| s.found = Found::Nothing);
                for _ in 0..40 {
                    rt.frame(1.0 / 60.0);
                }
                let answer = found(kind, url.clone());
                let live = rt.state.live.as_ref().unwrap();
                send(
                    &live.outbox.clone(),
                    &live.wake.clone(),
                    Box::new(move |s, _| {
                        s.found = answer;
                        s.card = Card::default();
                        true
                    }),
                );
                rt.frame(1.0 / 60.0);
                assert!(!matches!(rt.state.found, Found::Nothing), "{case}: the answer landed");
                assert!(art(&rt).is_empty(), "{case}: a cover before it was fetched");
                until(&mut rt, "the cover", move |s| crate::covers::get(s, &url).is_some());
                let first = art(&rt);
                assert_eq!(first.len(), 1, "{case}: the cover over its placeholder");
                assert!(first[0] < 1.0, "{case}: the cover cut in at {}", first[0]);
                for _ in 0..40 {
                    rt.frame(1.0 / 60.0);
                }
                assert!(art(&rt).iter().all(|o| (o - 1.0).abs() < 1e-3), "{case}: the cover never finished fading in: {:?}", art(&rt));
            }
        }
        assert_eq!(*asked.lock().unwrap(), urls, "each cover asked once, through the strict client");
        rt.state.live = None;
    }

    #[test]
    fn a_downloaded_task_shows_the_cover_its_lookup_named() {
        use ahg_runtime::engine_update::Fetched;
        let dir = scratch("task-cover");
        let media = dir.join("Downloads").join("a.mp3");
        fs::write(&media, vec![0u8; 2048]).unwrap();
        let png = crate::covers::tests::png(4, 4);
        let outside = Outside {
            cover: Box::new(move |url: &str, _: Option<&str>, _: &[std::net::SocketAddr]| {
                Some(Fetched {
                    status: 200,
                    final_url: url.to_owned(),
                    content_length: None,
                    location: None,
                    body: Box::new(std::io::Cursor::new(png.clone())),
                })
            }),
            ..Outside::standin()
        };
        let record = RECORD.replace(r#""duration":212"#, r#""duration":212,"thumbnail":"https://i.example.com/a.jpg""#);
        let script = format!("out [FILEPATH]{}\nexit 0\n", media.display());
        let mut rt = live_with(&dir, &format!("out {record}\nexit 0\n"), &script, outside);
        submit(&mut rt);
        until(&mut rt, "the task's cover", |s| s.tasks.first().is_some_and(|t| s.cover(t).is_some()));
        assert_eq!(rt.state.tasks[0].thumbnail_url, "https://i.example.com/a.jpg", "yt-dlp's thumbnail reached the task");
        rt.update(|s, _| actions::go(s, Nav::Downloads));
        for _ in 0..40 {
            rt.frame(1.0 / 60.0);
        }
        assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some(crate::covers::ART)), "the row draws the task's cover");
        rt.state.live = None;
    }

    #[test]
    fn an_answer_after_the_bones_went_up_waits_out_their_minimum() {
        use crate::motion::{LOADING_DELAY, LOADING_MIN};
        let dir = scratch("hold");
        // The stand-in answers just after the bones would show.
        let mut rt = live(&dir, &format!("sleep {}\nout {RECORD}\nexit 0\n", (LOADING_DELAY * 1000.0) as u64 + 50), "exit 0\n");
        let began = Instant::now();
        rt.update(|s, tx| {
            s.set_field(LINK);
            actions::submit(s, tx);
        });
        until(&mut rt, "the lookup's answer", |s| matches!(&s.found, Found::Media(_)));
        let took = began.elapsed().as_secs_f64();
        assert!(took >= LOADING_DELAY + LOADING_MIN - 0.02, "the bones went after {took:.3} s");
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
            let (count, changed, _) = drain(&mut rt.state, 0.0, &mut tx);
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
        // The three keys are written one after the other: wait for the last.
        until(&mut rt, "the pin written back", |_| read()["navPinned"] == json!(false) && read().get("ownPalette").is_some());
        assert_eq!(
            read(),
            json!({ "preset": 4, "navPinned": false, "ownPalette": false, "holdUnconfirmed": true, "splashSeen": true }),
            "other keys are kept"
        );
        rt.state.live = None;
    }

    /// The owner's report (2026-11-20): after a restart every preset showed palette 0 while
    /// Settings named the saved palette. Launch restores it, or the look's own mood when that was chosen.
    #[test]
    fn the_saved_palette_comes_back_at_launch_in_every_preset() {
        let ids = ahg_engine::config::PALETTE_IDS;
        let mut failed = Vec::new();
        for (i, (name, ..)) in PRESETS.iter().enumerate() {
            let at = (i + 2) % ids.len();
            for own_palette in [false, true] {
                let dir = scratch(&format!("palette-{i}-{own_palette}"));
                fs::write(dir.join("config.json"), json!({ "palette": ids[at] }).to_string()).unwrap();
                fs::write(dir.join("settings-rust.json"), json!({ "preset": i, "ownPalette": own_palette }).to_string()).unwrap();
                let mut rt = live(&dir, "exit 0\n", "exit 0\n");
                until(&mut rt, "the saved preset", |s| crate::gate::preset(s) == i);
                let want = if own_palette { 0 } else { at + 1 };
                if rt.state.palette != want {
                    failed.push(format!("preset {name}, ownPalette {own_palette}: palette {} (want {want})", rt.state.palette));
                }
                rt.state.live = None;
            }
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }

    #[test]
    fn choosing_the_look_s_own_palette_is_kept_in_settings_rust_json() {
        let dir = scratch("own-palette");
        let mut rt = live(&dir, "exit 0\n", "exit 0\n");
        let file = dir.join("settings-rust.json");
        let read = || serde_json::from_str::<Value>(&fs::read_to_string(&file).unwrap_or_default()).unwrap_or_default();
        rt.update(|s, tx| actions::set_palette(s, 0, tx));
        until(&mut rt, "ownPalette written", |_| read()["ownPalette"] == json!(true));
        rt.update(|s, tx| actions::set_palette(s, 3, tx));
        until(&mut rt, "ownPalette cleared", |_| read()["ownPalette"] == json!(false));
        rt.state.live = None;
    }

    // ── The ring, the chains and the wake, by stepped time. One clock per test: Rune's frames
    // (`rt.frame(dt)`, so `poll` gets Rune's clock and its wake is really asked), or `poll`
    // called by hand from the clock reached; never Rune's frames again after that.

    /// A quarter and a half of the file, as yt-dlp's progress template reports them.
    pub(crate) const P25: &str = "err [PROGRESS]1048576|4194304|NA|524288|6";
    const P50: &str = "err [PROGRESS]2097152|4194304|NA|524288|4";

    /// The pass's name, without its value.
    fn kind(p: Option<Pass>) -> String {
        p.map_or("None".into(), |p| format!("{p:?}").split('(').next().unwrap_or_default().to_owned())
    }

    fn ring_of(s: &State) -> Option<Pass> {
        s.tasks.first().and_then(|t| s.rings.pass(&t.task_id))
    }

    /// The link looked up and its download asked for.
    fn submit(rt: &mut Runtime<State>) {
        rt.update(|s, tx| {
            s.set_field(LINK);
            actions::submit(s, tx);
        });
        until(rt, "the lookup's answer", |s| matches!(&s.found, Found::Media(m) if m.title == "A Song"));
        rt.update(|s, _| actions::download_found(s));
    }

    /// Rune's frames of `dt` until `done`, with a real pause for the stand-in between them.
    fn frames(rt: &mut Runtime<State>, what: &str, dt: f64, mut done: impl FnMut(&State) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            rt.frame(dt);
            if done(&rt.state) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("never saw {what}");
    }

    /// `poll` by hand, the clock stepped by `dt` from `c`, until `done` sees the drain's (changed, animated).
    fn polls(s: &mut State, c: &mut f64, what: &str, dt: f64, mut done: impl FnMut(&State, bool, bool) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            *c += dt;
            let (_, changed, animated) = drain(s, *c, &mut Tx::default());
            if done(s, changed, animated) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("never saw {what}");
    }

    #[test]
    fn a_download_s_ring_runs_its_passes_in_order_lands_after_the_hold_and_the_app_idles() {
        let dir = scratch("passes");
        let media = dir.join("Downloads").join("a.mp3");
        fs::write(&media, vec![0u8; 2048]).unwrap();
        let script = format!("sleep 500\n{P25}\nsleep 500\n{P50}\nsleep 500\nout [FILEPATH]{}\nexit 0\n", media.display());
        let mut rt = live(&dir, &format!("out {RECORD}\nexit 0\n"), &script);
        submit(&mut rt);
        let mut seen: Vec<String> = Vec::new();
        let mut reports: Vec<f64> = Vec::new();
        let mut last = None;
        frames(&mut rt, "the ring's Done", 0.02, |s| {
            let k = kind(ring_of(s));
            if matches!(ring_of(s), Some(Pass::Flying(_))) && ring_of(s) != last {
                reports.push(s.now);
            }
            last = ring_of(s);
            if seen.last() != Some(&k) {
                seen.push(k);
            }
            ring_of(s) == Some(Pass::Done)
        });
        let (id, done_at) = (rt.state.tasks[0].task_id.clone(), rt.state.now);
        // The chain's one deadline, and Rune asked to wake for it.
        assert_eq!(rt.state.timeline.next_deadline(), Some(done_at + RING_HOLD));
        assert!(rt.wake_deadline().is_some_and(|w| (w - (done_at + RING_HOLD)).abs() < 1e-6), "wake {:?}", rt.wake_deadline());
        rt.frame(RING_HOLD - 0.05);
        assert_eq!(rt.state.rings.pass(&id), Some(Pass::Done), "the ring holds");
        rt.frame(0.05 + 1e-6);
        assert_eq!(rt.state.rings.pass(&id), Some(Pass::Landed), "and lands at RING_HOLD");
        seen.push(kind(rt.state.rings.pass(&id)));
        // In order, each once: OnThePad or Planned first, Closing only if the stand-in converted.
        let order = ["None", "Planned", "OnThePad", "Flying", "Closing", "Done", "Landed"];
        let at: Vec<usize> = seen.iter().map(|k| order.iter().position(|o| o == k).unwrap_or_else(|| panic!("{k} in {seen:?}"))).collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "out of order: {seen:?}");
        for must in ["Flying", "Done", "Landed"] {
            assert!(seen.iter().any(|k| k == must), "{must} never seen: {seen:?}");
        }
        assert!(seen.iter().any(|k| k == "Planned" || k == "OnThePad"), "no pass before Flying: {seen:?}");
        // The queue has ended: nothing is due, Rune is asked for nothing, and `poll` says no change.
        // Each report moved the Stall deadline and asked Rune for the new one; Rune cannot take a
        // wake back, so those wakes still come, each a no-op poll, until STALL_AFTER after the last report.
        for _ in 0..30 {
            if let Some(w) = rt.wake_deadline() {
                assert!(reports.iter().any(|r| (w - (r + STALL_AFTER)).abs() < 1e-6), "wake {w} is no report's stall ({reports:?})");
            }
            rt.frame(0.5);
            assert_eq!(rt.state.timeline.next_deadline(), None);
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(rt.wake_deadline(), None, "an idle app asks for no wake");
        let now = rt.state.now;
        assert!(!poll(&mut rt.state, now + 0.02, &mut Tx::default()));
        rt.state.live = None;
    }

    #[test]
    fn the_chains_commits_are_animated() {
        let dir = scratch("chains");
        let media = dir.join("Downloads").join("a.mp3");
        fs::write(&media, vec![0u8; 2048]).unwrap();
        let script = format!("sleep 500\n{P25}\nsleep 500\nout [FILEPATH]{}\nexit 0\n", media.display());
        let mut rt = live(&dir, &format!("out {RECORD}\nexit 0\n"), &script);
        submit(&mut rt);
        let mut c = rt.state.now;
        let mut finish = None;
        polls(&mut rt.state, &mut c, "the ring's Done", 0.02, |s, changed, animated| {
            finish = Some((changed, animated));
            ring_of(s) == Some(Pass::Done)
        });
        assert_eq!(finish, Some((true, true)), "the commit that puts up Done");
        let done_at = rt.state.now;
        let (_, changed, animated) = drain(&mut rt.state, done_at + RING_HOLD, &mut Tx::default());
        assert_eq!((changed, animated, ring_of(&rt.state)), (true, true, Some(Pass::Landed)), "the landing");
        rt.state.live = None;

        let dir = scratch("chains-fail");
        let mut rt =
            live(&dir, &format!("out {RECORD}\nexit 0\n"), &format!("sleep 500\n{P25}\nsleep 500\nerr ERROR: Video unavailable\nexit 1\n"));
        submit(&mut rt);
        let mut c = rt.state.now;
        let mut fail = None;
        polls(&mut rt.state, &mut c, "the ring's Failed", 0.02, |s, changed, animated| {
            fail = Some((changed, animated));
            matches!(ring_of(s), Some(Pass::Failed(_)))
        });
        assert_eq!(fail, Some((true, true)), "the commit that puts up Failed");
        rt.state.live = None;
    }

    #[test]
    fn a_failed_download_runs_the_fail_chain_with_no_hold() {
        let dir = scratch("failed");
        let mut rt =
            live(&dir, &format!("out {RECORD}\nexit 0\n"), &format!("sleep 500\n{P25}\nsleep 500\nerr ERROR: Video unavailable\nexit 1\n"));
        submit(&mut rt);
        let mut flew = false;
        frames(&mut rt, "the ring's Failed", 0.02, |s| {
            flew |= matches!(ring_of(s), Some(Pass::Flying(_)));
            matches!(ring_of(s), Some(Pass::Failed(_)))
        });
        assert!(flew, "the failure was seen arriving, not first seen");
        assert_eq!(rt.state.tasks[0].progress.status, St::Failed);
        assert_eq!(rt.state.timeline.next_deadline(), None, "no hold, no landing");
        rt.frame(RING_HOLD + 0.1);
        assert!(matches!(ring_of(&rt.state), Some(Pass::Failed(_))), "a failure stays");
        rt.state.live = None;
    }

    #[test]
    fn a_values_only_report_commits_instantly_and_quiet_polls_change_nothing() {
        let dir = scratch("values");
        let mut rt = live(&dir, &format!("out {RECORD}\nexit 0\n"), &format!("sleep 500\n{P25}\nsleep 1500\n{P50}\nsleep 30000\nexit 0\n"));
        submit(&mut rt);
        let mut c = rt.state.now;
        polls(&mut rt.state, &mut c, "the first report", 0.02, |s, _, _| ring_of(s) == Some(Pass::Flying(0.25)));
        assert_eq!(rt.state.rings.overall_value(), Some(0.25));
        let mut quiet = 0;
        let mut moved = None;
        polls(&mut rt.state, &mut c, "the second report", 0.02, |s, changed, animated| {
            if ring_of(s) == Some(Pass::Flying(0.5)) {
                moved = Some((changed, animated));
                return true;
            }
            // Between reports: no change, and the ring stays at the report.
            if !changed {
                quiet += 1;
                assert_eq!(ring_of(s), Some(Pass::Flying(0.25)));
                assert_eq!(s.rings.overall_value(), Some(s.tasks[0].progress.percent as f32 / 100.0));
            }
            false
        });
        assert!(quiet > 10, "only {quiet} quiet polls between the reports");
        assert_eq!(moved, Some((true, false)), "the report is a change, committed instantly");
        assert_eq!(rt.state.rings.overall_value(), Some(0.5), "the ring follows the report");
        rt.state.live = None;
    }

    #[test]
    fn a_silent_download_stalls_exactly_at_stall_after_by_its_deadline() {
        let dir = scratch("stall");
        let mut rt = live(&dir, &format!("out {RECORD}\nexit 0\n"), &format!("sleep 500\n{P25}\nsleep 30000\nexit 0\n"));
        submit(&mut rt);
        let mut c = rt.state.now;
        polls(&mut rt.state, &mut c, "the report", 0.02, |s, _, _| ring_of(s) == Some(Pass::Flying(0.25)));
        let seen_at = rt.state.now;
        assert_eq!(rt.state.timeline.next_deadline(), Some(seen_at + STALL_AFTER), "one Stall deadline, from the report");
        assert!(!poll(&mut rt.state, seen_at + STALL_AFTER - 0.001, &mut Tx::default()));
        assert!(matches!(ring_of(&rt.state), Some(Pass::Flying(_))));
        assert!(poll(&mut rt.state, seen_at + STALL_AFTER, &mut Tx::default()), "the deadline's poll changes State");
        assert_eq!(ring_of(&rt.state), Some(Pass::Stalled(0.25)));
        assert_eq!(rt.state.timeline.next_deadline(), None, "stalled: no deadline left");
        rt.state.live = None;
    }

    #[test]
    fn the_cause_table_animates_its_rows_and_nothing_while_booting() {
        use Change as C;
        // (change, animated on Home, animated elsewhere). The match breaks the build on a new change.
        let rows = [
            (C::Pass, true, true),
            (C::Chain, true, true),
            (C::Landed, true, true),
            (C::Answer, true, true),
            (C::Freed, true, true),
            (C::Bones, false, false),
            (C::WaitLine, true, true),
            (C::Notice, true, true),
            (C::Stats, true, false),
            (C::Cover, true, true),
        ];
        assert!(!rows.is_empty());
        for (change, home, elsewhere) in rows {
            match change {
                C::Pass | C::Chain | C::Landed | C::Answer | C::Freed | C::Bones | C::WaitLine | C::Notice | C::Stats | C::Cover => {}
            }
            assert_eq!(deserves_motion(change, false, true), home, "{change:?} on Home");
            assert_eq!(deserves_motion(change, false, false), elsewhere, "{change:?} elsewhere");
            assert!(!deserves_motion(change, true, true) && !deserves_motion(change, true, false), "{change:?} while booting");
        }
    }

    /// Rune's `--scene` applies the scene to the initial state and then runs `setup`: the rings
    /// must be those of the scene's tasks, not of the samples it replaced.
    #[test]
    fn a_scene_that_replaces_the_tasks_draws_its_own_rings() {
        use crate::gate::{Layout, SCENES};
        let bar = PRESETS.iter().position(|p| p.2 == Some(Layout::Bar)).unwrap();
        let cases = [("idle", false), ("home-first", false), ("first", false), ("finished", false), ("downloads-active", true)];
        assert!(!cases.is_empty());
        for (scene, unfinished) in cases {
            let f = SCENES.iter().find(|s| s.0 == scene).unwrap_or_else(|| panic!("no scene {scene}")).1;
            let rt = App::new(crate::spec())
                .init(move || {
                    let mut s = State::sample(bar);
                    f(&mut s);
                    s
                })
                .headless_runtime((1240.0, 820.0), Env { system_dark: true, scale: 1.0 });
            assert_eq!(rt.state.rings.overall_value().is_some(), unfinished, "{scene}: the overall ring");
            for t in &rt.state.tasks {
                assert_eq!(rt.state.rings.pass(&t.task_id).is_none(), t.progress.status == St::Cancelled, "{scene}: {}", t.task_id);
            }
        }
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
            assert!(refused(ok(&p)), "{} was accepted: {:?}", p.display(), ok(&p));
        }
        // A relative path, from the working folder: it exists only on the working folder's own drive
        // (CI keeps the workspace on D: and the temp folder on C:, where there is none).
        if cwd.components().next() == real.components().next() {
            let up = cwd.components().count() - 1;
            // After the drive and the root: `..` up to the root, then down to the real folder.
            let below: PathBuf = real.components().skip(2).collect();
            let relative: PathBuf = std::iter::repeat_n("..", up).collect::<PathBuf>().join(below);
            let answer = ok(&relative);
            assert!(refused(answer.clone()), "{} was accepted: {answer:?}", relative.display());
        }
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
