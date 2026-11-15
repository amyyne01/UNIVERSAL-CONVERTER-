//! Downloads: one surface of passes. Each row's cover sits inside its orbit while it
//! travels; when it lands the ring goes and the cover becomes a square tile, the file
//! with its name, size, format and place.
use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode};
use rune::{Color, El, Icon, ScrollSpec, col, el, icon, row, spacer};

use super::draw::{Pass, orbit};
use super::look::{BODY, BODY_M, FIG, FIG_M, FIG_S, PAGE_TITLE, Pal, SMALL, SMALL_M, mix, txt};
use super::model::{self, is_active, is_live};
use super::{Tone, button, cover, icon_button, icons};
use crate::copy;
use crate::gate::{Nav, State};

const ROW_H: f32 = 64.0;
const RING: f32 = 52.0;

pub fn view(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let mut page = col().w_full().max_w(1600.0).px(40.0).pt(32.0).pb(48.0).gap(20.0).child(header(tasks, p));
    if tasks.is_empty() {
        page = page.child(framed(p, list_surface(p).child(empty_row(p))));
    } else {
        page = page.child(strip(tasks, p));
        let interrupted = tasks.iter().filter(|t| t.progress.status == St::Paused && t.interrupted == Some(true)).count();
        if interrupted > 0 {
            page = page.child(banner(p, interrupted));
        }
        page = page.child(framed(p, list(s, p)));
    }
    col().key("downloads").scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0).w_full().child(page)
}

// ── Header ──────────────────────────────────────────────────────────────────

fn header(tasks: &[DownloadTask], p: &Pal) -> El {
    let active = tasks.iter().filter(|t| is_active(t)).count();
    let paused = tasks.iter().any(|t| t.progress.status == St::Paused);
    let all_paused = paused && !tasks.iter().any(is_live);
    let mut counts = row().child(txt(active.to_string(), FIG, p.text2)).child(txt(copy::DL_ACTIVE_WORD, FIG, p.muted));
    if all_paused {
        counts = counts.child(txt(copy::DL_PAUSED_SUFFIX, FIG, p.warning));
    }
    counts = counts.child(txt(format!(" · {}{}", tasks.len(), copy::DL_TOTAL_WORD), FIG, p.muted));
    let mut left = col().gap(8.0).child(txt(copy::DL_TITLE, PAGE_TITLE, p.text)).child(counts);
    if tasks.len() >= 2 {
        left = left.child(txt(summary(tasks), SMALL, p.text2));
    }

    let mut head = row().w_full().gap(24.0).items_end().justify_between().child(left);
    if !tasks.is_empty() {
        head = head.child(
            row()
                .w(240.0)
                .h(32.0)
                .px(12.0)
                .gap(8.0)
                .center_y()
                .radius(8.0)
                .fill(p.card)
                .border(1.0, p.track)
                .label(copy::DL_SEARCH_ARIA)
                .child(icon(&icons::SEARCH, 16.0).color(p.muted))
                .child(txt(copy::DL_SEARCH_PLACEHOLDER, SMALL, p.muted).truncate()),
        );
    }
    head
}

/// The header's bulk actions, in Electron's order.
fn bulk(tasks: &[DownloadTask], p: &Pal) -> El {
    let active = tasks.iter().filter(|t| is_active(t)).count();
    let paused = tasks.iter().any(|t| t.progress.status == St::Paused);
    let all_paused = paused && !tasks.iter().any(is_live);
    let mut right = row().gap(8.0).center_y().flex_wrap().justify_end();
    if tasks.iter().any(|t| t.progress.status == St::Failed && !model::is_permanent(t.error_code)) {
        right = right.child(button(p, "retry-all", copy::DL_RETRY_ALL_FAILED, Some(&icons::RETRY), Tone::Ghost));
    }
    if tasks.iter().any(|t| t.progress.status == St::Done) {
        right = right.child(button(p, "clear-done", copy::DL_CLEAR_COMPLETED, Some(&icons::REMOVE), Tone::Ghost));
    }
    let pausable = tasks.iter().any(|t| matches!(t.progress.status, St::Queued | St::FetchingInfo | St::Downloading));
    if all_paused {
        right = right.child(button(p, "resume-all", copy::DL_RESUME_ALL, Some(&icons::PLAY), Tone::Primary));
    } else if paused || pausable {
        right = right.child(button(p, "pause-all", copy::DL_PAUSE_ALL, Some(&icons::PAUSE), Tone::Ghost));
    }
    if active > 0 {
        right = right.child(button(p, "cancel-all", copy::DL_CANCEL_ALL, Some(&icons::CANCEL), Tone::Ghost));
    }
    right
}

