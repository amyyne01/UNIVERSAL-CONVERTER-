//! Downloads: every task as a sleeve on one surface. While a file comes in its
//! record stands beside the sleeve, its rim filling; once it is done the record is back inside
//! and the row reads as a file you own: its name and where it is.
use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode};
use rune::{Color, El, Icon, ScrollSpec, col, el, icon, row, text};

use super::parts::{self, Disc, danger_button, ghost, icon_button, primary, run, run_mono, runs};
use super::theme::{Pal, mono, sans, serif};
use crate::gate::{Confirm, Filter, Nav, State};
use crate::keys::{self, RowAct};
use crate::ring::Pass;
use crate::{actions, copy, format, icons};

const ROW_H: f32 = 60.0;
const SLEEVE: f32 = 48.0;

pub fn view(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let mut page = col().w_full().max_w(1480.0).self_center().h_full().min_h(0.0).px(40.0).pt(16.0).pb(24.0).gap(16.0).child(header(s, p));
    if !tasks.is_empty() {
        page = page.child(strip(s, p));
    }
    let interrupted = tasks.iter().filter(|t| t.progress.status == St::Paused && t.interrupted == Some(true)).count();
    if interrupted > 0 {
        page = page.child(banner(p, interrupted));
    }
    let shown: Vec<&DownloadTask> = actions::visible(s).collect();
    page = page
        .child(col().key("list-scroll").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list(s, p, &shown)));
    col().key("downloads").w_full().grow(1.0).min_h(0.0).child(page)
}

// ── States shared with The collection ───────────────────────────────────────

/// An armed two-step action: its question, a way back, and the red confirm.
pub(crate) fn armed(p: &Pal, aria: &str, question: &str) -> El {
    row()
        .gap(8.0)
        .center_y()
        .role("group")
        .label(aria)
        .child(text(question).font(sans(13.0, 500.0)).color(p.text).single_line().mr(4.0))
        .child(ghost(p, keys::CONFIRM_NO, None, copy::DL_CONFIRM_CANCEL, p.text))
        .child(danger_button(p, keys::CONFIRM_YES, copy::DL_CONFIRM_YES))
}

/// A quiet text button, no edge.
fn plain(p: &Pal, key: &str, label: &str) -> El {
    row()
        .key(key)
        .h(32.0)
        .px(8.0)
        .center_y()
        .radius(8.0)
        .hover(p.raised)
        .focusable(true)
        .role("button")
        .label(label)
        .child(text(label).font(sans(13.0, 500.0)).color(p.text2).single_line())
}

/// What is ticked: the count, Cancel for the active ones, Remove for the finished ones
/// (armed, "Remove N?"), and the way to clear the ticks. Selection includes hidden rows.
pub(crate) fn selection(s: &State, p: &Pal) -> El {
    let picked = |t: &&DownloadTask| s.selected.contains(&t.task_id);
    let active = s.tasks.iter().filter(picked).any(format::is_active);
    let finished = s.tasks.iter().filter(picked).filter(|t| matches!(t.progress.status, St::Done | St::Failed | St::Cancelled)).count();
    let off = |b: El, on: bool| if on { b } else { b.disabled(true).opacity(0.4) };
    let remove = if s.confirm == Some(Confirm::Remove) {
        armed(p, copy::DL_REMOVE_CONFIRM_ARIA, &copy::remove_confirm(finished))
    } else {
        off(ghost(p, keys::arm(Confirm::Remove), Some(&icons::REMOVE), copy::DL_SELECTION_REMOVE, p.text), finished > 0)
    };
    row()
        .key("selection")
        .gap(8.0)
        .center_y()
        .child(text(copy::selected_count(s.selected.len())).font(mono(12.0, 500.0)).color(p.text).single_line().mr(4.0))
        .child(off(ghost(p, keys::CANCEL_SELECTED, None, copy::DL_SELECTION_CANCEL, p.text), active))
        .child(remove)
        .child(plain(p, keys::CLEAR_SELECTION, copy::DL_SELECTION_CLEAR))
}

