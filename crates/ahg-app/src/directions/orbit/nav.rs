//! The sidebar: the three places as three bodies on one vertical orbit line. Collapsed it
//! is a slim strip; hovering it opens the names over the content (it never pushes it);
//! "Keep open" pins it, and pinned it takes its width and the content reflows.
//! The place you are in is the solid body. While anything is downloading, the Downloads
//! body is on its orbit: the ring, the one place the accent shows here.
use rune::{El, Icon, col, el, icon, row, spacer};

use super::draw::{Pass, orbit};
use super::look::{BODY, BODY_M, FIG, Pal, SMALL, mix, txt};
use super::{icons, model};
use crate::copy;
use crate::gate::{Nav, State};

/// The collapsed strip.
const STRIP: f32 = 64.0;
/// The sidebar opened.
const OPEN: f32 = 240.0;
/// A body's slot, and the body inside it.
const SLOT: f32 = 48.0;
const BODY_D: f32 = 28.0;
/// Space round the bodies.
const PAD: f32 = 8.0;

pub fn view(s: &State, p: &Pal) -> El {
    let open = s.nav_open || s.nav_pinned;
    let mut slot = el()
        .key("sidebar")
        .w(if s.nav_pinned { OPEN } else { STRIP })
        .h_full()
        .on_hover_change(|s: &mut State, over: bool, _| s.nav_open = over)
        .child(panel(s, p, open));
    if open && !s.nav_pinned {
        slot = slot.raise();
    }
    slot
}

fn panel(s: &State, p: &Pal, open: bool) -> El {
    let active: Vec<_> = s.tasks.iter().filter(|t| model::is_active(t)).collect();
    let flying = (!active.is_empty()).then(|| active.iter().map(|t| model::percent(t)).sum::<f32>() / active.len() as f32);
    // The orbit line: from the first body's centre to the last's, behind them.
    let foot = PAD + if open { 40.0 + PAD } else { 0.0 };
    let line = el().w(1.0).fill(p.track).pin(Some(PAD + SLOT / 2.0 - 0.5), Some(PAD + SLOT / 2.0), None, Some(foot + SLOT / 2.0));
    let mut c = col()
        .w(if open { OPEN } else { STRIP })
        .h_full()
        .padding(PAD)
        .gap(PAD)
        .fill(p.deep)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line)
        .role("tablist")
        .label(copy::RAIL_ARIA)
        .child(line)
        .child(item(s, p, Nav::Home, &icons::HOME, copy::NAV_HOME, 1, 0, None, open))
        .child(item(s, p, Nav::Downloads, &icons::DOWNLOAD, copy::NAV_DOWNLOADS, 2, active.len(), flying, open))
        .child(spacer())
        .child(item(s, p, Nav::Settings, &icons::SETTINGS, copy::NAV_SETTINGS, 3, 0, None, open));
    if open {
        c = c.child(pin(s, p));
        if !s.nav_pinned {
            c = c.shadow(8.0, 24.0, mix(p.deep, rune::Color::BLACK, 0.6).alpha(if p.dark { 0.5 } else { 0.12 }));
        }
    }
    c.absolute(0.0, 0.0)
}

#[allow(clippy::too_many_arguments)]
fn item(s: &State, p: &Pal, nav: Nav, glyph: &'static Icon, label: &str, n: usize, count: usize, flying: Option<f32>, open: bool) -> El {
    let on = s.nav == nav;
    let disc = el()
        .size(BODY_D, BODY_D)
        .radius(BODY_D / 2.0)
        .center()
        .fill(if on { p.text } else { p.deep })
        .border(1.0, if on { p.text } else { p.track })
        .child(icon(glyph, 16.0).color(if on { p.deep } else { p.text2 }));
    let mut body = el().size(SLOT, SLOT).radius(SLOT / 2.0).center().fill(p.deep);
    if let Some(pct) = flying {
        body = body.shape(orbit(SLOT, Pass::Flying(pct.max(0.02)), p.track, p.accent, p.deep));
    }
    body = body.child(disc);
    if count > 0 && !open {
        body = body.child(
            row()
                .h(16.0)
                .px(4.0)
                .center()
                .radius(8.0)
                .fill(p.deep)
                .border(1.0, p.track)
                .child(txt(count.to_string(), FIG, p.text))
                .absolute(28.0, 0.0),
        );
    }
    let mut r = row()
        .key(format!("nav-{n}"))
        .w_full()
        .h(SLOT)
        .gap(8.0)
        .center_y()
        .radius(SLOT / 2.0)
        .focusable(true)
        .role("tab")
        .checked(on)
        .label(label.to_string())
        .help(copy::nav_tooltip(label, n))
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| s.nav = nav)
        .child(body);
    if !on {
        r = r.hover(p.hover);
    }
    if open {
        r = r
            .pr(16.0)
            .child(txt(label.to_string(), if on { BODY_M } else { BODY }, if on { p.text } else { p.text2 }).single_line())
            .child(spacer());
        if count > 0 {
            r = r.child(txt(count.to_string(), FIG, p.text).mr(8.0));
        }
        r = r.child(txt(format!("{}+{n}", copy::KEY_CTRL), FIG, p.muted));
    }
    r
}

/// "Keep open": pins the sidebar.
fn pin(s: &State, p: &Pal) -> El {
    let on = s.nav_pinned;
    let ink = if on { p.text } else { p.text2 };
    row()
        .key("nav-pin")
        .w_full()
        .h(40.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .hover(p.hover)
        .focusable(true)
        .role("switch")
        .checked(on)
        .label("Keep open")
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut State| s.nav_pinned = !s.nav_pinned)
        .child(el().size(SLOT, 40.0).center().child(icon(&icons::PIN, 16.0).color(ink)))
        .child(txt("Keep open", SMALL, ink))
}
