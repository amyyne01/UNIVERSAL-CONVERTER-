//! The stream under the bar: what is in flight sits in one recessed well,
//! like the channels of a desk; what needs a hand and what is finished run
//! under it as plain lines. Each section's header carries its own action.
use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode as E};
use rune::{Align, Color, El, Icon, ScrollSpec, col, el, icon, row, spacer};

use crate::copy;
use crate::directions::signal::icons;
use crate::directions::signal::model::{self, Group};
use crate::directions::signal::parts::{Line, art, ghost, icon_button, label, meter_held, mono, sans, stack};
use crate::directions::signal::theme::{Pal, source_color};
use crate::gate::State;

const COUNTER: f32 = 72.0;
const METER: f32 = 168.0;
const STATE: f32 = 112.0;
const ACTIONS: f32 = 64.0;
const OWNED: f32 = 272.0;

pub fn view(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let mut out = col().w_full().pb(32.0).gap(24.0);
    if tasks.is_empty() {
        out = out.child(nothing_yet(p));
    }
    for g in [Group::InProgress, Group::Attention, Group::Completed] {
        let mut rows: Vec<&DownloadTask> = tasks.iter().filter(|t| model::group_of(t) == g).collect();
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
        out = out.child(section(s, g, &rows, p));
    }
    col().w_full().scroll(ScrollSpec { stick_bottom: false }).child(out)
}

fn nothing_yet(p: &Pal) -> El {
    row().w_full().h(56.0).px(16.0).gap(12.0).center_y().radius(8.0).border(1.0, p.line).child(icon(&icons::DOWNLOAD, 16.0).color(p.muted)).child(label(copy::EMPTY_NONE_TITLE, sans(13.0, 400.0), p.text2))
}

