//! Settings, the third place (Ctrl+3): one ruled column read top to bottom,
//! like the index card in a record box. Each section's name and hint stand in
//! a left margin under a numbered spine label; its settings run beside them,
//! one per line, the control at the right. Where files go leads, on its own.
//! Both of the old app's disclosures are drawn open. Every section and row is
//! `settings_spec::sections`, so the wording is `copy::settings` word for word
//! and the values, locks and dimming are the spec's. The controls below are
//! shared with The collection, which wears this look.
use rune::{Color, El, Icon, ScrollSpec, Tx, col, el, icon, row, text};
use serde_json::Value;

use super::parts::{app_version, danger_button, ghost, text_of};
use super::theme::{self, Pal, mono, sans, serif};
use crate::copy::{found, settings as w};
use crate::gate::{self, Confirm, Overlay, State};
use crate::settings_spec::{self, Kind, Opt, Row, Section};
use crate::{actions, copy, icons, keys, palette};

/// The margin that holds each section's name.
const MARGIN: f32 = 248.0;
/// A select's or a field's width in a row.
const CONTROL: f32 = 200.0;
/// The values of `theme`, in the order of `w::THEMES`.
const THEME_IDS: [&str; 3] = ["light", "dark", "system"];

pub fn view(s: &State, p: &Pal) -> El {
    let head = row()
        .w_full()
        .gap(16.0)
        .items_end()
        .child(text(w::TITLE).font(serif(44.0).lh(48.0)).color(p.text))
        .child(el().grow(1.0))
        .child(text(w::SAVES).font(sans(13.0, 400.0)).color(p.muted).mb(4.0));
    let mut page = col().w_full().max_w(1040.0).self_center().px(40.0).pt(24.0).pb(48.0).child(head);
    for (i, sec) in settings_spec::sections(s).iter().enumerate() {
        page = page.child(if sec.title.is_empty() {
            destination(p, &sec.rows[0])
                .key(keys::section(i))
                .h(64.0)
                .px(16.0)
                .mt(24.0)
                .mb(8.0)
                .radius(12.0)
                .fill(p.card)
                .border(1.0, p.line_soft)
        } else {
            section(s, p, i, sec)
        });
    }
    col().key("settings").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(page)
}

// ── Structure ───────────────────────────────────────────────────────────────

/// A section: a rule across, the name in the margin under its spine label, the settings beside it.
fn section(s: &State, p: &Pal, n: usize, sec: &Section) -> El {
    let tag = el()
        .h(20.0)
        .px(4.0)
        .center()
        .radius(2.0)
        .fill(theme::label_paper())
        .border(1.0, theme::label_ink().alpha(0.18))
        .child(text(format!("{n:02}")).font(mono(11.0, 500.0)).color(theme::label_ink()));
    let mut name =
        col().w(MARGIN).pr(32.0).gap(8.0).child(row().child(tag)).child(text(sec.title).font(sans(15.0, 500.0)).color(p.text).wrap());
    if let Some(h) = sec.hint {
        name = name.child(text(h).font(sans(12.0, 400.0)).color(p.muted).wrap());
    }
    let mut c = col();
    for (i, it) in body(s, p, sec, |r| line(s, p, r)).into_iter().enumerate() {
        c = c.child(if i == 0 { it } else { it.border_sides([1.0, 0.0, 0.0, 0.0], p.line_soft) });
    }
    row()
        .key(keys::section(n))
        .w_full()
        .pt(24.0)
        .pb(16.0)
        .items_start()
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .child(name)
        .child(c.grow(1.0).min_w(0.0))
}

/// A section's parts in order: Download defaults set its three choices side by side, Plan &
/// license ends with the machine line. `each` draws one row in the preset's own frame.
pub(crate) fn body(s: &State, p: &Pal, sec: &Section, each: impl Fn(&Row) -> El) -> Vec<El> {
    let mut rows = sec.rows.as_slice();
    let mut out = Vec::new();
    if sec.title == w::DEFAULTS && rows.len() >= 3 {
        out.push(trio(p, &rows[..3]));
        rows = &rows[3..];
    }
    out.extend(rows.iter().map(each));
    if sec.title == w::PLAN {
        out.push(machine(s, p));
    }
    out
}

/// A setting: its name and description, the control at the right; a dimmed one a step back.
pub(crate) fn line(s: &State, p: &Pal, r: &Row) -> El {
    if r.kind == Kind::Danger {
        return danger(s, p, r);
    }
    let l = row().w_full().min_h(56.0).py(12.0).gap(24.0).center_y().child(words(s, p, r)).child(control(s, p, r));
    if r.dimmed { l.opacity(0.6) } else { l }
}

