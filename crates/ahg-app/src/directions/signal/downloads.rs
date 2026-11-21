//! Downloads: the desk's channel list. Every row is 64 points in every state.
//! An in-flight row carries the instrument (the segmented ring beside the tape
//! counter); a finished row shows the file you now own in the same slot.
use ahg_engine::types::{DownloadStatus as St, DownloadTask};
use rune::{Align, Color, El, ScrollSpec, col, el, icon, row, spacer, text_input};

use super::model;
use super::parts::{
    self, Line, art, button, checkbox, confirm, ghost, icon_button, idle_ring, label, mono, row_act, sans, stack, task_ring,
};
use super::theme::{Pal, source_color};
use crate::actions;
use crate::copy;
use crate::format::{self, Group};
use crate::gate::{Confirm, Filter, Nav, State};
use crate::icons;
use crate::keys::{self, RowAct};

const INSTRUMENT: f32 = 220.0;
/// A finished row's file takes more room than the instrument: names are long.
const OWNED: f32 = 300.0;
const STATE_COL: f32 = 128.0;
const ACTIONS: f32 = 124.0;

pub fn view(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let mut page = col().w_full().max_w(1600.0).px(32.0).pt(28.0).pb(32.0).gap(16.0).child(header(s, p));
    let interrupted = tasks.iter().filter(|t| t.progress.status == St::Paused && t.interrupted == Some(true)).count();
    if interrupted > 0 {
        page = page.child(banner(interrupted, p));
    }
    if !tasks.is_empty() {
        page = page.child(strip(s, p));
    }
    page = page.child(list(s, p));
    col().w_full().h_full().scroll(ScrollSpec { stick_bottom: false }).min_h(0.0).items_center().child(page)
}

fn header(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let active = format::active_count(tasks);
    let all_paused = format::all_paused(tasks);
    let mut left = stack(8.0).child(label(copy::DL_TITLE, sans(28.0, 500.0), p.text)).child(label(
        copy::active_and_total(active, tasks.len(), all_paused),
        mono(12.0, 400.0),
        p.muted,
    ));
    if tasks.len() >= 2 {
        left = left.child(label(copy::downloads_summary(tasks.len(), &format::summary_parts(tasks)), sans(13.0, 400.0), p.text2));
    }
    let mut right = row().gap(4.0).center_y();
    if !tasks.is_empty() {
        right = right.child(
            row()
                .w(240.0)
                .h(32.0)
                .px(12.0)
                .gap(8.0)
                .center_y()
                .radius(8.0)
                .fill(p.surface)
                .border(1.0, p.line_strong)
                .mr(8.0)
                .child(icon(&icons::SEARCH, 16.0).color(p.muted))
                .child(
                    text_input(&s.query, sans(13.0, 400.0))
                        .key(keys::DOWNLOADS_SEARCH)
                        .single_line()
                        .color(p.text)
                        .placeholder(copy::DL_SEARCH_PLACEHOLDER, p.muted)
                        .label(copy::DL_SEARCH_ARIA)
                        .on_edit(|s: &mut State| &mut s.query)
                        .grow(1.0)
                        .min_w(0.0),
                ),
        );
    }
    if tasks.iter().any(|t| t.progress.status == St::Failed && !format::is_permanent(t.error_code)) {
        right = right.child(
            ghost(Some(&icons::RETRY), copy::DL_RETRY_ALL_FAILED, p.text2, p)
                .key(keys::RETRY_ALL_FAILED)
                .on_press(actions::retry_all_failed),
        );
    }
    let done = tasks.iter().filter(|t| t.progress.status == St::Done).count();
    if s.confirm == Some(Confirm::Clear) && done > 0 {
        right = right.child(confirm(&copy::clear_confirm(done), copy::DL_CLEAR_CONFIRM_ARIA, p));
    } else if done > 0 {
        right = right.child(
            ghost(Some(&icons::REMOVE), copy::DL_CLEAR_COMPLETED, p.text2, p)
                .key(keys::arm(Confirm::Clear))
                .on_press(|s: &mut State| actions::arm(s, Confirm::Clear)),
        );
    }
    let pausable = tasks.iter().any(|t| matches!(t.progress.status, St::Queued | St::FetchingInfo | St::Downloading));
    if pausable || tasks.iter().any(|t| t.progress.status == St::Paused) {
        right = right.child(
            if all_paused {
                button(Some(&icons::PLAY), copy::DL_RESUME_ALL, p.accent_ink, p)
            } else {
                button(Some(&icons::PAUSE), copy::DL_PAUSE_ALL, p.text, p)
            }
            .key(keys::PAUSE_ALL)
            .on_press(actions::pause_or_resume_all),
        );
    }
    if active > 0 {
        right =
            right.child(ghost(Some(&icons::CANCEL), copy::DL_CANCEL_ALL, p.text2, p).key(keys::CANCEL_ALL).on_press(actions::cancel_all));
    }
    row().w_full().items_end().justify_between().gap(16.0).flex_wrap().child(left).child(right)
}

