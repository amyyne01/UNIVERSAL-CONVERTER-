//! The four personas: each a seeded run of real inputs with its own expectations.
use std::fs;

use ahg_engine::tier::Plan;
use ahg_engine::types::DownloadStatus as St;
use rune::Key;

use super::{Persona, Ux, hover};
use crate::gate::{Filter, Found, Nav, Overlay, PRESETS};
use crate::keys::{self, RowAct};
use crate::live::tests as lt;
use crate::{actions, copy};

pub fn play(ux: &mut Ux, persona: Persona) {
    match persona {
        Persona::Normal => normal(ux),
        Persona::Lazy => lazy(ux),
        Persona::Power => power(ux),
        Persona::Stupid => stupid(ux),
    }
}

fn media(ux: &mut Ux, secs: f64) -> bool {
    ux.wait(secs, |s| matches!(s.found, Found::Media(_)))
}

fn done(s: &crate::gate::State) -> bool {
    s.tasks.first().is_some_and(|t| t.progress.status == St::Done)
}

/// The happy path, with pauses: paste, pick a format, download, watch it finish, then Settings and the theme.
fn normal(ux: &mut Ux) {
    hover::sweep(ux, "Home");
    hover::focus(ux, "Home");
    if !ux.type_in_field(lt::LINK) {
        return;
    }
    ux.pause();
    ux.key(Key::Enter);
    let found = media(ux, 10.0);
    ux.check("Enter on a link shows what it is", found, format!("found step is {}", found_name(ux)));
    if !found {
        return;
    }
    ux.pause();
    // Audio, then MP3, where the look offers them as buttons.
    if ux.press(keys::CHOICE_AUDIO).is_some() && ux.press(&keys::choice_format("mp3")).is_some() {
        let mp3 = ux.rt.state.choice.format == "mp3" && !ux.rt.state.choice.video;
        ux.check("picking audio and MP3 takes", mp3, format!("choice is {:?}", ux.rt.state.choice.format));
    }
    ux.pause();
    let before = ux.tasks();
    ux.press_expect(keys::FOUND_DOWNLOAD, "Download starts it");
    let row = ux.wait(5.0, |s| !s.tasks.is_empty());
    ux.check("one row for one Download", row && ux.tasks() == before + 1, format!("{} rows", ux.tasks()));
    let said = ux.drawn(copy::found::TOAST_ADDED);
    ux.check("it says Added to downloads", said, "no added notice drawn");
    if !ux.go(Nav::Downloads) {
        return;
    }
    let finished = ux.wait_or_hang("the download finish", 20.0, done);
    ux.check("the download finishes", finished, "it never reached Done");
    ux.check("its row is drawn on Downloads", ux.drawn("A Song"), "no `A Song` on Downloads");
    hover::sweep(ux, "Downloads");
    ux.pause();
    if !ux.go(Nav::Settings) {
        return;
    }
    hover::sweep(ux, "Settings");
    let dark = ux.rt.state.settings.get("theme").and_then(|v| v.as_str()) != Some("light") && ux.rt.state.live.is_some();
    // Light from a dark run, Dark from a light one (the theme row is Light, Dark, System).
    let want = if dark { 0 } else { 1 };
    let before = ux.rt.state.settings.get("theme").cloned();
    ux.press_expect(&keys::theme(want), "the theme changes");
    let after = ux.rt.state.settings.get("theme").cloned();
    ux.check("the theme setting is saved", after != before, format!("theme stays {after:?}"));
}

/// Never opens Settings: pastes, presses Download once, expects the defaults to just work.
fn lazy(ux: &mut Ux) {
    ux.clip.set_text(lt::LINK);
    let mut steps = 1;
    ux.ctrl('v');
    if matches!(ux.rt.state.found, Found::Nothing) && !ux.rt.state.field.text.is_empty() {
        // The paste went into a focused field: Enter too.
        steps += 1;
        ux.key(Key::Enter);
    }
    let found = media(ux, 10.0);
    ux.check("a paste on Home looks the link up", found, format!("found step is {}", found_name(ux)));
    if !found {
        return;
    }
    steps += 1;
    ux.press_expect(keys::FOUND_DOWNLOAD, "one press on Download");
    ux.run.steps_to_first = Some(steps);
    ux.check("paste and one press reach a download", steps <= super::STEP_BUDGET, format!("{steps} steps"));
    let row = ux.wait(5.0, |s| !s.tasks.is_empty());
    ux.check("exactly one download", row && ux.tasks() == 1, format!("{} rows", ux.tasks()));
    let finished = ux.wait_or_hang("the lazy download finish", 20.0, done);
    let first_try = finished && ux.rt.state.tasks.first().is_some_and(|t| t.retry_count.is_none());
    ux.check("it works on the first try with the defaults", first_try, "not done, or retried");
    let error = ux.rt.state.notices.iter().any(|n| n.tone == crate::gate::Tone::Error);
    ux.check("no error on the way", !error, "an error notice showed");
}

