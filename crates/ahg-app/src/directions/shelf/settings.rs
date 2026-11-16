//! Settings, the third place (Ctrl+3): one ruled column read top to bottom,
//! like the index card in a record box. Each section's name and hint stand in
//! a left margin under a numbered spine label; its settings run beside them,
//! one per line, the control at the right. Where files go leads, on its own.
//! Both of the old app's disclosures are drawn open. Wording is
//! `copy::settings`, word for word; values are stand-ins (the gate has no settings).
use rune::{Color, El, Icon, ScrollSpec, col, el, icon, row, text};

use super::icons;
use super::parts::ghost;
use super::theme::{self, Pal, mono, sans, serif};
use crate::copy::settings as w;
use crate::gate::{self, State};

/// Stand-in values.
const FOLDER: &str = "C:\\Users\\Alex\\Music";
const VERSION: &str = "v2.0.0";
const ENGINE_VERSION: &str = "2026.11.01";
/// Palette names for the swatches' accessible names: Shelf's own, then the app's seven.
const PALETTES: [&str; 8] = ["Shelf", "Aurora", "Slate", "Moss", "Ember", "Paper", "Lagoon", "Orchid"];

/// The margin that holds each section's name.
const MARGIN: f32 = 248.0;

pub fn view(s: &State, p: &Pal) -> El {
    let head = row()
        .w_full()
        .gap(16.0)
        .items_end()
        .child(text(w::TITLE).font(serif(44.0).lh(48.0)).color(p.text))
        .child(el().grow(1.0))
        .child(text(w::SAVES).font(sans(13.0, 400.0)).color(p.muted).mb(4.0));
    let page = col()
        .w_full()
        .max_w(1040.0)
        .self_center()
        .px(40.0)
        .pt(24.0)
        .pb(48.0)
        .child(head)
        .child(destination(p).mt(24.0).mb(8.0))
        .child(section(p, 1, w::DEFAULTS, Some(w::DEFAULTS_HINT), defaults(p)))
        .child(section(p, 2, w::MORE, Some(w::MORE_HINT), more(p)))
        .child(section(p, 3, w::APPEARANCE, None, appearance(s, p)))
        .child(section(p, 4, w::SCHEDULE, Some(w::SCHEDULE_HINT), schedule(p)))
        .child(section(p, 5, w::SOFTWARE, Some(w::SOFTWARE_HINT), software(p)))
        .child(section(p, 6, w::ADVANCED, Some(w::ADVANCED_HINT), advanced(p)))
        .child(section(p, 7, w::PLAN, None, plan(p)))
        .child(section(p, 8, w::HELP, None, help(p)));
    col().key("settings").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(page)
}

// ── Structure ───────────────────────────────────────────────────────────────

/// A section: a rule across, the name in the margin under its spine label, the settings beside it.
fn section(p: &Pal, n: usize, title: &str, hint: Option<&str>, rows: El) -> El {
    let tag = el()
        .h(20.0)
        .px(4.0)
        .center()
        .radius(2.0)
        .fill(theme::label_paper())
        .border(1.0, theme::label_ink().alpha(0.18))
        .child(text(format!("{n:02}")).font(mono(11.0, 500.0)).color(theme::label_ink()));
    let mut name = col()
        .w(MARGIN)
        .pr(32.0)
        .gap(8.0)
        .child(row().child(tag))
        .child(text(title.to_string()).font(sans(15.0, 500.0)).color(p.text).wrap());
    if let Some(h) = hint {
        name = name.child(text(h.to_string()).font(sans(12.0, 400.0)).color(p.muted).wrap());
    }
    row().w_full().pt(24.0).pb(16.0).items_start().border_sides([1.0, 0.0, 0.0, 0.0], p.line).child(name).child(rows.grow(1.0).min_w(0.0))
}

/// The settings of a section, a hairline between each.
fn lines(items: Vec<El>, p: &Pal) -> El {
    let mut c = col();
    for (i, it) in items.into_iter().enumerate() {
        c = c.child(if i == 0 { it } else { it.border_sides([1.0, 0.0, 0.0, 0.0], p.line_soft) });
    }
    c
}

