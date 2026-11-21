//! The functionality scorecard (LOOP-9's FUNC score, step F0a): a fixed list of end-to-end
//! scenarios, each driven through the drawn controls of the real app (Rune's scripted headless
//! runtime) in live mode on a scratch folder, with the stand-in yt-dlp and no network. Every
//! scenario runs in every preset, dark and light. `rt.update` is for set-up only; the step under
//! test is a click on a key from `keys.rs` or a key press, so a control a preset does not draw
//! fails that preset alone.
//!
//! Score per preset = 10 x the weighted pass fraction over both themes; any failed core-flow
//! scenario caps the preset at 6.0. The table goes to stdout and `target/func-scorecard.md`, and
//! the test fails when a preset is below [`FLOOR`] (the baseline; the loop raises it to 9.0).
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ahg_runtime::core::{Core, Outside};
use ahg_runtime::ytdlp::YtDlp;
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};

use crate::gate::{Found, Loading, Nav, PRESETS, State};
use crate::keys::{self, RowAct};
use crate::{actions, live};

mod flows;
mod queue;
mod settings;
mod shell;
mod walk;

/// The lowest FUNC score a preset may have. Baseline (step F0a, 2026-11-21): One bar 8.9, The collection 8.8,
/// Now playing 8.8, Signal 9.0, Shelf 9.0, Orbit 9.0. After its fixes: 9.5 in every preset. The floor
/// leaves room for a flaky scenario on a loaded machine (one cost Orbit 0.3 once).
const FLOOR: f64 = 9.0;
/// A failed core-flow scenario caps the preset here.
const CORE_CAP: f64 = 6.0;

