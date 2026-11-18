//! Every drawn control of Signal and One bar answers to its action: each test drives the real
//! window headlessly (scripted pointer and keys) and reads the result from `State`. A missing key
//! fails with the preset and the key in the message.
use ahg_engine::types::DownloadStatus as St;
use rune::{App, Env, Key, KeyEvent, Mods, PhysicalInput, Runtime};
use serde_json::Value;

use crate::gate::{Act, Band, Confirm, Filter, Found, Matched, Nav, Overlay, State};
use crate::keys::{self, RowAct};
use crate::{actions, format, sample};

/// Signal (the classic look) and One bar (its layout), by place in `gate::PRESETS`.
const LOOKS: [(usize, &str); 2] = [(3, "Signal"), (0, "One bar")];

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
}

fn runtime(preset: usize) -> Runtime<State> {
    let mut rt = App::new(crate::spec()).headless_runtime((1240.0, 820.0), Env { system_dark: true, scale: 1.0 });
    rt.frame(0.0);
    rt.update(move |s, tx| actions::set_preset(s, preset, tx));
    settle(&mut rt);
    rt
}

/// The test body once per look, on a fresh window.
fn each(body: impl Fn(&mut Runtime<State>, &str)) {
    assert!(!LOOKS.is_empty());
    for (preset, name) in LOOKS {
        let mut rt = runtime(preset);
        body(&mut rt, name);
    }
}

fn go(rt: &mut Runtime<State>, nav: Nav) {
    rt.update(move |s, _| actions::go(s, nav));
    settle(rt);
}

/// A press on the control with this key, scrolled into view first.
fn click_with(rt: &mut Runtime<State>, name: &str, key: &str, mods: Mods) {
    let k = key.to_owned();
    rt.update(move |_, tx| tx.scroll_into_view(k));
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("preset {name}: no element with key `{key}`"));
    rt.physical_input(PhysicalInput::Modifiers(mods));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    // The press runs on the next frame, with the modifiers still held.
    settle(rt);
    rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
}

fn click(rt: &mut Runtime<State>, name: &str, key: &str) {
    click_with(rt, name, key, Mods::default());
}

fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    settle(rt);
}

fn type_text(rt: &mut Runtime<State>, text: &str) {
    for c in text.chars() {
        rt.key(KeyEvent { key: Key::Text(c.to_string()), mods: Mods::default() });
    }
    settle(rt);
}

fn first(rt: &Runtime<State>, status: St) -> String {
    rt.state.tasks.iter().find(|t| t.progress.status == status).map(|t| t.task_id.clone()).unwrap_or_else(|| panic!("no {status:?} task"))
}

fn status(rt: &Runtime<State>, id: &str) -> St {
    rt.state.tasks.iter().find(|t| t.task_id == id).map(|t| t.progress.status).unwrap_or_else(|| panic!("no task {id}"))
}

fn setting(rt: &Runtime<State>, key: &str) -> Option<Value> {
    rt.state.settings.get(key).cloned()
}

/// What the field found, in a word.
fn found_kind(s: &State) -> &'static str {
    match &s.found {
        Found::Nothing => "nothing",
        Found::LookingUp => "looking up",
        Found::LookupFailed(_) => "failed",
        Found::Search { .. } => "search",
        Found::Playlist { .. } => "playlist",
        Found::Media(_) => "media",
    }
}

