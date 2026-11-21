//! The first-launch setup (owner, 2026-11-21): right after the splash, before any preset is
//! used, the user picks the layout, the look and the download defaults, so the app does not
//! drop them into a layout they never chose. One shared brand screen, not six: it runs before a
//! preset is chosen, so it wears the splash's brand (the Orbit-dialect planet, the palette's
//! Aurora-family roles, Geist), while its six layout cards are each preset's own picture.
//!
//! Every control calls the shared action Settings uses (`set_preset`, `set_theme`,
//! `set_palette`, `pick_setting`, `set_setting`, `choose_folder`), so a choice applies live and
//! the core and the settings files stay the one source. Finish and Skip setup write only the
//! Rust-only `onboarded` flag (`settings-rust.json`); Skip changes nothing else, so an Electron
//! user's own `config.json` is never overwritten with defaults.
use rune::{Color, Cx, El, Family, TextStyle, Tx, canvas, col, el, icon, row, text};
use serde_json::{Value, json};

use crate::copy::{onboarding as w, settings as c};
use crate::gate::{Nav, State};
use crate::settings_spec::{self, Kind, Row};
use crate::{actions, icons, keys, live, motion, palette, picker, splash};

/// Layout, Look, Essentials.
pub const STEPS: usize = 3;

/// The Essentials rows, by config key, in the order shown: Settings' own rows, so their labels,
/// values, clamps and Basic locks are the same ones.
pub const ESSENTIALS: [&str; 6] =
    ["outputDir", "defaultFormat", "defaultQuality", "defaultVideoQuality", "clipboardWatch", "showNotifications"];

/// The setup on show.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Onboarding {
    /// 0 Layout, 1 Look, 2 Essentials.
    pub step: usize,
    /// Esc on the first step asked whether to skip.
    pub confirm_skip: bool,
}

// ── Actions

/// Opens the setup on its first step (the first launch, and Settings' Run setup again).
pub fn open(s: &mut State) {
    actions::close(s);
    actions::disarm(s);
    s.onboarding = Some(Onboarding::default());
    s.sync();
}

/// Next, or Finish on the last step.
pub fn next(s: &mut State) {
    let Some(ob) = s.onboarding.as_mut() else { return };
    ob.confirm_skip = false;
    if ob.step + 1 < STEPS {
        ob.step += 1;
    } else {
        finish(s);
    }
}

/// Back; on the first step it asks whether to skip (a second Esc skips).
pub fn back(s: &mut State) {
    let Some(ob) = s.onboarding.as_mut() else { return };
    if ob.confirm_skip {
        return finish(s);
    }
    if ob.step == 0 {
        ob.confirm_skip = true;
    } else {
        ob.step -= 1;
    }
}

/// The skip question answered no.
pub fn keep_going(s: &mut State) {
    if let Some(ob) = s.onboarding.as_mut() {
        ob.confirm_skip = false;
    }
}

/// Finish and Skip setup alike: what was chosen is already written; the flag is set and the
/// setup fades into the chosen preset's Home.
pub fn finish(s: &mut State) {
    s.onboarding = None;
    live::tell(s, |core| {
        core.set_own_setting("onboarded", json!(true));
    });
    actions::go(s, Nav::Home);
}

/// The keys while the setup is up: Esc goes back (with nothing focused Rune hands it here before
/// the modal sees it); every command chord (Ctrl+1..3, Ctrl+K, Ctrl+., Ctrl+/) is swallowed. Tab
/// and the rest go on to Rune; Enter presses Next, the setup's default button.
pub fn on_key(s: &mut State, ev: &rune::KeyEvent) -> bool {
    if ev.key == rune::Key::Escape {
        back(s);
        return true;
    }
    matches!(ev.key, rune::Key::Command(_))
}

// ── Drawing

const W: f32 = 840.0;
const FAMILY: Family = Family::Named("Geist");
const MONO: Family = Family::Named("Geist Mono");

fn sans(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).family(FAMILY)
}

fn mono(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).family(MONO)
}