/// A row's name (with its version for the two updaters), its description, and on Basic its lock.
fn words(s: &State, p: &Pal, r: &Row) -> El {
    let mut name = row().gap(8.0).center_y().child(text(r.label).font(sans(13.0, 500.0)).color(p.text));
    let version = match r.label {
        w::APP_NAME => Some(app_version()),
        w::ENGINE_NAME => Some(s.engine.current.clone()),
        _ => None,
    };
    if let Some(v) = version {
        name = name.child(text(v).font(mono(12.0, 400.0)).color(p.muted));
    }
    if r.lock.is_none() && r.gated() {
        // Some of its choices need Premium: the drop-down's title says what that means.
        name = name.child(lock(p.muted).help(found::LOCK_SELECT_TITLE).key(keys::setting_lock(r.key.unwrap_or_default())));
    }
    let mut c = col().grow(1.0).min_w(0.0).gap(4.0).child(name);
    if !r.desc.is_empty() {
        c = c.child(text(r.desc.clone()).font(sans(12.0, 400.0)).color(p.text2).wrap());
    }
    if let Some(sentence) = &r.lock {
        c = c.child(
            row()
                .gap(8.0)
                .center_y()
                .child(lock(p.text2))
                .child(text(sentence.clone()).font(sans(12.0, 400.0)).color(p.text2).wrap().shrink()),
        );
    }
    c
}

/// "Available with Premium": a lock in ink, never the accent.
fn lock(ink: Color) -> El {
    icon(&icons::LOCK_SIGNAL, 16.0).color(ink)
}

// ── Controls ────────────────────────────────────────────────────────────────

/// The control a row's kind asks for, keyed by its config key.
fn control(s: &State, p: &Pal, r: &Row) -> El {
    let invalid = r.key.is_some_and(|k| s.invalid.contains(k));
    let c = match &r.kind {
        Kind::Toggle => {
            let on = r.value.as_bool().unwrap_or(false);
            let switch = toggle(p, r.label, on);
            // TODO(shared): a locked row's tap should say what Premium adds, and `holdUnconfirmed` is not a config key
            // (`Core::set_hold_unconfirmed`); no action takes either, so those two rows stay unwired.
            match r.key {
                Some(key) if r.lock.is_none() && key != "holdUnconfirmed" => {
                    switch.on_press(move |s: &mut State| _ = actions::set_setting(s, key, Value::Bool(!on)))
                }
                _ => switch,
            }
        }
        Kind::Select(options) => select(p, r.label, &chosen(options, &r.value)).w(CONTROL),
        Kind::Text => input(p, r.label, &text_of(&r.value), r.placeholder.unwrap_or_default(), invalid).w(CONTROL),
        Kind::Time => input(p, r.label, &text_of(&r.value), "", invalid).w(96.0),
        Kind::Days => days(p, &r.value),
        Kind::Theme => theme_switch(p, &r.value),
        Kind::Palette => swatches(s, p),
        Kind::Layout => {
            row().gap(12.0).center_y().child(text(gate::PRESETS[gate::preset(s)].0).font(sans(13.0, 400.0)).color(p.text2)).child(
                ghost(p, keys::LAYOUT_CHOOSE, None, w::LAYOUT_CHOOSE, p.text).on_press(|s: &mut State| actions::open(s, Overlay::Picker)),
            )
        }
        Kind::Status => status(p, r),
        Kind::Folder | Kind::Danger => el(),
    };
    let c = match r.key {
        Some(k) => c.key(keys::setting(k)),
        None => c,
    };
    // Locked on Basic: still pressable, so a tap can say what Premium adds. A dimmed row is already faded.
    if r.lock.is_some() && !r.dimmed { c.opacity(0.5) } else { c }
}

fn toggle(p: &Pal, label: &str, on: bool) -> El {
    el().size(36.0, 20.0)
        .radius(10.0)
        .fill(if on { p.accent } else { p.control.alpha(0.6) })
        .focusable(true)
        .role("switch")
        .label(label)
        .checked(on)
        // Off, an ink thumb on the paper track, so it reads at 3:1 (jury C-1).
        .child(
            el().size(12.0, 12.0)
                .radius(6.0)
                .fill(if on { p.bg } else { p.text2.mix(p.text, 0.5) })
                .absolute(if on { 20.0 } else { 4.0 }, 4.0),
        )
}

/// The label of the chosen option.
fn chosen(options: &[Opt], value: &Value) -> String {
    let v = text_of(value);
    options.iter().find(|o| o.value == v).map(|o| o.label.clone()).unwrap_or_default()
}

/// A select showing its value, with a caret.
// TODO(shared): choosing an option needs the open select in `State` and an action that writes the option (`set_setting`).
fn select(p: &Pal, label: &str, value: &str) -> El {
    row()
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
        .label(label)
        .child(text(value).font(sans(13.0, 400.0)).color(p.text).single_line().truncate().grow(1.0).min_w(0.0))
        .child(icon(&icons::CARET_DOWN, 16.0).color(p.muted))
}

