//! The progress ring shared by the presets: its model (`Pass`, `pass_of`, `tint`) and its
//! geometry (`shape`), which a look dresses through a `Material`.
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, TAU};
use std::mem::discriminant;

use ahg_engine::types::{DownloadStatus as St, DownloadTask, RETRY_BACKOFF_MS};
use rune::{Color, LineCap, Shape};

use crate::directions::orbit::model::percent;
use crate::{palette, timeline};

/// `n` points on a circle from `from` radians, clockwise on screen, over `sweep` radians.
pub(crate) fn arc(cx: f32, cy: f32, r: f32, from: f32, sweep: f32, n: usize) -> Vec<[f32; 2]> {
    (0..=n).map(|i| from + sweep * i as f32 / n as f32).map(|a| [cx + r * a.cos(), cy + r * a.sin()]).collect()
}

pub(crate) fn disc(cx: f32, cy: f32, r: f32) -> Vec<[f32; 2]> {
    let mut d = arc(cx, cy, r, 0.0, TAU, 40);
    d.pop();
    d
}

/// How a download sits on its orbit (MOTION-SYSTEM 5.1). Payloads are the arc's value, 0 to 1.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pass {
    /// Waiting: the orbit is only planned, a dotted circle.
    Planned,
    /// Started, nothing measured yet: the craft waits at twelve o'clock.
    OnThePad,
    /// Moving at a known percent: the arc from twelve o'clock, the craft at its head.
    Flying(f32),
    /// Downloading with no changed report for `STALL_AFTER` (D47): the arc where it stopped.
    Stalled(f32),
    /// Downloaded and being finished (converting, tagging): the orbit closed, the craft home.
    Closing,
    /// Held at a percent (paused): the arc stops, the craft hollow.
    Holding(f32),
    /// Waiting to retry: the arc (the value it empties from) is drawn hollow; the emptying is
    /// the later motion step's, from `Rings::countdown`.
    Countdown(f32),
    /// Finished, held for `RING_HOLD`: the full arc.
    Done,
    /// Finished and past the hold, or finished when first seen: no ring, the look's landed form.
    Landed,
    /// Failed: the arc where it stopped.
    Failed(f32),
}

