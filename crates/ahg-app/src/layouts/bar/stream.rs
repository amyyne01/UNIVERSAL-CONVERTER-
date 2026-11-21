//! The stream of downloads: what is in flight sits in one recessed well,
//! like the channels of a desk; what needs a hand and what is finished run
//! under it as plain lines. Home shows the well, the problems and the latest
//! three that landed; Downloads shows everything.
use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode as E};
use rune::{Align, Color, El, Icon, ScrollSpec, col, el, icon, row, spacer};

use super::cover;
use crate::actions;
use crate::copy;
use crate::directions::signal::model;
use crate::directions::signal::parts::{self, Line, ghost, icon_button, label, mono, row_act, sans, stack, task_ring};
use crate::directions::signal::theme::{Pal, source_color};
use crate::format::{self, Group};
use crate::gate::{Filter, Nav, State};
use crate::icons;
use crate::keys::{self, RowAct};

const COUNTER: f32 = 72.0;
/// The ring's slot, as wide as the meter it replaced so the readings stay in line.
const RING_SLOT: f32 = 168.0;
const STATE: f32 = 112.0;
const ACTIONS: f32 = 64.0;
const OWNED: f32 = 272.0;
/// How many landed downloads Home shows.
const RECENT: usize = 3;

pub fn view(s: &State, p: &Pal, home: bool) -> El {
    // Downloads shows what the filter and the search leave; Home shows everything.
    let tasks: Vec<&DownloadTask> = if home { s.tasks.iter().collect() } else { actions::visible(s).collect() };
    let mut out = col().w_full().pb(32.0).gap(24.0);
    if s.tasks.is_empty() {
        out = out.child(nothing_yet(p));
    } else if tasks.is_empty() {
        out = out.child(filtered_empty(s, p));
    }
    for g in [Group::InProgress, Group::Attention, Group::Completed] {
        let mut rows: Vec<&DownloadTask> = tasks.iter().copied().filter(|t| format::group_of(t) == g).collect();
        if rows.is_empty() {
            continue;
        }
        if g == Group::InProgress {
            // Live rows, then paused, then queued (3.4).
            rows.sort_by_key(|t| match t.progress.status {
                St::Paused => 1,
                St::Queued => 2,
                _ => 0,
            });
        }
        out = out.child(section(s, g, &rows, home, p));
    }
    col().w_full().scroll(ScrollSpec { stick_bottom: false }).child(out)
}

