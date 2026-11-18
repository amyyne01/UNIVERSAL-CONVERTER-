//! Orbit and Now playing answer to their controls: each drawn control, found by its shared key
//! (`keys.rs`), calls its action (`actions.rs`) and the state shows it. A missing key fails the
//! test naming the preset and the key. Both presets are driven through Rune's scripted runtime.
use ahg_engine::types::DownloadStatus as St;
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};

use crate::gate::{self, Band, Confirm, Filter, Found, Nav, Overlay, PRESETS, State};
use crate::{actions, keys};

/// The two presets of this lane, by their names in `PRESETS`.
const LANE: [&str; 2] = ["Orbit", "Now playing"];

fn runtime(preset: &str) -> Runtime<State> {
    let index = PRESETS.iter().position(|p| p.0 == preset).unwrap_or_else(|| panic!("no preset named {preset}"));
    let mut rt = App::new(crate::spec()).headless_runtime((1240.0, 820.0), Env { system_dark: true, scale: 1.0 });
    rt.frame(0.0);
    rt.update(move |s, tx| actions::set_preset(s, index, tx));
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
}

/// Runs `check` on a fresh runtime of each preset of the lane.
fn each(check: impl Fn(&mut Runtime<State>, &str)) {
    assert!(!LANE.is_empty());
    for preset in LANE {
        check(&mut runtime(preset), preset);
    }
}

/// Puts the state in one of the named scenes.
fn scene(rt: &mut Runtime<State>, name: &'static str) {
    let (_, set) = gate::SCENES.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("no scene {name}"));
    rt.update(move |s, _| {
        set(s);
        s.sync();
    });
    settle(rt);
}