/// `7 downloads: 1 downloading, 6 failed`, in the fixed order, zero counts left out.
fn summary(tasks: &[DownloadTask]) -> String {
    let bucket = |t: &DownloadTask| match t.progress.status {
        St::FetchingInfo | St::Downloading | St::Converting | St::Embedding => 0,
        St::Retrying => 1,
        St::Queued => 2,
        St::Paused => 3,
        St::Done => 4,
        St::Failed => 5,
        St::Cancelled => 6,
    };
    let mut n = [0usize; 7];
    for t in tasks {
        n[bucket(t)] += 1;
    }
    let parts: Vec<String> = n.iter().zip(copy::SUMMARY_WORDS).filter(|(c, _)| **c > 0).map(|(c, w)| format!("{c} {w}")).collect();
    copy::downloads_summary(tasks.len(), &parts)
}

// ── Control strip and banner ────────────────────────────────────────────────

fn checkbox(p: &Pal, label: &'static str) -> El {
    el().size(16.0, 16.0).radius(4.0).border(1.5, p.track).role("checkbox").label(label).focusable(true).cursor(rune::Cursor::Pointer).hover(p.raised)
}

fn strip(tasks: &[DownloadTask], p: &Pal) -> El {
    let count = |f: fn(&DownloadTask) -> bool| tasks.iter().filter(|t| f(t)).count();
    let filters = [
        (copy::DL_FILTER_ALL, tasks.len()),
        (copy::DL_FILTER_ACTIVE, count(is_active)),
        (copy::DL_FILTER_DONE, count(|t| t.progress.status == St::Done)),
        (copy::DL_FILTER_FAILED, count(|t| matches!(t.progress.status, St::Failed | St::Cancelled))),
    ];
    let segments = filters.iter().enumerate().map(|(i, (label, n))| {
        let on = i == 0;
        let mut seg = row()
            .key(format!("filter-{i}"))
            .h(28.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(6.0)
            .role("radio")
            .focusable(true)
            .cursor(rune::Cursor::Pointer)
            .child(txt(*label, SMALL_M, if on { p.text } else { p.text2 }))
            .child(txt(n.to_string(), FIG, p.muted));
        seg = if on { seg.fill(p.raised) } else { seg.hover(p.hover) };
        seg
    });
    row()
        .w_full()
        .h(36.0)
        .gap(16.0)
        .pl(28.0)
        .center_y()
        .child(checkbox(p, copy::DL_SELECT_ALL_ARIA))
        .child(row().gap(2.0).padding(2.0).radius(8.0).border(1.0, p.line).label(copy::DL_FILTER_ARIA).children(segments))
        .child(spacer())
        .child(bulk(tasks, p))
}

fn banner(p: &Pal, n: usize) -> El {
    let resume = button(p, "resume-interrupted", copy::DL_INTERRUPTED_RESUME, Some(&icons::PLAY), Tone::Primary);
    let discard = button(p, "discard-interrupted", copy::DL_INTERRUPTED_DISCARD, Some(&icons::REMOVE), Tone::Ghost);
    row()
        .w_full()
        .h(48.0)
        .pl(16.0)
        .pr(8.0)
        .gap(12.0)
        .center_y()
        .radius(12.0)
        .fill(mix(p.bg, p.warning, 0.07))
        .border(1.0, mix(p.bg, p.warning, 0.30))
        .child(icon(&icons::PAUSE, 16.0).color(p.warning))
        .child(row().center_y().child(txt(n.to_string(), FIG, p.text)).child(txt(copy::interrupted_sentence(n), BODY, p.text)))
        .child(spacer())
        .child(resume)
        .child(discard)
}

// ── The list ────────────────────────────────────────────────────────────────

fn list_surface(p: &Pal) -> El {
    col().w_full().radius(12.0).fill(p.card).clip_rounded(12.0)
}

/// The surface's hairline, drawn over the rows so their fills never hide it.
fn framed(p: &Pal, surface: El) -> El {
    surface.child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).radius(12.0).border(1.0, p.line).inert())
}

