//! The small parts every Signal screen is built from: type, the segmented
//! meter, keycaps, buttons, artwork.
use ahg_engine::types::{DownloadTask, SourcePlatform};
use rune::{Color, El, Icon, Span, SurfaceFit, TextStyle, col, el, icon, rich, row, text};

use super::icons;
use super::theme::{Pal, source_color};
use crate::gate::State;

/// Interface type (Geist).
pub fn sans(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight)
}

/// Figures (Geist Mono, tabular by nature).
pub fn mono(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).mono().features("'tnum' 1")
}

pub fn label(s: impl Into<String>, style: TextStyle, c: Color) -> El {
    text(s).font(style).color(c)
}

/// One line of text in several colours, built piece by piece.
#[derive(Default)]
pub struct Line {
    s: String,
    spans: Vec<Span>,
}

impl Line {
    pub fn new() -> Line {
        Line::default()
    }
    fn push(mut self, part: &str, color: Color, mono: bool, weight: Option<f32>) -> Line {
        if part.is_empty() {
            return self;
        }
        let at = self.s.len();
        self.s.push_str(part);
        self.spans.push(Span { color: Some(color), mono, weight, ..Span::new(at, self.s.len()) });
        self
    }
    pub fn add(self, part: &str, color: Color) -> Line {
        self.push(part, color, false, None)
    }
    pub fn fig(self, part: &str, color: Color) -> Line {
        self.push(part, color, true, None)
    }
    pub fn strong(self, part: &str, color: Color) -> Line {
        self.push(part, color, false, Some(500.0))
    }
    /// One line, cut with an ellipsis when it does not fit.
    pub fn one(self, style: TextStyle) -> El {
        rich(self.s, self.spans).font(style).single_line().truncate().min_w(0.0)
    }
    /// Wraps at the element's width.
    pub fn wrap(self, style: TextStyle) -> El {
        rich(self.s, self.spans).font(style)
    }
}

/// The instrument. `n` segments, the first `frac` of them lit; the last lit one
/// is the head and stays full strength while the rest of the run sits a step back.
/// `seg` is a segment's width; `None` lets the segments share the parent's width.
pub fn meter(frac: f64, n: usize, seg: Option<f32>, h: f32, gap: f32, lit: Color, unlit: Color) -> El {
    meter_held(frac, n, seg, h, gap, lit, unlit, false)
}

/// A meter that is not moving (paused, waiting to retry): no head, the run dimmed,
/// so the state reads without its colour.
#[allow(clippy::too_many_arguments)]
pub fn meter_held(frac: f64, n: usize, seg: Option<f32>, h: f32, gap: f32, lit: Color, unlit: Color, held: bool) -> El {
    let frac = if frac.is_finite() { frac.clamp(0.0, 1.0) } else { 0.0 };
    let on = (frac * n as f64).round() as usize;
    let on = if frac > 0.0 { on.max(1) } else { 0 };
    let mut m = row().gap(gap).h(h);
    m = if seg.is_none() { m.w_full() } else { m };
    for i in 0..n {
        let c = if i >= on { unlit } else if held { lit.alpha(0.5) } else if i + 1 == on { lit } else { lit.alpha(0.78) };
        let s = el().h(h).radius(1.0).fill(c);
        m = m.child(match seg {
            Some(w) => s.w(w),
            None => s.grow(1.0),
        });
    }
    m
}

/// A key as printed on the board.
pub fn keycap(key: &str, p: &Pal) -> El {
    el().h(20.0).px(6.0).center().radius(4.0).fill(p.raised).border(1.0, p.line_strong).child(label(key, mono(11.0, 500.0), p.text2))
}

/// A square icon button whose label is its tooltip.
pub fn icon_button(i: &'static Icon, help: &str, color: Color, p: &Pal) -> El {
    el().size(28.0, 28.0).center().radius(6.0).hover(p.hover).press(p.press).cursor(rune::Cursor::Pointer).focusable(true).role("button").label(help).help(help).child(icon(i, 16.0).color(color))
}

/// A quiet text button with an optional leading icon.
pub fn ghost(i: Option<&'static Icon>, words: &str, color: Color, p: &Pal) -> El {
    let mut b = row().h(32.0).px(12.0).gap(8.0).center_y().radius(8.0).hover(p.hover).press(p.press).cursor(rune::Cursor::Pointer).focusable(true).role("button");
    if let Some(i) = i {
        b = b.child(icon(i, 16.0).color(color));
    }
    b.child(label(words, sans(13.0, 500.0), color))
}

/// A bordered button on the raised surface.
pub fn button(i: Option<&'static Icon>, words: &str, color: Color, p: &Pal) -> El {
    ghost(i, words, color, p).fill(p.raised).border(1.0, p.line)
}

pub fn checkbox(p: &Pal) -> El {
    el().size(16.0, 16.0).radius(4.0).border(1.0, p.line_strong).hover(p.hover).focusable(true).role("checkbox")
}

pub fn platform_icon(source: SourcePlatform) -> &'static Icon {
    match source {
        SourcePlatform::Youtube => &icons::YOUTUBE,
        SourcePlatform::Spotify => &icons::SPOTIFY,
        SourcePlatform::Soundcloud => &icons::SOUNDCLOUD,
        SourcePlatform::Instagram | SourcePlatform::Tiktok | SourcePlatform::Facebook => &icons::REELS,
        _ => &icons::TRACK,
    }
}

/// A task's cover, or a flat tint of its source with the source's mark.
/// `w: None` takes the parent's width.
pub fn art(s: &State, t: &DownloadTask, w: Option<f32>, h: f32, radius: f32, p: &Pal) -> El {
    let base = el().h(h).radius(radius);
    let base = match w {
        Some(w) => base.w(w),
        None => base.w_full(),
    };
    match s.cover(&t.task_id) {
        Some(img) => base.fill(p.raised).pixels_fit(img.pixels(), SurfaceFit::Fill),
        None => {
            let c = source_color(p, t.source);
            base.fill(c.alpha(0.14)).center().child(icon(platform_icon(t.source), (h * 0.42).min(28.0)).color(c))
        }
    }
}

/// A small dot, the tally light.
pub fn dot(c: Color, size: f32) -> El {
    el().size(size, size).radius(size / 2.0).fill(c)
}

/// A labelled column: a wrapper for the common "title, then body" stack.
pub fn stack(gap: f32) -> El {
    col().gap(gap).min_w(0.0)
}
