//! Settings, the third place: everything that is not a download, in two
//! columns (left, what a download produces; right, the app, its engine and the
//! licence), as Electron assigned them. Every section and row is
//! `settings_spec::sections`, drawn with the Shelf look's controls
//! (`shelf::settings`), each section a card on the page.
use rune::{El, Icon, ScrollSpec, col, row, text};

use crate::copy::settings as w;
use crate::directions::shelf::settings::{body, destination, line};
use crate::directions::shelf::theme::{Pal, sans, serif};
use crate::gate::State;
use crate::keys;
use crate::settings_spec::{self, Section};

#[allow(dead_code)] // unused until cleanup (step 19): `icons::CARET_DOWN` replaces it
static CARET_DOWN: Icon = Icon {
    name: "collection-caret-down",
    view: 256.0,
    body: r#"<path d="M213.66,101.66l-80,80a8,8,0,0,1-11.32,0l-80-80A8,8,0,0,1,53.66,90.34L128,164.69l74.34-74.35a8,8,0,0,1,11.32,11.32Z" fill="currentColor"/>"#,
};
#[allow(dead_code)] // unused until cleanup (step 19): `icons::CARET_RIGHT` replaces it
pub(super) static CARET_RIGHT: Icon = Icon {
    name: "collection-caret-right",
    view: 256.0,
    body: r#"<path d="M181.66,133.66l-80,80a8,8,0,0,1-11.32-11.32L164.69,128,90.34,53.66a8,8,0,0,1,11.32-11.32l80,80A8,8,0,0,1,181.66,133.66Z" fill="currentColor"/>"#,
};

/// Which of the nine sections each column holds: what a download produces, then the app.
const LEFT: [usize; 4] = [0, 1, 2, 3];
const RIGHT: [usize; 5] = [4, 5, 7, 6, 8];

pub fn view(s: &State, p: &Pal) -> El {
    let secs = settings_spec::sections(s);
    let column = |which: &[usize]| {
        let mut c = col().grow(1.0).min_w(0.0).gap(16.0);
        for &i in which {
            c = c.child(card(s, p, i, &secs[i]));
        }
        c
    };
    let page = col()
        .w_full()
        .px(32.0)
        .pt(24.0)
        .pb(48.0)
        .child(
            row()
                .w_full()
                .gap(16.0)
                .items_end()
                .mb(24.0)
                .child(text(w::TITLE).font(serif(44.0).lh(48.0)).color(p.text))
                .child(text(w::SAVES).font(sans(13.0, 400.0)).color(p.text2).mb(8.0)),
        )
        .child(row().w_full().gap(16.0).items_start().child(column(&LEFT)).child(column(&RIGHT)));
    col().key("settings-place").grow(1.0).h_full().min_w(0.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(page)
}

/// A section's paper: a card on the page, its title and hint, then its settings.
fn card(s: &State, p: &Pal, i: usize, sec: &Section) -> El {
    let c = col().key(keys::section(i)).w_full().px(20.0).py(16.0).gap(8.0).radius(12.0).fill(p.card).border(1.0, p.line_soft);
    if sec.title.is_empty() {
        // Destination has no heading: the folder alone, on its own paper.
        return c.child(destination(p, &sec.rows[0]));
    }
    let mut head = col().w_full().gap(4.0).mb(8.0).child(text(sec.title).font(sans(16.0, 500.0)).color(p.text));
    if let Some(h) = sec.hint {
        head = head.child(text(h).font(sans(12.0, 400.0)).color(p.text2).wrap());
    }
    let mut c = c.child(head);
    for it in body(s, p, sec, |r| line(s, p, r).min_h(0.0).py(4.0).gap(16.0)) {
        c = c.child(it);
    }
    c
}
