//! The shelf dividers: the three places as a slim strip down the left edge.
//! Hovering it opens the dividers over the content (it never pushes the wall);
//! the pin keeps them open, and pinned they take their width in the layout.
//! The place you are in is the divider card pulled up: the cream of the
//! sleeves' spine labels, printed in their ink.
use rune::{El, Icon, col, el, icon, row, text};

use crate::directions::shelf::theme::{self, Pal, mono, sans};
use crate::gate::{Nav, State};
use crate::{actions, copy, format, icons, keys};

/// The collapsed strip.
const STRIP: f32 = 56.0;
/// The dividers opened.
const OPEN: f32 = 224.0;

#[allow(dead_code)] // unused until cleanup (step 19): `icons::PIN` replaces it
static PIN: Icon = Icon {
    name: "collection-pin",
    view: 256.0,
    body: r#"<path d="M235.32,81.37,174.63,20.69a16,16,0,0,0-22.63,0L98.37,74.49c-10.66-3.34-35-7.37-60.4,13.14a16,16,0,0,0-1.29,23.78L85,159.71,42.34,202.34a8,8,0,0,0,11.32,11.32L96.29,171l48.29,48.29A16,16,0,0,0,155.9,224c.38,0,.75,0,1.13,0a15.93,15.93,0,0,0,11.64-6.33c19.64-26.1,17.75-47.32,13.19-60L235.33,104A16,16,0,0,0,235.32,81.37ZM224,92.69h0l-57.27,57.46a8,8,0,0,0-1.49,9.22c9.46,18.93-1.8,38.59-9.34,48.62L48,100.08c12.08-9.74,23.64-12.31,32.48-12.31A40.13,40.13,0,0,1,96.81,91a8,8,0,0,0,9.25-1.51L163.32,32,224,92.68Z" fill="currentColor"/>"#,
};

pub fn view(s: &State, p: &Pal) -> El {
    let active = format::active_count(&s.tasks);
    let pinned = s.own.nav_pinned;
    // Pinned, the dividers take their width in the layout; opened by hover they float over the wall.
    let floating = s.nav_open && !pinned;
    let mut slot = el()
        .key("dividers")
        .w(if pinned { OPEN } else { STRIP })
        .h_full()
        .on_hover_change(|s: &mut State, over: bool, _| s.nav_open = over)
        .child(panel(s, p, active, s.nav_open || pinned, floating));
    if floating {
        slot = slot.raise();
    }
    slot
}

fn panel(s: &State, p: &Pal, active: usize, open: bool, floating: bool) -> El {
    let w = if open { OPEN } else { STRIP };
    let mut c = col()
        .w(w)
        .h_full()
        .px(8.0)
        .pt(16.0)
        .pb(12.0)
        .gap(8.0)
        .fill(p.sunk)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line_soft)
        .role("tablist")
        .label(copy::RAIL_ARIA)
        .child(item(s, p, Nav::Home, &icons::HOME, copy::NAV_HOME, 1, 0, open))
        .child(item(s, p, Nav::Downloads, &icons::DOWNLOAD, copy::NAV_DOWNLOADS, 2, active, open))
        .child(el().grow(1.0))
        .child(item(s, p, Nav::Settings, &icons::SETTINGS, copy::NAV_SETTINGS, 3, 0, open));
    c = c.child(if open {
        row()
            .w_full()
            .h(32.0)
            .mt(8.0)
            .pl(12.0)
            .gap(12.0)
            .center_y()
            .radius(8.0)
            .hover(p.raised)
            .focusable(true)
            .key(keys::NAV_PIN)
            .role("switch")
            .checked(s.own.nav_pinned)
            .label(copy::KEEP_OPEN)
            .on_press(actions::toggle_pin)
            .border_sides([1.0, 0.0, 0.0, 0.0], p.line_soft)
            .child(icon(&icons::PIN, 16.0).color(p.text2))
            .child(text(copy::KEEP_OPEN).font(sans(13.0, 400.0)).color(p.text2))
    } else {
        el().h(32.0).mt(8.0)
    });
    if floating {
        c = c.shadow(8.0, 24.0, theme::label_ink().alpha(if p.dark { 0.5 } else { 0.16 }));
    }
    c.absolute(0.0, 0.0)
}

#[allow(clippy::too_many_arguments)]
fn item(s: &State, p: &Pal, nav: Nav, glyph: &'static Icon, label: &str, n: usize, badge: usize, open: bool) -> El {
    let on = s.nav == nav;
    let ink = if on { theme::label_ink() } else { p.text2 };
    let mut mark = el().size(40.0, 40.0).center().child(icon(glyph, 16.0).color(ink));
    if badge > 0 && !open {
        mark = mark.child(text(badge.to_string()).font(mono(12.0, 500.0)).color(if on { ink } else { p.accent }).absolute(26.0, 2.0));
    }
    let mut r = row()
        .key(keys::nav(nav))
        .w_full()
        .h(40.0)
        .center_y()
        .radius(8.0)
        .focusable(true)
        .role("tab")
        .checked(on)
        .label(label.to_string())
        .help(copy::nav_tooltip(label, n))
        .on_press(move |s: &mut State| actions::go(s, nav))
        .child(mark);
    r = if on { r.fill(theme::label_paper()).border(1.0, theme::label_ink().alpha(0.18)) } else { r.hover(p.raised) };
    if open {
        r = r
            .pr(12.0)
            .child(
                text(label.to_string()).font(sans(13.0, if on { 500.0 } else { 400.0 })).color(if on { ink } else { p.text }).single_line(),
            )
            .child(el().grow(1.0));
        if badge > 0 {
            r = r.child(text(badge.to_string()).font(mono(12.0, 500.0)).color(if on { ink } else { p.accent }).mr(12.0));
        }
        r = r.child(text(format!("{}+{n}", copy::KEY_CTRL)).font(mono(12.0, 400.0)).color(if on { ink.alpha(0.6) } else { p.muted }));
    }
    r
}
