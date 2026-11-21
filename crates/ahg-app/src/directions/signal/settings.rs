//! Settings: the desk's patch sheet. The folder leads in a recessed well (the
//! one answer people come here for); then each section is a label column on
//! the left and its panel of rows on the right, so the eye runs down the
//! names. Panels vary by what they hold: a field grid for the plain choices,
//! rows for switches, status lines for the two updaters. Every section, row,
//! word and value comes from `settings_spec::sections`; this file only draws.
//! One bar places the same panels (`panel_of`) beside its section list.
use rune::{Color, El, Icon, PopupItem, PopupMenu, ScrollSpec, Tx, col, el, icon, row};
use serde_json::Value;

use super::parts::{Line, button, dot, ghost, label, mono, popup, sans};
use super::theme::{self, Pal};
use crate::actions;
use crate::copy::{self, found as f, settings as w};
use crate::gate::{Confirm, Overlay, State};
use crate::icons;
use crate::keys;
use crate::palette;
use crate::settings_spec::{self, Kind, Row, Section};

/// The section names' column.
const NAMES: f32 = 200.0;
/// A select's or a field's width in a row.
const CONTROL: f32 = 176.0;
/// Plain choices set side by side in one grid line.
const GRID: usize = 3;

/// Palette names, `s.palette` order: Signal's own, then the app's seven.
fn palette_name(i: usize) -> &'static str {
    if i == 0 { copy::PRESET_SIGNAL } else { palette::NAMES[i - 1] }
}

pub fn view(s: &State, p: &Pal) -> El {
    let head = row()
        .w_full()
        .items_end()
        .gap(16.0)
        .child(label(w::TITLE, sans(28.0, 500.0), p.text))
        .child(label(w::SAVES, sans(13.0, 400.0), p.muted).mb(4.0));
    let mut page = col().w_full().max_w(1120.0).px(32.0).pt(28.0).pb(48.0).gap(32.0).child(head);
    for sec in settings_spec::sections(s) {
        page = page.child(match sec.rows.first() {
            Some(r) if sec.title.is_empty() && r.kind == Kind::Folder => destination(r, p),
            _ => section(&sec, panel_of(s, &sec, p), p),
        });
    }
    col().key("settings").w_full().h_full().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).items_center().child(page)
}

/// Destination has no heading: the folder, set like a tape label in the desk's well.
fn destination(r: &Row, p: &Pal) -> El {
    let dir = text_of(&r.value);
    let (parent, leaf) = dir.rsplit_once('\\').map_or(("", dir.as_str()), |(a, b)| (a, b));
    row()
        .w_full()
        .h(72.0)
        .pl(20.0)
        .pr(16.0)
        .gap(16.0)
        .center_y()
        .radius(12.0)
        .fill(p.deep)
        .border(1.0, p.line_strong)
        .child(
            el().size(40.0, 40.0)
                .center()
                .radius(8.0)
                .fill(p.surface)
                .border(1.0, p.line)
                .child(icon(&icons::FOLDER, 16.0).color(p.accent_ink)),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(label(r.label, sans(12.0, 400.0), p.text2))
                .child(Line::new().fig(&format!("{parent}\\"), p.text2).fig(leaf, p.text).one(mono(16.0, 500.0)).help(dir.clone())),
        )
        .child(row_button(r.button.unwrap_or(w::CHANGE), p))
}

/// A section: its name and hint in the left column, its panel beside them.
fn section(sec: &Section, body: El, p: &Pal) -> El {
    let mut names = col().w(NAMES).pt(8.0).gap(4.0).child(label(sec.title, sans(16.0, 500.0), p.text));
    if let Some(h) = sec.hint {
        names = names.child(label(h, sans(12.0, 400.0).lh(16.0), p.text2).wrap());
    }
    row().w_full().gap(24.0).items_start().child(names).child(body.grow(1.0).min_w(0.0))
}

