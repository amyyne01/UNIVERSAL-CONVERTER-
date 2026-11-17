//! The preset switch (`MOTION-SYSTEM.md` 4.3 to 4.10): `main.rs::view` keys the shown preset by
//! its place in `PRESETS`, so a switch replaces one keyed element with another and Rune dissolves
//! between them over a backdrop filled with the shown preset's ground. Switches are pressed on
//! the picker's tiles, real input, so they animate as in the window.
//!
//! A runtime has no pixels: "the window never shows Rune's clear colour" is checked by its proxy,
//! an opaque full-window backdrop under the preset on every frame. The pictures are the strip
//! sheet of step 47.
use rune::{ElementInfo, Runtime};

use super::{DT, name, runtime, steps};
use crate::actions;
use crate::gate::{Nav, PRESETS, State};
use crate::keys;

/// The shown preset's root, live or leaving.
fn shown(els: &[ElementInfo], p: usize) -> Option<(usize, &ElementInfo)> {
    let key = format!("preset-{p}");
    els.iter().enumerate().find(|(_, e)| e.key.as_deref() == Some(&key))
}

/// Whether element `i` sits under element `root`.
fn inside(els: &[ElementInfo], mut i: usize, root: usize) -> bool {
    while let Some(up) = els[i].parent {
        if up == root {
            return true;
        }
        i = up;
    }
    false
}

/// A press on the element keyed `key`, as the pointer makes it.
fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element with key `{key}`"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
}

/// `from` with the picker open (scene `picker`), dark, reduced motion or not, after two frames.
fn on_picker(from: usize, reduced: bool) -> Runtime<State> {
    let mut rt = runtime(from, Some("picker"), true);
    rt.set_system_reduced_motion(reduced);
    rt.frame(0.0);
    steps(&mut rt, 2);
    rt
}

/// What every frame of a switch must hold: the backdrop is opaque and fills the window; neither
/// preset is scaled; the two are never both fully opaque at once.
fn frame_holds(case: &str, f: usize, els: &[ElementInfo], from: usize, to: usize) {
    let (_, new) = shown(els, to).unwrap_or_else(|| panic!("{case}, frame {f}: the new preset is not shown"));
    let backdrop = &els[new.parent.unwrap_or_else(|| panic!("{case}: the preset has no parent"))];
    assert!(backdrop.fill.a >= 0.999, "{case}, frame {f}: the backdrop is not opaque ({:?})", backdrop.fill);
    assert_eq!(backdrop.rect, [0.0, 0.0, 1240.0, 820.0], "{case}, frame {f}: the backdrop does not fill the window");
    assert!((new.scale - 1.0).abs() < 1e-3, "{case}, frame {f}: the new preset is scaled ({})", new.scale);
    if let Some((_, old)) = shown(els, from) {
        assert!((old.scale - 1.0).abs() < 1e-3, "{case}, frame {f}: the old preset is scaled ({})", old.scale);
        assert!(old.exiting, "{case}, frame {f}: the old preset is still live");
        assert!(!(old.opacity >= 0.999 && new.opacity >= 0.999), "{case}, frame {f}: both presets fully opaque");
    }
}

/// A layout check only (`MOTION-SYSTEM.md` 4.10, owner decision 2): the leaving preset's texts
/// keep their boxes while it fades. A leaving element is out of layout, so this cannot see glyphs
/// re-shaped inside a box; the faces themselves are judged on step 47's strip sheet.
#[test]
fn the_leaving_preset_keeps_its_text() {
    for (from, to) in [(3, 5), (4, 0), (2, 1), (0, 4)] {
        let mut rt = on_picker(from, false);
        let texts = |els: &[ElementInfo]| -> std::collections::HashMap<String, f32> {
            let Some((root, _)) = shown(els, from) else { return Default::default() };
            (0..els.len())
                .filter(|i| inside(els, *i, root) && els[*i].text.is_some())
                .map(|i| (els[i].path.clone(), els[i].rect[2]))
                .collect()
        };
        let before = texts(&rt.elements());
        press(&mut rt, &keys::preset(to));
        for f in 0..4 {
            rt.frame(DT);
            let after = texts(&rt.elements());
            let changed = before.iter().filter(|(p, w)| after.get(*p).is_some_and(|a| (a - **w).abs() > 0.5)).count();
            let case = format!("{} -> {}, frame {f}", name(from), name(to));
            assert!(before.len() > 10 && after.len() == before.len(), "{case}: {} texts before, {} after", before.len(), after.len());
            assert_eq!(changed, 0, "{case}: {changed} of {} leaving texts changed width", before.len());
        }
    }
}

