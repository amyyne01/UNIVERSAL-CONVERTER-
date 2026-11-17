//! The structural half of the preset switch: `main.rs::view` keys the shown preset by its place in
//! `PRESETS`, so a switch replaces one keyed element with another and Rune dissolves between them.
//! (The timed half, MOTION-SYSTEM.md 4.10, arrives with step M6.)
use super::{name, runtime, steps};
use crate::actions;
use crate::gate::PRESETS;

fn shown(rt: &rune::Runtime<crate::gate::State>, p: usize) -> Option<rune::ElementInfo> {
    let key = format!("preset-{p}");
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&key))
}

#[test]
fn every_ordered_pair_swaps_the_keyed_preset() {
    let n = PRESETS.len();
    let mut pairs = 0;
    for from in 0..n {
        for to in (0..n).filter(|t| *t != from) {
            let mut rt = runtime(from, None, true);
            rt.frame(0.0);
            let case = format!("{} -> {}", name(from), name(to));
            assert!(shown(&rt, from).is_some_and(|e| !e.exiting), "{case}: the old preset is not shown before the switch");
            assert!(shown(&rt, to).is_none(), "{case}: the new key exists before the switch");
            rt.update(move |s, tx| actions::set_preset(s, to, tx));
            rt.frame(0.0);
            assert!(shown(&rt, to).is_some_and(|e| !e.exiting), "{case}: the new preset is not shown");
            assert!(shown(&rt, from).is_none_or(|e| e.exiting), "{case}: the old preset is still live");
            // The old one is gone after the dissolve.
            steps(&mut rt, 60);
            assert!(shown(&rt, from).is_none(), "{case}: the old preset never left");
            pairs += 1;
        }
    }
    assert_eq!(pairs, n * (n - 1));
}