/// A section's rows on their panel, a hairline between them. Consecutive plain
/// choices (a select or a field with no description) share a grid line.
pub(crate) fn panel_of(s: &State, sec: &Section, p: &Pal) -> El {
    let mut parts = Vec::new();
    let mut cells: Vec<&Row> = Vec::new();
    for r in &sec.rows {
        if r.desc.is_empty() && matches!(r.kind, Kind::Select(_) | Kind::Text) {
            cells.push(r);
            if cells.len() == GRID {
                parts.push(fields(&cells, p));
                cells.clear();
            }
            continue;
        }
        if !cells.is_empty() {
            parts.push(fields(&cells, p));
            cells.clear();
        }
        parts.push(setting(s, r, p));
    }
    if !cells.is_empty() {
        parts.push(fields(&cells, p));
    }
    if sec.title == w::PLAN {
        parts.push(machine(s, p));
        // TODO(shared): the transfer cooldown date (`w::cooldown_note`) has no fact in `State` yet.
    }
    let mut c = col().radius(12.0).fill(p.surface).border(1.0, p.line).clip_rounded(12.0);
    for (i, part) in parts.into_iter().enumerate() {
        c = c.child(if i > 0 { part.border_sides([1.0, 0.0, 0.0, 0.0], p.line) } else { part });
    }
    c
}

