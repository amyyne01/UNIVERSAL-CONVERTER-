//! Downloads, the second place (Ctrl+2): every download in one dense list, with the filters,
//! the search and the bulk actions. A row in flight keeps its cover on its orbit; a landed
//! one is a square tile; a problem carries its sentence on its second line.
use ahg_engine::types::{DownloadStatus as St, DownloadTask};
use rune::{Cursor, El, Icon, ScrollSpec, col, el, row, spacer};

use super::parts::{BODY, BODY_M, FIG, HERO, SMALL, SMALL_M, act, actions, button, cover, glyph, icon_button, state_word, tinted};
use super::{Fit, motion_line, on_orbit};
use crate::actions as acts;
use crate::copy;
use crate::directions::orbit::Tone;
use crate::directions::orbit::downloads::{Make, clear, filtered_words, search_text, select_all, selection, tick};
use crate::directions::orbit::look::{Pal, mix, txt};
use crate::directions::orbit::model;
use crate::directions::orbit::on_row;
use crate::format::{self, is_active, is_live};
use crate::gate::{Filter, State};
use crate::icons;
use crate::keys::{self, RowAct};

const ROW_H: f32 = 52.0;

pub fn view(s: &State, p: &Pal, fit: Fit) -> El {
    let tasks = &s.tasks;
    let make = |k: &str, l: &str, g: Option<&'static Icon>, t: Tone| match t {
        Tone::Primary => button(p, k.into(), l, g, true),
        Tone::Ghost => button(p, k.into(), l, g, false),
        Tone::Danger => tinted(p, k.into(), l, g, Some((p.error, p.bg))),
    };
    let mut page = col().w_full().px(fit.pad + 8.0).pt(fit.pad).pb(fit.pad + 8.0).gap(16.0).child(header(s, p));
    if tasks.is_empty() {
        page = page.child(
            row()
                .w_full()
                .h(ROW_H)
                .gap(12.0)
                .center_y()
                .border_sides([1.0, 0.0, 1.0, 0.0], p.line)
                .child(glyph(&icons::DOWNLOAD, p.muted))
                .child(txt(copy::EMPTY_NONE_TITLE, BODY_M, p.text))
                .child(txt(copy::EMPTY_NONE_BODY, SMALL, p.text2)),
        );
    } else {
        page = page.child(strip(s, p, &make));
        let interrupted = tasks.iter().filter(|t| t.progress.status == St::Paused && t.interrupted == Some(true)).count();
        if interrupted > 0 {
            page = page.child(banner(p, interrupted));
        }
        let shown: Vec<&DownloadTask> = acts::visible(s).collect();
        page = page.child(if shown.is_empty() { filtered(s, p, &make) } else { list(s, p, &shown) });
    }
    col().key("downloads").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(page)
}

/// `7 downloads: 1 downloading, 6 failed`, in the fixed order.
#[allow(dead_code)] // unused until cleanup (step 19)
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

fn header(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let active = format::active_count(tasks);
    let all_paused = format::all_paused(tasks);
    let mut left = col().gap(4.0).child(txt(copy::DL_TITLE, HERO, p.text)).child(txt(
        copy::active_and_total(active, tasks.len(), all_paused),
        FIG,
        p.muted,
    ));
    if tasks.len() >= 2 {
        left = left.child(txt(copy::downloads_summary(tasks.len(), &format::summary_parts(tasks)), SMALL, p.text2));
    }
    let search = row()
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
        .child(search_text(s, p, BODY));
    row().w_full().gap(24.0).items_end().child(left).child(spacer()).child(search)
}

/// The filter and search show nothing: the sentence, the total, and `Show all`.
fn filtered(s: &State, p: &Pal, make: Make) -> El {
    let (i, title, body, all) = filtered_words(s);
    row()
        .w_full()
        .h(ROW_H)
        .gap(12.0)
        .center_y()
        .border_sides([1.0, 0.0, 1.0, 0.0], p.line)
        .child(glyph(i, p.muted))
        .child(txt(title, BODY_M, p.text))
        .child(txt(body, SMALL, p.text2))
        .child(spacer())
        // TODO(shared): `actions::show_all(s)`, which also empties the search box.
        .child(make(keys::SHOW_ALL, &all, None, Tone::Ghost).on_press(|s: &mut State| acts::set_filter(s, Filter::All)))
}