/// The splash's roles: palette 0 (a look's own mood) wears Aurora, Electron's own.
fn roles(s: &State, dark: bool) -> palette::Roles {
    palette::roles(s.palette.saturating_sub(1), dark)
}

/// The setup over the whole window, once the splash is gone.
pub fn view(s: &State, cx: &mut Cx<State>) -> Option<El> {
    let ob = s.onboarding.as_ref().filter(|_| s.splash.is_none())?;
    let dark = s.dark(cx);
    let r = roles(s, dark);
    let (title, line) = [w::STEP_LAYOUT, w::STEP_LOOK, w::STEP_ESSENTIALS][ob.step];
    let body = match ob.step {
        0 => layouts(s, dark),
        1 => look(s, &r, dark),
        _ => essentials(s, &r),
    };
    let step = col()
        .key(format!("onboard-step-{}", ob.step))
        .w(W)
        .gap(6.0)
        .child(text(title).font(sans(26.0, 500.0)).color(r.text_primary))
        .child(text(line).font(sans(14.0, 400.0)).color(r.text_secondary).mb(22.0))
        .child(body);
    let sheet = col()
        .w_full()
        .h_full()
        .pt(crate::TITLEBAR)
        .px(40.0)
        .pb(28.0)
        .child(head(ob, &r))
        .child(col().grow(1.0).w_full().center().child(motion::onboarding_step(step)))
        .child(foot(ob, &r));
    Some(motion::onboarding(
        el().key(keys::ONBOARDING)
            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .fill(r.bg_primary)
            .modal_tx(false, |s: &mut State, _: &mut Tx| back(s))
            .child(sheet),
    ))
}

/// The brand at the leading end, the step indicator at the trailing one.
fn head(ob: &Onboarding, r: &palette::Roles) -> El {
    let mut bars = row().gap(6.0);
    for i in 0..STEPS {
        let lit = i <= ob.step;
        bars = bars.child(el().size(28.0, 3.0).radius(1.5).fill(if lit { r.accent } else { r.border }));
    }
    row()
        .w_full()
        .center_y()
        .child(canvas(splash::mark(28.0, r.text_primary, r.accent, r.bg_primary)).size(28.0, 28.0).mr(10.0))
        .child(text(crate::copy::splash::WORD_BRAND).font(mono(13.0, 500.0)).color(r.text_primary).mr(8.0))
        .child(text(crate::copy::splash::WORD_NAME).font(mono(13.0, 400.0)).color(r.text_secondary))
        .child(el().grow(1.0))
        .child(text(w::step_of(ob.step + 1, STEPS)).font(mono(12.0, 400.0)).color(r.text_muted).mr(12.0))
        .child(bars.role("progressbar").label(w::step_of(ob.step + 1, STEPS)))
}

/// Skip setup at the leading end, the keys line, Back and Next (or the skip question).
fn foot(ob: &Onboarding, r: &palette::Roles) -> El {
    let foot = row().w_full().center_y().gap(12.0);
    if ob.confirm_skip {
        return foot
            .child(text(w::SKIP_CONFIRM).font(sans(14.0, 400.0)).color(r.text_primary))
            .child(el().grow(1.0))
            .child(ghost(keys::ONBOARD_SKIP_NO, w::SKIP_NO, r).on_press(keep_going))
            .child(solid(keys::ONBOARD_SKIP_YES, w::SKIP_YES, r).default_button().on_press(finish));
    }
    let last = ob.step + 1 == STEPS;
    let mut foot = foot
        .child(ghost(keys::ONBOARD_SKIP, w::SKIP_ALL, r).on_press(finish))
        .child(el().grow(1.0))
        .child(text(w::KEYS_HINT).font(mono(11.0, 400.0)).color(r.text_muted))
        .child(el().grow(1.0));
    if ob.step > 0 {
        foot = foot.child(ghost(keys::ONBOARD_BACK, w::BACK, r).on_press(back));
    }
    foot.child(solid(keys::ONBOARD_NEXT, if last { w::FINISH } else { w::NEXT }, r).default_button().on_press(next))
}

fn button(key: &str, words: &str) -> El {
    el().key(key).h(36.0).px(16.0).center().radius(8.0).cursor(rune::Cursor::Pointer).role("button").label(words)
}

