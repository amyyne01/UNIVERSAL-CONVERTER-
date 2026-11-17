//! The motion vocabulary: the one place for every timing, curve, distance and loading rule.
//! Values are Rune's own tokens wherever one fits, so the six presets stay one family.
//! A preset names a purpose; it never types a number (the lint test at the end enforces it).
//! Design: `docs/design-gate/MOTION-SYSTEM.md` section 2. Not here yet, left for the steps
//! that need a live window or `State`: the `Moves` verbs on `El`, `Ctx`/`ctx`,
//! `preset_switch()`, `growth()` (M6/M8 and the presets), `timeline::show` (M2).

use rune::tok::motion as t;
use rune::{Beat, Curve, Easing, LoopSpec, Spring, SpringPair};

// ---- Feedback ----

/// What a press scales to: text buttons, icon buttons, large tiles (today's values).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Press {
    Button,
    Icon,
    Tile,
}

impl Press {
    pub const fn scale(self) -> f32 {
        match self {
            Press::Button => 0.97,
            Press::Icon => 0.94,
            Press::Tile => 0.98,
        }
    }
}

/// Controls that appear on row hover or focus: in, out (set on the hover group).
pub const REVEAL: (Curve, Curve) = (Curve::tween(0.12, Easing::ENTER), Curve::tween(0.10, Easing::EXIT));

// ---- Springs, always a Rune pair so exits are shorter ----

pub const CONTROL: SpringPair = t::pair::SNAP;
pub const OVERLAY: SpringPair = t::pair::SNAP;
pub const SHEET: SpringPair = t::pair::GLIDE;
pub const ROW: SpringPair = t::pair::SETTLE;
pub const REFLOW: SpringPair = t::pair::REFLOW;
/// The two rich moments (the found step's reveal, the month's numbers).
pub const MOMENT: Spring = t::REFLOW;
/// Light, dark, palette (through `tx.color_motion`).
pub const THEME: Spring = t::REFLOW;
/// A handler that changes the preset, the palette or the theme recolours the window on `THEME`.
pub fn recolour(tx: &mut rune::Tx) {
    tx.color_motion(THEME);
}
/// A commit live mode's `poll` makes (data or a deadline): animated when the cause table says so,
/// and one wake at the timeline's next deadline. Nothing else asks Rune for a wake.
pub fn data_commit(tx: &mut rune::Tx, animated: bool, wake: Option<f64>) {
    if animated {
        tx.animated();
    }
    if let Some(at) = wake {
        tx.wake_at(at);
    }
}
/// Home / Downloads / Settings swapping: opacity only, 150 ms in, 110 ms out. Ctrl+1/2/3 uses the same.
pub const PLACE: (Curve, Curve) = (t::CSS_ENTER, t::CSS_EXIT);
pub const STAGGER: f32 = 0.03;
pub const STAGGER_MAX: usize = 6;

// ---- The preset switch (owner decision 3) and the totals (decision 4) ----

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dissolve {
    /// Fade with a blur of this many points, no scale.
    FadeBlur(f32),
    /// Plain fade; flip `DISSOLVE` to this if the blur costs frames.
    Fade,
}
pub const DISSOLVE: Dissolve = Dissolve::FadeBlur(1.5);
/// The dissolve rides `ROW` (0.30 s in, 0.24 s out).
pub const DISSOLVE_SPRING: SpringPair = ROW;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Digits {
    Crossfade,
}
pub const DIGITS: Digits = Digits::Crossfade;

// ---- The ring ----

/// Provisional, judged live.
pub const RING_FOLLOW: Spring = Spring::new(0.6, 0.0);
/// Seconds per turn while the amount is unknown; linear. Provisional.
pub const RING_SWEEP: f64 = 1.6;
pub const RING_BREATHE: LoopSpec = LoopSpec::opacity(2.4, 0.0, 0.6, 1.0);
pub const RING_DRAW: Curve = Curve::tween(0.40, Easing::DRAW_ON);
/// The one beat of a finished download.
pub const RING_DONE: &[Beat] = &[Beat::new(0.0), Beat::new(0.12).scale(1.05), Beat::new(0.36)];
pub const RING_HOLD: f64 = 0.9;
/// Turning rings in one surface; the rest show the still waiting form.
pub const RING_LOOPS_MAX: usize = 12;
/// Seconds without a changed report before a downloading ring counts as stalled. Provisional.
pub const STALL_AFTER: f64 = 8.0;

// ---- Waiting. Bones stay still: there is no bone pulse. ----

