//! Downloads, the second place (Ctrl+2): every download in one dense list, with the filters,
//! the search and the bulk actions. A row in flight keeps its cover on its orbit; a landed
//! one is a square tile; a problem carries its sentence on its second line.
use ahg_engine::types::{DownloadStatus as St, DownloadTask};
use rune::{Cursor, El, ScrollSpec, col, el, row, spacer};

use super::parts::{BODY, BODY_M, FIG, HERO, SMALL, SMALL_M, actions, button, cover, glyph, icon_button, state_word};
use super::{Fit, motion_line, on_orbit};
use crate::copy;
use crate::directions::orbit::icons;
use crate::directions::orbit::look::{Pal, mix, txt};
use crate::directions::orbit::model::{self, is_active, is_live};
use crate::gate::State;

const ROW_H: f32 = 52.0;

pub fn view(s: &State, p: &Pal, fit: Fit) -> El {
    let tasks = &s.tasks;
    let mut page = col().w_full().px(fit.pad + 8.0).pt(fit.pad).pb(fit.pad + 8.0).gap(16.0).child(header(tasks, p));
    if tasks.is_empty() {
        page = page.child(row().w_full().h(ROW_H).gap(12.0).center_y().border_sides([1.0, 0.0, 1.0, 0.0], p.line).child(glyph(&icons::DOWNLOAD, p.muted)).child(txt(copy::EMPTY_NONE_TITLE, BODY_M, p.text)).child(txt(copy::EMPTY_NONE_BODY, SMALL, p.text2)));
    } else {
        page = page.child(strip(tasks, p));
        let interrupted = tasks.iter().filter(|t| t.progress.status == St::Paused && t.interrupted == Some(true)).count();
        if interrupted > 0 {
            page = page.child(banner(p, interrupted));
        }
        page = page.child(list(s, p));
    }
    col().key("downloads").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(page)
}

/// `7 downloads: 1 downloading, 6 failed`, in the fixed order.
fn summary(tasks: &[DownloadTask]) -> String {
    let mut n = [0usize; 7];
    for t in tasks {
        n[match t.progress.status {
            St::FetchingInfo | St::Downloading | St::Converting | St::Embedding => 0,
            St::Retrying => 1,
            St::Queued => 2,
            St::Paused => 3,
            St::Done => 4,
            St::Failed => 5,
            St::Cancelled => 6,
        }] += 1;
    }
    let parts: Vec<String> = n.iter().zip(copy::SUMMARY_WORDS).filter(|(c, _)| **c > 0).map(|(c, w)| format!("{c} {w}")).collect();
    copy::downloads_summary(tasks.len(), &parts)
}

fn header(tasks: &[DownloadTask], p: &Pal) -> El {
    let active = tasks.iter().filter(|t| is_active(t)).count();
    let all_paused = tasks.iter().any(|t| t.progress.status == St::Paused) && !tasks.iter().any(is_live);
    let mut left = col().gap(4.0).child(txt(copy::DL_TITLE, HERO, p.text)).child(txt(copy::active_and_total(active, tasks.len(), all_paused), FIG, p.muted));
    if tasks.len() >= 2 {
        left = left.child(txt(summary(tasks), SMALL, p.text2));
    }
    let search = row()
        .key("dl-search")
        .w(280.0)
        .h(36.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.card)
        .border(1.0, p.track)
        .label(copy::DL_SEARCH_ARIA)
        .child(glyph(&icons::SEARCH, p.muted))
        .child(txt(copy::DL_SEARCH_PLACEHOLDER, BODY, p.muted).truncate());
    row().w_full().gap(24.0).items_end().child(left).child(spacer()).child(search)
}

fn checkbox(p: &Pal, label: &'static str) -> El {
    el().size(16.0, 16.0).radius(4.0).border(1.0, p.track).role("checkbox").label(label).focusable(true).cursor(Cursor::Pointer).hover(p.raised)
}

fn strip(tasks: &[DownloadTask], p: &Pal) -> El {
    let count = |f: fn(&DownloadTask) -> bool| tasks.iter().filter(|t| f(t)).count();
    let filters = [
        (copy::DL_FILTER_ALL, tasks.len()),
        (copy::DL_FILTER_ACTIVE, count(is_active)),
        (copy::DL_FILTER_DONE, count(|t| t.progress.status == St::Done)),
        (copy::DL_FILTER_FAILED, count(|t| matches!(t.progress.status, St::Failed | St::Cancelled))),
    ];
    let mut seg = row().h(32.0).padding(4.0).gap(4.0).radius(8.0).border(1.0, p.line).role("radiogroup").label(copy::DL_FILTER_ARIA);
    for (i, (label, n)) in filters.iter().enumerate() {
        let on = i == 0;
        let cell = row().key(format!("filter-{i}")).h_full().px(8.0).gap(8.0).center_y().radius(4.0).role("radio").checked(on).focusable(true).cursor(Cursor::Pointer).child(txt(*label, SMALL_M, if on { p.text } else { p.text2 })).child(txt(n.to_string(), FIG, p.muted));
        seg = seg.child(if on { cell.fill(p.raised) } else { cell.hover(p.hover) });
    }

    let paused = tasks.iter().any(|t| t.progress.status == St::Paused);
    let all_paused = paused && !tasks.iter().any(is_live);
    let mut bulk = row().gap(8.0).center_y();
    if tasks.iter().any(|t| t.progress.status == St::Failed && !model::is_permanent(t.error_code)) {
        bulk = bulk.child(button(p, "dl-retry-all".into(), copy::DL_RETRY_ALL_FAILED, Some(&icons::RETRY), false));
    }
    if tasks.iter().any(|t| t.progress.status == St::Done) {
        bulk = bulk.child(button(p, "dl-clear".into(), copy::DL_CLEAR_COMPLETED, Some(&icons::REMOVE), false));
    }
    if all_paused {
        bulk = bulk.child(button(p, "dl-resume-all".into(), copy::DL_RESUME_ALL, Some(&icons::PLAY), false));
    } else if paused || tasks.iter().any(|t| matches!(t.progress.status, St::Queued | St::FetchingInfo | St::Downloading)) {
        bulk = bulk.child(button(p, "dl-pause-all".into(), copy::DL_PAUSE_ALL, Some(&icons::PAUSE), false));
    }
    if tasks.iter().any(is_active) {
        bulk = bulk.child(button(p, "dl-cancel-all".into(), copy::DL_CANCEL_ALL, Some(&icons::CANCEL), false));
    }
    row().w_full().gap(16.0).pl(16.0).center_y().child(checkbox(p, copy::DL_SELECT_ALL_ARIA)).child(seg).child(spacer()).child(bulk)
}