/// The pass a task is on; `None` once cancelled (no ring). `seen_at` is when its report last changed,
/// `landed` whether the finish chain has run (or the task was already `Done` when first seen).
pub fn pass_of_at(t: &DownloadTask, seen_at: f64, now: f64, landed: bool) -> Option<Pass> {
    let p = percent(t);
    Some(match t.progress.status {
        St::Queued => Pass::Planned,
        St::FetchingInfo => Pass::OnThePad,
        St::Downloading if timeline::stalled(seen_at, now) => Pass::Stalled(p),
        St::Downloading => Pass::Flying(p),
        St::Converting | St::Embedding => Pass::Closing,
        St::Paused => Pass::Holding(p),
        St::Retrying => Pass::Countdown(p),
        St::Done if landed => Pass::Landed,
        St::Done => Pass::Done,
        St::Failed => Pass::Failed(p),
        St::Cancelled => return None,
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
    if pass == Pass::Landed {
        return s;
    }
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
        Pass::Flying(p) | Pass::Stalled(p) | Pass::Failed(p) => (p, false),
        Pass::Holding(p) | Pass::Countdown(p) => (p, true),
        Pass::Closing | Pass::Done => (1.0, false),
        Pass::OnThePad | Pass::Planned | Pass::Landed => (0.0, false),
    }
}

/// The last stretch of the arc where the craft's head shows (Orbit's craft), as a fraction.
const HEAD: f32 = 0.05;
/// A stalled arc is drawn at this opacity under reduced motion (5.5); otherwise the loop breathes it.
const STALLED_STILL: f32 = 0.6;

/// One task's ring. `span`, `stepped` and `head` are the values the view draws.
// TODO(live): plain f32 now; they become Live<[f32; 2]> (span, head) when the motion step binds them.
#[derive(Default)]
struct Ring {
    pass: Option<Pass>,
    /// The arc, `[0, percent / 100]`; a lower report snaps, it never lags above the report.
    span: [f32; 2],
    /// The span's end rounded down to whole Signal segments.
    stepped: f32,
    /// A short span ending at the value (Orbit's craft).
    head: [f32; 2],
    /// When the report (percent, downloaded) or the status last changed.
    seen_at: f64,
    seen: (f64, f64, Option<St>),
    landed: bool,
    /// A retry's wait: (entered at, seconds), set once on entering `Retrying`.
    countdown: Option<(f64, f64)>,
}

/// Every task's ring and the overall one (5.2). `sync` is the only writer.
#[derive(Default)]
pub struct Rings {
    rings: HashMap<String, Ring>,
    /// The overall ring: `[0, mean percent]` of the tasks not yet finished, shown while any is.
    overall: [f32; 2],
    overall_on: bool,
}

/// What a `sync` changed.
#[derive(Default, Debug)]
pub struct Synced {
    /// Some ring is new or entered another kind of pass; values alone do not count.
    pub pass_changed: bool,
    /// Tasks that went to `Done` in this sync (first-seen `Done` is `Landed` and not listed).
    pub finished: Vec<String>,
    pub failed: Vec<String>,
    /// The earliest moment a flying task becomes stalled.
    pub next_stall: Option<f64>,
}

impl Rings {
    /// Brings every ring to the tasks as they are `now`; rings of tasks no longer listed go.
    pub fn sync(&mut self, tasks: &[DownloadTask], now: f64) -> Synced {
        let mut out = Synced::default();
        self.rings.retain(|id, _| tasks.iter().any(|t| &t.task_id == id));
        let (mut sum, mut active) = (0.0, 0);
        for t in tasks {
            let pr = &t.progress;
            let seen = (pr.percent, pr.downloaded, Some(pr.status));
            let new = !self.rings.contains_key(&t.task_id);
            let r = self.rings.entry(t.task_id.clone()).or_default();
            if new {
                (r.seen, r.landed) = (seen, pr.status == St::Done);
            }
            if new || r.seen != seen {
                (r.seen, r.seen_at) = (seen, now);
            }
            let p = percent(t);
            (r.span, r.stepped, r.head) = ([0.0, p], lit(p, SIGNAL_SEGMENTS) as f32 / SIGNAL_SEGMENTS as f32, [(p - HEAD).max(0.0), p]);
            r.countdown = match (pr.status, r.countdown) {
                (St::Retrying, None) => {
                    let step = (t.retry_count.unwrap_or(0.0).max(0.0) as usize).min(RETRY_BACKOFF_MS.len() - 1);
                    Some((now, RETRY_BACKOFF_MS[step] as f64 / 1000.0))
                }
                (St::Retrying, c) => c,
                _ => None,
            };
            let pass = pass_of_at(t, r.seen_at, now, r.landed);
            let changed = new || pass.as_ref().map(discriminant) != r.pass.as_ref().map(discriminant);
            out.pass_changed |= changed;
            if changed && !new {
                match pass {
                    Some(Pass::Done) => out.finished.push(t.task_id.clone()),
                    Some(Pass::Failed(_)) => out.failed.push(t.task_id.clone()),
                    _ => {}
                }
            }
            r.pass = pass;
            if !matches!(pass, Some(Pass::Done | Pass::Landed | Pass::Failed(_)) | None) {
                (sum, active) = (sum + p, active + 1);
            }
        }
        (self.overall_on, self.overall) = (active > 0, [0.0, if active > 0 { sum / active as f32 } else { 0.0 }]);
        let seen: Vec<_> = tasks.iter().map(|t| (t.task_id.as_str(), t.progress.status, self.rings[&t.task_id].seen_at)).collect();
        out.next_stall = timeline::next_stall(&seen, now);
        out
    }

    /// The task's pass; `None` when it has no ring.
    pub fn pass(&self, task_id: &str) -> Option<Pass> {
        self.rings.get(task_id)?.pass
    }

    /// The finish chain's last step: `Done` becomes `Landed`.
    pub fn land(&mut self, task_id: &str) {
        if let Some(r) = self.rings.get_mut(task_id) {
            r.landed = true;
            if r.pass == Some(Pass::Done) {
                r.pass = Some(Pass::Landed);
            }
        }
    }
}

/// What to draw for a pass: the pass and its opacity, or nothing. Reduced motion draws a stalled
/// arc still at `STALLED_STILL` (5.5); the waiting arc is always still here.
fn drawn(pass: Option<Pass>, reduced: bool) -> Option<(Pass, f32)> {
    match pass? {
        Pass::Landed => None,
        p @ Pass::Stalled(_) if reduced => Some((p, STALLED_STILL)),
        p => Some((p, 1.0)),
    }
}

fn ring_el(key: &str, size: f32, pass: Option<Pass>, mat: &Material, reduced: bool) -> rune::El {
    let cell = rune::el().key(key).size(size, size);
    match drawn(pass, reduced) {
        Some((p, a)) => cell.opacity(a).shape(shape(size, p, mat)),
        None => cell,
    }
}

// `rank` is the creation order the motion step caps loops by (`RING_LOOPS_MAX`); `reduced` replaces
// `&motion::Ctx` of 5.2, which does not exist yet. Drawing only: no motion is attached here.

/// A task's ring.
pub fn view(key: &str, size: f32, rings: &Rings, task_id: &str, mat: &Material, reduced: bool, _rank: usize) -> rune::El {
    ring_el(key, size, rings.pass(task_id), mat, reduced)
}

/// The ring of something waiting that is not a task (a look-up, a search): the pad, still.
pub fn waiting(key: &str, size: f32, mat: &Material, reduced: bool, _rank: usize) -> rune::El {
    ring_el(key, size, Some(Pass::OnThePad), mat, reduced)
}

/// The overall ring: nothing while no task is unfinished.
pub fn overall(key: &str, size: f32, rings: &Rings, mat: &Material, reduced: bool) -> rune::El {
    ring_el(key, size, rings.overall_on.then_some(Pass::Flying(rings.overall[1])), mat, reduced)
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
    use crate::motion::STALL_AFTER;

    fn task(status: St, source: Src, percent: f64) -> DownloadTask {
        let mut t = create_task(&TaskInit { url: Some("https://example.com/a".into()), ..Default::default() }, "id".into(), 0);
        t.progress.status = status;
        t.progress.percent = percent;
        t.source = source;
        t
    }

    const T0: f64 = 100.0;

    /// The pass for a status, per clock case: `fresh` (a report just seen), `stalled` (none for
    /// `STALL_AFTER`), `landed` (the finish chain has run). The match is exhaustive: a new status breaks the build.
    fn expected(st: St, case: &str) -> Option<Pass> {
        let p = 0.4;
        match st {
            St::Queued => Some(Pass::Planned),
            St::FetchingInfo => Some(Pass::OnThePad),
            St::Downloading if case == "stalled" => Some(Pass::Stalled(p)),
            St::Downloading => Some(Pass::Flying(p)),
            St::Converting | St::Embedding => Some(Pass::Closing),
            St::Paused => Some(Pass::Holding(p)),
            St::Retrying => Some(Pass::Countdown(p)),
            St::Done if case == "landed" => Some(Pass::Landed),
            St::Done => Some(Pass::Done),
            St::Failed => Some(Pass::Failed(p)),
            St::Cancelled => None,
        }
    }

    const ALL: [St; 10] = [
        St::Queued,
        St::FetchingInfo,
        St::Downloading,
        St::Converting,
        St::Embedding,
        St::Paused,
        St::Retrying,
        St::Failed,
        St::Done,
        St::Cancelled,
    ];

    #[test]
    fn pass_of_at_covers_every_status_fresh_stalled_and_landed() {
        assert_eq!(ALL.len(), 10);
        let cases = [("fresh", T0, false), ("stalled", T0 + STALL_AFTER + 1.0, false), ("landed", T0, true)];
        assert!(!cases.is_empty());
        for st in ALL {
            for (case, now, landed) in cases {
                assert_eq!(pass_of_at(&task(st, Src::Youtube, 40.0), T0, now, landed), expected(st, case), "{st:?} {case}");
            }
        }
    }

    #[test]
    fn stall_begins_exactly_at_stall_after() {
        let t = task(St::Downloading, Src::Youtube, 40.0);
        assert_eq!(pass_of_at(&t, T0, T0 + STALL_AFTER - 0.001, false), Some(Pass::Flying(0.4)));
        assert_eq!(pass_of_at(&t, T0, T0 + STALL_AFTER, false), Some(Pass::Stalled(0.4)));
    }

    fn tasks(list: &[(&str, St, f64)]) -> Vec<DownloadTask> {
        list.iter()
            .map(|(id, st, pct)| {
                let mut t = task(*st, Src::Youtube, *pct);
                t.task_id = (*id).into();
                t
            })
            .collect()
    }

    #[test]
    fn a_values_only_report_changes_no_pass() {
        let mut rings = Rings::default();
        let a = tasks(&[("a", St::Downloading, 10.0)]);
        assert!(rings.sync(&a, T0).pass_changed, "a new ring is a change");
        let s = rings.sync(&tasks(&[("a", St::Downloading, 30.0)]), T0 + 1.0);
        assert!(!s.pass_changed);
        assert_eq!(rings.pass("a"), Some(Pass::Flying(0.3)));
        let s = rings.sync(&tasks(&[("a", St::Converting, 100.0)]), T0 + 2.0);
        assert!(s.pass_changed);
        assert_eq!(rings.pass("a"), Some(Pass::Closing));
    }

    #[test]
    fn a_silent_task_stalls_and_a_report_wakes_it() {
        let mut rings = Rings::default();
        let a = tasks(&[("a", St::Downloading, 10.0), ("b", St::Queued, 0.0)]);
        let s = rings.sync(&a, T0);
        assert_eq!(s.next_stall, Some(T0 + STALL_AFTER));
        // Same report again: seen_at stays, the deadline arrives, the pass changes.
        let s = rings.sync(&a, T0 + STALL_AFTER);
        assert!(s.pass_changed);
        assert_eq!(rings.pass("a"), Some(Pass::Stalled(0.1)));
        assert_eq!(s.next_stall, None, "already stalled: no deadline");
        // A changed report: Flying again, a new deadline.
        let s = rings.sync(&tasks(&[("a", St::Downloading, 11.0), ("b", St::Queued, 0.0)]), T0 + STALL_AFTER + 1.0);
        assert_eq!(rings.pass("a"), Some(Pass::Flying(0.11)));
        assert_eq!(s.next_stall, Some(T0 + STALL_AFTER + 1.0 + STALL_AFTER));
    }

    #[test]
    fn finish_and_fail_are_reported_once_and_history_lands_at_once() {
        let mut rings = Rings::default();
        // History at launch: a Done task is Landed, nothing is announced.
        let s = rings.sync(&tasks(&[("old", St::Done, 100.0), ("bad", St::Failed, 20.0), ("a", St::Downloading, 50.0)]), T0);
        assert_eq!((s.finished.len(), s.failed.len()), (0, 0));
        assert_eq!(rings.pass("old"), Some(Pass::Landed));
        let go = tasks(&[("old", St::Done, 100.0), ("bad", St::Failed, 20.0), ("a", St::Done, 100.0), ("b", St::Downloading, 5.0)]);
        let s = rings.sync(&go, T0 + 1.0);
        assert_eq!(s.finished, vec!["a".to_string()]);
        assert_eq!(rings.pass("a"), Some(Pass::Done));
        assert_eq!(rings.sync(&go, T0 + 1.5).finished, Vec::<String>::new(), "reported once");
        // Land moves Done to Landed and stays there through the next sync.
        rings.land("a");
        assert_eq!(rings.pass("a"), Some(Pass::Landed));
        rings.sync(&go, T0 + 2.0);
        assert_eq!(rings.pass("a"), Some(Pass::Landed));
        // A failure after the first sight is announced.
        let s = rings.sync(&tasks(&[("b", St::Failed, 5.0)]), T0 + 3.0);
        assert_eq!(s.failed, vec!["b".to_string()]);
        assert_eq!(rings.pass("b"), Some(Pass::Failed(0.05)));
        assert_eq!(rings.pass("old"), None, "a removed task has no ring");
    }

    #[test]
    fn the_arc_never_passes_the_report() {
        let mut rings = Rings::default();
        let seq = [0.0, 12.0, 12.0, 40.0, 25.0, 25.0, 99.0, 3.0, 100.0];
        assert!(!seq.is_empty());
        for (i, pct) in seq.into_iter().enumerate() {
            rings.sync(&tasks(&[("a", St::Downloading, pct)]), T0 + i as f64);
            let r = &rings.rings["a"];
            assert!(r.span[1] <= pct as f32 / 100.0 + f32::EPSILON, "span {:?} after {pct}", r.span);
            assert!(r.stepped <= r.span[1] && r.head[1] <= r.span[1] && r.head[0] <= r.head[1]);
            if pct == 3.0 {
                assert_eq!(r.span[1], 0.03, "a lower report snaps");
            }
        }
    }

    #[test]
    fn the_overall_ring_follows_the_unfinished_tasks() {
        let mut rings = Rings::default();
        rings.sync(&tasks(&[("a", St::Downloading, 50.0), ("b", St::Queued, 0.0), ("c", St::Done, 100.0)]), T0);
        assert!(rings.overall_on);
        assert_eq!(rings.overall, [0.0, 0.25]);
        rings.sync(&tasks(&[("c", St::Done, 100.0)]), T0 + 1.0);
        assert!(!rings.overall_on);
    }

    #[test]
    fn a_retry_starts_its_countdown_once() {
        let mut rings = Rings::default();
        let mut t = tasks(&[("a", St::Retrying, 40.0)]);
        t[0].retry_count = Some(1.0);
        rings.sync(&t, T0);
        rings.sync(&t, T0 + 2.0);
        assert_eq!(rings.rings["a"].countdown, Some((T0, 15.0)), "entered once, the second backoff step");
        assert_eq!(rings.pass("a"), Some(Pass::Countdown(0.4)));
        rings.sync(&tasks(&[("a", St::Downloading, 40.0)]), T0 + 3.0);
        assert_eq!(rings.rings["a"].countdown, None);
    }

    #[test]
    fn reduced_motion_draws_the_waiting_arc_still_and_a_stalled_arc_at_six_tenths() {
        assert_eq!(drawn(Some(Pass::OnThePad), true), Some((Pass::OnThePad, 1.0)));
        assert_eq!(drawn(Some(Pass::Stalled(0.4)), true), Some((Pass::Stalled(0.4), 0.6)));
        assert_eq!(drawn(Some(Pass::Stalled(0.4)), false), Some((Pass::Stalled(0.4), 1.0)), "normally the loop breathes it");
        assert_eq!(drawn(Some(Pass::Flying(0.4)), true), Some((Pass::Flying(0.4), 1.0)));
        assert_eq!(drawn(Some(Pass::Landed), true), None);
        assert_eq!(drawn(None, false), None);
    }

    #[test]
    fn view_waiting_and_overall_build() {
        let m = Material { track: Color::BLACK, under: Color::BLACK, stroke: Color::WHITE, craft: Color::WHITE, segments: None };
        let mut rings = Rings::default();
        rings.sync(&tasks(&[("a", St::Downloading, 50.0)]), T0);
        for reduced in [false, true] {
            let _ = (view("r", 28.0, &rings, "a", &m, reduced, 0), view("r", 28.0, &rings, "gone", &m, reduced, 0));
            let _ = (waiting("w", 28.0, &m, reduced, 0), overall("o", 28.0, &rings, &m, reduced));
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
            _ => unreachable!("the old Orbit had five passes"),
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
