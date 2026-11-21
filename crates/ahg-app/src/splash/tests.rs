//! The splash through Rune's scripted runtime, in all six presets, dark and light: drawn first and
//! alone, gone only after its floor and every required stage, its bar the real done share.
use rune::{App, Env, Runtime};

use super::*;
use crate::gate::{Nav, PRESETS};

/// One frame at 60 Hz.
const DT: f64 = 1.0 / 60.0;

fn runtime(preset: usize, dark: bool, stages: &'static [Stage], floor: Option<f64>, reduced: bool) -> Runtime<State> {
    let mut rt = App::new(crate::spec())
        .init(move || {
            let mut s = State::sample(preset);
            let mut sp = Splash::new(stages);
            sp.floor = floor;
            s.splash = Some(sp);
            s.sync();
            s
        })
        .headless_runtime((1240.0, 820.0), Env { system_dark: dark, scale: 1.0 });
    rt.set_system_reduced_motion(reduced);
    rt
}

/// Frames until the clock reaches `t` seconds.
fn to(rt: &mut Runtime<State>, t: f64) {
    while rt.clock < t {
        rt.frame(DT);
    }
}

fn up(rt: &Runtime<State>) -> bool {
    rt.elements().iter().any(|e| !e.exiting && e.key.as_deref() == Some(keys::SPLASH))
}

fn preset_drawn(rt: &Runtime<State>) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("preset-")))
}

fn cells() -> Vec<(usize, bool, String)> {
    let all: Vec<_> = (0..PRESETS.len())
        .flat_map(|p| [true, false].map(|d| (p, d, format!("{} {}", PRESETS[p].0, if d { "dark" } else { "light" }))))
        .collect();
    assert_eq!(all.len(), 12, "six presets, two themes");
    all
}

#[test]
fn the_stages_are_in_order_their_weights_sum_to_one_and_the_layout_waits_for_the_data() {
    let total: f32 = STAGES.iter().map(|s| s.weight).sum();
    assert!((total - 1.0).abs() < 1e-6, "the weights sum to {total}");
    assert!(STAGES.iter().all(|s| s.weight > 0.0 && !s.label.is_empty()));
    let at = |label: &str| STAGES.iter().position(|s| s.label == label).unwrap();
    assert!(at(words::STAGE_ENGINE) < at(words::STAGE_SETTINGS));
    assert!(at(words::STAGE_SETTINGS) < at(words::STAGE_LAYOUT) && at(words::STAGE_DOWNLOADS) < at(words::STAGE_LAYOUT));
    assert!(STAGES[at(words::STAGE_LAYOUT)].required && !STAGES[at(words::STAGE_SHELL)].required && !STAGES[at(words::STAGE_ART)].required);
}

/// The first frame is the splash alone: no preset under it, no stage begun.
#[test]
fn the_first_frame_is_the_splash_alone_in_every_preset() {
    for (p, dark, name) in cells() {
        let mut rt = runtime(p, dark, STAGES, None, false);
        rt.frame(0.0);
        assert!(up(&rt), "{name}: no splash on the first frame");
        assert!(!preset_drawn(&rt), "{name}: the preset is built behind the first frame");
        let sp = rt.state.splash.as_ref().unwrap();
        assert!(sp.status.iter().all(|st| *st == Status::Waiting), "{name}: a stage began before the first frame: {:?}", sp.status);
        // Then the work starts, and the preset is built under the splash before it goes.
        rt.frame(DT);
        assert_ne!(rt.state.splash.as_ref().unwrap().status[0], Status::Waiting, "{name}: nothing began on the next turn");
        to(&mut rt, 0.5);
        assert!(up(&rt) && preset_drawn(&rt), "{name}: the preset is not under the splash before it goes");
    }
}

