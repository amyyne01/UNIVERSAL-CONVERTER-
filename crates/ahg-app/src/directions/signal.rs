//! Design gate, direction "Signal". A transfer desk: meters and timecode in Geist Mono, cool graphite.
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
    let line = format!("Signal · {screen} · {} downloads · palette {} · {} covers", s.tasks.len(), s.palette, s.covers.len());
    col().w_full().h_full().center().gap(8.0).fill(t.bg).child(text(line).font(type_::TITLE).color(t.text))
}