fn banner(n: usize, p: &Pal) -> El {
    row()
        .w_full()
        .h(48.0)
        .pl(16.0)
        .pr(8.0)
        .gap(12.0)
        .center_y()
        .radius(10.0)
        .fill(p.warning_soft)
        .border(1.0, p.warning.alpha(0.22))
        .child(icon(&icons::PAUSE, 16.0).color(p.warning))
        .child(Line::new().fig(&n.to_string(), p.text).add(&copy::interrupted_sentence(n), p.text).one(sans(13.0, 400.0)).grow(1.0))
        .child(
            primary(Some(&icons::PLAY), copy::DL_INTERRUPTED_RESUME, p).key(keys::RESUME_INTERRUPTED).on_press(actions::resume_interrupted),
        )
        .child(
            ghost(Some(&icons::REMOVE), copy::DL_INTERRUPTED_DISCARD, p.text2, p)
                .key(keys::DISCARD_INTERRUPTED)
                .on_press(actions::discard_interrupted),
        )
}

/// The one filled button: off-white ink on dark, ink on light.
fn primary(i: Option<&'static rune::Icon>, words: &str, p: &Pal) -> El {
    ghost(i, words, p.bg, p).fill(p.text).hover(p.bg.alpha(0.12))
}

/// A tick: empty, ticked, or `mixed` (some of the rows on show).
fn tick(on: bool, mixed: bool, p: &Pal) -> El {
    let b = checkbox(p).center().checked(on);
    if on {
        b.fill(p.text).border(1.0, p.text).child(icon(&icons::CHECK_SIGNAL, 12.0).color(p.surface))
    } else if mixed {
        b.child(el().size(8.0, 2.0).radius(1.0).fill(p.text))
    } else {
        b
    }
}

/// The control strip: the select-all tick and the filters; with a selection, its actions at the right.
fn strip(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let counts = format::filter_counts(tasks);
    let filters = [
        (copy::DL_FILTER_ALL, counts[0]),
        (copy::DL_FILTER_ACTIVE, counts[1]),
        (copy::DL_FILTER_DONE, counts[2]),
        (copy::DL_FILTER_FAILED, counts[3]),
    ];
    let mut seg = row().padding(4.0).gap(4.0).radius(8.0).fill(p.deep).border(1.0, p.line).role("radiogroup").label(copy::DL_FILTER_ARIA);
    for (i, (name, n)) in filters.into_iter().enumerate() {
        let on = Filter::ALL[i] == s.filter;
        seg = seg.child(
            row()
                .key(keys::filter(Filter::ALL[i]))
                .on_press(move |s: &mut State| actions::set_filter(s, Filter::ALL[i]))
                .role("radio")
                .checked(on)
                .h(28.0)
                .px(12.0)
                .gap(8.0)
                .center_y()
                .radius(6.0)
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .hover(p.hover)
                .cursor(rune::Cursor::Pointer)
                .child(label(name, sans(13.0, if on { 500.0 } else { 400.0 }), if on { p.text } else { p.text2 }))
                .child(label(n.to_string(), mono(12.0, 400.0), p.muted)),
        );
    }
    let shown: Vec<&DownloadTask> = actions::visible(s).collect();
    let ticked = shown.iter().filter(|t| s.selected.contains(&t.task_id)).count();
    let all = !shown.is_empty() && ticked == shown.len();
    let mut r = row()
        .w_full()
        .h(40.0)
        .pl(16.0)
        .gap(16.0)
        .center_y()
        .child(tick(all, ticked > 0 && !all, p).key(keys::SELECT_ALL).label(copy::DL_SELECT_ALL_ARIA).on_press(actions::toggle_select_all))
        .child(seg);
    if !s.selected.is_empty() {
        r = r.child(spacer()).child(selection(s, p));
    }
    r
}