fn banner(p: &Pal, n: usize) -> El {
    row()
        .w_full()
        .h(48.0)
        .pl(16.0)
        .pr(8.0)
        .gap(12.0)
        .center_y()
        .radius(8.0)
        .fill(mix(p.bg, p.warning, 0.06))
        .border(1.0, mix(p.bg, p.warning, 0.28))
        .child(glyph(&icons::PAUSE, p.warning))
        .child(row().center_y().child(txt(n.to_string(), FIG, p.text)).child(txt(copy::interrupted_sentence(n), BODY, p.text)))
        .child(spacer())
        .child(button(p, "dl-resume-interrupted".into(), copy::DL_INTERRUPTED_RESUME, Some(&icons::PLAY), false))
        .child(button(p, "dl-discard-interrupted".into(), copy::DL_INTERRUPTED_DISCARD, None, false))
}

fn list(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let pick = |f: fn(&DownloadTask) -> bool| -> Vec<&DownloadTask> { tasks.iter().filter(|t| f(t)).collect() };
    let mut moving = pick(|t| is_live(t) || t.progress.status == St::Paused);
    moving.extend(pick(|t| t.progress.status == St::Queued));
    let groups = [
        (copy::GROUP_IN_PROGRESS, moving),
        (copy::GROUP_NEEDS_ATTENTION, pick(|t| matches!(t.progress.status, St::Failed | St::Cancelled))),
        (copy::GROUP_COMPLETED, pick(|t| t.progress.status == St::Done)),
    ];
    let mut out = col().w_full();
    for (name, items) in groups.iter().filter(|(_, items)| !items.is_empty()) {
        out = out.child(row().w_full().h(32.0).gap(8.0).center_y().border_sides([0.0, 0.0, 1.0, 0.0], p.line).child(txt(*name, SMALL_M, p.text2)).child(txt(items.len().to_string(), FIG, p.muted)));
        for t in items {
            out = out.child(task_row(s, p, t));
        }
    }
    out
}

fn task_row(s: &State, p: &Pal, t: &DownloadTask) -> El {
    let st = t.progress.status;
    let landed = matches!(st, St::Done | St::Failed | St::Cancelled);
    let art = if landed {
        el().size(40.0, 40.0).center().child(cover(s, p, t, 32.0, false).opacity(if st == St::Done { 1.0 } else { 0.5 }))
    } else {
        on_orbit(s, p, t, 40.0, 6.0)
    };
    let (src, color) = model::source(p, t.source);
    let mut line2 = row().w_full().min_w(0.0).gap(8.0).center_y().child(txt(src, SMALL_M, color));
    if let Some(reason) = model::reason(t) {
        line2 = line2.child(txt(reason, SMALL, p.text2).truncate().shrink().help(t.progress.error.clone()));
    } else if st == St::Done {
        line2 = line2.child(txt(model::file_name(&t.progress.filename).to_string(), SMALL, p.text2).truncate().shrink().help(t.progress.filename.clone())).child(txt(model::specs(t), FIG, p.muted).single_line());
    } else {
        if !t.uploader.is_empty() {
            line2 = line2.child(txt(t.uploader.clone(), SMALL, p.text2).truncate().shrink());
        }
        line2 = line2.child(txt(model::specs(t), FIG, p.muted).single_line());
    }
    let state = if landed {
        let (word, tone) = state_word(p, t);
        row().w(160.0).justify_end().child(txt(word, SMALL_M, tone))
    } else {
        row().w(160.0).justify_end().child(motion_line(p, t))
    };
    let mut acts = row().w(136.0).justify_end().gap(0.0);
    for (label, i) in actions(t) {
        acts = acts.child(icon_button(p, format!("dl-{label}-{}", t.task_id), label, i, if label == copy::ACT_RETRY || label == copy::ACT_RESUME { p.text } else { p.text2 }));
    }
    if model::is_finished(t) {
        acts = acts.child(icon_button(p, format!("dl-remove-{}", t.task_id), copy::ACT_REMOVE, &icons::REMOVE, p.muted).group_opacity(0.0, 1.0));
    }
    row()
        .key(format!("dl-row-{}", t.task_id))
        .w_full()
        .h(ROW_H)
        .pl(16.0)
        .gap(16.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
        .hover_group()
        .child(checkbox(p, copy::ROW_SELECT_ARIA))
        .child(art)
        .child(col().grow(1.0).min_w(0.0).gap(4.0).child(txt(model::label(t), if landed { BODY } else { BODY_M }, if st == St::Done || !landed { p.text } else { p.text2 }).truncate()).child(line2))
        .child(state)
        .child(acts)
}
