//! Downloads: the desk's channel list. Every row is 64 points in every state.
//! An in-flight row carries the instrument (the tape counter over a segmented
//! level meter); a finished row shows the file you now own in the same slot.
use ahg_engine::types::{DownloadStatus as St, DownloadTask};
use rune::{Align, Color, El, ScrollSpec, col, el, icon, row, spacer};

use super::icons;
use super::model::{self, Group};
use super::parts::{Line, art, button, checkbox, ghost, icon_button, label, meter, meter_held, mono, sans, stack};
use super::theme::{Pal, source_color};
use crate::copy;
use crate::gate::{Nav, State};

const INSTRUMENT: f32 = 220.0;
/// A finished row's file takes more room than the meter: names are long.
const OWNED: f32 = 300.0;
const STATE_COL: f32 = 128.0;
const ACTIONS: f32 = 124.0;
const SEGMENTS: usize = 36;

pub fn view(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let mut page = col().w_full().max_w(1600.0).px(32.0).pt(28.0).pb(32.0).gap(16.0).child(header(tasks, p));
    let interrupted = tasks.iter().filter(|t| t.progress.status == St::Paused && t.interrupted == Some(true)).count();
    if interrupted > 0 {
        page = page.child(banner(interrupted, p));
    }
    if !tasks.is_empty() {
        page = page.child(strip(tasks, p));
    }
    page = page.child(list(s, p));
    col().w_full().h_full().scroll(ScrollSpec { stick_bottom: false }).min_h(0.0).items_center().child(page)
}

fn header(tasks: &[DownloadTask], p: &Pal) -> El {
    let active = model::active_count(tasks);
    let all_paused = model::all_paused(tasks);
    let mut left = stack(8.0).child(label(copy::DL_TITLE, sans(28.0, 500.0), p.text)).child(label(
        copy::active_and_total(active, tasks.len(), all_paused),
        mono(12.0, 400.0),
        p.muted,
    ));
    if tasks.len() >= 2 {
        left = left.child(label(copy::downloads_summary(tasks.len(), &model::summary_parts(tasks)), sans(13.0, 400.0), p.text2));
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
                .role("searchbox")
                .label(copy::DL_SEARCH_ARIA)
                .child(icon(&icons::SEARCH, 16.0).color(p.muted))
                .child(label(copy::DL_SEARCH_PLACEHOLDER, sans(13.0, 400.0), p.muted)),
        );
    }
    if tasks.iter().any(|t| t.progress.status == St::Failed && !model::is_permanent(t.error_code)) {
        right = right.child(ghost(Some(&icons::RETRY), copy::DL_RETRY_ALL_FAILED, p.text2, p));
    }
    if tasks.iter().any(|t| t.progress.status == St::Done) {
        right = right.child(ghost(Some(&icons::REMOVE), copy::DL_CLEAR_COMPLETED, p.text2, p));
    }
    let pausable = tasks.iter().any(|t| matches!(t.progress.status, St::Queued | St::FetchingInfo | St::Downloading));
    if pausable || tasks.iter().any(|t| t.progress.status == St::Paused) {
        right = right.child(if all_paused {
            button(Some(&icons::PLAY), copy::DL_RESUME_ALL, p.accent_ink, p)
        } else {
            button(Some(&icons::PAUSE), copy::DL_PAUSE_ALL, p.text, p)
        });
    }
    if active > 0 {
        right = right.child(ghost(Some(&icons::CANCEL), copy::DL_CANCEL_ALL, p.text2, p));
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
        .child(primary(Some(&icons::PLAY), copy::DL_INTERRUPTED_RESUME, p))
        .child(ghost(Some(&icons::REMOVE), copy::DL_INTERRUPTED_DISCARD, p.text2, p))
}

/// The one filled button: off-white ink on dark, ink on light.
fn primary(i: Option<&'static rune::Icon>, words: &str, p: &Pal) -> El {
    ghost(i, words, p.bg, p).fill(p.text).hover(p.bg.alpha(0.12))
}