/// The structural half, as before step 47: a data commit swaps the keyed preset.
#[test]
fn every_ordered_pair_swaps_the_keyed_preset() {
    let n = PRESETS.len();
    let mut pairs = 0;
    for from in 0..n {
        for to in (0..n).filter(|t| *t != from) {
            let mut rt = runtime(from, None, true);
            rt.frame(0.0);
            let case = format!("{} -> {}", name(from), name(to));
            let at = |rt: &Runtime<State>, p| shown(&rt.elements(), p).map(|(_, e)| e.exiting);
            assert_eq!(at(&rt, from), Some(false), "{case}: the old preset is not shown before the switch");
            assert_eq!(at(&rt, to), None, "{case}: the new key exists before the switch");
            rt.update(move |s, tx| actions::set_preset(s, to, tx));
            rt.frame(0.0);
            assert_eq!(at(&rt, to), Some(false), "{case}: the new preset is not shown");
            assert!(at(&rt, from).is_none_or(|exiting| exiting), "{case}: the old preset is still live");
            steps(&mut rt, 60);
            assert_eq!(at(&rt, from), None, "{case}: the old preset never left");
            pairs += 1;
        }
    }
    assert_eq!(pairs, n * (n - 1));
}

#[test]
fn every_ordered_pair_dissolves_without_a_flash() {
    let n = PRESETS.len();
    let mut pairs = 0;
    for from in 0..n {
        for to in (0..n).filter(|t| *t != from) {
            let case = format!("{} -> {}", name(from), name(to));
            let mut rt = on_picker(from, false);
            let els = rt.elements();
            assert!(shown(&els, from).is_some_and(|(_, e)| !e.exiting), "{case}: the old preset is not shown before the switch");
            assert!(shown(&els, to).is_none(), "{case}: the new key exists before the switch");
            let tile = rt.presented(&keys::preset(to)).map(|r| r.0);
            // A rail held open by hover must not pop open in the new preset.
            rt.update(|s, _| s.nav_open = true);
            rt.frame(DT);
            press(&mut rt, &keys::preset(to));
            rt.frame(DT);

            // Frame 0: the new preset arrives whole, the old one is still on screen.
            let els = rt.elements();
            let (ni, new) = shown(&els, to).unwrap_or_else(|| panic!("{case}: the new preset is not shown"));
            assert!(!new.exiting && new.opacity < 0.999, "{case}: the new preset cut in (opacity {})", new.opacity);
            let drawn = (0..els.len()).filter(|i| inside(&els, *i, ni)).count();
            assert!(drawn > 20, "{case}: the new preset is blank on its first frame ({drawn} elements)");
            let (_, old) = shown(&els, from).unwrap_or_else(|| panic!("{case}: the old preset left at once"));
            assert!(old.opacity > 0.5, "{case}: the old preset is nearly gone on frame 0 ({})", old.opacity);

            let mut blurred = false;
            for f in 0..40 {
                let els = rt.elements();
                frame_holds(&case, f, &els, from, to);
                blurred |= shown(&els, to).is_some_and(|(_, e)| e.blur > 0.0);
                // The picker does not move or change face under the pointer.
                assert_eq!(rt.presented(&keys::preset(to)).map(|r| r.0), tile, "{case}, frame {f}: the picker's tile moved");
                rt.frame(DT);
            }
            assert!(blurred, "{case}: the dissolve has no blur");
            let els = rt.elements();
            assert!(shown(&els, from).is_none(), "{case}: the old preset never left");
            let (_, new) = shown(&els, to).unwrap_or_else(|| panic!("{case}: the new preset left"));
            assert!(new.opacity >= 0.999 && new.blur == 0.0, "{case}: the new preset did not settle ({}, {})", new.opacity, new.blur);
            assert!(!rt.state.nav_open, "{case}: the sidebar is open after the switch");
            pairs += 1;
        }
    }
    assert_eq!(pairs, n * (n - 1));
}

