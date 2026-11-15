//! Design gate, direction "Shelf": a record collection. Cover art leads; every
//! download is a sleeve, and the record slides out of it as the file comes in.
//! Warm paper-and-ink neutrals, Instrument Sans with IBM Plex Mono for figures,
//! Instrument Serif for the one headline per screen. The signature is the
//! sleeve's spine label (see `parts::sleeve`).
//!
//! This file draws the shell: the title bar, the rail and the screen beside it.
mod downloads;
mod home;
pub(crate) mod icons;
pub(crate) mod parts;
pub(crate) mod theme;

use rune::{Cx, El, Icon, Image, col, el, icon, rich, row, text};

use crate::copy;
use crate::gate::{Screen, State};
use parts::{is_active, keycap};
use theme::{Pal, sans};

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = Pal::new(s.palette, cx.system_dark());
    cx.caption_color(p.text);
    let insets = cx.chrome_insets();
    let active = s
        .tasks
        .iter()
        .filter(|t| is_active(t.progress.status))
        .count();
    let screen = match s.screen {
        Screen::Home => home::view(s, &p, active),
        Screen::Downloads => downloads::view(s, &p),
    };
    // The carrier line: a hairline of accent along the top of the content while anything is in flight.
    let carrier = el().h(1.0).w_full().fill(if active > 0 {
        p.accent.alpha(0.85)
    } else {
        p.line_soft
    });
    let content = col()
        .grow(1.0)
        .h_full()
        .min_h(0.0)
        .min_w(0.0)
        .fill(p.bg)
        .child(carrier)
        .child(screen);
    col()
        .w_full()
        .h_full()
        .fill(p.sunk)
        .child(titlebar(s, &p, insets.leading, insets.trailing))
        .child(
            row()
                .grow(1.0)
                .min_h(0.0)
                .w_full()
                .child(rail(s, &p, active))
                .child(content),
        )
}

// ── Title bar ───────────────────────────────────────────────────────────────

fn titlebar(s: &State, p: &Pal, leading: f32, trailing: f32) -> El {
    let side = 260.0;
    let mark = match &s.logo {
        Some(logo) => logo_mark(logo),
        None => el().size(18.0, 18.0),
    };
    let wordmark = rich(
        format!("{}{}", copy::BRAND, copy::BRAND_REST),
        vec![
            rune::Span::new(0, copy::BRAND.len())
                .weight(600.0)
                .color(p.text),
            rune::Span::new(
                copy::BRAND.len(),
                copy::BRAND.len() + copy::BRAND_REST.len(),
            )
            .color(p.muted),
        ],
    )
    .font(sans(13.0, 450.0))
    .single_line();
    let brand = row()
        .w(side)
        .pl(leading + 16.0)
        .gap(10.0)
        .center_y()
        .child(mark)
        .child(wordmark);
    let search = row()
        .key("search")
        .w(420.0)
        .h(30.0)
        .px(10.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.bg)
        .border(1.0, p.line)
        .hover(p.raised)
        .no_window_drag()
        .focusable(true)
        .role("button")
        .label(copy::SEARCH_CAPSULE_ARIA)
        .child(icon(&icons::SEARCH, 16.0).color(p.muted))
        .child(
            text(copy::SEARCH_CAPSULE)
                .font(sans(13.0, 400.0))
                .color(p.muted),
        )
        .child(el().grow(1.0))
        .child(
            row()
                .gap(4.0)
                .child(keycap(p, copy::KEY_CTRL))
                .child(keycap(p, copy::KEY_K)),
        );
    row()
        .key("titlebar")
        .w_full()
        .h(crate::TITLEBAR)
        .center_y()
        .fill(p.sunk)
        .window_drag()
        .child(brand)
        .child(row().grow(1.0).justify_center().child(search))
        .child(el().w(side.max(trailing + 16.0)))
}

fn logo_mark(logo: &Image) -> El {
    el().size(18.0, 18.0).pixels(logo.pixels())
}

