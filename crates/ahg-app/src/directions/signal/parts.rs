//! The small parts every Signal screen is built from: type, the segmented
//! meter, keycaps, buttons, artwork.
use ahg_engine::types::{DownloadTask, SourcePlatform};
use rune::{Color, El, Icon, PopupMenu, Span, SurfaceFit, TextStyle, Tx, col, el, icon, rich, row, text};

use super::theme::{Pal, source_color};
use crate::actions;
use crate::copy;
use crate::gate::{Filter, State};
use crate::icons;
use crate::keys::{self, RowAct};
use crate::ring;
use crate::settings_spec::{self, Kind, Row};
use serde_json::Value;

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
        let c = if i >= on {
            unlit
        } else if held {
            lit.alpha(0.5)
        } else if i + 1 == on {
            lit
        } else {
            lit.alpha(0.78)
        };
        let s = el().h(h).radius(1.0).fill(c);
        m = m.child(match seg {
            Some(w) => s.w(w),
            None => s.grow(1.0),
        });
    }
    m
}

/// A ring's square in a row, a well or the deck.
pub const RING: f32 = 28.0;

/// Signal's ring material (`ring::signal`), its unlit segments in the look's own ink and a
/// step stronger than the meter's, so a dark ring still reads at row size.
pub fn ring_material(s: &State, p: &Pal, tint: Color) -> ring::Material {
    // `under` from the look's own ground: ring.rs stands Aurora in for palette 0.
    ring::Material { track: p.text.alpha(if p.dark { 0.16 } else { 0.18 }), under: p.bg, ..ring::signal(s.palette, p.dark, tint) }
}

/// A task's ring, tinted by its platform.
pub fn task_ring(s: &State, t: &DownloadTask, p: &Pal) -> El {
    let rank = s.tasks.iter().position(|x| x.task_id == t.task_id).unwrap_or(0);
    let m = ring_material(s, p, ring::tint(t, p.dark));
    // TODO(motion): the real reduced-motion flag (step 52).
    ring::view(&format!("ring-{}", t.task_id), RING, &s.rings, &t.task_id, &m, false, rank)
}

/// The ring with nothing in it: every segment unlit, still.
pub fn idle_ring(s: &State, p: &Pal) -> El {
    el().size(RING, RING).shape(ring::shape(RING, ring::Pass::Flying(0.0), &ring_material(s, p, p.accent)))
}

/// A key as printed on the board.
pub fn keycap(key: &str, p: &Pal) -> El {
    el().h(20.0).px(8.0).center().radius(4.0).fill(p.raised).border(1.0, p.line_strong).child(label(key, mono(11.0, 500.0), p.text2))
}

/// A square icon button whose label is its tooltip.
pub fn icon_button(i: &'static Icon, help: &str, color: Color, p: &Pal) -> El {
    el().size(28.0, 28.0)
        .center()
        .radius(6.0)
        .hover(p.hover)
        .press(p.press)
        .cursor(rune::Cursor::Pointer)
        .focusable(true)
        .role("button")
        .label(help)
        .help(help)
        .child(icon(i, 16.0).color(color))
}