fn strip(tasks: &[DownloadTask], p: &Pal) -> El {
    let count = |g: &dyn Fn(&DownloadTask) -> bool| tasks.iter().filter(|t| g(t)).count();
    let filters = [
        (copy::DL_FILTER_ALL, tasks.len()),
        (copy::DL_FILTER_ACTIVE, count(&|t| model::is_active(t))),
        (copy::DL_FILTER_DONE, count(&|t| t.progress.status == St::Done)),
        (copy::DL_FILTER_FAILED, count(&|t| matches!(t.progress.status, St::Failed | St::Cancelled))),
    ];
    let mut seg = row().padding(4.0).gap(4.0).radius(8.0).fill(p.deep).border(1.0, p.line).role("radiogroup").label(copy::DL_FILTER_ARIA);
    for (i, (name, n)) in filters.into_iter().enumerate() {
        let on = i == 0;
        seg = seg.child(
            row()
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
    row().w_full().h(40.0).pl(16.0).gap(16.0).center_y().child(checkbox(p).label(copy::DL_SELECT_ALL_ARIA)).child(seg)
}

fn list(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let surface = col().w_full().radius(12.0).fill(p.surface).border(1.0, p.line).clip_rounded(12.0);
    if tasks.is_empty() {
        return surface.child(empty_row(p));
    }
    let groups = [Group::InProgress, Group::Attention, Group::Completed];
    let present: Vec<Group> = groups.into_iter().filter(|g| tasks.iter().any(|t| model::group_of(t) == *g)).collect();
    let mut out = surface;
    for g in &present {
        let mut rows: Vec<&DownloadTask> = tasks.iter().filter(|t| model::group_of(t) == *g).collect();
        if *g == Group::InProgress {
            // Live rows, then paused, then queued (3.4).
            rows.sort_by_key(|t| match t.progress.status {
                St::Paused => 1,
                St::Queued => 2,
                _ => 0,
            });
        }
        if present.len() > 1 {
            out = out.child(group_header(*g, &rows, tasks, p));
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
    if g == Group::InProgress && model::all_paused(tasks) {
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
    let live = tasks.iter().filter(|t| model::is_live(t)).count();
    if g == Group::InProgress && live > 0 {
        let speed = copy::format_speed(model::total_speed(tasks));
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

fn empty_row(p: &Pal) -> El {
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
        // The idle channel: the meter's well, every segment dark until something comes in.
        .child(
            stack(0.0)
                .w(INSTRUMENT)
                .h(48.0)
                .px(12.0)
                .justify_center()
                .radius(6.0)
                .fill(p.deep)
                .border(1.0, p.line)
                .child(meter(0.0, SEGMENTS, None, 8.0, 2.0, p.accent, p.unlit)),
        )
        .child(
            row()
                .w(STATE_COL + ACTIONS + 12.0)
                .justify_end()
                .child(button(None, copy::EMPTY_NONE_ACTION, p.text, p).on_press(|s: &mut State| s.nav = Nav::Home)),
        )
}

fn task_row(s: &State, t: &DownloadTask, rule_above: bool, p: &Pal) -> El {
    let st = t.progress.status;
    let failed = st == St::Failed;
    let mut r = row()
        .key(t.task_id.as_str())
        .w_full()
        .h(64.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .hover_group()
        .hover(p.hover)
        .child(el().w(16.0))
        .child(checkbox(p).label(copy::ROW_SELECT_ARIA))
        .child(art(s, t, Some(40.0), 40.0, 6.0, p))
        .child(title_block(t, p));
    if rule_above {
        r = r.border_sides([1.0, 0.0, 0.0, 0.0], p.line);
    }
    if failed {
        r = r.fill(p.error_soft);
    }
    if let Some(slot) = instrument(t, p) {
        r = r.child(slot);
    }
    r.child(state_col(t, p)).child(actions(t, p))
}

fn title_block(t: &DownloadTask, p: &Pal) -> El {
    let st = t.progress.status;
    let quiet = matches!(st, St::Done | St::Cancelled);
    let name = model::label(t);
    let mut title = if quiet { Line::new().add(&name, p.text2) } else { Line::new().strong(&name, p.text) };
    let show_link = st == St::Failed;
    if show_link && !t.title.trim().is_empty() {
        title = title.add(&copy::row_dot_part(&model::link_label(&t.url)), p.muted);
    }
    let title = title.one(sans(14.0, 400.0)).help(if show_link { t.url.clone() } else { name.clone() });

    let mut detail = Line::new().add(model::source_name(t.source), source_color(p, t.source));
    let reason = model::reason(t);
    let skipped = st == St::Done && t.progress.already_on_disk == Some(true);
    let wraps = reason.is_some();
    if let Some(reason) = &reason {
        detail = detail.add(&copy::row_dot_part(reason), p.error);
    } else if skipped {
        detail = detail.add(copy::ROW_ALREADY_DOWNLOADED, p.text2);
    } else if st == St::Retrying {
        let k = t.retry_count.unwrap_or(0.0).max(0.0) as usize;
        let delay = [5, 15, 45][k.min(2)];
        detail = detail.add(&copy::row_retrying(delay, k), p.warning);
    } else {
        if !t.uploader.is_empty() {
            detail = detail.add(&copy::row_dot_part(&t.uploader), p.text2);
        }
        for part in specs(t) {
            detail = detail.add(&copy::row_dot_part(""), p.muted).fig(&part, p.muted);
        }
    }
    let style = sans(13.0, 400.0).lh(16.0);
    let detail = if wraps { detail.wrap(style).max_lines(2).min_w(0.0) } else { detail.one(style) };
    let detail = if t.progress.error.is_empty() { detail } else { detail.help(t.progress.error.clone()) };
    stack(4.0).grow(1.0).child(title).child(detail)
}

/// `[playlist position] · [duration] · [size] · [format]`, empty parts dropped.
fn specs(t: &DownloadTask) -> Vec<String> {
    let pr = &t.progress;
    let mut parts = Vec::new();
    if t.is_playlist && pr.playlist_total > 0.0 {
        parts.push(format!("{}/{}", pr.playlist_index, pr.playlist_total));
    }
    parts.push(copy::format_duration(t.duration));
    let bytes = if pr.total > 0.0 { pr.total } else { pr.downloaded };
    if bytes > 0.0 {
        parts.push(copy::format_bytes(bytes));
    }
    parts.push(t.format.clone());
    parts.retain(|s| !s.is_empty());
    parts
}

/// The instrument slot: the tape counter over the meter while in flight, the
/// owned file once done, nothing for rows that need attention.
fn instrument(t: &DownloadTask, p: &Pal) -> Option<El> {
    let pr = &t.progress;
    let st = pr.status;
    if st == St::Done {
        let file = model::file_name(&pr.filename);
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
                        .child(label(model::folder(&pr.filename), mono(11.0, 400.0), p.muted).single_line().truncate_middle().min_w(0.0)),
                )
                .help(pr.filename.clone()),
        );
    }
    if matches!(st, St::Failed | St::Cancelled) {
        return None;
    }
    let pct = pr.percent.clamp(0.0, 100.0) / 100.0;
    // (how far the meter is lit, its colour, the counter's ink)
    let (frac, lit, ink) = match st {
        St::Downloading => (pct, p.accent, p.text),
        St::Converting | St::Embedding => (1.0, p.text2, p.text2),
        St::Paused | St::Retrying => (pct, p.warning, p.text2),
        _ => (0.0, p.muted, p.muted),
    };
    let held = matches!(st, St::Paused | St::Retrying);
    let counter = if t.duration > 0.0 { label(model::timecode(t.duration * frac), mono(13.0, 500.0), ink) } else { el().h(16.0) };
    // The display window of the desk: a recessed well the counter and meter sit in.
    Some(
        stack(4.0)
            .w(INSTRUMENT)
            .h(48.0)
            .px(12.0)
            .justify_center()
            .radius(6.0)
            .fill(p.deep)
            .border(1.0, p.line)
            .child(counter)
            .child(meter_held(frac, SEGMENTS, None, 8.0, 2.0, lit, p.unlit, held)),
    )
}

fn state_col(t: &DownloadTask, p: &Pal) -> El {
    let pr = &t.progress;
    let c = col().w(STATE_COL).items_end().gap(4.0);
    if pr.status == St::Downloading {
        let pct = format!("{}%", pr.percent.clamp(0.0, 100.0).round());
        return c
            .child(label(pct, mono(16.0, 500.0), p.text))
            .child(label(copy::rate_line(pr.speed, pr.eta), mono(11.0, 400.0), p.muted).align(Align::End));
    }
    let (i, word, tone): (&'static rune::Icon, &str, Color) = match pr.status {
        St::Queued => (&icons::CLOCK, copy::STATE_QUEUED, p.muted),
        St::FetchingInfo => (&icons::LOADING, copy::STATE_STARTING, p.text2),
        St::Converting => (&icons::LOADING, copy::STATE_CONVERTING, p.warning),
        St::Embedding => (&icons::LOADING, copy::STATE_TAGGING, p.warning),
        St::Paused => (&icons::PAUSE, copy::STATE_PAUSED, p.warning),
        St::Retrying => (&icons::RETRY, copy::STATE_RETRYING, p.warning),
        St::Done if pr.already_on_disk == Some(true) => (&icons::CHECK, copy::STATE_SKIPPED, p.success),
        St::Done => (&icons::CHECK, copy::STATE_DONE, p.success),
        St::Failed if model::is_permanent(t.error_code) => (&icons::ALERT, copy::STATE_CANT_DOWNLOAD, p.error),
        St::Failed => (&icons::ALERT, copy::STATE_FAILED, p.error),
        St::Cancelled | St::Downloading => (&icons::CANCEL, copy::STATE_CANCELED, p.muted),
    };
    c.child(row().gap(8.0).center_y().child(icon(i, 16.0).color(tone)).child(label(word, sans(13.0, 500.0), tone)))
}

fn actions(t: &DownloadTask, p: &Pal) -> El {
    use ahg_engine::types::ErrorCode as E;
    let st = t.progress.status;
    let mut a = row().w(ACTIONS).gap(4.0).justify_end().center_y();
    if matches!(st, St::Queued | St::FetchingInfo | St::Downloading) {
        a = a.child(icon_button(&icons::PAUSE, copy::ACT_PAUSE, p.text2, p));
    }
    if st == St::Paused {
        a = a.child(icon_button(&icons::PLAY, copy::ACT_RESUME, p.accent_ink, p));
    }
    if model::is_active(t) {
        a = a.child(icon_button(&icons::CANCEL, copy::ACT_CANCEL, p.text2, p));
    }
    let attention = matches!(st, St::Failed | St::Cancelled);
    if st == St::Failed && !t.progress.error.is_empty() {
        a = a.child(icon_button(&icons::COPY, copy::ACT_COPY_DETAILS, p.text2, p));
    }
    if attention && matches!(t.error_code, Some(E::Private | E::AgeRestricted | E::LoginRequired | E::DiskFull)) {
        a = a.child(icon_button(&icons::SETTINGS, copy::ACT_OPEN_SETTINGS, p.text2, p));
    }
    if attention && t.error_code == Some(E::Unknown) {
        a = a.child(icon_button(&icons::REFRESH, copy::ACT_UPDATE_ENGINE, p.text2, p));
    }
    if st == St::Done && t.progress.already_on_disk == Some(true) {
        a = a.child(icon_button(&icons::DOWNLOAD, copy::ACT_DOWNLOAD_AGAIN, p.text2, p));
    }
    if attention && !model::is_permanent(t.error_code) {
        a = a.child(icon_button(&icons::RETRY, copy::ACT_RETRY, p.error, p));
    }
    if st == St::Done {
        a = a.child(icon_button(&icons::EXTERNAL, copy::ACT_OPEN_FILE, p.text2, p)).child(icon_button(
            &icons::FOLDER,
            copy::ACT_SHOW_IN_FOLDER,
            p.text2,
            p,
        ));
    }
    if matches!(st, St::Done | St::Failed | St::Cancelled) {
        a = a.child(icon_button(&icons::REMOVE, copy::ACT_REMOVE, p.muted, p).group_opacity(0.0, 1.0));
    }
    a
}
