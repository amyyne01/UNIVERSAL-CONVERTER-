//! Settings, the third place (Ctrl+3): every section side by side in three columns, like the
//! liner notes on the back of a sleeve, so nothing hides behind a sub-page; Advanced opens in
//! place, as its disclosure did.
//!
//! The wording is the Electron Settings screen's (05-tabs.md 4.4); the shared `copy` stops at
//! Home and Downloads, so it is written here.
use rune::{Cursor, El, ScrollSpec, col, el, row, spacer};

use super::parts::{BODY, BODY_M, FIG, HERO, SMALL, SMALL_M, button, glyph};
use super::{AUDIO_FORMAT, Fit, OUTPUT_DIR};
use crate::copy;
use crate::directions::orbit::icons;
use crate::directions::orbit::look::{Pal, txt};

pub fn view(p: &Pal, fit: Fit) -> El {
    let head = row()
        .w_full()
        .gap(16.0)
        .items_end()
        .child(txt(copy::NAV_SETTINGS, HERO, p.text))
        .child(spacer())
        .child(txt("Machine ID 7F3A-19C2-E04B", FIG, p.muted).help("What a key binds to on this computer"));
    let columns = row()
        .w_full()
        .gap(if fit.wide { 40.0 } else { 24.0 })
        .items_start()
        .child(column().child(destination(p)).child(defaults(p)).child(more_options(p)))
        .child(column().child(schedule(p)).child(software(p)).child(closed(p, "Advanced", "Network and shortcut settings most people never touch")))
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

fn toggle(p: &Pal, on: bool) -> El {
    // The accent stays on the stage's ring: an on switch is ink, not colour.
    let knob = el().size(16.0, 16.0).radius(8.0).fill(if on { p.bg } else { p.text2 }).absolute(if on { 14.0 } else { 2.0 }, 2.0);
    el().size(32.0, 20.0).radius(10.0).fill(if on { p.text } else { p.raised }).border(1.0, if on { p.text } else { p.track }).cursor(Cursor::Pointer).focusable(true).role("switch").checked(on).child(knob)
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

fn destination(p: &Pal) -> El {
    section(p, "Destination", None).child(setting(p, "Saving to", Some(OUTPUT_DIR), button(p, "dest-change".into(), copy::REST_CHANGE, Some(&icons::FOLDER), false)))
}

fn defaults(p: &Pal) -> El {
    section(p, "Download defaults", Some("What a new download starts with. Each platform tab can still override them."))
        .child(setting(p, "Audio format", None, value(p, &AUDIO_FORMAT.to_uppercase())))
        .child(setting(p, "Audio quality", None, value(p, "320 kbps")))
        .child(setting(p, "Video quality", None, value(p, "1080p")))
        .child(setting(p, "Skip existing", Some("Don't re-download files already present on disk"), toggle(p, true)))
        .child(setting(p, "Embed thumbnail", Some("Write cover art into the audio file"), toggle(p, true)))
        .child(setting(p, "Embed metadata", Some("Write title, artist, and album tags"), toggle(p, true)))
        .child(setting(p, "Simultaneous downloads", None, value(p, "3")))
}

fn more_options(p: &Pal) -> El {
    section(p, "More download options", Some("Subtitles, chapters, sponsor segments, container and file naming"))
        .child(setting(p, "Sign in with", None, value(p, "Firefox")))
        .child(setting(p, "Import cookies.txt", Some("For sites your browser is signed in to but cannot be read directly"), small_button(p, "cookies", "Choose file")))
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
        .child(col().grow(1.0).min_w(0.0).child(txt(title.to_string(), BODY_M, p.text)).child(txt(hint.to_string(), SMALL, p.muted).wrap()))
        .child(glyph(&icons::FORWARD, p.muted))
}

fn schedule(p: &Pal) -> El {
    section(p, "Scheduled downloads", Some("Start downloads automatically at a set time"))
        .child(setting(p, "Enable scheduler", None, toggle(p, true)))
        .child(setting(p, "Start time", None, value(p, "03:00")))
        .child(setting(p, "Active days", None, segments(p, &["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"], &[1, 2, 3, 4, 5])))
        .child(setting(p, "Shutdown after queue", None, toggle(p, false)))
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
    section(p, "Software", Some("Two moving parts: the app itself, and the yt-dlp engine that does the downloading."))
        .child(status(p, "AHG Universal Converter", "v2.0.0", small_button(p, "check-app", "Check for updates")))
        .child(status(p, "Download engine", "2026.11.01", small_button(p, "check-engine", "Check for engine update")))
        .child(setting(p, "Auto-update engine", None, toggle(p, true)))
}

fn plan(p: &Pal) -> El {
    section(p, "Plan & license", None)
        .child(status(p, "Your plan", "Premium", glyph(&icons::CHECK, p.success)))
        .child(txt("Every feature is included on this machine", SMALL, p.muted).pb(8.0))
        .child(setting(p, "Deactivate license", Some("Release this machine so you can activate on another device"), small_button(p, "deactivate", "Deactivate")))
        .child(setting(p, "Move to another PC", Some("Deactivate here and release the key so it can be activated on a different computer"), small_button(p, "move", "Move")))
}

fn appearance(p: &Pal) -> El {
    let mut swatches = row().gap(8.0).center_y();
    for i in 1..=7 {
        let c = Pal::new(i, p.dark).accent;
        let dot = el().size(16.0, 16.0).radius(8.0).fill(c).cursor(Cursor::Pointer).role("radio");
        swatches = swatches.child(if i == 1 { el().size(24.0, 24.0).center().radius(12.0).border(1.0, p.text2).child(dot) } else { el().size(24.0, 24.0).center().child(dot) });
    }
    let theme = if p.dark { 1 } else { 0 };
    section(p, "Appearance & behavior", None)
        .child(setting(p, "Theme", None, segments(p, &["Light", "Dark", "System"], &[theme])))
        .child(setting(p, "Palette", None, swatches))
        .child(setting(p, "Clipboard watcher", None, toggle(p, true)))
        .child(setting(p, "Desktop notifications", None, toggle(p, true)))
        .child(setting(p, "Discord Rich Presence", None, toggle(p, false)))
}

fn help(p: &Pal) -> El {
    section(p, "Help", None)
        .child(setting(p, copy::SHORTCUTS_TITLE, None, small_button(p, "show-shortcuts", "Show shortcuts")))
        .child(setting(p, "Report a problem", None, small_button(p, "support", "Get support")))
}
