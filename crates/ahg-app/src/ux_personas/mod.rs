//! The UX scorecard (step U0a), the third score beside UI and FUNC: four seeded personas
//! (normal, lazy, power, stupid) use and abuse every preset, dark and light, live on a scratch
//! folder with the stand-in yt-dlp, through Rune's scripted runtime: real presses, keys,
//! pastes, pointer moves and window sizes. Each persona has its own expectations; the score per
//! preset is 10 x the weighted pass fraction minus friction penalties, capped at 5.0 for a
//! panic, hang, stuck overlay or duplicate task. The table goes to stdout and
//! `target/ux-scorecard.md`; the test fails when a preset is below [`THRESHOLD`].
//!
//! Needs the stand-in first: `cargo build -p ahg-runtime --bin fake_ytdlp`.
mod hover;
mod personas;
mod regressions;

use std::fmt::Write as _;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rune::clipboard::MemoryClipboard;
use rune::{ElementInfo, Key, KeyEvent, Mods, Runtime};

use crate::gate::{Found, Overlay, PRESETS, State};
use crate::live::tests as lt;
use crate::{actions, keys};

/// The lowest UX score a preset may have. Starts at the measured baseline; the loop raises it to 9.0.
const THRESHOLD: f64 = 0.0;

/// Steps from launch to the first download a lazy user takes: paste, then one press.
const STEP_BUDGET: u32 = 2;

/// The score of any preset that panicked, hung, kept an overlay up or doubled a task.
const HARD_CAP: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Persona {
    Normal,
    Lazy,
    Power,
    Stupid,
}

impl Persona {
    const ALL: [Persona; 4] = [Persona::Normal, Persona::Lazy, Persona::Power, Persona::Stupid];

    fn weight(self) -> f64 {
        match self {
            Persona::Normal => 0.25,
            Persona::Lazy => 0.20,
            Persona::Power => 0.25,
            Persona::Stupid => 0.30,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Persona::Normal => "normal",
            Persona::Lazy => "lazy",
            Persona::Power => "power",
            Persona::Stupid => "stupid",
        }
    }
}

/// One expectation and how much of it held (0 to 1).
pub struct Check {
    name: String,
    score: f64,
    why: String,
}

/// What one persona saw in one preset and theme.
#[derive(Default)]
pub struct Run {
    checks: Vec<Check>,
    /// Panics, hangs, stuck overlays and duplicate tasks: each caps the preset at [`HARD_CAP`].
    faults: Vec<String>,
    /// Presses with no visible result.
    dead_clicks: u32,
    steps_to_first: Option<u32>,
    /// Messages without a reason and a fix.
    unclear: u32,
    /// Wrong states with no obvious way out.
    dead_ends: u32,
}

impl Run {
    fn fraction(&self) -> f64 {
        if self.checks.is_empty() { 0.0 } else { self.checks.iter().map(|c| c.score).sum::<f64>() / self.checks.len() as f64 }
    }

    fn penalty(&self) -> f64 {
        let over = self.steps_to_first.map_or(0, |n| n.saturating_sub(STEP_BUDGET));
        (self.dead_clicks as f64 * 0.1).min(2.0) + over as f64 * 0.5 + self.unclear as f64 * 0.3 + self.dead_ends as f64 * 0.5
    }
}

/// A preset's score from its persona runs (both themes): 10 x the weighted pass fraction minus
/// the friction penalties, at most [`HARD_CAP`] after a fault, never below 0.
fn score(runs: &[(Persona, &Run)]) -> f64 {
    let s = uncapped(runs);
    if runs.iter().any(|(_, r)| !r.faults.is_empty()) { s.min(HARD_CAP) } else { s }
}