/// A quiet text button with an optional leading icon.
pub fn ghost(i: Option<&'static Icon>, words: &str, color: Color, p: &Pal) -> El {
    let mut b = row()
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .hover(p.hover)
        .press(p.press)
        .cursor(rune::Cursor::Pointer)
        .focusable(true)
        .role("button");
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

/// A text setting from `State.settings`; empty when unset.
pub fn config_text(s: &State, key: &str) -> String {
    s.settings.get(key).and_then(|v| v.as_str()).unwrap_or("").to_owned()
}

/// The settings rows by config key: the values on show and the label of a chosen option.
pub struct Spec(Vec<Row>);

impl Spec {
    pub fn new(s: &State) -> Spec {
        Spec(settings_spec::sections(s).into_iter().flat_map(|section| section.rows).collect())
    }
    pub fn value(&self, key: &str) -> Value {
        self.0.iter().find(|r| r.key == Some(key)).map_or(Value::Null, |r| r.value.clone())
    }
    /// The value of the row with this label, for rows that are not a setting (the plan).
    pub fn text_where(&self, label: &str) -> String {
        self.0.iter().find(|r| r.label == label).and_then(|r| r.value.as_str()).unwrap_or_default().to_owned()
    }
    pub fn text(&self, key: &str) -> String {
        match self.value(key) {
            Value::String(t) => t,
            Value::Null => String::new(),
            other => other.to_string(),
        }
    }
    /// The chosen option's label, or the raw value when it is not one of the options.
    pub fn pick(&self, key: &str) -> String {
        let value = self.text(key);
        let label = self.0.iter().find(|r| r.key == Some(key)).and_then(|r| match &r.kind {
            Kind::Select(options) => options.iter().find(|o| o.value == value).map(|o| o.label.clone()),
            _ => None,
        });
        label.unwrap_or(value)
    }
}

/// A two-step action asking once more: the question, a quiet Cancel, a red Confirm.
pub fn confirm(question: &str, aria: &str, p: &Pal) -> El {
    row()
        .gap(4.0)
        .center_y()
        .role("group")
        .label(aria)
        .child(label(question, sans(13.0, 500.0), p.text2).mr(4.0))
        .child(ghost(None, copy::DL_CONFIRM_CANCEL, p.text2, p).key(keys::CONFIRM_NO).on_press(actions::disarm))
        .child(
            ghost(None, copy::DL_CONFIRM_YES, p.deep, p)
                .fill(p.error)
                .hover(p.deep.alpha(0.12))
                .key(keys::CONFIRM_YES)
                .on_press(actions::confirm),
        )
}

/// A row's button: its key, and the action it answers to with the row's task.
pub fn row_act(b: El, which: RowAct, id: &str) -> El {
    let key = keys::row_action(which, id);
    let id = id.to_owned();
    b.key(key).on_press_tx(move |s: &mut State, tx: &mut Tx| match which {
        RowAct::Promote => actions::promote(s, tx, &id),
        RowAct::Pause => actions::pause(s, tx, &id),
        RowAct::Resume => actions::resume(s, tx, &id),
        RowAct::Cancel => actions::cancel(s, tx, &id),
        RowAct::CopyDetails => actions::copy_details(s, tx, &id),
        RowAct::Fix => actions::fix(s, tx, &id),
        RowAct::DownloadAgain => actions::download_again(s, tx, &id),
        RowAct::Retry => actions::retry(s, tx, &id),
        RowAct::OpenFile => actions::open_file(s, tx, &id),
        RowAct::ShowInFolder => actions::show_in_folder(s, tx, &id),
        RowAct::Remove => actions::remove(s, tx, &id),
    })
}

/// A control that opens a native menu of choices under it; `pick` gets the chosen id.
// TODO(shared): `actions::pick_setting(s, tx, key, options)`; the menu is opened here.
pub fn popup(b: El, key: &str, menu: PopupMenu, pick: impl Fn(&mut State, &str) + Clone + 'static) -> El {
    let anchor = key.to_owned();
    b.key(key).on_press_tx(move |_: &mut State, tx: &mut Tx| {
        let pick = pick.clone();
        tx.popup_menu(menu.clone(), anchor.clone(), move |s: &mut State, id: Option<&str>, _: &mut Tx| {
            if let Some(id) = id {
                pick(s, id);
            }
        });
    })
}

// TODO(shared): `format::selection(tasks, selected) -> (active, finished)`; Signal and One bar use this copy.
/// The selected rows that can be canceled (active) and removed (finished: done, failed, canceled).
pub fn selection(s: &State) -> (usize, usize) {
    let picked = || s.tasks.iter().filter(|t| s.selected.contains(&t.task_id));
    (picked().filter(|t| crate::format::is_active(t)).count(), picked().filter(|t| !crate::format::is_active(t)).count())
}

// TODO(shared): `copy::empty_filter_title(Filter)`; the table of CONTENT.md 3.4, `All` included.
/// The title of a list that the filter and the search empty, and its glyph.
pub fn filtered_empty(filter: Filter) -> (&'static str, &'static Icon) {
    match filter {
        Filter::Active => (copy::EMPTY_FILTER_ACTIVE, &icons::CLOCK),
        Filter::Done => (copy::EMPTY_FILTER_DONE, &icons::CLOCK),
        Filter::Failed => (copy::EMPTY_FILTER_FAILED, &icons::CHECK_SIGNAL),
        Filter::All => (copy::EMPTY_FILTER_FAILED, &icons::CLOCK),
    }
}