fn nothing_yet(p: &Pal) -> El {
    row()
        .w_full()
        .h(56.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .radius(8.0)
        .border(1.0, p.line)
        .child(icon(&icons::DOWNLOAD, 16.0).color(p.muted))
        .child(label(copy::EMPTY_NONE_TITLE, sans(13.0, 400.0), p.text2))
}

/// The filter and the search leave nothing: say which, and the way back to all.
fn filtered_empty(s: &State, p: &Pal) -> El {
    let (title, glyph) = parts::filtered_empty(s.filter);
    let total = s.tasks.len();
    row()
        .w_full()
        .h(64.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .radius(8.0)
        .border(1.0, p.line)
        .child(icon(glyph, 16.0).color(p.muted))
        .child(stack(4.0).grow(1.0).child(label(title, sans(13.0, 500.0), p.text)).child(label(
            copy::empty_filter_body(total),
            sans(13.0, 400.0),
            p.text2,
        )))
        // TODO(shared): `actions::show_all(s)`: the filter to All and the search emptied; Filter::All alone leaves a typed search on.
        .child(
            ghost(None, &copy::show_all(total), p.text2, p)
                .key(keys::SHOW_ALL)
                .on_press(|s: &mut State| actions::set_filter(s, Filter::All)),
        )
}

fn view_all(p: &Pal) -> El {
    ghost(Some(&icons::FORWARD), copy::CELL_RECENT_VIEW_ALL, p.text2, p).on_press(|s: &mut State| actions::go(s, Nav::Downloads))
}

fn section(s: &State, g: Group, rows: &[&DownloadTask], home: bool, p: &Pal) -> El {
    let tasks = &s.tasks;
    let name = match g {
        Group::InProgress => copy::GROUP_IN_PROGRESS,
        Group::Attention => copy::GROUP_NEEDS_ATTENTION,
        Group::Completed if home => copy::CELL_RECENT,
        Group::Completed => copy::GROUP_COMPLETED,
    };
    let mut title = Line::new().strong(name, p.text2);
    if g == Group::InProgress && format::all_paused(tasks) {
        title = title.add(copy::DL_PAUSED_SUFFIX, p.warning);
    }
    let mut head = row()
        .w_full()
        .h(32.0)
        .px(4.0)
        .gap(8.0)
        .center_y()
        .child(title.one(sans(13.0, 400.0)))
        .child(label(rows.len().to_string(), mono(12.0, 400.0), p.muted))
        .child(spacer());
    // Home's headers carry their section's own action; on Downloads the toolbar has them all.
    if home {
        match g {
            Group::InProgress if format::all_paused(tasks) => {
                head = head.child(
                    ghost(Some(&icons::PLAY), copy::DL_RESUME_ALL, p.accent_ink, p)
                        .key(keys::PAUSE_ALL)
                        .on_press(actions::pause_or_resume_all),
                )
            }
            Group::InProgress => {
                head = head.child(
                    ghost(Some(&icons::PAUSE), copy::DL_PAUSE_ALL, p.text2, p).key(keys::PAUSE_ALL).on_press(actions::pause_or_resume_all),
                )
            }
            Group::Attention if rows.iter().any(|t| t.progress.status == St::Failed && !format::is_permanent(t.error_code)) => {
                head = head.child(
                    ghost(Some(&icons::RETRY), copy::DL_RETRY_ALL_FAILED, p.text2, p)
                        .key(keys::RETRY_ALL_FAILED)
                        .on_press(actions::retry_all_failed),
                )
            }
            _ => {}
        }
        if g != Group::InProgress && rows.len() > RECENT || g == Group::Completed {
            head = head.child(view_all(p));
        }
    }
    let shown = if home && g != Group::InProgress { &rows[..rows.len().min(RECENT)] } else { rows };
    let body = match g {
        Group::InProgress => well(s, shown, p),
        _ => {
            let mut list = col().w_full();
            for (i, t) in shown.iter().enumerate() {
                let r = if g == Group::Attention { trouble_row(s, t, p) } else { owned_row(s, t, p) };
                list = list.child(if i > 0 { r.border_sides([1.0, 0.0, 0.0, 0.0], p.line) } else { r });
            }
            list
        }
    };
    col().w_full().gap(8.0).child(head).child(body)
}

/// Everything in flight, in one recessed well.
fn well(s: &State, rows: &[&DownloadTask], p: &Pal) -> El {
    let mut w = col().w_full().radius(8.0).fill(p.deep).border(1.0, p.line).clip_rounded(8.0);
    let interrupted = rows.iter().filter(|t| t.progress.status == St::Paused && t.interrupted == Some(true)).count();
    let mut first = true;
    if interrupted > 0 {
        w = w.child(banner(interrupted, p));
        first = false;
    }
    let queued = rows.iter().filter(|t| t.progress.status == St::Queued).count();
    let mut seam = false;
    for t in rows {
        if t.progress.status == St::Queued && !seam {
            seam = true;
            if !first {
                w = w.child(row().w_full().h(32.0).px(12.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], p.line).child(label(
                    copy::up_next(queued),
                    mono(12.0, 400.0),
                    p.muted,
                )));
                first = true;
            }
        }
        let r = live_row(s, t, p);
        w = w.child(if first { r } else { r.border_sides([1.0, 0.0, 0.0, 0.0], p.line) });
        first = false;
    }
    w
}

fn banner(n: usize, p: &Pal) -> El {
    row()
        .w_full()
        .h(48.0)
        .pl(12.0)
        .pr(8.0)
        .gap(12.0)
        .center_y()
        .fill(p.warning_soft)
        .child(icon(&icons::PAUSE, 16.0).color(p.warning))
        .child(Line::new().fig(&n.to_string(), p.text).add(&copy::interrupted_sentence(n), p.text).one(sans(13.0, 400.0)).grow(1.0))
        .child(
            ghost(Some(&icons::PLAY), copy::DL_INTERRUPTED_RESUME, p.bg, p)
                .fill(p.text)
                .hover(p.bg.alpha(0.12))
                .key(keys::RESUME_INTERRUPTED)
                .on_press(actions::resume_interrupted),
        )
        .child(
            ghost(Some(&icons::REMOVE), copy::DL_INTERRUPTED_DISCARD, p.text2, p)
                .key(keys::DISCARD_INTERRUPTED)
                .on_press(actions::discard_interrupted),
        )
}

/// A channel: what it is, its tape counter, its ring, its reading.
fn live_row(s: &State, t: &DownloadTask, p: &Pal) -> El {
    let pr = &t.progress;
    let st = pr.status;
    let pct = pr.percent.clamp(0.0, 100.0) / 100.0;
    // (how far the counter has run, its ink)
    let (frac, ink) = match st {
        St::Downloading => (pct, p.text),
        St::Converting | St::Embedding => (1.0, p.text2),
        St::Paused | St::Retrying => (pct, p.text2),
        _ => (0.0, p.muted),
    };
    let counter =
        label(if t.duration > 0.0 { model::timecode(t.duration * frac) } else { String::new() }, mono(13.0, 500.0), ink).w(COUNTER);
    row()
        .key(keys::row(&t.task_id))
        .on_press_tx(pick_with_mods(&t.task_id))
        .w_full()
        .h(56.0)
        .px(12.0)
        .gap(12.0)
        .center_y()
        .hover(p.hover)
        .fill(picked(s, t, p))
        .child(cover(s, t, 32.0, p))
        .child(title_block(t, true, p))
        .child(counter)
        .child(el().w(RING_SLOT).center_y().child(task_ring(s, t, p)))
        .child(reading(t, p))
        .child(live_actions(t, actions::can_promote(s, t), p))
}

/// A press on a row with Ctrl or Shift held picks it, or lets it go.
// TODO(shared): `actions::pick_row(s, id, mods)`; a Shift press should take the range from the last pick, here it picks the one row.
fn pick_with_mods(id: &str) -> impl Fn(&mut State, &mut rune::Tx) + 'static {
    let id = id.to_owned();
    move |s, tx| {
        if tx.mods().cmd || tx.mods().shift {
            actions::toggle_select(s, &id);
        }
    }
}