#[test]
fn a_second_switch_at_frame_6_retargets() {
    let n = PRESETS.len();
    let mut cases = 0;
    for from in 0..n {
        for to in (0..n).filter(|t| *t != from) {
            // Back to where it came from: the leaving preset revives from where it is.
            let case = format!("{} -> {} -> back", name(from), name(to));
            let mut rt = on_picker(from, false);
            press(&mut rt, &keys::preset(to));
            steps(&mut rt, 6);
            let before = shown(&rt.elements(), from).map(|(_, e)| e.opacity).unwrap_or_else(|| panic!("{case}: the old preset left early"));
            press(&mut rt, &keys::preset(from));
            rt.frame(DT);
            let els = rt.elements();
            let (_, back) = shown(&els, from).unwrap_or_else(|| panic!("{case}: the first preset is not shown"));
            assert!(!back.exiting, "{case}: the first preset did not come back");
            assert!(back.opacity > 0.2 && back.opacity >= before - 0.1, "{case}: it restarted ({before} -> {})", back.opacity);
            for f in 0..40 {
                frame_holds(&case, f, &rt.elements(), to, from);
                rt.frame(DT);
            }
            let els = rt.elements();
            assert!(shown(&els, to).is_none() && shown(&els, from).is_some_and(|(_, e)| e.opacity >= 0.999), "{case}: did not settle");

            // On to a third: two leave, one arrives, nothing queues.
            let third = (0..n).find(|t| *t != from && *t != to).unwrap_or(0);
            let case = format!("{} -> {} -> {}", name(from), name(to), name(third));
            let mut rt = on_picker(from, false);
            press(&mut rt, &keys::preset(to));
            steps(&mut rt, 6);
            press(&mut rt, &keys::preset(third));
            rt.frame(DT);
            let els = rt.elements();
            assert!(shown(&els, third).is_some_and(|(_, e)| !e.exiting), "{case}: the third preset is not shown");
            assert!(shown(&els, to).is_none_or(|(_, e)| e.exiting), "{case}: the second preset is still live");
            steps(&mut rt, 40);
            let els = rt.elements();
            let left = [from, to].iter().filter(|p| shown(&els, **p).is_some()).count();
            assert_eq!(left, 0, "{case}: a leaving preset stayed");
            assert_eq!(crate::gate::preset(&rt.state), third, "{case}");
            cases += 2;
        }
    }
    assert_eq!(cases, 2 * n * (n - 1));
}

#[test]
fn reduced_motion_is_a_plain_short_fade() {
    let n = PRESETS.len();
    let mut pairs = 0;
    for from in 0..n {
        for to in (0..n).filter(|t| *t != from) {
            let case = format!("reduced {} -> {}", name(from), name(to));
            let mut rt = on_picker(from, true);
            press(&mut rt, &keys::preset(to));
            rt.frame(DT);
            let mut gone_at = None;
            for f in 0..30 {
                let els = rt.elements();
                frame_holds(&case, f, &els, from, to);
                for p in [from, to] {
                    if let Some((_, e)) = shown(&els, p) {
                        assert_eq!(e.blur, 0.0, "{case}, frame {f}: blur under reduced motion");
                    }
                }
                if gone_at.is_none() && shown(&els, from).is_none() {
                    gone_at = Some(f);
                }
                rt.frame(DT);
            }
            // The 0.18 s fade, give or take the frame it settles on.
            let gone = gone_at.unwrap_or_else(|| panic!("{case}: the old preset never left"));
            assert!(gone <= 14, "{case}: the reduced fade took {gone} frames");
            pairs += 1;
        }
    }
    assert_eq!(pairs, n * (n - 1));
}

