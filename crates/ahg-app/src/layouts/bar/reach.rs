//! Settings' sections and the bar's command mode (Ctrl+K). The section list
//! is the same in both: each section with its state where it has one, so the
//! list doubles as a status board. In command mode it drops out of the bar
//! with the places, the search scopes and the actions under it, and the
//! highlighted entry's sheet is shown beside it before Enter is pressed.
use rune::{Color, Cursor, El, Icon, ScrollSpec, Tx, col, el, icon, row, spacer};

use super::keycap;
use crate::actions::{self, Command};
use crate::copy;
use crate::copy::settings as w;
use crate::directions::signal::parts::{Spec, label, mono, platform_icon, sans};
use crate::directions::signal::settings::panel_of;
use crate::directions::signal::theme::{Pal, source_color};
use crate::format;
use crate::gate::{Overlay, State};
use crate::icons;
use crate::keys;
use crate::settings_spec::{self, Section};

const PANEL_H: f32 = 520.0;
const LIST_W: f32 = 336.0;
// The Settings sections, in the old app's order. Destination had no heading
// there; its row's label stands for it.
const SECTIONS: [(&str, &Icon); 9] = [
    (w::SAVING_TO, &icons::FOLDER),
    (w::DEFAULTS, &icons::DOWNLOAD),
    (w::MORE, &icons::SETTINGS),
    (w::APPEARANCE, &icons::SETTINGS),
    (w::SCHEDULE, &icons::CLOCK),
    (w::SOFTWARE, &icons::REFRESH),
    (w::ADVANCED, &icons::SETTINGS),
    (w::PLAN, &icons::CHECK_SIGNAL),
    (w::HELP, &icons::ALERT),
];