fn list(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let live: Vec<&DownloadTask> = tasks.iter().filter(|t| is_live(t)).collect();
    let paused: Vec<&DownloadTask> = tasks.iter().filter(|t| t.progress.status == St::Paused).collect();
    let queued: Vec<&DownloadTask> = tasks.iter().filter(|t| t.progress.status == St::Queued).collect();
    let trouble: Vec<&DownloadTask> = tasks.iter().filter(|t| matches!(t.progress.status, St::Failed | St::Cancelled)).collect();
    let done: Vec<&DownloadTask> = tasks.iter().filter(|t| t.progress.status == St::Done).collect();
    let in_progress = live.len() + paused.len() + queued.len();
    let groups = [in_progress > 0, !trouble.is_empty(), !done.is_empty()].iter().filter(|g| **g).count();
    let headed = groups > 1;
    let reorderable = queued.len() >= 2;

    let mut out = list_surface(p);
    let mut first = true;
    let rows = |out: El, items: &[&DownloadTask], first: &mut bool| -> El {
        let mut out = out;
        for t in items {
            out = out.child(task_row(s, p, t, !*first, reorderable && t.progress.status == St::Queued));
            *first = false;
        }
        out
    };
    if in_progress > 0 {
        if headed {
            let all_paused = !paused.is_empty() && live.is_empty();
            let speed: f64 = tasks.iter().map(|t| t.progress.speed).sum();
            let mut aside = row().gap(8.0).center_y();
            if !live.is_empty() {
                let figure = if speed > 0.0 { format!("{} · {}", live.len(), copy::format_speed(speed)) } else { live.len().to_string() };
                aside = aside.child(icon(&icons::DOWNLOAD, 16.0).color(p.muted)).child(txt(figure, FIG, p.text2)).label(copy::transferring_aria(live.len(), &copy::format_speed(speed)));
            }
            out = out.child(group_head(p, copy::GROUP_IN_PROGRESS, in_progress, all_paused, aside, first));
            first = false;
        }
        let lead: Vec<&DownloadTask> = live.iter().chain(paused.iter()).copied().collect();
        out = rows(out, &lead, &mut first);
        if !lead.is_empty() && !queued.is_empty() {
            out = out.child(row().w_full().h(28.0).px(16.0).center_y().fill(p.card).border_sides([1.0, 0.0, 0.0, 0.0], p.line).child(txt(copy::up_next(queued.len()), FIG_S, p.muted)));
            first = true;
        }
        out = rows(out, &queued, &mut first);
    }
    if !trouble.is_empty() {
        if headed {
            out = out.child(group_head(p, copy::GROUP_NEEDS_ATTENTION, trouble.len(), false, row(), first));
            first = false;
        }
        out = rows(out, &trouble, &mut first);
    }
    if !done.is_empty() {
        if headed {
            out = out.child(group_head(p, copy::GROUP_COMPLETED, done.len(), false, row(), first));
            first = false;
        }
        out = rows(out, &done, &mut first);
    }
    out
}

fn group_head(p: &Pal, name: &'static str, n: usize, paused: bool, aside: El, first: bool) -> El {
    let mut title = row().gap(8.0).center_y().child(txt(name, SMALL_M, p.text2));
    if paused {
        title = title.child(txt(copy::DL_PAUSED_SUFFIX, FIG, p.warning));
    }
    title = title.child(txt(n.to_string(), FIG, p.muted));
    row()
        .w_full()
        .h(32.0)
        .px(16.0)
        .center_y()
        .fill(mix(p.card, p.bg, 0.55))
        .border_sides([if first { 0.0 } else { 1.0 }, 0.0, 1.0, 0.0], p.line)
        .child(title)
        .child(spacer())
        .child(aside)
}

fn empty_row(p: &Pal) -> El {
    let go = button(p, "paste-a-link", copy::EMPTY_NONE_ACTION, None, Tone::Ghost).on_press(|s: &mut State| s.nav = Nav::Home);
    row()
        .w_full()
        .h(ROW_H)
        .fill(p.card)
        .pl(16.0 + 16.0 + 12.0 + 16.0 + 12.0)
        .pr(16.0)
        .gap(12.0)
        .center_y()
        .child(el().size(RING, RING).center().child(el().size(40.0, 40.0).radius(8.0).fill(p.raised).center().child(icon(&icons::DOWNLOAD, 16.0).color(p.muted))))
        .child(col().grow(1.0).gap(4.0).child(txt(copy::EMPTY_NONE_TITLE, BODY_M, p.text)).child(txt(copy::EMPTY_NONE_BODY, SMALL, p.text2)))
        .child(go)
}

// ── A row ───────────────────────────────────────────────────────────────────

/// The orbit a task is on, and the ink it is drawn in; `None` once it has landed or stopped.
fn pass(p: &Pal, t: &DownloadTask) -> Option<(Pass, Color)> {
    let pct = model::percent(t);
    Some(match t.progress.status {
        St::Queued => (Pass::Planned, p.muted),
        St::FetchingInfo => (Pass::OnThePad, p.accent),
        St::Downloading => (Pass::Flying(pct), p.accent),
        St::Converting | St::Embedding => (Pass::Closing, p.accent),
        St::Paused | St::Retrying => (Pass::Holding(pct), p.warning),
        St::Done | St::Failed | St::Cancelled => return None,
    })
}

