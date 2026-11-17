//! Nothing animates on the first frame: for every preset, theme and scene, frame 0 has no element
//! that is moving, exiting, revealing or scrolling (a loop that is due is the one allowed motion).
use super::{name, runtime, steps};
use crate::gate::{PRESETS, SCENES};

/// What is in motion on this runtime's current frame, as `path: flags`.
fn in_motion(rt: &rune::Runtime<crate::gate::State>, loops_ok: bool) -> Vec<String> {
    rt.elements()
        .iter()
        .filter(|e| (e.moving && (!loops_ok || e.loop_phase.is_none())) || e.exiting || e.revealing || e.scrolling)
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
                        for (scene, _) in SCENES {
                            let mut rt = runtime(p, Some(scene), dark);
                            rt.frame(0.0);
                            cases += 1;
                            let theme = if dark { "dark" } else { "light" };
                            let moving = in_motion(&rt, true);
                            if !moving.is_empty() {
                                bad.push(format!("{} / {theme} / {scene} / frame 0: {}", name(p), moving.join("; ")));
                            }
                            // No input for half a second: a scene at rest has nothing moving either
                            // (a loop or tween started by the view itself would show here).
                            steps(&mut rt, 30);
                            let moving = in_motion(&rt, false);
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
    assert_eq!(cases, PRESETS.len() * 2 * SCENES.len());
    assert!(bad.is_empty(), "{} case(s) animate on frame 0:\n{}", bad.len(), bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}