pub type Res = Result<(), String>;
type Scenario = (&'static str, fn(&mut Ctx) -> Res);

/// One scenario group: its weight out of 100 and its scenarios, run in order on one live app.
pub struct Group {
    pub name: &'static str,
    pub weight: u32,
    pub scenarios: &'static [Scenario],
}

/// The fixed list. Startup's 5 come out of shell (10 -> 5): both are about the process's life,
/// and shell's other half (the window's own close button, the second window) needs a live window.
const GROUPS: [Group; 9] = [
    Group { name: "core flow", weight: 25, scenarios: flows::CORE },
    Group { name: "search + playlists", weight: 15, scenarios: flows::SEARCH },
    Group { name: "queue controls", weight: 15, scenarios: queue::QUEUE },
    Group { name: "settings", weight: 15, scenarios: settings::SETTINGS },
    Group { name: "error states", weight: 10, scenarios: flows::ERRORS },
    Group { name: "keyboard + nav", weight: 5, scenarios: shell::KEYBOARD },
    Group { name: "shell", weight: 5, scenarios: shell::SHELL },
    Group { name: "startup", weight: 5, scenarios: shell::STARTUP },
    Group { name: "stability", weight: 5, scenarios: walk::STABILITY },
];

// ── The harness

/// Frames after a press, for the press to run and the answer to land.
const SETTLE: usize = 20;
/// How long a wait on the core may take before the scenario fails.
const PATIENCE: Duration = Duration::from_secs(20);

/// One live app on one preset and theme, on its own scratch folder.
pub struct Ctx {
    pub rt: Runtime<State>,
    pub dir: PathBuf,
    pub preset: usize,
    pub dark: bool,
}

impl Drop for Ctx {
    /// The core goes first, so no stand-in yt-dlp outlives a failed scenario.
    fn drop(&mut self) {
        self.rt.state.live = None;
    }
}

impl Ctx {
    pub fn new(preset: usize, dark: bool, tag: &str) -> Ctx {
        let dir = std::env::temp_dir().join(format!("ahg-func-{}-{preset}-{}-{tag}", std::process::id(), if dark { "d" } else { "l" }));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Downloads")).unwrap();
        let mut rt = App::new(crate::spec()).headless_runtime((1240.0, 820.0), Env { system_dark: dark, scale: 1.0 });
        rt.frame(0.0);
        let mut ctx = Ctx { rt, dir, preset, dark };
        ctx.json("exit 0\n");
        ctx.script("exit 0\n");
        ctx.attach();
        ctx.until("the first read of the core", |s| s.loading == Loading::Ready).expect("the core never answered");
        ctx.rt.update(move |s, tx| actions::set_preset(s, preset, tx));
        ctx.frames(SETTLE);
        ctx
    }

    /// The core on this folder, with the stand-ins.
    fn attach(&mut self) {
        let mut ytdlp = YtDlp::new(fake(), None);
        ytdlp.env("FAKE_YTDLP_SCRIPT_JSON", self.dir.join("json.txt").to_str().unwrap());
        ytdlp.env("FAKE_YTDLP_SCRIPT", self.dir.join("script.txt").to_str().unwrap());
        ytdlp.env("FAKE_YTDLP_ARGS", self.dir.join("args.txt").to_str().unwrap());
        let ytdlp = Arc::new(ytdlp);
        live::attach(&mut self.rt.state, &self.dir, Arc::new(|| {}), move |paths, plan, events| {
            Core::start_with(paths, ytdlp, plan, events, Outside::standin())
        });
    }

    /// Stops the core (as quitting does) and starts a new one on the same folder.
    pub fn restart(&mut self) -> Res {
        self.rt.state.live = None;
        self.attach();
        self.until("the restarted core's first read", |s| s.loading == Loading::Ready)?;
        self.frames(SETTLE);
        Ok(())
    }

    /// What the stand-in prints for a lookup or a search (`--dump-json`).
    pub fn json(&self, script: &str) {
        fs::write(self.dir.join("json.txt"), script).unwrap();
    }

    /// What the stand-in prints for a download.
    pub fn script(&self, script: &str) {
        fs::write(self.dir.join("script.txt"), script).unwrap();
    }

    pub fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.rt.frame(1.0 / 60.0);
        }
    }

    /// Steps frames until `done` holds, at most [`PATIENCE`].
    pub fn until(&mut self, what: &str, done: impl Fn(&State) -> bool) -> Res {
        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline {
            self.rt.frame(1.0 / 60.0);
            if done(&self.rt.state) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(format!("never saw {what}"))
    }

    /// Whether an element with this key is drawn (scrolled into view first).
    pub fn has(&mut self, key: &str) -> bool {
        let k = key.to_owned();
        self.rt.update(move |_, tx| tx.scroll_into_view(k));
        self.frames(SETTLE);
        self.rt.find(key).is_some()
    }

    /// A press on the drawn control with this key.
    pub fn click(&mut self, key: &str) -> Res {
        if !self.has(key) {
            return Err(format!("no control `{key}` drawn"));
        }
        let (x, y) = self.rt.find(key).unwrap();
        self.rt.pointer_move(x, y);
        self.rt.pointer_down(x, y);
        self.rt.pointer_up(x, y);
        self.frames(SETTLE);
        Ok(())
    }

    /// A row's action button; when it is not drawn, the row is pressed first (a sleeve opens,
    /// a task goes on stage) and the button looked for again.
    pub fn row_act(&mut self, act: RowAct, id: &str) -> Res {
        let key = keys::row_action(act, id);
        if self.has(&key) {
            return self.click(&key);
        }
        let row = keys::row(id);
        if self.has(&row) {
            self.click(&row)?;
            if self.has(&key) {
                return self.click(&key);
            }
        }
        // Some looks keep a row's action in another place (Now playing's queue is on Home).
        for nav in [Nav::Home, Nav::Downloads] {
            self.go(nav)?;
            if self.has(&key) {
                return self.click(&key);
            }
        }
        Err(format!("no control `{key}` drawn on Home or Downloads"))
    }

    pub fn key(&mut self, key: Key, mods: Mods) {
        self.rt.key(KeyEvent { key, mods });
        self.frames(SETTLE);
    }

    /// Ctrl and a letter, digit or sign.
    pub fn ctrl(&mut self, c: char) {
        self.key(Key::Command(c), Mods { cmd: true, ctrl: true, ..Mods::default() });
    }

    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            self.rt.key(KeyEvent { key: Key::Text(c.to_string()), mods: Mods::default() });
        }
        self.frames(SETTLE);
    }

    /// Whether `words` are drawn somewhere.
    pub fn drawn(&self, words: &str) -> bool {
        self.rt.elements().iter().any(|e| !e.exiting && e.text.as_deref().is_some_and(|t| t.contains(words)))
    }

    /// To a place through the preset's own navigation.
    pub fn go(&mut self, nav: Nav) -> Res {
        self.click(keys::nav(nav))?;
        expect(self.rt.state.nav == nav, || format!("`{}` left the app on {:?}", keys::nav(nav), self.rt.state.nav))
    }

    /// Home, the field emptied, the link or words typed into it and Enter pressed.
    pub fn enter(&mut self, text: &str) -> Res {
        self.rt.update(|s, _| actions::dismiss_found(s));
        self.go(Nav::Home)?;
        self.click(keys::FIELD)?;
        self.type_text(text);
        expect(self.rt.state.field.text == text, || format!("the field holds `{}` after typing", self.rt.state.field.text))?;
        self.key(Key::Enter, Mods::default());
        Ok(())
    }

    /// Queues `link` with the saved defaults (set-up: the palette's "Paste link & download").
    pub fn queue(&mut self, link: &str) {
        let link = link.to_owned();
        self.rt.update(move |s, _| actions::paste_and_download(s, &link));
    }

    /// The id of the first task matching `pick`.
    pub fn task(&self, pick: impl Fn(&ahg_engine::types::DownloadTask) -> bool) -> Option<String> {
        self.rt.state.tasks.iter().find(|t| pick(t)).map(|t| t.task_id.clone())
    }

    /// A file in the Downloads folder, for the stand-in to name as its output.
    pub fn media(&self, name: &str) -> PathBuf {
        let file = self.dir.join("Downloads").join(name);
        fs::write(&file, vec![0u8; 2048]).unwrap();
        file
    }
}

