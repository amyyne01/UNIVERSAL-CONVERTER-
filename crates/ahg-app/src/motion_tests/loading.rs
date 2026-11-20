//! Loading: while something is fetched its bones shimmer (owner, 2026-11-20: "no shimmering when
//! fetching", reversing DECISIONS Q8's still bones), in every preset, both themes, every palette.
use super::first_frame::LOADING;
use super::{name, runtime, steps};
use crate::gate::PRESETS;
use crate::timeline::SHEEN;

/// A shimmer band: its path, its loop's phase, its rect and its parent's.
type Band = (String, Option<f64>, [f32; 4], [f32; 4]);

fn sheens(rt: &rune::Runtime<crate::gate::State>) -> Vec<Band> {
    let els = rt.elements();
    els.iter()
        .filter(|e| e.key.as_deref() == Some(SHEEN))
        .map(|e| (e.path.clone(), e.loop_phase, e.rect, e.parent.map_or([0.0; 4], |i| els[i].rect)))
        .collect()
}

#[test]
fn every_preset_shimmers_its_bones_while_loading_and_nowhere_else() {
    assert!(!PRESETS.is_empty() && !LOADING.is_empty());
    for p in 0..PRESETS.len() {
        for (dark, palette) in [(true, 0), (false, 3), (true, 7)] {
            for scene in LOADING {
                let case = format!("{} / {} / palette {palette} / {scene}", name(p), if dark { "dark" } else { "light" });
                let mut rt = runtime(p, Some(scene), dark);
                rt.frame(0.0);
                rt.update(move |s, _| s.palette = palette);
                assert!(!sheens(&rt).is_empty(), "{case}: no shimmer on the bones");
                // Past the show delay, mid-sweep: the band moves from one frame to the next.
                steps(&mut rt, 40);
                let a = sheens(&rt);
                steps(&mut rt, 3);
                let b = sheens(&rt);
                assert!(a.iter().all(|x| x.1.is_some()), "{case}: a band without its loop");
                let moved = a.iter().zip(&b).filter(|(x, y)| x.0 == y.0 && x.1 != y.1).count();
                assert_eq!(moved, a.len(), "{case}: {moved} of {} bands moved", a.len());
            }
        }
        for scene in ["media", "search", "home-history", "downloads-everything"] {
            let mut rt = runtime(p, Some(scene), true);
            rt.frame(0.0);
            assert!(sheens(&rt).is_empty(), "{} / {scene}: a shimmer with nothing loading", name(p));
        }
    }
}

#[test]
fn under_reduced_motion_the_band_rests_off_its_bone() {
    for p in 0..PRESETS.len() {
        let mut rt = runtime(p, Some("looking-up"), true);
        rt.set_system_reduced_motion(true);
        rt.frame(0.0);
        steps(&mut rt, 60);
        let bands = sheens(&rt);
        assert!(!bands.is_empty(), "{}: no bones", name(p));
        for (path, _, band, bone) in bands {
            assert!(band[0] >= bone[0] + bone[2] - 0.5, "{} {path}: the band {band:?} sits on its bone {bone:?}", name(p));
        }
    }
}
