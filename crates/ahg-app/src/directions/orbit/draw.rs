//! The three drawings Orbit makes: the orbit ring (the progress mark, and nothing else),
//! the planet (the product's own mark, flat), and the speed trace.
use std::f32::consts::TAU;

use ahg_engine::types::DownloadTask;
use rune::{Color, El, LineCap, Shape, el};

use super::look::Pal;
use crate::gate::State;
pub use crate::ring::Pass;
use crate::ring::{self, Material, arc, disc, shape};

/// Orbit's ring material (`ring::orbit`) in `tint`, on the look's own track, with the craft's
/// gap cut in `under`, the surface the ring sits on. Both replace the shared roles' colours, so
/// the palette index given to `ring::orbit` does not reach the result.
pub fn material(p: &Pal, tint: Color, under: Color) -> Material {
    Material { track: p.track, under, ..ring::orbit(0, p.dark, tint) }
}

/// Whether a task is drawn on its ring: not once it has landed, and not when it failed (its
/// cover stands faded, as Shelf's record goes back to no ring).
pub fn has_ring(s: &State, t: &DownloadTask) -> bool {
    !matches!(s.rings.pass(&t.task_id), None | Some(Pass::Landed | Pass::Failed(_)))
}

/// A task's ring, `ring::view` in Orbit's material.
pub fn task_ring(s: &State, t: &DownloadTask, size: f32, m: &Material) -> El {
    let rank = s.tasks.iter().position(|x| x.task_id == t.task_id).unwrap_or(0);
    // TODO(motion): `reduced` comes from the motion context at step 52.
    let ring = ring::view(&format!("ring-{}", t.task_id), size, &s.rings, &t.task_id, &body(m), false, rank);
    with_craft(ring, size, s.rings.pass(&t.task_id), m)
}

/// Something being worked out that is not a task: `ring::waiting`, the craft on the pad.
pub fn waiting_ring(size: f32, m: &Material) -> El {
    // TODO(motion): `reduced` comes from the motion context at step 52.
    with_craft(ring::waiting("ring-waiting", size, &body(m), false, 0), size, Some(Pass::OnThePad), m)
}

/// The overall ring, `ring::overall`: nothing drawn while no task is unfinished.
pub fn overall_ring(s: &State, key: &str, size: f32, m: &Material) -> El {
    // ponytail: the craft's place repeats `Rings::sync`'s overall mean (the value is private to
    // `ring.rs`); read it from `Rings` once an accessor exists.
    let open: Vec<f32> = s
        .tasks
        .iter()
        .filter(|t| !matches!(s.rings.pass(&t.task_id), None | Some(Pass::Done | Pass::Landed | Pass::Failed(_))))
        .map(super::model::percent)
        .collect();
    let at = (!open.is_empty()).then(|| Pass::Flying(open.iter().sum::<f32>() / open.len() as f32));
    // TODO(motion): `reduced` comes from the motion context at step 52.
    with_craft(ring::overall(key, size, &s.rings, &body(m), false), size, at, m)
}

/// The track and the arc only: the craft is laid over them as strokes by [`with_craft`].
fn body(m: &Material) -> Material {
    Material { track: m.track, under: Color::TRANSPARENT, stroke: m.stroke, craft: Color::TRANSPARENT, segments: None }
}

/// The approved craft: a short rounded stroke at the pass's head with a gap cut round it in
/// `under`, hollow while held (the drawing of `ring::gallery`). Thinner below 40 pt, where the
/// gallery's 9 / 5 / 2 pt read chunky: in proportion to the size, down to [`CRAFT_MIN`].
fn with_craft(ring: El, size: f32, pass: Option<Pass>, m: &Material) -> El {
    // Mirrors the private `ring::travelled`: how far round the craft is, and whether it is held.
    let (at, hollow) = match pass {
        Some(Pass::Flying(v) | Pass::Stalled(v) | Pass::Failed(v)) => (v, false),
        Some(Pass::Holding(v) | Pass::Countdown(v)) => (v, true),
        Some(Pass::Closing | Pass::Done) => (1.0, false),
        Some(Pass::OnThePad) => (0.0, false),
        Some(Pass::Planned | Pass::Landed) | None => return ring,
    };
    let k = (size / 40.0).clamp(CRAFT_MIN, 1.0);
    let [a, b] = ring::craft_span(size, at);
    let mut layers = vec![(9.0 * k, m.under), (5.0 * k, m.craft)];
    if hollow {
        layers.push((2.0 * k, m.under));
    }
    ring.children(
        layers.into_iter().map(|(w, ink)| el().size(size, size).absolute(0.0, 0.0).shape(ring::craft(size, w, ink).trimmed(a, b))),
    )
}

/// The least share of the gallery's craft widths, reached at 18 pt (the found step's 16 pt ring).
const CRAFT_MIN: f32 = 0.45;

/// The signature, in Orbit's dress: its track, its ink for arc and craft, the row beneath.
/// The geometry is the shared ring's (`crate::ring::shape`).
#[allow(dead_code)] // unused until cleanup (step 19)
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
    // Fewer than two samples make no line: the rules alone.
    if samples.len() < 2 {
        return s;
    }
    let n = samples.len() - 1;
    let pts: Vec<[f32; 2]> =
        samples.iter().enumerate().map(|(i, v)| [w * i as f32 / n as f32, h - 1.0 - (v / scale).clamp(0.0, 1.0) * (h - 2.0)]).collect();
    s.stroke(pts, 1.5, ink)
}
