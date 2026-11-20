//! Nothing animates on the first frame: for every preset, theme and scene, frame 0 has no element
//! that is moving, exiting, revealing or scrolling (a loop that is due is the one allowed motion).
//! At rest the one motion allowed is the bones' shimmer, in the scenes that wait for something.
use super::{name, runtime, steps};
use crate::gate::{PRESETS, SCENES};

/// What is in motion on this runtime's current frame, as `path: flags`; `may_loop` names the loops allowed.
fn in_motion(rt: &rune::Runtime<crate::gate::State>, may_loop: impl Fn(&rune::ElementInfo) -> bool) -> Vec<String> {
    rt.elements()
        .iter()
        .filter(|e| (e.moving && !(e.loop_phase.is_some() && may_loop(e))) || e.exiting || e.revealing || e.scrolling)
        .map(|e| {
            format!(
                "{} (moving {}, exiting {}, revealing {}, scrolling {})",
                e.key.as_deref().unwrap_or(&e.path),
                e.moving,
                e.exiting,
                e.revealing,
                e.scrolling
            )
        })
        .collect()
}

/// The scenes with bones: their shimmer loops on (owner, 2026-11-20), every other scene rests.
pub const LOADING: [&str; 3] = ["looking-up", "search-loading", "playlist"];

/// Light theme runs every this-many scenes (a full light pass cost minutes; dark covers every scene).
const LIGHT_EVERY: usize = 4;

#[test]
fn frame_zero_is_still_in_every_preset_theme_and_scene() {
    assert!(!PRESETS.is_empty() && !SCENES.is_empty());
    // One thread per preset: 12 x 41 headless runtimes are slow in a row.
    let per_preset: Vec<(usize, Vec<String>)> = std::thread::scope(|sc| {
        let jobs: Vec<_> = (0..PRESETS.len())
            .map(|p| {
                sc.spawn(move || {
                    let (mut cases, mut bad) = (0, Vec::new());
                    for dark in [true, false] {
                        for (i, (scene, _)) in SCENES.iter().enumerate() {
                            // Dark in every scene, light in every fourth: the same code draws both themes.
                            if !dark && i % LIGHT_EVERY != 0 {
                                continue;
                            }
                            let mut rt = runtime(p, Some(scene), dark);
                            rt.frame(0.0);
                            cases += 1;
                            let theme = if dark { "dark" } else { "light" };
                            let moving = in_motion(&rt, |_| true);
                            if !moving.is_empty() {
                                bad.push(format!("{} / {theme} / {scene} / frame 0: {}", name(p), moving.join("; ")));
                            }
                            // No input for half a second: a scene at rest has nothing moving either
                            // (a loop or tween started by the view itself would show here).
                            steps(&mut rt, 30);
                            let shimmers = LOADING.contains(scene);
                            let moving = in_motion(&rt, |e| shimmers && e.key.as_deref() == Some(crate::timeline::SHEEN));
                            if !moving.is_empty() {
                                bad.push(format!("{} / {theme} / {scene} / frame 31: {}", name(p), moving.join("; ")));
                            }
                        }
                    }
                    (cases, bad)
                })
            })
            .collect();
        jobs.into_iter().map(|j| j.join().expect("a preset thread panicked")).collect()
    });
    let cases: usize = per_preset.iter().map(|r| r.0).sum();
    let bad: Vec<String> = per_preset.into_iter().flat_map(|r| r.1).collect();
    assert_eq!(cases, PRESETS.len() * (SCENES.len() + SCENES.len().div_ceil(LIGHT_EVERY)));
    assert!(bad.is_empty(), "{} case(s) animate on frame 0:\n{}", bad.len(), bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}
