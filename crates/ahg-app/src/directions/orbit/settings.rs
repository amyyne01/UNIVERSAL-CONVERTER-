//! Settings, the third place, laid out as a spec sheet: where files go leads, as the one
//! reading in large figures; then each section is a band across the page, its name and
//! purpose in a left margin, its settings as ruled lines on the right. No cards: the rules
//! do the grouping, the margin makes it scannable. Sections, rows, wording, values, locks and
//! dimming all come from `settings_spec::sections`; this file only draws them. Now playing
//! draws the same rows (`words`, `control`) in its own frame.
use rune::{Color, El, Icon, PopupItem, ScrollSpec, TextStyle, col, el, icon, row, spacer};
use serde_json::Value;

use super::look::{BODY, BODY_M, CELL_TITLE, FIG, FIG_L, PAGE_TITLE, Pal, SMALL, SMALL_M, mix, txt};
use super::{Tone, button, keycap, pick_list};
use crate::actions;
use crate::copy::{self, found as f, settings as w};
use crate::format;
use crate::gate::{self, Confirm, Overlay, State};
use crate::icons;
use crate::keys;
use crate::settings_spec::{self, Kind, Row};

/// The left margin that carries each section's name.
const MARGIN: f32 = 216.0;
/// What a dimmed row (a switch above it is off) or a locked control is drawn at.
pub(crate) const DIM: f32 = 0.48;
// TODO(shared): State.transfer_cooldown, the local end date of the 7-day move cooldown.
const TRANSFER_COOLDOWN: Option<&str> = None;
/// The saved theme values, in the order of `copy::settings::THEMES`.
const THEME_VALUES: [&str; 3] = ["light", "dark", "system"];

pub fn view(s: &State, p: &Pal) -> El {
    let head = col().w_full().gap(4.0).child(txt(w::TITLE, PAGE_TITLE, p.text)).child(txt(w::SAVES, SMALL, p.text2));
    let sections = settings_spec::sections(s);
    let mut bands = col().w_full();
    let mut destination = None;
    for (i, sec) in sections.iter().enumerate() {
        if sec.title.is_empty() {
            destination = sec.rows.first().map(|r| reading(s, p, r));
            continue;
        }
        let mut lines: Vec<El> = sec.rows.iter().map(|r| line(s, p, r)).collect();
        if sec.title == w::PLAN {
            lines.push(machine(s, p));
        }
        bands = bands.child(band(p, sec.title, sec.hint, lines).key(keys::section(i)));
    }
    let mut page = col().w_full().max_w(1120.0).px(40.0).pt(32.0).pb(48.0).gap(32.0).child(head);
    if let Some(d) = destination {
        page = page.child(d);
    }
    col().key("settings").scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0).w_full().child(page.child(bands))
}

// ── Pieces ──────────────────────────────────────────────────────────────────

/// A band: a rule across, the name and hint in the margin, the lines on the right.
fn band(p: &Pal, title: &str, hint: Option<&str>, lines: Vec<El>) -> El {
    let mut name = col().w(MARGIN).gap(4.0).pt(20.0).child(txt(title.to_string(), CELL_TITLE, p.text));
    if let Some(h) = hint {
        name = name.child(txt(h.to_string(), SMALL, p.text2).wrap());
    }
    let mut right = col().grow(1.0).min_w(0.0);
    for (i, l) in lines.into_iter().enumerate() {
        right = right.child(l.border_sides([if i == 0 { 0.0 } else { 1.0 }, 0.0, 0.0, 0.0], p.line));
    }
    row().w_full().gap(32.0).pb(16.0).items_start().border_sides([1.0, 0.0, 0.0, 0.0], p.track).child(name).child(right)
}

/// A setting on a ruled line: its words, its control at the right.
fn line(s: &State, p: &Pal, r: &Row) -> El {
    let l = row().w_full().min_h(64.0).py(12.0).gap(24.0).center_y().child(words(s, p, r, BODY_M)).child(control(s, p, r));
    if r.dimmed { l.opacity(DIM) } else { l }
}