/// The select-all tick: it acts on the rows on show after the filter and the search.
pub(crate) fn select_all(s: &State, p: &Pal) -> El {
    let shown: Vec<&DownloadTask> = actions::visible(s).collect();
    let ticked = shown.iter().filter(|t| s.selected.contains(&t.task_id)).count();
    let all = !shown.is_empty() && ticked == shown.len();
    parts::checkbox(p, keys::SELECT_ALL.into(), copy::DL_SELECT_ALL_ARIA, all, ticked > 0 && !all)
}

/// The search box: what is typed, or the placeholder.
pub(crate) fn search_text(s: &State, p: &Pal) -> El {
    let typed = s.query.text.as_str();
    let (shown, ink) = if typed.is_empty() { (copy::DL_SEARCH_PLACEHOLDER, p.muted) } else { (typed, p.text) };
    text(shown).font(sans(13.0, 400.0)).color(ink).single_line().truncate().shrink()
}

/// The filter and the search show nothing although downloads exist: why, and the way back.
pub(crate) fn filtered_empty(s: &State, p: &Pal, sleeve: f32) -> El {
    let total = s.tasks.len();
    let (glyph, title) = match s.filter {
        Filter::Active => (&icons::CLOCK, copy::EMPTY_FILTER_ACTIVE),
        Filter::Done => (&icons::CLOCK, copy::EMPTY_FILTER_DONE),
        Filter::Failed => (&icons::CHECK_SHELF, copy::EMPTY_FILTER_FAILED),
        // Electron's wording for a search that matches nothing under All (CONTENT.md 5).
        Filter::All => (&icons::CLOCK, copy::EMPTY_FILTER_FAILED),
    };
    // TODO(shared): keys::SHOW_ALL, the filtered-empty state's "Show all N".
    let show_all = ghost(p, "show-all", None, &copy::show_all(total), p.text);
    empty_shape(p, sleeve, glyph, title, &copy::empty_filter_body(total), show_all)
}

/// An empty state in a row's shape, so the columns stay aligned: a blank sleeve, words, one button.
fn empty_shape(p: &Pal, sleeve: f32, glyph: &'static Icon, title: &str, body: &str, button: El) -> El {
    let blank = el()
        .size(sleeve, sleeve)
        .radius(4.0)
        .border(1.0, p.control.alpha(0.6))
        .border_dash(4.0, 4.0)
        .center()
        .child(icon(glyph, 16.0).color(p.muted));
    row()
        .key("empty")
        .w_full()
        .h(ROW_H + 12.0)
        .pl(64.0)
        .pr(16.0)
        .gap(16.0)
        .center_y()
        .child(blank)
        .child(
            col()
                .grow(1.0)
                .gap(4.0)
                .child(text(title).font(sans(15.0, 500.0)).color(p.text))
                .child(text(body).font(sans(13.0, 400.0)).color(p.text2)),
        )
        .child(button)
}

// ── Header ──────────────────────────────────────────────────────────────────