fn strip(s: &State, p: &Pal, make: Make) -> El {
    let tasks = &s.tasks;
    let counts = format::filter_counts(tasks);
    let filters = [
        (copy::DL_FILTER_ALL, counts[0]),
        (copy::DL_FILTER_ACTIVE, counts[1]),
        (copy::DL_FILTER_DONE, counts[2]),
        (copy::DL_FILTER_FAILED, counts[3]),
    ];
    let mut seg = row().h(32.0).padding(4.0).gap(4.0).radius(8.0).border(1.0, p.line).role("radiogroup").label(copy::DL_FILTER_ARIA);
    for (i, (label, n)) in filters.iter().enumerate() {
        let on = Filter::ALL[i] == s.filter;
        let cell = row()
            .key(keys::filter(Filter::ALL[i]))
            .h_full()
            .px(8.0)
            .gap(8.0)
            .center_y()
            .radius(4.0)
            .role("radio")
            .checked(on)
            .focusable(true)
            .cursor(Cursor::Pointer)
            .on_press(move |s: &mut State| acts::set_filter(s, Filter::ALL[i]))
            .child(txt(*label, SMALL_M, if on { p.text } else { p.text2 }))
            .child(txt(n.to_string(), FIG, p.muted));
        seg = seg.child(if on { cell.fill(p.raised) } else { cell.hover(p.hover) });
    }

    let paused = tasks.iter().any(|t| t.progress.status == St::Paused);
    let all_paused = format::all_paused(tasks);
    let mut bulk = row().gap(8.0).center_y();
    if tasks.iter().any(|t| t.progress.status == St::Failed && !format::is_permanent(t.error_code)) {
        bulk = bulk.child(
            button(p, keys::RETRY_ALL_FAILED.into(), copy::DL_RETRY_ALL_FAILED, Some(&icons::RETRY), false)
                .on_press(acts::retry_all_failed),
        );
    }
    if tasks.iter().any(|t| t.progress.status == St::Done) {
        bulk = bulk.child(clear(s, p, make));
    }
    if all_paused {
        bulk = bulk
            .child(button(p, keys::PAUSE_ALL.into(), copy::DL_RESUME_ALL, Some(&icons::PLAY), false).on_press(acts::pause_or_resume_all));
    } else if paused || tasks.iter().any(|t| matches!(t.progress.status, St::Queued | St::FetchingInfo | St::Downloading)) {
        bulk = bulk
            .child(button(p, keys::PAUSE_ALL.into(), copy::DL_PAUSE_ALL, Some(&icons::PAUSE), false).on_press(acts::pause_or_resume_all));
    }
    if tasks.iter().any(is_active) {
        bulk = bulk.child(button(p, keys::CANCEL_ALL.into(), copy::DL_CANCEL_ALL, Some(&icons::CANCEL), false).on_press(acts::cancel_all));
    }
    // While anything is ticked the selection's actions take the bulk actions' place.
    let right = if s.selected.is_empty() { bulk } else { selection(s, p, make) };
    row().w_full().h(32.0).gap(16.0).pl(16.0).center_y().child(select_all(s, p)).child(seg).child(spacer()).child(right)
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
        .child(
            button(p, keys::RESUME_INTERRUPTED.into(), copy::DL_INTERRUPTED_RESUME, Some(&icons::PLAY), false)
                .on_press(acts::resume_interrupted),
        )
        .child(button(p, keys::DISCARD_INTERRUPTED.into(), copy::DL_INTERRUPTED_DISCARD, None, false).on_press(acts::discard_interrupted))
}

