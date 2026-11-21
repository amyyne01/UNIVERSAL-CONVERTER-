//! The launch splash, Electron's boot screen "Carrier Lock" (`index.html`, `public/splash-boot.js`,
//! `src/lib/splash.ts`): the wordmark resolving, a raw signal turning into a clean trace as the
//! bar fills, and a looping joke line. One brand splash over every preset, drawn in the palette's
//! colours and the Theme setting, with the planet mark in its Orbit dialect
//! (`docs/design-gate/logo-ideas/adapted.html`, section 1).
//!
//! Unlike Electron's timed ramp, the bar is real: it is the done share of [`STAGES`], ordered
//! loading work that starts only once the splash is on screen (the first view builds the splash
//! and nothing else). The splash goes only when every required stage is done and the full bar
//! has read for `SPLASH_HOLD`, and never before its floor (`SPLASH_FLOOR` on a first launch,
//! `SPLASH_FLOOR_WARM` after, from `splashSeen` in `settings-rust.json`). A failed required
//! stage, or one still going at `SPLASH_GIVE_UP`, shows the app with a notice instead.
use rune::{Color, Cx, El, Family, Shape, TextStyle, Tx, Waker, canvas, col, el, row, text};

use crate::copy::splash as words;
use crate::gate::{Loading, State, Tone};
use crate::{actions, keys, motion, palette, ring};

/// One piece of loading work.
pub struct Stage {
    /// What the stall line and a failure notice call it.
    pub label: &'static str,
    /// Its share of the bar; a list's weights sum to 1.
    pub weight: f32,
    /// The splash waits for it; an optional stage that is not done goes on behind the app.
    pub required: bool,
    /// Starts it, in a poll turn once every required stage before it is done.
    pub begin: fn(&mut State),
    /// Done, failed with why, or `None` while it is going. Asked every turn from its begin on.
    pub done: fn(&State) -> Option<Result<(), String>>,
}

/// The app's stages, in order. The tools are located inside `Core::start` (engine); fonts are
/// registered by Rune before the first frame and cannot move behind the splash; the update
/// checks, cover fetches and the picker's previews come after it.
pub const STAGES: &[Stage] = &[
    Stage { label: words::STAGE_ENGINE, weight: 0.30, required: true, begin: begin_engine, done: at_once },
    Stage { label: words::STAGE_ART, weight: 0.10, required: false, begin: nothing, done: art_decoded },
    Stage { label: words::STAGE_SETTINGS, weight: 0.10, required: true, begin: nothing, done: settings_read },
    Stage { label: words::STAGE_DOWNLOADS, weight: 0.25, required: true, begin: nothing, done: history_read },
    Stage { label: words::STAGE_LAYOUT, weight: 0.20, required: true, begin: begin_layout, done: layout_built },
    Stage { label: words::STAGE_SHELL, weight: 0.05, required: false, begin: begin_shell, done: at_once },
];

fn nothing(_: &mut State) {}

fn at_once(_: &State) -> Option<Result<(), String>> {
    Some(Ok(()))
}

/// Live mode's core (`--live`): its config, statistics, history file and tools. Nothing in sample mode.
fn begin_engine(s: &mut State) {
    let waker = s.splash.as_ref().and_then(|sp| sp.waker.clone());
    if let (Some(folder), Some(waker)) = (crate::live::FOLDER.get(), waker) {
        crate::live::start(s, folder, waker);
    }
}

/// The logo and the sample covers Home draws.
fn art_decoded(s: &State) -> Option<Result<(), String>> {
    let ready = s.logo.as_ref().is_none_or(|i| i.is_ready()) && s.covers.values().all(|i| i.is_ready());
    ready.then_some(Ok(()))
}

/// `settings-rust.json`: the preset, the pin, the palette and the seen flag (`live::own`).
fn settings_read(s: &State) -> Option<Result<(), String>> {
    s.live.as_ref().is_none_or(|l| l.own_read()).then_some(Ok(()))
}

/// The first read of the core: the downloads and the month's numbers.
fn history_read(s: &State) -> Option<Result<(), String>> {
    (s.loading == Loading::Ready).then_some(Ok(()))
}

/// The preset is built under the splash; done once a view has run with it.
fn begin_layout(s: &mut State) {
    if let Some(sp) = s.splash.as_mut() {
        sp.under = true;
    }
}

fn layout_built(s: &State) -> Option<Result<(), String>> {
    s.splash.as_ref().is_some_and(|sp| sp.built).then_some(Ok(()))
}