fn header(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let st = |f: &dyn Fn(St) -> bool| tasks.iter().filter(|t| f(t.progress.status)).count();
    let active = format::active_count(tasks);
    let any_paused = st(&|s| s == St::Paused) > 0;
    let all_paused = format::all_paused(tasks);

    let line = copy::active_and_total(active, tasks.len(), all_paused);
    let mut counts = Vec::new();
    let mut rest = line.as_str();
    // Figures in secondary ink, the words muted, ` · paused` in the warning colour.
    while !rest.is_empty() {
        let (part, color) = if rest.starts_with(copy::DL_PAUSED_SUFFIX) {
            (copy::DL_PAUSED_SUFFIX, p.warning)
        } else {
            let digits = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
            if digits > 0 {
                (&rest[..digits], p.text2)
            } else {
                let stop = rest.find(|c: char| c.is_ascii_digit()).unwrap_or(rest.len());
                let stop = rest[..stop].find(copy::DL_PAUSED_SUFFIX).unwrap_or(stop);
                (&rest[..stop], p.muted)
            }
        };
        counts.push(run_mono(part, color));
        rest = &rest[part.len()..];
    }
    // The words give way to the actions (an armed "Clear N?" is the widest): the summary wraps.
    let mut left = col()
        .shrink()
        .min_w(0.0)
        .gap(4.0)
        .child(text(copy::DL_TITLE).font(serif(44.0).lh(48.0)).color(p.text))
        .child(runs(counts).font(mono(12.0, 400.0)).single_line());
    if tasks.len() >= 2 {
        left = left
            .child(text(copy::downloads_summary(tasks.len(), &format::summary_parts(tasks))).font(sans(13.0, 400.0)).color(p.text2).wrap());
    }

    let mut right = row().grow(1.0).gap(8.0).center_y().justify_end();
    if tasks.iter().any(|t| t.progress.status == St::Failed && !format::is_permanent(t.error_code)) {
        right = right.child(ghost(p, keys::RETRY_ALL_FAILED, Some(&icons::RETRY), copy::DL_RETRY_ALL_FAILED, p.text));
    }
    let done = st(&|s| s == St::Done);
    if done > 0 {
        right = right.child(if s.confirm == Some(Confirm::Clear) {
            armed(p, copy::DL_CLEAR_CONFIRM_ARIA, &copy::clear_confirm(done))
        } else {
            ghost(p, keys::arm(Confirm::Clear), Some(&icons::REMOVE), copy::DL_CLEAR_COMPLETED, p.text)
        });
    }
    if any_paused || st(&|s| matches!(s, St::Queued | St::FetchingInfo | St::Downloading)) > 0 {
        right = right.child(if all_paused {
            ghost(p, keys::PAUSE_ALL, Some(&icons::PLAY), copy::DL_RESUME_ALL, p.accent)
        } else {
            ghost(p, keys::PAUSE_ALL, Some(&icons::PAUSE), copy::DL_PAUSE_ALL, p.text)
        });
    }
    if active > 0 {
        right = right.child(ghost(p, keys::CANCEL_ALL, Some(&icons::CANCEL), copy::DL_CANCEL_ALL, p.text));
    }
    row().w_full().gap(24.0).items_end().child(left).child(right)
}

// ── Control strip ───────────────────────────────────────────────────────────

fn strip(s: &State, p: &Pal) -> El {
    let counts = format::filter_counts(&s.tasks);
    let labels = [copy::DL_FILTER_ALL, copy::DL_FILTER_ACTIVE, copy::DL_FILTER_DONE, copy::DL_FILTER_FAILED];
    let segments = Filter::ALL.into_iter().zip(labels).zip(counts).map(|((filter, label), count)| (filter, label, count));
    let mut group = row()
        .h(36.0)
        .padding(4.0)
        .gap(4.0)
        .center_y()
        .radius(10.0)
        .fill(p.sunk)
        .border(1.0, p.line_soft)
        .role("radiogroup")
        .label(copy::DL_FILTER_ARIA);
    for (filter, label, count) in segments {
        let on = s.filter == filter;
        let mut seg = row()
            .key(keys::filter(filter))
            .h(28.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(6.0)
            .focusable(true)
            .role("radio")
            .checked(on)
            .child(text(label).font(sans(13.0, if on { 500.0 } else { 400.0 })).color(if on { p.text } else { p.text2 }))
            .child(text(count.to_string()).font(mono(12.0, 400.0)).color(p.muted));
        seg = if on { seg.fill(p.raised).border(1.0, p.line_soft) } else { seg.hover(p.raised.alpha(0.6)) };
        group = group.child(seg);
    }
    let search = row()
        .key(keys::DOWNLOADS_SEARCH)
        .w(240.0)
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.card)
        .border(1.0, p.control.alpha(0.55))
        .role("searchbox")
        .label(copy::DL_SEARCH_ARIA)
        .child(icon(&icons::SEARCH, 16.0).color(p.muted))
        .child(search_text(s, p));
    let mut strip = row().w_full().h(36.0).gap(16.0).center_y().pl(28.0).child(select_all(s, p).ml(8.0)).child(group).child(el().grow(1.0));
    if !s.selected.is_empty() {
        strip = strip.child(selection(s, p));
    }
    strip.child(search)
}

