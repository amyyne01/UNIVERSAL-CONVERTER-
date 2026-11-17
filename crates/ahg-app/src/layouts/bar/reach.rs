//! Settings' sections and the bar's command mode (Ctrl+K). The section list
//! is the same in both: each section with its state where it has one, so the
//! list doubles as a status board. In command mode it drops out of the bar
//! with the places, the search scopes and the actions under it, and the
//! highlighted entry's sheet is shown beside it before Enter is pressed.
use ahg_engine::types::SourcePlatform;
use rune::{Color, Cursor, El, Icon, ScrollSpec, col, el, icon, row, spacer};

use super::keycap;
use crate::copy;
use crate::copy::settings as w;
use crate::directions::signal::parts::{Spec, label, mono, platform_icon, sans, stack};
use crate::directions::signal::theme::{Pal, source_color};
use crate::format;
use crate::gate::State;
use crate::icons;

const PANEL_H: f32 = 520.0;
const LIST_W: f32 = 336.0;
/// The section shown open.
pub(crate) const PICKED: &str = w::SCHEDULE;
pub(crate) const SAVES_ITSELF: &str = w::SAVES;

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
        .child(label(SAVES_ITSELF, sans(12.0, 400.0), p.muted));
    let list = col()
        .w(LIST_W)
        .h_full()
        .min_h(0.0)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(command_list(s, p));
    col()
        .h(PANEL_H)
        .radius(12.0)
        .fill(p.surface)
        .border(1.0, p.line_strong)
        .shadow(16.0, 48.0, Color::BLACK.alpha(if p.dark { 0.45 } else { 0.14 }))
        .clip_rounded(12.0)
        .role("dialog")
        .label(copy::PALETTE_ARIA)
        .child(row().w_full().grow(1.0).min_h(0.0).child(list).child(schedule_sheet(&Spec::new(s), p, true).px(24.0).py(20.0)))
        .child(footer)
}

fn hint(cap: &str, words: &str, p: &Pal) -> El {
    row().gap(4.0).center_y().child(keycap(cap, p)).child(label(words.trim(), sans(12.0, 400.0), p.muted))
}

/// The Settings sections, each with its state, and This month.
pub(crate) fn section_list(s: &State, p: &Pal) -> El {
    let month = s.stats.as_ref().map(|st| st.current.files as u64).unwrap_or(0);
    let v = Spec::new(s);
    let status = |name: &str| -> String {
        match name {
            w::SAVING_TO => format::file_name(&v.text("outputDir")).to_owned(),
            w::DEFAULTS => format!("{} · {}", v.text("defaultFormat").to_uppercase(), v.pick("defaultQuality")),
            w::SCHEDULE => v.text("scheduleTime"),
            w::SOFTWARE => env!("CARGO_PKG_VERSION").into(),
            w::PLAN => w::PLAN_PREMIUM.0.into(),
            _ => String::new(),
        }
    };
    let mut l = col().w_full();
    for (name, i) in SECTIONS {
        l = l.child(entry(Some(i), None, name, &status(name), &[], name == PICKED, p));
    }
    l.child(entry(Some(&icons::TRACK), None, copy::CELL_THIS_MONTH, &format!("{month} {}", copy::file_word(month)), &[], false, p))
}