// ── Rail ────────────────────────────────────────────────────────────────────

struct Item {
    key: &'static str,
    glyph: &'static Icon,
    /// The accessible name and tooltip.
    label: &'static str,
    caption: &'static str,
    screen: Option<Screen>,
}

const ITEMS: [Item; 7] = [
    Item {
        key: "nav-home",
        glyph: &icons::HOME,
        label: copy::NAV_HOME,
        caption: copy::NAV_HOME,
        screen: Some(Screen::Home),
    },
    Item {
        key: "nav-downloads",
        glyph: &icons::DOWNLOAD,
        label: copy::NAV_DOWNLOADS,
        caption: copy::NAV_DOWNLOADS,
        screen: Some(Screen::Downloads),
    },
    Item {
        key: "nav-youtube",
        glyph: &icons::YOUTUBE,
        label: copy::NAV_YOUTUBE,
        caption: copy::NAV_YOUTUBE,
        screen: None,
    },
    Item {
        key: "nav-spotify",
        glyph: &icons::SPOTIFY,
        label: copy::NAV_SPOTIFY,
        caption: copy::NAV_SPOTIFY,
        screen: None,
    },
    Item {
        key: "nav-soundcloud",
        glyph: &icons::SOUNDCLOUD,
        label: copy::NAV_SOUNDCLOUD,
        caption: copy::NAV_SOUNDCLOUD,
        screen: None,
    },
    Item {
        key: "nav-reels",
        glyph: &icons::REELS,
        label: copy::NAV_REELS,
        caption: copy::NAV_REELS_RAIL_CAPTION,
        screen: None,
    },
    Item {
        key: "nav-settings",
        glyph: &icons::SETTINGS,
        label: copy::NAV_SETTINGS,
        caption: copy::NAV_SETTINGS,
        screen: None,
    },
];

fn rail(s: &State, p: &Pal, active: usize) -> El {
    let item = |i: usize| rail_item(s, p, i, if i == 1 { active } else { 0 });
    col()
        .key("rail")
        .w(76.0)
        .h_full()
        .pt(8.0)
        .pb(12.0)
        .gap(4.0)
        .items_center()
        .fill(p.sunk)
        .role("tablist")
        .label(copy::RAIL_ARIA)
        .child(item(0))
        .child(item(1))
        .child(el().w(28.0).h(1.0).my(6.0).fill(p.line))
        .child(item(2))
        .child(item(3))
        .child(item(4))
        .child(item(5))
        .child(el().grow(1.0))
        .child(item(6))
}

fn rail_item(s: &State, p: &Pal, i: usize, badge: usize) -> El {
    let it = &ITEMS[i];
    let on = it.screen == Some(s.screen);
    let ink = if on { p.text } else { p.muted };
    let mut glyph = el()
        .size(20.0, 20.0)
        .child(icon(it.glyph, 20.0).color(if on { p.accent } else { p.muted }));
    if badge > 0 {
        glyph = glyph.child(
            text(badge.to_string())
                .font(theme::mono(10.0, 500.0))
                .color(p.accent)
                .absolute(17.0, -5.0),
        );
    }
    let mut b = col()
        .key(it.key)
        .w(64.0)
        .h(52.0)
        .gap(4.0)
        .center()
        .radius(10.0)
        .focusable(true)
        .role("tab")
        .label(it.label)
        .help(copy::nav_tooltip(it.label, i + 1))
        .child(glyph)
        .child(
            text(it.caption)
                .font(sans(11.0, if on { 500.0 } else { 400.0 }))
                .color(ink)
                .single_line(),
        );
    b = if on {
        b.fill(p.raised).border(1.0, p.line_soft)
    } else {
        b.hover(p.raised.alpha(0.6)).press(p.raised)
    };
    if let Some(screen) = it.screen {
        b = b.on_press(move |s: &mut State| s.screen = screen);
    }
    b
}