fn banner(p: &Pal, n: usize) -> El {
    row()
        .key("interrupted")
        .w_full()
        .h(40.0)
        .pl(16.0)
        .pr(4.0)
        .gap(12.0)
        .center_y()
        .radius(12.0)
        .fill(p.soft(p.warning))
        .border(1.0, p.warning.alpha(0.16))
        .child(icon(&icons::PAUSE, 16.0).color(p.warning))
        .child(
            runs(vec![
                parts::Run { text: n.to_string(), color: p.text, mono: true, weight: Some(500.0) },
                run(copy::interrupted_sentence(n), p.text),
            ])
            .font(sans(13.0, 400.0))
            .single_line(),
        )
        .child(el().grow(1.0))
        .child(primary(p, keys::RESUME_INTERRUPTED, Some(&icons::PLAY), copy::DL_INTERRUPTED_RESUME))
        .child(ghost(p, keys::DISCARD_INTERRUPTED, Some(&icons::REMOVE), copy::DL_INTERRUPTED_DISCARD, p.text))
}

// ── The list ────────────────────────────────────────────────────────────────

/// The rows on show (`tasks`: after the filter and the search), grouped.
fn list(s: &State, p: &Pal, tasks: &[&DownloadTask]) -> El {
    let surface = col().key("list").w_full().radius(14.0).fill(p.card).border(1.0, p.line_soft).clip_rounded(14.0);
    if s.tasks.is_empty() {
        return surface.child(empty_row(p));
    }
    if tasks.is_empty() {
        return surface.child(filtered_empty(s, p, SLEEVE));
    }
    let only = |f: &dyn Fn(St) -> bool| -> Vec<&DownloadTask> { tasks.iter().copied().filter(|t| f(t.progress.status)).collect() };
    let live: Vec<&DownloadTask> = tasks.iter().copied().filter(|t| format::is_live(t)).collect();
    let paused = only(&|st| st == St::Paused);
    let queued = only(&|st| st == St::Queued);
    let attention = only(&|st| matches!(st, St::Failed | St::Cancelled));
    let done = only(&|st| st == St::Done);
    let in_progress = live.len() + paused.len() + queued.len();
    let groups = [in_progress, attention.len(), done.len()].iter().filter(|n| **n > 0).count();
    let all_paused = format::all_paused(&s.tasks);

    let mut out = surface;
    let mut first = true;
    let mut add = |out: El, row: El| {
        let r = if first { row } else { col().w_full().child(el().w_full().h(1.0).fill(p.line_soft)).child(row) };
        first = false;
        out.child(r)
    };
    if in_progress > 0 {
        if groups > 1 {
            let total = if live.is_empty() {
                None
            } else {
                let sp = format::speed(format::total_speed(&s.tasks));
                let line = if sp.is_empty() { live.len().to_string() } else { format!("{} · {sp}", live.len()) };
                Some(
                    row()
                        .gap(8.0)
                        .center_y()
                        .label(copy::transferring_aria(live.len(), &sp))
                        .child(icon(&icons::DOWNLOAD, 16.0).color(p.muted))
                        .child(text(line).font(mono(12.0, 400.0)).color(p.text2)),
                )
            };
            out = add(out, group_header(p, "group-progress", copy::GROUP_IN_PROGRESS, in_progress, all_paused, total));
        }
        for t in live.iter().chain(paused.iter()) {
            out = add(out, task_row(s, p, t, false, false));
        }
        if !queued.is_empty() && (!live.is_empty() || !paused.is_empty()) {
            out = add(
                out,
                row()
                    .key("up-next")
                    .w_full()
                    .h(28.0)
                    .pl(64.0)
                    .center_y()
                    .child(text(copy::up_next(queued.len())).font(mono(11.0, 400.0)).color(p.muted)),
            );
        }
        let reorderable = queued.len() >= 2;
        for (i, t) in queued.iter().enumerate() {
            out = add(out, task_row(s, p, t, reorderable, reorderable && i > 0));
        }
    }
    if !attention.is_empty() {
        if groups > 1 {
            out = add(out, group_header(p, "group-attention", copy::GROUP_NEEDS_ATTENTION, attention.len(), false, None));
        }
        for t in &attention {
            out = add(out, task_row(s, p, t, false, false));
        }
    }
    if !done.is_empty() {
        if groups > 1 {
            out = add(out, group_header(p, "group-done", copy::GROUP_COMPLETED, done.len(), false, None));
        }
        for t in &done {
            out = add(out, task_row(s, p, t, false, false));
        }
    }
    out
}