pub fn expect(ok: bool, why: impl FnOnce() -> String) -> Res {
    if ok { Ok(()) } else { Err(why()) }
}

/// What the field found, in a word, for a cause.
pub fn found_kind(s: &State) -> &'static str {
    match &s.found {
        Found::Nothing => "nothing",
        Found::LookingUp => "looking up",
        Found::LookupFailed(_) => "a failed lookup",
        Found::Search { .. } => "a search",
        Found::Playlist { .. } => "a playlist",
        Found::Media(_) => "one item",
    }
}

/// The stand-in yt-dlp, built by `cargo build -p ahg-runtime --bin fake_ytdlp` beside this test's folder.
fn fake() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let fake = exe.parent().and_then(Path::parent).unwrap().join("fake_ytdlp.exe");
    assert!(fake.exists(), "build the stand-in first: cargo build -p ahg-runtime --bin fake_ytdlp ({})", fake.display());
    fake
}

// ── Running and scoring

/// One scenario's outcome in one preset and theme.
struct Outcome {
    preset: usize,
    dark: bool,
    group: usize,
    scenario: &'static str,
    result: Res,
}

/// Every group's scenarios on one preset and theme, a fresh app per group.
fn run_cell(preset: usize, dark: bool) -> Vec<Outcome> {
    let mut out = Vec::new();
    for (g, group) in GROUPS.iter().enumerate() {
        let made = std::panic::catch_unwind(|| Ctx::new(preset, dark, &g.to_string()));
        let mut ctx = match made {
            Ok(ctx) => Some(ctx),
            Err(e) => {
                let why = format!("the app did not start: {}", panic_text(&e));
                out.extend(group.scenarios.iter().map(|(name, _)| Outcome {
                    preset,
                    dark,
                    group: g,
                    scenario: name,
                    result: Err(why.clone()),
                }));
                None
            }
        };
        let Some(ctx) = ctx.as_mut() else { continue };
        for (name, run) in group.scenarios {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(ctx)))
                .unwrap_or_else(|e| Err(format!("panicked: {}", panic_text(&e))));
            out.push(Outcome { preset, dark, group: g, scenario: name, result });
        }
    }
    out
}

fn panic_text(e: &Box<dyn std::any::Any + Send>) -> String {
    e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_else(|| "a panic".into())
}

/// FUNC for one preset: `groups[i]` is (passed, run) of `GROUPS[i]` over both themes.
fn func(groups: &[(usize, usize)], core_failed: bool) -> f64 {
    assert_eq!(groups.len(), GROUPS.len(), "a group has no result");
    let total: u32 = GROUPS.iter().map(|g| g.weight).sum();
    let earned: f64 = GROUPS
        .iter()
        .zip(groups)
        .map(|(g, &(passed, run))| if run == 0 { 0.0 } else { g.weight as f64 * passed as f64 / run as f64 })
        .sum();
    let score = 10.0 * earned / total as f64;
    if core_failed { score.min(CORE_CAP) } else { score }
}