pub fn panel(s: &State, p: &Pal) -> El {
    let footer = row()
        .w_full()
        .h(40.0)
        .px(16.0)
        .gap(16.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .child(hint(copy::PALETTE_FOOTER_NAVIGATE_CAP, copy::PALETTE_FOOTER_NAVIGATE, p))
        .child(hint(copy::PALETTE_FOOTER_SELECT_CAP, copy::PALETTE_FOOTER_SELECT, p))
        .child(hint(copy::PALETTE_FOOTER_CLOSE_CAP, copy::PALETTE_FOOTER_CLOSE, p))
        .child(spacer())
        .child(label(w::SAVES, sans(12.0, 400.0), p.muted));
    let list = col()
        .w(LIST_W)
        .h_full()
        .min_h(0.0)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(command_list(s, p));
    // The highlighted entry's sheet, before Enter is pressed.
    let sections = settings_spec::sections(s);
    let open = sections.get(s.section).map(|sec| crumbed(s, sec, p)).unwrap_or_else(el);
    col()
        .h(PANEL_H)
        .radius(12.0)
        .fill(p.surface)
        .border(1.0, p.line_strong)
        .shadow(16.0, 48.0, Color::BLACK.alpha(if p.dark { 0.45 } else { 0.14 }))
        .clip_rounded(12.0)
        .role("dialog")
        .label(copy::PALETTE_ARIA)
        .child(row().w_full().grow(1.0).min_h(0.0).child(list).child(open))
        .child(footer)
}

fn hint(cap: &str, words: &str, p: &Pal) -> El {
    row().gap(4.0).center_y().child(keycap(cap, p)).child(label(words.trim(), sans(12.0, 400.0), p.muted))
}

/// The Settings sections, each with its state, and This month. `lit`: the one in view is marked
/// (Settings); the command panel marks its highlighted command instead.
pub(crate) fn section_list(s: &State, lit: bool, p: &Pal) -> El {
    let month = s.stats.as_ref().map(|st| st.current.files as u64).unwrap_or(0);
    let v = Spec::new(s);
    let status = |name: &str| -> String {
        match name {
            w::SAVING_TO => format::file_name(&v.text("outputDir")).to_owned(),
            w::DEFAULTS => format!("{} · {}", v.text("defaultFormat").to_uppercase(), v.pick("defaultQuality")),
            w::SCHEDULE => v.text("scheduleTime"),
            w::SOFTWARE => env!("CARGO_PKG_VERSION").into(),
            w::PLAN => v.text_where(w::YOUR_PLAN),
            _ => String::new(),
        }
    };
    let mut l = col().w_full();
    for (at, (name, i)) in SECTIONS.into_iter().enumerate() {
        l = l.child(entry(Some(i), None, name, &status(name), &[], lit && at == s.section, p).key(keys::section(at)));
    }
    l.child(entry(Some(&icons::TRACK), None, copy::CELL_THIS_MONTH, &format!("{month} {}", copy::file_word(month)), &[], false, p))
}

/// The palette's commands that match what is typed, in their groups, then the sections.
fn command_list(s: &State, p: &Pal) -> El {
    let mut l = col().w_full().padding(8.0);
    let all = actions::commands(s);
    let found = actions::palette_filter(s);
    if found.is_empty() {
        l = l.child(row().w_full().h(48.0).px(8.0).center_y().child(label(copy::PALETTE_NO_MATCH, sans(13.0, 400.0), p.text2)));
    }
    let mut last = "";
    for (at, c) in found.iter().enumerate() {
        if c.group() != last {
            last = c.group();
            l = l.child(group(last, p));
        }
        let (glyph, tint) = match c {
            Command::Scope(src) => (Some(platform_icon(*src)), Some(source_color(p, *src))),
            _ => (None, None),
        };
        let hint = c.hint().unwrap_or_default();
        let caps: Vec<&str> = hint.split('+').filter(|k| !k.is_empty()).collect();
        let index = all.iter().position(|x| x == c).unwrap_or(0);
        let c = *c;
        let row = entry(glyph, tint, &c.label(s), "", &caps, at == s.command.highlight, p).key(keys::command(index));
        // Pressable in a real palette session only: the `nav` scene draws the same list at rest.
        l = l.child(if s.overlay == Overlay::Palette {
            row.on_press_tx(move |s: &mut State, tx: &mut Tx| actions::run(s, tx, c))
        } else {
            row
        });
    }
    l.child(group(copy::NAV_SETTINGS, p)).child(section_list(s, false, p))
}

fn group(name: &str, p: &Pal) -> El {
    row().w_full().h(32.0).px(8.0).items_end().pb(4.0).child(label(name, sans(12.0, 500.0), p.muted))
}

fn entry(i: Option<&'static Icon>, tint: Option<Color>, name: &str, status: &str, keys: &[&str], on: bool, p: &Pal) -> El {
    let glyph = el().size(16.0, 16.0);
    let glyph = match i {
        Some(i) => glyph.child(icon(i, 16.0).color(tint.unwrap_or(if on { p.text } else { p.muted }))),
        None => glyph,
    };
    let mut r = row()
        .w_full()
        .h(32.0)
        .px(8.0)
        .gap(12.0)
        .center_y()
        .radius(6.0)
        .hover(p.hover)
        .cursor(Cursor::Pointer)
        .role("option")
        .child(glyph)
        .child(
            label(name, sans(13.0, if on { 500.0 } else { 400.0 }), if on { p.text } else { p.text2 })
                .single_line()
                .truncate()
                .grow(1.0)
                .min_w(0.0),
        );
    if !status.is_empty() {
        r = r.child(label(status, mono(12.0, 400.0), p.muted));
    }
    if !keys.is_empty() {
        let mut caps = row().gap(4.0);
        for k in keys {
            caps = caps.child(keycap(k, p));
        }
        r = r.child(caps);
    }
    if on {
        // The highlighted entry: raised, with one lit segment at its edge.
        r = r.fill(p.raised).child(el().size(2.0, 16.0).radius(1.0).fill(p.accent).absolute(0.0, 8.0));
    }
    r
}

/// A section as One bar shows it: its name and hint over its panel of rows.
pub(crate) fn sheet(s: &State, sec: &Section, p: &Pal) -> El {
    let mut c = col().w_full().gap(4.0);
    if !sec.title.is_empty() {
        c = c.child(label(sec.title, sans(16.0, 500.0), p.text));
    }
    if let Some(h) = sec.hint {
        c = c.child(label(h, sans(13.0, 400.0), p.text2).wrap());
    }
    c.child(panel_of(s, sec, p).mt(12.0))
}

/// The sheet in the bar's panel, which has no other title: where it sits, then the sheet.
fn crumbed(s: &State, sec: &Section, p: &Pal) -> El {
    let name = SECTIONS.get(s.section).map_or(sec.title, |(n, _)| *n);
    let crumb = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .mb(8.0)
        .child(label(copy::NAV_SETTINGS, sans(12.0, 400.0), p.muted))
        .child(label("/", sans(12.0, 400.0), p.muted))
        .child(label(name, sans(12.0, 400.0), p.text2));
    col().grow(1.0).h_full().min_w(0.0).px(24.0).py(20.0).scroll(ScrollSpec { stick_bottom: false }).child(crumb).child(sheet(s, sec, p))
}