/// A first launch: loaded at once (sample mode), the full bar holds until 3.4 s; then the splash
/// goes, the preset under it has not moved, and a press reaches it.
#[test]
fn a_first_launch_holds_the_full_bar_until_its_floor_then_hands_over_without_a_shift() {
    for (p, dark, name) in cells() {
        let mut rt = runtime(p, dark, STAGES, Some(motion::SPLASH_FLOOR), false);
        rt.frame(0.0);
        to(&mut rt, 1.0);
        let sp = rt.state.splash.as_ref().unwrap_or_else(|| panic!("{name}: gone before its floor"));
        assert_eq!(sp.bar, 1.0, "{name}: everything is loaded, the bar is not full");
        let under = rt.elements().iter().find(|e| e.key.as_deref() == Some(keys::nav(Nav::Downloads))).map(|e| e.target);
        to(&mut rt, motion::SPLASH_FLOOR - 0.05);
        assert!(up(&rt), "{name}: dismissed before the 3.4 s floor");
        to(&mut rt, motion::SPLASH_FLOOR + motion::SPLASH_HOLD + 0.6);
        assert!(rt.state.splash.is_none() && !up(&rt), "{name}: still up after the floor");
        assert!(rt.find(keys::SPLASH).is_none(), "{name}: the splash is still drawn after its fade");
        let after = rt.elements().iter().find(|e| e.key.as_deref() == Some(keys::nav(Nav::Downloads))).map(|e| e.target);
        assert!(under.is_some(), "{name}: no Downloads button under the splash");
        assert_eq!(under, after, "{name}: the preset moved when the splash went");
        // Input reaches the app.
        let (x, y) = rt.find(keys::nav(Nav::Downloads)).unwrap();
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        for _ in 0..20 {
            rt.frame(DT);
        }
        assert_eq!(rt.state.nav, Nav::Downloads, "{name}: the press did not reach the app after the splash");
    }
}

/// A later launch: gone by its 1 s floor (plus the hold), still up just before it.
#[test]
fn a_later_launch_has_the_warm_floor_in_every_preset() {
    for (p, dark, name) in cells() {
        let mut rt = runtime(p, dark, STAGES, Some(motion::SPLASH_FLOOR_WARM), false);
        rt.frame(0.0);
        to(&mut rt, motion::SPLASH_FLOOR_WARM - 0.05);
        assert!(up(&rt), "{name}: dismissed before the warm floor");
        to(&mut rt, motion::SPLASH_FLOOR_WARM + motion::SPLASH_HOLD + 0.1);
        assert!(rt.state.splash.is_none(), "{name}: still up after the warm floor");
    }
}

/// Reduced motion: the wordmark is plain from the first frame (no reveal), the bar stands at the
/// loaded share (no sweep), and the floors are the same.
#[test]
fn reduced_motion_has_no_reveal_or_sweep_and_the_same_floors() {
    for (p, dark, name) in cells() {
        let mut rt = runtime(p, dark, STAGES, Some(motion::SPLASH_FLOOR), true);
        rt.frame(0.0);
        let text: String = rt.elements().iter().filter_map(|e| e.text.clone()).collect();
        assert!(text.contains("AHGUniversal Converter"), "{name}: the wordmark scrambles under reduced motion: {text:?}");
        to(&mut rt, motion::SPLASH_FLOOR - 0.05);
        assert!(up(&rt), "{name}: reduced motion shortened the floor");
        to(&mut rt, motion::SPLASH_FLOOR + motion::SPLASH_HOLD + 0.6);
        assert!(rt.state.splash.is_none(), "{name}: reduced motion kept the splash");
    }
}

// ── The bar and the stages, with stand-ins

fn waiting(s: &State) -> f64 {
    s.splash.as_ref().map_or(f64::MAX, |sp| sp.now - sp.shown.unwrap_or(sp.now))
}

/// Done 2 s after the splash showed.
fn slow(s: &State) -> Option<Result<(), String>> {
    (waiting(s) >= 2.0).then_some(Ok(()))
}

fn never(_: &State) -> Option<Result<(), String>> {
    None
}

fn broken(_: &State) -> Option<Result<(), String>> {
    Some(Err("it broke".into()))
}

