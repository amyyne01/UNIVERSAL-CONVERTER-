//! The jury's bug list of 2026-11-21 (`docs/design-gate/audit-2026-11/05-jury.md`, findings
//! L-1, L-2, L-4, ST-1 to ST-3, ST-5, C-1, C-2, K-1, D-1, F-1), each held in all six presets.
//! Every check loops over `PRESETS`, so a preset that is missing or drawn differently fails it.
use rune::{App, Color, ElementInfo, Env, Runtime};

use crate::gate::{PRESETS, SCENES, State};
use crate::{copy, keys, motion, timeline};

/// The real app on `preset`, a scene applied and `edit` after it, `palette`, settled.
fn runtime(preset: usize, scene: &str, dark: bool, palette: usize, edit: fn(&mut State)) -> Runtime<State> {
    let apply = SCENES.iter().find(|s| s.0 == scene).unwrap_or_else(|| panic!("no scene `{scene}`")).1;
    let mut rt = App::new(crate::spec())
        .init(move || {
            let mut s = State::sample(preset);
            apply(&mut s);
            edit(&mut s);
            s.palette = palette;
            s.sync();
            s
        })
        .headless_runtime((1240.0, 820.0), Env { system_dark: dark, scale: 1.0 });
    for _ in 0..40 {
        rt.frame(1.0 / 60.0);
    }
    rt
}

fn plain(preset: usize, scene: &str, dark: bool) -> Runtime<State> {
    runtime(preset, scene, dark, 0, |_| {})
}

fn name(preset: usize) -> &'static str {
    PRESETS[preset].0
}

fn keyed<'a>(els: &'a [ElementInfo], key: &str) -> Option<&'a ElementInfo> {
    els.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn inside(inner: [f32; 4], outer: [f32; 4]) -> bool {
    inner[0] >= outer[0] - 0.5
        && inner[1] >= outer[1] - 0.5
        && inner[0] + inner[2] <= outer[0] + outer[2] + 0.5
        && inner[1] + inner[3] <= outer[1] + outer[3] + 0.5
}

fn meets(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] - 0.5 && b[0] < a[0] + a[2] - 0.5 && a[1] < b[1] + b[3] - 0.5 && b[1] < a[1] + a[3] - 0.5
}

/// Whether element `i` is element `of` or inside it.
fn within(els: &[ElementInfo], i: usize, of: usize) -> bool {
    std::iter::successors(Some(i), |&j| els[j].parent).any(|j| j == of)
}

/// Whether elements `a` and `b` share a parent at most four levels above each: one row's parts.
fn near(els: &[ElementInfo], a: usize, b: usize) -> bool {
    let up = |i: usize| std::iter::successors(Some(i), |&j| els[j].parent).take(5).collect::<Vec<_>>();
    let (ua, ub) = (up(a), up(b));
    ua.iter().any(|x| ub.contains(x))
}

/// The nearest opaque fill behind element `i` (its ancestors'), or the theme's ground.
fn ground(els: &[ElementInfo], i: usize, dark: bool) -> Color {
    std::iter::successors(els[i].parent, |&j| els[j].parent).map(|j| els[j].fill).find(|c| c.a > 0.99).unwrap_or(if dark {
        Color::BLACK
    } else {
        Color::WHITE
    })
}

/// WCAG contrast of `fg` composited over the opaque `bg`, on `bg`.
fn contrast(fg: Color, bg: Color) -> f32 {
    let fg = over(fg, bg);
    let (a, b) = (fg.luminance() + 0.05, bg.luminance() + 0.05);
    a.max(b) / a.min(b)
}

fn over(fg: Color, bg: Color) -> Color {
    fg.mix(bg.alpha(fg.a), 1.0 - fg.a).alpha(1.0)
}

// ── L-4: one soft band per card ─────────────────────────────────────────────

