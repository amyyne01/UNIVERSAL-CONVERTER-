//! Settings, the third place: everything that is not a download, in two
//! columns (left, what a download produces; right, the app, its engine and the
//! licence), as Electron assigned them. Wording is `copy::settings`, word for
//! word; values are read from `State.settings`.
use rune::{Color, El, Icon, ScrollSpec, col, el, icon, row, text};

use crate::copy::settings as w;
use crate::directions::shelf::parts::{app_version, on, raw, rows, shown};
use crate::directions::shelf::theme::{Pal, mono, sans, serif};
use crate::gate::State;
use crate::settings_spec::Row;
use crate::{icons, palette};

#[allow(dead_code)] // unused until cleanup (step 19): `icons::CARET_DOWN` replaces it
static CARET_DOWN: Icon = Icon {
    name: "collection-caret-down",
    view: 256.0,
    body: r#"<path d="M213.66,101.66l-80,80a8,8,0,0,1-11.32,0l-80-80A8,8,0,0,1,53.66,90.34L128,164.69l74.34-74.35a8,8,0,0,1,11.32,11.32Z" fill="currentColor"/>"#,
};
#[allow(dead_code)] // unused until cleanup (step 19): `icons::CARET_RIGHT` replaces it
pub(super) static CARET_RIGHT: Icon = Icon {
    name: "collection-caret-right",
    view: 256.0,
    body: r#"<path d="M181.66,133.66l-80,80a8,8,0,0,1-11.32-11.32L164.69,128,90.34,53.66a8,8,0,0,1,11.32-11.32l80,80A8,8,0,0,1,181.66,133.66Z" fill="currentColor"/>"#,
};

