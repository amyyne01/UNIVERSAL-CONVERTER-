//! The vocabulary the catalogue leans on: every timing token a row names exists in `motion.rs`,
//! and the per-look table lines up with the presets.
use std::collections::HashSet;

use crate::gate::PRESETS;
use crate::motion::{LOOKS, Press, TOKENS};

/// The distinct timing tokens in the "Timing" column of MOTION.md's 165 rows.
const USED: &[&str] = &[
    "CONTROL",
    "REFLOW",
    "ROW_IN",
    "ROW_OUT",
    "PLACE",
    "MOMENT",
    "REVEAL",
    "HOVER",
    "OVERLAY_IN",
    "OVERLAY_OUT",
    "THEME",
    "RING_SWEEP",
    "RING_DRAW",
    "RING_FOLLOW",
    "ICON_SWAP",
    "SHEET_IN",
    "SHEET_OUT",
    "PRESET_SWITCH",
    "PRESS_BUTTON",
    "PRESS_ICON",
    "STEP",
    "GLIDE",
    "Live",
];

/// Named in the catalogue, and not a `TOKENS` entry: a Rune value, or a verb/enum of `motion.rs`.
const ELSEWHERE: &[&str] = &["HOVER", "PRESS_BUTTON", "PRESS_ICON", "STEP", "GLIDE", "Live"];

/// Named in the catalogue and not built yet (M1 left them out of `motion.rs`).
const PENDING: &[&str] = &["ICON_SWAP", "PRESET_SWITCH"];

/// A name resolves when it is a token, or the in/out pair of one.
fn resolves(n: &str) -> bool {
    let names: HashSet<_> = TOKENS.iter().map(|t| t.name).collect();
    names.contains(n) || names.contains(format!("{n}_IN").as_str())
}

#[test]
fn every_token_a_row_names_is_in_the_vocabulary_or_listed_as_elsewhere_or_pending() {
    assert!(USED.len() > 20);
    for n in USED {
        let known = resolves(n) || ELSEWHERE.contains(n) || PENDING.contains(n);
        assert!(known, "`{n}` is named by a catalogue row and is neither a token nor listed");
    }
    // The two lists stay honest: a pending name that was built, or an elsewhere name that is a token, is stale.
    for n in PENDING.iter().chain(ELSEWHERE) {
        assert!(!resolves(n), "`{n}` is a token now: take it off the list");
        assert!(USED.contains(n), "`{n}` is on a list but no row uses it");
    }
}

#[test]
#[ignore = "red until the tokens ICON_SWAP and PRESET_SWITCH exist in motion.rs"]
fn nothing_is_pending() {
    assert!(PENDING.is_empty(), "{PENDING:?}");
}

#[test]
fn token_names_are_unique() {
    assert!(!TOKENS.is_empty());
    let names: HashSet<_> = TOKENS.iter().map(|t| t.name).collect();
    assert_eq!(names.len(), TOKENS.len());
}

#[test]
fn one_look_per_preset_and_a_layout_wears_its_direction() {
    assert_eq!(LOOKS.len(), PRESETS.len());
    for (i, (name, direction, layout)) in PRESETS.iter().enumerate() {
        if layout.is_some() {
            let twin = PRESETS.iter().position(|p| p.1 == *direction && p.2.is_none()).expect("a classic twin");
            let (mine, theirs) = (LOOKS[i], LOOKS[twin]);
            // A layout is its look with a field or two changed (the nav indicator).
            let same = crate::motion::Look { nav: theirs.nav, ..mine } == theirs;
            assert!(same, "{name} differs from its direction's look in more than the nav indicator");
        }
    }
}

#[test]
fn presses_shrink_by_weight() {
    assert!(Press::Icon.scale() < Press::Button.scale() && Press::Button.scale() < Press::Tile.scale());
}