/// A setting: its name and description, the control at the right.
fn setting(p: &Pal, (label, desc): (&str, &str), control: El) -> El {
    let mut words = col().grow(1.0).min_w(0.0).gap(4.0).child(text(label.to_string()).font(sans(13.0, 500.0)).color(p.text));
    if !desc.is_empty() {
        words = words.child(text(desc.to_string()).font(sans(12.0, 400.0)).color(p.text2).wrap());
    }
    row().w_full().min_h(56.0).py(12.0).gap(24.0).center_y().child(words).child(control)
}

// ── Controls ────────────────────────────────────────────────────────────────

fn toggle(p: &Pal, label: &str, on: bool) -> El {
    el().size(36.0, 20.0)
        .radius(10.0)
        .fill(if on { p.accent } else { p.control.alpha(0.6) })
        .focusable(true)
        .role("switch")
        .label(label.to_string())
        .checked(on)
        .child(el().size(12.0, 12.0).radius(6.0).fill(if on { p.bg } else { p.card }).absolute(if on { 20.0 } else { 4.0 }, 4.0))
}

/// A select showing its value, with a caret.
fn select(p: &Pal, label: &str, value: &str, width: f32) -> El {
    row()
        .w(width)
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.card)
        .border(1.0, p.control.alpha(0.55))
        .hover(p.raised)
        .focusable(true)
        .role("combobox")
        .label(label.to_string())
        .child(text(value.to_string()).font(sans(13.0, 400.0)).color(p.text).single_line().truncate().grow(1.0))
        .child(icon(&icons::CARET_DOWN, 16.0).color(p.muted))
}

/// A typed value in the figures face, or its placeholder, muted.
fn input(p: &Pal, label: &str, value: &str, placeholder: &str) -> El {
    let (shown, ink) = if value.is_empty() { (placeholder, p.muted) } else { (value, p.text) };
    row()
        .w(200.0)
        .h(32.0)
        .px(12.0)
        .center_y()
        .radius(8.0)
        .fill(p.card)
        .border(1.0, p.control.alpha(0.55))
        .focusable(true)
        .role("textbox")
        .label(label.to_string())
        .child(text(shown.to_string()).font(mono(12.0, 400.0)).color(ink).single_line().truncate())
}

fn button(p: &Pal, key: &str, glyph: Option<&'static Icon>, label: &str) -> El {
    ghost(p, key, glyph, label, p.text)
}

// ── Sections ────────────────────────────────────────────────────────────────

/// Where files go: no heading in the old app, so it leads the page on its own.
fn destination(p: &Pal) -> El {
    let (parent, leaf) = FOLDER.rsplit_once('\\').unwrap_or(("", FOLDER));
    row()
        .key("destination")
        .w_full()
        .h(64.0)
        .pl(16.0)
        .pr(16.0)
        .gap(16.0)
        .center_y()
        .radius(12.0)
        .fill(p.card)
        .border(1.0, p.line_soft)
        .child(icon(&icons::FOLDER, 16.0).color(p.text2))
        .child(text(w::SAVING_TO).font(sans(13.0, 400.0)).color(p.text2))
        .child(
            row()
                .grow(1.0)
                .min_w(0.0)
                .child(text(format!("{parent}\\")).font(mono(13.0, 400.0)).color(p.muted).single_line())
                .child(text(leaf.to_string()).font(mono(13.0, 500.0)).color(p.text).single_line()),
        )
        .child(button(p, "change-folder", None, w::CHANGE))
}

fn defaults(p: &Pal) -> El {
    let labelled = |label: &str, value: &str| {
        col()
            .grow(1.0)
            .min_w(0.0)
            .gap(8.0)
            .child(text(label.to_string()).font(sans(12.0, 400.0)).color(p.text2))
            .child(select(p, label, value, 160.0).w_full())
    };
    let trio = row()
        .w_full()
        .gap(16.0)
        .pb(16.0)
        .child(labelled(w::AUDIO_FORMAT, "MP3"))
        .child(labelled(w::AUDIO_QUALITY, "320 kbps"))
        .child(labelled(w::VIDEO_QUALITY, "1080p Full HD"));
    lines(
        vec![
            trio,
            setting(p, w::SKIP_EXISTING, toggle(p, w::SKIP_EXISTING.0, true)),
            setting(p, w::EMBED_THUMBNAIL, toggle(p, w::EMBED_THUMBNAIL.0, true)),
            setting(p, w::EMBED_METADATA, toggle(p, w::EMBED_METADATA.0, true)),
            setting(p, w::SIMULTANEOUS, select(p, w::SIMULTANEOUS.0, "3 at a time", 160.0)),
        ],
        p,
    )
}