pub fn view(s: &State, p: &Pal) -> El {
    let rows = rows(s);
    let rows = rows.as_slice();
    let left = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(16.0)
        .child(destination(rows, p))
        .child(defaults(rows, p))
        .child(more(rows, p))
        .child(appearance(s, rows, p));
    let right = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(16.0)
        .child(schedule(rows, p))
        .child(software(s, rows, p))
        .child(plan(p))
        .child(card(p).child(folded(p, w::ADVANCED, w::ADVANCED_HINT, false)))
        .child(help(p));
    let page = col()
        .w_full()
        .px(32.0)
        .pt(24.0)
        .pb(48.0)
        .child(
            row()
                .w_full()
                .gap(16.0)
                .items_end()
                .mb(24.0)
                .child(text(w::TITLE).font(serif(44.0).lh(48.0)).color(p.text))
                .child(text(w::SAVES).font(sans(13.0, 400.0)).color(p.text2).mb(8.0)),
        )
        .child(row().w_full().gap(16.0).items_start().child(left).child(right))
        .child(
            row()
                .mt(24.0)
                .gap(8.0)
                .help(w::MACHINE_ID_TITLE)
                .child(text(w::MACHINE_ID).font(sans(12.0, 400.0)).color(p.muted))
                .child(text(s.machine_id.clone()).font(mono(12.0, 400.0)).color(p.muted)),
        );
    col().key("settings-place").grow(1.0).h_full().min_w(0.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(page)
}

/// A section's paper: a card on the page.
fn card(p: &Pal) -> El {
    col().w_full().px(20.0).py(16.0).gap(8.0).radius(12.0).fill(p.card).border(1.0, p.line_soft)
}

fn section(p: &Pal, title: &str, hint: Option<&str>) -> El {
    let mut c = col().w_full().gap(4.0).mb(8.0).child(text(title.to_string()).font(sans(16.0, 500.0)).color(p.text));
    if let Some(h) = hint {
        c = c.child(text(h.to_string()).font(sans(12.0, 400.0)).color(p.text2).wrap());
    }
    card(p).child(c)
}

/// A setting: its name and description, the control on the right.
fn setting(p: &Pal, label: &str, desc: &str, control: El) -> El {
    let mut words = col().grow(1.0).min_w(0.0).gap(4.0).child(text(label.to_string()).font(sans(13.0, 500.0)).color(p.text));
    if !desc.is_empty() {
        words = words.child(text(desc.to_string()).font(sans(12.0, 400.0)).color(p.text2).wrap());
    }
    row().w_full().gap(16.0).py(4.0).center_y().child(words).child(control)
}

fn toggle(p: &Pal, label: &str, on: bool) -> El {
    el().size(32.0, 20.0)
        .radius(10.0)
        .fill(if on { p.accent } else { p.control.alpha(0.6) })
        .focusable(true)
        .role("switch")
        .label(label.to_string())
        .checked(on)
        .child(el().size(16.0, 16.0).radius(8.0).fill(if on { p.bg } else { p.card }).absolute(if on { 14.0 } else { 2.0 }, 2.0))
}

/// A bordered field showing its value (or its placeholder, muted): a select with a caret, or a typed value.
fn field(p: &Pal, label: &str, value: &str, select: bool, placeholder: bool) -> El {
    let mut f = row()
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .border(1.0, p.line)
        .hover(p.raised)
        .focusable(true)
        .role(if select { "combobox" } else { "textbox" })
        .label(label.to_string())
        .child(
            text(value.to_string())
                .font(if select { sans(13.0, 400.0) } else { mono(13.0, 400.0) })
                .color(if placeholder { p.muted } else { p.text })
                .single_line(),
        );
    if select {
        f = f.child(el().grow(1.0)).child(icon(&icons::CARET_DOWN, 16.0).color(p.muted));
    }
    f
}

fn bordered(p: &Pal, glyph: &'static Icon, label: &str) -> El {
    row()
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .border(1.0, p.line)
        .hover(p.raised)
        .focusable(true)
        .role("button")
        .label(label.to_string())
        .child(icon(glyph, 16.0).color(p.text2))
        .child(text(label.to_string()).font(sans(13.0, 500.0)).color(p.text).single_line())
}

fn destination(rows: &[Row], p: &Pal) -> El {
    let folder = raw(rows, "outputDir");
    let (parent, leaf) = folder.rsplit_once('\\').unwrap_or(("", folder.as_str()));
    card(p).child(
        row()
            .w_full()
            .gap(16.0)
            .center_y()
            .child(
                col().grow(1.0).gap(4.0).child(text(w::SAVING_TO).font(sans(12.0, 500.0)).color(p.text2)).child(
                    row()
                        .child(text(format!("{parent}\\")).font(mono(13.0, 400.0)).color(p.text2))
                        .child(text(leaf.to_string()).font(mono(13.0, 500.0)).color(p.text)),
                ),
            )
            .child(bordered(p, &icons::FOLDER, w::CHANGE)),
    )
}

fn defaults(rows: &[Row], p: &Pal) -> El {
    let labelled = |label: &str, value: &str| {
        col()
            .grow(1.0)
            .min_w(0.0)
            .gap(8.0)
            .child(text(label.to_string()).font(sans(12.0, 500.0)).color(p.text2))
            .child(field(p, label, value, true, false))
    };
    let toggled = |(label, desc): (&str, &str), key: &str| setting(p, label, desc, toggle(p, label, on(rows, key)));
    section(p, w::DEFAULTS, Some(w::DEFAULTS_HINT))
        .child(
            row()
                .w_full()
                .gap(12.0)
                .mb(8.0)
                .child(labelled(w::AUDIO_FORMAT, &shown(rows, "defaultFormat")))
                .child(labelled(w::AUDIO_QUALITY, &shown(rows, "defaultQuality")))
                .child(labelled(w::VIDEO_QUALITY, &shown(rows, "defaultVideoQuality"))),
        )
        .child(toggled(w::SKIP_EXISTING, "skipExisting"))
        .child(toggled(w::EMBED_THUMBNAIL, "embedThumbnail"))
        .child(toggled(w::EMBED_METADATA, "embedMetadata"))
        .child(setting(p, w::SIMULTANEOUS.0, "", field(p, w::SIMULTANEOUS.0, &shown(rows, "maxConcurrentDownloads"), true, false)))
}

/// "More download options", open at the cookies: where a failed sleeve's "Open settings" lands.
fn more(rows: &[Row], p: &Pal) -> El {
    let picked = |label: &str, key: &str| setting(p, label, "", field(p, label, &shown(rows, key), true, false));
    let template = raw(rows, "outputTemplate");
    let (naming, empty) = if template.is_empty() { (w::FILE_NAMING_PLACEHOLDER.to_string(), true) } else { (template, false) };
    section(p, w::MORE, Some(w::MORE_HINT))
        .child(picked(w::SIGN_IN_WITH, "cookieBrowser"))
        .child(setting(p, w::IMPORT_COOKIES.0, w::IMPORT_COOKIES.1, bordered(p, &icons::FOLDER, w::CHOOSE_FILE)))
        .child(picked(w::SUBTITLES, "subtitleMode"))
        .child(picked(w::SPONSOR_SEGMENTS, "sponsorBlock"))
        .child(picked(w::VIDEO_CONTAINER, "videoContainer"))
        .child(setting(p, w::FILE_NAMING.0, w::FILE_NAMING.1, field(p, w::FILE_NAMING.0, &naming, false, empty)))
}

fn appearance(s: &State, rows: &[Row], p: &Pal) -> El {
    let chosen = match raw(rows, "theme").as_str() {
        "light" => 0,
        "dark" => 1,
        _ => 2,
    };
    let mut theme = row().h(32.0).padding(2.0).gap(2.0).radius(8.0).border(1.0, p.line).role("radiogroup").label(w::THEME.0);
    for (i, name) in w::THEMES.into_iter().enumerate() {
        let on = i == chosen;
        theme = theme.child(
            el().h(26.0)
                .px(12.0)
                .center()
                .radius(6.0)
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .role("radio")
                .checked(on)
                .child(text(name).font(sans(13.0, if on { 500.0 } else { 400.0 })).color(if on { p.text } else { p.text2 })),
        );
    }
    let mut swatches = row().gap(8.0).role("radiogroup").label(w::PALETTE_ARIA);
    for i in 0..=palette::NAMES.len() {
        let sw = Pal::new(i, p.dark);
        swatches = swatches.child(
            el().size(24.0, 24.0)
                .radius(12.0)
                .center()
                .fill(sw.bg)
                .border(2.0, if i == s.palette { p.text } else { sw.accent })
                .role("radio")
                .checked(i == s.palette)
                .child(el().size(8.0, 8.0).radius(4.0).fill(sw.accent)),
        );
    }
    let toggled = |(label, desc): (&str, &str), key: &str| setting(p, label, desc, toggle(p, label, on(rows, key)));
    section(p, w::APPEARANCE, None)
        .child(setting(p, w::THEME.0, w::THEME.1, theme))
        .child(setting(p, w::PALETTE.0, w::PALETTE.1, swatches))
        .child(toggled(w::CLIPBOARD_WATCHER, "clipboardWatch"))
        .child(toggled(w::NOTIFICATIONS, "showNotifications"))
        .child(toggled(w::DISCORD, "discordRichPresence"))
}

fn schedule(rows: &[Row], p: &Pal) -> El {
    let chosen: Vec<u64> = rows
        .iter()
        .find(|r| r.key == Some("scheduleDays"))
        .and_then(|r| r.value.as_array())
        .map_or_else(Vec::new, |days| days.iter().filter_map(|d| d.as_u64()).collect());
    // Electron's order: Sunday first.
    let mut days = row().gap(4.0).role("group").label(w::ACTIVE_DAYS.0);
    for (i, d) in w::DAYS_SHORT.iter().enumerate() {
        let on = chosen.contains(&(i as u64));
        days = days.child(
            el().size(32.0, 28.0)
                .center()
                .radius(8.0)
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .border(1.0, if on { p.control } else { p.line })
                .focusable(true)
                .role("checkbox")
                .checked(on)
                .child(text(*d).font(mono(12.0, 500.0)).color(if on { p.text } else { p.muted })),
        );
    }
    // As in Electron: the section's own heading carries the master switch.
    let head = row()
        .w_full()
        .gap(16.0)
        .mb(8.0)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .gap(4.0)
                .child(text(w::SCHEDULE).font(sans(16.0, 500.0)).color(p.text))
                .child(text(w::SCHEDULE_HINT).font(sans(12.0, 400.0)).color(p.text2)),
        )
        .child(toggle(p, w::ENABLE_SCHEDULER, on(rows, "scheduleEnabled")));
    card(p)
        .child(head)
        .child(setting(p, w::START_TIME.0, w::START_TIME.1, field(p, w::START_TIME.0, &raw(rows, "scheduleTime"), false, false)))
        .child(setting(p, w::ACTIVE_DAYS.0, w::ACTIVE_DAYS.1, days))
        .child(setting(p, w::SHUTDOWN.0, w::SHUTDOWN.1, toggle(p, w::SHUTDOWN.0, on(rows, "scheduleShutdown"))))
}