/// What a selection can do: cancel the active ones, remove the finished ones (asking once more), or let go.
fn selection(s: &State, p: &Pal) -> El {
    let (active, finished) = parts::selection(s);
    let mut r = row().gap(4.0).center_y().child(label(copy::selected_count(s.selected.len()), mono(12.0, 500.0), p.text).mr(8.0));
    let cancel =
        ghost(Some(&icons::CANCEL), copy::DL_SELECTION_CANCEL, p.text2, p).key(keys::CANCEL_SELECTED).on_press(actions::cancel_selected);
    let armed = s.confirm == Some(Confirm::Remove) && finished > 0;
    if !armed {
        r = r.child(if active > 0 { cancel } else { cancel.opacity(0.4) });
    }
    if s.confirm == Some(Confirm::Remove) && finished > 0 {
        r = r.child(confirm(&copy::remove_confirm(finished), copy::DL_REMOVE_CONFIRM_ARIA, p));
    } else {
        let remove = ghost(Some(&icons::REMOVE), copy::DL_SELECTION_REMOVE, p.text2, p)
            .key(keys::arm(Confirm::Remove))
            .on_press(|s: &mut State| actions::arm(s, Confirm::Remove));
        r = r.child(if finished > 0 { remove } else { remove.opacity(0.4) });
    }
    // TODO(shared): `actions::clear_selection(s)`; the selection is let go here with the field itself.
    r.child(ghost(None, copy::DL_SELECTION_CLEAR, p.text2, p).key(keys::CLEAR_SELECTION).on_press(|s: &mut State| s.selected.clear()))
}

fn list(s: &State, p: &Pal) -> El {
    let surface = col().w_full().radius(12.0).fill(p.surface).border(1.0, p.line).clip_rounded(12.0);
    if s.tasks.is_empty() {
        return surface.child(empty_row(s, p));
    }
    // The filter and the search decide the rows; the group headers count what is left.
    let shown: Vec<DownloadTask> = actions::visible(s).cloned().collect();
    if shown.is_empty() {
        return surface.child(filtered_row(s, p));
    }
    let tasks = &shown;
    let groups = [Group::InProgress, Group::Attention, Group::Completed];
    let present: Vec<Group> = groups.into_iter().filter(|g| tasks.iter().any(|t| format::group_of(t) == *g)).collect();
    let mut out = surface;
    for g in &present {
        let mut rows: Vec<&DownloadTask> = tasks.iter().filter(|t| format::group_of(t) == *g).collect();
        if *g == Group::InProgress {
            // Live rows, then paused, then queued (3.4).
            rows.sort_by_key(|t| match t.progress.status {
                St::Paused => 1,
                St::Queued => 2,
                _ => 0,
            });
        }
        if present.len() > 1 {
            out = out.child(group_header(*g, &rows, &s.tasks, p));
        }
        let queued = rows.iter().filter(|t| t.progress.status == St::Queued).count();
        let mut seam_done = false;
        for (i, t) in rows.iter().enumerate() {
            if t.progress.status == St::Queued && !seam_done {
                seam_done = true;
                if i > 0 {
                    out = out.child(row().w_full().h(28.0).px(16.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], p.line).child(label(
                        copy::up_next(queued),
                        mono(11.0, 400.0),
                        p.muted,
                    )));
                }
            }
            out = out.child(task_row(s, t, i > 0, p));
        }
    }
    out
}

