//! Behaviour tests driven through Rune's scripted runtime: one set-up, one loop over `PRESETS`.
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};

use crate::gate::{Filter, Nav, Overlay, PRESETS, State};
use crate::{actions, keys};

/// The one headless set-up: the real spec, one preset, dark or light.
pub(crate) fn runtime(preset: usize, dark: bool) -> Runtime<State> {
    let mut rt = App::new(crate::spec()).headless_runtime((1240.0, 820.0), Env { system_dark: dark, scale: 1.0 });
    rt.frame(0.0);
    rt.update(move |s, tx| actions::set_preset(s, preset, tx));
    settle(&mut rt);
    rt
}

pub(crate) fn settle(rt: &mut Runtime<State>) {
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
}

/// Press the control with this key. A missing key is a panic naming the preset and the key.
pub(crate) fn click(rt: &mut Runtime<State>, preset: &str, key: &str) {
    let k = key.to_owned();
    rt.update(move |_, tx| tx.scroll_into_view(k));
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("preset {preset}: no element with key `{key}`"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn ctrl_k(rt: &mut Runtime<State>) {
    rt.key(KeyEvent { key: Key::Command('k'), mods: Mods { cmd: true, ctrl: true, ..Mods::default() } });
    settle(rt);
}

/// The whole walk for one preset.
fn walk(rt: &mut Runtime<State>, name: &str) {
    for nav in [Nav::Downloads, Nav::Settings, Nav::Home] {
        click(rt, name, keys::nav(nav));
        assert_eq!(rt.state.nav, nav, "preset {name}: {}", keys::nav(nav));
    }
    assert!(rt.find(keys::FIELD).is_some(), "preset {name}: no field on Home (`{}`)", keys::FIELD);

    ctrl_k(rt);
    assert_eq!(rt.state.overlay, Overlay::Palette, "preset {name}: Ctrl+K opens the palette");
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    settle(rt);
    assert_eq!(rt.state.overlay, Overlay::None, "preset {name}: Escape closes the palette");

    click(rt, name, keys::nav(Nav::Downloads));
    for f in Filter::ALL {
        click(rt, name, &keys::filter(f));
        assert_eq!(rt.state.filter, f, "preset {name}: chip {}", keys::filter(f));
    }

    click(rt, name, keys::nav(Nav::Settings));
    let before = rt.state.settings.get("showNotifications").cloned();
    click(rt, name, &keys::setting("showNotifications"));
    assert_ne!(rt.state.settings.get("showNotifications"), before.as_ref(), "preset {name}: the toggle changes State.settings");

    click(rt, name, keys::LAYOUT_CHOOSE);
    assert!(rt.state.picker, "preset {name}: the Layout button opens the picker");
}

#[test]
fn set_up_works_for_every_preset() {
    assert!(!PRESETS.is_empty());
    for (i, (name, ..)) in PRESETS.iter().enumerate() {
        for dark in [true, false] {
            let rt = runtime(i, dark);
            assert!(!rt.elements().is_empty(), "preset {name}: the first frame has no elements");
            assert_eq!(crate::gate::preset(&rt.state), i, "preset {name}");
        }
    }
}

#[test]
fn walk_every_preset() {
    assert!(!PRESETS.is_empty());
    // Every preset is walked, then all the failures are reported together.
    let failed: Vec<String> = PRESETS
        .iter()
        .enumerate()
        .filter_map(|(i, (name, ..))| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| walk(&mut runtime(i, true), name)))
                .err()
                .map(|e| e.downcast_ref::<String>().cloned().unwrap_or_else(|| format!("preset {name}: panicked")))
        })
        .collect();
    assert!(
        failed.is_empty(),
        "{} of {} presets fail:
{}",
        failed.len(),
        PRESETS.len(),
        failed.join(
            "
"
        )
    );
}

#[test]
#[should_panic(expected = "no element with key `no-such-key`")]
fn walk_fails_on_a_missing_key() {
    assert!(!PRESETS.is_empty());
    click(&mut runtime(0, true), PRESETS[0].0, "no-such-key");
}

// ── Notices after the found step's Download

/// The scene's set-up, by name.
fn scene(name: &str) -> fn(&mut State) {
    crate::gate::SCENES.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("no scene {name}")).1
}

/// `text` drawn once, and the only notice button the shared toast's, labelled `label`.
fn one_notice(rt: &Runtime<State>, at: &str, text: &str, label: &str) {
    let shown: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting).collect();
    let texts = shown.iter().filter(|e| e.text.as_deref() == Some(text)).count();
    assert_eq!(texts, 1, "{at}: `{text}` is drawn {texts} times");
    let buttons: Vec<Option<String>> =
        shown.iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with(keys::NOTICE_ACTION))).map(|e| e.label.clone()).collect();
    assert_eq!(buttons, vec![Some(label.to_owned())], "{at}: the notice's buttons");
}

/// Runs `check` on every preset, dark and light, each palette, and reports every failure together.
fn every_look(check: impl Fn(&mut Runtime<State>, &str)) {
    assert!(!PRESETS.is_empty());
    let palettes = ahg_engine::config::PALETTE_IDS.len();
    let mut failed = Vec::new();
    for (i, (name, ..)) in PRESETS.iter().enumerate() {
        for dark in [true, false] {
            let mut rt = runtime(i, dark);
            for palette in 0..=palettes {
                let at = format!("preset {name}, {}, palette {palette}", if dark { "dark" } else { "light" });
                rt.update(move |s, tx| actions::set_palette(s, palette, tx));
                settle(&mut rt);
                if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&mut rt, &at))) {
                    failed.push(e.downcast_ref::<String>().cloned().unwrap_or_else(|| format!("{at}: panicked")));
                }
                rt.update(|s, _| actions::dismiss_found(s));
                settle(&mut rt);
            }
        }
    }
    assert!(failed.is_empty(), "{} failures:\n{}", failed.len(), failed.join("\n"));
}

#[test]
fn a_download_says_added_once_in_the_shared_toast_and_closes_the_step_in_every_preset() {
    every_look(|rt, at| {
        rt.update(|s, _| {
            scene("media")(s);
            s.sync();
        });
        settle(rt);
        click(rt, at, keys::FOUND_DOWNLOAD);
        one_notice(rt, at, crate::copy::found::TOAST_ADDED, crate::gate::Act::ViewDownloads.label());
        assert!(matches!(rt.state.found, crate::gate::Found::Nothing), "{at}: the found step closes");
        assert!(rt.find(keys::FOUND).is_none(), "{at}: the found card is gone");
        assert_eq!(rt.state.field.text, "", "{at}: the field is empty");
    });
}

#[test]
fn a_basic_limit_keeps_its_tone_and_see_plans_once_in_every_preset() {
    every_look(|rt, at| {
        // Every result ticked on Basic, then Download.
        rt.update(|s, _| {
            scene("search-limit")(s);
            actions::dismiss_notice(s);
            actions::pick_all_results(s);
        });
        settle(rt);
        assert!(rt.state.picked.len() > 5, "{at}: more than five ticked");
        click(rt, at, keys::FOUND_DOWNLOAD);
        let last = rt.state.notices.last().cloned();
        assert_eq!(
            last.map(|n| (n.tone, n.action)),
            Some((crate::gate::Tone::Premium, Some(crate::gate::Act::SeePlans))),
            "{at}: the limit is a premium notice"
        );
        one_notice(rt, at, crate::copy::found::LIMIT_BATCH, crate::gate::Act::SeePlans.label());
        assert!(matches!(rt.state.found, crate::gate::Found::Search { .. }), "{at}: a refused add keeps the step");
    });
}