/// A selected row (Ctrl or Shift and a click) sits under a faint wash of the accent.
fn picked(s: &State, t: &DownloadTask, p: &Pal) -> Color {
    if s.selected.contains(&t.task_id) { p.accent.alpha(0.10) } else { Color::TRANSPARENT }
}

fn title_block(t: &DownloadTask, strong: bool, p: &Pal) -> El {
    let st = t.progress.status;
    let name = format::label(t);
    let title = if strong { Line::new().strong(&name, p.text) } else { Line::new().add(&name, p.text) };
    let mut detail = Line::new().add(format::source_name(t.source), source_color(p, t.source));
    if st == St::Retrying {
        let k = t.retry_count.unwrap_or(0.0).max(0.0) as usize;
        detail = detail.add(&copy::row_retrying(format::retry_wait(t), k), p.warning);
    } else {
        if !t.uploader.is_empty() {
            detail = detail.add(&copy::row_dot_part(&t.uploader), p.text2);
        }
        let pr = &t.progress;
        let bytes = if pr.total > 0.0 { pr.total } else { pr.downloaded };
        for part in [format::duration(t.duration), if bytes > 0.0 { format::bytes(bytes) } else { String::new() }] {
            if !part.is_empty() {
                detail = detail.add(&copy::row_dot_part(""), p.muted).fig(&part, p.muted);
            }
        }
    }
    stack(4.0).grow(1.0).child(title.one(sans(14.0, 400.0)).help(name.clone())).child(detail.one(sans(13.0, 400.0)))
}

fn state_word(i: &'static Icon, word: &str, tone: Color) -> El {
    row().gap(8.0).center_y().child(icon(i, 16.0).color(tone)).child(label(word, sans(13.0, 500.0), tone))
}