/// [`score`] before the fault cap, so the loop sees progress under it.
fn uncapped(runs: &[(Persona, &Run)]) -> f64 {
    let mean = |p: Persona, f: &dyn Fn(&Run) -> f64| {
        let of: Vec<f64> = runs.iter().filter(|(q, _)| *q == p).map(|(_, r)| f(r)).collect();
        if of.is_empty() { 0.0 } else { of.iter().sum::<f64>() / of.len() as f64 }
    };
    let passed: f64 = Persona::ALL.iter().map(|&p| p.weight() * mean(p, &Run::fraction)).sum();
    let penalty: f64 = Persona::ALL.iter().map(|&p| mean(p, &Run::penalty)).sum();
    (10.0 * passed - penalty).clamp(0.0, 10.0)
}

// ── The harness

/// One persona's session: the real app, live on its own scratch folder, in one preset and theme.
pub struct Ux {
    pub rt: Runtime<State>,
    pub run: Run,
    pub clip: MemoryClipboard,
    pub preset: usize,
    pub dir: PathBuf,
    rng: u64,
}

impl Drop for Ux {
    fn drop(&mut self) {
        self.rt.state.live = None;
    }
}

impl Ux {
    /// `download_ms`: how long each stand-in download runs before it finishes.
    fn new(preset: usize, dark: bool, persona: Persona, download_ms: u64) -> Ux {
        let dir = lt::scratch(&format!("ux-{preset}-{}-{}", u8::from(dark), persona.name()));
        let media = dir.join("Downloads").join("a.mp3");
        fs::write(&media, vec![0u8; 2048]).unwrap();
        let half = download_ms / 2;
        let script = format!("sleep {half}\n{}\nsleep {half}\nout [FILEPATH]{}\nexit 0\n", lt::P25, media.display());
        let mut rt = lt::live(&dir, &format!("out {}\nexit 0\n", lt::RECORD), &script);
        let clip = rune::clipboard::memory();
        rt.use_clipboard(&clip);
        rt.update(move |s, tx| {
            actions::set_preset(s, preset, tx);
            actions::set_theme(s, if dark { "dark" } else { "light" }, tx);
        });
        let seed = 0x9e37_79b9_7f4a_7c15 ^ ((preset as u64) << 8) ^ (persona as u64) ^ (u64::from(dark) << 16);
        let mut ux = Ux { rt, run: Run::default(), clip, preset, dir, rng: seed | 1 };
        ux.settle();
        ux
    }

