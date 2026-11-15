//! The rail beside the stage: what waits (Up next) and what needs a hand (Needs attention),
//! each problem with its own sentence and the buttons that fix it.
use ahg_engine::types::{DownloadStatus as St, DownloadTask};
use rune::{El, ScrollSpec, col, el, row, spacer};

use super::parts::{BODY_M, FIG, SMALL, SMALL_M, button, glyph, icon_button, text_button};
use super::{Fit, on_orbit};
use crate::copy;
use crate::directions::orbit::icons;
use crate::directions::orbit::look::{Pal, mix, txt};
use crate::directions::orbit::model;
use crate::gate::State;

pub fn view(s: &State, p: &Pal, fit: Fit) -> El {
    let queued: Vec<&DownloadTask> = s.tasks.iter().filter(|t| t.progress.status == St::Queued).collect();
    let trouble: Vec<&DownloadTask> = s.tasks.iter().filter(|t| matches!(t.progress.status, St::Failed | St::Cancelled)).collect();
    let interrupted = s.tasks.iter().filter(|t| t.progress.status == St::Paused && t.interrupted == Some(true)).count();

    let mut c = col().w_full().px(fit.pad).py(fit.pad).gap(if fit.tall { 32.0 } else { 24.0 });
    if interrupted > 0 {
        c = c.child(interrupted_note(p, interrupted));
    }
    c = c.child(up_next(s, p, &queued));
    if !trouble.is_empty() {
        c = c.child(attention(p, &trouble));
    }
    col().key("rail").w(fit.rail).h_full().min_h(0.0).border_sides([0.0, 0.0, 0.0, 1.0], p.line).scroll(ScrollSpec { stick_bottom: false }).child(c)
}

fn head(p: &Pal, name: &str, n: usize, aside: Option<El>) -> El {
    let mut r = row().w_full().h(24.0).gap(8.0).center_y().child(txt(name.to_string(), BODY_M, p.text2));
    if n > 0 {
        r = r.child(txt(n.to_string(), FIG, p.muted));
    }
    r = r.child(spacer());
    if let Some(a) = aside {
        r = r.child(a);
    }
    r
}

fn interrupted_note(p: &Pal, n: usize) -> El {
    let sentence = txt(format!("{n}{}", copy::interrupted_sentence(n)), SMALL, p.text).wrap();
    col()
        .w_full()
        .padding(12.0)
        .gap(12.0)
        .radius(8.0)
        .fill(mix(p.bg, p.warning, 0.06))
        .border(1.0, mix(p.bg, p.warning, 0.28))
        .child(row().w_full().gap(8.0).items_start().child(glyph(&icons::PAUSE, p.warning)).child(sentence.grow(1.0).min_w(0.0)))
        .child(row().gap(8.0).pl(24.0).child(button(p, "resume-interrupted".into(), copy::DL_INTERRUPTED_RESUME, Some(&icons::PLAY), false)).child(button(p, "discard-interrupted".into(), copy::DL_INTERRUPTED_DISCARD, None, false)))
}

fn up_next(s: &State, p: &Pal, queued: &[&DownloadTask]) -> El {
    // `Up next · 1` split into its words and its figure, as the other headers print.
    let words = copy::up_next(queued.len());
    let name = words.split(" · ").next().unwrap_or_default();
    let mut c = col().w_full().gap(12.0).child(head(p, name, queued.len(), None));
    if queued.is_empty() {
        return c.child(txt(copy::EMPTY_FILTER_ACTIVE, SMALL, p.muted));
    }
    for t in queued {
        let line = row().gap(8.0).min_w(0.0).child(txt(copy::STATE_QUEUED, SMALL_M, p.muted)).child(txt(model::specs(t), FIG, p.muted).truncate().shrink());
        c = c.child(
            row()
                .key(format!("next-{}", t.task_id))
                .w_full()
                .h(48.0)
                .gap(12.0)
                .center_y()
                .hover_group()
                .child(on_orbit(s, p, t, 48.0, 8.0))
                .child(col().grow(1.0).min_w(0.0).gap(4.0).child(txt(model::label(t), BODY_M, p.text).truncate()).child(line))
                .child(icon_button(p, format!("next-first-{}", t.task_id), copy::ACT_DO_THIS_NEXT, &icons::FORWARD, p.text2).group_opacity(0.0, 1.0))
                .child(icon_button(p, format!("next-cancel-{}", t.task_id), copy::ACT_CANCEL, &icons::CANCEL, p.text2).group_opacity(0.0, 1.0)),
        );
    }
    c
}

fn attention(p: &Pal, trouble: &[&DownloadTask]) -> El {
    let retryable = trouble.iter().any(|t| t.progress.status == St::Failed && !model::is_permanent(t.error_code));
    let aside = retryable.then(|| text_button(p, "retry-all".into(), copy::DL_RETRY_ALL_FAILED));
    let mut c = col().w_full().gap(16.0).child(head(p, copy::GROUP_NEEDS_ATTENTION, trouble.len(), aside));
    for t in trouble {
        c = c.child(problem(p, t));
    }
    c
}

/// A problem: the mark, the title and its buttons, then the sentence that says what to do.
fn problem(p: &Pal, t: &DownloadTask) -> El {
    let (word, tone) = super::parts::state_word(p, t);
    let canceled = t.progress.status == St::Cancelled;
    let mark = el().size(16.0, 32.0).center().child(glyph(if canceled { &icons::CANCEL } else { &icons::ALERT }, tone));
    let mut buttons = row().gap(0.0);
    for (label, i) in super::parts::actions(t) {
        let ink = if label == copy::ACT_RETRY { p.text } else { p.text2 };
        buttons = buttons.child(icon_button(p, format!("{label}-{}", t.task_id), label, i, ink));
    }
    let title = row().w_full().h(32.0).gap(8.0).center_y().child(txt(model::label(t), BODY_M, if canceled { p.text2 } else { p.text }).truncate().shrink().help(t.url.clone())).child(spacer()).child(buttons);
    let (src, src_color) = model::source(p, t.source);
    let mut words = col().grow(1.0).min_w(0.0).child(title).child(row().gap(8.0).child(txt(word, SMALL_M, tone)).child(txt(src, SMALL, src_color)));
    if let Some(reason) = model::reason(t) {
        words = words.child(txt(reason, SMALL, p.text2).wrap().max_lines(3).mt(4.0).help(t.progress.error.clone()));
    }
    row().key(format!("problem-{}", t.task_id)).w_full().gap(12.0).items_start().child(mark).child(words)
}