const SLOW: &[Stage] = &[
    Stage { label: "a", weight: 0.25, required: true, begin: nothing, done: at_once },
    Stage { label: "b", weight: 0.5, required: true, begin: nothing, done: slow },
    Stage { label: "c", weight: 0.25, required: true, begin: nothing, done: at_once },
];
const FAILING: &[Stage] = &[
    Stage { label: "a", weight: 0.5, required: true, begin: nothing, done: at_once },
    Stage { label: "the thing", weight: 0.5, required: true, begin: nothing, done: broken },
];
const OPTIONAL: &[Stage] = &[
    Stage { label: "a", weight: 0.5, required: true, begin: nothing, done: at_once },
    Stage { label: "b", weight: 0.5, required: false, begin: nothing, done: never },
];
const HUNG: &[Stage] = &[Stage { label: "the hung thing", weight: 1.0, required: true, begin: nothing, done: never }];

#[test]
fn the_bar_never_goes_back_and_comes_to_the_done_share() {
    let mut rt = runtime(0, true, SLOW, Some(motion::SPLASH_FLOOR_WARM), false);
    rt.frame(0.0);
    let mut last = 0.0;
    while rt.clock < 1.5 {
        rt.frame(DT);
        let sp = rt.state.splash.as_ref().unwrap();
        assert!(sp.bar >= last, "the bar went back from {last} to {}", sp.bar);
        assert!(sp.bar <= sp.loaded() + 1e-6, "the bar {} is ahead of the done share {}", sp.bar, sp.loaded());
        last = sp.bar;
    }
    // "a" is done, "b" holds "c" back: the bar has come to a quarter and stays there.
    let sp = rt.state.splash.as_ref().unwrap();
    assert_eq!((sp.loaded(), sp.bar), (0.25, 0.25));
    assert_eq!(sp.status, [Status::Done, Status::Going, Status::Waiting]);
}

#[test]
fn a_slow_stage_holds_the_bar_and_the_splash_past_the_floor() {
    let mut rt = runtime(0, true, SLOW, Some(motion::SPLASH_FLOOR_WARM), false);
    rt.frame(0.0);
    to(&mut rt, 1.9);
    assert!(up(&rt), "gone with a required stage going");
    assert!(rt.state.splash.as_ref().unwrap().bar < 1.0);
    to(&mut rt, 2.0 + motion::SPLASH_COMPLETE + motion::SPLASH_HOLD + 0.2);
    assert!(rt.state.splash.is_none(), "still up after every stage was done");
}

#[test]
fn a_failed_required_stage_shows_the_app_with_a_notice() {
    let mut rt = runtime(0, true, FAILING, Some(motion::SPLASH_FLOOR), false);
    rt.frame(0.0);
    to(&mut rt, 0.2);
    assert!(rt.state.splash.is_none(), "an endless splash after a failure");
    let notice = rt.state.notices.first().expect("no notice");
    assert_eq!((notice.tone, notice.text.as_str()), (Tone::Error, "Could not load the thing: it broke"));
    to(&mut rt, 1.0);
    assert!(preset_drawn(&rt));
}

#[test]
fn an_optional_stage_never_holds_the_splash() {
    let mut rt = runtime(0, true, OPTIONAL, Some(motion::SPLASH_FLOOR_WARM), false);
    rt.frame(0.0);
    to(&mut rt, 0.6);
    assert_eq!(rt.state.splash.as_ref().unwrap().bar, 1.0, "the bar waits for an optional stage");
    to(&mut rt, motion::SPLASH_FLOOR_WARM + motion::SPLASH_HOLD + 0.1);
    assert!(rt.state.splash.is_none());
}