fn reading(t: &DownloadTask, p: &Pal) -> El {
    let pr = &t.progress;
    let c = col().w(STATE).items_end().gap(4.0);
    if pr.status == St::Downloading {
        let pct = format!("{}%", format::percent(t));
        return c
            .child(label(pct, mono(14.0, 500.0), p.text))
            .child(label(format::rate(pr.speed, pr.eta), mono(12.0, 400.0), p.muted).align(Align::End));
    }
    let (i, word, tone): (&'static Icon, &str, Color) = match pr.status {
        St::FetchingInfo => (&icons::LOADING, copy::STATE_STARTING, p.text2),
        St::Converting => (&icons::LOADING, copy::STATE_CONVERTING, p.warning),
        St::Embedding => (&icons::LOADING, copy::STATE_TAGGING, p.warning),
        St::Paused => (&icons::PAUSE, copy::STATE_PAUSED, p.warning),
        St::Retrying => (&icons::RETRY, copy::STATE_RETRYING, p.warning),
        _ => (&icons::CLOCK, copy::STATE_QUEUED, p.muted),
    };
    c.child(state_word(i, word, tone))
}

fn live_actions(t: &DownloadTask, promote: bool, p: &Pal) -> El {
    let st = t.progress.status;
    // As wide as the other sections' actions, so a waiting row has room for "Do this next".
    let mut a = row().w(ACTIONS + 32.0).gap(4.0).justify_end().center_y();
    let id = &t.task_id;
    if promote {
        a = a.child(row_act(icon_button(&icons::TO_FRONT, copy::ACT_DO_THIS_NEXT, p.text2, p), RowAct::Promote, id));
    }
    if matches!(st, St::Queued | St::FetchingInfo | St::Downloading) {
        a = a.child(row_act(icon_button(&icons::PAUSE, copy::ACT_PAUSE, p.text2, p), RowAct::Pause, id));
    }
    if st == St::Paused {
        a = a.child(row_act(icon_button(&icons::PLAY, copy::ACT_RESUME, p.accent_ink, p), RowAct::Resume, id));
    }
    a.child(row_act(icon_button(&icons::CANCEL, copy::ACT_CANCEL, p.text2, p), RowAct::Cancel, id))
}

/// A row that needs a hand: the reason in full, and what to do about it.
fn trouble_row(s: &State, t: &DownloadTask, p: &Pal) -> El {
    let st = t.progress.status;
    let failed = st == St::Failed;
    let name = format::label(t);
    let mut title = Line::new().add(&name, if failed { p.text } else { p.text2 });
    if failed && !t.title.trim().is_empty() {
        title = title.add(&copy::row_dot_part(&format::link_label(&t.url)), p.muted);
    }
    let mut detail = Line::new().add(format::source_name(t.source), source_color(p, t.source));
    if let Some(reason) = format::reason(t) {
        detail = detail.add(&copy::row_dot_part(&reason), p.text2);
    } else if !t.uploader.is_empty() {
        detail = detail.add(&copy::row_dot_part(&t.uploader), p.text2);
    }
    let text = stack(4.0)
        .grow(1.0)
        .child(title.one(sans(14.0, 400.0)).help(t.url.clone()))
        .child(detail.wrap(sans(13.0, 400.0).lh(20.0)).max_lines(2).min_w(0.0));
    let (i, word, tone): (&'static Icon, &str, Color) = match st {
        St::Failed if format::is_permanent(t.error_code) => (&icons::ALERT, copy::STATE_CANT_DOWNLOAD, p.error),
        St::Failed => (&icons::ALERT, copy::STATE_FAILED, p.error),
        _ => (&icons::CANCEL, copy::STATE_CANCELED, p.muted),
    };
    let id = &t.task_id;
    let mut a = row().w(ACTIONS + 32.0).gap(4.0).justify_end().center_y();
    if failed && !t.progress.error.is_empty() {
        a = a.child(row_act(icon_button(&icons::COPY, copy::ACT_COPY_DETAILS, p.text2, p), RowAct::CopyDetails, id));
    }
    if matches!(t.error_code, Some(E::Private | E::AgeRestricted | E::LoginRequired | E::DiskFull)) {
        // Goes to the Settings place, at the section it names.
        a = a.child(row_act(icon_button(&icons::SETTINGS, copy::ACT_OPEN_SETTINGS, p.text2, p), RowAct::Fix, id));
    }
    if t.error_code == Some(E::Unknown) && failed {
        a = a.child(row_act(icon_button(&icons::REFRESH, copy::ACT_UPDATE_ENGINE, p.text2, p), RowAct::Fix, id));
    }
    if !format::is_permanent(t.error_code) || !failed {
        a = a.child(row_act(icon_button(&icons::RETRY, copy::ACT_RETRY, if failed { p.error } else { p.text2 }, p), RowAct::Retry, id));
    }
    // A failed line carries one lit segment in the error colour on its edge, outside the grid.
    let edge = el().w(2.0).radius(1.0).fill(if failed { p.error } else { p.line_strong }).pin(Some(0.0), Some(12.0), None, Some(12.0));
    row()
        .key(keys::row(&t.task_id))
        .on_press_tx(pick_with_mods(&t.task_id))
        .w_full()
        .py(12.0)
        .px(12.0)
        .gap(12.0)
        .items_center()
        .hover_group()
        .hover(p.hover)
        .fill(picked(s, t, p))
        .child(edge)
        .child(cover(s, t, 32.0, p))
        .child(text)
        .child(col().w(STATE).items_end().child(state_word(i, word, tone)))
        .child(a)
}

