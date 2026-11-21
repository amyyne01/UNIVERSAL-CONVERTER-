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

/// The power persona's spam (U0a, One bar 50 rows became 48): Cancel x3, then Retry x10 one
/// frame apart at one spot. The retried row leaves its section and a finished row slides under
/// the pointer: its Remove, hidden until the row is hovered, must not be what is there.
#[test]
fn cancel_then_retry_mashed_on_the_first_of_fifty_rows_keeps_every_row_in_every_preset() {
    use ahg_engine::types::DownloadStatus as St;
    use keys::RowAct;
    every_preset(|i, name| {
        let mut ux = super::Ux::new(i, true, super::Persona::Power, 6000);
        ux.rt.state.plan = ahg_engine::tier::Plan::Premium;
        let links: Vec<String> = (0..50).map(|i| format!("https://www.youtube.com/watch?v=ux{i:09}")).collect();
        assert!(ux.type_in_field(&links.join("\n")), "preset {name}: the field");
        ux.key(Key::Enter);
        assert!(ux.wait(20.0, |s| s.tasks.len() >= 50), "preset {name}: 50 rows");
        // Finished rows below the troubled ones, as in the persona's run.
        let done = |s: &crate::gate::State| s.tasks.iter().filter(|t| t.progress.status == St::Done).count() >= 2;
        assert!(ux.wait(40.0, done), "preset {name}: two downloads finish");
        if ux.rt.state.nav != Nav::Downloads {
            ux.go(Nav::Downloads);
        }
        let id = ux.rt.state.tasks.iter().find(|t| crate::format::is_active(t)).unwrap().task_id.clone();
        assert!(ux.mash(&keys::row_action(RowAct::Cancel, &id), 3), "preset {name}: the row's Cancel");
        if !ux.wait(2.0, |s| s.tasks.iter().any(|t| t.progress.status == St::Cancelled)) {
            ux.rt.update(move |s, tx| crate::actions::cancel(s, tx, &id));
            ux.settle();
        }
        let gone = ux.rt.state.tasks.iter().find(|t| t.progress.status == St::Cancelled).map(|t| t.task_id.clone());
        let gone = gone.unwrap_or_else(|| panic!("preset {name}: nothing cancelled"));
        let rows = ux.tasks();
        assert!(ux.mash(&keys::row_action(RowAct::Retry, &gone), 10), "preset {name}: the row's Retry");
        ux.wait(3.0, |_| false);
        assert_eq!(ux.tasks(), rows, "preset {name}: Cancel x3 then Retry x10 keeps every row");
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

/// Hovers `key` from an empty spot: (lit in the first frame, lit once settled), or `None` when
/// the preset has no such control. A control that shows only in an opened hover sidebar (its
/// pin) is reached through the sidebar, as a user does.
fn hover_lights(rt: &mut rune::Runtime<crate::gate::State>, key: &str) -> Option<(bool, bool)> {
    rt.pointer_leave();
    let k = key.to_owned();
    rt.update(move |_, tx| tx.scroll_into_view(k));
    rt.settle_all();
    settle(rt);
    if rt.find(key).is_none() {
        let (x, y) = rt.find(keys::nav(Nav::Settings))?;
        rt.pointer_move(x, y);
        rt.settle_all();
        settle(rt);
    }
    let (x, y) = rt.find(key)?;
    let path = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key))?.path;
    let before = rt.elements();
    rt.pointer_move(x, y);
    rt.frame(1.0 / 60.0);
    let first = rt.elements();
    rt.settle_all();
    settle(rt);
    let after = rt.elements();
    let shows =
        |els: &[rune::ElementInfo]| super::hover::lit(els, &path) || super::hover::paint(&before, &path) != super::hover::paint(els, &path);
    Some((super::hover::lit(&first, &path), shows(&after)))
}

/// What U0a found unlit (the settings switches, the day chips, the swatches and theme
/// segments, the pin, the chosen filter, the place you are in, Shelf's and Orbit's Change): each
/// shows its look's hover in every preset and theme.
// Not held yet (step UXF item 4): in the four hover sidebars a place lights one frame after the
// pointer lands (the opening commit re-lays the strip out); `hover_lights` reports the first frame.
#[test]
fn the_controls_the_personas_found_unlit_show_hover_in_every_preset() {
    let mut must: Vec<(Nav, String)> = ["skipExisting", "embedThumbnail", "embedMetadata", "holdUnconfirmed"]
        .iter()
        .map(|k| (Nav::Settings, keys::setting(k)))
        .chain((0..3).map(|i| (Nav::Settings, keys::theme(i))))
        .chain((0..8).map(|i| (Nav::Settings, keys::palette(i))))
        .chain(crate::gate::Filter::ALL.into_iter().map(|f| (Nav::Downloads, keys::filter(f))))
        .collect();
    for nav in [Nav::Home, Nav::Downloads, Nav::Settings] {
        must.extend([Nav::Home, Nav::Downloads, Nav::Settings].map(|on| (nav, keys::nav(on).to_owned())));
    }
    let may: Vec<(Nav, String)> = ["scheduleEnabled", "scheduleShutdown", "autoUpdateEngine"]
        .iter()
        .map(|k| (Nav::Settings, keys::setting(k)))
        .chain((0..7).map(|i| (Nav::Settings, keys::day(i))))
        .chain([
            (Nav::Downloads, keys::NAV_PIN.to_owned()),
            (Nav::Settings, keys::NAV_PIN.to_owned()),
            (Nav::Home, keys::CHANGE_FOLDER.to_owned()),
        ])
        .collect();
    every_preset(|i, name| {
        let mut bad = Vec::new();
        for dark in [true, false] {
            let mut rt = runtime(i, dark);
            for (required, list) in [(true, &must), (false, &may)] {
                for (nav, key) in list.iter() {
                    let nav = *nav;
                    rt.update(move |s, _| crate::actions::go(s, nav));
                    settle(&mut rt);
                    match hover_lights(&mut rt, key) {
                        None if required => bad.push(format!("no `{key}` on {nav:?}")),
                        None => {}
                        Some((_, false)) => bad.push(format!("`{key}` on {nav:?}: no hover")),
                        Some(_) => {}
                    }
                }
            }
        }
        assert!(bad.is_empty(), "preset {name}: {}", bad.join("; "));
    });
}

/// A playlist's Download with nothing ticked is disabled, as Electron's: drawn so, taking no
/// press, adding no task, in every preset and theme.
#[test]
fn a_playlist_download_with_nothing_ticked_is_disabled_in_every_preset() {
    every_preset(|i, name| {
        for dark in [true, false] {
            let mut rt = runtime(i, dark);
            rt.update(|s, _| {
                if let Some((_, playlist)) = crate::gate::SCENES.iter().find(|(n, _)| *n == "playlist") {
                    playlist(s);
                }
                crate::actions::deselect_all_tracks(s);
            });
            settle(&mut rt);
            let rows = rt.state.tasks.len();
            let k = keys::FOUND_DOWNLOAD.to_owned();
            rt.update(move |_, tx| tx.scroll_into_view(k));
            settle(&mut rt);
            let b = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(keys::FOUND_DOWNLOAD) && !e.exiting);
            let b = b.unwrap_or_else(|| panic!("preset {name}: no playlist Download"));
            assert!(b.inert, "preset {name}: the Download is drawn disabled and takes no press");
            let (x, y) = rt.find(keys::FOUND_DOWNLOAD).unwrap();
            rt.pointer_move(x, y);
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            settle(&mut rt);
            assert_eq!(rt.state.tasks.len(), rows, "preset {name}: the press adds nothing");
            assert!(matches!(rt.state.found, crate::gate::Found::Playlist { .. }), "preset {name}: the step stays open");
            // One tick and it is a button again.
            rt.update(|s, _| {
                if let crate::gate::Found::Playlist { selected, .. } = &mut s.found {
                    selected[0] = true;
                }
            });
            settle(&mut rt);
            let b = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(keys::FOUND_DOWNLOAD) && !e.exiting).unwrap();
            assert!(!b.inert && b.focusable, "preset {name}: with a tick the Download takes presses");
        }
    });
}
