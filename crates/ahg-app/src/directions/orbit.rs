//! Design gate, direction "Orbit". The planet logo's world, drawn flat and exact like a
//! ground track: a download is a pass coming in to land. Deep ink by night, a chart on paper
//! by day; Space Grotesk for the interface, Space Mono for every figure.
//!
//! The signature is the orbit ring (`draw::orbit`): it marks a download's progress and
//! appears nowhere else. A download in flight is round, on its orbit; once it lands it is a
//! square tile, a file you own.
mod downloads;
pub(crate) mod draw;
pub(crate) mod found;
mod home;
// The shared layer took over what these three hold; the old copies go in the cleanup.
#[allow(dead_code)] // unused until cleanup (step 19)
pub(crate) mod icons;
#[allow(dead_code)] // unused until cleanup (step 19)
pub(crate) mod look;
#[allow(dead_code)] // unused until cleanup (step 19)
pub(crate) mod model;
mod nav;
pub(crate) mod settings;

use rune::{ChromeInsets, Color, Cx, El, Icon, SurfaceFit, col, el, icon, row, spacer};

use crate::copy;
use crate::gate::{Nav, State};
use ahg_engine::types::DownloadTask;
use look::{FIG, Pal, SMALL, SMALL_M, mix, txt};

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = Pal::new(s.palette, s.dark(cx));
    cx.titlebar_height(crate::TITLEBAR);
    cx.caption_color(p.text2);
    let insets = cx.chrome_insets();
    let content = match s.nav {
        Nav::Home => home::view(s, &p),
        Nav::Downloads => downloads::view(s, &p),
        Nav::Settings => settings::view(s, &p),
    };
    col().w_full().h_full().fill(p.bg).child(titlebar(s, &p, insets)).child(
        row()
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .child(nav::view(s, &p))
            .child(col().key("content").grow(1.0).min_w(0.0).h_full().child(content)),
    )
}

// ── Shell ───────────────────────────────────────────────────────────────────

fn titlebar(s: &State, p: &Pal, insets: ChromeInsets) -> El {
    let mark = match &s.logo {
        Some(img) if img.is_ready() => el().size(18.0, 18.0).radius(4.0).pixels_fit(img.pixels(), SurfaceFit::Fill),
        _ => el().size(18.0, 18.0),
    };
    let brand =
        row().gap(12.0).center_y().child(mark).child(row().child(txt(copy::BRAND, FIG, p.text)).child(txt(copy::BRAND_REST, FIG, p.muted)));
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
        .cursor(rune::Cursor::Pointer)
        .no_window_drag()
        .label(copy::SEARCH_CAPSULE_ARIA)
        .child(icon(&crate::icons::SEARCH, 16.0).color(p.muted))
        .child(txt(copy::SEARCH_CAPSULE, SMALL, p.muted))
        .child(spacer())
        .child(keycap(p, copy::KEY_CTRL))
        .child(keycap(p, copy::KEY_K));
    row()
        .key("titlebar")
        .w_full()
        .h(crate::TITLEBAR)
        .fill(p.deep)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
        .center_y()
        .window_drag()
        // Even padding and two equal sides keep the search capsule on the window's centre line;
        // the caption buttons draw over the trailing side.
        .px(16.0)
        .child(row().grow(1.0).pl(insets.leading).child(brand))
        .child(search)
        .child(spacer())
}

// ── Pieces both screens use ─────────────────────────────────────────────────

pub(super) fn keycap(p: &Pal, label: &str) -> El {
    row().h(20.0).px(4.0).center().radius(4.0).fill(p.raised).border(1.0, p.line).child(txt(label, FIG, p.text2))
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Tone {
    /// The one solid button on a screen.
    Primary,
    /// A hairline outline.
    Ghost,
}

pub(super) fn button(p: &Pal, key: &str, label: &str, glyph: Option<&'static Icon>, tone: Tone) -> El {
    let (fill, ink, edge) = match tone {
        Tone::Primary => (p.accent, p.on_accent, p.accent),
        Tone::Ghost => (Color::TRANSPARENT, p.text, p.track),
    };
    let mut b = row()
        .key(key.to_string())
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center()
        .radius(8.0)
        .fill(fill)
        .border(1.0, edge)
        .role("button")
        .label(label.to_string())
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .press_scale(0.97)
        .hover(if tone == Tone::Primary { mix(p.accent, p.text, 0.15) } else { p.raised });
    if let Some(g) = glyph {
        b = b.child(icon(g, 16.0).color(if tone == Tone::Primary { ink } else { p.text2 }));
    }
    b.child(txt(label.to_string(), SMALL_M, ink))
}

pub(super) fn icon_button(p: &Pal, key: String, label: &'static str, glyph: &'static Icon, ink: Color) -> El {
    el().key(key)
        .size(28.0, 28.0)
        .center()
        .radius(6.0)
        .role("button")
        .label(label)
        .help(label)
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .hover(p.hover)
        .press_scale(0.94)
        .child(icon(glyph, 16.0).color(ink))
}

/// A task's artwork. Round while it is on its orbit, a square tile once it has landed.
pub(super) fn cover(s: &State, p: &Pal, t: &DownloadTask, size: f32, round: bool) -> El {
    let r = if round { size / 2.0 } else { (size * 0.2).round() };
    match s.cover(&t.task_id) {
        Some(img) => el().size(size, size).radius(r).fill(p.raised).pixels_fit(img.pixels(), SurfaceFit::Fill),
        None => {
            let glyph = if t.is_audio_only { &crate::icons::MUSIC } else { &crate::icons::VIDEO };
            el().size(size, size).radius(r).fill(p.raised).center().child(icon(glyph, (size * 0.45).round()).color(p.muted))
        }
    }
}