    /// The next seeded number below `n`.
    pub fn rand(&mut self, n: usize) -> usize {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % n.max(1) as u64) as usize
    }

    /// Every track landed, a few frames drawn.
    pub fn settle(&mut self) {
        self.rt.settle_all();
        self.frames(3);
    }

    pub fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.rt.frame(1.0 / 60.0);
        }
    }

    /// A human pause of a seeded length.
    pub fn pause(&mut self) {
        let n = 5 + self.rand(25);
        self.frames(n);
    }

    /// Frames, with real time for the core between them, until `done` or `secs` pass.
    pub fn wait(&mut self, secs: f64, done: impl Fn(&State) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs_f64(secs);
        while Instant::now() < deadline {
            self.rt.frame(1.0 / 60.0);
            if done(&self.rt.state) {
                self.settle();
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// [`Ux::wait`]; a timeout is a hang.
    pub fn wait_or_hang(&mut self, what: &str, secs: f64, done: impl Fn(&State) -> bool) -> bool {
        let ok = self.wait(secs, done);
        if !ok {
            self.fault(format!("hang: never saw {what}"));
        }
        ok
    }

    pub fn check(&mut self, name: impl Into<String>, ok: bool, why: impl Into<String>) {
        let why = if ok { String::new() } else { why.into() };
        self.run.checks.push(Check { name: name.into(), score: if ok { 1.0 } else { 0.0 }, why });
    }

    /// A check that holds for `good` of `all` cases.
    pub fn part(&mut self, name: impl Into<String>, good: usize, all: usize, why: impl Into<String>) {
        let score = if all == 0 { 1.0 } else { good as f64 / all as f64 };
        let why = if good == all { String::new() } else { why.into() };
        self.run.checks.push(Check { name: name.into(), score, why });
    }

    pub fn fault(&mut self, why: impl Into<String>) {
        self.run.faults.push(why.into());
    }

    /// The elements on show: not leaving and not see-through.
    pub fn shown(&self) -> Vec<ElementInfo> {
        self.rt.elements().into_iter().filter(|e| !e.exiting && e.opacity > 0.01).collect()
    }

    /// What the user sees, as one number: every element's key, words, place and paint, without hover and press.
    pub fn look(&self) -> u64 {
        let mut h = DefaultHasher::new();
        for e in self.shown() {
            (&e.key, e.role, &e.text, &e.label).hash(&mut h);
            e.rect.map(|v| v.round() as i32).hash(&mut h);
            ((e.opacity * 20.0).round() as i32, format!("{:?}{:?}", e.fill, e.text_color)).hash(&mut h);
        }
        h.finish()
    }

    pub fn drawn(&self, words: &str) -> bool {
        self.shown().iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains(words)))
    }

    /// Where the control with this key is, scrolled into view first.
    pub fn find(&mut self, key: &str) -> Option<(f32, f32)> {
        let k = key.to_owned();
        self.rt.update(move |_, tx| tx.scroll_into_view(k));
        self.settle();
        self.rt.find(key)
    }

    /// Presses the control: `None` when there is none, else whether the picture changed.
    pub fn press(&mut self, key: &str) -> Option<bool> {
        let (x, y) = self.find(key)?;
        self.rt.pointer_move(x, y);
        self.settle();
        // Hovering may open what holds it (a sidebar): press where it is now.
        let (x, y) = self.rt.find(key)?;
        self.rt.pointer_move(x, y);
        self.settle();
        let before = self.look();
        self.rt.pointer_down(x, y);
        self.rt.pointer_up(x, y);
        self.settle();
        Some(self.look() != before)
    }

    /// A press the persona expects to do something: a missing control or no visible result fails.
    pub fn press_expect(&mut self, key: &str, name: &str) -> bool {
        match self.press(key) {
            None => {
                self.check(name, false, format!("no control `{key}`"));
                false
            }
            Some(false) => {
                self.run.dead_clicks += 1;
                self.check(name, false, format!("`{key}` pressed, nothing visible changed"));
                false
            }
            Some(true) => {
                self.check(name, true, "");
                true
            }
        }
    }

    /// Down and up on the control `n` times in a row, one frame apart.
    pub fn mash(&mut self, key: &str, n: usize) -> bool {
        let Some((x, y)) = self.find(key) else { return false };
        self.rt.pointer_move(x, y);
        for _ in 0..n {
            self.rt.pointer_down(x, y);
            self.rt.pointer_up(x, y);
            self.rt.frame(1.0 / 60.0);
        }
        self.settle();
        true
    }

    pub fn key(&mut self, key: Key) {
        self.rt.key(KeyEvent { key, mods: Mods::default() });
        self.settle();
    }

    pub fn ctrl(&mut self, c: char) {
        self.rt.key(KeyEvent { key: Key::Command(c), mods: Mods { cmd: true, ctrl: true, ..Mods::default() } });
        self.settle();
    }

    /// Clicks the field, empties it and pastes `text` into it, as a user does.
    pub fn type_in_field(&mut self, text: &str) -> bool {
        if self.press(keys::FIELD).is_none() {
            self.check("the field is there", false, format!("no `{}`", keys::FIELD));
            return false;
        }
        self.rt.update(|s, _| s.set_field(""));
        self.rt.paste(text.to_owned());
        self.settle();
        true
    }

    /// The message on show, and whether it carries a button.
    pub fn message(&self) -> Option<(String, bool)> {
        let s = &self.rt.state;
        if let Some(n) = s.notices.last() {
            return Some((n.text.clone(), n.action.is_some()));
        }
        match &s.found {
            Found::LookupFailed(text) => Some((text.clone(), false)),
            _ => None,
        }
    }

    /// The persona expects a message drawn with a reason and a fix (two sentences, or one and a button).
    pub fn expect_message(&mut self, name: &str) -> bool {
        let Some((text, button)) = self.message() else {
            self.check(name, false, "silence: no message");
            return false;
        };
        let head: String = text.chars().take(24).collect();
        let drawn = self.drawn(&head);
        let sentences = text.matches(['.', '!', '?']).count();
        let clear = drawn && (sentences >= 2 || button);
        if drawn && !clear {
            self.run.unclear += 1;
        }
        self.check(name, clear, if drawn { format!("no reason and fix in `{text}`") } else { format!("`{text}` is not drawn") });
        clear
    }

    /// Esc up to three times: every overlay, confirm and found step must close. Else a stuck overlay or a dead end.
    pub fn escape_out(&mut self, after: &str) -> bool {
        for _ in 0..3 {
            let s = &self.rt.state;
            if s.overlay == Overlay::None && !s.picker && s.confirm.is_none() && matches!(s.found, Found::Nothing) {
                return true;
            }
            self.key(Key::Escape);
        }
        let s = &self.rt.state;
        if s.overlay != Overlay::None || s.picker {
            let overlay = s.overlay;
            self.fault(format!("stuck overlay {overlay:?} after {after}"));
            false
        } else if s.confirm.is_some() || !matches!(s.found, Found::Nothing) {
            self.run.dead_ends += 1;
            self.check(format!("Esc gets out after {after}"), false, "Esc leaves a confirm or the found step open");
            false
        } else {
            true
        }
    }

    pub fn tasks(&self) -> usize {
        self.rt.state.tasks.len()
    }

    /// Goes to a place through its visible control.
    pub fn go(&mut self, nav: crate::gate::Nav) -> bool {
        let ok = self.press(keys::nav(nav)).is_some() && self.rt.state.nav == nav;
        if !ok {
            self.check(format!("{} is reachable", keys::nav(nav)), false, "its control is missing or did nothing");
        }
        ok
    }
}

