//! Design gate, direction "Signal": a transfer desk. Graphite surfaces, one
//! cue-blue accent used like a lit meter segment, Geist for words and Geist
//! Mono for every figure. Progress reads as an instrument (a segmented level
//! meter and a tape counter); a finished row shows the file you now own.
//!
//! The signature is the tape counter (`model::timecode`): media time carried
//! across so far, `hh:mm:ss`, on in-flight Downloads rows and nowhere else.
mod downloads;
mod home;
mod icons;
mod model;
mod parts;
mod theme;

use rune::{Color, Cx, El, col, el, icon, row, spacer, text};

use crate::copy;
use crate::gate::{Screen, State};
use parts::{keycap, label, mono, sans};
use theme::Pal;

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = theme::pal(s.palette, cx.system_dark());
    cx.caption_color(if p.dark { Color::WHITE } else { Color::BLACK });
    let insets = cx.chrome_insets();
    let content = match s.screen {
        Screen::Home => home::view(s, &p),
        Screen::Downloads => downloads::view(s, &p),
    };
    col()
        .w_full()
        .h_full()
        .fill(p.bg)
        .child(titlebar(s, &p, insets.trailing))
        .child(row().w_full().grow(1.0).min_h(0.0).child(rail(s, &p)).child(col().grow(1.0).h_full().min_w(0.0).child(content)))
}

fn titlebar(s: &State, p: &Pal, trailing: f32) -> El {
    let logo = match &s.logo {
        Some(img) if img.is_ready() => el().size(18.0, 18.0).pixels(img.pixels()),
        _ => el().size(18.0, 18.0),
    };
    let mark = row().gap(10.0).center_y().child(logo).child(parts::Line::new().strong(copy::BRAND, p.text).add(copy::BRAND_REST, p.muted).one(mono(12.0, 400.0)));
    let search = row()
        .w(420.0)
        .h(30.0)
        .px(10.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.bg)
        .border(1.0, p.line)
        .hover(p.hover)
        .no_window_drag()
        .cursor(rune::Cursor::Pointer)
        .role("button")
        .label(copy::SEARCH_CAPSULE_ARIA)
        .child(icon(&icons::SEARCH, 15.0).color(p.muted))
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

struct NavItem {
    label: &'static str,
    caption: &'static str,
    icon: &'static rune::Icon,
    screen: Option<Screen>,
}

const NAV: [NavItem; 7] = [
    NavItem { label: copy::NAV_HOME, caption: copy::NAV_HOME, icon: &icons::HOME, screen: Some(Screen::Home) },
    NavItem { label: copy::NAV_DOWNLOADS, caption: copy::NAV_DOWNLOADS, icon: &icons::DOWNLOAD, screen: Some(Screen::Downloads) },
    NavItem { label: copy::NAV_YOUTUBE, caption: copy::NAV_YOUTUBE, icon: &icons::YOUTUBE, screen: None },
    NavItem { label: copy::NAV_SPOTIFY, caption: copy::NAV_SPOTIFY, icon: &icons::SPOTIFY, screen: None },
    NavItem { label: copy::NAV_SOUNDCLOUD, caption: copy::NAV_SOUNDCLOUD, icon: &icons::SOUNDCLOUD, screen: None },
    NavItem { label: copy::NAV_REELS, caption: copy::NAV_REELS_RAIL_CAPTION, icon: &icons::REELS, screen: None },
    NavItem { label: copy::NAV_SETTINGS, caption: copy::NAV_SETTINGS, icon: &icons::SETTINGS, screen: None },
];

fn rail(s: &State, p: &Pal) -> El {
    let active = model::active_count(&s.tasks);
    let item = |i: usize| -> El {
        let n = &NAV[i];
        let on = n.screen == Some(s.screen);
        let target = n.screen;
        let ink = if on { p.text } else { p.muted };
        let mut glyph = el().size(20.0, 20.0).child(icon(n.icon, 20.0).color(ink));
        if n.screen == Some(Screen::Downloads) && active > 0 {
            glyph = glyph.child(text(active.to_string()).font(mono(10.0, 600.0)).color(p.accent_ink).absolute(15.0, -6.0));
        }
        let mut cell = el()
            .w(76.0)
            .h(56.0)
            .center()
            .child(
                col()
                    .w(64.0)
                    .h(52.0)
                    .center()
                    .gap(5.0)
                    .radius(8.0)
                    .fill(if on { p.raised } else { Color::TRANSPARENT })
                    .hover(p.hover)
                    .press(p.press)
                    .cursor(rune::Cursor::Pointer)
                    .focusable(true)
                    .role("tab")
                    .label(n.label)
                    .help(copy::nav_tooltip(n.label, i + 1))
                    .child(glyph)
                    .child(label(n.caption, sans(10.5, if on { 500.0 } else { 400.0 }), ink))
                    .on_press(move |s: &mut State| {
                        if let Some(screen) = target {
                            s.screen = screen;
                        }
                    }),
            );
        if on {
            // The tally: a lit segment on the rail's edge.
            cell = cell.child(el().size(3.0, 22.0).radius(1.5).fill(p.accent).absolute(0.0, 17.0));
        }
        cell
    };
    col()
        .w(76.0)
        .h_full()
        .fill(p.deep)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line)
        .pt(8.0)
        .pb(8.0)
        .role("tablist")
        .label(copy::RAIL_ARIA)
        .child(item(0))
        .child(item(1))
        .child(el().w(28.0).h(1.0).fill(p.line_strong).mt(6.0).mb(6.0).self_center())
        .child(item(2))
        .child(item(3))
        .child(item(4))
        .child(item(5))
        .child(spacer())
        .child(item(6))
}
