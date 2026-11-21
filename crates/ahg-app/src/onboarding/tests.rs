//! The first-launch setup in every preset, pressed through its drawn controls (sample mode; the
//! live first launch, the files and a restart are the functional scorecard's startup group).
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::json;

use crate::behaviour_tests::{click, runtime, settle};
use crate::gate::{Nav, PRESETS, State, preset};
use crate::keys;

fn press(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    settle(rt);
}

fn ctrl(c: char) -> (Key, Mods) {
    (Key::Command(c), Mods { cmd: true, ctrl: true, ..Mods::default() })
}

/// Every preset failing is reported together.
fn every_preset(walk: impl Fn(&mut Runtime<State>, usize, &str)) {
    assert!(!PRESETS.is_empty());
    let failed: Vec<String> = PRESETS
        .iter()
        .enumerate()
        .flat_map(|(i, (name, ..))| [true, false].map(|dark| (i, *name, dark)))
        .filter_map(|(i, name, dark)| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| walk(&mut runtime(i, dark), i, name)))
                .err()
                .map(|e| e.downcast_ref::<String>().cloned().unwrap_or_else(|| format!("{name} dark {dark}: panicked")))
        })
        .collect();
    assert!(failed.is_empty(), "{} of {} cells fail:\n{}", failed.len(), PRESETS.len() * 2, failed.join("\n"));
}

/// Settings' Run setup again opens it in each preset; each step's controls apply live; Finish lands on Home.
#[test]
fn run_setup_again_then_every_step_applies_live_in_every_preset() {
    every_preset(|rt, i, name| {
        click(rt, name, keys::nav(Nav::Settings));
        click(rt, name, keys::RUN_SETUP);
        assert!(rt.state.onboarding.is_some(), "{name}: `{}` opened nothing", keys::RUN_SETUP);
        assert!(rt.find(keys::ONBOARDING).is_some(), "{name}: the setup is not drawn");
        // Layout: another preset's card switches the app behind the setup.
        let other = (i + 1) % PRESETS.len();
        click(rt, name, &keys::preset(other));
        assert_eq!(preset(&rt.state), other, "{name}: the layout card");
        assert!(rt.state.onboarding.is_some(), "{name}: a card closed the setup");
        click(rt, name, keys::ONBOARD_NEXT);
        // Look.
        click(rt, name, &keys::onboard_theme(0));
        assert_eq!(rt.state.settings["theme"], json!("light"), "{name}: Light");
        click(rt, name, &keys::onboard_palette(6));
        assert_eq!(rt.state.palette, 6, "{name}: the sixth palette");
        click(rt, name, keys::ONBOARD_NEXT);
        // Essentials: Settings' own rows.
        click(rt, name, &keys::onboard_setting("defaultFormat", Some("ogg")));
        assert_eq!(rt.state.settings["defaultFormat"], json!("ogg"), "{name}: the format chip");
        let before = rt.state.settings["clipboardWatch"].clone();
        click(rt, name, &keys::onboard_setting("clipboardWatch", None));
        assert_ne!(rt.state.settings["clipboardWatch"], before, "{name}: the clipboard toggle");
        assert!(rt.find(keys::ONBOARD_FOLDER).is_some(), "{name}: no folder button");
        click(rt, name, keys::ONBOARD_NEXT);
        assert!(rt.state.onboarding.is_none(), "{name}: Finish left it open");
        assert_eq!(rt.state.nav, Nav::Home, "{name}: Finish lands on Home");
        assert!(rt.elements().iter().all(|e| e.exiting || e.key.as_deref() != Some(keys::ONBOARDING)), "{name}: still drawn");
    });
}

/// Enter is Next and Finish (the default button), Esc is Back, Esc on the first step asks and a
/// second Esc skips, and Ctrl+1..3 and Ctrl+K do nothing while it is up.
#[test]
fn the_keys_while_the_setup_is_up() {
    every_preset(|rt, _, name| {
        rt.update(|s, _| crate::onboarding::open(s));
        settle(rt);
        let nav = rt.state.nav;
        for c in ['2', '3', 'k', '.'] {
            let (k, m) = ctrl(c);
            press(rt, k, m);
        }
        assert_eq!(rt.state.nav, nav, "{name}: Ctrl+digit moved under the setup");
        assert_eq!(rt.state.overlay, crate::gate::Overlay::None, "{name}: a chord opened an overlay");
        press(rt, Key::Enter, Mods::default());
        assert_eq!(rt.state.onboarding.as_ref().map(|o| o.step), Some(1), "{name}: Enter is Next");
        // Esc after a click (focus on a control) and with nothing focused: both go back.
        click(rt, name, &keys::onboard_theme(1));
        press(rt, Key::Escape, Mods::default());
        assert_eq!(rt.state.onboarding.as_ref().map(|o| o.step), Some(0), "{name}: Esc is Back");
        press(rt, Key::Escape, Mods::default());
        assert!(rt.state.onboarding.as_ref().is_some_and(|o| o.confirm_skip), "{name}: Esc on the first step asks");
        assert!(rt.find(keys::ONBOARD_SKIP_YES).is_some(), "{name}: the skip question is drawn");
        press(rt, Key::Escape, Mods::default());
        assert!(rt.state.onboarding.is_none(), "{name}: the second Esc skips");
    });
}

/// The lazy persona: Home in three presses of Enter, or one click on Skip setup.
#[test]
fn a_lazy_user_reaches_home_in_three_presses() {
    every_preset(|rt, _, name| {
        rt.update(|s, _| crate::onboarding::open(s));
        settle(rt);
        let before = rt.state.settings.clone();
        for _ in 0..3 {
            press(rt, Key::Enter, Mods::default());
        }
        assert!(rt.state.onboarding.is_none() && rt.state.nav == Nav::Home, "{name}: not Home after three Enters");
        assert_eq!(rt.state.settings, before, "{name}: Next changed a setting");
        rt.update(|s, _| crate::onboarding::open(s));
        settle(rt);
        click(rt, name, keys::ONBOARD_SKIP);
        assert!(rt.state.onboarding.is_none(), "{name}: Skip setup is one press");
    });
}

/// On Basic the Premium choices carry their lock: a press writes nothing and says why.
#[test]
fn basic_locks_the_premium_choices() {
    let mut rt = runtime(0, true);
    rt.update(|s, _| {
        s.plan = ahg_engine::tier::Plan::Basic;
        crate::onboarding::open(s);
        crate::onboarding::next(s);
        crate::onboarding::next(s);
    });
    settle(&mut rt);
    let before = rt.state.settings["defaultFormat"].clone();
    click(&mut rt, "One bar", &keys::onboard_setting("defaultFormat", Some("flac")));
    assert_eq!(rt.state.settings["defaultFormat"], before, "a locked format was written");
    assert!(!rt.state.notices.is_empty(), "no notice says why");
}