/// A row's name, its description (the armed sentence once a danger row is armed) and, on
/// Basic, the lock sentence. `name` is the preset's label size.
pub(crate) fn words(s: &State, p: &Pal, r: &Row, name: TextStyle) -> El {
    let danger = r.kind == Kind::Danger;
    let armed = danger && s.confirm == Some(confirm_of(r));
    let ink = if danger { p.error } else { p.text };
    let mut c = col().grow(1.0).min_w(0.0).gap(4.0);
    c = match status_figure(s, r) {
        Some(figure) => c.child(row().gap(8.0).center_y().child(txt(r.label, name, ink)).child(txt(figure, FIG, p.text2))),
        None => c.child(txt(r.label, name, ink)),
    };
    let desc = match r.armed {
        Some((sentence, _)) if armed => sentence.to_string(),
        _ => r.desc.clone(),
    };
    if !desc.is_empty() {
        c = c.child(txt(desc, SMALL, if armed { p.text } else { p.text2 }).wrap());
    }
    // Where files go, when the preset draws it as a plain row (Now playing).
    if r.kind == Kind::Folder {
        c = c.child(txt(shown(r), FIG, p.text2).truncate_middle().help(shown(r)));
    }
    if let (Some(d), true) = (TRANSFER_COOLDOWN, r.label == w::MOVE.0) {
        c = c.child(txt(w::cooldown_note(d), SMALL, p.warning).wrap());
    }
    if let Some(lock) = &r.lock {
        c = c.child(
            row()
                .w_full()
                .min_w(0.0)
                .gap(8.0)
                .items_start()
                .child(icon(&icons::LOCK_ORBIT, 16.0).color(p.text2))
                .child(txt(lock.clone(), SMALL, p.text2).wrap().grow(1.0).min_w(0.0)),
        );
    }
    c
}

/// The figure beside a status row's name: the app's version, the engine's, the plan's name.
fn status_figure(s: &State, r: &Row) -> Option<String> {
    match r.label {
        w::APP_NAME => Some(format!("v{}", env!("CARGO_PKG_VERSION"))),
        w::ENGINE_NAME => Some(s.engine.current.clone()),
        w::YOUR_PLAN => r.value.as_str().map(str::to_owned),
        _ => None,
    }
}

fn confirm_of(r: &Row) -> Confirm {
    if r.label == w::DEACTIVATE.0 { Confirm::Deactivate } else { Confirm::Move }
}

/// A status row's button calls its action; See plans has none yet.
// TODO(shared): `actions::see_plans(s)`.
fn wire(b: El, key: &str) -> El {
    match key {
        keys::CHANGE_FOLDER => b.on_press_tx(actions::choose_folder),
        keys::IMPORT_COOKIES => b.on_press_tx(actions::import_cookies),
        keys::CHECK_UPDATE => b.on_press(actions::check_update),
        keys::CHECK_ENGINE => b.on_press(actions::check_engine),
        keys::SHOW_SHORTCUTS => b.on_press(actions::show_shortcuts),
        keys::GET_SUPPORT => b.on_press_tx(actions::get_support),
        _ => b,
    }
}

/// A status row's button: its key and its glyph, by its word.
fn action(word: &'static str) -> (&'static str, Option<&'static Icon>) {
    match word {
        w::CHANGE => (keys::CHANGE_FOLDER, Some(&icons::FOLDER)),
        w::CHOOSE_FILE => (keys::IMPORT_COOKIES, Some(&icons::FOLDER)),
        w::CHECK_FOR_UPDATES => (keys::CHECK_UPDATE, Some(&icons::REFRESH)),
        w::ENGINE_CHECK => (keys::CHECK_ENGINE, Some(&icons::REFRESH)),
        w::SHOW_SHORTCUTS => (keys::SHOW_SHORTCUTS, None),
        w::GET_SUPPORT => (keys::GET_SUPPORT, Some(&icons::EXTERNAL)),
        w::LAYOUT_CHOOSE => (keys::LAYOUT_CHOOSE, None),
        _ => (keys::SEE_PLANS, None),
    }
}