fn section(s: &State, g: Group, rows: &[&DownloadTask], p: &Pal) -> El {
    let tasks = &s.tasks;
    let name = match g {
        Group::InProgress => copy::GROUP_IN_PROGRESS,
        Group::Attention => copy::GROUP_NEEDS_ATTENTION,
        Group::Completed => copy::GROUP_COMPLETED,
    };
    let mut title = Line::new().strong(name, p.text2);
    if g == Group::InProgress && model::all_paused(tasks) {
        title = title.add(copy::DL_PAUSED_SUFFIX, p.warning);
    }
    let mut head = row().w_full().h(32.0).px(4.0).gap(8.0).center_y().child(title.one(sans(13.0, 400.0))).child(label(rows.len().to_string(), mono(12.0, 400.0), p.muted)).child(spacer());
    match g {
        Group::InProgress => {
            if model::all_paused(tasks) {
                head = head.child(ghost(Some(&icons::PLAY), copy::DL_RESUME_ALL, p.accent_ink, p));
            } else {
                head = head.child(ghost(Some(&icons::PAUSE), copy::DL_PAUSE_ALL, p.text2, p));
            }
            head = head.child(ghost(Some(&icons::CANCEL), copy::DL_CANCEL_ALL, p.text2, p));
        }
        Group::Attention => {
            if rows.iter().any(|t| t.progress.status == St::Failed && !model::is_permanent(t.error_code)) {
                head = head.child(ghost(Some(&icons::RETRY), copy::DL_RETRY_ALL_FAILED, p.text2, p));
            }
        }
        Group::Completed => head = head.child(ghost(Some(&icons::REMOVE), copy::DL_CLEAR_COMPLETED, p.text2, p)),
    }
    let body = match g {
        Group::InProgress => well(s, rows, p),
        _ => {
            let mut list = col().w_full();
            for (i, t) in rows.iter().enumerate() {
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
                w = w.child(row().w_full().h(32.0).px(16.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], p.line).child(label(copy::up_next(queued), mono(12.0, 400.0), p.muted)));
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
        .pl(16.0)
        .pr(8.0)
        .gap(12.0)
        .center_y()
        .fill(p.warning_soft)
        .child(icon(&icons::PAUSE, 16.0).color(p.warning))
        .child(Line::new().fig(&n.to_string(), p.text).add(&copy::interrupted_sentence(n), p.text).one(sans(13.0, 400.0)).grow(1.0))
        .child(ghost(Some(&icons::PLAY), copy::DL_INTERRUPTED_RESUME, p.bg, p).fill(p.text).hover(p.bg.alpha(0.12)))
        .child(ghost(Some(&icons::REMOVE), copy::DL_INTERRUPTED_DISCARD, p.text2, p))
}

/// A channel: what it is, its tape counter, its meter, its reading.
fn live_row(s: &State, t: &DownloadTask, p: &Pal) -> El {
    let pr = &t.progress;
    let st = pr.status;
    let pct = pr.percent.clamp(0.0, 100.0) / 100.0;
    // (how far the meter is lit, its colour, the counter's ink)
    let (frac, lit, ink) = match st {
        St::Downloading => (pct, p.accent, p.text),
        St::Converting | St::Embedding => (1.0, p.text2, p.text2),
        St::Paused | St::Retrying => (pct, p.muted, p.text2),
        _ => (0.0, p.muted, p.muted),
    };
    let held = matches!(st, St::Paused | St::Retrying);
    let counter = label(if t.duration > 0.0 { model::timecode(t.duration * frac) } else { String::new() }, mono(13.0, 500.0), ink).w(COUNTER);
    row()
        .key(t.task_id.as_str())
        .w_full()
        .h(56.0)
        .px(12.0)
        .gap(12.0)
        .center_y()
        .hover(p.hover)
        .child(art(s, t, Some(32.0), 32.0, 4.0, p))
        .child(title_block(t, true, p))
        .child(counter)
        .child(el().w(METER).child(meter_held(frac, 28, None, 8.0, 2.0, lit, p.unlit, held)))
        .child(reading(t, p))
        .child(live_actions(t, p))
}

fn title_block(t: &DownloadTask, strong: bool, p: &Pal) -> El {
    let st = t.progress.status;
    let name = model::label(t);
    let title = if strong { Line::new().strong(&name, p.text) } else { Line::new().add(&name, p.text) };
    let mut detail = Line::new().add(model::source_name(t.source), source_color(p, t.source));
    if st == St::Retrying {
        let k = t.retry_count.unwrap_or(0.0).max(0.0) as usize;
        detail = detail.add(&copy::row_retrying([5, 15, 45][k.min(2)], k), p.warning);
    } else {
        if !t.uploader.is_empty() {
            detail = detail.add(&copy::row_dot_part(&t.uploader), p.text2);
        }
        let pr = &t.progress;
        let bytes = if pr.total > 0.0 { pr.total } else { pr.downloaded };
        for part in [copy::format_duration(t.duration), if bytes > 0.0 { copy::format_bytes(bytes) } else { String::new() }] {
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
        let pct = format!("{}%", pr.percent.clamp(0.0, 100.0).round());
        return c.child(label(pct, mono(14.0, 500.0), p.text)).child(label(copy::rate_line(pr.speed, pr.eta), mono(12.0, 400.0), p.muted).align(Align::End));
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

fn live_actions(t: &DownloadTask, p: &Pal) -> El {
    let st = t.progress.status;
    let mut a = row().w(ACTIONS).gap(4.0).justify_end().center_y();
    if matches!(st, St::Queued | St::FetchingInfo | St::Downloading) {
        a = a.child(icon_button(&icons::PAUSE, copy::ACT_PAUSE, p.text2, p));
    }
    if st == St::Paused {
        a = a.child(icon_button(&icons::PLAY, copy::ACT_RESUME, p.accent_ink, p));
    }
    a.child(icon_button(&icons::CANCEL, copy::ACT_CANCEL, p.text2, p))
}

/// A row that needs a hand: the reason in full, and what to do about it.
fn trouble_row(s: &State, t: &DownloadTask, p: &Pal) -> El {
    let st = t.progress.status;
    let failed = st == St::Failed;
    let name = model::label(t);
    let mut title = Line::new().add(&name, if failed { p.text } else { p.text2 });
    if failed && !t.title.trim().is_empty() {
        title = title.add(&copy::row_dot_part(&model::link_label(&t.url)), p.muted);
    }
    let mut detail = Line::new().add(model::source_name(t.source), source_color(p, t.source));
    if let Some(reason) = model::reason(t) {
        detail = detail.add(&copy::row_dot_part(&reason), p.text2);
    } else if !t.uploader.is_empty() {
        detail = detail.add(&copy::row_dot_part(&t.uploader), p.text2);
    }
    let text = stack(4.0).grow(1.0).child(title.one(sans(14.0, 400.0)).help(t.url.clone())).child(detail.wrap(sans(13.0, 400.0).lh(20.0)).max_lines(2).min_w(0.0));
    let (i, word, tone): (&'static Icon, &str, Color) = match st {
        St::Failed if model::is_permanent(t.error_code) => (&icons::ALERT, copy::STATE_CANT_DOWNLOAD, p.error),
        St::Failed => (&icons::ALERT, copy::STATE_FAILED, p.error),
        _ => (&icons::CANCEL, copy::STATE_CANCELED, p.muted),
    };
    let mut a = row().w(ACTIONS + 32.0).gap(4.0).justify_end().center_y();
    if failed && !t.progress.error.is_empty() {
        a = a.child(icon_button(&icons::COPY, copy::ACT_COPY_DETAILS, p.text2, p));
    }
    if matches!(t.error_code, Some(E::Private | E::AgeRestricted | E::LoginRequired | E::DiskFull)) {
        // Opens the sheet it names, out of the bar.
        a = a.child(icon_button(&icons::SETTINGS, copy::ACT_OPEN_SETTINGS, p.text2, p));
    }
    if t.error_code == Some(E::Unknown) && failed {
        a = a.child(icon_button(&icons::REFRESH, copy::ACT_UPDATE_ENGINE, p.text2, p));
    }
    if !model::is_permanent(t.error_code) || !failed {
        a = a.child(icon_button(&icons::RETRY, copy::ACT_RETRY, if failed { p.error } else { p.text2 }, p));
    }
    // A failed line carries one lit segment in the error colour at its edge.
    let edge = el().w(2.0).h(32.0).radius(1.0).fill(if failed { p.error } else { p.line_strong });
    row()
        .key(t.task_id.as_str())
        .w_full()
        .py(12.0)
        .pr(12.0)
        .gap(12.0)
        .items_center()
        .hover_group()
        .hover(p.hover)
        .child(edge)
        .child(art(s, t, Some(32.0), 32.0, 4.0, p))
        .child(text)
        .child(col().w(STATE).items_end().child(state_word(i, word, tone)))
        .child(a)
}

/// A finished line: what it is, then the file you now own.
fn owned_row(s: &State, t: &DownloadTask, p: &Pal) -> El {
    let pr = &t.progress;
    let skipped = pr.already_on_disk == Some(true);
    let file = model::file_name(&pr.filename);
    let tag = el().h(20.0).px(4.0).center().radius(4.0).border(1.0, p.line_strong).child(label(t.format.to_uppercase(), mono(12.0, 400.0), p.text2));
    let owned = row()
        .w(OWNED)
        .gap(8.0)
        .center_y()
        .child(tag)
        .child(stack(4.0).grow(1.0).child(label(file, mono(12.0, 500.0), p.text).single_line().truncate_middle().min_w(0.0)).child(label(model::folder(&pr.filename), mono(12.0, 400.0), p.muted).single_line().truncate_middle().min_w(0.0)))
        .help(pr.filename.clone());
    let (word, tone) = if skipped { (copy::STATE_SKIPPED, p.success) } else { (copy::STATE_DONE, p.success) };
    let mut a = row().w(ACTIONS + 32.0).gap(4.0).justify_end().center_y();
    if skipped {
        a = a.child(icon_button(&icons::DOWNLOAD, copy::ACT_DOWNLOAD_AGAIN, p.text2, p));
    }
    a = a
        .child(icon_button(&icons::EXTERNAL, copy::ACT_OPEN_FILE, p.text2, p))
        .child(icon_button(&icons::FOLDER, copy::ACT_SHOW_IN_FOLDER, p.text2, p))
        .child(icon_button(&icons::REMOVE, copy::ACT_REMOVE, p.muted, p).group_opacity(0.0, 1.0));
    row()
        .key(t.task_id.as_str())
        .w_full()
        .h(56.0)
        .pl(14.0)
        .pr(12.0)
        .gap(12.0)
        .center_y()
        .hover_group()
        .hover(p.hover)
        .child(art(s, t, Some(32.0), 32.0, 4.0, p))
        .child(title_block(t, false, p))
        .child(owned)
        .child(col().w(STATE).items_end().child(state_word(&icons::CHECK, word, tone)))
        .child(a)
}
