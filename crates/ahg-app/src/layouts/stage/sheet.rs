//! Settings, the third place (Ctrl+3): every section side by side in three columns, like the
//! liner notes on the back of a sleeve, so nothing hides behind a sub-page; Advanced opens in
//! place, as its disclosure did.
//!
//! Every word comes from `copy::settings` (the Electron Settings screen's); values (folder,
//! versions, times, the machine ID) are stand-ins.
use rune::{Cursor, El, ScrollSpec, col, el, row, spacer};

use super::parts::{BODY, BODY_M, FIG, HERO, SMALL, SMALL_M, button, glyph};
use super::{AUDIO_FORMAT, Fit, OUTPUT_DIR};
use crate::copy::found::{AUDIO_QUALITIES, VIDEO_QUALITIES};
use crate::copy::settings as w;
use crate::directions::orbit::icons;
use crate::directions::orbit::look::{Pal, txt};
use crate::gate::State;

pub fn view(p: &Pal, fit: Fit) -> El {
    let head = row()
        .w_full()
        .gap(16.0)
        .items_end()
        .child(txt(w::TITLE, HERO, p.text))
        .child(txt(w::SAVES, SMALL, p.muted).mb(4.0))
        .child(spacer())
        // Not in the shared copy: the old app's machine ID line, kept as a stand-in.
        .child(txt("Machine ID 7F3A-19C2-E04B", FIG, p.muted));
    let columns = row()
        .w_full()
        .gap(if fit.wide { 40.0 } else { 24.0 })
        .items_start()
        .child(column().child(destination(p)).child(defaults(p)).child(more_options(p)))
        .child(column().child(schedule(p)).child(software(p)).child(closed(p, w::ADVANCED, w::ADVANCED_HINT)))
        .child(column().child(plan(p)).child(appearance(p)).child(help(p)));
    col()
        .key("settings")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(col().w_full().px(fit.pad + 8.0).pt(fit.pad).pb(fit.pad + 8.0).gap(fit.pad).child(head).child(columns))
}

fn column() -> El {
    col().grow(1.0).min_w(0.0).gap(24.0)
}

fn section(p: &Pal, title: &str, hint: Option<&str>) -> El {
    let mut c = col().w_full().child(txt(title.to_string(), BODY_M, p.text));
    if let Some(h) = hint {
        c = c.child(txt(h.to_string(), SMALL, p.muted).wrap().mt(4.0));
    }
    c.child(el().h(8.0))
}

/// A setting: its name (and a line under it), and its control on the right.
fn setting(p: &Pal, name: &str, desc: Option<&str>, control: El) -> El {
    let mut words = col().grow(1.0).min_w(0.0).child(txt(name.to_string(), BODY, p.text));
    if let Some(d) = desc {
        words = words.child(txt(d.to_string(), SMALL, p.muted).wrap());
    }
    row().w_full().min_h(40.0).py(8.0).gap(16.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], p.line).child(words).child(control)
}

/// A setting from a (label, description) row.
fn pair(p: &Pal, (name, desc): (&str, &str), control: El) -> El {
    setting(p, name, Some(desc), control)
}

fn toggle(p: &Pal, label: &str, on: bool) -> El {
    // The accent stays on the stage's ring: an on switch is ink, not colour.
    let knob = el().size(16.0, 16.0).radius(8.0).fill(if on { p.bg } else { p.text2 }).absolute(if on { 14.0 } else { 2.0 }, 2.0);
    el().size(32.0, 20.0).radius(10.0).fill(if on { p.text } else { p.raised }).border(1.0, if on { p.text } else { p.track }).cursor(Cursor::Pointer).focusable(true).role("switch").label(label.to_string()).checked(on).child(knob)
}

/// A toggle row from a (label, description) pair.
fn switch(p: &Pal, row_words: (&str, &str), on: bool) -> El {
    pair(p, row_words, toggle(p, row_words.0, on))
}

fn value(p: &Pal, v: &str) -> El {
    row().h(28.0).px(8.0).gap(8.0).center_y().radius(8.0).border(1.0, p.track).cursor(Cursor::Pointer).role("combobox").child(txt(v.to_string(), FIG, p.text))
}

fn segments(p: &Pal, items: &[&str], on: &[usize]) -> El {
    let mut r = row().h(28.0).radius(8.0).border(1.0, p.track).clip_rounded(8.0);
    for (i, item) in items.iter().enumerate() {
        let lit = on.contains(&i);
        let mut cell = el().h_full().px(8.0).center().cursor(Cursor::Pointer).role("checkbox").checked(lit).child(txt(item.to_string(), SMALL_M, if lit { p.text } else { p.muted }));
        if i > 0 {
            cell = cell.border_sides([0.0, 0.0, 0.0, 1.0], p.line);
        }
        if lit {
            cell = cell.fill(p.raised);
        }
        r = r.child(cell);
    }
    r
}

fn small_button(p: &Pal, key: &str, label: &str) -> El {
    button(p, key.into(), label, None, false)
}

/// The old app gave the destination no heading: the row stands alone.
fn destination(p: &Pal) -> El {
    setting(p, w::SAVING_TO, Some(OUTPUT_DIR), button(p, "dest-change".into(), w::CHANGE, Some(&icons::FOLDER), false))
}

