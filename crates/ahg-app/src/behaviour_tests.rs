//! Behaviour tests driven through Rune's scripted runtime: one set-up, one loop over `PRESETS`.
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};

use crate::gate::{Filter, Nav, Overlay, PRESETS, State};
use crate::{actions, keys};

/// The one headless set-up: the real spec, one preset, dark or light.
fn runtime(preset: usize, dark: bool) -> Runtime<State> {
    let mut rt = App::new(crate::spec()).headless_runtime((1240.0, 820.0), Env { system_dark: dark, scale: 1.0 });
    rt.frame(0.0);
    rt.update(move |s, tx| actions::set_preset(s, preset, tx));
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
}

/// Press the control with this key. A missing key is a panic naming the preset and the key.
fn click(rt: &mut Runtime<State>, preset: &str, key: &str) {
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
#[ignore = "red until steps 54/55: presets do not use keys.rs yet"]
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