/// A finished line: what it is, then the file you now own.
fn owned_row(s: &State, t: &DownloadTask, p: &Pal) -> El {
    let pr = &t.progress;
    let skipped = pr.already_on_disk == Some(true);
    let file = format::file_name(&pr.filename);
    let tag = el().h(20.0).px(4.0).center().radius(4.0).border(1.0, p.line_strong).child(label(
        t.format.to_uppercase(),
        mono(12.0, 400.0),
        p.text2,
    ));
    let owned = row()
        .w(OWNED)
        .gap(8.0)
        .center_y()
        .child(tag)
        .child(
            stack(4.0)
                .grow(1.0)
                .child(label(file, mono(12.0, 500.0), p.text).single_line().truncate_middle().min_w(0.0))
                .child(label(format::folder(&pr.filename), mono(12.0, 400.0), p.muted).single_line().truncate_middle().min_w(0.0)),
        )
        .help(pr.filename.clone());
    let word = if skipped { copy::STATE_SKIPPED } else { copy::STATE_DONE };
    let id = &t.task_id;
    // Remove, hidden until the row is hovered, leads: the last spot is where every section's
    // last action (Cancel, Retry) sits, and a row that slides under a resting pointer is not
    // hovered yet, so a repeated press there must not land on a hidden Remove (U0a: 50 rows became 48).
    let mut a = row().w(ACTIONS + 32.0).gap(4.0).justify_end().center_y().child(row_act(
        icon_button(&icons::REMOVE, copy::ACT_REMOVE, p.muted, p).group_opacity(0.0, 1.0),
        RowAct::Remove,
        id,
    ));
    if skipped {
        a = a.child(row_act(icon_button(&icons::DOWNLOAD, copy::ACT_DOWNLOAD_AGAIN, p.text2, p), RowAct::DownloadAgain, id));
    }
    a = a.child(row_act(icon_button(&icons::EXTERNAL, copy::ACT_OPEN_FILE, p.text2, p), RowAct::OpenFile, id)).child(row_act(
        icon_button(&icons::FOLDER, copy::ACT_SHOW_IN_FOLDER, p.text2, p),
        RowAct::ShowInFolder,
        id,
    ));
    row()
        .key(keys::row(&t.task_id))
        .on_press_tx(pick_with_mods(&t.task_id))
        .w_full()
        .h(56.0)
        .px(12.0)
        .gap(12.0)
        .center_y()
        .hover_group()
        .hover(p.hover)
        .fill(picked(s, t, p))
        .child(cover(s, t, 32.0, p))
        .child(title_block(t, false, p))
        .child(owned)
        .child(col().w(STATE).items_end().child(state_word(&icons::CHECK_SIGNAL, word, p.success)))
        .child(a)
}
