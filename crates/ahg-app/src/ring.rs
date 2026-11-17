//! The progress ring shared by the presets: its model (`Pass`, `pass_of`, `tint`) and its
//! geometry (`shape`), which a look dresses through a `Material`.
use std::f32::consts::{FRAC_PI_2, TAU};

use ahg_engine::types::{DownloadStatus as St, DownloadTask};
use rune::{Color, LineCap, Shape};

use crate::directions::orbit::model::percent;
use crate::palette;

/// `n` points on a circle from `from` radians, clockwise on screen, over `sweep` radians.
pub(crate) fn arc(cx: f32, cy: f32, r: f32, from: f32, sweep: f32, n: usize) -> Vec<[f32; 2]> {
    (0..=n).map(|i| from + sweep * i as f32 / n as f32).map(|a| [cx + r * a.cos(), cy + r * a.sin()]).collect()
}

pub(crate) fn disc(cx: f32, cy: f32, r: f32) -> Vec<[f32; 2]> {
    let mut d = arc(cx, cy, r, 0.0, TAU, 40);
    d.pop();
    d
}

/// How a download sits on its orbit.
#[derive(Clone, Copy, PartialEq, Debug)]
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

/// The pass a task is on; `None` once it has landed or stopped (done, failed, cancelled).
pub(crate) fn pass_of(t: &DownloadTask) -> Option<Pass> {
    Some(match t.progress.status {
        St::Queued => Pass::Planned,
        St::FetchingInfo => Pass::OnThePad,
        St::Downloading => Pass::Flying(percent(t)),
        St::Converting | St::Embedding => Pass::Closing,
        St::Paused | St::Retrying => Pass::Holding(percent(t)),
        St::Done | St::Failed | St::Cancelled => return None,
    })
}

/// The task's platform colour.
pub(crate) fn tint(t: &DownloadTask, dark: bool) -> Color {
    palette::platform(t.source, dark)
}

/// What a look supplies to dress the ring. `under` is the colour beneath it (the gap round
/// the craft is cut in it), `stroke` the travelled arc and the dots, `craft` the craft disc.
/// `segments` cuts the ring into that many pieces, lit whole; `None` is one unbroken circle.
pub(crate) struct Material {
    pub track: Color,
    pub under: Color,
    pub stroke: Color,
    pub craft: Color,
    pub segments: Option<u32>,
}

/// A 1.5 pt track circling the square `size`, the travelled part in 2 pt from twelve
/// o'clock clockwise, and the craft: a 3 pt disc with a gap cut round it in `under`.
pub(crate) fn shape(size: f32, pass: Pass, m: &Material) -> Shape {
    let c = size / 2.0;
    let r = c - 3.0;
    let top = -FRAC_PI_2;
    let mut s = Shape::new();
    if pass == Pass::Planned {
        // A dot every 3 pt or so, at most 32: a small ring stays dotted, not solid.
        let n = ((TAU * r) / 3.0).clamp(8.0, 32.0) as usize;
        for i in 0..n {
            let a = top + TAU * i as f32 / n as f32;
            s = s.fill(disc(c + r * a.cos(), c + r * a.sin(), 0.9), m.stroke);
        }
        return s;
    }
    let (travelled, hollow) = travelled(pass);
    if let Some(n) = m.segments {
        // Whole segments, the newest at full strength and the run a step back, as Signal's meter.
        let (on, step) = (lit(travelled, n), TAU / n as f32);
        for i in 0..n {
            let ink = match i {
                i if i >= on => m.track,
                _ if hollow => m.stroke.alpha(0.5),
                i if i + 1 == on => m.stroke,
                _ => m.stroke.alpha(0.78),
            };
            s = s.stroke_with(arc(c, c, r, top + step * (i as f32 + 0.125), step * 0.75, 8), false, 2.0, LineCap::Butt, ink);
        }
        return s;
    }
    s = s.stroke_with(arc(c, c, r, 0.0, TAU, 64), true, 1.5, LineCap::Butt, m.track);
    if travelled > 0.0 {
        let n = ((64.0 * travelled).ceil() as usize).max(2);
        s = s.stroke_with(arc(c, c, r, top, TAU * travelled.min(1.0), n), false, 2.0, LineCap::Round, m.stroke);
    }
    let head = top + TAU * travelled.min(1.0);
    let (hx, hy) = (c + r * head.cos(), c + r * head.sin());
    s = s.fill(disc(hx, hy, 5.0), m.under).fill(disc(hx, hy, 3.0), m.craft);
    if hollow {
        s = s.fill(disc(hx, hy, 1.6), m.under);
    }
    s
}