/// What a key binds to on this machine: a quiet figure under the plan.
fn machine(s: &State, p: &Pal) -> El {
    row()
        .w_full()
        .h(44.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .help(w::MACHINE_ID_TITLE)
        .child(label(w::MACHINE_ID, sans(12.0, 400.0), p.muted))
        .child(label(s.machine_id.clone(), mono(12.0, 400.0), p.text2).selectable(true))
}

/// The row's value as text; a number or a list as JSON.
fn text_of(v: &Value) -> String {
    match v {
        Value::String(t) => t.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// The label of the chosen option, or the raw value when it is not one of them.
fn chosen(r: &Row) -> String {
    let value = text_of(&r.value);
    match &r.kind {
        Kind::Select(options) => options.iter().find(|o| o.value == value).map_or(value, |o| o.label.clone()),
        _ => value,
    }
}

/// What a row's button does.
type Press = fn(&mut State, &mut Tx);

/// The key, icon and action of a row's one button, by its wording.
fn button_of(words: &str) -> (Option<&'static str>, Option<&'static Icon>, Option<Press>) {
    match words {
        w::CHANGE => (Some(keys::CHANGE_FOLDER), Some(&icons::FOLDER), Some(actions::choose_folder)),
        w::CHOOSE_FILE => (Some(keys::IMPORT_COOKIES), Some(&icons::FOLDER), Some(actions::import_cookies)),
        w::CHECK_FOR_UPDATES => (Some(keys::CHECK_UPDATE), Some(&icons::REFRESH), Some(|s, _| actions::check_update(s))),
        w::ENGINE_CHECK => (Some(keys::CHECK_ENGINE), Some(&icons::REFRESH), Some(|s, _| actions::check_engine(s))),
        w::SHOW_SHORTCUTS => (Some(keys::SHOW_SHORTCUTS), None, Some(|s, _| actions::show_shortcuts(s))),
        w::GET_SUPPORT => (Some(keys::GET_SUPPORT), Some(&icons::EXTERNAL), Some(actions::get_support)),
        w::LAYOUT_CHOOSE => (Some(keys::LAYOUT_CHOOSE), None, Some(|s, _| actions::open(s, Overlay::Picker))),
        // TODO(shared): `actions::see_plans(s)`; the plans sheet has no action yet.
        w::SEE_PLANS => (Some(keys::SEE_PLANS), None, None),
        _ => (None, None, None),
    }
}

fn row_button(words: &str, p: &Pal) -> El {
    let (key, glyph, press) = button_of(words);
    let mut b = button(glyph, words, p.text, p);
    if let Some(k) = key {
        b = b.key(k);
    }
    match press {
        Some(f) => b.on_press_tx(f),
        None => b,
    }
}

/// Some of a select's choices need Premium: a lock right after its name, its title saying why.
fn name_lock(r: &Row, p: &Pal) -> El {
    el().size(16.0, 16.0)
        .key(keys::setting_lock(r.key.unwrap_or_default()))
        .help(f::LOCK_SELECT_TITLE)
        .child(icon(&icons::LOCK_SIGNAL, 16.0).color(p.muted))
}

/// The lock a Basic row carries, and the sentence that says what Premium adds.
fn lock_line(sentence: &str, p: &Pal) -> El {
    row()
        .gap(8.0)
        .items_start()
        .child(el().size(16.0, 16.0).child(icon(&icons::LOCK_SIGNAL, 16.0).color(p.text2)))
        .child(label(sentence, sans(12.0, 400.0).lh(16.0), p.text2).wrap().grow(1.0).min_w(0.0))
}

/// One row: its name and description on the left, its control on the right.
/// A dimmed row (it depends on a switch that is off) and a locked one sit a step back.
fn setting(s: &State, r: &Row, p: &Pal) -> El {
    match r.kind {
        Kind::Status if r.label == w::APP_NAME => {
            let version = format!("v{}", env!("CARGO_PKG_VERSION"));
            return status(r, &version, p.muted, p);
        }
        Kind::Status if r.label == w::ENGINE_NAME => return status(r, &s.engine.current, p.muted, p),
        Kind::Status if r.label == w::YOUR_PLAN => return plan(r, p),
        Kind::Danger => return danger(s, r, p),
        _ => {}
    }
    let back = r.dimmed || r.lock.is_some();
    let mut name = row().gap(8.0).center_y().child(label(r.label, sans(14.0, 400.0), if back { p.muted } else { p.text }));
    if r.lock.is_none() && r.gated() {
        name = name.child(name_lock(r, p));
    }
    let mut text = col().grow(1.0).min_w(0.0).gap(4.0).child(name);
    if !r.desc.is_empty() {
        text = text.child(label(r.desc.clone(), sans(12.0, 400.0).lh(16.0), if back { p.muted } else { p.text2 }).wrap());
    }
    if let Some(lock) = &r.lock {
        text = text.child(lock_line(lock, p).mt(4.0));
    }
    let c = control(s, r, Some(CONTROL), p);
    let c = if back { c.opacity(0.6) } else { c };
    row().w_full().min_h(56.0).px(16.0).py(12.0).gap(16.0).center_y().child(text).child(c)
}

/// Plain choices set side by side, each with its name above it.
fn fields(cells: &[&Row], p: &Pal) -> El {
    let mut r = row().w_full().px(16.0).pt(16.0).pb(20.0).gap(12.0).items_start();
    for c in cells {
        let mut name = row().gap(8.0).center_y().child(label(c.label, sans(12.0, 400.0), p.text2));
        if c.lock.is_none() && c.gated() {
            name = name.child(name_lock(c, p));
        }
        let ctl = control_plain(c, None, p);
        let mut cell = col().grow(1.0).min_w(0.0).gap(8.0).child(name).child(ctl);
        if let Some(lock) = &c.lock {
            cell = cell.child(lock_line(lock, p));
        }
        r = r.child(if c.dimmed || c.lock.is_some() { cell.opacity(0.6) } else { cell });
    }
    r
}

/// The control of a row, by its kind.
fn control(s: &State, r: &Row, w_: Option<f32>, p: &Pal) -> El {
    let key = r.key.unwrap_or_default();
    match &r.kind {
        Kind::Toggle => {
            let on = r.value.as_bool().unwrap_or(false);
            switch(r.label, on, p).key(keys::setting(key)).on_press(move |s: &mut State| {
                actions::set_setting(s, key, Value::Bool(!on));
            })
        }
        Kind::Days => days(r, p),
        Kind::Theme => theme_control(r, p),
        Kind::Palette => swatches(s, p),
        Kind::Layout => row()
            .gap(12.0)
            .center_y()
            .child(label(text_of(&r.value), mono(13.0, 400.0), p.text2))
            .child(row_button(r.button.unwrap_or(w::LAYOUT_CHOOSE), p)),
        Kind::Folder => row()
            .gap(12.0)
            .center_y()
            .min_w(0.0)
            .shrink()
            .child(label(text_of(&r.value), mono(13.0, 400.0), p.text).single_line().truncate().min_w(0.0).shrink())
            .child(row_button(r.button.unwrap_or(w::CHANGE), p)),
        Kind::Status => match r.button {
            Some(b) => row_button(b, p),
            None => el(),
        },
        Kind::Time => control_plain(r, Some(80.0), p),
        _ => control_plain(r, w_, p),
    }
}

/// A select or a typed field.
fn control_plain(r: &Row, w_: Option<f32>, p: &Pal) -> El {
    let key = keys::setting(r.key.unwrap_or_default());
    match &r.kind {
        Kind::Select(options) => {
            let (setting, now) = (r.key.unwrap_or_default(), text_of(&r.value));
            // A choice that needs Premium is listed but not pressable.
            let menu = PopupMenu::new()
                .items(
                    options
                        .iter()
                        .map(|o| PopupItem::new(o.value.clone(), o.label.clone()).checked(o.value == now).enabled(o.lock.is_none())),
                )
                .selected(now.clone());
            let s = popup(select(r.label, &chosen(r), w_, p), &key, menu, move |s, id| {
                actions::set_setting(s, setting, id.into());
            });
            if r.gated() { s.help(f::LOCK_SELECT_TITLE) } else { s }
        }
        // TODO(shared): a typed setting needs a `TextEdit` per key in `State` and an `actions::commit_text(s, key)`.
        _ => input(r.label, &text_of(&r.value), r.placeholder.unwrap_or_default(), w_, p).key(key),
    }
}

/// A switch on the desk: a square-shouldered toggle; on, its track is the lit segment.
fn switch(name: &str, on: bool, p: &Pal) -> El {
    el().size(36.0, 20.0)
        .radius(6.0)
        .fill(if on { p.accent } else { p.deep })
        .border(1.0, if on { p.accent } else { p.line_strong })
        .hover(p.hover)
        .press(p.press)
        .cursor(rune::Cursor::Pointer)
        .focusable(true)
        .role("switch")
        .label(name.to_string())
        .checked(on)
        // Off, the thumb is in the second ink, so it reads at 3:1 on the dark track (jury C-1).
        .child(el().size(12.0, 12.0).radius(4.0).fill(if on { p.deep } else { p.text2 }).absolute(if on { 19.0 } else { 3.0 }, 3.0))
}

fn well(w_: Option<f32>, name: &str, role: &'static str) -> El {
    let r = row()
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .focusable(true)
        .role(role)
        .label(name.to_string())
        .cursor(rune::Cursor::Pointer);
    match w_ {
        Some(v) => r.w(v),
        None => r.w_full(),
    }
}

/// A closed select showing its value. Its lock, when some choices need Premium, stands after
/// the setting's name (`name_lock`), not in here.
fn select(name: &str, value: &str, w_: Option<f32>, p: &Pal) -> El {
    well(w_, name, "combobox")
        .fill(p.deep)
        .border(1.0, p.line_strong)
        .hover(p.hover)
        .child(label(value, sans(13.0, 400.0), p.text).single_line().truncate().grow(1.0).min_w(0.0))
        .child(icon(&icons::CARET_DOWN, 16.0).color(p.muted))
}

/// A typed field: figures in mono; an empty one shows its placeholder.
fn input(name: &str, value: &str, placeholder: &str, w_: Option<f32>, p: &Pal) -> El {
    let (t, c) = if value.is_empty() { (placeholder, p.muted) } else { (value, p.text) };
    well(w_, name, "textbox")
        .fill(p.deep)
        .border(1.0, p.line_strong)
        .child(label(t, mono(13.0, 400.0), c).single_line().truncate().min_w(0.0))
}

fn theme_control(r: &Row, p: &Pal) -> El {
    let on_at = ["light", "dark", "system"].iter().position(|t| *t == text_of(&r.value)).unwrap_or(2);
    let mut g = row()
        .key(keys::setting("theme"))
        .padding(4.0)
        .gap(4.0)
        .radius(8.0)
        .fill(p.deep)
        .border(1.0, p.line)
        .role("radiogroup")
        .label(r.label);
    for (i, name) in w::THEMES.into_iter().enumerate() {
        let on = i == on_at;
        g = g.child(
            el().key(keys::theme(i))
                .h(28.0)
                .px(12.0)
                .center()
                .radius(6.0)
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .hover(p.hover)
                .cursor(rune::Cursor::Pointer)
                .on_press_tx(move |s: &mut State, tx: &mut Tx| actions::set_theme(s, ["light", "dark", "system"][i], tx))
                .role("radio")
                .checked(on)
                .child(label(name, sans(13.0, if on { 500.0 } else { 400.0 }), if on { p.text } else { p.text2 })),
        );
    }
    g
}

fn swatches(s: &State, p: &Pal) -> El {
    let mut g = row().gap(8.0).flex_wrap().justify_end().max_w(280.0).role("radiogroup").label(w::PALETTE_ARIA);
    for i in 0..=palette::NAMES.len() {
        let name = palette_name(i);
        let sw = theme::pal(i, p.dark);
        let on = s.palette == i;
        g = g.child(
            el().key(keys::palette(i))
                .size(28.0, 28.0)
                .radius(8.0)
                .center()
                .fill(sw.deep)
                .border(if on { 2.0 } else { 1.0 }, if on { p.text } else { p.line_strong })
                .hover(p.hover)
                .cursor(rune::Cursor::Pointer)
                .role("radio")
                .label(name)
                .help(name)
                .checked(on)
                .on_press_tx(move |s: &mut State, tx: &mut Tx| actions::set_palette(s, i, tx))
                // The palette's accent as one lit segment.
                .child(el().size(12.0, 4.0).radius(1.0).fill(sw.accent)),
        );
    }
    g
}

fn days(r: &Row, p: &Pal) -> El {
    let mut g = row().gap(4.0).role("group").label(r.label);
    for (i, day) in w::DAYS.iter().enumerate() {
        let on = r.value.as_array().is_some_and(|d| d.iter().any(|x| x.as_u64() == Some(i as u64)));
        g = g.child(
            el().key(keys::day(i))
                .size(32.0, 28.0)
                .center()
                .radius(6.0)
                .fill(if on { p.raised } else { p.deep })
                .border(1.0, if on { p.line_strong } else { p.line })
                .hover(p.hover)
                .press(p.press)
                .cursor(rune::Cursor::Pointer)
                .focusable(true)
                .on_press(move |s: &mut State| actions::toggle_day(s, i as u8))
                .role("checkbox")
                .label(*day)
                .checked(on)
                .child(label(w::DAYS_SHORT[i], mono(12.0, 500.0), if on { p.text } else { p.muted })),
        );
    }
    g
}

/// An updater: name and version, its state on a tally light, its one button.
// TODO(shared): the status sentence and tone from `State.update` / `State.engine` (checking,
// available, failed ...); until then the row's own description, the idle state.
fn status(r: &Row, version: &str, tone: Color, p: &Pal) -> El {
    let words = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .child(row().gap(8.0).center_y().child(label(r.label, sans(14.0, 400.0), p.text)).child(label(version, mono(12.0, 400.0), p.muted)))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(dot(tone, 8.0))
                .child(label(r.desc.clone(), sans(12.0, 400.0), p.text2).single_line().truncate().min_w(0.0)),
        );
    let mut out = row().w_full().min_h(64.0).px(16.0).py(12.0).gap(16.0).center_y().child(words);
    if let Some(b) = r.button {
        out = out.child(row_button(b, p));
    }
    out
}

/// The plan: Premium lit like the one segment that matters; Basic in ink, with the way up.
fn plan(r: &Row, p: &Pal) -> El {
    let name = text_of(&r.value);
    let tag = if r.button.is_some() {
        el().h(24.0).px(8.0).center().radius(4.0).fill(p.raised).border(1.0, p.line_strong).child(label(name, sans(12.0, 500.0), p.text))
    } else {
        el().h(24.0).px(8.0).center().radius(4.0).fill(p.accent).child(label(name, sans(12.0, 500.0), p.deep))
    };
    let mut end = row().gap(12.0).center_y().child(tag);
    if let Some(b) = r.button {
        end = end.child(row_button(b, p));
    }
    row()
        .w_full()
        .min_h(64.0)
        .px(16.0)
        .py(12.0)
        .gap(16.0)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(label(r.label, sans(14.0, 400.0), p.text))
                .child(label(r.desc.clone(), sans(12.0, 400.0).lh(16.0), p.text2).wrap()),
        )
        .child(end)
}

/// A licence action: it costs the user something, so its name is in the error
/// colour. Armed, it says what happens and asks once more.
fn danger(s: &State, r: &Row, p: &Pal) -> El {
    // TODO(shared): the row's own `Confirm` in the spec; matched by its wording until then.
    let which = if r.label == w::DEACTIVATE.0 { Confirm::Deactivate } else { Confirm::Move };
    let armed = s.confirm == Some(which);
    let base = row().key(keys::arm(which)).w_full().min_h(56.0).px(16.0).py(12.0).gap(16.0).center_y();
    if let (true, Some((sentence, yes))) = (armed, r.armed) {
        return base
            .fill(p.error_soft)
            .role("alertdialog")
            .label(r.label)
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(4.0)
                    .child(label(r.label, sans(14.0, 500.0), p.error))
                    .child(label(sentence, sans(12.0, 400.0).lh(16.0), p.text).wrap()),
            )
            .child(ghost(None, w::CANCEL, p.text2, p).key(keys::CONFIRM_NO).on_press(actions::disarm))
            .child(ghost(None, yes, p.deep, p).fill(p.error).key(keys::CONFIRM_YES).on_press(actions::confirm));
    }
    base.on_press(move |s: &mut State| actions::arm(s, which))
        .hover(p.hover)
        .cursor(rune::Cursor::Pointer)
        .focusable(true)
        .role("button")
        .label(r.label)
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(label(r.label, sans(14.0, 400.0), p.error))
                .child(label(r.desc.clone(), sans(12.0, 400.0).lh(16.0), p.text2).wrap()),
        )
        .child(icon(&icons::CARET_RIGHT, 16.0).color(p.muted))
}
