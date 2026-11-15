//! Design gate, direction "Orbit". The planet logo's world: a download coming in to land, deep ink, an orbit ring as the progress mark.
//!
//! Not designed yet: this only proves the direction is reachable.
use rune::tok::{self, type_};
use rune::{Cx, El, col, text};

use crate::gate::{Screen, State};

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let t = tok::theme(cx.system_dark());
    let screen = match s.screen {
        Screen::Home => "Home",
        Screen::Downloads => "Downloads",
    };
    let line = format!("Orbit · {screen} · {} downloads · palette {} · {} covers", s.tasks.len(), s.palette, s.covers.len());
    col().w_full().h_full().center().gap(8.0).fill(t.bg).child(text(line).font(type_::TITLE).color(t.text))
}