/// An updater: name and version, its state in a sentence, its one button.
fn status_object(p: &Pal, name: &str, version: &str, desc: &str, button: El) -> El {
    row()
        .w_full()
        .gap(16.0)
        .py(4.0)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(text(name.to_string()).font(sans(13.0, 500.0)).color(p.text))
                        .child(text(version.to_string()).font(mono(12.0, 400.0)).color(p.muted)),
                )
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(el().size(8.0, 8.0).radius(4.0).fill(p.success))
                        .child(text(desc.to_string()).font(sans(12.0, 400.0)).color(p.text2)),
                ),
        )
        .child(button)
}

fn software(s: &State, rows: &[Row], p: &Pal) -> El {
    section(p, w::SOFTWARE, Some(w::SOFTWARE_HINT))
        .child(status_object(p, w::APP_NAME, &app_version(), w::APP_UP_TO_DATE, bordered(p, &icons::REFRESH, w::CHECK_FOR_UPDATES)))
        .child(status_object(p, w::ENGINE_NAME, &s.engine.current, w::ENGINE_UP_TO_DATE, bordered(p, &icons::REFRESH, w::ENGINE_CHECK)))
        .child(setting(
            p,
            w::AUTO_UPDATE_ENGINE.0,
            w::AUTO_UPDATE_ENGINE.1,
            toggle(p, w::AUTO_UPDATE_ENGINE.0, on(rows, "autoUpdateEngine")),
        ))
}