/// Focus on a control of the leaving preset is dropped, never handed to some control of the new
/// one (`MOTION-SYSTEM.md` 4.6). (No preset draws a `TextEdit` field yet, the one focus 4.6
/// carries; picker tiles take no focus.)
#[test]
fn focus_inside_the_leaving_preset_is_dropped() {
    let n = PRESETS.len();
    for from in 0..n {
        let to = (from + 1) % n;
        let case = format!("{} -> {}", name(from), name(to));
        let mut rt = runtime(from, None, true);
        rt.frame(0.0);
        // The preset's first focusable control.
        rt.update(move |_, tx| tx.focus(format!("preset-{from}"), true));
        steps(&mut rt, 2);
        let before = focused_path(&rt).unwrap_or_else(|| panic!("{case}: nothing in the preset took focus"));
        rt.update(move |s, tx| actions::set_preset(s, to, tx));
        steps(&mut rt, 30);
        let after = focused_path(&rt);
        assert!(after.is_none(), "{case}: focus on `{before}` was handed to `{after:?}`");
    }
}

fn focused_path(rt: &Runtime<State>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.focused && !e.exiting).map(|e| e.path)
}

/// Where the element keyed `key` is drawn, leaving presets aside.
fn live(rt: &Runtime<State>, key: &str) -> Option<[f32; 4]> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| e.rect)
}

/// The Settings control every preset that has it is scrolled to, landed on the frame the new
/// preset first draws (no glide).
#[test]
fn a_switch_from_settings_keeps_the_layout_control_in_view() {
    let n = PRESETS.len();
    let mut checked = 0;
    for from in 0..n {
        for to in (0..n).filter(|t| *t != from) {
            let case = format!("{} -> {} in Settings", name(from), name(to));
            let mut rt = runtime(from, Some("picker"), true);
            rt.update(|s, _| actions::go(s, Nav::Settings));
            rt.frame(0.0);
            steps(&mut rt, 2);
            press(&mut rt, &keys::preset(to));
            rt.frame(DT);
            let Some(first) = live(&rt, keys::LAYOUT_CHOOSE) else {
                // One bar and The collection draw the control without the shared key (presets' gap).
                assert!(matches!(to, 0 | 1), "{case}: no `{}` in the new Settings", keys::LAYOUT_CHOOSE);
                continue;
            };
            assert!(first[1] >= 0.0 && first[1] + first[3] <= 820.0, "{case}: the Layout control is off screen ({first:?})");
            steps(&mut rt, 30);
            let later = live(&rt, keys::LAYOUT_CHOOSE);
            assert_eq!(later, Some(first), "{case}: the Layout control glided after the first frame");
            checked += 1;
        }
    }
    assert!(checked >= 16, "only {checked} pairs had the Layout control");
}

/// The first selected row is kept in view on Downloads, in every preset that draws it there.
#[test]
fn a_switch_keeps_the_first_selected_row_in_view() {
    let n = PRESETS.len();
    let mut checked = 0;
    for from in 0..n {
        for to in (0..n).filter(|t| *t != from) {
            let case = format!("{} -> {} on Downloads", name(from), name(to));
            let mut rt = runtime(from, Some("picker"), true);
            let id = rt.state.tasks.last().map(|t| t.task_id.clone()).unwrap_or_else(|| panic!("{case}: no tasks"));
            let pick = id.clone();
            rt.update(move |s, _| {
                actions::go(s, Nav::Downloads);
                s.selected.insert(pick);
            });
            rt.frame(0.0);
            steps(&mut rt, 2);
            press(&mut rt, &keys::preset(to));
            rt.frame(DT);
            let Some(row) = live(&rt, &keys::row(&id)) else { continue };
            assert!(row[1] >= 0.0 && row[1] < 820.0, "{case}: the selected row is off screen ({row:?})");
            checked += 1;
        }
    }
    assert!(checked >= 10, "only {checked} pairs drew the selected row");
}