/// How far round a pass has come, and whether it is held.
fn travelled(pass: Pass) -> (f32, bool) {
    match pass {
        Pass::Flying(p) => (p, false),
        Pass::Holding(p) => (p, true),
        Pass::Closing => (1.0, false),
        Pass::OnThePad | Pass::Planned => (0.0, false),
    }
}

/// Signal's ring is cut into this many meter segments (SIG-RNG-01).
pub(crate) const SIGNAL_SEGMENTS: u32 = 16;

/// The segments of `n` lit at `p` (0 to 1): a segment lights whole, once passed (SIG-RNG-01).
pub(crate) fn lit(p: f32, n: u32) -> u32 {
    (p.clamp(0.0, 1.0) * n as f32).floor() as u32
}

/// The shared roles for `s.palette`; palette 0, the look's own mood, is not in that table and
/// Aurora stands in (as `overlays::Skin::shared`).
fn roles(palette: usize, dark: bool) -> palette::Roles {
    palette::roles(palette.saturating_sub(1), dark)
}

/// Signal: the ring in meter segments, unlit ones in the meter's unlit ink, lit in the tint.
pub(crate) fn signal(palette: usize, dark: bool, tint: Color) -> Material {
    let r = roles(palette, dark);
    let unlit = r.text_primary.alpha(if dark { 0.09 } else { 0.08 });
    Material { track: unlit, under: r.bg_primary, stroke: tint, craft: tint, segments: Some(SIGNAL_SEGMENTS) }
}

/// Shelf: the record is the track, its rim filling in the tint. A rim has no craft.
pub(crate) fn shelf(_palette: usize, _dark: bool, tint: Color) -> Material {
    use crate::directions::shelf::theme;
    Material { track: theme::rim(), under: Color::TRANSPARENT, stroke: tint, craft: Color::TRANSPARENT, segments: None }
}

/// Orbit: the track of its star chart, the arc and the craft in the tint.
pub(crate) fn orbit(palette: usize, dark: bool, tint: Color) -> Material {
    let r = roles(palette, dark);
    let track = r.bg_primary.mix(r.text_primary, if dark { 0.28 } else { 0.24 });
    Material { track, under: r.bg_primary, stroke: tint, craft: tint, segments: None }
}

/// The craft's length along the ring, in points.
const CRAFT: f32 = 4.0;

/// Orbit's craft as a stroke (MOTION-SYSTEM 4.11 item 7): one turn plus a craft's length,
/// from just before twelve o'clock, alone in its shape so that [`craft_span`] trims it alone.
pub(crate) fn craft(size: f32, width: f32, color: Color) -> Shape {
    let (c, lead) = (size / 2.0, CRAFT / (size / 2.0 - 3.0));
    Shape::new().stroke_with(arc(c, c, c - 3.0, -FRAC_PI_2 - lead, TAU + lead, 128), false, width, LineCap::Round, color)
}

/// The trim of [`craft`] that ends at `travelled` (0 to 1) of the ring.
pub(crate) fn craft_span(size: f32, travelled: f32) -> [f32; 2] {
    let lead = CRAFT / (size / 2.0 - 3.0);
    let total = TAU + lead;
    let end = (lead + TAU * travelled.clamp(0.0, 1.0)) / total;
    [end - lead / total, end]
}