fn more(p: &Pal) -> El {
    lines(
        vec![
            setting(p, (w::SIGN_IN_WITH, ""), select(p, w::SIGN_IN_WITH, "Chrome", 200.0)),
            setting(p, w::IMPORT_COOKIES, button(p, "import-cookies", Some(&icons::FOLDER), w::CHOOSE_FILE)),
            setting(p, (w::SUBTITLES, ""), select(p, w::SUBTITLES, "Off", 200.0)),
            setting(p, (w::SUBTITLE_LANGUAGES, ""), input(p, w::SUBTITLE_LANGUAGES, "", w::SUBTITLE_LANGUAGES_PLACEHOLDER)),
            setting(p, w::AUTO_CAPTIONS, toggle(p, w::AUTO_CAPTIONS.0, false)),
            setting(p, (w::SPONSOR_SEGMENTS, ""), select(p, w::SPONSOR_SEGMENTS, "Off", 200.0)),
            setting(p, (w::VIDEO_CONTAINER, ""), select(p, w::VIDEO_CONTAINER, "MP4 (most compatible)", 200.0)),
            setting(p, (w::VIDEO_CODEC, ""), select(p, w::VIDEO_CODEC, "Best available", 200.0)),
            setting(p, w::EMBED_CHAPTERS, toggle(p, w::EMBED_CHAPTERS.0, true)),
            setting(p, w::SPLIT_BY_CHAPTER, toggle(p, w::SPLIT_BY_CHAPTER.0, false)),
            setting(p, w::SAVE_COVER_ART, toggle(p, w::SAVE_COVER_ART.0, false)),
            setting(p, w::FILE_NAMING, input(p, w::FILE_NAMING.0, "", w::FILE_NAMING_PLACEHOLDER)),
        ],
        p,
    )
}

fn appearance(s: &State, p: &Pal) -> El {
    // The gate follows the system (Rune's --theme), so System is the one chosen.
    let mut theme =
        row().h(32.0).padding(4.0).gap(4.0).radius(8.0).fill(p.sunk).border(1.0, p.line_soft).role("radiogroup").label(w::THEME.0);
    for (i, name) in w::THEMES.into_iter().enumerate() {
        let on = i == 2;
        theme = theme.child(
            el().h(24.0)
                .px(12.0)
                .center()
                .radius(4.0)
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .focusable(true)
                .role("radio")
                .checked(on)
                .child(text(name).font(sans(13.0, if on { 500.0 } else { 400.0 })).color(if on { p.text } else { p.text2 })),
        );
    }
    let mut swatches = row().gap(8.0).role("radiogroup").label(w::PALETTE_ARIA);
    for (i, name) in PALETTES.into_iter().enumerate() {
        let sw = Pal::new(i, p.dark);
        let on = i == s.palette;
        swatches = swatches.child(
            el().size(24.0, 24.0)
                .radius(12.0)
                .center()
                .fill(sw.bg)
                .border(1.0, sw.line)
                .outline(if on { 2.0 } else { 0.0 }, 2.0, p.text)
                .focusable(true)
                .role("radio")
                .checked(on)
                .label(name)
                .help(name)
                .child(el().size(8.0, 8.0).radius(4.0).fill(sw.accent)),
        );
    }
    let preset = gate::PRESETS[gate::preset(s)].0;
    let layout = row()
        .gap(12.0)
        .center_y()
        .child(text(preset).font(sans(13.0, 400.0)).color(p.text2))
        .child(button(p, "layout-choose", None, w::LAYOUT_CHOOSE).on_press(|s: &mut State| s.picker = true));
    lines(
        vec![
            setting(p, w::THEME, theme),
            setting(p, w::PALETTE, swatches),
            setting(p, w::LAYOUT, layout),
            setting(p, w::CLIPBOARD_WATCHER, toggle(p, w::CLIPBOARD_WATCHER.0, true)),
            setting(p, w::NOTIFICATIONS, toggle(p, w::NOTIFICATIONS.0, true)),
            setting(p, w::DISCORD, toggle(p, w::DISCORD.0, false)),
        ],
        p,
    )
}

