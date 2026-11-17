//! The three drawings Orbit makes: the orbit ring (the progress mark, and nothing else),
//! the planet (the product's own mark, flat), and the speed trace.
use std::f32::consts::TAU;

use rune::{Color, LineCap, Shape};

pub use crate::ring::Pass;
use crate::ring::{Material, arc, disc, shape};

/// The signature, in Orbit's dress: its track, its ink for arc and craft, the row beneath.
/// The geometry is the shared ring's (`crate::ring::shape`).
pub fn orbit(size: f32, pass: Pass, track: Color, ink: Color, under: Color) -> Shape {
    shape(size, pass, &Material { track, under, stroke: ink, craft: ink, segments: None })
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
        let mut pts: Vec<[f32; 2]> =
            (0..=n).map(|i| from + (to - from) * i as f32 / n as f32).map(|a| rot(ro * a.cos(), ry_o * a.sin())).collect();
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
        arc(0.0, 0.0, 1.0, 0.0, std::f32::consts::PI, 96)
            .iter()
            .filter(|p| (rx * p[0]).abs() < r + gap)
            .map(|p| rot(rx * p[0], ry * p[1] + dy))
            .collect()
    };
    s = s
        .stroke(edge(ri, ry_i, lift), gap * 2.0, page)
        .stroke(edge(ro, ry_o, 0.0), gap * 2.0, page)
        .fill(band(0.0, std::f32::consts::PI), ink);
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
    let pts: Vec<[f32; 2]> =
        samples.iter().enumerate().map(|(i, v)| [w * i as f32 / n as f32, h - 1.0 - (v / scale).clamp(0.0, 1.0) * (h - 2.0)]).collect();
    s.stroke(pts, 1.5, ink)
}