fn command_list(s: &State, p: &Pal) -> El {
    let mut l = col()
        .w_full()
        .padding(8.0)
        .child(group(copy::NAV_SETTINGS, p))
        .child(section_list(s, p))
        .child(group(copy::PALETTE_GROUP_NAVIGATE, p));
    for (i, name) in [copy::NAV_HOME, copy::NAV_DOWNLOADS, copy::NAV_SETTINGS].into_iter().enumerate() {
        l = l.child(entry(None, None, &copy::palette_go_to(name), "", &[copy::KEY_CTRL, ["1", "2", "3"][i]], false, p));
    }
    for (src, name) in [
        (SourcePlatform::Youtube, copy::NAV_YOUTUBE),
        (SourcePlatform::Spotify, copy::NAV_SPOTIFY),
        (SourcePlatform::Soundcloud, copy::NAV_SOUNDCLOUD),
        (SourcePlatform::Instagram, copy::NAV_REELS),
    ] {
        l = l.child(entry(Some(platform_icon(src)), Some(source_color(p, src)), name, "", &[], false, p));
    }
    l = l.child(group(copy::PALETTE_GROUP_ACTIONS, p));
    let actions: [(&str, &[&str]); 9] = [
        (copy::PALETTE_PASTE_AND_DOWNLOAD, &[copy::KEY_CTRL, copy::REST_KEY_V]),
        (if p.dark { copy::PALETTE_SWITCH_TO_LIGHT } else { copy::PALETTE_SWITCH_TO_DARK }, &[]),
        (copy::PALETTE_CHECK_UPDATES, &[]),
        (copy::PALETTE_CLEAR_COMPLETED, &[]),
        (copy::PALETTE_PAUSE_ALL, &[]),
        (copy::PALETTE_RESUME_ALL, &[]),
        (copy::PALETTE_CHANGE_FOLDER, &[]),
        (copy::PALETTE_SHORTCUTS, &[copy::KEY_CTRL, copy::KEYCAP_SLASH]),
        (copy::PALETTE_CANCEL_ALL, &[]),
    ];
    for (name, keys) in actions {
        l = l.child(entry(None, None, name, "", keys, false, p));
    }
    l
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

/// The Scheduled downloads section, as the Electron Settings screen words it.
/// `crumb`: show where it sits (in the bar's panel, which has no other title).
pub(crate) fn schedule_sheet(v: &Spec, p: &Pal, crumb: bool) -> El {
    let chosen = v.value("scheduleDays");
    let mut chips = row().gap(4.0);
    for (i, day) in w::DAYS.into_iter().enumerate() {
        // The chips are short; the day's full name is their label.
        let d = &day[..3];
        let on = chosen.as_array().is_some_and(|d| d.iter().any(|x| x.as_u64() == Some(i as u64)));
        let chip = el().h(32.0).w(48.0).center().radius(6.0).hover(p.hover).cursor(Cursor::Pointer).role("checkbox").checked(on).label(day);
        chips = chips.child(if on {
            chip.fill(p.raised)
                .border(1.0, p.line_strong)
                .child(label(d, sans(13.0, 500.0), p.text))
                .child(el().size(16.0, 2.0).radius(1.0).fill(p.accent).absolute(16.0, 24.0))
        } else {
            chip.border(1.0, p.line).child(label(d, sans(13.0, 400.0), p.muted))
        });
    }
    let time = row()
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(6.0)
        .fill(p.deep)
        .border(1.0, p.line_strong)
        .child(icon(&icons::CLOCK, 16.0).color(p.muted))
        .child(label(v.text("scheduleTime"), mono(13.0, 500.0), p.text));
    let mut c = col().grow(1.0).h_full().min_w(0.0).gap(4.0);
    if crumb {
        c = c.child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .mb(8.0)
                .child(label(copy::NAV_SETTINGS, sans(12.0, 400.0), p.muted))
                .child(label("/", sans(12.0, 400.0), p.muted))
                .child(label(PICKED, sans(12.0, 400.0), p.text2)),
        );
    }
    c.child(label(PICKED, sans(16.0, 500.0), p.text))
        .child(label(w::SCHEDULE_HINT, sans(13.0, 400.0), p.text2).mb(16.0))
        .child(setting((w::ENABLE_SCHEDULER, ""), toggle(v.on("scheduleEnabled"), p), p))
        .child(setting(w::START_TIME, time, p))
        .child(setting(w::ACTIVE_DAYS, chips, p))
        .child(setting(w::SHUTDOWN, toggle(v.on("scheduleShutdown"), p), p))
}

/// A Settings row: (label, description) and its control.
pub(crate) fn setting(row_words: (&str, &str), control: El, p: &Pal) -> El {
    let mut words = stack(4.0).grow(1.0).child(label(row_words.0, sans(13.0, 500.0), p.text));
    if !row_words.1.is_empty() {
        words = words.child(label(row_words.1, sans(13.0, 400.0), p.text2).wrap());
    }
    row().w_full().min_h(56.0).py(12.0).gap(16.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], p.line).child(words).child(control)
}

pub(crate) fn toggle(on: bool, p: &Pal) -> El {
    let knob = el().size(12.0, 12.0).radius(6.0).fill(if on { p.bg } else { p.text2 }).absolute(if on { 24.0 } else { 4.0 }, 4.0);
    el().size(40.0, 20.0)
        .radius(10.0)
        .fill(if on { p.accent } else { p.raised })
        .border(1.0, if on { p.accent } else { p.line_strong })
        .cursor(Cursor::Pointer)
        .focusable(true)
        .role("switch")
        .checked(on)
        .child(knob)
}
