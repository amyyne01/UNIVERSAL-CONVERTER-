//! Design gate, direction "Orbit". The planet logo's world, drawn flat and exact like a
//! ground track: a download is a pass coming in to land. Deep ink by night, a chart on paper
//! by day; Space Grotesk for the interface, Space Mono for every figure.
//!
//! The signature is the orbit ring (`draw::orbit`): it marks a download's progress and
//! appears nowhere else. A download in flight is round, on its orbit; once it lands it is a
//! square tile, a file you own.
mod downloads;
pub(crate) mod draw;
mod home;
pub(crate) mod icons;
pub(crate) mod look;
pub(crate) mod model;

use rune::{ChromeInsets, Color, Cx, El, Icon, SurfaceFit, col, el, icon, row, spacer};

use crate::copy;
use crate::gate::{Screen, State};
use ahg_engine::types::DownloadTask;
use look::{CAPTION, FIG, FIG_S, Pal, SMALL, SMALL_M, mix, txt};

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = Pal::new(s.palette, cx.system_dark());
    cx.titlebar_height(crate::TITLEBAR);
    cx.caption_color(p.text2);
    let insets = cx.chrome_insets();
    let content = match s.screen {
        Screen::Home => home::view(s, &p),
        Screen::Downloads => downloads::view(s, &p),
    };
    let active = s.tasks.iter().filter(|t| model::is_active(t)).count();
    col()
        .w_full()
        .h_full()
        .fill(p.bg)
        .child(titlebar(s, &p, insets, active))
        .child(row().w_full().grow(1.0).min_h(0.0).child(rail(s, &p, active)).child(col().key("content").grow(1.0).min_w(0.0).h_full().child(content)))
}

// ── Shell ───────────────────────────────────────────────────────────────────

fn titlebar(s: &State, p: &Pal, insets: ChromeInsets, active: usize) -> El {
    let mark = match &s.logo {
        Some(img) if img.is_ready() => el().size(18.0, 18.0).radius(4.0).pixels_fit(img.pixels(), SurfaceFit::Fill),
        _ => el().size(18.0, 18.0),
    };
    let brand = row().gap(10.0).center_y().child(mark).child(row().child(txt(copy::BRAND, FIG, p.text)).child(txt(copy::BRAND_REST, FIG, p.muted)));
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
        .cursor(rune::Cursor::Pointer)
        .no_window_drag()
        .label(copy::SEARCH_CAPSULE_ARIA)
        .child(icon(&icons::SEARCH, 14.0).color(p.muted))
        .child(txt(copy::SEARCH_CAPSULE, SMALL, p.muted))
        .child(spacer())
        .child(keycap(p, copy::KEY_CTRL))
        .child(keycap(p, copy::KEY_K));
    // The carrier line: a hairline that takes the accent while anything is in flight.
    let carrier = if active > 0 { mix(p.bg, p.accent, 0.55) } else { p.line };
    row()
        .key("titlebar")
        .w_full()
        .h(crate::TITLEBAR)
        .fill(p.deep)
        .border_sides([0.0, 0.0, 1.0, 0.0], carrier)
        .center_y()
        .window_drag()
        // Even padding and two equal sides keep the search capsule on the window's centre line;
        // the caption buttons draw over the trailing side.
        .px(16.0)
        .child(row().grow(1.0).pl(insets.leading).child(brand))
        .child(search)
        .child(spacer())
}

struct NavItem {
    label: &'static str,
    caption: &'static str,
    glyph: &'static Icon,
    screen: Option<Screen>,
}

const NAV: [NavItem; 7] = [
    NavItem { label: copy::NAV_HOME, caption: copy::NAV_HOME, glyph: &icons::HOME, screen: Some(Screen::Home) },
    NavItem { label: copy::NAV_DOWNLOADS, caption: copy::NAV_DOWNLOADS, glyph: &icons::DOWNLOAD, screen: Some(Screen::Downloads) },
    NavItem { label: copy::NAV_YOUTUBE, caption: copy::NAV_YOUTUBE, glyph: &icons::YOUTUBE_LINE, screen: None },
    NavItem { label: copy::NAV_SPOTIFY, caption: copy::NAV_SPOTIFY, glyph: &icons::SPOTIFY_LINE, screen: None },
    NavItem { label: copy::NAV_SOUNDCLOUD, caption: copy::NAV_SOUNDCLOUD, glyph: &icons::SOUNDCLOUD_LINE, screen: None },
    NavItem { label: copy::NAV_REELS, caption: copy::NAV_REELS_RAIL_CAPTION, glyph: &icons::REELS_LINE, screen: None },
    NavItem { label: copy::NAV_SETTINGS, caption: copy::NAV_SETTINGS, glyph: &icons::SETTINGS, screen: None },
];

fn rail(s: &State, p: &Pal, active: usize) -> El {
    let item = |i: usize| {
        let n = &NAV[i];
        let on = n.screen == Some(s.screen);
        let ink = if on { p.text } else { p.muted };
        let mut glyph = el().size(20.0, 20.0).child(icon(n.glyph, 20.0).color(ink));
        if n.screen == Some(Screen::Downloads) && active > 0 {
            glyph = glyph.child(txt(active.to_string(), FIG_S, p.accent).absolute(15.0, -7.0));
        }
        let mut b = col()
            .key(format!("nav-{i}"))
            .w(64.0)
            .h(54.0)
            .gap(5.0)
            .center()
            .radius(10.0)
            .role("tab")
            .label(n.label)
            .help(copy::nav_tooltip(n.label, i + 1))
            .focusable(true)
            .cursor(rune::Cursor::Pointer)
            .child(glyph)
            .child(txt(n.caption, CAPTION, ink));
        b = if on { b.fill(p.raised) } else { b.hover(mix(p.hover, p.deep, 0.5)) };
        if let Some(screen) = n.screen {
            b = b.on_press(move |s: &mut State| s.screen = screen);
        }
        // The active tab's mark: a short accent bar on the rail's edge.
        let bar = el().w(2.0).h(20.0).radius(1.0).fill(if on { p.accent } else { Color::TRANSPARENT });
        row().w_full().center_y().child(bar).child(el().w(4.0)).child(b)
    };
    col()
        .key("rail")
        .w(76.0)
        .h_full()
        .fill(p.deep)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line)
        .pt(12.0)
        .pb(12.0)
        .gap(4.0)
        .role("tablist")
        .label(copy::RAIL_ARIA)
        .child(item(0))
        .child(item(1))
        .child(el().w(28.0).h(1.0).fill(p.line).ml(24.0).my(8.0))
        .child(item(2))
        .child(item(3))
        .child(item(4))
        .child(item(5))
        .child(spacer())
        .child(item(6))
}

// ── Pieces both screens use ─────────────────────────────────────────────────

pub(super) fn keycap(p: &Pal, label: &str) -> El {
    row().h(18.0).px(5.0).center().radius(4.0).fill(p.raised).border(1.0, p.line).child(txt(label, FIG_S, p.text2))
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
        .gap(6.0)
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
            let glyph = if t.is_audio_only { &icons::MUSIC } else { &icons::VIDEO };
            el().size(size, size).radius(r).fill(p.raised).center().child(icon(glyph, (size * 0.45).round()).color(p.muted))
        }
    }
}