// throwaway, removed at step 80 (K54)
/// The ring in the three materials, in every pass, at row and stage size, with one Downloads
/// row per look: the pictures for the owner's approval of the materials (Plan B step 10).
pub(crate) fn gallery(s: &crate::gate::State, cx: &mut rune::Cx<crate::gate::State>) -> rune::El {
    use ahg_engine::types::SourcePlatform as Src;
    use rune::{El, TextStyle, col, el, row, text};
    type Make = fn(usize, bool, Color) -> Material;
    let dark = s.dark(cx);
    let r = roles(s.palette, dark);
    let passes = [
        Pass::Planned,
        Pass::OnThePad,
        Pass::Flying(0.0),
        Pass::Flying(0.25),
        Pass::Flying(0.5),
        Pass::Flying(0.75),
        Pass::Flying(1.0),
        Pass::Closing,
        Pass::Holding(0.5),
    ];
    let looks: [(usize, Make, Src); 3] = [(3, signal, Src::Youtube), (4, shelf, Src::Spotify), (5, orbit, Src::Soundcloud)];
    let label = |words: String, size: f32, ink: Color| text(words).font(TextStyle::new(size, 500.0)).color(ink).single_line();
    let ring = |look: usize, size: f32, pass: Pass, m: &Material| -> El {
        let mut cell = el().size(size, size);
        if look == 4 {
            // The record itself, its rim inset by the ring's 3 pt.
            cell = cell.radius(size / 2.0).fill(crate::directions::shelf::theme::vinyl());
        }
        if look != 5 {
            return cell.shape(shape(size, pass, m));
        }
        // Orbit: the body without its disc, then the craft as trimmed strokes (gap, craft, hollow).
        let body = Material { track: m.track, under: Color::TRANSPARENT, stroke: m.stroke, craft: Color::TRANSPARENT, segments: None };
        cell = cell.shape(shape(size, pass, &body));
        if pass == Pass::Planned {
            return cell;
        }
        let (at, hollow) = travelled(pass);
        let [a, b] = craft_span(size, at);
        let mut layers = vec![(9.0, m.under), (5.0, m.craft)];
        if hollow {
            layers.push((2.0, m.under));
        }
        cell.children(layers.into_iter().map(|(w, ink)| el().size(size, size).absolute(0.0, 0.0).shape(craft(size, w, ink).trimmed(a, b))))
    };
    let bar = |w: f32, h: f32, a: f32| el().w(w).h(h).radius(h / 2.0).fill(r.text_primary.alpha(a));
    col().w_full().h_full().fill(r.bg_primary).padding(24.0).gap(20.0).children(looks.map(|(look, make, src)| {
        let m = make(s.palette, dark, palette::platform(src, dark));
        let cells = |size: f32| {
            row().gap(12.0).children(passes.map(|p| {
                col().gap(6.0).items_center().child(ring(look, size, p, &m)).child(label(format!("{p:?}"), 11.0, r.text_secondary))
            }))
        };
        let downloads_row = row()
            .w(420.0)
            .h(56.0)
            .px(14.0)
            .gap(12.0)
            .center_y()
            .radius(8.0)
            .fill(r.bg_secondary)
            .child(col().grow(1.0).gap(8.0).child(bar(200.0, 10.0, 0.16)).child(bar(120.0, 8.0, 0.09)))
            .child(ring(look, 28.0, Pass::Flying(0.6), &m));
        col()
            .gap(10.0)
            .child(label(crate::gate::PRESETS[look].0.into(), 15.0, r.text_primary))
            .child(row().gap(40.0).center_y().child(cells(28.0)).child(downloads_row))
            .child(cells(160.0))
    }))
}

#[cfg(test)]
mod tests {
    use ahg_engine::queue::{TaskInit, create_task};
    use ahg_engine::types::SourcePlatform as Src;

    use rune::oklcha;

    use super::*;

    fn task(status: St, source: Src, percent: f64) -> DownloadTask {
        let mut t = create_task(&TaskInit { url: Some("https://example.com/a".into()), ..Default::default() }, "id".into(), 0);
        t.progress.status = status;
        t.progress.percent = percent;
        t.source = source;
        t
    }