/// The shown preset's backdrop colour.
fn ground(els: &[ElementInfo], p: usize) -> Option<rune::Color> {
    shown(els, p).and_then(|(_, e)| e.parent).map(|i| els[i].fill)
}

/// The preset's keyed elements with their rects: what a recolour must leave alone.
fn layout_of(els: &[ElementInfo], p: usize) -> Vec<(String, [f32; 4])> {
    let Some((root, _)) = shown(els, p) else { return Vec::new() };
    let keyed = (0..els.len()).filter(|i| *i == root || inside(els, *i, root));
    let mut out: Vec<_> = keyed.filter_map(|i| els[i].key.clone().map(|k| (k, els[i].rect))).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The theme recolours in place (`MOTION-SYSTEM.md` 4.7): run from the command palette's
/// "Switch to light" row, a press, the colours spring over several frames; nothing is re-keyed
/// and nothing moves. One bar draws its own command panel without the shared row keys: left out.
#[test]
fn theme_springs_its_colours_and_moves_nothing() {
    let toggle = keys::command(actions::commands(&State::sample(0)).iter().position(|c| *c == actions::Command::ToggleTheme).unwrap_or(0));
    let mut checked = 0;
    for p in 1..PRESETS.len() {
        let mut rt = runtime(p, Some("palette"), true);
        rt.frame(0.0);
        steps(&mut rt, 2);
        let before = rt.elements();
        let start = ground(&before, p);
        press(&mut rt, &toggle);
        steps(&mut rt, 3);
        let early = ground(&rt.elements(), p);
        steps(&mut rt, 40);
        let after = rt.elements();
        let theme = rt.state.settings.get("theme").and_then(|v| v.as_str());
        assert_eq!(theme, Some("light"), "{}: the row did not switch to light", name(p));
        assert_eq!(layout_of(&before, p), layout_of(&after, p), "{}: a theme change re-keyed or moved something", name(p));
        // Shelf, Signal and The collection still read the system's appearance, not the setting.
        if ground(&after, p) != start {
            assert!(early != start && early != ground(&after, p), "{}: the ground cut ({start:?} -> {early:?})", name(p));
            checked += 1;
        }
    }
    assert!(checked >= 2, "only {checked} presets follow the theme setting");
}

/// A palette lands in place: nothing is re-keyed, nothing moves. (No swatch calls
/// `set_palette` from a press yet, so the spring itself is seen once Settings wires it.)
#[test]
fn palette_recolours_in_place() {
    for p in 0..PRESETS.len() {
        let mut rt = runtime(p, None, true);
        rt.frame(0.0);
        steps(&mut rt, 2);
        let before = rt.elements();
        rt.update(|s, tx| actions::set_palette(s, 3, tx));
        steps(&mut rt, 40);
        let after = rt.elements();
        assert_eq!(rt.state.palette, 3, "{}", name(p));
        assert_eq!(layout_of(&before, p), layout_of(&after, p), "{}: a palette change re-keyed or moved something", name(p));
        assert_ne!(ground(&after, p), ground(&before, p), "{}: the backdrop kept its colour", name(p));
    }
}

/// The backdrop wears the shown preset's own ground (its root's fill), in both appearances and
/// on the look's own palette and a shared one, so the dissolve dips only between the two grounds.
#[test]
fn the_backdrop_is_the_shown_presets_ground() {
    let mut checked = 0;
    for p in 0..PRESETS.len() {
        for dark in [true, false] {
            for palette in [0, 3] {
                let case = format!("{} dark {dark} palette {palette}", name(p));
                let mut rt = runtime(p, None, dark);
                rt.update(move |s, tx| actions::set_palette(s, palette, tx));
                rt.frame(0.0);
                steps(&mut rt, 40);
                let els = rt.elements();
                let (root, _) = shown(&els, p).unwrap_or_else(|| panic!("{case}: no preset"));
                let own = els.iter().find(|e| e.parent == Some(root)).map(|e| e.fill);
                assert_eq!(ground(&els, p), own, "{case}: the backdrop is not the preset's ground");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, PRESETS.len() * 4);
}