// ── The scorecard

fn play(preset: usize, dark: bool, persona: Persona) -> Run {
    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ux = Ux::new(preset, dark, persona, if persona == Persona::Power { 6000 } else { 600 });
        personas::play(&mut ux, persona);
        std::mem::take(&mut ux.run)
    }));
    ran.unwrap_or_else(|e| {
        let why = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
        Run { faults: vec![format!("panic: {why}")], ..Run::default() }
    })
}

#[test]
fn ux_scorecard_for_every_preset() {
    assert!(!PRESETS.is_empty());
    let began = Instant::now();
    let mut runs: Vec<(usize, bool, Persona, Run)> = Vec::new();
    for i in 0..PRESETS.len() {
        for dark in [true, false] {
            for p in Persona::ALL {
                let t = Instant::now();
                let run = play(i, dark, p);
                eprintln!("ux: {} {} {}: {:.1} s", PRESETS[i].0, if dark { "dark" } else { "light" }, p.name(), t.elapsed().as_secs_f64());
                runs.push((i, dark, p, run));
            }
        }
    }
    assert_eq!(runs.len(), PRESETS.len() * 2 * Persona::ALL.len(), "every preset, theme and persona ran");

    // A path that stopped early still owes its expectations: every run of a persona is scored
    // over as many checks as its longest run made.
    for p in Persona::ALL {
        let most = runs.iter().filter(|r| r.2 == p).map(|r| r.3.checks.len()).max().unwrap_or(0);
        for r in runs.iter_mut().filter(|r| r.2 == p) {
            while r.3.checks.len() < most {
                r.3.checks.push(Check { name: "not reached".into(), score: 0.0, why: "the path stopped before this expectation".into() });
            }
        }
    }

    let mut out = String::from("# UX scorecard\n\n| preset | theme | persona | pass | checks | faults |\n|---|---|---|---|---|---|\n");
    for (i, dark, p, r) in &runs {
        let good = r.checks.iter().filter(|c| c.score >= 1.0).count();
        let _ = writeln!(
            out,
            "| {} | {} | {} | {:.0}% | {good}/{} | {} |",
            PRESETS[*i].0,
            if *dark { "dark" } else { "light" },
            p.name(),
            r.fraction() * 100.0,
            r.checks.len(),
            r.faults.len()
        );
    }
    out.push_str("\n| preset | UX score | before the cap | dead clicks | steps to first download | unclear messages | dead ends | faults |\n|---|---|---|---|---|---|---|---|\n");
    let mut scores = Vec::new();
    for (i, (name, ..)) in PRESETS.iter().enumerate() {
        let mine: Vec<(Persona, &Run)> = runs.iter().filter(|r| r.0 == i).map(|r| (r.2, &r.3)).collect();
        let total = |f: &dyn Fn(&Run) -> u32| mine.iter().map(|(_, r)| f(r)).sum::<u32>();
        let steps: Vec<String> = mine.iter().filter_map(|(_, r)| r.steps_to_first.map(|n| n.to_string())).collect();
        let faults = total(&|r| r.faults.len() as u32);
        let s = score(&mine);
        let _ = writeln!(
            out,
            "| {name} | {s:.2} | {:.2} | {} | {} | {} | {} | {faults} |",
            uncapped(&mine),
            total(&|r| r.dead_clicks),
            steps.join(", "),
            total(&|r| r.unclear),
            total(&|r| r.dead_ends),
        );
        scores.push((name, s));
    }
    out.push_str(
        "\nNot measured headless: whether each look's hover colour is its own (Rune's tree carries the hover amount, \
         not the overlay colour) and whether a focus ring is drawn (`focus_visible` only says focus came from the keyboard).\n",
    );
    out.push_str("\n## What failed\n\n");
    for (i, dark, p, r) in &runs {
        for f in &r.faults {
            let _ = writeln!(out, "- **{}** {} {}: FAULT {f}", PRESETS[*i].0, if *dark { "dark" } else { "light" }, p.name());
        }
        for c in r.checks.iter().filter(|c| c.score < 1.0) {
            let _ = writeln!(
                out,
                "- {} {} {}: {} ({:.0}%): {}",
                PRESETS[*i].0,
                if *dark { "dark" } else { "light" },
                p.name(),
                c.name,
                c.score * 100.0,
                c.why
            );
        }
    }
    let _ = writeln!(out, "\nRan in {:.0} s.", began.elapsed().as_secs_f64());
    println!("{out}");
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ux-scorecard.md");
    let _ = fs::write(&target, &out);
    let low: Vec<String> = scores.iter().filter(|(_, s)| *s < THRESHOLD).map(|(n, s)| format!("{n} {s:.2}")).collect();
    assert!(low.is_empty(), "below the UX threshold {THRESHOLD}: {}", low.join(", "));
}