/// Keyboard first: places, the palette, Tab order, a 50-link batch, filters, spam on the rows,
/// switching looks mid-download and a resize storm.
fn power(ux: &mut Ux) {
    for (c, nav) in [('2', Nav::Downloads), ('3', Nav::Settings), ('1', Nav::Home)] {
        let before = ux.look();
        ux.ctrl(c);
        let moved = ux.rt.state.nav == nav && ux.look() != before;
        ux.check(format!("Ctrl+{c} shows {nav:?}"), moved, format!("nav is {:?}", ux.rt.state.nav));
    }
    // Ctrl+K, type, Enter runs the first match.
    ux.ctrl('k');
    ux.check("Ctrl+K opens the palette", ux.rt.state.overlay == Overlay::Palette, "no palette");
    for c in "downloads".chars() {
        ux.rt.key(rune::KeyEvent { key: Key::Text(c.to_string()), mods: rune::Mods::default() });
        ux.frames(1);
    }
    ux.key(Key::Enter);
    ux.check("typing a place and Enter goes there", ux.rt.state.nav == Nav::Downloads, format!("nav is {:?}", ux.rt.state.nav));
    ux.escape_out("the palette");
    ux.ctrl('1');
    // Ctrl+K a hundred times: it ends where an even count leaves it.
    for _ in 0..100 {
        ux.ctrl('k');
    }
    ux.check("Ctrl+K x100 ends closed", ux.rt.state.overlay == Overlay::None, format!("overlay {:?}", ux.rt.state.overlay));
    ux.escape_out("Ctrl+K x100");
    hover::focus(ux, "Home");
    // Tab mashing never lands on something hidden.
    let mut hidden = Vec::new();
    for _ in 0..60 {
        ux.rt.key(rune::KeyEvent { key: Key::Tab, mods: rune::Mods::default() });
        ux.frames(1);
        if let Some(e) = ux.shown().into_iter().find(|e| e.focused)
            && (e.inert || e.visible[2] < 1.0 || e.visible[3] < 1.0)
        {
            hidden.push(e.key.clone().unwrap_or(e.path.clone()));
        }
    }
    ux.part("Tab x60 stays on visible controls", 60 - hidden.len().min(60), 60, format!("focus on hidden {hidden:?}"));
    ux.rt.update(|_, tx| tx.blur());
    ux.settle();

    // A Premium user pastes 50 links at once.
    ux.rt.state.plan = Plan::Premium;
    let links: Vec<String> = (0..50)
        .map(|i| match i % 3 {
            0 => format!("https://www.youtube.com/watch?v=ux{i:09}"),
            1 => format!("https://soundcloud.com/someone/track-{i}"),
            _ => format!("https://vimeo.com/{}", 100_000 + i),
        })
        .collect();
    if !ux.type_in_field(&links.join("\n")) {
        return;
    }
    ux.key(Key::Enter);
    let all = ux.wait(20.0, |s| s.tasks.len() >= 50);
    ux.check("a 50-link batch queues 50 downloads", all && ux.tasks() == 50, format!("{} rows", ux.tasks()));
    ux.check("the batch shows Downloads", ux.rt.state.nav == Nav::Downloads, format!("nav {:?}", ux.rt.state.nav));
    if ux.tasks() == 0 {
        return;
    }
    if ux.rt.state.nav != Nav::Downloads {
        ux.go(Nav::Downloads);
    }
    let mut chips = 0;
    for f in Filter::ALL {
        if ux.press(&keys::filter(f)).is_some() && ux.rt.state.filter == f {
            chips += 1;
        }
    }
    ux.part("every filter chip filters", chips, Filter::ALL.len(), "a chip is missing or did nothing");
    ux.press(&keys::filter(Filter::All));

    // Pause and resume spam on the first row, then cancel and retry spam.
    let rows = ux.tasks();
    let id = ux.rt.state.tasks[0].task_id.clone();
    let mut spammed = 0;
    for i in 0..20 {
        let act = if i % 2 == 0 { RowAct::Pause } else { RowAct::Resume };
        if ux.mash(&keys::row_action(act, &id), 1) {
            spammed += 1;
        }
    }
    ux.wait(1.0, |_| false);
    ux.check("pause and resume spam keeps every row", ux.tasks() == rows, format!("{rows} rows became {}", ux.tasks()));
    ux.check("the row has Pause or Resume", spammed > 0, "no pause or resume control on the first row");
    if ux.mash(&keys::row_action(RowAct::Cancel, &id), 3) {
        ux.wait(2.0, |s| s.tasks.iter().any(|t| t.progress.status == St::Cancelled));
        let cancelled = ux.rt.state.tasks.iter().find(|t| t.progress.status == St::Cancelled).map(|t| t.task_id.clone());
        if let Some(gone) = cancelled {
            let before = ux.tasks();
            ux.mash(&keys::row_action(RowAct::Retry, &gone), 10);
            ux.wait(3.0, |_| false);
            if ux.tasks() > before {
                ux.fault(format!("duplicate task: Retry x10 on one row made {} rows of {before}", ux.tasks()));
            }
            ux.check("Retry x10 retries once", ux.tasks() == before, format!("{before} rows became {}", ux.tasks()));
        }
    }
    let before = ux.tasks();
    ux.mash(keys::PAUSE_ALL, 10);
    ux.check("Pause all x10 keeps every row", ux.tasks() == before, format!("{before} rows became {}", ux.tasks()));

    // Switching looks, themes and palettes mid-download.
    let rows = ux.tasks();
    let me = ux.preset;
    for k in 0..24 {
        let to = (me + 1 + k) % PRESETS.len();
        if k % 4 == 0 {
            ux.ctrl('.');
            ux.press(&keys::preset(to));
            ux.escape_out("the picker");
        } else {
            ux.rt.update(move |s, tx| actions::set_preset(s, to, tx));
        }
        let dark = k % 2 == 0;
        ux.rt.set_system_dark(dark);
        ux.rt.update(move |s, tx| actions::set_palette(s, k % 8, tx));
        ux.frames(2);
    }
    ux.rt.update(move |s, tx| {
        actions::set_preset(s, me, tx);
        actions::set_palette(s, 0, tx);
    });
    ux.settle();
    ux.check("switching looks x24 mid-download keeps every row", ux.tasks() == rows, format!("{rows} rows became {}", ux.tasks()));
    ux.escape_out("the look switching");

    // A resize storm, then back.
    for k in 0..40 {
        let (w, h) = [(960.0, 640.0), (1920.0, 1080.0), (1001.0, 703.0), (1600.0, 650.0), (980.0, 1200.0)][k % 5];
        ux.rt.set_viewport((w, h), 1.0);
        ux.frames(1);
    }
    ux.rt.set_viewport((1240.0, 820.0), 1.0);
    ux.settle();
    ux.ctrl('1');
    let field = ux.find(keys::FIELD).is_some_and(|(x, y)| (0.0..1240.0).contains(&x) && (0.0..820.0).contains(&y));
    ux.check("after a resize storm the field is on screen", field, "no field inside the window");
    ux.rt.update(|s, _| actions::cancel_all(s));
    ux.settle();
}

