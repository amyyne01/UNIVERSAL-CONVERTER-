//! The three drawings Orbit makes: the orbit ring (the progress mark, and nothing else),
//! the planet (the product's own mark, flat), and the speed trace.
use std::f32::consts::{FRAC_PI_2, TAU};

use rune::{Color, LineCap, Shape};

/// `n` points on a circle from `from` radians, clockwise on screen, over `sweep` radians.
fn arc(cx: f32, cy: f32, r: f32, from: f32, sweep: f32, n: usize) -> Vec<[f32; 2]> {
    (0..=n).map(|i| from + sweep * i as f32 / n as f32).map(|a| [cx + r * a.cos(), cy + r * a.sin()]).collect()
}

fn disc(cx: f32, cy: f32, r: f32) -> Vec<[f32; 2]> {
    let mut d = arc(cx, cy, r, 0.0, TAU, 40);
    d.pop();
    d
}

/// How a download sits on its orbit.
#[derive(Clone, Copy, PartialEq)]
pub enum Pass {
    /// Waiting: the orbit is only planned, a dotted circle.
    Planned,
    /// Moving at a known percent: the arc from twelve o'clock, the craft at its head.
    Flying(f32),
    /// Started, nothing measured yet: the craft waits at twelve o'clock.
    OnThePad,
    /// Downloaded and being finished (converting, tagging): the orbit closed, the craft home.
    Closing,
    /// Held at a percent (paused, retrying): the arc stops, the craft hollow.
    Holding(f32),
}

/// The signature. A 1.5 pt track circling the cover, the travelled part in 2 pt from twelve
/// o'clock clockwise, and the craft: a 3 pt disc with a gap cut round it in the row's colour.
/// `size` is the square it is drawn in; `under` is the colour beneath (the row).
pub fn orbit(size: f32, pass: Pass, track: Color, ink: Color, under: Color) -> Shape {
    let c = size / 2.0;
    let r = c - 3.0;
    let top = -FRAC_PI_2;
    let mut s = Shape::new();
    if pass == Pass::Planned {
        for i in 0..32 {
            let a = top + TAU * i as f32 / 32.0;
            s = s.fill(disc(c + r * a.cos(), c + r * a.sin(), 0.9), ink);
        }
        return s;
    }
    s = s.stroke_with(arc(c, c, r, 0.0, TAU, 64), true, 1.5, LineCap::Butt, track);
    let (travelled, hollow) = match pass {
        Pass::Flying(p) => (p, false),
        Pass::Holding(p) => (p, true),
        Pass::Closing => (1.0, false),
        Pass::OnThePad | Pass::Planned => (0.0, false),
    };
    if travelled > 0.0 {
        let n = ((64.0 * travelled).ceil() as usize).max(2);
        s = s.stroke_with(arc(c, c, r, top, TAU * travelled.min(1.0), n), false, 2.0, LineCap::Round, ink);
    }
    let head = top + TAU * travelled.min(1.0);
    let (hx, hy) = (c + r * head.cos(), c + r * head.sin());
    s = s.fill(disc(hx, hy, 5.0), under).fill(disc(hx, hy, 3.0), ink);
    if hollow {
        s = s.fill(disc(hx, hy, 1.6), under);
    }
    s
}

/// The planet, flat, as in the app's splash: a disc and a tilted ring passing behind it,
/// cut apart by a gap in the page colour. Drawn in a box `w` wide and `w * 0.59` tall.
pub fn planet(w: f32, ink: Color, page: Color) -> Shape {
    let h = w * 0.592;
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r = w * 0.274;
    let tilt: f32 = 0.17;
    let (ts, tc) = tilt.sin_cos();
    let rot = |x: f32, y: f32| [cx + x * tc - y * ts, cy + x * ts + y * tc];
    // A half of the ring: outer edge one way, inner edge back.
    let (ro, ry_o, ri, ry_i, lift) = (w * 0.478, w * 0.128, w * 0.418, w * 0.074, -w * 0.012);
    let band = |from: f32, to: f32| {
        let n = 48;
        let mut pts: Vec<[f32; 2]> = (0..=n).map(|i| from + (to - from) * i as f32 / n as f32).map(|a| rot(ro * a.cos(), ry_o * a.sin())).collect();
        pts.extend((0..=n).rev().map(|i| from + (to - from) * i as f32 / n as f32).map(|a| rot(ri * a.cos(), ry_i * a.sin() + lift)));
        pts
    };
    let gap = w * 0.016;
    // The back half reaches a little past each end so the halves meet without a seam.
    let mut s = Shape::new().fill(band(std::f32::consts::PI - 0.03, TAU + 0.03), ink);
    s = s.fill(disc(cx, cy, r + gap), page).fill(disc(cx, cy, r), ink);
    // The front half, with the gap cut between it and the disc.
    // Only across the disc: beyond it the stroke would notch the ring where its halves meet.
    let edge = |rx: f32, ry: f32, dy: f32| -> Vec<[f32; 2]> {
        arc(0.0, 0.0, 1.0, 0.0, std::f32::consts::PI, 96).iter().filter(|p| (rx * p[0]).abs() < r + gap).map(|p| rot(rx * p[0], ry * p[1] + dy)).collect()
    };
    s = s.stroke(edge(ri, ry_i, lift), gap * 2.0, page).stroke(edge(ro, ry_o, 0.0), gap * 2.0, page).fill(band(0.0, std::f32::consts::PI), ink);
    // Two craters, as in the mark.
    let crater = rot(-r * 0.47, -r * 0.62);
    s.fill(disc(crater[0], crater[1], r * 0.085), page).fill(disc(crater[0] + r * 0.11, crater[1] + r * 0.07, r * 0.035), page)
}

/// The speed trace: three faint rules at 0, half and full scale, and the line.
pub fn trace(samples: &[f32], scale: f32, w: f32, h: f32, rule: Color, ink: Color) -> Shape {
    let mut s = Shape::new();
    for y in [0.5, h / 2.0, h - 0.5] {
        s = s.line([0.0, y], [w, y], 1.0, LineCap::Butt, rule);
    }
    let n = samples.len().max(2) - 1;
    let pts: Vec<[f32; 2]> = samples.iter().enumerate().map(|(i, v)| [w * i as f32 / n as f32, h - 1.0 - (v / scale).clamp(0.0, 1.0) * (h - 2.0)]).collect();
    s.stroke(pts, 1.5, ink)
}