fn ghost(key: &str, words: &str, r: &palette::Roles) -> El {
    button(key, words).border(1.0, r.border_control).hover(r.bg_hover).child(text(words).font(sans(14.0, 500.0)).color(r.text_primary))
}

fn solid(key: &str, words: &str, r: &palette::Roles) -> El {
    button(key, words).fill(r.accent).hover(r.accent_hover).child(text(words).font(sans(14.0, 500.0)).color(r.on_accent))
}

/// The six presets, each card its own preset's picture (the picker's tile).
fn layouts(s: &State, dark: bool) -> El {
    let line = |from: usize| row().gap(48.0).children((from..from + 3).map(|i| picker::tile(s, i, dark)));
    col().gap(24.0).child(line(0)).child(line(3))
}

/// Light, Dark, System and the seven palettes.
fn look(s: &State, r: &palette::Roles, dark: bool) -> El {
    let theme = s.settings.get("theme").and_then(Value::as_str).unwrap_or("system");
    let mut themes =
        row().gap(4.0).padding(4.0).radius(10.0).fill(r.bg_secondary).border(1.0, r.border_soft).role("radiogroup").label(c::THEME.0);
    for (i, (name, value)) in c::THEMES.into_iter().zip(["light", "dark", "system"]).enumerate() {
        let on = theme == value;
        themes = themes.child(
            el().key(keys::onboard_theme(i))
                .h(32.0)
                .px(18.0)
                .center()
                .radius(7.0)
                .fill(if on { r.bg_surface } else { Color::TRANSPARENT })
                .hover(r.bg_hover)
                .cursor(rune::Cursor::Pointer)
                .role("radio")
                .checked(on)
                .label(name)
                .on_press_tx(move |s: &mut State, tx: &mut Tx| actions::set_theme(s, value, tx))
                .child(text(name).font(sans(14.0, if on { 500.0 } else { 400.0 })).color(if on {
                    r.text_primary
                } else {
                    r.text_secondary
                })),
        );
    }
    let mut swatches = row().gap(14.0).role("radiogroup").label(c::PALETTE_ARIA);
    for (i, name) in palette::NAMES.into_iter().enumerate() {
        let at = i + 1;
        let on = s.palette == at;
        let p = palette::roles(i, dark);
        let card = el()
            .size(96.0, 64.0)
            .radius(10.0)
            .fill(p.bg_primary)
            .border(if on { 2.0 } else { 1.0 }, if on { r.accent } else { r.border })
            .padding(12.0)
            .child(
                col()
                    .gap(6.0)
                    .child(el().size(28.0, 4.0).radius(2.0).fill(p.accent))
                    .child(el().size(52.0, 3.0).radius(1.5).fill(p.text_secondary.alpha(0.6)))
                    .child(el().size(40.0, 3.0).radius(1.5).fill(p.text_muted.alpha(0.5))),
            );
        swatches = swatches.child(
            col()
                .key(keys::onboard_palette(at))
                .gap(8.0)
                .items_center()
                .cursor(rune::Cursor::Pointer)
                .role("radio")
                .checked(on)
                .label(name)
                .on_press_tx(move |s: &mut State, tx: &mut Tx| actions::set_palette(s, at, tx))
                .child(card)
                .child(text(name).font(sans(13.0, if on { 500.0 } else { 400.0 })).color(if on {
                    r.text_primary
                } else {
                    r.text_secondary
                })),
        );
    }
    col().gap(10.0).child(caption(c::THEME, r)).child(themes.self_start().mb(20.0)).child(caption(c::PALETTE, r)).child(swatches)
}

fn caption((label, desc): (&str, &str), r: &palette::Roles) -> El {
    row()
        .gap(10.0)
        .center_y()
        .child(text(label.to_owned()).font(sans(14.0, 500.0)).color(r.text_primary))
        .child(text(desc.to_owned()).font(sans(13.0, 400.0)).color(r.text_muted))
}