fn group_header(g: Group, rows: &[&DownloadTask], tasks: &[DownloadTask], p: &Pal) -> El {
    let name = match g {
        Group::InProgress => copy::GROUP_IN_PROGRESS,
        Group::Attention => copy::GROUP_NEEDS_ATTENTION,
        Group::Completed => copy::GROUP_COMPLETED,
    };
    let mut line = Line::new().strong(name, p.text2);
    if g == Group::InProgress && format::all_paused(tasks) {
        line = line.add(copy::DL_PAUSED_SUFFIX, p.warning);
    }
    let mut band = row()
        .w_full()
        .h(32.0)
        .px(16.0)
        .gap(8.0)
        .center_y()
        .fill(p.deep.alpha(0.5))
        .border_sides([if g == Group::InProgress { 0.0 } else { 1.0 }, 0.0, 1.0, 0.0], p.line)
        .child(line.one(sans(13.0, 400.0)))
        .child(label(rows.len().to_string(), mono(12.0, 400.0), p.muted))
        .child(spacer());
    let live = tasks.iter().filter(|t| format::is_live(t)).count();
    if g == Group::InProgress && live > 0 {
        let speed = format::speed(format::total_speed(tasks));
        let figure = if speed.is_empty() { live.to_string() } else { format!("{live}{}", copy::row_dot_part(&speed)) };
        band = band.child(
            row()
                .gap(8.0)
                .center_y()
                .label(copy::transferring_aria(live, &speed))
                .child(icon(&icons::DOWNLOAD, 16.0).color(p.accent_ink))
                .child(label(figure, mono(12.0, 500.0), p.text2)),
        );
    }
    band
}