/// Tray, hotkey, clipboard watcher, presence, taskbar and toasts (`shell::install`).
fn begin_shell(_: &mut State) {
    crate::start_shell();
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Waiting,
    Going,
    Done,
    Failed(String),
}

/// The splash on screen and its loading.
pub struct Splash {
    stages: &'static [Stage],
    pub status: Vec<Status>,
    /// The clock of its first frame; `None` before the first poll.
    pub shown: Option<f64>,
    /// The last poll's clock.
    pub now: f64,
    /// `SPLASH_FLOOR` or `SPLASH_FLOOR_WARM`; `None` until the seen flag is read (warm then).
    pub floor: Option<f64>,
    /// The bar as drawn, 0 to 1: it moves toward the done share and never back.
    pub bar: f32,
    /// When the bar reached 1.
    full_at: Option<f64>,
    /// The preset is built under it (the layout stage began).
    pub under: bool,
    /// A view has run with the preset under it: a commit went out with `under` set.
    built: bool,
    /// For live mode's core, from `setup`.
    pub waker: Option<Waker>,
    /// What the last commit drew (bar in thousandths, line, scramble step, `under`, stages done),
    /// to commit only when it changes.
    drawn: (u32, usize, usize, bool, usize),
    seed: u64,
}

impl Splash {
    pub fn new(stages: &'static [Stage]) -> Splash {
        // A shuffle per launch; a headless picture is the same every time.
        let seed = if rune::is_headless() {
            0
        } else {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64)
        };
        Splash {
            stages,
            status: vec![Status::Waiting; stages.len()],
            shown: None,
            now: 0.0,
            floor: None,
            bar: 0.0,
            full_at: None,
            under: false,
            built: false,
            waker: None,
            drawn: (u32::MAX, 0, 0, false, 0),
            seed,
        }
    }

    /// The done share: the weights of the done stages, or all of it once every required one is done.
    pub fn loaded(&self) -> f32 {
        if self.required_done() {
            return 1.0;
        }
        let total: f32 = self.stages.iter().map(|s| s.weight).sum();
        let done: f32 = self.stages.iter().zip(&self.status).filter(|(_, st)| **st == Status::Done).map(|(s, _)| s.weight).sum();
        done / total
    }

    fn required_done(&self) -> bool {
        self.stages.iter().zip(&self.status).all(|(s, st)| !s.required || *st == Status::Done)
    }

    /// The first required stage not done, for the stall line.
    fn pending(&self) -> Option<&'static str> {
        self.stages.iter().zip(&self.status).find(|(s, st)| s.required && **st != Status::Done).map(|(s, _)| s.label)
    }

    fn elapsed(&self) -> f64 {
        self.shown.map_or(0.0, |at| self.now - at)
    }

    /// The status line on show: a joke, or what is still loading past `SPLASH_STALL`.
    fn line(&self) -> (usize, String) {
        if self.elapsed() >= motion::SPLASH_STALL
            && let Some(what) = self.pending()
        {
            return (usize::MAX, words::still_loading(what));
        }
        let every = motion::SPLASH_LINE_HOLD + motion::SPLASH_FLIP_SPREAD + motion::SPLASH_FLIP_SPRING.enter.duration as f64;
        let k = (self.elapsed() / every) as usize;
        (k, words::MESSAGES[shuffled(self.seed, k)].to_owned())
    }

    /// The scramble's step, or `None` once the wordmark has settled.
    fn scramble_step(&self) -> Option<usize> {
        let chars = words::WORD_BRAND.len() + words::WORD_NAME.len();
        let end = chars as f64 * motion::SPLASH_SCRAMBLE_STAGGER + motion::SPLASH_SCRAMBLE;
        (self.elapsed() < end).then(|| (self.elapsed() / motion::SPLASH_GLYPH_SWAP) as usize)
    }
}

/// The `k`th line of a shuffled walk through the twenty: every one shows before any repeats,
/// and each pass is shuffled again (Fisher-Yates over a small LCG).
fn shuffled(seed: u64, k: usize) -> usize {
    let n = words::MESSAGES.len();
    let mut order: Vec<usize> = (0..n).collect();
    let mut x = seed ^ (k / n) as u64 ^ 0x9E37_79B9_7F4A_7C15;
    for i in (1..n).rev() {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        order.swap(i, (x >> 33) as usize % (i + 1));
    }
    order[k % n]
}

/// The splash a launch starts with, unless a `--scene` other than the splash's own was asked for.
pub fn arm() -> Option<Splash> {
    let args: Vec<String> = std::env::args().collect();
    let scene = args.iter().position(|a| a == "--scene").and_then(|at| args.get(at + 1));
    scene.is_none_or(|n| n.starts_with("splash")).then(|| Splash::new(STAGES))
}

