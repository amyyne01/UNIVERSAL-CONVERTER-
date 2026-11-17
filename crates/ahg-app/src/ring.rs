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
/// `segments` is for a look that draws the ring in pieces; `None` is one unbroken circle
/// (the only form `shape` draws so far).
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
    s = s.stroke_with(arc(c, c, r, 0.0, TAU, 64), true, 1.5, LineCap::Butt, m.track);
    let (travelled, hollow) = match pass {
        Pass::Flying(p) => (p, false),
        Pass::Holding(p) => (p, true),
        Pass::Closing => (1.0, false),
        Pass::OnThePad | Pass::Planned => (0.0, false),
    };
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
}