/// Garbage, wrong links, locked and disabled controls, wrong keys, double clicks, overlay
/// storms, empty drags and a look switch during a lookup. Expects a reason and a fix, never
/// silence, never a duplicate task, never a stuck overlay.
fn stupid(ux: &mut Ux) {
    let garbage: [(&str, String); 8] = [
        ("empty text", String::new()),
        ("spaces", "    ".into()),
        ("one letter", "a".into()),
        ("100 KB of text", "lorem ipsum dolor ".repeat(100 * 1024 / 18)),
        ("emoji", "🎵🔥💀🤡".into()),
        ("garbage", "asdf;;qwer!!//".into()),
        ("a site we do not support", "http://notasite/watch/123".into()),
        ("a refused link", "file:///C:/Windows/win.ini".into()),
    ];
    for (what, text) in &garbage {
        let rows = ux.tasks();
        if !ux.type_in_field(text) {
            return;
        }
        ux.key(Key::Enter);
        // A search or lookup gets a moment to answer.
        ux.wait(3.0, |s| !matches!(s.found, Found::LookingUp) && !search_loading(s));
        let searched = matches!(ux.rt.state.found, Found::Search { .. });
        if !searched {
            ux.expect_message(&format!("{what} says why and what to do"));
        } else {
            ux.check(format!("{what} searches without breaking"), true, "");
        }
        ux.check(format!("{what} queues nothing by itself"), ux.tasks() == rows, format!("{rows} rows became {}", ux.tasks()));
        ux.escape_out(what);
        ux.rt.update(|s, _| actions::dismiss_notice(s));
    }

    // The same link twice in one paste.
    let rows = ux.tasks();
    ux.type_in_field(&format!("{} {}", lt::LINK, lt::LINK));
    ux.key(Key::Enter);
    ux.wait(3.0, |s| s.tasks.len() > rows);
    ux.wait(0.5, |_| false);
    let made = ux.tasks() - rows;
    if made > 1 {
        ux.fault(format!("duplicate task: the same link twice in one paste made {made}"));
    }
    ux.check("the same link twice in one paste queues it once", made == 1, format!("{made} new rows"));
    ux.ctrl('1');

    // 50 presses on Download: exactly one task.
    ux.type_in_field(lt::LINK);
    ux.key(Key::Enter);
    if media(ux, 10.0) {
        let rows = ux.tasks();
        ux.mash(keys::FOUND_DOWNLOAD, 50);
        ux.wait(3.0, |s| s.tasks.len() > rows);
        ux.wait(0.5, |_| false);
        let made = ux.tasks() - rows;
        if made > 1 {
            ux.fault(format!("duplicate task: Download x50 made {made}"));
        }
        ux.check("Download x50 adds exactly one", made == 1, format!("{made} rows"));
    } else {
        ux.check("a link shows what it is", false, format!("found step is {}", found_name(ux)));
    }
    ux.rt.update(|s, _| actions::dismiss_notice(s));
    ux.ctrl('1');

    // A locked control on Basic, then a disabled Download (nothing ticked).
    ux.rt.state.plan = Plan::Basic;
    ux.type_in_field(lt::LINK);
    ux.key(Key::Enter);
    if media(ux, 10.0) {
        ux.press(keys::CHOICE_AUDIO);
        if ux.press(&keys::choice_format("flac")).is_some() {
            ux.expect_message("a locked format says it is Premium");
            ux.check("a locked format stays unchosen", ux.rt.state.choice.format != "flac", "FLAC was taken on Basic");
        }
    }
    ux.escape_out("the locked format");
    ux.rt.update(|s, _| {
        actions::dismiss_notice(s);
        if let Some((_, playlist)) = crate::gate::SCENES.iter().find(|(n, _)| *n == "playlist") {
            playlist(s);
        }
        actions::deselect_all_tracks(s);
    });
    ux.settle();
    let rows = ux.tasks();
    if let Some(changed) = ux.press(keys::FOUND_DOWNLOAD) {
        let inert = ux.shown().iter().any(|e| e.key.as_deref() == Some(keys::FOUND_DOWNLOAD) && e.inert);
        if !changed && !inert {
            ux.run.dead_clicks += 1;
        }
        ux.check("Download with nothing ticked says so or is disabled", changed || inert, "a dead click");
        ux.check("Download with nothing ticked adds nothing", ux.tasks() == rows, "rows were added");
    }
    ux.escape_out("the empty playlist");

    // Keys in the wrong places.
    for nav in [Nav::Downloads, Nav::Settings, Nav::Home] {
        ux.rt.update(move |s, _| actions::go(s, nav));
        ux.settle();
        // Nothing focused: a key on a focused button is that button's, not a stray one.
        ux.rt.update(|_, tx| tx.blur());
        ux.wait(0.5, |_| false);
        let mut changed = Vec::new();
        for key in [Key::Enter, Key::Text(" ".into()), Key::Delete, Key::Backspace, Key::Text("q".into()), Key::PageDown, Key::Up] {
            let rows = ux.tasks();
            let name = format!("{key:?}");
            ux.key(key);
            if ux.tasks() != rows {
                changed.push(format!("{name}: {rows} -> {}", ux.tasks()));
            }
        }
        ux.ctrl('a');
        ux.check(format!("stray keys on {nav:?} change no rows"), changed.is_empty(), format!("{changed:?}"));
        ux.escape_out(&format!("stray keys on {nav:?}"));
    }

    // Double and triple clicks on a seeded dozen of the controls of Downloads.
    ux.rt.update(|s, _| actions::go(s, Nav::Downloads));
    ux.settle();
    let controls = hover::controls(ux);
    let rows = ux.tasks();
    for _ in 0..12.min(controls.len()) {
        let pick = controls[ux.rand(controls.len())].clone();
        if pick.0.as_deref().is_some_and(|k| k.starts_with("nav-") || k == keys::LAYOUT_CHOOSE) {
            continue;
        }
        let (x, y) = pick.1;
        ux.rt.pointer_move(x, y);
        for _ in 0..2 + ux.rand(2) {
            ux.rt.pointer_down(x, y);
            ux.rt.pointer_up(x, y);
        }
        ux.settle();
        ux.escape_out("a double click");
        ux.rt.update(|s, _| actions::go(s, Nav::Downloads));
        ux.settle();
    }
    ux.check("double and triple clicks add no rows", ux.tasks() <= rows, format!("{rows} rows became {}", ux.tasks()));

    // An overlay closed and opened 200 times.
    for _ in 0..200 {
        ux.rt.key(rune::KeyEvent { key: Key::Command('k'), mods: rune::Mods { cmd: true, ctrl: true, ..rune::Mods::default() } });
        ux.rt.frame(1.0 / 60.0);
        ux.rt.key(rune::KeyEvent { key: Key::Escape, mods: rune::Mods::default() });
        ux.rt.frame(1.0 / 60.0);
    }
    ux.settle();
    ux.check("the palette opened and closed x200 ends closed", ux.rt.state.overlay == Overlay::None, "still open");
    ux.escape_out("the palette x200");
    for _ in 0..30 {
        ux.rt.key(rune::KeyEvent { key: Key::Escape, mods: rune::Mods::default() });
        ux.rt.frame(1.0 / 60.0);
    }
    ux.settle();

    // Dragging nothing onto nothing.
    ux.rt.drag_files(Vec::new(), Some((600.0, 400.0)));
    ux.frames(2);
    ux.rt.drop_files(Vec::new(), Some((600.0, 400.0)));
    ux.rt.pointer_move(3.0, 700.0);
    ux.rt.pointer_down(3.0, 700.0);
    ux.rt.pointer_move(400.0, 760.0);
    ux.rt.pointer_up(400.0, 760.0);
    ux.settle();
    ux.escape_out("an empty drag");

    // A look switch while a lookup runs: the answer still lands in the new look.
    fs::write(ux.dir.join("json.txt"), format!("sleep 1200\nout {}\nexit 0\n", lt::RECORD)).unwrap();
    ux.ctrl('1');
    ux.type_in_field(lt::LINK);
    ux.key(Key::Enter);
    let other = (ux.preset + 1) % PRESETS.len();
    ux.rt.update(move |s, tx| actions::set_preset(s, other, tx));
    let landed = media(ux, 10.0) && ux.find(keys::FOUND).is_some();
    ux.check("a lookup during a look switch lands in the new look", landed, format!("found step is {}", found_name(ux)));
    let me = ux.preset;
    ux.rt.update(move |s, tx| actions::set_preset(s, me, tx));
    ux.escape_out("the switch during a lookup");
}

fn search_loading(s: &crate::gate::State) -> bool {
    matches!(&s.found, Found::Search { bands, .. } if bands.iter().any(|b| matches!(b, crate::gate::Band::Loading)))
}

fn found_name(ux: &Ux) -> &'static str {
    match ux.rt.state.found {
        Found::Nothing => "nothing",
        Found::LookingUp => "still looking up",
        Found::LookupFailed(_) => "failed",
        Found::Search { .. } => "a search",
        Found::Playlist { .. } => "a playlist",
        Found::Media(_) => "media",
    }
}
