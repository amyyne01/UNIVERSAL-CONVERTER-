//! The handlers of Shelf and The collection, driven through Rune's scripted runtime: a real press
//! or key on the control that carries the shared key, then a read of `State`. A missing key fails
//! with the look and the key in the message.
use ahg_engine::types::DownloadStatus as St;
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};

use crate::gate::{self, Confirm, Filter, Found, Nav, Overlay, State};
use crate::keys::{self, RowAct};
use crate::{actions, format};

/// The preset's place in `PRESETS`, and its name for the messages.
const LOOKS: [(usize, &str); 2] = [(1, "The collection"), (4, "Shelf")];

fn runtime(preset: usize) -> Runtime<State> {
    let mut rt = App::new(crate::spec()).headless_runtime((1240.0, 3600.0), Env { system_dark: true, scale: 1.0 });
    crate::splash::skip(&mut rt.state); // the launch splash has its own tests (splash/tests.rs)
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

/// Puts the app in one of `gate::SCENES`.
fn scene(rt: &mut Runtime<State>, name: &str) {
    let (_, apply) = gate::SCENES.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("no scene `{name}`"));
    let apply = *apply;
    rt.update(move |s, _| apply(s));
    settle(rt);
}

/// Presses the control (the window is tall enough that nothing needs scrolling). A missing key is a panic naming the look and the key.
fn click(rt: &mut Runtime<State>, look: &str, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("{look}: no element with key `{key}`"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn press(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    settle(rt);
}

/// Focuses the field by pressing it, then types.
fn type_in(rt: &mut Runtime<State>, look: &str, key: &str, words: &str) {
    click(rt, look, key);
    for c in words.chars() {
        press(rt, Key::Text(c.to_string()), Mods::default());
    }
}

fn each_look(body: impl Fn(&mut Runtime<State>, &str)) {
    assert!(!LOOKS.is_empty());
    for (preset, look) in LOOKS {
        body(&mut runtime(preset), look);
    }
}

fn first_with(rt: &Runtime<State>, status: St) -> String {
    rt.state
        .tasks
        .iter()
        .find(|t| t.progress.status == status)
        .map(|t| t.task_id.clone())
        .unwrap_or_else(|| panic!("no {status:?} task in the sample"))
}

#[test]
fn a_typed_link_and_enter_queue_a_download() {
    each_look(|rt, look| {
        let before = rt.state.tasks.len();
        let link = "https://example.com/clip.mp3";
        type_in(rt, look, keys::FIELD, link);
        assert_eq!(
            (rt.state.field.text.as_str(), rt.state.input.as_str()),
            (link, link),
            "{look}: typing fills `{}` and its copy",
            keys::FIELD
        );
        press(rt, Key::Enter, Mods::default());
        assert_eq!(rt.state.tasks.len(), before + 1, "{look}: Enter in `{}` queued the link", keys::FIELD);
        assert_eq!(rt.state.nav, Nav::Downloads, "{look}: and the app shows Downloads");
    });
}

#[test]
fn typed_words_and_enter_search() {
    each_look(|rt, look| {
        type_in(rt, look, keys::FIELD, "khruangbin");
        press(rt, Key::Enter, Mods::default());
        assert!(matches!(rt.state.found, Found::Search { .. }), "{look}: words in `{}` search", keys::FIELD);
    });
}

#[test]
fn the_found_step_chooses_and_downloads() {
    each_look(|rt, look| {
        scene(rt, "media");
        click(rt, look, keys::CHOICE_AUDIO);
        assert!(!rt.state.choice.video, "{look}: `{}`", keys::CHOICE_AUDIO);
        click(rt, look, &keys::choice_format("flac"));
        assert_eq!(rt.state.choice.format, "flac", "{look}: `{}`", keys::choice_format("flac"));
        let before = rt.state.tasks.len();
        click(rt, look, keys::FOUND_DOWNLOAD);
        assert_eq!(rt.state.tasks.len(), before + 1, "{look}: `{}` queued the choice", keys::FOUND_DOWNLOAD);
        assert!(matches!(rt.state.found, Found::Nothing), "{look}: an added download closes the step");
        scene(rt, "media");
        click(rt, look, keys::FOUND_CLOSE);
        assert!(matches!(rt.state.found, Found::Nothing), "{look}: `{}`", keys::FOUND_CLOSE);
    });
}

#[test]
fn search_results_are_ticked() {
    each_look(|rt, look| {
        scene(rt, "search");
        click(rt, look, &keys::result_tick("yt-2"));
        assert!(rt.state.picked.contains("yt-2"), "{look}: `{}`", keys::result_tick("yt-2"));
        click(rt, look, &keys::result_tick("yt-2"));
        assert!(rt.state.picked.is_empty(), "{look}: ticking again unticks");
        // `keys::RESULTS_CLEAR` is drawn but no action empties `State.picked` yet (reported, not tested).
    });
}

#[test]
fn a_row_pauses_resumes_cancels_and_undoes() {
    each_look(|rt, look| {
        scene(rt, "active");
        click(rt, look, keys::NAV_DOWNLOADS);
        let id = first_with(rt, St::Downloading);
        let status = |rt: &Runtime<State>| rt.state.tasks.iter().find(|t| t.task_id == id).map(|t| t.progress.status);
        click(rt, look, &keys::row_action(RowAct::Pause, &id));
        assert_eq!(status(rt), Some(St::Paused), "{look}: {}", keys::row_action(RowAct::Pause, &id));
        click(rt, look, &keys::row_action(RowAct::Resume, &id));
        assert_eq!(status(rt), Some(St::Queued), "{look}: {}", keys::row_action(RowAct::Resume, &id));
        click(rt, look, &keys::row_action(RowAct::Cancel, &id));
        assert_eq!(status(rt), Some(St::Cancelled), "{look}: {}", keys::row_action(RowAct::Cancel, &id));
        assert!(!rt.state.notices.is_empty(), "{look}: the cancel offers Undo");
        click(rt, look, keys::NOTICE_ACTION);
        assert_ne!(status(rt), Some(St::Cancelled), "{look}: `{}` undid it", keys::NOTICE_ACTION);
    });
}

#[test]
fn the_filter_and_the_search_narrow_downloads() {
    each_look(|rt, look| {
        click(rt, look, keys::NAV_DOWNLOADS);
        let all = actions::visible(&rt.state).count();
        click(rt, look, &keys::filter(Filter::Done));
        assert_eq!(rt.state.filter, Filter::Done, "{look}: {}", keys::filter(Filter::Done));
        click(rt, look, &keys::filter(Filter::All));
        type_in(rt, look, keys::DOWNLOADS_SEARCH, "artemis");
        assert_eq!(rt.state.query.text, "artemis", "{look}: typing in `{}`", keys::DOWNLOADS_SEARCH);
        let some = actions::visible(&rt.state).count();
        assert!(some < all, "{look}: the search narrowed {all} rows to {some}");
    });
}

#[test]
fn two_ticked_rows_are_removed_through_the_confirm() {
    each_look(|rt, look| {
        click(rt, look, keys::NAV_DOWNLOADS);
        let done: Vec<String> =
            rt.state.tasks.iter().filter(|t| t.progress.status == St::Done).take(2).map(|t| t.task_id.clone()).collect();
        assert_eq!(done.len(), 2, "{look}: the sample has two finished downloads");
        for id in &done {
            click(rt, look, &keys::row_select(id));
        }
        assert_eq!(rt.state.selected.len(), 2, "{look}: `{}` ticks", keys::row_select(&done[0]));
        click(rt, look, keys::arm(Confirm::Remove));
        assert_eq!(rt.state.confirm, Some(Confirm::Remove), "{look}: {}", keys::arm(Confirm::Remove));
        let before = rt.state.tasks.len();
        click(rt, look, keys::CONFIRM_YES);
        assert_eq!(rt.state.tasks.len(), before - 2, "{look}: `{}` removed both", keys::CONFIRM_YES);
        assert!(rt.state.selected.is_empty() && rt.state.confirm.is_none(), "{look}: nothing stays ticked or armed");
    });
}

#[test]
fn the_confirm_can_be_walked_back() {
    each_look(|rt, look| {
        click(rt, look, keys::NAV_DOWNLOADS);
        click(rt, look, keys::arm(Confirm::Clear));
        assert_eq!(rt.state.confirm, Some(Confirm::Clear), "{look}: {}", keys::arm(Confirm::Clear));
        click(rt, look, keys::CONFIRM_NO);
        assert_eq!(rt.state.confirm, None, "{look}: `{}`", keys::CONFIRM_NO);
    });
}

#[test]
fn the_bulk_actions_act() {
    each_look(|rt, look| {
        scene(rt, "active");
        click(rt, look, keys::NAV_DOWNLOADS);
        click(rt, look, keys::PAUSE_ALL);
        let running = |rt: &Runtime<State>| {
            rt.state.tasks.iter().filter(|t| matches!(t.progress.status, St::Queued | St::FetchingInfo | St::Downloading)).count()
        };
        assert!(running(rt) == 0 && rt.state.tasks.iter().any(|t| t.progress.status == St::Paused), "{look}: `{}`", keys::PAUSE_ALL);
        click(rt, look, keys::CANCEL_ALL);
        assert!(rt.state.tasks.iter().all(|t| !format::is_active(t)), "{look}: `{}`", keys::CANCEL_ALL);
    });
}

#[test]
fn settings_write_through_set_setting() {
    each_look(|rt, look| {
        click(rt, look, keys::NAV_SETTINGS);
        click(rt, look, &keys::theme(1));
        assert_eq!(rt.state.settings.get("theme").and_then(|v| v.as_str()), Some("dark"), "{look}: {}", keys::theme(1));
        click(rt, look, &keys::palette(2));
        assert_eq!(rt.state.palette, 2, "{look}: {}", keys::palette(2));
        for name in ["embedThumbnail", "showNotifications"] {
            let key = keys::setting(name);
            let before = rt.state.settings.get(name).cloned();
            click(rt, look, &key);
            assert_ne!(rt.state.settings.get(name), before.as_ref(), "{look}: {key}");
        }
        let days = rt.state.settings.get("scheduleDays").cloned();
        click(rt, look, &keys::day(2));
        assert_ne!(rt.state.settings.get("scheduleDays"), days.as_ref(), "{look}: {}", keys::day(2));
    });
}

#[test]
fn the_sidebar_pins() {
    each_look(|rt, look| {
        rt.update(|s, _| s.nav_open = true);
        settle(rt);
        click(rt, look, keys::NAV_PIN);
        assert!(rt.state.nav_pinned && rt.state.own.nav_pinned, "{look}: `{}` pins", keys::NAV_PIN);
        click(rt, look, keys::NAV_PIN);
        assert!(!rt.state.nav_pinned, "{look}: `{}` unpins", keys::NAV_PIN);
    });
}

#[test]
fn the_picker_switches_preset() {
    each_look(|rt, look| {
        click(rt, look, keys::NAV_SETTINGS);
        click(rt, look, keys::LAYOUT_CHOOSE);
        assert!(rt.state.picker, "{look}: `{}` opens the picker", keys::LAYOUT_CHOOSE);
        click(rt, look, &keys::preset(0));
        assert_eq!(gate::preset(&rt.state), 0, "{look}: {}", keys::preset(0));
    });
}

#[test]
fn the_palette_and_the_shortcuts_open_and_close() {
    each_look(|rt, look| {
        // The title bar's own control for the palette: Shelf's search capsule, The collection's keys.
        let opener = if look == "Shelf" { "search" } else { "palette" };
        click(rt, look, opener);
        assert_eq!(rt.state.overlay, Overlay::Palette, "{look}: `{opener}` opens the palette");
        press(rt, Key::Escape, Mods::default());
        assert_eq!(rt.state.overlay, Overlay::None, "{look}: Escape closes it");
        click(rt, look, keys::NAV_SETTINGS);
        click(rt, look, keys::SHOW_SHORTCUTS);
        assert_eq!(rt.state.overlay, Overlay::Shortcuts, "{look}: `{}`", keys::SHOW_SHORTCUTS);
        click(rt, look, keys::SHORTCUTS_CLOSE);
        assert_eq!(rt.state.overlay, Overlay::None, "{look}: `{}`", keys::SHORTCUTS_CLOSE);
    });
}

#[test]
fn a_missing_key_fails_with_the_look_and_the_key() {
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| click(&mut runtime(LOOKS[0].0), LOOKS[0].1, "no-such-key")));
    let why = failed.expect_err("a missing key must fail");
    let why = why.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(why.contains(LOOKS[0].1) && why.contains("no-such-key"), "{why}");
}