/// The table, per preset and theme, and the failures with their causes.
fn report(outcomes: &[Outcome], scores: &[f64]) -> String {
    let mut md = String::from(
        "# Functionality scorecard (FUNC)\n\nPassed / run per scenario group; FUNC = 10 x weighted pass fraction over both themes, a failed core-flow scenario caps it at 6.0.\n\n| Preset | Theme |",
    );
    for g in &GROUPS {
        let _ = write!(md, " {} ({}) |", g.name, g.weight);
    }
    md.push_str(" FUNC |\n|---|---|");
    md.push_str(&"---|".repeat(GROUPS.len() + 1));
    md.push('\n');
    for (p, (name, ..)) in PRESETS.iter().enumerate() {
        for dark in [true, false] {
            let _ = write!(md, "| {name} | {} |", if dark { "dark" } else { "light" });
            for g in 0..GROUPS.len() {
                let cell: Vec<_> = outcomes.iter().filter(|o| o.preset == p && o.dark == dark && o.group == g).collect();
                let _ = write!(md, " {}/{} |", cell.iter().filter(|o| o.result.is_ok()).count(), cell.len());
            }
            let _ = writeln!(md, " {:.1} |", scores[p]);
        }
    }
    md.push_str("\n## Failures\n\n");
    for o in outcomes.iter().filter(|o| o.result.is_err()) {
        let _ = writeln!(
            md,
            "- {} / {} / {} / {}: {}",
            PRESETS[o.preset].0,
            if o.dark { "dark" } else { "light" },
            GROUPS[o.group].name,
            o.scenario,
            o.result.as_ref().unwrap_err().replace('\n', " ")
        );
    }
    md
}

/// `target/func-scorecard.md`, beside the test binary's `debug` folder.
fn report_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.ancestors().nth(3).unwrap().join("func-scorecard.md")
}

#[test]
fn functional_scorecard_every_preset_dark_and_light() {
    assert!(!PRESETS.is_empty() && GROUPS.iter().all(|g| !g.scenarios.is_empty()));
    // One thread per preset, its two themes in a row. `AHG_FUNC_ONLY=3` runs one preset (the others score 0).
    let only: Option<usize> = std::env::var("AHG_FUNC_ONLY").ok().and_then(|v| v.parse().ok());
    let outcomes: Vec<Outcome> = std::thread::scope(|sc| {
        let jobs: Vec<_> = (0..PRESETS.len())
            .filter(|p| only.is_none_or(|o| o == *p))
            .map(|p| sc.spawn(move || [true, false].into_iter().flat_map(|dark| run_cell(p, dark)).collect::<Vec<_>>()))
            .collect();
        jobs.into_iter().flat_map(|j| j.join().expect("a preset thread panicked")).collect()
    });
    let scores: Vec<f64> = (0..PRESETS.len())
        .map(|p| {
            let mine = || outcomes.iter().filter(move |o| o.preset == p);
            let groups: Vec<(usize, usize)> = (0..GROUPS.len())
                .map(|g| (mine().filter(|o| o.group == g && o.result.is_ok()).count(), mine().filter(|o| o.group == g).count()))
                .collect();
            func(&groups, mine().any(|o| o.group == 0 && o.result.is_err()))
        })
        .collect();
    let md = report(&outcomes, &scores);
    println!("{md}");
    for (p, (name, ..)) in PRESETS.iter().enumerate() {
        println!("FUNC {name}: {:.1}", scores[p]);
    }
    let _ = fs::write(report_path(), &md);
    let below: Vec<String> = PRESETS
        .iter()
        .zip(&scores)
        .enumerate()
        .filter(|(p, (_, s))| only.is_none_or(|o| o == *p) && **s < FLOOR)
        .map(|(_, ((name, ..), s))| format!("{name} {s:.1}"))
        .collect();
    assert!(below.is_empty(), "below the floor {FLOOR}: {}\n{md}", below.join(", "));
}

#[test]
fn the_weights_sum_to_100_and_a_core_failure_caps_at_6() {
    assert_eq!(GROUPS.iter().map(|g| g.weight).sum::<u32>(), 100);
    let all: Vec<(usize, usize)> = GROUPS.iter().map(|g| (g.scenarios.len(), g.scenarios.len())).collect();
    assert_eq!(func(&all, false), 10.0);
    assert_eq!(func(&all, true), CORE_CAP);
    let none: Vec<(usize, usize)> = GROUPS.iter().map(|g| (0, g.scenarios.len())).collect();
    assert_eq!(func(&none, true), 0.0);
    // Half of the core flow: 10 x (100 - 12.5) / 100, capped.
    let mut half = all.clone();
    half[0] = (1, 2);
    assert_eq!(func(&half, true), CORE_CAP);
    assert!((func(&half, false) - 8.75).abs() < 1e-9);
}

#[test]
#[should_panic(expected = "a group has no result")]
fn a_missing_group_fails_the_score() {
    func(&[(1, 1)], false);
}