fn defaults(p: &Pal) -> El {
    section(p, w::DEFAULTS, Some(w::DEFAULTS_HINT))
        .child(setting(p, w::AUDIO_FORMAT, None, value(p, &AUDIO_FORMAT.to_uppercase())))
        .child(setting(p, w::AUDIO_QUALITY, None, value(p, AUDIO_QUALITIES[3])))
        .child(setting(p, w::VIDEO_QUALITY, None, value(p, VIDEO_QUALITIES[3])))
        .child(switch(p, w::SKIP_EXISTING, true))
        .child(switch(p, w::EMBED_THUMBNAIL, true))
        .child(switch(p, w::EMBED_METADATA, true))
        .child(pair(p, w::SIMULTANEOUS, value(p, "3")))
}

fn more_options(p: &Pal) -> El {
    section(p, w::MORE, Some(w::MORE_HINT)).child(setting(p, w::SIGN_IN_WITH, None, value(p, "Firefox"))).child(pair(p, w::IMPORT_COOKIES, small_button(p, "cookies", w::CHOOSE_FILE)))
}

fn closed(p: &Pal, title: &str, hint: &str) -> El {
    row()
        .w_full()
        .py(12.0)
        .gap(12.0)
        .center_y()
        .border_sides([1.0, 0.0, 1.0, 0.0], p.line)
        .cursor(Cursor::Pointer)
        .role("button")
        .label(title.to_string())
        .child(col().grow(1.0).min_w(0.0).child(txt(title.to_string(), BODY_M, p.text)).child(txt(hint.to_string(), SMALL, p.muted).wrap()))
        .child(glyph(&icons::FORWARD, p.muted))
}

fn schedule(p: &Pal) -> El {
    // The segments carry the days' first three letters; the full names are the copy's.
    let days: Vec<&str> = w::DAYS.iter().map(|d| &d[..3]).collect();
    section(p, w::SCHEDULE, Some(w::SCHEDULE_HINT))
        .child(setting(p, w::ENABLE_SCHEDULER, None, toggle(p, w::ENABLE_SCHEDULER, true)))
        .child(pair(p, w::START_TIME, value(p, "03:00")))
        .child(pair(p, w::ACTIVE_DAYS, segments(p, &days, &[1, 2, 3, 4, 5])))
        .child(switch(p, w::SHUTDOWN, false))
}

/// A part with a version and the one thing to do about it.
fn status(p: &Pal, name: &str, version: &str, action: El) -> El {
    row()
        .w_full()
        .min_h(48.0)
        .py(8.0)
        .gap(16.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .child(col().grow(1.0).min_w(0.0).child(txt(name.to_string(), BODY, p.text)).child(txt(version.to_string(), FIG, p.muted)))
        .child(action)
}

fn software(p: &Pal) -> El {
    section(p, w::SOFTWARE, Some(w::SOFTWARE_HINT))
        .child(status(p, w::APP_NAME, "v2.0.0", small_button(p, "check-app", w::CHECK_FOR_UPDATES)))
        .child(status(p, w::ENGINE_NAME, "2026.11.01", small_button(p, "check-engine", w::ENGINE_CHECK)))
        .child(switch(p, w::AUTO_UPDATE_ENGINE, true))
}

fn plan(p: &Pal) -> El {
    // Deactivate and Move have no button word in the copy: the row itself is the control.
    let act = |row_words: (&str, &str)| pair(p, row_words, glyph(&icons::CARET_RIGHT, p.muted)).cursor(Cursor::Pointer).role("button").label(row_words.0.to_string());
    section(p, w::PLAN, None)
        .child(status(p, w::YOUR_PLAN, w::PLAN_PREMIUM.0, glyph(&icons::CHECK, p.success)))
        .child(txt(w::PLAN_PREMIUM.1, SMALL, p.muted).pb(8.0))
        .child(act(w::DEACTIVATE))
        .child(act(w::MOVE))
}

fn appearance(p: &Pal) -> El {
    let mut swatches = row().gap(8.0).center_y().role("radiogroup").label(w::PALETTE_ARIA);
    for i in 1..=7 {
        let c = Pal::new(i, p.dark).accent;
        let dot = el().size(16.0, 16.0).radius(8.0).fill(c).cursor(Cursor::Pointer).role("radio");
        swatches = swatches.child(if i == 1 { el().size(24.0, 24.0).center().radius(12.0).border(1.0, p.text2).child(dot) } else { el().size(24.0, 24.0).center().child(dot) });
    }
    let theme = if p.dark { 1 } else { 0 };
    let layout = button(p, "set-layout".into(), w::LAYOUT_CHOOSE, None, false).on_press(|s: &mut State| s.picker = true);
    section(p, w::APPEARANCE, None)
        .child(pair(p, w::THEME, segments(p, &w::THEMES, &[theme])))
        .child(pair(p, w::PALETTE, swatches))
        .child(pair(p, w::LAYOUT, layout))
        .child(switch(p, w::CLIPBOARD_WATCHER, true))
        .child(switch(p, w::NOTIFICATIONS, true))
        .child(switch(p, w::DISCORD, false))
}

fn help(p: &Pal) -> El {
    section(p, w::HELP, None).child(pair(p, w::SHORTCUTS, small_button(p, "show-shortcuts", w::SHOW_SHORTCUTS))).child(pair(p, w::REPORT, small_button(p, "support", w::GET_SUPPORT)))
}