    /// Every status with its expected pass; the match is exhaustive, so a new variant breaks the build.
    fn expected(st: St) -> Option<Pass> {
        match st {
            St::Queued => Some(Pass::Planned),
            St::FetchingInfo => Some(Pass::OnThePad),
            St::Downloading => Some(Pass::Flying(0.4)),
            St::Converting | St::Embedding => Some(Pass::Closing),
            St::Paused | St::Retrying => Some(Pass::Holding(0.4)),
            St::Done | St::Failed | St::Cancelled => None,
        }
    }

    #[test]
    fn pass_of_covers_every_status() {
        use St::*;
        let all = [Queued, FetchingInfo, Downloading, Converting, Embedding, Paused, Retrying, Failed, Done, Cancelled];
        assert_eq!(all.len(), 10);
        for st in all {
            assert_eq!(pass_of(&task(st, Src::Youtube, 40.0)), expected(st), "{st:?}");
        }
    }

    #[test]
    fn tint_is_the_platform_colour_for_every_source() {
        use Src::*;
        let all = [
            Youtube,
            Spotify,
            Soundcloud,
            Instagram,
            Tiktok,
            Facebook,
            Direct,
            Twitter,
            Vimeo,
            Twitch,
            Bandcamp,
            Reddit,
            Dailymotion,
            Mixcloud,
            Rumble,
            Generic,
            Unknown,
        ];
        // Exhaustive: a new platform breaks the build here until it is listed above.
        for s in all {
            match s {
                Youtube | Spotify | Soundcloud | Instagram | Tiktok | Facebook | Direct | Twitter | Vimeo | Twitch | Bandcamp | Reddit
                | Dailymotion | Mixcloud | Rumble | Generic | Unknown => {}
            }
            for dark in [true, false] {
                assert_eq!(tint(&task(St::Queued, s, 0.0), dark), palette::platform(s, dark));
            }
        }
    }

    /// Orbit's ring as it was drawn before the move (`directions/orbit/draw.rs::orbit`), verbatim.
    fn old_orbit(size: f32, pass: Pass, track: Color, ink: Color, under: Color) -> Shape {
        let c = size / 2.0;
        let r = c - 3.0;
        let top = -FRAC_PI_2;
        let mut s = Shape::new();
        if pass == Pass::Planned {
            let n = ((TAU * r) / 3.0).clamp(8.0, 32.0) as usize;
            for i in 0..n {
                let a = top + TAU * i as f32 / n as f32;
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

    #[test]
    fn shape_matches_the_old_orbit_numbers() {
        let (track, ink, under) = (oklcha(0.4, 0.0, 0.0, 1.0), oklcha(0.7, 0.15, 60.0, 1.0), oklcha(0.2, 0.0, 0.0, 1.0));
        let m = Material { track, under, stroke: ink, craft: ink, segments: None };
        let passes =
            [Pass::Flying(0.0), Pass::Flying(0.5), Pass::Flying(1.0), Pass::Planned, Pass::OnThePad, Pass::Closing, Pass::Holding(0.5)];
        for size in [24.0, 48.0] {
            for p in passes {
                assert_eq!(shape(size, p, &m), old_orbit(size, p, track, ink, under), "{p:?} at {size}");
            }
        }
    }

    #[test]
    fn segments_light_whole_once_passed() {
        let n = SIGNAL_SEGMENTS;
        // Every whole step, just under one, 99.9 % and past the end, with the segments lit.
        let mut table: Vec<(f32, u32)> = (0..=n).map(|k| (k as f32 / n as f32, k)).collect();
        table.extend([(0.999 / n as f32, 0), (0.999, n - 1), (1.5, n), (-0.2, 0)]);
        assert!(!table.is_empty());
        let m = signal(0, true, oklcha(0.7, 0.15, 60.0, 1.0));
        for (p, on) in table {
            assert_eq!(lit(p, n), on, "{p}");
            // The picture is the lit count's: the same anywhere inside one step, different across one.
            let step = on as f32 / n as f32;
            assert_eq!(shape(48.0, Pass::Flying(p), &m), shape(48.0, Pass::Flying(step), &m), "{p}");
        }
        let one = 1.0 / n as f32;
        assert_ne!(shape(48.0, Pass::Flying(one * 0.999), &m), shape(48.0, Pass::Flying(one), &m));
    }
}
