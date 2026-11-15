//! Design gate, direction "Shelf". A record collection: cover art leads, warm paper-and-ink neutrals, a spine label as the signature.
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
    let line = format!("Shelf · {screen} · {} downloads · palette {} · {} covers", s.tasks.len(), s.palette, s.covers.len());
    col().w_full().h_full().center().gap(8.0).fill(t.bg).child(text(line).font(type_::TITLE).color(t.text))
}