/// Steps the splash: begins the next stage, checks the ones going, moves the bar, and takes the
/// splash away when it may. True when what it draws changed.
pub fn poll(s: &mut State, clock: f64, tx: &mut Tx) -> bool {
    let Some(sp) = s.splash.as_mut() else { return false };
    motion::splash_frame(tx);
    let dt = (clock - sp.now).max(0.0);
    sp.now = clock;
    // The first frame draws the splash alone; work starts on the next turn.
    if sp.shown.is_none() {
        sp.shown = Some(clock);
        return true;
    }
    sp.built = sp.drawn.3;
    let stages = sp.stages;
    // Check what is going, then begin the next stage whose required predecessors are done.
    for (i, stage) in stages.iter().enumerate() {
        let going = s.splash.as_ref().is_some_and(|sp| sp.status[i] == Status::Going);
        if going && let Some(result) = (stage.done)(s) {
            set(s, i, result);
        }
    }
    let next = s.splash.as_ref().and_then(|sp| {
        let at = sp.status.iter().position(|st| *st == Status::Waiting)?;
        stages[..at].iter().zip(&sp.status).all(|(st, status)| !st.required || *status == Status::Done).then_some(at)
    });
    if let Some(i) = next {
        if let Some(sp) = s.splash.as_mut() {
            sp.status[i] = Status::Going;
        }
        (stages[i].begin)(s);
        if let Some(result) = (stages[i].done)(s) {
            set(s, i, result);
        }
    }
    let Some(sp) = s.splash.as_mut() else { return true };
    let target = sp.loaded();
    sp.bar = (sp.bar + (dt / motion::SPLASH_COMPLETE) as f32).min(target).max(sp.bar);
    if sp.bar >= 1.0 && sp.full_at.is_none() {
        sp.full_at = Some(clock);
    }
    let floor = sp.floor.unwrap_or(motion::SPLASH_FLOOR_WARM);
    let failed = stages.iter().zip(&sp.status).find_map(|(st, status)| match status {
        Status::Failed(why) if st.required => Some(words::did_not_load(st.label, why)),
        _ => None,
    });
    let gave_up = (sp.elapsed() >= motion::SPLASH_GIVE_UP)
        .then(|| sp.pending().map(|what| words::did_not_load(what, words::TOOK_TOO_LONG)))
        .flatten();
    let finished = sp.full_at.is_some_and(|at| clock >= at + motion::SPLASH_HOLD) && sp.elapsed() >= floor;
    if let Some(why) = failed.or(gave_up) {
        leave(s);
        actions::notify(s, Tone::Error, why, None);
        motion::data_commit(tx, true, None);
        return true;
    }
    if finished {
        leave(s);
        motion::data_commit(tx, true, None);
        return true;
    }
    let done = sp.status.iter().filter(|st| **st == Status::Done).count();
    let drawn = ((sp.bar * 1000.0) as u32, sp.line().0, sp.scramble_step().unwrap_or(usize::MAX), sp.under, done);
    let changed = drawn != sp.drawn;
    // A new status line flips in; the rest of the splash's commits land.
    motion::data_commit(tx, drawn.1 != sp.drawn.1, None);
    sp.drawn = drawn;
    changed
}

fn set(s: &mut State, i: usize, result: Result<(), String>) {
    if let Some(sp) = s.splash.as_mut() {
        sp.status[i] = result.map_or_else(Status::Failed, |()| Status::Done);
    }
}

/// The splash goes (Rune fades it out) and the work kept for after it starts. Tests of other
/// things start here, past the splash.
#[cfg_attr(not(test), allow(dead_code))]
pub fn skip(s: &mut State) {
    leave(s);
    for image in s.logo.iter().chain(s.covers.values()).chain(&s.previews) {
        let _ = image.wait();
    }
}

fn leave(s: &mut State) {
    s.splash = None;
    crate::gate::load_previews(s);
}

/// Whether the preset is drawn yet (under the splash or with none).
pub fn preset_shown(s: &State) -> bool {
    s.splash.as_ref().is_none_or(|sp| sp.under)
}

// ── Drawing

/// The stage's width (`--sp-stage-w`, `min(520px, 68vw)`; the window is never narrower than 765).
const W: f32 = 520.0;
/// The trace's height and its waveform's period, in points (the SVG's 1040 x 40 at 200 %).
const TRACE_H: f32 = 40.0;
const PERIOD: f32 = 520.0;
/// The bar's least width (`START`, 5 %), so the scan head shows from the first frame.
const START: f32 = 0.05;