/// The rows on show, after the filter and the search.
fn list(s: &State, p: &Pal, tasks: &[&DownloadTask]) -> El {
    let pick = |f: fn(&DownloadTask) -> bool| -> Vec<&DownloadTask> { tasks.iter().copied().filter(|t| f(t)).collect() };
    let mut moving = pick(|t| is_live(t) || t.progress.status == St::Paused);
    moving.extend(pick(|t| t.progress.status == St::Queued));
    let groups = [
        (copy::GROUP_IN_PROGRESS, moving),
        (copy::GROUP_NEEDS_ATTENTION, pick(|t| matches!(t.progress.status, St::Failed | St::Cancelled))),
        (copy::GROUP_COMPLETED, pick(|t| t.progress.status == St::Done)),
    ];
    let mut out = col().w_full();
    for (name, items) in groups.iter().filter(|(_, items)| !items.is_empty()) {
        out = out.child(
            row()
                .w_full()
                .h(32.0)
                .gap(8.0)
                .center_y()
                .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
                .child(txt(*name, SMALL_M, p.text2))
                .child(txt(items.len().to_string(), FIG, p.muted)),
        );
        for t in items {
            out = out.child(task_row(s, p, t));
        }
    }
    out
}

fn task_row(s: &State, p: &Pal, t: &DownloadTask) -> El {
    let st = t.progress.status;
    let ticked = s.selected.contains(&t.task_id);
    let landed = matches!(st, St::Done | St::Failed | St::Cancelled);
    let art = if landed {
        el().size(40.0, 40.0).center().child(cover(s, p, t, 32.0, false).opacity(if st == St::Done { 1.0 } else { 0.5 }))
    } else {
        on_orbit(s, p, t, 40.0, 6.0)
    };
    let (src, color) = model::source(p, t.source);
    let mut line2 = row().w_full().min_w(0.0).gap(8.0).center_y().child(txt(src, SMALL_M, color));
    if let Some(reason) = format::reason(t) {
        line2 = line2.child(txt(reason, SMALL, p.text2).truncate().shrink().help(t.progress.error.clone()));
    } else if st == St::Done {
        line2 = line2
            .child(
                txt(format::file_name(&t.progress.filename).to_string(), SMALL, p.text2)
                    .truncate()
                    .shrink()
                    .help(t.progress.filename.clone()),
            )
            .child(txt(format::specs(t), FIG, p.muted).single_line());
    } else {
        if !t.uploader.is_empty() {
            line2 = line2.child(txt(t.uploader.clone(), SMALL, p.text2).truncate().shrink());
        }
        line2 = line2.child(txt(format::specs(t), FIG, p.muted).single_line());
    }
    let state = if landed {
        let (word, tone) = state_word(p, t);
        row().w(160.0).justify_end().child(txt(word, SMALL_M, tone))
    } else {
        row().w(160.0).justify_end().child(motion_line(p, t))
    };
    let mut acts = row().w(136.0).justify_end().gap(0.0);
    for (label, i) in actions(t) {
        let ink = if label == copy::ACT_RETRY || label == copy::ACT_RESUME { p.text } else { p.text2 };
        acts = acts.child(on_row(icon_button(p, keys::row_action(act(label), &t.task_id), label, i, ink), act(label), &t.task_id));
    }
    if model::is_finished(t) {
        let remove = icon_button(p, keys::row_action(RowAct::Remove, &t.task_id), copy::ACT_REMOVE, &icons::REMOVE, p.muted);
        acts = acts.child(on_row(remove.group_opacity(0.0, 1.0), RowAct::Remove, &t.task_id));
    }
    let mut r = row()
        .key(keys::row(&t.task_id))
        .w_full()
        .h(ROW_H)
        .pl(16.0)
        .gap(16.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
        .hover_group();
    // A ticked row is tinted in ink: the accent stays on the rings.
    if ticked {
        r = r.fill(mix(p.bg, p.text, 0.06));
    }
    let id = t.task_id.clone();
    r.child(
        tick(p, keys::row_select(&t.task_id), if ticked { copy::ROW_DESELECT_ARIA } else { copy::ROW_SELECT_ARIA }, ticked, false)
            .on_press(move |s: &mut State| acts::toggle_select(s, &id)),
    )
    .child(art)
    .child(
        col()
            .grow(1.0)
            .min_w(0.0)
            .gap(4.0)
            .child(
                txt(format::label(t), if landed { BODY } else { BODY_M }, if st == St::Done || !landed { p.text } else { p.text2 })
                    .truncate(),
            )
            .child(line2),
    )
    .child(state)
    .child(acts)
}