fn group_header(p: &Pal, key: &str, name: &str, count: usize, paused: bool, total: Option<El>) -> El {
    let mut title = vec![run(name, p.text2)];
    if paused {
        title.push(run(copy::DL_PAUSED_SUFFIX, p.warning));
    }
    let mut r = row()
        .key(key)
        .w_full()
        .h(32.0)
        .px(16.0)
        .gap(8.0)
        .center_y()
        .fill(p.sunk.mix(p.card, 0.5))
        .child(runs(title).font(sans(12.0, 500.0)).single_line())
        .child(text(count.to_string()).font(mono(12.0, 400.0)).color(p.muted));
    if let Some(t) = total {
        r = r.child(el().grow(1.0)).child(t);
    }
    r
}

fn empty_row(p: &Pal) -> El {
    let paste = ghost(p, "paste-a-link", None, copy::EMPTY_NONE_ACTION, p.text).on_press(|s: &mut State| s.nav = Nav::Home);
    empty_shape(p, SLEEVE, &icons::DOWNLOAD, copy::EMPTY_NONE_TITLE, copy::EMPTY_NONE_BODY, paste)
}

// ── A row ───────────────────────────────────────────────────────────────────

/// What the state column says, and in which tone.
fn state_word(p: &Pal, t: &DownloadTask) -> (&'static Icon, &'static str, Color) {
    match t.progress.status {
        St::Queued => (&icons::CLOCK, copy::STATE_QUEUED, p.muted),
        St::FetchingInfo => (&icons::SPINNER, copy::STATE_STARTING, p.text2),
        St::Converting => (&icons::SPINNER, copy::STATE_CONVERTING, p.warning),
        St::Embedding => (&icons::SPINNER, copy::STATE_TAGGING, p.warning),
        St::Paused => (&icons::PAUSE, copy::STATE_PAUSED, p.warning),
        St::Retrying => (&icons::RETRY, copy::STATE_RETRYING, p.warning),
        St::Done if t.progress.already_on_disk == Some(true) => (&icons::CHECK_SHELF, copy::STATE_SKIPPED, p.success),
        St::Done => (&icons::CHECK_SHELF, copy::STATE_DONE, p.success),
        St::Failed if format::is_permanent(t.error_code) => (&icons::ALERT, copy::STATE_CANT_DOWNLOAD, p.error),
        St::Failed => (&icons::ALERT, copy::STATE_FAILED, p.error),
        St::Cancelled => (&icons::CANCEL, copy::STATE_CANCELED, p.muted),
        St::Downloading => (&icons::DOWNLOAD, "", p.text),
    }
}

/// The record stands beside its sleeve while its rim is the ring (decision 9a); landed, it is filed.
fn disc(s: &State, t: &DownloadTask) -> Disc {
    match (t.progress.status, s.rings.pass(&t.task_id)) {
        (St::Failed | St::Cancelled, _) => Disc::None,
        (St::Done, Some(Pass::Landed) | None) => Disc::Out(0.0),
        _ => Disc::Out(1.0),
    }
}