struct Ink {
    bg: Color,
    ink: Color,
    ink2: Color,
    accent: Color,
    raw: Color,
    tick: Color,
    glow: Color,
}

fn ink(s: &State, cx: &Cx<State>) -> Ink {
    let dark = s.dark(cx);
    // Palette 0 is a look's own mood; the one brand splash wears Aurora there, Electron's own.
    let r = palette::roles(s.palette.saturating_sub(1), dark);
    let (raw, tick, glow) = if dark { (0.55, 0.35, 0.55) } else { (0.45, 0.30, 0.28) };
    Ink {
        bg: r.bg_primary,
        ink: r.text_primary,
        ink2: r.text_secondary,
        accent: r.accent,
        raw: r.text_muted.alpha(raw),
        tick: r.text_muted.alpha(tick),
        glow: r.accent.alpha(glow * 0.35),
    }
}

/// The splash, over the whole window: inert below it, its own keys only.
pub fn view(s: &State, cx: &mut Cx<State>) -> Option<El> {
    let sp = s.splash.as_ref()?;
    let c = ink(s, cx);
    let reduced = cx.is_reduced_motion();
    let bar = if reduced { sp.loaded().max(sp.bar) } else { sp.bar }.max(START);
    let stage = col()
        .w(W)
        .child(canvas(mark(44.0, c.ink, c.accent, c.bg)).size(44.0, 44.0).mb(18.0))
        .child(wordmark(sp, &c, reduced).mb(18.0))
        .child(trace(bar, &c))
        .child(status(sp, &c, reduced).mt(12.0));
    Some(
        el().key(keys::SPLASH)
            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .paint_order(i16::MAX)
            .fill(c.bg)
            .center()
            .modal(false, |_: &mut State| {})
            .transition(motion::splash_out())
            .child(stage),
    )
}

/// The Orbit dialect of the planet, on its 100 grid: a ring tilted 22 degrees whose far half
/// passes behind a hairline disc, two stars.
pub fn mark(size: f32, ink: Color, accent: Color, paper: Color) -> Shape {
    let k = size / 100.0;
    let (ts, tc) = (-22f32).to_radians().sin_cos();
    let ring_half = |from: f32| -> Vec<[f32; 2]> {
        (0..=48)
            .map(|i| from + std::f32::consts::PI * i as f32 / 48.0)
            .map(|a| (44.0 * a.cos(), 14.0 * a.sin()))
            .map(|(x, y)| [(50.0 + x * tc - y * ts) * k, (50.0 + x * ts + y * tc) * k])
            .collect()
    };
    let w = 4.0 * k;
    Shape::new()
        .stroke(ring_half(std::f32::consts::PI), w, accent)
        .fill(ring::disc(50.0 * k, 50.0 * k, 26.0 * k), paper)
        .stroke(ring::arc(50.0 * k, 50.0 * k, 26.0 * k, 0.0, std::f32::consts::TAU, 64), w, ink)
        .stroke(ring_half(0.0), w, accent)
        .fill(ring::disc(84.0 * k, 22.0 * k, 2.5 * k), ink)
        .fill(ring::disc(16.0 * k, 78.0 * k, 2.5 * k), ink)
}

const WORD: TextStyle = TextStyle::new(13.0, 500.0).lh(18.0).family(Family::Named("Geist Mono"));
const NAME: TextStyle = TextStyle::new(13.0, 400.0).lh(18.0).family(Family::Named("Geist Mono"));
const LINE: TextStyle = TextStyle::new(11.0, 400.0).lh(16.0).family(Family::Named("Geist Mono"));

/// "AHG Universal Converter", each character resolving out of churning glyphs in the accent.
fn wordmark(sp: &Splash, c: &Ink, reduced: bool) -> El {
    let step = if reduced { None } else { sp.scramble_step() };
    let glyphs: Vec<char> = words::GLYPHS.chars().collect();
    let mut at = 0;
    let mut part = |word: &str, style: TextStyle, color: Color| {
        let mut r = row();
        for ch in word.chars() {
            let i = at;
            at += 1;
            let settled = step.is_none_or(|_| sp.elapsed() >= i as f64 * motion::SPLASH_SCRAMBLE_STAGGER + motion::SPLASH_SCRAMBLE);
            let piece = if settled || ch == ' ' {
                text(ch.to_string()).font(style).color(color)
            } else {
                // Churning from its own start; before it, the glyph it was built with.
                let churn = if sp.elapsed() >= i as f64 * motion::SPLASH_SCRAMBLE_STAGGER { step.unwrap_or(0) } else { 0 };
                let pick = (hash(sp.seed, i, churn) % glyphs.len() as u64) as usize;
                text(glyphs[pick].to_string()).font(style).color(c.accent)
            };
            r = r.child(piece);
        }
        r
    };
    let brand = part(words::WORD_BRAND, WORD, c.ink);
    let name = part(words::WORD_NAME, NAME, c.ink2);
    row().gap(8.0).child(brand).child(name)
}

