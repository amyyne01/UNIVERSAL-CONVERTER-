//! Settings, the third place (Ctrl+3): every section side by side in three columns, like the
//! liner notes on the back of a sleeve, so nothing hides behind a sub-page.
//!
//! Sections, rows, wording, values, locks and dimming come from `settings_spec::sections`;
//! the rows' words and controls are Orbit's (`orbit::settings::{words, control}`), this file
//! gives them the sheet's frame.
use rune::{El, ScrollSpec, col, el, row, spacer};

use super::Fit;
use super::parts::{BODY, BODY_M, FIG, HERO, SMALL};
use crate::copy::settings as w;
use crate::directions::orbit::look::{Pal, txt};
use crate::directions::orbit::settings::{DIM, control, section_lines, words};
use crate::gate::State;
use crate::keys;
use crate::settings_spec::{self, Row, Section};

/// Which sections stand in which column, by their place in `settings_spec::sections`:
/// where files go, the defaults and the look; the other download options and the schedule;
/// software, advanced, the plan and help.
const COLUMNS: [&[usize]; 3] = [&[0, 1, 3], &[2, 4], &[5, 6, 7, 8]];
/// The same sections in two columns, when three would leave a row too narrow for its words
/// and its control side by side.
const TWO_COLUMNS: [&[usize]; 2] = [&[0, 1, 3, 2], &[4, 5, 6, 7, 8]];

pub fn view(s: &State, p: &Pal, fit: Fit) -> El {
    let sections = settings_spec::sections(s);
    let head = row()
        .w_full()
        .gap(16.0)
        .items_end()
        .child(txt(w::TITLE, HERO, p.text))
        .child(txt(w::SAVES, SMALL, p.muted).mb(4.0))
        .child(spacer())
        .child(row().gap(8.0).help(w::MACHINE_ID_TITLE).child(txt(w::MACHINE_ID, SMALL, p.muted)).child(txt(
            s.machine_id.clone(),
            FIG,
            p.muted,
        )));
    let mut columns = row().w_full().gap(if fit.wide { 40.0 } else { 24.0 }).items_start();
    let lists: &[&[usize]] = if fit.roomy { &COLUMNS } else { &TWO_COLUMNS };
    for list in lists {
        let mut c = col().grow(1.0).min_w(0.0).gap(24.0);
        for i in list.iter() {
            if let Some(sec) = sections.get(*i) {
                c = c.child(section(s, p, sec).key(keys::section(*i)));
            }
        }
        columns = columns.child(c);
    }
    col()
        .key("settings")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(col().w_full().px(fit.pad + 8.0).pt(fit.pad).pb(fit.pad + 8.0).gap(fit.pad).child(head).child(columns))
}

/// A section: its title and hint (Destination has neither), then its rows on hairlines.
fn section(s: &State, p: &Pal, sec: &Section) -> El {
    let mut c = col().w_full();
    if !sec.title.is_empty() {
        c = c.child(txt(sec.title, BODY_M, p.text));
        if let Some(h) = sec.hint {
            c = c.child(txt(h, SMALL, p.muted).wrap().mt(4.0));
        }
        c = c.child(el().h(8.0));
    }
    let each = |r: &Row| {
        let line = row().w_full().min_h(40.0).py(8.0).gap(16.0).center_y().child(words(s, p, r, BODY)).child(control(s, p, r));
        if r.dimmed { line.opacity(DIM) } else { line }
    };
    for line in section_lines(s, p, sec, BODY, each) {
        c = c.child(line.border_sides([1.0, 0.0, 0.0, 0.0], p.line));
    }
    c
}
