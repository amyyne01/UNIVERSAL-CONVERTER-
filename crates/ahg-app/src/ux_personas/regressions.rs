//! The bugs the personas found, pinned in every preset (sample mode, no stand-in needed).
use rune::{Key, KeyEvent, Mods};

use crate::behaviour_tests::{click, runtime, settle};
use crate::gate::{Nav, Overlay, PRESETS};
use crate::keys;

/// Runs `check` on every preset and reports every failure together.
fn every_preset(check: impl Fn(usize, &str)) {
    assert!(!PRESETS.is_empty());
    let failed: Vec<String> = PRESETS
        .iter()
        .enumerate()
        .filter_map(|(i, (name, ..))| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(i, name)))
                .err()
                .map(|e| e.downcast_ref::<String>().cloned().unwrap_or_else(|| format!("preset {name}: panicked")))
        })
        .collect();
    assert!(failed.is_empty(), "{} of {} presets fail:\n{}", failed.len(), PRESETS.len(), failed.join("\n"));
}

/// Pastes `text` into Home's field, as a user does.
fn paste_into_field(rt: &mut rune::Runtime<crate::gate::State>, name: &str, text: &str) {
    click(rt, name, keys::FIELD);
    rt.paste(text.to_owned());
    settle(rt);
}

#[test]
fn several_links_pasted_on_several_lines_are_a_batch_in_every_preset() {
    every_preset(|i, name| {
        let mut rt = runtime(i, true);
        let rows = rt.state.tasks.len();
        paste_into_field(&mut rt, name, "https://www.youtube.com/watch?v=aaaaaaaaaaa\r\nhttps://soundcloud.com/someone/a-track\n");
        assert_eq!(rt.state.tasks.len(), rows + 2, "preset {name}: two links on two lines queue two downloads");
        assert_eq!(rt.state.nav, Nav::Downloads, "preset {name}: the batch shows Downloads");
        assert_eq!(rt.state.field.text, "", "preset {name}: the links are not run together in the field");
    });
}

#[test]
fn one_line_pasted_stays_in_the_field_in_every_preset() {
    every_preset(|i, name| {
        let mut rt = runtime(i, true);
        let rows = rt.state.tasks.len();
        paste_into_field(&mut rt, name, "daft punk\nget lucky");
        assert_eq!(rt.state.tasks.len(), rows, "preset {name}: words on two lines are no batch");
        assert!(rt.state.field.text.contains("daft punk"), "preset {name}: the words are in the field");
    });
}

#[test]
fn the_same_link_twice_in_one_paste_is_queued_once_in_every_preset() {
    every_preset(|i, name| {
        let mut rt = runtime(i, true);
        let rows = rt.state.tasks.len();
        let link = "https://www.youtube.com/watch?v=bbbbbbbbbbb";
        paste_into_field(&mut rt, name, &format!("{link}\n{link}\nhttps://vimeo.com/123456"));
        assert_eq!(rt.state.tasks.len(), rows + 2, "preset {name}: two distinct links, two downloads");
        // Typed on one line with Enter, the same.
        let rows = rt.state.tasks.len();
        rt.update(|s, tx| {
            crate::actions::go(s, Nav::Home);
            s.set_field(&format!("{link} {link}"));
            crate::actions::submit(s, tx);
        });
        settle(&mut rt);
        assert_eq!(rt.state.tasks.len(), rows + 1, "preset {name}: one link twice on one line, one download");
    });
}

#[test]
fn typing_in_the_command_palette_and_enter_runs_the_match_in_every_preset() {
    every_preset(|i, name| {
        let mut rt = runtime(i, true);
        rt.key(KeyEvent { key: Key::Command('k'), mods: Mods { cmd: true, ctrl: true, ..Mods::default() } });
        settle(&mut rt);
        assert_eq!(rt.state.overlay, Overlay::Palette, "preset {name}: Ctrl+K opens the palette");
        for c in "downloads".chars() {
            rt.key(KeyEvent { key: Key::Text(c.to_string()), mods: Mods::default() });
            rt.frame(1.0 / 60.0);
        }
        settle(&mut rt);
        assert_eq!(rt.state.command.text, "downloads", "preset {name}: what is typed reaches the palette");
        rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
        settle(&mut rt);
        assert_eq!(rt.state.nav, Nav::Downloads, "preset {name}: Enter runs Go to Downloads");
        assert_eq!(rt.state.overlay, Overlay::None, "preset {name}: and closes the palette");
    });
}

/// The pointer follows the place it wants as the navigation opens under it (a hover sidebar
/// grows its labels), then presses it there: the navigation must stay open under the pointer.
#[test]
fn a_place_pressed_where_the_opened_navigation_shows_it_goes_there_in_every_preset() {
    every_preset(|i, name| {
        for nav in [Nav::Downloads, Nav::Settings, Nav::Home] {
            let mut rt = runtime(i, true);
            if nav == Nav::Home {
                rt.update(|s, _| crate::actions::go(s, Nav::Downloads));
                settle(&mut rt);
            }
            let k = keys::nav(nav);
            let (x, y) = rt.find(k).unwrap_or_else(|| panic!("preset {name}: no `{k}`"));
            rt.pointer_move(x, y);
            settle(&mut rt);
            let (x, y) = rt.find(k).unwrap_or_else(|| panic!("preset {name}: `{k}` went away under the pointer"));
            rt.pointer_move(x, y);
            settle(&mut rt);
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            settle(&mut rt);
            assert_eq!(rt.state.nav, nav, "preset {name}: `{k}` pressed where it is drawn once the navigation opened");
        }
    });
}
