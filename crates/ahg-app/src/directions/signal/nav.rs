//! The desk's channel strip: the three places as a slim graphite strip down
//! the left edge. Hovering it opens the strip over the content (it never
//! pushes the page); "Keep open" pins it, and pinned it takes its width in the
//! layout. The place you are in carries the one lit segment on the strip's edge.
use rune::{Color, El, Icon, col, el, icon, row};

use super::icons;
use super::model;
use super::parts::{label, mono, sans};
use super::theme::Pal;
use crate::copy;
use crate::gate::{Nav, State};

/// The collapsed strip.
const STRIP: f32 = 56.0;
/// The strip opened.
const OPEN: f32 = 224.0;
const PIN_LABEL: &str = "Keep open";

pub fn view(s: &State, p: &Pal) -> El {
    let open = s.nav_open || s.nav_pinned;
    let mut slot = el()
        .key("nav")
        .w(if s.nav_pinned { OPEN } else { STRIP })
        .h_full()
        .child(panel(s, p, open));
    if open && !s.nav_pinned {
        slot = slot.raise();
    }
    slot
}

fn panel(s: &State, p: &Pal, open: bool) -> El {
    let active = model::active_count(&s.tasks);
    let mut c = col()
        .w(if open { OPEN } else { STRIP })
        .h_full()
        .px(8.0)
        .pt(8.0)
        .pb(8.0)
        .gap(4.0)
        .fill(p.deep)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line)
        .role("tablist")
        .label(copy::RAIL_ARIA)
        .child(item(s, p, Nav::Home, &icons::HOME, copy::NAV_HOME, 1, 0, open))
        .child(item(s, p, Nav::Downloads, &icons::DOWNLOAD, copy::NAV_DOWNLOADS, 2, active, open))
        .child(el().grow(1.0))
        .child(item(s, p, Nav::Settings, &icons::SETTINGS, copy::NAV_SETTINGS, 3, 0, open));
    if open {
        c = c.child(el().w_full().h(1.0).fill(p.line).my(4.0)).child(pin(s, p));
    }
    if open && !s.nav_pinned {
        c = c.shadow(8.0, 24.0, Color::BLACK.alpha(if p.dark { 0.5 } else { 0.14 }));
    }
    // Hover sits on the panel, not the slot: opened, the panel is wider than its slot.
    c.on_hover_change(|s: &mut State, over: bool, _| s.nav_open = over).absolute(0.0, 0.0)
}

#[allow(clippy::too_many_arguments)]
fn item(s: &State, p: &Pal, nav: Nav, glyph: &'static Icon, name: &str, n: usize, badge: usize, open: bool) -> El {
    let on = s.nav == nav;
    let ink = if on { p.text } else { p.muted };
    let mut mark = el().size(40.0, 40.0).center().child(icon(glyph, 16.0).color(ink));
    if badge > 0 && !open {
        // The count as a lit segment beside the glyph, clear of its corner.
        mark = mark.child(el().h(16.0).min_w(16.0).px(4.0).center().radius(4.0).fill(p.accent).absolute(28.0, 2.0).child(label(badge.to_string(), mono(12.0, 500.0), p.deep)));
    }
    let mut r = row()
        .key(format!("nav-{n}"))
        .w_full()
        .h(40.0)
        .center_y()
        .radius(8.0)
        .cursor(rune::Cursor::Pointer)
        .focusable(true)
        .role("tab")
        .checked(on)
        .label(name.to_string())
        .help(copy::nav_tooltip(name, n))
        .on_press(move |s: &mut State| s.nav = nav)
        .child(mark);
    r = if on { r.fill(p.raised) } else { r.hover(p.hover).press(p.press) };
    if on {
        // The tally: the one lit segment, on the strip's edge.
        r = r.child(el().size(4.0, 16.0).radius(1.0).fill(p.accent).absolute(-8.0, 12.0));
    }
    if open {
        r = r.pr(12.0).gap(8.0).child(label(name.to_string(), sans(13.0, if on { 500.0 } else { 400.0 }), if on { p.text } else { p.text2 }).single_line()).child(el().grow(1.0));
        if badge > 0 {
            r = r.child(label(badge.to_string(), mono(12.0, 500.0), p.accent_ink));
        }
        r = r.child(label(format!("{}+{n}", copy::KEY_CTRL), mono(12.0, 400.0), p.muted));
    }
    r
}

fn pin(s: &State, p: &Pal) -> El {
    let on = s.nav_pinned;
    let ink = if on { p.accent_ink } else { p.text2 };
    row()
        .key("nav-pin")
        .w_full()
        .h(40.0)
        .pr(12.0)
        .center_y()
        .radius(8.0)
        .hover(p.hover)
        .press(p.press)
        .cursor(rune::Cursor::Pointer)
        .focusable(true)
        .role("switch")
        .checked(on)
        .label(PIN_LABEL)
        .on_press(|s: &mut State| s.nav_pinned = !s.nav_pinned)
        .child(el().size(40.0, 40.0).center().child(icon(&icons::PIN, 16.0).color(ink)))
        .child(label(PIN_LABEL, sans(13.0, 400.0), if on { p.text } else { p.text2 }).ml(8.0))
        .child(el().grow(1.0))
        // A two-segment indicator: lit while pinned.
        .child(row().gap(4.0).child(el().size(8.0, 4.0).radius(1.0).fill(if on { p.accent } else { p.unlit })).child(el().size(8.0, 4.0).radius(1.0).fill(if on { p.accent } else { p.unlit })))
}
