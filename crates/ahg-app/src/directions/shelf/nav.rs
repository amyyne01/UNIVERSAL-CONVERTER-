//! The sidebar: the three places as a slim strip of icons down the left edge.
//! Hovering it opens it over the page (the page never moves); "Keep open" pins
//! it, and pinned it takes its width and the page reflows. The place you are in
//! wears a spine label: its icon printed on the cream paper of the sleeves'
//! spines, the row behind it raised.
use rune::{El, Icon, col, el, icon, row, text};

use super::icons;
use super::theme::{self, Pal, mono, sans};
use crate::copy;
use crate::gate::{Nav, State};

/// The collapsed strip.
pub const STRIP: f32 = 64.0;
/// The sidebar opened.
const OPEN: f32 = 232.0;
/// Invented: the old app had no pin. Collection's sidebar uses the same words.
const KEEP_OPEN: &str = "Keep open";

pub fn view(s: &State, p: &Pal, active: usize) -> El {
    let open = s.nav_open || s.nav_pinned;
    let floating = s.nav_open && !s.nav_pinned;
    let mut panel = col()
        .key("sidebar")
        .w(if open { OPEN } else { STRIP })
        .h_full()
        .px(8.0)
        .pt(8.0)
        .pb(12.0)
        .gap(4.0)
        .fill(p.sunk)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line_soft)
        .role("tablist")
        .label(copy::RAIL_ARIA)
        .on_hover_change(|s: &mut State, over: bool, _| s.nav_open = over)
        .child(item(s, p, Nav::Home, &icons::HOME, copy::NAV_HOME, 1, 0, open))
        .child(item(s, p, Nav::Downloads, &icons::DOWNLOAD, copy::NAV_DOWNLOADS, 2, active, open))
        .child(el().grow(1.0))
        .child(item(s, p, Nav::Settings, &icons::SETTINGS, copy::NAV_SETTINGS, 3, 0, open))
        .child(pin(s, p, open))
        .absolute(0.0, 0.0);
    if floating {
        panel = panel.shadow(8.0, 24.0, theme::label_ink().alpha(if p.dark { 0.5 } else { 0.16 }));
    }
    let slot = el().key("sidebar-slot").w(if s.nav_pinned { OPEN } else { STRIP }).h_full().child(panel);
    if floating { slot.raise() } else { slot }
}

/// "Keep open": drawn only while the sidebar is open; collapsed, its room stays so nothing jumps.
fn pin(s: &State, p: &Pal, open: bool) -> El {
    if !open {
        return el().h(40.0).mt(4.0);
    }
    let on = s.nav_pinned;
    let ink = if on { p.accent } else { p.text2 };
    row()
        .key("nav-pin")
        .w_full()
        .h(40.0)
        .mt(4.0)
        .gap(0.0)
        .center_y()
        .radius(8.0)
        .hover(p.raised)
        .focusable(true)
        .role("switch")
        .checked(on)
        .label(KEEP_OPEN)
        .on_press(|s: &mut State| {
            s.nav_pinned = !s.nav_pinned;
            s.nav_open = false;
        })
        .child(el().size(48.0, 40.0).center().child(icon(&icons::PIN, 16.0).color(ink)))
        .child(text(KEEP_OPEN).font(sans(13.0, 400.0)).color(ink).single_line())
}

#[allow(clippy::too_many_arguments)]
fn item(s: &State, p: &Pal, nav: Nav, glyph: &'static Icon, label: &str, n: usize, badge: usize, open: bool) -> El {
    let on = s.nav == nav;
    // The spine label: the place you are in, printed on the sleeves' paper.
    let mut mark = el().size(48.0, 40.0).center();
    mark = if on {
        mark.child(
            el().size(32.0, 32.0)
                .center()
                .radius(4.0)
                .fill(theme::label_paper())
                .border(1.0, theme::label_ink().alpha(0.18))
                .child(icon(glyph, 16.0).color(theme::label_ink())),
        )
    } else {
        mark.child(icon(glyph, 16.0).color(p.text2))
    };
    if badge > 0 && !open {
        mark = mark.child(
            el().h(16.0)
                .px(4.0)
                .center()
                .radius(8.0)
                .fill(p.accent)
                .absolute(32.0, 0.0)
                .child(text(badge.to_string()).font(mono(11.0, 500.0)).color(p.sunk)),
        );
    }
    let mut r = row()
        .key(format!("nav-{n}"))
        .w_full()
        .h(40.0)
        .center_y()
        .radius(8.0)
        .focusable(true)
        .role("tab")
        .checked(on)
        .label(label.to_string())
        .help(copy::nav_tooltip(label, n))
        .on_press(move |s: &mut State| s.nav = nav)
        .child(mark);
    r = if on { r.fill(p.raised) } else { r.hover(p.raised.alpha(0.6)) };
    if open {
        r = r
            .pr(12.0)
            .child(
                text(label.to_string())
                    .font(sans(13.0, if on { 500.0 } else { 400.0 }))
                    .color(if on { p.text } else { p.text2 })
                    .single_line(),
            )
            .child(el().grow(1.0));
        if badge > 0 {
            r = r.child(text(badge.to_string()).font(mono(12.0, 500.0)).color(p.accent).mr(12.0));
        }
        r = r.child(text(format!("{}+{n}", copy::KEY_CTRL)).font(mono(12.0, 400.0)).color(p.muted));
    }
    r
}
