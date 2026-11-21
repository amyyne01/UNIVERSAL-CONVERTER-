//! Design gate, direction "Signal": a transfer desk. Graphite surfaces, one
//! cue-blue accent used like a lit meter segment, Geist for words and Geist
//! Mono for every figure. Progress reads as an instrument (a segmented level
//! meter and a tape counter); a finished row shows the file you now own.
//!
//! The signature is the tape counter (`model::timecode`): media time carried
//! across so far, `hh:mm:ss`, on in-flight Downloads rows and nowhere else.
mod downloads;
pub(crate) mod found;
mod home;
pub(crate) mod icons;
pub(crate) mod model;
mod nav;
pub(crate) mod parts;
pub(crate) mod settings;
pub(crate) mod theme;

#[cfg(test)]
mod handlers_tests;

use rune::{Color, Cx, El, col, el, icon, row, spacer};

use crate::gate::{Nav, Overlay, State};
use crate::{actions, copy, keys};
use parts::{keycap, label, mono, sans};
use theme::Pal;

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = theme::pal(s.palette, s.dark(cx));
    cx.caption_color(if p.dark { Color::WHITE } else { Color::BLACK });
    // Keyboard focus: the desk's lit segment, a 2 pt accent edge just off the key.
    cx.focus_ring(2.0, 2.0, p.accent);
    let insets = cx.chrome_insets();
    let content = match s.nav {
        Nav::Home => home::view(s, &p),
        Nav::Downloads => downloads::view(s, &p),
        Nav::Settings => settings::view(s, &p),
    };
    col()
        .w_full()
        .h_full()
        .fill(p.bg)
        .child(titlebar(s, &p, insets.trailing))
        .child(row().w_full().grow(1.0).min_h(0.0).child(nav::view(s, &p)).child(col().grow(1.0).h_full().min_w(0.0).child(content)))
}

fn titlebar(s: &State, p: &Pal, trailing: f32) -> El {
    let logo = match &s.logo {
        Some(img) if img.is_ready() => el().size(18.0, 18.0).pixels(img.pixels()),
        _ => el().size(18.0, 18.0),
    };
    let mark = row()
        .gap(8.0)
        .center_y()
        .child(logo)
        .child(parts::Line::new().strong(copy::BRAND, p.text).add(copy::BRAND_REST, p.muted).one(mono(12.0, 400.0)));
    let search = row()
        .w(420.0)
        .h(30.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.bg)
        .border(1.0, p.line)
        .hover(p.hover)
        .no_window_drag()
        .cursor(rune::Cursor::Pointer)
        .role("button")
        .key(keys::PALETTE)
        .on_press(|s: &mut State| actions::open(s, Overlay::Palette))
        .label(copy::SEARCH_CAPSULE_ARIA)
        .child(icon(&icons::SEARCH, 16.0).color(p.muted))
        .child(label(copy::SEARCH_CAPSULE, sans(13.0, 400.0), p.muted))
        .child(spacer())
        .child(row().gap(4.0).child(keycap(copy::KEY_CTRL, p)).child(keycap(copy::KEY_K, p)));
    // While anything is in flight the band's lower edge carries the accent: the desk is live.
    let edge = if model::active_count(&s.tasks) > 0 { p.accent.alpha(0.55) } else { p.line };
    row()
        .key("titlebar")
        .w_full()
        .h(crate::TITLEBAR)
        .fill(p.deep)
        .border_sides([0.0, 0.0, 1.0, 0.0], edge)
        .center_y()
        .window_drag()
        .child(row().grow(1.0).min_w(0.0).pl(16.0).child(mark))
        .child(search)
        .child(el().grow(1.0).min_w(trailing + 12.0))
}