pub const LOADING_DELAY: f64 = 0.30;
pub const LOADING_MIN: f64 = 0.40;
pub const NAV_OPEN_DELAY: f64 = 0.12;
pub const NAV_CLOSE_DELAY: f64 = 0.30;
pub const COPIED_HOLD: f64 = 1.2;

// ---- Fixed by Electron, never retuned ----

pub const NOTICE: f64 = 6.0;
pub const NOTICE_LONG: f64 = 9.0;
pub const NEXT_PULSE: f64 = 0.6;
pub const WAIT_STILL_WORKING: f64 = 4.0;
pub const WAIT_STILL_GOING: f64 = 15.0;

// ---- Preset-only tokens, named by the catalogue row they serve ----

/// SIG-NUM-02, SIG-FLD-01: seconds between one lit segment and the next.
pub const SEGMENT_GAP: f32 = 0.012;
/// SIG-FND-01: seconds between the three beats of the found module.
pub const MODULE_BEAT: f32 = 0.06;
/// SIG-RNG-02: how long the newest segment stays bright (it relaxes on `ROW`).
pub const PEAK_HOLD: f64 = 1.0;
/// SHF-RNG-02 (delight tier): the record turns while downloading.
pub const RECORD_TURN: LoopSpec = LoopSpec::rotation(1.8);
/// SHF-CTL-01 (delight tier): the plan stamp landing.
pub const STAMP: &[Beat] = &[Beat::new(0.0).opacity(0.0).scale(1.08), Beat::new(0.3)];

// ---- Per-look personality ----

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurfaceIn {
    Light,
    Rise(f32),
    Focus { scale: f32, blur: f32 },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RowIn {
    Fade,
    Rise(f32),
    Grow(f32),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    CutIn,
    Cross,
    CrossFocus(f32),
}
/// Lit in place / one marker slides / matched label / body along a line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Indicator {
    Lamp,
    Slide,
    Label,
    Body,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Figure {
    Step,
    Cross,
}

/// Distances, scales and kinds only; every time comes from the tokens above.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub surface_in: SurfaceIn,
    pub row_in: RowIn,
    pub place: Place,
    pub nav: Indicator,
    pub marker: Indicator,
    pub figure: Figure,
    /// Blur a cover fades in from: 0 or 2.
    pub cover_focus: f32,
}

const SIGNAL: Look = Look {
    surface_in: SurfaceIn::Light,
    row_in: RowIn::Fade,
    place: Place::CutIn,
    nav: Indicator::Lamp,
    marker: Indicator::Lamp,
    figure: Figure::Step,
    cover_focus: 0.0,
};
const SHELF: Look = Look {
    surface_in: SurfaceIn::Rise(6.0),
    row_in: RowIn::Rise(8.0),
    place: Place::Cross,
    nav: Indicator::Label,
    marker: Indicator::Slide,
    figure: Figure::Cross,
    cover_focus: 0.0,
};
const ORBIT: Look = Look {
    surface_in: SurfaceIn::Focus { scale: 0.96, blur: 2.0 },
    row_in: RowIn::Grow(0.98),
    place: Place::CrossFocus(1.5),
    nav: Indicator::Body,
    marker: Indicator::Slide,
    figure: Figure::Cross,
    cover_focus: 2.0,
};

/// In the order of `gate::PRESETS`. A layout is its look with a field or two changed.
pub const LOOKS: [Look; 6] =
    [Look { nav: Indicator::Slide, ..SIGNAL }, SHELF, Look { nav: Indicator::Slide, ..ORBIT }, SIGNAL, SHELF, ORBIT];

/// The look of the preset that is on.
pub fn look(s: &crate::gate::State) -> &'static Look {
    &LOOKS[crate::gate::preset(s)]
}

// ---- What a moving token does under reduced motion ----

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reduced {
    /// Rune turns travel into its 0.18 s fade (`t::REDUCED_FADE`).
    Fade,
    /// Held at its first or resting pose.
    Still,
    /// The value lands at once.
    Snap,
}

/// Every time token: seconds, and its reduced-motion form when it moves (None = a delay or hold).
pub struct Token {
    pub name: &'static str,
    pub secs: f64,
    pub reduced: Option<Reduced>,
}

const fn tok(name: &'static str, secs: f64, reduced: Option<Reduced>) -> Token {
    Token { name, secs, reduced }
}

const fn dur(c: Curve) -> f64 {
    match c {
        Curve::Tween { duration, .. } => duration as f64,
        _ => 0.0,
    }
}