fn plan(p: &Pal) -> El {
    let danger = |(label, desc): (&str, &str)| {
        row()
            .w_full()
            .py(8.0)
            .gap(16.0)
            .center_y()
            .border_sides([1.0, 0.0, 0.0, 0.0], p.line_soft)
            .focusable(true)
            .role("button")
            .label(label.to_string())
            .child(
                col()
                    .grow(1.0)
                    .gap(4.0)
                    .child(text(label.to_string()).font(sans(13.0, 500.0)).color(p.error))
                    .child(text(desc.to_string()).font(sans(12.0, 400.0)).color(p.text2).wrap()),
            )
            .child(icon(&icons::CARET_RIGHT, 16.0).color(p.muted))
    };
    section(p, w::PLAN, None)
        .child(status_object(p, w::YOUR_PLAN, w::PLAN_PREMIUM.0, w::PLAN_PREMIUM.1, el()))
        .child(danger(w::DEACTIVATE))
        .child(danger(w::MOVE))
}

fn help(p: &Pal) -> El {
    section(p, w::HELP, None)
        .child(setting(p, w::SHORTCUTS.0, w::SHORTCUTS.1, bordered(p, &icons::EXTERNAL, w::SHOW_SHORTCUTS)))
        .child(setting(p, w::REPORT.0, w::REPORT.1, bordered(p, &icons::EXTERNAL, w::GET_SUPPORT)))
}

/// A section kept closed: its title and Electron's hint, one press to open.
fn folded(p: &Pal, title: &str, hint: &str, rule: bool) -> El {
    row()
        .w_full()
        .gap(12.0)
        .center_y()
        .border_sides([if rule { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0], p.line_soft)
        .focusable(true)
        .role("button")
        .label(title.to_string())
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(text(title.to_string()).font(sans(16.0, 500.0)).color(p.text))
                .child(text(hint.to_string()).font(sans(12.0, 400.0)).color(p.text2).wrap()),
        )
        .child(icon(&icons::CARET_RIGHT, 16.0).color(p.muted))
}