fn empty_row(s: &State, p: &Pal) -> El {
    row()
        .w_full()
        .h(64.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .child(el().w(44.0))
        .child(el().size(40.0, 40.0).radius(8.0).fill(p.raised).center().child(icon(&icons::DOWNLOAD, 16.0).color(p.muted)))
        .child(stack(4.0).grow(1.0).child(label(copy::EMPTY_NONE_TITLE, sans(14.0, 500.0), p.text)).child(label(
            copy::EMPTY_NONE_BODY,
            sans(13.0, 400.0),
            p.text2,
        )))
        // The idle channel: the instrument's well, its ring dark until something comes in.
        .child(well(p).child(idle_ring(s, p)))
        .child(
            row()
                .w(STATE_COL + ACTIONS + 12.0)
                .justify_end()
                .child(button(None, copy::EMPTY_NONE_ACTION, p.text, p).on_press(|s: &mut State| actions::go(s, Nav::Home))),
        )
}

/// The filter and the search leave nothing: which, how many there are, and the way back to all.
fn filtered_row(s: &State, p: &Pal) -> El {
    let (title, glyph) = parts::filtered_empty(s.filter);
    let total = s.tasks.len();
    row()
        .w_full()
        .h(64.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .child(el().w(44.0))
        .child(el().size(40.0, 40.0).radius(8.0).fill(p.raised).center().child(icon(glyph, 16.0).color(p.muted)))
        .child(stack(4.0).grow(1.0).child(label(title, sans(14.0, 500.0), p.text)).child(label(
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

fn task_row(s: &State, t: &DownloadTask, rule_above: bool, p: &Pal) -> El {
    let st = t.progress.status;
    let failed = st == St::Failed;
    let on = s.selected.contains(&t.task_id);
    let mut r = row()
        .key(keys::row(&t.task_id))
        .w_full()
        .h(64.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .hover_group()
        .hover(p.hover)
        .child(el().w(16.0))
        .child(tick(on, false, p).key(keys::row_select(&t.task_id)).label(copy::ROW_SELECT_ARIA).on_press({
            let id = t.task_id.clone();
            move |s: &mut State| actions::toggle_select(s, &id)
        }))
        .child(art(s, t, Some(40.0), 40.0, 6.0, p))
        .child(title_block(t, p));
    if rule_above {
        r = r.border_sides([1.0, 0.0, 0.0, 0.0], p.line);
    }
    if on {
        r = r.fill(p.accent.alpha(0.10));
    } else if failed {
        r = r.fill(p.error_soft);
    }
    if let Some(slot) = instrument(s, t, p) {
        r = r.child(slot);
    }
    r.child(state_col(t, p)).child(actions(t, actions::can_promote(s, t), p))
}

fn title_block(t: &DownloadTask, p: &Pal) -> El {
    let st = t.progress.status;
    let quiet = matches!(st, St::Done | St::Cancelled);
    let name = format::label(t);
    let mut title = if quiet { Line::new().add(&name, p.text2) } else { Line::new().strong(&name, p.text) };
    let show_link = st == St::Failed;
    if show_link && !t.title.trim().is_empty() {
        title = title.add(&copy::row_dot_part(&format::link_label(&t.url)), p.muted);
    }
    let title = title.one(sans(14.0, 400.0)).help(if show_link { t.url.clone() } else { name.clone() });

    let mut detail = Line::new().add(format::source_name(t.source), source_color(p, t.source));
    let reason = format::reason(t);
    let skipped = st == St::Done && t.progress.already_on_disk == Some(true);
    let wraps = reason.is_some();
    if let Some(reason) = &reason {
        detail = detail.add(&copy::row_dot_part(reason), p.error);
    } else if skipped {
        detail = detail.add(copy::ROW_ALREADY_DOWNLOADED, p.text2);
    } else if st == St::Retrying {
        let k = t.retry_count.unwrap_or(0.0).max(0.0) as usize;
        detail = detail.add(&copy::row_retrying(format::retry_wait(t), k), p.warning);
    } else {
        if !t.uploader.is_empty() {
            detail = detail.add(&copy::row_dot_part(&t.uploader), p.text2);
        }
        for part in format::specs(t).split(" · ").filter(|part| !part.is_empty()) {
            detail = detail.add(&copy::row_dot_part(""), p.muted).fig(part, p.muted);
        }
    }
    let style = sans(13.0, 400.0).lh(16.0);
    let detail = if wraps { detail.wrap(style).max_lines(2).min_w(0.0) } else { detail.one(style) };
    let detail = if t.progress.error.is_empty() { detail } else { detail.help(t.progress.error.clone()) };
    stack(4.0).grow(1.0).child(title).child(detail)
}

/// The instrument slot: the ring and the tape counter while in flight, the
/// owned file once done, nothing for rows that need attention.
fn instrument(s: &State, t: &DownloadTask, p: &Pal) -> Option<El> {
    let pr = &t.progress;
    let st = pr.status;
    if st == St::Done {
        let file = format::file_name(&pr.filename);
        let format_tag = el().h(20.0).px(8.0).center().radius(4.0).border(1.0, p.line_strong).child(label(
            t.format.to_uppercase(),
            mono(11.0, 500.0),
            p.text2,
        ));
        return Some(
            row()
                .w(OWNED)
                .gap(12.0)
                .center_y()
                .child(format_tag)
                .child(
                    stack(4.0)
                        .grow(1.0)
                        .child(label(file, mono(12.0, 500.0), p.text).single_line().truncate_middle().min_w(0.0))
                        .child(label(format::folder(&pr.filename), mono(11.0, 400.0), p.muted).single_line().truncate_middle().min_w(0.0)),
                )
                .help(pr.filename.clone()),
        );
    }
    if matches!(st, St::Failed | St::Cancelled) {
        return None;
    }
    let pct = pr.percent.clamp(0.0, 100.0) / 100.0;
    // (how far the counter has run, its ink)
    let (frac, ink) = match st {
        St::Downloading => (pct, p.text),
        St::Converting | St::Embedding => (1.0, p.text2),
        St::Paused | St::Retrying => (pct, p.text2),
        _ => (0.0, p.muted),
    };
    let counter = if t.duration > 0.0 { label(model::timecode(t.duration * frac), mono(13.0, 500.0), ink) } else { el().h(16.0) };
    // The display window of the desk: a recessed well the ring and the counter sit in.
    Some(well(p).child(task_ring(s, t, p)).child(counter))
}

/// The instrument's well: recessed, the ring at its left.
fn well(p: &Pal) -> El {
    row().w(INSTRUMENT).h(48.0).px(10.0).gap(12.0).center_y().radius(6.0).fill(p.deep).border(1.0, p.line)
}

fn state_col(t: &DownloadTask, p: &Pal) -> El {
    let pr = &t.progress;
    let c = col().w(STATE_COL).items_end().gap(4.0);
    if pr.status == St::Downloading {
        let pct = format!("{}%", format::percent(t));
        return c
            .child(label(pct, mono(16.0, 500.0), p.text))
            .child(label(format::rate(pr.speed, pr.eta), mono(11.0, 400.0), p.muted).align(Align::End));
    }
    let (i, word, tone): (&'static rune::Icon, &str, Color) = match pr.status {
        St::Queued => (&icons::CLOCK, copy::STATE_QUEUED, p.muted),
        St::FetchingInfo => (&icons::LOADING, copy::STATE_STARTING, p.text2),
        St::Converting => (&icons::LOADING, copy::STATE_CONVERTING, p.warning),
        St::Embedding => (&icons::LOADING, copy::STATE_TAGGING, p.warning),
        St::Paused => (&icons::PAUSE, copy::STATE_PAUSED, p.warning),
        St::Retrying => (&icons::RETRY, copy::STATE_RETRYING, p.warning),
        St::Done if pr.already_on_disk == Some(true) => (&icons::CHECK_SIGNAL, copy::STATE_SKIPPED, p.success),
        St::Done => (&icons::CHECK_SIGNAL, copy::STATE_DONE, p.success),
        St::Failed if format::is_permanent(t.error_code) => (&icons::ALERT, copy::STATE_CANT_DOWNLOAD, p.error),
        St::Failed => (&icons::ALERT, copy::STATE_FAILED, p.error),
        St::Cancelled | St::Downloading => (&icons::CANCEL, copy::STATE_CANCELED, p.muted),
    };
    c.child(row().gap(8.0).center_y().child(icon(i, 16.0).color(tone)).child(label(word, sans(13.0, 500.0), tone)))
}

fn actions(t: &DownloadTask, promote: bool, p: &Pal) -> El {
    use ahg_engine::types::ErrorCode as E;
    let st = t.progress.status;
    let id = &t.task_id;
    let act = |i: &'static rune::Icon, words: &str, ink: Color, which: RowAct| row_act(icon_button(i, words, ink, p), which, id);
    let mut a = row().w(ACTIONS).gap(4.0).justify_end().center_y();
    if promote {
        a = a.child(act(&icons::TO_FRONT, copy::ACT_DO_THIS_NEXT, p.text2, RowAct::Promote));
    }
    if matches!(st, St::Queued | St::FetchingInfo | St::Downloading) {
        a = a.child(act(&icons::PAUSE, copy::ACT_PAUSE, p.text2, RowAct::Pause));
    }
    if st == St::Paused {
        a = a.child(act(&icons::PLAY, copy::ACT_RESUME, p.accent_ink, RowAct::Resume));
    }
    if format::is_active(t) {
        a = a.child(act(&icons::CANCEL, copy::ACT_CANCEL, p.text2, RowAct::Cancel));
    }
    let attention = matches!(st, St::Failed | St::Cancelled);
    if st == St::Failed && !t.progress.error.is_empty() {
        a = a.child(act(&icons::COPY, copy::ACT_COPY_DETAILS, p.text2, RowAct::CopyDetails));
    }
    if attention && matches!(t.error_code, Some(E::Private | E::AgeRestricted | E::LoginRequired | E::DiskFull)) {
        a = a.child(act(&icons::SETTINGS, copy::ACT_OPEN_SETTINGS, p.text2, RowAct::Fix));
    }
    if attention && t.error_code == Some(E::Unknown) {
        a = a.child(act(&icons::REFRESH, copy::ACT_UPDATE_ENGINE, p.text2, RowAct::Fix));
    }
    if st == St::Done && t.progress.already_on_disk == Some(true) {
        a = a.child(act(&icons::DOWNLOAD, copy::ACT_DOWNLOAD_AGAIN, p.text2, RowAct::DownloadAgain));
    }
    if attention && !format::is_permanent(t.error_code) {
        a = a.child(act(&icons::RETRY, copy::ACT_RETRY, p.error, RowAct::Retry));
    }
    if st == St::Done {
        a = a.child(act(&icons::EXTERNAL, copy::ACT_OPEN_FILE, p.text2, RowAct::OpenFile)).child(act(
            &icons::FOLDER,
            copy::ACT_SHOW_IN_FOLDER,
            p.text2,
            RowAct::ShowInFolder,
        ));
    }
    if matches!(st, St::Done | St::Failed | St::Cancelled) {
        a = a.child(act(&icons::REMOVE, copy::ACT_REMOVE, p.muted, RowAct::Remove).group_opacity(0.0, 1.0));
    }
    a
}
