//! The preset picker: six previews over whichever preset is on, a press
//! switches in place. Shared by all six, so it wears its own plain colours.
//! Opened with Ctrl+. until every preset's Settings has a row that opens it.
use rune::color::hex;
use rune::{Color, Cx, El, Family, TextStyle, Tx, col, el, row, text};

use crate::gate::{self, State};
use crate::{actions, copy, keys};

const TILE_W: f32 = 248.0;
const TILE_H: f32 = 164.0;

/// The picker keeps one face whichever preset is on, so a switch does not reshape it under
/// the pointer (MOTION-SYSTEM.md 1.5 row 5).
const FAMILY: Family = Family::Named("Geist");

fn style(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).family(FAMILY)
}

/// The previews, one per entry of `gate::PRESETS`: each preset's first-launch
/// picture (no cover art in it), 496 px wide. Regenerate from the gate's
/// shots when a preset changes.
pub const PREVIEWS: [&[u8]; 6] = [
    include_bytes!("../assets/presets/0.png"),
    include_bytes!("../assets/presets/1.png"),
    include_bytes!("../assets/presets/2.png"),
    include_bytes!("../assets/presets/3.png"),
    include_bytes!("../assets/presets/4.png"),
    include_bytes!("../assets/presets/5.png"),
];

fn c(code: &str) -> Color {
    hex(code).unwrap_or(Color::BLACK)
}

pub(crate) fn tile(s: &State, index: usize, dark: bool) -> El {
    let (name, _, layout) = gate::PRESETS[index];
    let on = gate::preset(s) == index;
    let (ink, muted, edge) = if dark { (c("#f2f2f4"), c("#9a9aa3"), c("#34343b")) } else { (c("#17171a"), c("#6b6b74"), c("#d4d4da")) };
    let picture = match s.previews.get(index) {
        Some(image) if image.is_ready() => el().size(TILE_W, TILE_H).pixels(image.pixels()),
        _ => el().size(TILE_W, TILE_H),
    };
    let frame = el().radius(8.0).clip_rounded(8.0).border(if on { 2.0 } else { 1.0 }, if on { ink } else { edge }).child(picture);
    let kind = if layout.is_some() { copy::PICKER_NEW_LAYOUT } else { copy::PICKER_CLASSIC_LAYOUT };
    let caption = row()
        .gap(8.0)
        .center_y()
        .child(text(name).font(style(14.0, 500.0)).color(ink))
        .child(text(kind).font(style(12.0, 400.0)).color(muted));
    col()
        .key(keys::preset(index))
        .gap(8.0)
        .cursor(rune::Cursor::Pointer)
        .press_scale(0.98)
        .child(frame)
        .child(caption)
        .on_press_tx(move |s: &mut State, tx: &mut Tx| gate::set_preset(s, index, tx))
}

/// The picker, drawn over the whole window.
pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let dark = s.dark(cx);
    let (panel, ink, muted, edge) = if dark {
        (c("#1b1b1f"), c("#f2f2f4"), c("#9a9aa3"), c("#34343b"))
    } else {
        (c("#fbfbfc"), c("#17171a"), c("#6b6b74"), c("#d4d4da"))
    };
    let line = |from: usize| row().gap(24.0).child(tile(s, from, dark)).child(tile(s, from + 1, dark)).child(tile(s, from + 2, dark));
    let head = row()
        .center_y()
        .child(text(copy::PICKER_TITLE).font(style(20.0, 500.0)).color(ink))
        .child(el().grow(1.0))
        .child(text(copy::PICKER_ESC).font(style(12.0, 400.0)).color(muted));
    let sheet = col()
        .padding(32.0)
        .gap(24.0)
        .radius(16.0)
        .fill(panel)
        .border(1.0, edge)
        .shadow(24.0, 64.0, Color::BLACK.alpha(0.4))
        .child(head)
        .child(line(0))
        .child(line(3));
    el().key(keys::PICKER)
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .center()
        .fill(Color::BLACK.alpha(0.5))
        .on_press(actions::close)
        .transition(rune::Transition::opacity())
        .child(sheet.on_press(|_: &mut State| {}))
}