/// Presses the control with this key, as a pointer does. A missing key names the preset and the key.
fn click(rt: &mut Runtime<State>, preset: &str, key: &str) {
    assert!(rt.find(key).is_some(), "preset {preset}: no element with key `{key}`");
    let to = key.to_owned();
    rt.update(move |_, tx| tx.scroll_into_view(to));
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("preset {preset}: `{key}` went away"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn press(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    settle(rt);
}

fn type_text(rt: &mut Runtime<State>, text: &str) {
    press(rt, Key::Text(text.into()), Mods::default());
}

fn status(rt: &Runtime<State>, id: &str) -> St {
    rt.state.tasks.iter().find(|t| t.task_id == id).unwrap_or_else(|| panic!("no task {id}")).progress.status
}

#[test]
fn a_link_typed_in_the_field_is_looked_up_on_enter() {
    each(|rt, p| {
        const LINK: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        click(rt, p, keys::FIELD);
        type_text(rt, LINK);
        assert_eq!(rt.state.input, LINK, "preset {p}: typing mirrors into State.input");
        press(rt, Key::Enter, Mods::default());
        assert!(!matches!(rt.state.found, Found::Nothing), "preset {p}: Enter in the field looks the link up");
    });
}

#[test]
fn words_typed_in_the_field_search_and_look_up_is_a_button() {
    each(|rt, p| {
        click(rt, p, keys::FIELD);
        type_text(rt, "daft punk");
        click(rt, p, keys::LOOK_UP);
        assert!(matches!(rt.state.found, Found::Search { .. }), "preset {p}: Look up searches words");
    });
}

#[test]
fn a_format_chip_sets_the_choice_and_download_queues_a_task() {
    each(|rt, p| {
        scene(rt, "media");
        click(rt, p, keys::CHOICE_AUDIO);
        assert!(!rt.state.choice.video, "preset {p}: Audio");
        click(rt, p, &keys::choice_format("flac"));
        assert_eq!(rt.state.choice.format, "flac", "preset {p}: the flac chip");
        let before = rt.state.tasks.len();
        click(rt, p, keys::FOUND_DOWNLOAD);
        assert_eq!(rt.state.tasks.len(), before + 1, "preset {p}: Download queues the item");
        assert!(!rt.state.notices.is_empty(), "preset {p}: Download says it was added");
    });
}

#[test]
fn the_found_step_closes_and_a_pick_ticks() {
    each(|rt, p| {
        scene(rt, "search");
        let Found::Search { bands, .. } = &rt.state.found else { panic!("preset {p}: the search scene found no search") };
        let id = bands
            .iter()
            .find_map(|b| if let Band::Results(rows) = b { rows.first().map(|r| r.id.clone()) } else { None })
            .unwrap_or_else(|| panic!("preset {p}: the search scene has no results"));
        click(rt, p, &keys::result_tick(&id));
        assert!(rt.state.picked.contains(&id), "preset {p}: the result's tick");
        click(rt, p, keys::RESULTS_CLEAR);
        assert!(rt.state.picked.is_empty(), "preset {p}: Clear unticks the results");
        click(rt, p, keys::FOUND_CLOSE);
        assert!(matches!(rt.state.found, Found::Nothing), "preset {p}: the close button");
    });
}

#[test]
fn a_playlist_track_ticks() {
    each(|rt, p| {
        scene(rt, "playlist");
        let before = matches!(&rt.state.found, Found::Playlist { selected, .. } if selected[0]);
        click(rt, p, &keys::track(0));
        let after = matches!(&rt.state.found, Found::Playlist { selected, .. } if selected[0]);
        assert_ne!(before, after, "preset {p}: the first track's row");
        click(rt, p, keys::FOUND_SELECT_ALL);
    });
}

#[test]
fn a_row_pauses_resumes_and_cancels_with_undo() {
    each(|rt, p| {
        click(rt, p, keys::nav(Nav::Downloads));
        click(rt, p, &keys::row_action(keys::RowAct::Pause, "get-lucky"));
        assert_eq!(status(rt, "get-lucky"), St::Paused, "preset {p}: Pause");
        click(rt, p, &keys::row_action(keys::RowAct::Resume, "get-lucky"));
        assert_ne!(status(rt, "get-lucky"), St::Paused, "preset {p}: Resume");
        click(rt, p, &keys::row_action(keys::RowAct::Cancel, "big-buck-bunny"));
        assert_eq!(status(rt, "big-buck-bunny"), St::Cancelled, "preset {p}: Cancel");
        assert!(!rt.state.notices.is_empty(), "preset {p}: Cancel says so with an Undo");
        click(rt, p, keys::NOTICE_ACTION);
        assert_ne!(status(rt, "big-buck-bunny"), St::Cancelled, "preset {p}: Undo");
    });
}

#[test]
fn a_finished_row_opens_retries_and_removes() {
    each(|rt, p| {
        click(rt, p, keys::nav(Nav::Downloads));
        click(rt, p, &keys::row_action(keys::RowAct::Retry, "cancelled"));
        assert_eq!(status(rt, "cancelled"), St::Queued, "preset {p}: Retry starts the row over");
        click(rt, p, &keys::row_action(keys::RowAct::Remove, "dreams"));
        assert!(rt.state.tasks.iter().all(|t| t.task_id != "dreams"), "preset {p}: Remove takes the row out");
    });
}

#[test]
fn the_chips_filter_and_the_box_searches() {
    each(|rt, p| {
        click(rt, p, keys::nav(Nav::Downloads));
        for f in Filter::ALL {
            click(rt, p, &keys::filter(f));
            assert_eq!(rt.state.filter, f, "preset {p}: {}", keys::filter(f));
        }
        click(rt, p, &keys::filter(Filter::All));
        click(rt, p, keys::DOWNLOADS_SEARCH);
        type_text(rt, "artemis");
        assert_eq!(rt.state.query.text, "artemis", "preset {p}: the search box");
        assert_eq!(actions::visible(&rt.state).count(), 1, "preset {p}: one row matches");
    });
}

#[test]
fn two_ticked_rows_are_removed_through_the_confirm() {
    each(|rt, p| {
        click(rt, p, keys::nav(Nav::Downloads));
        click(rt, p, &keys::row_select("dreams"));
        click(rt, p, &keys::row_select("humble"));
        assert_eq!(rt.state.selected.len(), 2, "preset {p}: two ticks");
        click(rt, p, keys::arm(Confirm::Remove));
        assert_eq!(rt.state.confirm, Some(Confirm::Remove), "preset {p}: Remove arms");
        click(rt, p, keys::CONFIRM_NO);
        assert_eq!(rt.state.confirm, None, "preset {p}: Cancel disarms");
        click(rt, p, keys::arm(Confirm::Remove));
        click(rt, p, keys::CONFIRM_YES);
        assert!(rt.state.tasks.iter().all(|t| t.task_id != "dreams" && t.task_id != "humble"), "preset {p}: the confirm removes both");
        assert!(rt.state.selected.is_empty(), "preset {p}: nothing stays ticked");
    });
}

#[test]
fn select_all_and_clear_selection() {
    each(|rt, p| {
        click(rt, p, keys::nav(Nav::Downloads));
        click(rt, p, keys::SELECT_ALL);
        assert_eq!(rt.state.selected.len(), rt.state.tasks.len(), "preset {p}: select all");
        click(rt, p, keys::CLEAR_SELECTION);
        assert!(rt.state.selected.is_empty(), "preset {p}: Clear selection");
    });
}

#[test]
fn the_bulk_buttons_act_on_every_row() {
    each(|rt, p| {
        click(rt, p, keys::nav(Nav::Downloads));
        click(rt, p, keys::PAUSE_ALL);
        assert_eq!(status(rt, "get-lucky"), St::Paused, "preset {p}: Pause all");
        click(rt, p, keys::RETRY_ALL_FAILED);
        assert_eq!(status(rt, "no-match"), St::Queued, "preset {p}: Retry all failed");
        click(rt, p, keys::arm(Confirm::Clear));
        click(rt, p, keys::CONFIRM_YES);
        assert!(rt.state.tasks.iter().all(|t| t.progress.status != St::Done), "preset {p}: Clear completed");
        click(rt, p, keys::CANCEL_ALL);
        assert!(rt.state.tasks.iter().all(|t| !crate::format::is_active(t)), "preset {p}: Cancel all");
    });
}

#[test]
fn an_empty_filter_offers_show_all() {
    each(|rt, p| {
        scene(rt, "filtered-empty");
        click(rt, p, keys::SHOW_ALL);
        assert_eq!(rt.state.filter, Filter::All, "preset {p}: Show all");
    });
}

#[test]
fn settings_controls_write_their_setting() {
    each(|rt, p| {
        click(rt, p, keys::nav(Nav::Settings));
        click(rt, p, &keys::theme(1));
        assert_eq!(rt.state.settings.get("theme").and_then(|v| v.as_str()), Some("dark"), "preset {p}: Dark");
        click(rt, p, &keys::palette(2));
        assert_eq!(rt.state.palette, 2, "preset {p}: a palette swatch");
        // Far down the page: the walk's own toggle, reached by scrolling to it.
        let before = rt.state.settings.get("showNotifications").cloned();
        click(rt, p, &keys::setting("showNotifications"));
        assert_ne!(rt.state.settings.get("showNotifications"), before.as_ref(), "preset {p}: a toggle");
        click(rt, p, &keys::setting("defaultFormat"));
        rt.resolve_popup(Some("flac")).unwrap_or_else(|e| panic!("preset {p}: the format select opens no list: {e:?}"));
        settle(rt);
        assert_eq!(rt.state.settings.get("defaultFormat").and_then(|v| v.as_str()), Some("flac"), "preset {p}: a select");
        click(rt, p, &keys::setting("maxConcurrentDownloads"));
        rt.resolve_popup(Some("2")).unwrap_or_else(|e| panic!("preset {p}: the slots select opens no list: {e:?}"));
        settle(rt);
        assert_eq!(rt.state.settings.get("maxConcurrentDownloads").and_then(|v| v.as_u64()), Some(2), "preset {p}: a numeric select");
    });
}

#[test]
fn the_schedule_days_toggle() {
    each(|rt, p| {
        click(rt, p, keys::nav(Nav::Settings));
        let on = |rt: &Runtime<State>| {
            rt.state.settings.get("scheduleDays").and_then(|v| v.as_array()).is_some_and(|d| d.iter().any(|x| x.as_u64() == Some(3)))
        };
        let before = on(rt);
        click(rt, p, &keys::day(3));
        assert_ne!(on(rt), before, "preset {p}: Wednesday");
    });
}

#[test]
fn the_status_and_danger_buttons_act() {
    each(|rt, p| {
        click(rt, p, keys::nav(Nav::Settings));
        click(rt, p, keys::CHECK_UPDATE);
        assert_eq!(rt.state.update.phase, "checking", "preset {p}: Check for updates");
        click(rt, p, keys::CHECK_ENGINE);
        assert_eq!(rt.state.engine.state, "checking", "preset {p}: Check the engine");
        click(rt, p, keys::SHOW_SHORTCUTS);
        assert_eq!(rt.state.overlay, Overlay::Shortcuts, "preset {p}: Show shortcuts");
        click(rt, p, keys::SHORTCUTS_CLOSE);
        assert_eq!(rt.state.overlay, Overlay::None, "preset {p}: the sheet's close button");
        click(rt, p, keys::arm(Confirm::Deactivate));
        assert_eq!(rt.state.confirm, Some(Confirm::Deactivate), "preset {p}: the danger row arms");
        click(rt, p, keys::CONFIRM_NO);
        assert_eq!(rt.state.confirm, None, "preset {p}: and disarms");
    });
}

#[test]
fn the_layout_button_opens_the_picker_and_a_card_switches_preset() {
    each(|rt, p| {
        click(rt, p, keys::nav(Nav::Settings));
        click(rt, p, keys::LAYOUT_CHOOSE);
        assert!(rt.state.picker, "preset {p}: the Layout button");
        click(rt, p, &keys::preset(0));
        assert_eq!(gate::preset(&rt.state), 0, "preset {p}: the first card switches to {}", PRESETS[0].0);
    });
}

#[test]
fn the_palette_and_the_shortcuts_open_and_close() {
    each(|rt, p| {
        click(rt, p, "search");
        assert_eq!(rt.state.overlay, Overlay::Palette, "preset {p}: the search capsule opens the palette");
        press(rt, Key::Escape, Mods::default());
        assert_eq!(rt.state.overlay, Overlay::None, "preset {p}: Esc closes it");
        press(rt, Key::Command('/'), Mods { cmd: true, ctrl: true, ..Mods::default() });
        assert_eq!(rt.state.overlay, Overlay::Shortcuts, "preset {p}: Ctrl+/");
        click(rt, p, keys::SHORTCUTS_CLOSE);
        assert_eq!(rt.state.overlay, Overlay::None, "preset {p}: close");
    });
}

#[test]
fn the_places_answer_to_their_buttons_and_ctrl_digits() {
    each(|rt, p| {
        for nav in [Nav::Downloads, Nav::Settings, Nav::Home] {
            click(rt, p, keys::nav(nav));
            assert_eq!(rt.state.nav, nav, "preset {p}: {}", keys::nav(nav));
        }
        press(rt, Key::Command('2'), Mods { cmd: true, ctrl: true, ..Mods::default() });
        assert_eq!(rt.state.nav, Nav::Downloads, "preset {p}: Ctrl+2");
    });
}

#[test]
fn orbits_sidebar_pins_and_its_pill_and_view_all_go_to_downloads() {
    let mut rt = runtime("Orbit");
    let (x, y) = rt.find(keys::NAV_HOME).expect("Orbit: no nav-home");
    rt.pointer_move(x, y);
    settle(&mut rt);
    click(&mut rt, "Orbit", keys::NAV_PIN);
    assert!(rt.state.nav_pinned, "Orbit: Keep open pins the sidebar");
    click(&mut rt, "Orbit", "in-flight");
    assert_eq!(rt.state.nav, Nav::Downloads, "Orbit: the N in flight pill");
    click(&mut rt, "Orbit", keys::nav(Nav::Home));
    click(&mut rt, "Orbit", "view-all");
    assert_eq!(rt.state.nav, Nav::Downloads, "Orbit: View all");
}

#[test]
fn now_playings_satellite_takes_the_stage_and_its_transport_pauses() {
    let mut rt = runtime("Now playing");
    let sat =
        rt.elements().into_iter().filter_map(|e| e.key).find(|k| k.starts_with("sat-")).expect("Now playing: the sample has no satellite");
    click(&mut rt, "Now playing", &sat);
    assert_eq!(rt.state.staged.as_deref(), sat.strip_prefix("sat-"), "Now playing: a satellite takes the stage");
    click(&mut rt, "Now playing", keys::PAUSE_ALL);
    assert!(rt.state.tasks.iter().any(|t| t.progress.status == St::Paused && t.task_id == "get-lucky"), "Now playing: play-pause");
}