fn schedule(p: &Pal) -> El {
    let mut days = row().gap(4.0).role("group").label(w::ACTIVE_DAYS.0);
    for (i, day) in w::DAYS.iter().enumerate() {
        // Weekdays on (Electron's order: Sunday first).
        let on = (1..=5).contains(&i);
        days = days.child(
            el().size(32.0, 28.0)
                .center()
                .radius(8.0)
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .border(1.0, if on { p.control } else { p.line })
                .focusable(true)
                .role("checkbox")
                .checked(on)
                .label(*day)
                .child(text(&day[..2]).font(mono(12.0, 500.0)).color(if on { p.text } else { p.muted })),
        );
    }
    lines(
        vec![
            setting(p, (w::ENABLE_SCHEDULER, ""), toggle(p, w::ENABLE_SCHEDULER, true)),
            setting(p, w::START_TIME, input(p, w::START_TIME.0, "03:00", "").w(96.0)),
            setting(p, w::ACTIVE_DAYS, days),
            setting(p, w::SHUTDOWN, toggle(p, w::SHUTDOWN.0, false)),
        ],
        p,
    )
}

/// An updater: its name and version, what it is doing, its one button.
fn updater(p: &Pal, name: &str, version: &str, desc: &str, action: El) -> El {
    let mut words = col().grow(1.0).min_w(0.0).gap(4.0).child(
        row()
            .gap(8.0)
            .center_y()
            .child(text(name.to_string()).font(sans(13.0, 500.0)).color(p.text))
            .child(text(version.to_string()).font(mono(12.0, 400.0)).color(p.muted)),
    );
    if !desc.is_empty() {
        words = words.child(text(desc.to_string()).font(sans(12.0, 400.0)).color(p.text2).wrap());
    }
    row().w_full().min_h(56.0).py(12.0).gap(24.0).center_y().child(words).child(action)
}

fn software(p: &Pal) -> El {
    lines(
        vec![
            updater(p, w::APP_NAME, VERSION, w::APP_REST, button(p, "check-updates", Some(&icons::REFRESH), w::CHECK_FOR_UPDATES)),
            updater(p, w::ENGINE_NAME, ENGINE_VERSION, "", button(p, "check-engine", Some(&icons::REFRESH), w::ENGINE_CHECK)),
            setting(p, w::AUTO_UPDATE_ENGINE, toggle(p, w::AUTO_UPDATE_ENGINE.0, true)),
        ],
        p,
    )
}

fn advanced(p: &Pal) -> El {
    lines(
        vec![
            setting(p, w::RATE_LIMIT, input(p, w::RATE_LIMIT.0, "", w::RATE_LIMIT_PLACEHOLDER)),
            setting(p, w::PROXY, input(p, w::PROXY.0, "", w::PROXY_PLACEHOLDER)),
            setting(p, w::GLOBAL_HOTKEY, input(p, w::GLOBAL_HOTKEY.0, "Ctrl+Shift+D", w::GLOBAL_HOTKEY_PLACEHOLDER)),
        ],
        p,
    )
}

fn plan(p: &Pal) -> El {
    // The plan, stamped on the sleeves' paper.
    let stamp = el()
        .h(24.0)
        .px(8.0)
        .center()
        .radius(4.0)
        .fill(theme::label_paper())
        .border(1.0, theme::label_ink().alpha(0.18))
        .child(text(w::PLAN_PREMIUM.0).font(sans(13.0, 500.0)).color(theme::label_ink()));
    let danger = |key: &str, (label, desc): (&str, &str)| {
        row()
            .key(key)
            .w_full()
            .min_h(56.0)
            .py(12.0)
            .gap(24.0)
            .center_y()
            .hover(p.soft(p.error))
            .focusable(true)
            .role("button")
            .label(label.to_string())
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(4.0)
                    .child(text(label.to_string()).font(sans(13.0, 500.0)).color(p.error))
                    .child(text(desc.to_string()).font(sans(12.0, 400.0)).color(p.text2).wrap()),
            )
            .child(icon(&icons::CARET_RIGHT, 16.0).color(p.muted))
    };
    lines(vec![setting(p, (w::YOUR_PLAN, w::PLAN_PREMIUM.1), stamp), danger("deactivate", w::DEACTIVATE), danger("move-pc", w::MOVE)], p)
}

fn help(p: &Pal) -> El {
    lines(
        vec![
            setting(p, w::SHORTCUTS, button(p, "show-shortcuts", None, w::SHOW_SHORTCUTS)),
            setting(p, w::REPORT, button(p, "get-support", Some(&icons::EXTERNAL), w::GET_SUPPORT)),
        ],
        p,
    )
}
