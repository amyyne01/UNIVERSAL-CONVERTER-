//! The three places: each has a button key where the preset uses `keys.rs` (most do not yet), and
//! Ctrl+1/2/3 reach them everywhere. The slide, lamp and rail timings arrive with the lane steps.
use rune::{Key, KeyEvent, Mods};

use super::{name, runtime, steps};
use crate::gate::{Nav, PRESETS};
use crate::keys;

const PLACES: [Nav; 3] = [Nav::Home, Nav::Downloads, Nav::Settings];

#[test]
fn ctrl_1_2_3_reach_the_three_places_in_every_preset() {
    assert!(!PRESETS.is_empty());
    for p in 0..PRESETS.len() {
        let mut rt = runtime(p, None, true);
        rt.frame(0.0);
        for (i, place) in PLACES.iter().enumerate().rev() {
            let c = char::from(b'1' + i as u8);
            rt.key(KeyEvent { key: Key::Command(c), mods: Mods { cmd: true, ctrl: true, ..Mods::default() } });
            steps(&mut rt, 10);
            assert_eq!(rt.state.nav, *place, "preset {}: Ctrl+{c}", name(p));
        }
    }
}

/// The keyed buttons that exist today, per preset, on the preset's Home.
fn present(p: usize) -> Vec<Nav> {
    let mut rt = runtime(p, None, true);
    rt.frame(0.0);
    PLACES.iter().copied().filter(|n| rt.find(keys::nav(*n)).is_some()).collect()
}

#[test]
fn a_preset_that_keys_its_places_keys_all_three() {
    assert!(!PRESETS.is_empty());
    for p in 0..PRESETS.len() {
        let found = present(p);
        // Partial keying (one or two of three) is a half-done preset; none yet is allowed today.
        assert!(found.is_empty() || found.len() == 3, "preset {}: keys only {found:?}", name(p));
    }
}

#[test]
#[ignore = "red until steps 54/55: presets do not use keys.rs yet"]
fn every_preset_keys_its_three_places() {
    assert!(!PRESETS.is_empty());
    for p in 0..PRESETS.len() {
        assert_eq!(present(p), PLACES, "preset {}", name(p));
    }
}