/// A typed value in the figures face, or its placeholder, muted; a refused one edged in red.
// TODO(shared): a `TextEdit` per setting key in `State`, committed through `set_setting`; none exists yet.
fn input(p: &Pal, label: &str, value: &str, placeholder: &str, invalid: bool) -> El {
    let (shown, ink) = if value.is_empty() { (placeholder, p.muted) } else { (value, p.text) };
    row()
        .h(32.0)
        .px(12.0)
        .center_y()
        .radius(8.0)
        .fill(p.card)
        .border(1.0, if invalid { p.error } else { p.control.alpha(0.55) })
        .focusable(true)
        .role("textbox")
        .label(label)
        .invalid(invalid)
        .child(text(shown).font(mono(12.0, 400.0)).color(ink).single_line().truncate().min_w(0.0))
}

/// The weekday chips, Sunday first as in Electron.
fn days(p: &Pal, value: &Value) -> El {
    let chosen: Vec<u64> = value.as_array().map_or_else(Vec::new, |d| d.iter().filter_map(Value::as_u64).collect());
    let mut days = row().gap(4.0).role("group").label(w::ACTIVE_DAYS.0);
    for (i, day) in w::DAYS.iter().enumerate() {
        let on = chosen.contains(&(i as u64));
        days = days.child(
            el().key(keys::day(i))
                .on_press(move |s: &mut State| actions::toggle_day(s, i as u8))
                .size(32.0, 28.0)
                .center()
                .radius(8.0)
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .border(1.0, if on { p.control } else { p.line })
                .focusable(true)
                .role("checkbox")
                .checked(on)
                .label(*day)
                .child(text(w::DAYS_SHORT[i]).font(mono(12.0, 500.0)).color(if on { p.text } else { p.muted })),
        );
    }
    days
}