#[test]
fn a_fault_caps_the_score_and_friction_costs() {
    let clean = Run { checks: vec![Check { name: "a".into(), score: 1.0, why: String::new() }], ..Run::default() };
    let runs: Vec<(Persona, &Run)> = Persona::ALL.iter().map(|&p| (p, &clean)).collect();
    assert!((score(&runs) - 10.0).abs() < 1e-9);
    let faulty = Run {
        faults: vec!["panic".into()],
        ..Run { checks: vec![Check { name: "a".into(), score: 1.0, why: String::new() }], ..Run::default() }
    };
    let runs: Vec<(Persona, &Run)> =
        vec![(Persona::Normal, &faulty), (Persona::Lazy, &clean), (Persona::Power, &clean), (Persona::Stupid, &clean)];
    assert_eq!(score(&runs), HARD_CAP);
    let slow = Run {
        steps_to_first: Some(4),
        dead_clicks: 3,
        ..Run { checks: vec![Check { name: "a".into(), score: 1.0, why: String::new() }], ..Run::default() }
    };
    let runs: Vec<(Persona, &Run)> =
        vec![(Persona::Normal, &clean), (Persona::Lazy, &slow), (Persona::Power, &clean), (Persona::Stupid, &clean)];
    assert!((score(&runs) - (10.0 - 1.0 - 0.3)).abs() < 1e-9, "two steps over budget and three dead clicks: {}", score(&runs));
    assert_eq!(score(&[]), 0.0, "no runs is no score");
}
