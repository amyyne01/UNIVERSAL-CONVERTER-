//! Now playing: to be built.
use rune::{Cx, El};

use crate::gate::State;

pub fn view(_s: &State, _cx: &mut Cx<State>) -> El {
    rune::text("Now playing")
}