/// Every bone of a loading card is lit by the same band: each band is `SHIMMER_BAND` wide and
/// stands at the same x in the window, whatever its bone's place or width.
#[test]
fn every_loading_card_lights_all_its_bones_with_one_band() {
    // A bone's place is worked out from the layout's own sizes; a few points of drift in a
    // 240-point soft band is not visible.
    const DRIFT: f32 = 4.0;
    assert!(!PRESETS.is_empty());
    for p in 0..PRESETS.len() {
        for scene in ["looking-up", "search-loading", "playlist"] {
            for dark in [true, false] {
                let case = format!("{} / {scene} / dark {dark}", name(p));
                let mut rt = plain(p, scene, dark);
                rt.frame(0.3);
                let els = rt.elements();
                let bands: Vec<&ElementInfo> = els.iter().filter(|e| e.key.as_deref() == Some(timeline::SHEEN)).collect();
                assert!(!bands.is_empty(), "{case}: no shimmer");
                for b in &bands {
                    assert!((b.rect[2] - motion::SHIMMER_BAND).abs() < 0.5, "{case}: {} is {} wide", b.path, b.rect[2]);
                }
                let xs: Vec<f32> = bands.iter().map(|b| b.rect[0] + b.scrolled[0]).collect();
                let (lo, hi) = xs.iter().fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(*x), hi.max(*x)));
                assert!(hi - lo <= DRIFT, "{case}: the bands stand {lo}..{hi}, not as one: {xs:?}");
            }
        }
    }
}

#[test]
fn every_looks_band_is_soft_clear_at_both_ends_and_peaks_in_the_middle() {
    let peak = Color::WHITE.alpha(0.2);
    for spread in [0.0, 0.25, 0.28, 0.36, 0.5, 1.0] {
        let stops = timeline::sheen_stops(peak, spread);
        assert_eq!((stops[0].1.a, stops[stops.len() - 1].1.a), (0.0, 0.0), "spread {spread}: clear at the ends");
        assert!(stops.iter().any(|(at, c)| *at == 0.5 && c.a == peak.a), "spread {spread}: the peak at the middle");
        for w in stops.windows(2) {
            let (d, rise) = (w[1].0 - w[0].0, (w[1].1.a - w[0].1.a).abs());
            assert!(d >= 0.0, "spread {spread}: stops in order");
            // A hard line is a big change over a short run: no step is steeper than the whole
            // peak over a tenth of the band (24 points).
            assert!(rise <= peak.a * d / 0.1 + 1e-6, "spread {spread}: {w:?} is a hard edge");
        }
    }
}

// ── L-1, L-2: the loading card is the final card, still blank ───────────────

#[test]
fn the_loading_card_stands_where_the_final_card_will_and_its_ring_has_a_home() {
    for p in 0..PRESETS.len() {
        for dark in [true, false] {
            let case = format!("{} dark {dark}", name(p));
            let wait = plain(p, "looking-up", dark).elements();
            let done = plain(p, "media", dark).elements();
            let a = keyed(&wait, keys::FOUND_COVER).unwrap_or_else(|| panic!("{case}: no cover bone")).rect;
            let b = keyed(&done, keys::FOUND_COVER).unwrap_or_else(|| panic!("{case}: no cover on the media card")).rect;
            assert!(
                (a[0] - b[0]).abs() < 1.0 && (a[2] - b[2]).abs() < 1.0 && (a[3] - b[3]).abs() < 1.0,
                "{case}: cover {a:?} becomes {b:?}"
            );
            // Down from the card's top edge (Now playing's sheet stands on the bar, so its top moves).
            let top = |els: &[ElementInfo]| keyed(els, keys::FOUND).map_or(0.0, |e| e.rect[1]);
            let (ya, yb) = (a[1] - top(&wait), b[1] - top(&done));
            assert!((ya - yb).abs() <= 16.0, "{case}: the cover stands {ya} down the card, then {yb}");
            // The waiting ring sits in the cover's box, or just before "Reading the link…".
            let ring = keyed(&wait, keys::RING_WAITING).unwrap_or_else(|| panic!("{case}: no waiting ring")).rect;
            let note = wait.iter().find(|e| e.text.as_deref() == Some(copy::found::NOTE_READING)).map(|e| e.rect);
            let before = note.is_some_and(|t| {
                let gap = t[0] - (ring[0] + ring[2]);
                (0.0..=16.0).contains(&gap) && ((ring[1] + ring[3] / 2.0) - (t[1] + t[3] / 2.0)).abs() <= 4.0
            });
            assert!(inside(ring, a) || before, "{case}: the ring {ring:?} floats (cover {a:?}, note {note:?})");
        }
    }
}