fn state_word(p: &Pal, t: &DownloadTask) -> (&'static Icon, &'static str, Color) {
    match t.progress.status {
        St::Queued => (&icons::CLOCK, copy::STATE_QUEUED, p.muted),
        St::FetchingInfo => (&icons::SPINNER, copy::STATE_STARTING, p.text2),
        St::Converting => (&icons::SPINNER, copy::STATE_CONVERTING, p.warning),
        St::Embedding => (&icons::SPINNER, copy::STATE_TAGGING, p.warning),
        St::Paused => (&icons::PAUSE, copy::STATE_PAUSED, p.warning),
        St::Retrying => (&icons::RETRY, copy::STATE_RETRYING, p.warning),
        St::Done if t.progress.already_on_disk == Some(true) => (&icons::CHECK, copy::STATE_SKIPPED, p.success),
        St::Done => (&icons::CHECK, copy::STATE_DONE, p.success),
        St::Failed if model::is_permanent(t.error_code) => (&icons::ALERT, copy::STATE_CANT_DOWNLOAD, p.error),
        St::Failed => (&icons::ALERT, copy::STATE_FAILED, p.error),
        St::Cancelled | St::Downloading => (&icons::CANCEL, copy::STATE_CANCELED, p.muted),
    }
}

fn task_row(s: &State, p: &Pal, t: &DownloadTask, rule: bool, grip: bool) -> El {
    let st = t.progress.status;
    let failed = st == St::Failed;
    let landed = matches!(st, St::Done | St::Cancelled);
    let under = if failed { mix(p.card, p.error, 0.07) } else { p.card };
    let id = &t.task_id;

    // Where it is: on its orbit, or landed.
    let art = match pass(p, t) {
        Some((pass, ink)) => el().size(RING, RING).center().shape(orbit(RING, pass, p.track, ink, under)).child(cover(s, p, t, 32.0, true)),
        None => el().size(RING, RING).center().child(cover(s, p, t, 40.0, false).opacity(if failed || st == St::Cancelled { 0.55 } else { 1.0 })),
    };

    // Line 1: the label, and the link on failed rows that have a title.
    let label = model::label(t);
    let mut line1 = row().w_full().min_w(0.0).center_y().child(
        txt(label.clone(), if landed { BODY } else { BODY_M }, if landed { p.text2 } else { p.text }).truncate().shrink().help(if failed { t.url.clone() } else { label.clone() }),
    );
    if failed && !t.title.is_empty() {
        line1 = line1.child(txt(copy::row_dot_part(&model::link_label(&t.url)), BODY, p.muted).truncate().shrink());
    }
    // Where a landed file is: its folder, at the end of the title line (Electron keeps the
    // full path in a tooltip only).
    let name = model::file_name(&t.progress.filename);
    let folder = t.progress.filename.strip_suffix(name).unwrap_or("").trim_end_matches(['\\', '/']);
    if st == St::Done && !folder.is_empty() {
        line1 = line1.child(spacer()).child(row().ml(16.0).gap(8.0).center_y().child(icon(&icons::FOLDER, 16.0).color(p.muted)).child(txt(folder.to_string(), FIG, p.muted).truncate_middle()));
    }

    // Line 2: the source in its colour, then the first reading that applies.
    let (src_name, src_color) = model::source(p, t.source);
    let mut line2 = row().w_full().min_w(0.0).child(txt(src_name, SMALL_M, src_color));
    let file = model::file_name(&t.progress.filename).to_string();
    if let Some(reason) = model::reason(t) {
        line2 = line2.items_start().child(txt(copy::row_dot_part(&reason), SMALL, p.error).wrap().max_lines(2).shrink().help(t.progress.error.clone()));
    } else if st == St::Done && t.progress.already_on_disk == Some(true) {
        line2 = line2.child(txt(copy::ROW_ALREADY_DOWNLOADED, SMALL, p.text2));
        if !file.is_empty() {
            line2 = line2.child(txt(copy::row_dot_part(&file), SMALL, p.muted).truncate().shrink().help(t.progress.filename.clone()));
        }
    } else if st == St::Retrying {
        let k = t.retry_count.unwrap_or(0.0) as usize;
        let wait = [5u64, 15, 45][(k).min(2)];
        line2 = line2.child(txt(copy::row_retrying(wait, k), SMALL, p.warning));
    } else {
        if !t.uploader.is_empty() {
            line2 = line2.child(txt(copy::row_dot_part(&t.uploader), SMALL, p.text2).truncate().shrink());
        }
        let specs = model::specs(t);
        if !specs.is_empty() {
            line2 = line2.child(txt(copy::row_dot_part(&specs), FIG, p.muted).single_line());
        }
        if st == St::Done && !file.is_empty() {
            line2 = line2.child(txt(copy::row_dot_part(&file), SMALL_M, p.text).truncate().shrink().help(t.progress.filename.clone()));
        }
    }
    if model::reason(t).is_none() {
        line2 = line2.center_y();
    }

    // The state column: the percent while it moves, else a word.
    let state = if st == St::Downloading {
        col().w(128.0).items_end().gap(4.0).child(txt(format!("{}%", t.progress.percent.clamp(0.0, 100.0).round()), FIG_M, p.text)).child(txt(copy::rate_line(t.progress.speed, t.progress.eta), FIG_S, p.muted))
    } else {
        let (glyph, word, tone) = state_word(p, t);
        row().w(128.0).justify_end().gap(8.0).center_y().child(icon(glyph, 16.0).color(tone)).child(txt(word, SMALL_M, tone))
    };

    row()
        .key(format!("row-{id}"))
        .w_full()
        .h(ROW_H)
        .pl(16.0)
        .pr(12.0)
        .gap(12.0)
        .center_y()
        .fill(under)
        .hover(mix(under, p.text, 0.05))
        .hover_group()
        .border_sides([if rule { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0], p.line)
        .child(grip_cell(p, grip, &label))
        .child(checkbox(p, copy::ROW_SELECT_ARIA))
        .child(art)
        .child(col().grow(1.0).min_w(0.0).gap(4.0).child(line1).child(line2))
        .child(state)
        .child(actions(p, t))
}

fn grip_cell(p: &Pal, grip: bool, label: &str) -> El {
    let cell = el().size(16.0, 32.0).center();
    if grip { cell.label(copy::row_reorder_aria(label)).group_opacity(0.0, 1.0).child(icon(&icons::GRIP, 16.0).color(p.muted)) } else { cell }
}

/// The row's buttons, in Electron's order; `Remove` waits for hover.
fn actions(p: &Pal, t: &DownloadTask) -> El {
    let st = t.progress.status;
    let id = &t.task_id;
    let mut a = row().min_w(100.0).gap(4.0).justify_end().center_y();
    let b = |label: &'static str, glyph: &'static Icon, ink: Color| icon_button(p, format!("{label}-{id}"), label, glyph, ink);
    if matches!(st, St::Queued | St::FetchingInfo | St::Downloading) {
        a = a.child(b(copy::ACT_PAUSE, &icons::PAUSE, p.text2));
    }
    if st == St::Paused {
        a = a.child(b(copy::ACT_RESUME, &icons::PLAY, p.text));
    }
    if is_active(t) {
        a = a.child(b(copy::ACT_CANCEL, &icons::CANCEL, p.text2));
    }
    if st == St::Failed && !t.progress.error.is_empty() {
        a = a.child(b(copy::ACT_COPY_DETAILS, &icons::COPY, p.text2));
    }
    if matches!(st, St::Failed | St::Cancelled) {
        if matches!(t.error_code, Some(ErrorCode::Private | ErrorCode::AgeRestricted | ErrorCode::LoginRequired | ErrorCode::DiskFull)) {
            a = a.child(b(copy::ACT_OPEN_SETTINGS, &icons::SETTINGS, p.text2));
        }
        if t.error_code == Some(ErrorCode::Unknown) {
            a = a.child(b(copy::ACT_UPDATE_ENGINE, &icons::REFRESH, p.text2));
        }
    }
    if st == St::Done && t.progress.already_on_disk == Some(true) {
        a = a.child(b(copy::ACT_DOWNLOAD_AGAIN, &icons::DOWNLOAD, p.text2));
    }
    if matches!(st, St::Failed | St::Cancelled) && !model::is_permanent(t.error_code) {
        a = a.child(b(copy::ACT_RETRY, &icons::RETRY, p.error));
    }
    if st == St::Done {
        a = a.child(b(copy::ACT_OPEN_FILE, &icons::EXTERNAL, p.text2)).child(b(copy::ACT_SHOW_IN_FOLDER, &icons::FOLDER, p.text2));
    }
    if model::is_finished(t) {
        a = a.child(b(copy::ACT_REMOVE, &icons::REMOVE, p.muted).group_opacity(0.0, 1.0));
    }
    a
}