/// The saved value of a row as text.
fn saved(r: &Row) -> String {
    match &r.value {
        Value::String(t) => t.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// What the select's option `id` is saved as: a number where the setting holds one.
// TODO(shared): `actions::set_select(s, key, id)`, which also tells what a locked pick needs.
fn typed(current: &Value, id: &str) -> Value {
    if current.is_number() { id.parse::<u64>().map_or(Value::Null, Value::from) } else { id.into() }
}

/// The value of a row as text: a select's label for the saved value, a typed value as it is.
fn shown(r: &Row) -> String {
    let saved = saved(r);
    match &r.kind {
        Kind::Select(options) => options.iter().find(|o| o.value == saved).map(|o| o.label.clone()).unwrap_or_default(),
        _ => saved,
    }
}

/// A row's control, by its kind. Locked controls are drawn dimmed; their row carries the
/// lock sentence (`words`).
pub(crate) fn control(s: &State, p: &Pal, r: &Row) -> El {
    // Every row drawn as a toggle, select or field has a config key.
    let key = keys::setting(r.key.unwrap_or_default());
    // A locked row (Basic) does not answer: it is dimmed and carries the lock sentence.
    let live = r.lock.is_none();
    let c = match &r.kind {
        Kind::Toggle => {
            let (cfg, on) = (r.key.unwrap_or_default(), r.value.as_bool().unwrap_or(false));
            let t = toggle(p, key, r.label, on);
            if live {
                t.on_press(move |s: &mut State| {
                    actions::set_setting(s, cfg, Value::Bool(!on));
                })
            } else {
                t
            }
        }
        Kind::Select(options) => {
            let locks = options.iter().any(|o| o.lock.is_some());
            let mut f = field(p, key.clone(), r.label, &shown(r), true, false);
            if locks {
                f = f.help(f::LOCK_SELECT_TITLE).child(icon(&icons::LOCK_ORBIT, 16.0).color(p.muted));
            }
            let now = saved(r);
            // A locked choice cannot be picked here: it waits for the action that says what Premium adds.
            let items =
                options.iter().map(|o| PopupItem::new(o.value.clone(), o.label.clone()).checked(o.value == now).enabled(o.lock.is_none()));
            let (cfg, current) = (r.key.unwrap_or_default(), r.value.clone());
            if live {
                pick_list(f, &key, items.collect(), &now, move |s, id| {
                    actions::set_setting(s, cfg, typed(&current, id));
                })
            } else {
                f
            }
        }
        // TODO(shared): a text edit per setting (`State` has none) that calls `actions::set_setting` on commit.
        Kind::Text => {
            let value = shown(r);
            match r.placeholder {
                Some(hint) if value.is_empty() => field(p, key, r.label, hint, false, true),
                _ => field(p, key, r.label, &value, false, false),
            }
        }
        Kind::Time => field(p, key, r.label, &shown(r), false, false).min_w(96.0),
        Kind::Days => days(p, r, live),
        Kind::Theme => theme(p, r),
        Kind::Palette => swatches(s, p),
        Kind::Layout => layout(s, p, r),
        Kind::Folder | Kind::Status => match r.button {
            Some(b) => {
                let (k, glyph) = action(b);
                wire(button(p, k, b, glyph, Tone::Ghost), k)
            }
            // Premium's plan: the good mark.
            None => el().size(8.0, 8.0).radius(4.0).fill(p.success),
        },
        Kind::Danger => danger(s, p, r),
    };
    if live { c } else { c.opacity(DIM) }
}

fn toggle(p: &Pal, key: String, label: &str, on: bool) -> El {
    el().key(key)
        .size(36.0, 20.0)
        .radius(10.0)
        .fill(if on { p.text } else { p.raised })
        .border(1.0, if on { p.text } else { p.track })
        .focusable(true)
        .role("switch")
        .label(label.to_string())
        .checked(on)
        .cursor(rune::Cursor::Pointer)
        .child(el().size(12.0, 12.0).radius(6.0).fill(if on { p.deep } else { p.muted }).absolute(if on { 19.0 } else { 3.0 }, 3.0))
}

/// A field showing its value: a select (sans, with a caret) or a typed value (mono).
/// A muted value is a placeholder.
fn field(p: &Pal, key: String, label: &str, value: &str, select: bool, muted: bool) -> El {
    let mut f = row()
        .key(key)
        .h(32.0)
        .min_w(if select { 176.0 } else { 200.0 })
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.card)
        .border(1.0, p.track)
        .hover(p.raised)
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .role(if select { "combobox" } else { "textbox" })
        .label(label.to_string())
        .child(txt(value.to_string(), if select { SMALL } else { FIG }, if muted { p.muted } else { p.text }).single_line());
    if select {
        f = f.child(spacer()).child(icon(&icons::CARET_DOWN, 16.0).color(p.muted));
    }
    f
}

/// Light, Dark or System, as the saved theme says.
fn theme(p: &Pal, r: &Row) -> El {
    let now = THEME_VALUES.iter().position(|t| Some(*t) == r.value.as_str()).unwrap_or(2);
    let mut group =
        row().key(keys::setting("theme")).gap(4.0).padding(4.0).radius(8.0).border(1.0, p.track).role("radiogroup").label(r.label);
    for (i, name) in w::THEMES.into_iter().enumerate() {
        let on = i == now;
        let mut seg = el()
            .key(keys::theme(i))
            .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| actions::set_theme(s, THEME_VALUES[i], tx))
            .h(24.0)
            .px(12.0)
            .center()
            .radius(4.0)
            .role("radio")
            .checked(on)
            .focusable(true)
            .cursor(rune::Cursor::Pointer);
        seg = if on { seg.fill(p.raised) } else { seg.hover(p.hover) };
        group = group.child(seg.child(txt(name, if on { SMALL_M } else { SMALL }, if on { p.text } else { p.text2 })));
    }
    group
}