// ── ST-3: hotkeys as they read on a keyboard ────────────────────────────────

#[test]
fn no_preset_shows_a_raw_accelerator() {
    let hotkey = |s: &mut State| {
        s.settings.insert("globalHotkey".into(), "CommandOrControl+Shift+D".into());
    };
    for p in 0..PRESETS.len() {
        let mut shown = false;
        for scene in ["home-history", "idle", "shortcuts", "reach", "settings-basic"] {
            let els = runtime(p, scene, true, 0, hotkey).elements();
            for e in &els {
                let t = e.text.as_deref().unwrap_or_default();
                assert!(!t.contains("CommandOrControl"), "{} / {scene}: {} shows {t:?}", name(p), e.path);
                shown |= t.contains("Ctrl+Shift+D") || (t == copy::KEY_CTRL && els.iter().any(|o| o.text.as_deref() == Some("D")));
            }
        }
        assert!(shown, "{}: the hotkey is shown nowhere", name(p));
    }
    assert_eq!(crate::format::hotkey("CommandOrControl+Shift+D"), "Ctrl+Shift+D");
    assert_eq!(crate::format::hotkey_keys("Alt+F2"), ["Alt", "F2"]);
    assert_eq!(crate::format::hotkey(""), "");
}

// ── K-1: one Escape ────────────────────────────────────────────────────────

#[test]
fn escape_is_spelled_one_way_everywhere() {
    for p in 0..PRESETS.len() {
        for scene in ["media", "nav", "palette", "shortcuts", "reach", "looking-up"] {
            for e in plain(p, scene, true).elements() {
                if let Some(t) = e.text.as_deref().filter(|t| t.eq_ignore_ascii_case("esc")) {
                    assert_eq!(t, copy::KEYCAP_ESC, "{} / {scene}: {}", name(p), e.path);
                }
            }
        }
    }
}

// ── C-1, C-2: switches that read off and on ─────────────────────────────────

/// Every settings switch in dark, every palette: off, its thumb stands out from its track at
/// 3:1 (WCAG 1.4.11); on, Orbit's and Now playing's track wears the accent like the other four.
#[test]
fn switches_read_off_at_three_to_one_and_wear_the_accent_on() {
    for p in 0..PRESETS.len() {
        let orbit = ["Orbit", "Now playing"].contains(&name(p));
        for palette in 0..=7 {
            let (mut off, mut on) = (0, 0);
            for scene in ["settings-basic", "reach"] {
                let rt = runtime(p, scene, true, palette, |_| {});
                let els = rt.elements();
                for (i, track) in els.iter().enumerate().filter(|(_, e)| e.role == "switch") {
                    let Some(thumb) = els.iter().find(|e| e.parent == Some(i) && e.rect[2] <= 16.0 && e.rect[3] <= 16.0) else { continue };
                    let case = format!("{} palette {palette} {scene} {}", name(p), track.path);
                    let bg = ground(&els, i, true);
                    let lit = thumb.rect[0] + thumb.rect[2] / 2.0 > track.rect[0] + track.rect[2] / 2.0;
                    if lit {
                        on += 1;
                        if orbit {
                            let accent = crate::directions::orbit::look::Pal::new(palette, true).accent;
                            assert_eq!(track.fill, accent, "{case}: on, the track is not the accent");
                        }
                    } else {
                        off += 1;
                        let c = contrast(thumb.fill, over(track.fill, bg));
                        assert!(c >= 3.0, "{case}: off thumb on its track at {c:.2}:1");
                    }
                }
            }
            assert!(off > 0 && on > 0, "{} palette {palette}: {off} off and {on} on switches", name(p));
        }
    }
}

// ── ST-1, ST-2, ST-5: settings that fit ─────────────────────────────────────

