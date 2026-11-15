//! Design gate, direction "Shelf": a record collection. Cover art leads; every
//! download is a sleeve, and the record slides out of it as the file comes in.
//! Warm paper-and-ink neutrals, Instrument Sans with IBM Plex Mono for figures,
//! Instrument Serif for the one headline per screen. The signature is the
//! sleeve's spine label (see `parts::sleeve`).
//!
//! This file draws the shell: the title bar, the sidebar (`nav`) and the
//! place beside it, chosen by `State.nav`.
mod downloads;
mod home;
pub(crate) mod icons;
mod nav;
pub(crate) mod parts;
mod settings;
pub(crate) mod theme;

use rune::{Cx, El, Image, col, el, icon, rich, row, text};

use crate::copy;
use crate::gate::{Nav, State};
use parts::{is_active, keycap};
use theme::{Pal, sans};

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = Pal::new(s.palette, cx.system_dark());
    cx.caption_color(p.text);
    let insets = cx.chrome_insets();
    let active = s.tasks.iter().filter(|t| is_active(t.progress.status)).count();
    let place = match s.nav {
        Nav::Home => home::view(s, &p, active),
        Nav::Downloads => downloads::view(s, &p),
        Nav::Settings => settings::view(s, &p),
    };
    // The carrier line: a hairline of accent along the top of the content while anything is in flight.
    let carrier = el().h(1.0).w_full().fill(if active > 0 { p.accent.alpha(0.85) } else { p.line_soft });
    let content = col().grow(1.0).h_full().min_h(0.0).min_w(0.0).fill(p.bg).child(carrier).child(place);
    col()
        .w_full()
        .h_full()
        .fill(p.sunk)
        .child(titlebar(s, &p, insets.leading, insets.trailing))
        .child(row().grow(1.0).min_h(0.0).w_full().child(nav::view(s, &p, active)).child(content))
}

// ── Title bar ───────────────────────────────────────────────────────────────

fn titlebar(s: &State, p: &Pal, leading: f32, trailing: f32) -> El {
    let side = 260.0;
    let mark = match &s.logo {
        Some(logo) => logo_mark(logo),
        None => el().size(16.0, 16.0),
    };
    let wordmark = rich(
        format!("{}{}", copy::BRAND, copy::BRAND_REST),
        vec![
            rune::Span::new(0, copy::BRAND.len()).weight(500.0).color(p.text),
            rune::Span::new(copy::BRAND.len(), copy::BRAND.len() + copy::BRAND_REST.len()).color(p.muted),
        ],
    )
    .font(sans(13.0, 400.0))
    .single_line();
    let brand = row().w(side).pl(leading + 24.0).gap(12.0).center_y().child(mark).child(wordmark);
    let search = row()
        .key("search")
        .w(420.0)
        .h(32.0)
        .px(12.0)
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
        .child(text(copy::SEARCH_CAPSULE).font(sans(13.0, 400.0)).color(p.muted))
        .child(el().grow(1.0))
        .child(row().gap(4.0).child(keycap(p, copy::KEY_CTRL)).child(keycap(p, copy::KEY_K)));
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
    el().size(16.0, 16.0).pixels(logo.pixels())
}