/// Orbit's own colours first, then the app's seven palettes in Settings' order.
fn swatches(s: &State, p: &Pal) -> El {
    let mut group = row().gap(4.0).role("radiogroup").label(w::PALETTE_ARIA);
    for i in 0..=crate::palette::NAMES.len() {
        let name = if i == 0 { copy::PRESET_ORBIT } else { crate::palette::NAMES[i - 1] };
        let sw = Pal::new(i, p.dark);
        let on = i == s.palette;
        group = group.child(
            el().key(keys::palette(i))
                .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| actions::set_palette(s, i, tx))
                .size(28.0, 28.0)
                .radius(14.0)
                .center()
                .border(1.0, if on { p.text } else { Color::TRANSPARENT })
                .role("radio")
                .checked(on)
                .label(name.to_string())
                .help(name.to_string())
                .focusable(true)
                .cursor(rune::Cursor::Pointer)
                .child(
                    el().size(20.0, 20.0)
                        .radius(10.0)
                        .center()
                        .fill(sw.bg)
                        .border(1.0, mix(sw.bg, sw.text, 0.2))
                        .child(el().size(8.0, 8.0).radius(4.0).fill(sw.accent)),
                ),
        );
    }
    group
}

/// The preset on show, then the picker's shortcut and the button that opens it.
fn layout(s: &State, p: &Pal, r: &Row) -> El {
    let word = r.button.unwrap_or(w::LAYOUT_CHOOSE);
    row()
        .gap(12.0)
        .center_y()
        .child(txt(gate::PRESETS[gate::preset(s)].0, SMALL_M, p.text))
        .child(row().center_y().gap(4.0).child(keycap(p, copy::KEY_CTRL)).child(keycap(p, ".")))
        .child(button(p, keys::LAYOUT_CHOOSE, word, None, Tone::Ghost).on_press(|s: &mut State| actions::open(s, Overlay::Picker)))
}

/// The schedule's days, Sunday first; the saved ones lit.
fn days(p: &Pal, r: &Row, live: bool) -> El {
    let chosen: Vec<u64> = r.value.as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
    let mut group = row().gap(4.0).role("group").label(r.label);
    for (i, (full, short)) in w::DAYS.iter().zip(w::DAYS_SHORT).enumerate() {
        let on = chosen.contains(&(i as u64));
        let day = el().key(keys::day(i));
        group = group.child(
            if live { day.on_press(move |s: &mut State| actions::toggle_day(s, i as u8)) } else { day }
                .size(28.0, 28.0)
                .radius(14.0)
                .center()
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .border(1.0, if on { p.track } else { p.line })
                .focusable(true)
                .role("checkbox")
                .checked(on)
                .label(full.to_string())
                .help(full.to_string())
                .cursor(rune::Cursor::Pointer)
                .child(txt(short, FIG, if on { p.text } else { p.muted })),
        );
    }
    group
}

/// Deactivate or Move: a caret until armed, then Cancel and the red confirm.
fn danger(s: &State, p: &Pal, r: &Row) -> El {
    let c = confirm_of(r);
    match r.armed {
        Some((_, yes)) if s.confirm == Some(c) => row()
            .gap(8.0)
            .center_y()
            .child(button(p, keys::CONFIRM_NO, w::CANCEL, None, Tone::Ghost).on_press(actions::disarm))
            .child(button(p, keys::CONFIRM_YES, yes, None, Tone::Danger).on_press(actions::confirm)),
        _ => el()
            .key(keys::arm(c))
            .on_press(move |s: &mut State| actions::arm(s, c))
            .size(32.0, 32.0)
            .center()
            .radius(8.0)
            .hover(p.hover)
            .focusable(true)
            .role("button")
            .label(r.label)
            .cursor(rune::Cursor::Pointer)
            .child(icon(&icons::CARET_RIGHT, 16.0).color(p.muted)),
    }
}

// ── The two lines of their own ──────────────────────────────────────────────

/// The one large reading: where every file lands. The page's one bordered surface.
fn reading(s: &State, p: &Pal, r: &Row) -> El {
    let dir = shown(r);
    let (folder, name) = dir.split_at(dir.len() - format::file_name(&dir).len());
    row()
        .w_full()
        .gap(24.0)
        .px(24.0)
        .py(20.0)
        .radius(12.0)
        .fill(p.card)
        .border(1.0, p.line)
        .center_y()
        .child(el().size(40.0, 40.0).radius(20.0).center().border(1.0, p.track).child(icon(&icons::FOLDER, 16.0).color(p.text2)))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(txt(r.label, SMALL, p.text2))
                .child(row().child(txt(folder.to_string(), FIG_L, p.text2)).child(txt(name.to_string(), FIG_L, p.text))),
        )
        .child(control(s, p, r))
}

/// What a licence key binds to on this computer, in figures.
fn machine(s: &State, p: &Pal) -> El {
    row()
        .w_full()
        .min_h(48.0)
        .py(12.0)
        .gap(24.0)
        .center_y()
        .help(w::MACHINE_ID_TITLE)
        .child(txt(w::MACHINE_ID, BODY, p.text2))
        .child(spacer())
        .child(txt(s.machine_id.clone(), FIG, p.text2).single_line())
}