fn theme_switch(p: &Pal, value: &Value) -> El {
    let chosen = THEME_IDS.iter().position(|t| *t == text_of(value)).unwrap_or(2);
    let mut theme =
        row().h(32.0).padding(4.0).gap(4.0).radius(8.0).fill(p.sunk).border(1.0, p.line_soft).role("radiogroup").label(w::THEME.0);
    for (i, name) in w::THEMES.into_iter().enumerate() {
        let on = i == chosen;
        theme = theme.child(
            el().key(keys::theme(i))
                .on_press_tx(move |s: &mut State, tx: &mut Tx| actions::set_theme(s, THEME_IDS[i], tx))
                .h(24.0)
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
    theme
}

/// Shelf's own palette (warm paper) first, then the app's seven, each a swatch of its paper and accent.
fn swatches(s: &State, p: &Pal) -> El {
    let mut swatches = row().gap(8.0).flex_wrap().justify_end().role("radiogroup").label(w::PALETTE_ARIA);
    for i in 0..=palette::NAMES.len() {
        let name = if i == 0 { copy::PRESET_SHELF } else { palette::NAMES[i - 1] };
        let sw = Pal::new(i, p.dark);
        let on = i == s.palette;
        swatches = swatches.child(
            el().key(keys::palette(i))
                .on_press_tx(move |s: &mut State, tx: &mut Tx| actions::set_palette(s, i, tx))
                .size(24.0, 24.0)
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
    swatches.max_w(256.0)
}

/// A row that is a line with a button: the plan's stamp, an updater, the cookies import, help.
fn status(p: &Pal, r: &Row) -> El {
    let mut c = row().gap(12.0).center_y();
    if r.label == w::YOUR_PLAN {
        // The plan, stamped on the sleeves' paper.
        c = c.child(
            el().h(24.0)
                .px(8.0)
                .center()
                .radius(4.0)
                .fill(theme::label_paper())
                .border(1.0, theme::label_ink().alpha(0.18))
                .child(text(text_of(&r.value)).font(sans(13.0, 500.0)).color(theme::label_ink())),
        );
    }
    if let Some(b) = r.button {
        let (key, glyph): (&str, Option<&'static Icon>) = match b {
            w::CHOOSE_FILE => (keys::IMPORT_COOKIES, Some(&icons::FOLDER)),
            w::CHECK_FOR_UPDATES => (keys::CHECK_UPDATE, Some(&icons::REFRESH)),
            w::ENGINE_CHECK => (keys::CHECK_ENGINE, Some(&icons::REFRESH)),
            w::SHOW_SHORTCUTS => (keys::SHOW_SHORTCUTS, None),
            w::GET_SUPPORT => (keys::GET_SUPPORT, Some(&icons::EXTERNAL)),
            _ => (keys::SEE_PLANS, None),
        };
        let button = ghost(p, key, glyph, b, p.text);
        c = c.child(match key {
            keys::IMPORT_COOKIES => button.on_press_tx(actions::import_cookies),
            keys::CHECK_UPDATE => button.on_press(actions::check_update),
            keys::CHECK_ENGINE => button.on_press(actions::check_engine),
            keys::SHOW_SHORTCUTS => button.on_press(actions::show_shortcuts),
            keys::GET_SUPPORT => button.on_press_tx(actions::get_support),
            // TODO(shared): `actions::see_plans` (the plans sheet); no action opens it yet.
            _ => button,
        });
    }
    c
}

// ── Pieces ──────────────────────────────────────────────────────────────────

/// Where files go: no heading in the old app, so it leads the page on its own.
pub(crate) fn destination(p: &Pal, r: &Row) -> El {
    let folder = text_of(&r.value);
    let (parent, leaf) = folder.rsplit_once('\\').unwrap_or(("", folder.as_str()));
    row()
        .w_full()
        .gap(16.0)
        .center_y()
        .child(icon(&icons::FOLDER, 16.0).color(p.text2))
        .child(text(r.label).font(sans(13.0, 400.0)).color(p.text2))
        .child(
            row()
                .grow(1.0)
                .min_w(0.0)
                .help(folder.clone())
                .child(text(format!("{parent}\\")).font(mono(13.0, 400.0)).color(p.muted).single_line().truncate_head().shrink())
                .child(text(leaf.to_string()).font(mono(13.0, 500.0)).color(p.text).single_line()),
        )
        .child(ghost(p, keys::CHANGE_FOLDER, None, r.button.unwrap_or(w::CHANGE), p.text).on_press_tx(actions::choose_folder))
}

/// The three download defaults side by side, each with its name above it.
fn trio(p: &Pal, rows: &[Row]) -> El {
    let mut trio = row().w_full().gap(16.0).pb(16.0);
    for r in rows {
        let mut name = row().gap(8.0).center_y().child(text(r.label).font(sans(12.0, 400.0)).color(p.text2));
        if r.gated() {
            name = name.child(lock(p.muted).help(found::LOCK_SELECT_TITLE).key(keys::setting_lock(r.key.unwrap_or_default())));
        }
        let value = match &r.kind {
            Kind::Select(options) => chosen(options, &r.value),
            _ => text_of(&r.value),
        };
        let pick = select(p, r.label, &value).w_full();
        trio = trio.child(col().grow(1.0).min_w(0.0).gap(8.0).child(name).child(match r.key {
            Some(k) => pick.key(keys::setting(k)),
            None => pick,
        }));
    }
    trio
}

/// A licence action: its name in the error colour; armed, the warning and the confirm in its place.
fn danger(s: &State, p: &Pal, r: &Row) -> El {
    let which = if r.label == w::DEACTIVATE.0 { Confirm::Deactivate } else { Confirm::Move };
    let armed = s.confirm == Some(which);
    let desc = match r.armed {
        Some((warning, _)) if armed => warning.to_string(),
        _ => r.desc.clone(),
    };
    let words = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .child(text(r.label).font(sans(13.0, 500.0)).color(p.error))
        .child(text(desc).font(sans(12.0, 400.0)).color(p.text2).wrap());
    let l = row().key(keys::arm(which)).w_full().min_h(56.0).py(12.0).gap(24.0).center_y().child(words);
    match r.armed {
        Some((_, confirm)) if armed => l.child(
            row()
                .gap(8.0)
                .center_y()
                .child(ghost(p, keys::CONFIRM_NO, None, w::CANCEL, p.text).on_press(actions::disarm))
                .child(danger_button(p, keys::CONFIRM_YES, confirm).on_press(actions::confirm)),
        ),
        _ => l
            .hover(p.soft(p.error))
            .focusable(true)
            .role("button")
            .label(r.label)
            .on_press(move |s: &mut State| actions::arm(s, which))
            .child(icon(&icons::CARET_RIGHT, 16.0).color(p.muted)),
    }
}

/// What a licence key binds to on this computer, and when a move is next allowed.
fn machine(s: &State, p: &Pal) -> El {
    // TODO(shared): the licence's transfer cooldown end date on `State`; none exists yet.
    let cooldown: Option<&str> = None;
    let mut c = col().w_full().py(12.0).gap(4.0).child(
        row()
            .gap(8.0)
            .help(w::MACHINE_ID_TITLE)
            .child(text(w::MACHINE_ID).font(sans(12.0, 400.0)).color(p.muted))
            .child(text(s.machine_id.clone()).font(mono(12.0, 400.0)).color(p.muted)),
    );
    if let Some(date) = cooldown {
        c = c.child(text(w::cooldown_note(date)).font(sans(12.0, 400.0)).color(p.text2).wrap());
    }
    c
}
