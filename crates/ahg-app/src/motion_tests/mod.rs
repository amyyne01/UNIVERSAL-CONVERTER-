//! Motion tests, one module per class (a later step fills the empty ones).
//!
//! The set-up below repeats the pattern of `behaviour_tests.rs` (whose helpers are private to it
//! and may not be edited from here): the real spec on Rune's scripted runtime, so "motion needs a
//! cause" is on and `poll` runs.
use rune::{App, Env, Runtime};

use crate::gate::{PRESETS, SCENES, State};

mod feedback;
mod first_frame;
mod ids;
mod loading;
mod nav;
mod overlays;
mod reduced;
mod ring;
mod rows;
mod switch;
mod vocabulary;

/// One frame at 60 Hz, in seconds.
const DT: f64 = 1.0 / 60.0;

/// The real app on `preset`, light or dark, with the named scene applied before the first frame.
/// Nothing is drawn yet: the caller steps `frame`.
fn runtime(preset: usize, scene: Option<&str>, dark: bool) -> Runtime<State> {
    let apply = scene.map(|n| SCENES.iter().find(|s| s.0 == n).unwrap_or_else(|| panic!("no scene `{n}`")).1);
    App::new(crate::spec())
        .init(move || {
            let mut s = State::sample(preset);
            if let Some(f) = apply {
                f(&mut s);
            }
            s.sync();
            s
        })
        .headless_runtime((1240.0, 820.0), Env { system_dark: dark, scale: 1.0 })
}

/// `n` frames of 1/60 s.
fn steps(rt: &mut Runtime<State>, n: usize) {
    for _ in 0..n {
        rt.frame(DT);
    }
}

/// The name of every preset, in `PRESETS` order (a failing case names it).
fn name(preset: usize) -> &'static str {
    PRESETS[preset].0
}

#[test]
fn the_harness_builds_every_preset_and_scene() {
    assert!(!PRESETS.is_empty() && !SCENES.is_empty());
    for p in 0..PRESETS.len() {
        let mut rt = runtime(p, Some(SCENES[0].0), true);
        rt.frame(0.0);
        assert!(!rt.elements().is_empty(), "preset {}: nothing on the first frame", name(p));
        assert_eq!(crate::gate::preset(&rt.state), p);
    }
}