pub const TOKENS: &[Token] = &[
    tok("REVEAL_IN", dur(REVEAL.0), Some(Reduced::Fade)),
    tok("REVEAL_OUT", dur(REVEAL.1), Some(Reduced::Fade)),
    tok("CONTROL_IN", CONTROL.enter.duration as f64, Some(Reduced::Fade)),
    tok("CONTROL_OUT", CONTROL.exit.duration as f64, Some(Reduced::Fade)),
    tok("OVERLAY_IN", OVERLAY.enter.duration as f64, Some(Reduced::Fade)),
    tok("OVERLAY_OUT", OVERLAY.exit.duration as f64, Some(Reduced::Fade)),
    tok("SHEET_IN", SHEET.enter.duration as f64, Some(Reduced::Fade)),
    tok("SHEET_OUT", SHEET.exit.duration as f64, Some(Reduced::Fade)),
    tok("ROW_IN", ROW.enter.duration as f64, Some(Reduced::Fade)),
    tok("ROW_OUT", ROW.exit.duration as f64, Some(Reduced::Fade)),
    tok("REFLOW_IN", REFLOW.enter.duration as f64, Some(Reduced::Fade)),
    tok("REFLOW_OUT", REFLOW.exit.duration as f64, Some(Reduced::Fade)),
    tok("MOMENT", MOMENT.duration as f64, Some(Reduced::Fade)),
    tok("THEME", THEME.duration as f64, Some(Reduced::Snap)),
    tok("PLACE_IN", dur(PLACE.0), Some(Reduced::Fade)),
    tok("PLACE_OUT", dur(PLACE.1), Some(Reduced::Fade)),
    tok("RING_FOLLOW", RING_FOLLOW.duration as f64, Some(Reduced::Snap)),
    tok("RING_SWEEP", RING_SWEEP, Some(Reduced::Still)),
    tok("RING_BREATHE", RING_BREATHE.period, Some(Reduced::Still)),
    tok("RING_DRAW", dur(RING_DRAW), Some(Reduced::Fade)),
    tok("RING_DONE", RING_DONE[RING_DONE.len() - 1].at, Some(Reduced::Fade)),
    tok("RECORD_TURN", RECORD_TURN.period, Some(Reduced::Still)),
    tok("STAMP", STAMP[STAMP.len() - 1].at, Some(Reduced::Fade)),
    tok("RING_HOLD", RING_HOLD, None),
    tok("STALL_AFTER", STALL_AFTER, None),
    tok("LOADING_DELAY", LOADING_DELAY, None),
    tok("LOADING_MIN", LOADING_MIN, None),
    tok("NAV_OPEN_DELAY", NAV_OPEN_DELAY, None),
    tok("NAV_CLOSE_DELAY", NAV_CLOSE_DELAY, None),
    tok("COPIED_HOLD", COPIED_HOLD, None),
    tok("NOTICE", NOTICE, None),
    tok("NOTICE_LONG", NOTICE_LONG, None),
    tok("NEXT_PULSE", NEXT_PULSE, None),
    tok("WAIT_STILL_WORKING", WAIT_STILL_WORKING, None),
    tok("WAIT_STILL_GOING", WAIT_STILL_GOING, None),
    tok("PEAK_HOLD", PEAK_HOLD, None),
    tok("STAGGER", STAGGER as f64, None),
    tok("SEGMENT_GAP", SEGMENT_GAP as f64, None),
    tok("MODULE_BEAT", MODULE_BEAT as f64, None),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    const PAIRS: [(&str, SpringPair); 4] = [("CONTROL", CONTROL), ("SHEET", SHEET), ("ROW", ROW), ("REFLOW", REFLOW)];

    fn named(n: &str) -> f64 {
        TOKENS.iter().find(|t| t.name == n).unwrap_or_else(|| panic!("no token {n}")).secs
    }

    #[test]
    fn every_duration_is_positive_and_moving_tokens_have_a_reduced_form() {
        assert!(TOKENS.len() > 30);
        for t in TOKENS {
            assert!(t.secs > 0.0, "{} is {}", t.name, t.secs);
        }
        let moving = TOKENS.iter().filter(|t| t.reduced.is_some()).count();
        assert!(moving > 15, "only {moving} moving tokens");
        // The reduced-motion fade is Rune's own; its length is what the doc quotes.
        assert_eq!(dur(t::REDUCED_FADE), 0.18f32 as f64);
    }

    #[test]
    fn fast_before_normal_before_slow_and_exits_are_shorter() {
        let order = ["CONTROL_IN", "ROW_IN", "REFLOW_IN"];
        assert!(order.windows(2).all(|w| named(w[0]) < named(w[1])), "{order:?}");
        let pairs = [
            ("CONTROL_OUT", "CONTROL_IN"),
            ("OVERLAY_OUT", "OVERLAY_IN"),
            ("SHEET_OUT", "SHEET_IN"),
            ("ROW_OUT", "ROW_IN"),
            ("REFLOW_OUT", "REFLOW_IN"),
            ("REVEAL_OUT", "REVEAL_IN"),
            ("PLACE_OUT", "PLACE_IN"),
            ("NAV_OPEN_DELAY", "NAV_CLOSE_DELAY"),
            ("LOADING_DELAY", "LOADING_MIN"),
            ("NOTICE", "NOTICE_LONG"),
            ("WAIT_STILL_WORKING", "WAIT_STILL_GOING"),
        ];
        for (shorter, longer) in pairs {
            assert!(named(shorter) < named(longer), "{shorter} !< {longer}");
        }
        assert_eq!((NAV_OPEN_DELAY, NAV_CLOSE_DELAY, NOTICE, NOTICE_LONG), (0.12, 0.30, 6.0, 9.0));
    }

    #[test]
    fn ring_and_owner_decisions() {
        assert_eq!(DISSOLVE, Dissolve::FadeBlur(1.5));
        assert_eq!(DIGITS, Digits::Crossfade);
        assert_eq!(DISSOLVE_SPRING, ROW);
        assert_eq!(PLACE.0, t::CSS_ENTER);
        assert_eq!(RING_DONE.len(), 3);
        assert_eq!((Press::Button.scale(), Press::Icon.scale(), Press::Tile.scale()), (0.97, 0.94, 0.98));
        assert!(RING_LOOPS_MAX > 0 && STAGGER_MAX > 0);
    }

    #[test]
    fn one_family_every_spring_is_a_rune_pair_and_looks_share_three_bases() {
        assert!(!t::ALL.is_empty());
        for (name, pair) in PAIRS {
            assert!(t::ALL.iter().any(|(_, p)| *p == pair), "{name} is not one of Rune's pairs");
        }
        for s in [MOMENT, THEME] {
            assert!(t::ALL.iter().any(|(_, p)| p.enter == s), "{s:?} is not a Rune spring");
        }
        // Looks hold distances and kinds, not times; layouts differ from their base in `nav` only.
        assert_eq!(LOOKS.len(), 6);
        for i in 0..3 {
            assert_eq!(Look { nav: LOOKS[i + 3].nav, ..LOOKS[i] }, LOOKS[i + 3], "layout {i} differs from its look beyond nav");
        }
        assert_eq!([LOOKS[3], LOOKS[4], LOOKS[5]], [SIGNAL, SHELF, ORBIT]);
    }

    #[test]
    fn no_bounce_anywhere() {
        let springs = [CONTROL.enter, CONTROL.exit, OVERLAY.enter, OVERLAY.exit, SHEET.enter, SHEET.exit, ROW.enter, ROW.exit]
            .into_iter()
            .chain([REFLOW.enter, REFLOW.exit, MOMENT, THEME, RING_FOLLOW]);
        let mut n = 0;
        for s in springs {
            assert!(s.bounce <= 0.0, "{s:?} overshoots");
            n += 1;
        }
        assert!(n > 10);
        // No eased token may leave 0..=1 either (a cubic with y outside it overshoots).
        let easings = [Easing::ENTER, Easing::EXIT, Easing::DRAW_ON, Easing::EASE_IN_OUT]
            .into_iter()
            .chain([REVEAL.0, REVEAL.1, PLACE.0, PLACE.1, RING_DRAW].into_iter().filter_map(|c| match c {
                Curve::Tween { easing, .. } => Some(easing),
                _ => None,
            }))
            .chain(RING_DONE.iter().chain(STAMP).map(|b| b.easing));
        for e in easings {
            for i in 0..=100 {
                let y = e.eval(i as f32 / 100.0);
                assert!((-1e-4..=1.0 + 1e-4).contains(&y), "{e:?} overshoots at {i}: {y}");
            }
        }
    }

    // ---- The lint: no new timing outside this file ----

    /// Substrings that construct or decide a time. A line holding one outside motion.rs must be allowed below.
    const FORBIDDEN: &[&str] = &[
        "Duration::",
        "from_millis",
        "from_secs",
        ".ms(",
        "Spring::",
        "Curve::",
        "LoopSpec::",
        "Sequence::",
        "Beat::",
        "Cycle {",
        "Tween::",
        "tok::motion",
        "press_scale(",
        "enter_delay(",
        "time_scale(",
        ".transition(",
        "Transition::",
        "tx.animated(",
        "tx.instant(",
        "tx.motion(",
        "tx.color_motion(",
        "tx.wake_at(",
        "tx.wake_after(",
    ];

    /// (file under src/, forbidden substring, most lines that may hold it, why it exists today).
    /// Later steps move these into the vocabulary and shrink the rows; a row may only go down.
    const ALLOW: &[(&str, &str, usize, &str)] = &[
        ("main.rs", ".transition(", 1, "the whole-window dissolve, built from DISSOLVE and DISSOLVE_SPRING (M6, step 47)"),
        ("main.rs", "Transition::", 1, "same line: Transition::blur(DISSOLVE's blur).springs(DISSOLVE_SPRING)"),
        ("picker.rs", ".transition(", 1, "the picker's opacity swap; becomes PLACE (M6)"),
        ("picker.rs", "Transition::", 1, "same line: rune::Transition::opacity() (M6)"),
        (
            "overlays.rs",
            "press_scale(",
            2,
            "a button press through Press::Button.scale(); the call itself stays until the Moves verbs exist",
        ),
        ("picker.rs", "press_scale(", 1, "a tile press, 0.98 = Press::Tile (presets step)"),
        ("directions/orbit.rs", "press_scale(", 2, "button 0.97 and icon 0.94 = Press::Button / Press::Icon"),
        ("directions/orbit/home.rs", "press_scale(", 1, "button 0.97 = Press::Button"),
        ("layouts/stage/parts.rs", "press_scale(", 2, "button 0.97 and icon 0.94 = Press::Button / Press::Icon"),
    ];

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).expect("read src dir") {
            let p = e.expect("dir entry").path();
            if p.is_dir() {
                rust_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    #[test]
    fn no_timing_outside_motion_rs_beyond_the_allow_list() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rust_files(&src, &mut files);
        assert!(files.len() > 10, "the scan found {} files", files.len());
        assert!(!ALLOW.is_empty() && ALLOW.iter().all(|r| !r.3.is_empty()));
        let mut bad = Vec::new();
        let mut scanned_lines = 0usize;
        for f in &files {
            let rel = f.strip_prefix(&src).unwrap().to_string_lossy().replace('\\', "/");
            if rel == "motion.rs" {
                continue;
            }
            let text = std::fs::read_to_string(f).expect("read source");
            // Everything from a file's test module on is test code; its clocks are not app timing.
            // Only an inline `#[cfg(test)] mod x {` cuts: a `#[cfg(test)] mod x;` declaration (main.rs) does not.
            let lines: Vec<&str> = text.lines().collect();
            let cut = lines.windows(2).position(|w| w[0].trim() == "#[cfg(test)]" && w[1].trim_end().ends_with('{')).unwrap_or(lines.len());
            let code_lines: Vec<&str> = lines[..cut].iter().copied().filter(|l| !l.trim_start().starts_with("//")).collect();
            scanned_lines += code_lines.len();
            for pat in FORBIDDEN {
                let found = code_lines.iter().filter(|l| l.contains(pat)).count();
                let allowed = ALLOW.iter().find(|r| r.0 == rel && r.1 == *pat).map_or(0, |r| r.2);
                if found > allowed {
                    bad.push(format!("{rel}: {found} line(s) with `{pat}`, {allowed} allowed"));
                }
            }
        }
        assert!(scanned_lines > 1000, "scanned only {scanned_lines} lines");
        assert!(bad.is_empty(), "new timing outside motion.rs (put it in the vocabulary):\n{}", bad.join("\n"));
    }

    #[test]
    fn the_lint_catches_a_new_timing() {
        // The same counting rule, on a made-up file: one hit, none allowed.
        let code = "let a = 1;\nel().press_scale(0.9)\n// press_scale(0.5)";
        let lines: Vec<&str> = code.lines().filter(|l| !l.trim_start().starts_with("//")).collect();
        assert_eq!(lines.iter().filter(|l| l.contains("press_scale(")).count(), 1);
        assert!(ALLOW.iter().all(|r| FORBIDDEN.contains(&r.1)));
    }
}