#[test]
fn a_stall_names_what_is_loading_and_a_hang_ends_with_a_notice() {
    let mut rt = runtime(0, true, HUNG, Some(motion::SPLASH_FLOOR_WARM), false);
    rt.frame(0.0);
    let drawn = |rt: &Runtime<State>, words: &str| {
        let text: String = rt.elements().iter().filter(|e| !e.exiting).filter_map(|e| e.text.clone()).collect();
        text.contains(words)
    };
    while rt.clock < motion::SPLASH_STALL + 1.0 {
        rt.frame(0.25);
    }
    assert!(drawn(&rt, "Still loading the hung thing"), "the stall line is not shown");
    while rt.clock < motion::SPLASH_GIVE_UP + 0.5 {
        rt.frame(0.5);
    }
    assert!(rt.state.splash.is_none(), "an endless splash");
    assert_eq!(rt.state.notices[0].text, "Could not load the hung thing: it took too long");
}

#[test]
fn the_shuffle_shows_all_twenty_before_a_repeat() {
    let n = words::MESSAGES.len();
    for seed in [0, 7, 123_456_789] {
        let mut seen: Vec<usize> = (0..n).map(|k| shuffled(seed, k)).collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..n).collect::<Vec<_>>(), "seed {seed}");
    }
}

/// The launch's costs in this build, printed (`cargo test -p ahg-app splash::tests::costs -- --ignored --nocapture`):
/// fonts and setup, the first view (the splash alone), a frame with the preset, and
/// `Core::start` on an empty folder (the engine stage's one turn on the UI thread).
#[test]
#[ignore]
fn costs() {
    use std::time::Instant;
    // `SPLASH_COSTS=old`: the first view as it was before the splash, measured cold (alone).
    if std::env::var("SPLASH_COSTS").as_deref() == Ok("old") {
        let mut old = App::new(crate::spec()).headless_runtime((1240.0, 820.0), Env { system_dark: true, scale: 1.0 });
        old.state.splash = None;
        let at = Instant::now();
        old.frame(0.0);
        println!("without the splash, the first view (the preset) {:?}", at.elapsed());
        return;
    }
    let at = Instant::now();
    let mut rt = App::new(crate::spec()).headless_runtime((1240.0, 820.0), Env { system_dark: true, scale: 1.0 });
    let made = at.elapsed();
    let at = Instant::now();
    rt.frame(0.0);
    let first = at.elapsed();
    let at = Instant::now();
    if let Some(sp) = rt.state.splash.as_mut() {
        sp.under = true;
    }
    rt.update(|_, _| {});
    rt.frame(DT);
    let preset = at.elapsed();
    // A splash commit (the scramble's next step) with the preset already under it.
    let at = Instant::now();
    rt.update(|s, _| {
        if let Some(sp) = s.splash.as_mut() {
            sp.now += motion::SPLASH_GLYPH_SWAP;
        }
    });
    rt.frame(DT);
    println!("a splash commit with the preset under it {:?}", at.elapsed());
    let dir = std::env::temp_dir().join(format!("ahg-splash-costs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let at = Instant::now();
    crate::live::attach(&mut rt.state, &dir, std::sync::Arc::new(|| {}), |paths, plan, events| {
        ahg_runtime::core::Core::start_with(
            paths,
            std::sync::Arc::new(ahg_runtime::ytdlp::YtDlp::new("fake_ytdlp.exe".into(), None)),
            plan,
            events,
            ahg_runtime::core::Outside::standin(),
        )
    });
    let core = at.elapsed();
    rt.state.live = None;
    println!(
        "runtime (fonts + setup) {made:?}, first view (the splash) {first:?}, a frame with the preset under it {preset:?}, Core::start {core:?}"
    );
}

#[test]
fn the_waveform_is_the_path_of_index_html_twice() {
    let pts = wave();
    assert_eq!(pts.first(), Some(&[0.0, 20.0]));
    assert_eq!(pts.last(), Some(&[2.0 * PERIOD, 20.0]));
    assert!(pts.windows(2).all(|w| w[1][0] >= w[0][0]), "x goes back");
    assert!(pts.iter().all(|p| (6.0..=32.0).contains(&p[1])));
}