fn task_row(s: &State, p: &Pal, t: &DownloadTask, grip: bool, do_next: bool) -> El {
    let status = t.progress.status;
    let id = &t.task_id;
    let label = format::label(t);
    let failed = status == St::Failed;
    let selected = s.selected.contains(id);
    let finished = matches!(status, St::Done | St::Cancelled);
    let reason = format::reason(t);

    // 1-2. Grip and checkbox.
    let grip_cell = if grip {
        el().w(16.0)
            .h(28.0)
            .center()
            .group_opacity(0.0, 1.0)
            .focusable(true)
            .role("button")
            .label(copy::row_reorder_aria(&label))
            .child(icon(&icons::GRIP, 16.0).color(p.muted))
    } else {
        el().w(16.0)
    };
    // 3. The sleeve, in a fixed slot so the text column never moves.
    let slot = el().w(84.0).h(SLEEVE).child(parts::sleeve(s, p, t, SLEEVE, disc(s, t), matches!(status, St::Failed | St::Cancelled)));

    // 4. Title and detail.
    let show_link = failed;
    let mut title = vec![run(label.clone(), if finished { p.text2 } else { p.text })];
    if show_link && !t.title.is_empty() {
        title.push(run(copy::row_dot_part(&format::link_label(&t.url)), p.muted));
    }
    let title_el = runs(title).font(sans(13.0, if finished { 400.0 } else { 500.0 })).single_line().truncate().help(if show_link {
        t.url.clone()
    } else {
        label.clone()
    });

    let source = format::source_name(t.source);
    let mut detail = vec![run(source, p.source(t.source))];
    let mut wraps = false;
    let done = status == St::Done;
    if let Some(r) = reason.clone() {
        detail.push(run(copy::row_dot_part(&r), p.error));
        wraps = true;
    } else if done && t.progress.already_on_disk == Some(true) {
        detail.push(run(copy::ROW_ALREADY_DOWNLOADED, p.text2));
    } else if status == St::Retrying {
        let k = t.retry_count.unwrap_or(0.0) as usize;
        detail.push(run(copy::row_retrying(format::retry_wait(t), k), p.warning));
    } else {
        if !t.uploader.is_empty() {
            detail.push(run(copy::row_dot_part(&t.uploader), p.text2));
        }
        let specs = format::specs(t);
        if !specs.is_empty() {
            detail.push(run_mono(copy::row_dot_part(&specs), p.muted));
        }
    }
    let mut detail_el = runs(detail).font(sans(12.0, 400.0));
    detail_el = if wraps { detail_el.max_lines(2).wrap() } else { detail_el.single_line().truncate() };
    if (failed || status == St::Cancelled) && !t.progress.error.is_empty() {
        detail_el = detail_el.help(t.progress.error.clone());
    }
    let mut block = col().grow(1.0).min_w(0.0).gap(4.0).child(title_el).child(detail_el);
    if done && !t.progress.filename.is_empty() {
        // The file you now own: its name, then the folder it sits in.
        let file = format::file_name(&t.progress.filename).to_string();
        let folder = format::folder(&t.progress.filename).to_string();
        block = block.child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .help(t.progress.filename.clone())
                .child(icon(&icons::FOLDER, 16.0).color(p.muted))
                .child(text(file).font(sans(12.0, 500.0)).color(p.text).single_line().truncate().shrink())
                .child(text(folder).font(sans(12.0, 400.0)).color(p.muted).single_line().truncate().shrink()),
        );
    }

    // 5. State column.
    let state = if status == St::Downloading {
        let pct = format::percent(t);
        col()
            .w(128.0)
            .gap(0.0)
            .items_end()
            .child(text(format!("{pct}%")).font(mono(15.0, 500.0)).color(p.text))
            .child(text(format::rate(t.progress.speed, t.progress.eta)).font(mono(11.0, 400.0)).color(p.muted).single_line())
    } else {
        let (glyph, word, tone) = state_word(p, t);
        row()
            .w(128.0)
            .gap(8.0)
            .center_y()
            .justify_end()
            .child(icon(glyph, 16.0).color(tone))
            .child(text(word).font(sans(13.0, 500.0)).color(tone).single_line())
    };

    // 6. Actions.
    let fixable = matches!(status, St::Failed | St::Cancelled);
    let kind = t.error_code.unwrap_or(ErrorCode::Unknown);
    let mut actions = row().min_w(100.0).gap(4.0).center_y().justify_end();
    let mut act = |which: RowAct, glyph: &'static Icon, name: &'static str, ink: Color| {
        actions = std::mem::replace(&mut actions, row()).child(icon_button(p, keys::row_action(which, id), glyph, name, ink));
    };
    if do_next {
        act(RowAct::Promote, &icons::TO_FRONT, copy::ACT_DO_THIS_NEXT, p.text2);
    }
    if matches!(status, St::Queued | St::FetchingInfo | St::Downloading) {
        act(RowAct::Pause, &icons::PAUSE, copy::ACT_PAUSE, p.text2);
    }
    if status == St::Paused {
        act(RowAct::Resume, &icons::PLAY, copy::ACT_RESUME, p.accent);
    }
    if format::is_active(t) {
        act(RowAct::Cancel, &icons::CANCEL, copy::ACT_CANCEL, p.text2);
    }
    if failed && !t.progress.error.is_empty() {
        act(RowAct::CopyDetails, &icons::COPY, copy::ACT_COPY_DETAILS, p.text2);
    }
    if fixable
        && t.error_code.is_some()
        && matches!(kind, ErrorCode::Private | ErrorCode::AgeRestricted | ErrorCode::LoginRequired | ErrorCode::DiskFull)
    {
        act(RowAct::Fix, &icons::SETTINGS, copy::ACT_OPEN_SETTINGS, p.text2);
    }
    if fixable && t.error_code == Some(ErrorCode::Unknown) {
        act(RowAct::Fix, &icons::REFRESH, copy::ACT_UPDATE_ENGINE, p.text2);
    }
    if done && t.progress.already_on_disk == Some(true) {
        act(RowAct::DownloadAgain, &icons::DOWNLOAD, copy::ACT_DOWNLOAD_AGAIN, p.text2);
    }
    if fixable && !format::is_permanent(t.error_code) {
        act(RowAct::Retry, &icons::RETRY, copy::ACT_RETRY, p.error);
    }
    if done {
        act(RowAct::OpenFile, &icons::EXTERNAL, copy::ACT_OPEN_FILE, p.text2);
        act(RowAct::ShowInFolder, &icons::FOLDER, copy::ACT_SHOW_IN_FOLDER, p.text2);
    }
    if matches!(status, St::Done | St::Failed | St::Cancelled) {
        actions = actions
            .child(icon_button(p, keys::row_action(RowAct::Remove, id), &icons::REMOVE, copy::ACT_REMOVE, p.muted).group_opacity(0.0, 1.0));
    }

    let mut r = row()
        .key(keys::row(id))
        .w_full()
        .min_h(ROW_H)
        // A finished row carries a third line, the file: it gets room to breathe.
        .py(if done && !t.progress.filename.is_empty() { 8.0 } else { 4.0 })
        .pl(8.0)
        .pr(12.0)
        .gap(12.0)
        .center_y()
        .hover_group()
        .hover(p.raised.alpha(0.35))
        .child(grip_cell)
        .child(parts::checkbox(p, keys::row_select(id), copy::ROW_SELECT_ARIA, selected, false))
        .child(slot)
        .child(block)
        .child(state)
        .child(actions);
    if selected {
        r = r.fill(p.soft(p.accent));
    } else if failed {
        r = r.fill(p.error.alpha(0.05)).hover(p.error.alpha(0.08));
    }
    r
}