/// Settings' own rows for the essentials, as Settings has them now.
pub fn essential_rows(s: &State) -> Vec<Row> {
    let all: Vec<Row> = settings_spec::sections(s).into_iter().flat_map(|sec| sec.rows).collect();
    ESSENTIALS.iter().filter_map(|k| all.iter().find(|r| r.key == Some(*k)).cloned()).collect()
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(t) => t.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn essentials(s: &State, r: &palette::Roles) -> El {
    let mut list = col().w_full().gap(4.0).radius(12.0).fill(r.bg_secondary).border(1.0, r.border_soft).px(20.0).py(8.0);
    for (i, row_) in essential_rows(s).into_iter().enumerate() {
        if i > 0 {
            list = list.child(el().w_full().h(1.0).fill(r.border_soft));
        }
        list = list.child(setting(row_, r));
    }
    list
}

/// One row: its Settings label on the left, its control on the right.
fn setting(sr: Row, r: &palette::Roles) -> El {
    let key = sr.key.unwrap_or_default();
    let name = col()
        .gap(2.0)
        .grow(1.0)
        .min_w(0.0)
        .child(text(sr.label).font(sans(14.0, 500.0)).color(r.text_primary))
        .children((!sr.desc.is_empty()).then(|| text(sr.desc.clone()).font(sans(12.0, 400.0)).color(r.text_muted).wrap()));
    let control = match &sr.kind {
        Kind::Folder => {
            let folder = text_of(&sr.value);
            row()
                .gap(12.0)
                .center_y()
                .child(
                    text(folder.clone())
                        .font(mono(12.0, 400.0))
                        .color(r.text_secondary)
                        .single_line()
                        .truncate_head()
                        .max_w(360.0)
                        .help(folder),
                )
                .child(ghost(keys::ONBOARD_FOLDER, sr.button.unwrap_or(c::CHANGE), r).on_press_tx(actions::choose_folder))
        }
        Kind::Select(options) => {
            let now = text_of(&sr.value);
            let mut chips = row().gap(4.0).flex_wrap().justify_end().max_w(640.0).role("radiogroup").label(sr.label);
            for o in options {
                let on = o.value == now;
                let value = o.value.clone();
                let mut chip = row()
                    .key(keys::onboard_setting(key, Some(&o.value)))
                    .h(30.0)
                    .px(12.0)
                    .gap(6.0)
                    .center_y()
                    .radius(7.0)
                    .fill(if on { r.accent } else { Color::TRANSPARENT })
                    .border(1.0, if on { r.accent } else { r.border })
                    .hover(if on { r.accent_hover } else { r.bg_hover })
                    .cursor(rune::Cursor::Pointer)
                    .role("radio")
                    .checked(on)
                    .label(o.label.clone())
                    .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                        actions::pick_setting(s, tx, key, &value);
                    });
                if let Some(why) = &o.lock {
                    chip = chip.help(why.clone()).child(icon(&icons::LOCK_ORBIT, 12.0).color(r.text_muted));
                }
                chips = chips.child(chip.child(text(o.label.clone()).font(sans(13.0, if on { 500.0 } else { 400.0 })).color(if on {
                    r.on_accent
                } else if o.lock.is_some() {
                    r.text_muted
                } else {
                    r.text_primary
                })));
            }
            chips
        }
        Kind::Toggle => {
            let on = sr.value == json!(true);
            el().key(keys::onboard_setting(key, None))
                .size(40.0, 22.0)
                .radius(11.0)
                .fill(if on { r.accent } else { r.border_control })
                .cursor(rune::Cursor::Pointer)
                .role("switch")
                .checked(on)
                .label(sr.label)
                .on_press(move |s: &mut State| {
                    let now = s.settings.get(key) == Some(&json!(true));
                    actions::set_setting(s, key, json!(!now));
                })
                .child(el().absolute(if on { 20.0 } else { 2.0 }, 2.0).size(18.0, 18.0).radius(9.0).fill(if on {
                    r.on_accent
                } else {
                    r.bg_primary
                }))
        }
        _ => el(),
    };
    row().w_full().min_h(56.0).gap(24.0).center_y().child(name).child(control)
}

#[cfg(test)]
mod tests;