#[test]
fn the_field_types_and_enter_searches_and_look_up_looks_up() {
    each(|rt, name| {
        click(rt, name, keys::FIELD);
        type_text(rt, "khruangbin");
        assert_eq!(
            (rt.state.field.text.as_str(), rt.state.input.as_str()),
            ("khruangbin", "khruangbin"),
            "preset {name}: typing fills the field and its mirror"
        );
        press(rt, Key::Enter);
        assert_eq!(found_kind(&rt.state), "search", "preset {name}: Enter searches words");

        rt.update(|s, _| actions::dismiss_found(s));
        settle(rt);
        click(rt, name, keys::FIELD);
        type_text(rt, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        click(rt, name, keys::LOOK_UP);
        assert_eq!(found_kind(&rt.state), "looking up", "preset {name}: Look up reads the link (field `{}`)", rt.state.field.text);
    });
}

#[test]
fn the_found_step_chooses_downloads_ticks_and_closes() {
    each(|rt, name| {
        rt.update(|s, _| {
            s.found = Found::Media(sample::media());
            s.choice.video = false;
        });
        settle(rt);
        click(rt, name, &keys::choice_format("flac"));
        assert_eq!(rt.state.choice.format, "flac", "preset {name}: a format chip");
        click(rt, name, keys::CHOICE_VIDEO);
        assert!(rt.state.choice.video, "preset {name}: Video");
        click(rt, name, keys::CHOICE_AUDIO);
        assert!(!rt.state.choice.video, "preset {name}: Audio");

        let before = rt.state.tasks.len();
        click(rt, name, keys::FOUND_DOWNLOAD);
        assert_eq!(rt.state.tasks.len(), before + 1, "preset {name}: Download queues the track");

        rt.update(|s, _| s.found = Found::Media(sample::media()));
        settle(rt);
        click(rt, name, keys::FOUND_CLOSE);
        assert_eq!(found_kind(&rt.state), "nothing", "preset {name}: the close button");
    });
}

#[test]
fn a_search_result_and_an_album_track_tick_and_show_more_opens() {
    each(|rt, name| {
        let bands = sample::search();
        let (id, more) = (bands[0][0].id.clone(), bands[0].len() > 5);
        rt.update(move |s, _| s.found = Found::Search { query: sample::QUERY.into(), bands: bands.map(Band::Results) });
        settle(rt);
        click(rt, name, &keys::result(&id));
        assert!(rt.state.picked.contains(&id), "preset {name}: a result ticks");
        click(rt, name, &keys::result_tick(&id));
        assert!(rt.state.picked.is_empty(), "preset {name}: its own tick unticks");
        click(rt, name, &keys::result(&id));
        click(rt, name, keys::RESULTS_CLEAR);
        assert!(rt.state.picked.is_empty(), "preset {name}: Clear lets go");
        if more {
            click(rt, name, &keys::band_more(0));
            assert!(rt.state.expanded[0], "preset {name}: Show more");
        }

        rt.update(|s, _| {
            let playlist = sample::playlist();
            let rows = playlist.tracks.len();
            s.found = Found::Playlist { playlist, matched: vec![Matched::Confirmed; rows], selected: vec![true; rows] };
        });
        settle(rt);
        click(rt, name, &keys::track(0));
        assert!(matches!(&rt.state.found, Found::Playlist { selected, .. } if !selected[0]), "preset {name}: a track unticks");
    });
}

#[test]
fn a_row_pauses_resumes_cancels_with_undo_and_a_finished_one_goes() {
    each(|rt, name| {
        go(rt, Nav::Downloads);
        let id = first(rt, St::Downloading);
        click(rt, name, &keys::row_action(RowAct::Pause, &id));
        assert_eq!(status(rt, &id), St::Paused, "preset {name}: Pause");
        click(rt, name, &keys::row_action(RowAct::Resume, &id));
        assert_eq!(status(rt, &id), St::Queued, "preset {name}: Resume");
        click(rt, name, &keys::row_action(RowAct::Cancel, &id));
        assert_eq!(status(rt, &id), St::Cancelled, "preset {name}: Cancel");
        assert!(
            rt.state.notices.last().is_some_and(|n| n.action == Some(Act::Undo(vec![id.clone()]))),
            "preset {name}: the notice offers Undo"
        );
        click(rt, name, keys::NOTICE_ACTION);
        assert_ne!(status(rt, &id), St::Cancelled, "preset {name}: Undo");

        let done = first(rt, St::Done);
        click(rt, name, &keys::row_action(RowAct::Remove, &done));
        assert!(!rt.state.tasks.iter().any(|t| t.task_id == done), "preset {name}: Remove");
    });
}

#[test]
fn downloads_filter_search_select_and_remove_through_the_confirm() {
    each(|rt, name| {
        go(rt, Nav::Downloads);
        for f in Filter::ALL {
            click(rt, name, &keys::filter(f));
            assert_eq!(rt.state.filter, f, "preset {name}: chip {}", keys::filter(f));
        }
        click(rt, name, &keys::filter(Filter::All));

        click(rt, name, keys::DOWNLOADS_SEARCH);
        type_text(rt, "zzzqq");
        assert_eq!(rt.state.query.text, "zzzqq", "preset {name}: the search types");
        assert_eq!(actions::visible(&rt.state).count(), 0, "preset {name}: nothing matches");
        rt.update(|s, _| s.query.clear());
        settle(rt);

        let done: Vec<String> = rt.state.tasks.iter().filter(|t| t.progress.status == St::Done).take(2).map(|t| t.task_id.clone()).collect();
        assert_eq!(done.len(), 2, "two finished rows to pick");
        for id in &done {
            if name == "Signal" {
                click(rt, name, &keys::row_select(id));
            } else {
                click_with(rt, name, &keys::row(id), Mods { cmd: true, ctrl: true, ..Mods::default() });
            }
        }
        assert_eq!(rt.state.selected.len(), 2, "preset {name}: two rows picked");
        click(rt, name, keys::arm(Confirm::Remove));
        assert_eq!(rt.state.confirm, Some(Confirm::Remove), "preset {name}: Remove asks once more");
        click(rt, name, keys::CONFIRM_NO);
        assert_eq!(rt.state.confirm, None, "preset {name}: Cancel");
        click(rt, name, keys::arm(Confirm::Remove));
        click(rt, name, keys::CONFIRM_YES);
        assert!(!rt.state.tasks.iter().any(|t| done.contains(&t.task_id)), "preset {name}: Yes removes them");
    });
}

#[test]
fn the_bulk_buttons_pause_cancel_retry_and_clear() {
    each(|rt, name| {
        go(rt, Nav::Downloads);
        let waiting = |rt: &Runtime<State>| rt.state.tasks.iter().filter(|t| matches!(t.progress.status, St::Queued | St::Downloading)).count();
        click(rt, name, keys::PAUSE_ALL);
        assert_eq!(waiting(rt), 0, "preset {name}: Pause all");
        let failed = first(rt, St::Failed);
        click(rt, name, keys::RETRY_ALL_FAILED);
        assert_ne!(status(rt, &failed), St::Failed, "preset {name}: Retry all failed");
        click(rt, name, keys::CANCEL_ALL);
        assert_eq!(format::active_count(&rt.state.tasks), 0, "preset {name}: Cancel all");
        click(rt, name, keys::arm(Confirm::Clear));
        assert_eq!(rt.state.confirm, Some(Confirm::Clear), "preset {name}: Clear completed asks");
        click(rt, name, keys::CONFIRM_YES);
        assert!(!rt.state.tasks.iter().any(|t| t.progress.status == St::Done), "preset {name}: the finished rows go");
    });
}

#[test]
fn settings_controls_write_settings() {
    each(|rt, name| {
        go(rt, Nav::Settings);
        click(rt, name, &keys::theme(0));
        assert_eq!(setting(rt, "theme"), Some(Value::from("light")), "preset {name}: Light");
        click(rt, name, &keys::palette(2));
        assert_eq!(rt.state.palette, 2, "preset {name}: a palette swatch");
        let days = setting(rt, "scheduleDays");
        click(rt, name, &keys::day(2));
        assert_ne!(setting(rt, "scheduleDays"), days, "preset {name}: a weekday chip");
        let before = setting(rt, "showNotifications");
        click(rt, name, &keys::setting("showNotifications"));
        assert_ne!(setting(rt, "showNotifications"), before, "preset {name}: a toggle");

        click(rt, name, keys::CHECK_UPDATE);
        assert_eq!(rt.state.update.phase, "checking", "preset {name}: Check for updates");
        click(rt, name, keys::CHECK_ENGINE);
        assert_eq!(rt.state.engine.state, "checking", "preset {name}: Check for engine update");
        click(rt, name, keys::SHOW_SHORTCUTS);
        assert_eq!(rt.state.overlay, Overlay::Shortcuts, "preset {name}: Show shortcuts");
        click(rt, name, keys::SHORTCUTS_CLOSE);
        assert_eq!(rt.state.overlay, Overlay::None, "preset {name}: the sheet closes");
    });
}

#[test]
fn the_layout_row_opens_the_picker_and_a_card_switches_preset() {
    each(|rt, name| {
        go(rt, Nav::Settings);
        click(rt, name, keys::LAYOUT_CHOOSE);
        assert!(rt.state.picker, "preset {name}: the picker opens");
        click(rt, name, &keys::preset(1));
        assert_eq!(crate::gate::preset(&rt.state), 1, "preset {name}: a card switches");
    });
}

#[test]
fn the_palette_opens_from_the_title_bar_and_the_rail_pins() {
    let mut rt = runtime(3);
    click(&mut rt, "Signal", keys::PALETTE);
    assert_eq!(rt.state.overlay, Overlay::Palette, "preset Signal: the search capsule opens the palette");
    press(&mut rt, Key::Escape);
    assert_eq!(rt.state.overlay, Overlay::None);
    rt.update(|s, _| s.nav_open = true);
    settle(&mut rt);
    click(&mut rt, "Signal", keys::NAV_PIN);
    assert!(rt.state.nav_pinned, "preset Signal: Keep open pins the rail");
}