fn hash(seed: u64, i: usize, step: usize) -> u64 {
    let mut x = seed ^ ((i as u64) << 32) ^ step as u64;
    x = x.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^ (x >> 29)
}

/// The raw signal, the scan layer erasing it up to the bar with the clean trace in the accent,
/// the scan head and the two end ticks.
fn trace(bar: f32, c: &Ink) -> El {
    let fill = W * bar.min(1.0);
    let strip = |color: Color, glow: Option<Color>| {
        let pts = wave();
        let mut shape = Shape::new();
        if let Some(glow) = glow {
            shape = shape.stroke(pts.clone(), 5.0, glow);
        }
        el().w(PERIOD * 2.0).h(TRACE_H).looping(motion::SPLASH_DRIFT).shape(shape.stroke(pts, 1.0, color))
    };
    let tick = |x: f32| el().absolute(x, (TRACE_H - 9.0) / 2.0).w(1.0).h(9.0).fill(c.tick);
    el().w(W)
        .h(TRACE_H)
        .child(el().absolute(0.0, 0.0).w(W).h(TRACE_H).clip_rounded(0.0).child(strip(c.raw, None)))
        .child(el().absolute(0.0, 0.0).w(fill).h(TRACE_H).fill(c.bg).clip_rounded(0.0).child(strip(c.accent, Some(c.glow))))
        .child(el().absolute(fill - 2.0, (TRACE_H - 15.0) / 2.0).w(2.0).h(15.0).fill(c.accent).looping(motion::SPLASH_HEAD))
        .child(tick(0.0))
        .child(tick(W - 1.0))
}

/// The hand-authored waveform of `index.html` (`#sp-wave`), twice: bursts and rests on a 40 high line.
fn wave() -> Vec<[f32; 2]> {
    const D: &str = "M0 20H28L30 9L33 29L36 14L39 26L42 20H58L61 16L64 24L66 20H88L91 7L94 31L97 11L100 27L103 17L106 23L109 20H132L135 15L138 25L140 20H168L171 10L174 30L177 13L180 24L183 20H205L208 17L210 23L213 20H240L243 6L246 32L249 12L252 28L255 16L258 22L261 20H290L293 15L296 25L299 20H320L323 9L326 29L329 14L332 24L335 20H360L363 17L365 23L368 20H395L398 8L401 30L404 12L407 26L410 18L413 22L416 20H445L448 15L451 25L454 20H480L483 11L486 27L489 16L492 22L495 20H520";
    let mut one: Vec<[f32; 2]> = Vec::new();
    // The path is M, then H and L only: walk it as (letter, numbers).
    let mut letter = 'M';
    let mut nums = String::new();
    let flush = |letter: char, nums: &str, pts: &mut Vec<[f32; 2]>| {
        let v: Vec<f32> = nums.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        match (letter, v.as_slice()) {
            ('M' | 'L', [x, y]) => pts.push([*x, *y]),
            ('H', [x]) => pts.push([*x, pts.last().map_or(20.0, |p| p[1])]),
            _ => {}
        }
    };
    for ch in D.chars() {
        if ch.is_ascii_alphabetic() {
            flush(letter, &nums, &mut one);
            letter = ch;
            nums.clear();
        } else {
            nums.push(ch);
        }
    }
    flush(letter, &nums, &mut one);
    let second: Vec<[f32; 2]> = one.iter().skip(1).map(|p| [p[0] + PERIOD, p[1]]).collect();
    one.extend(second);
    one
}

/// The joke line, its characters flipping in one after another; a still crossfade when reduced.
fn status(sp: &Splash, c: &Ink, reduced: bool) -> El {
    let (k, line) = sp.line();
    let len = line.chars().count();
    let mut r = row().key(format!("splash-line-{k}")).absolute(0.0, 0.0);
    for (i, ch) in line.chars().enumerate() {
        let piece = text(ch.to_string()).font(LINE).color(c.ink2).key(format!("c{i}"));
        r = r.child(if reduced { piece } else { motion::splash_char(piece, i, len) });
    }
    el().w(W).h(22.0).child(r)
}

#[cfg(test)]
mod tests;