#[test]
fn settings_words_never_run_under_a_button_and_the_three_defaults_share_a_line() {
    let trio = ["defaultFormat", "defaultQuality", "defaultVideoQuality"].map(keys::setting);
    for p in 0..PRESETS.len() {
        let mut lined = false;
        for scene in ["reach", "settings-basic"] {
            let els = plain(p, scene, true).elements();
            for (b, button) in els.iter().enumerate().filter(|(_, e)| e.role == "button" && e.visible[2] > 0.0) {
                for (t, words) in els.iter().enumerate().filter(|(_, e)| e.text.is_some() && e.role == "text" && e.visible[2] > 0.0) {
                    // Words and a button of one settings row (a common parent a few levels up);
                    // a bar the page scrolls under is not a collision.
                    if !within(&els, t, b) && near(&els, t, b) && meets(words.visible, button.visible) {
                        panic!("{} / {scene}: {:?} runs under the button {}", name(p), words.text, button.path);
                    }
                }
            }
            let ys: Vec<f32> = trio.iter().filter_map(|k| keyed(&els, k)).map(|e| e.rect[1] + e.scrolled[1]).collect();
            if ys.len() == 3 {
                lined = true;
                assert!(ys.iter().all(|y| (y - ys[0]).abs() < 1.0), "{} / {scene}: the defaults stand at {ys:?}", name(p));
            }
        }
        assert!(lined, "{}: the three download defaults are never shown together", name(p));
    }
}

/// On Basic a select with locked choices carries one lock, right after its name, never in it.
#[test]
fn a_select_with_locked_choices_carries_its_lock_after_its_name() {
    for p in 0..PRESETS.len() {
        let mut seen = 0;
        for scene in ["settings-basic", "reach"] {
            let rt = runtime(p, scene, true, 0, |s| s.plan = ahg_engine::tier::Plan::Basic);
            let els = rt.elements();
            for sec in crate::settings_spec::sections(&rt.state) {
                for r in sec.rows.iter().filter(|r| r.lock.is_none() && r.gated() && r.key.is_some()) {
                    let k = r.key.unwrap_or_default();
                    let Some(select) = keyed(&els, &keys::setting(k)) else { continue };
                    let lock =
                        keyed(&els, &keys::setting_lock(k)).unwrap_or_else(|| panic!("{} / {scene}: {k} has no lock by its name", name(p)));
                    assert!(!meets(lock.rect, select.rect), "{} / {scene}: {k}'s lock sits in its select", name(p));
                    seen += 1;
                }
            }
        }
        assert!(seen > 0, "{}: no select with locked choices was drawn on Basic", name(p));
    }
}

// ── D-1: a long link fades before the dock's button ─────────────────────────

#[test]
fn a_long_link_in_a_field_never_runs_under_its_button() {
    let long = |s: &mut State| s.set_field("https://www.youtube.com/watch?v=example-with-a-very-long-identifier-and-more-and-more");
    for p in 0..PRESETS.len() {
        let els = runtime(p, "media", true, 0, long).elements();
        let Some(field) = keyed(&els, keys::FIELD) else { continue };
        if let Some(go) = keyed(&els, keys::LOOK_UP) {
            assert!(!meets(field.visible, go.visible), "{}: the field's text runs under Look up", name(p));
        }
    }
    let stage = PRESETS.iter().position(|p| p.0 == "Now playing").expect("Now playing");
    let els = runtime(stage, "media", true, 0, long).elements();
    let (Some(fade), Some(field)) = (keyed(&els, keys::FIELD_FADE), keyed(&els, keys::FIELD)) else {
        panic!("Now playing: no fading field")
    };
    assert!(inside(field.visible, fade.rect), "Now playing: the field is outside its fade");
}

// ── F-1: nothing under a found card answers ─────────────────────────────────

#[test]
fn nothing_under_the_found_card_can_be_pressed_or_focused() {
    for p in 0..PRESETS.len() {
        let els = plain(p, "media", true).elements();
        let (card, found) = els
            .iter()
            .enumerate()
            .find(|(_, e)| e.key.as_deref() == Some(keys::FOUND))
            .unwrap_or_else(|| panic!("{}: no found card", name(p)));
        for (i, e) in els.iter().enumerate() {
            let covered = e.focusable && !e.inert && e.visible[2] > 0.0 && meets(e.visible, found.visible);
            if covered && !within(&els, i, card) && !within(&els, card, i) {
                panic!("{}: {} is live under the found card", name(p), e.path);
            }
        }
    }
}
