//! Downloads: every task as a sleeve on one surface. While a file comes in its
//! record slides out of the sleeve; once it is done the record is back inside
//! and the row reads as a file you own: its name and where it is.
use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode};
use rune::{Color, El, Icon, ScrollSpec, col, el, icon, row, text};

use super::icons;
use super::parts::{
    self, Disc, ghost, icon_button, is_active, is_live, primary, run, run_mono, runs,
};
use super::theme::{Pal, mono, sans, serif};
use crate::copy;
use crate::gate::{Nav, State};

const ROW_H: f32 = 60.0;
const SLEEVE: f32 = 48.0;

pub fn view(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let mut page = col()
        .w_full()
        .max_w(1480.0)
        .self_center()
        .h_full()
        .min_h(0.0)
        .px(40.0)
        .pt(16.0)
        .pb(24.0)
        .gap(16.0)
        .child(header(p, tasks));
    if !tasks.is_empty() {
        page = page.child(strip(p, tasks));
    }
    let interrupted = tasks
        .iter()
        .filter(|t| t.progress.status == St::Paused && t.interrupted == Some(true))
        .count();
    if interrupted > 0 {
        page = page.child(banner(p, interrupted));
    }
    page = page.child(
        col()
            .key("list-scroll")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(list(s, p, tasks)),
    );
    col()
        .key("downloads")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(page)
}

// ── Header ──────────────────────────────────────────────────────────────────

fn header(p: &Pal, tasks: &[DownloadTask]) -> El {
    let st = |f: &dyn Fn(St) -> bool| tasks.iter().filter(|t| f(t.progress.status)).count();
    let active = st(&is_active);
    let any_paused = st(&|s| s == St::Paused) > 0;
    let all_paused = any_paused && st(&is_live) == 0;

    let line = copy::active_and_total(active, tasks.len(), all_paused);
    let mut counts = Vec::new();
    let mut rest = line.as_str();
    // Figures in secondary ink, the words muted, ` · paused` in the warning colour.
    while !rest.is_empty() {
        let (part, color) = if rest.starts_with(copy::DL_PAUSED_SUFFIX) {
            (copy::DL_PAUSED_SUFFIX, p.warning)
        } else {
            let digits = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            if digits > 0 {
                (&rest[..digits], p.text2)
            } else {
                let stop = rest
                    .find(|c: char| c.is_ascii_digit())
                    .unwrap_or(rest.len());
                let stop = rest[..stop].find(copy::DL_PAUSED_SUFFIX).unwrap_or(stop);
                (&rest[..stop], p.muted)
            }
        };
        counts.push(run_mono(part, color));
        rest = &rest[part.len()..];
    }
    let mut left = col()
        .gap(4.0)
        .child(
            text(copy::DL_TITLE)
                .font(serif(44.0).lh(48.0))
                .color(p.text),
        )
        .child(runs(counts).font(mono(12.0, 400.0)).single_line());
    if tasks.len() >= 2 {
        let n = [
            st(&|s| {
                matches!(
                    s,
                    St::FetchingInfo | St::Downloading | St::Converting | St::Embedding
                )
            }),
            st(&|s| s == St::Retrying),
            st(&|s| s == St::Queued),
            st(&|s| s == St::Paused),
            st(&|s| s == St::Done),
            st(&|s| s == St::Failed),
            st(&|s| s == St::Cancelled),
        ];
        let parts: Vec<String> = n
            .iter()
            .zip(copy::SUMMARY_WORDS)
            .filter(|(n, _)| **n > 0)
            .map(|(n, w)| format!("{n} {w}"))
            .collect();
        left = left.child(
            text(copy::downloads_summary(tasks.len(), &parts))
                .font(sans(13.0, 400.0))
                .color(p.text2),
        );
    }

    let mut right = row()
        .grow(1.0)
        .min_w(0.0)
        .gap(8.0)
        .center_y()
        .justify_end()
        .flex_wrap();
    if tasks
        .iter()
        .any(|t| t.progress.status == St::Failed && !parts::is_permanent(t))
    {
        right = right.child(ghost(
            p,
            "retry-all",
            Some(&icons::RETRY),
            copy::DL_RETRY_ALL_FAILED,
            p.text,
        ));
    }
    if tasks.iter().any(|t| t.progress.status == St::Done) {
        right = right.child(ghost(
            p,
            "clear-completed",
            Some(&icons::REMOVE),
            copy::DL_CLEAR_COMPLETED,
            p.text,
        ));
    }
    if any_paused || st(&|s| matches!(s, St::Queued | St::FetchingInfo | St::Downloading)) > 0 {
        right = right.child(if all_paused {
            ghost(
                p,
                "resume-all",
                Some(&icons::PLAY),
                copy::DL_RESUME_ALL,
                p.accent,
            )
        } else {
            ghost(
                p,
                "pause-all",
                Some(&icons::PAUSE),
                copy::DL_PAUSE_ALL,
                p.text,
            )
        });
    }
    if active > 0 {
        right = right.child(ghost(
            p,
            "cancel-all",
            Some(&icons::CANCEL),
            copy::DL_CANCEL_ALL,
            p.text,
        ));
    }
    row()
        .w_full()
        .gap(24.0)
        .items_end()
        .child(left)
        .child(right)
}

// ── Control strip ───────────────────────────────────────────────────────────

fn checkbox(p: &Pal, key: String, label: &str) -> El {
    el().key(key)
        .size(16.0, 16.0)
        .radius(4.0)
        .border(1.0, p.control)
        .hover(p.raised)
        .focusable(true)
        .role("checkbox")
        .label(label)
}

fn strip(p: &Pal, tasks: &[DownloadTask]) -> El {
    let n = |f: &dyn Fn(St) -> bool| tasks.iter().filter(|t| f(t.progress.status)).count();
    let segments = [
        ("filter-all", copy::DL_FILTER_ALL, tasks.len()),
        ("filter-active", copy::DL_FILTER_ACTIVE, n(&is_active)),
        ("filter-done", copy::DL_FILTER_DONE, n(&|s| s == St::Done)),
        (
            "filter-failed",
            copy::DL_FILTER_FAILED,
            n(&|s| matches!(s, St::Failed | St::Cancelled)),
        ),
    ];
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
    for (i, (key, label, count)) in segments.into_iter().enumerate() {
        let on = i == 0;
        let mut seg = row()
            .key(key)
            .h(28.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(6.0)
            .focusable(true)
            .role("radio")
            .child(
                text(label)
                    .font(sans(13.0, if on { 500.0 } else { 400.0 }))
                    .color(if on { p.text } else { p.text2 }),
            )
            .child(
                text(count.to_string())
                    .font(mono(12.0, 400.0))
                    .color(p.muted),
            );
        seg = if on {
            seg.fill(p.raised).border(1.0, p.line_soft)
        } else {
            seg.hover(p.raised.alpha(0.6))
        };
        group = group.child(seg);
    }
    let search = row()
        .key("dl-search")
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
        .child(
            text(copy::DL_SEARCH_PLACEHOLDER)
                .font(sans(13.0, 400.0))
                .color(p.muted)
                .single_line()
                .truncate(),
        );
    row()
        .w_full()
        .h(36.0)
        .gap(16.0)
        .center_y()
        .pl(28.0)
        .child(checkbox(p, "select-all".into(), copy::DL_SELECT_ALL_ARIA).ml(8.0))
        .child(group)
        .child(el().grow(1.0))
        .child(search)
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
                parts::Run {
                    text: n.to_string(),
                    color: p.text,
                    mono: true,
                    weight: Some(500.0),
                },
                run(copy::interrupted_sentence(n), p.text),
            ])
            .font(sans(13.0, 400.0))
            .single_line(),
        )
        .child(el().grow(1.0))
        .child(primary(
            p,
            "resume-interrupted",
            Some(&icons::PLAY),
            copy::DL_INTERRUPTED_RESUME,
        ))
        .child(ghost(
            p,
            "discard-interrupted",
            Some(&icons::REMOVE),
            copy::DL_INTERRUPTED_DISCARD,
            p.text,
        ))
}

// ── The list ────────────────────────────────────────────────────────────────

fn list(s: &State, p: &Pal, tasks: &[DownloadTask]) -> El {
    let surface = col()
        .key("list")
        .w_full()
        .radius(14.0)
        .fill(p.card)
        .border(1.0, p.line_soft)
        .clip_rounded(14.0);
    if tasks.is_empty() {
        return surface.child(empty_row(p));
    }
    let live: Vec<&DownloadTask> = tasks
        .iter()
        .filter(|t| is_live(t.progress.status))
        .collect();
    let paused: Vec<&DownloadTask> = tasks
        .iter()
        .filter(|t| t.progress.status == St::Paused)
        .collect();
    let queued: Vec<&DownloadTask> = tasks
        .iter()
        .filter(|t| t.progress.status == St::Queued)
        .collect();
    let attention: Vec<&DownloadTask> = tasks
        .iter()
        .filter(|t| matches!(t.progress.status, St::Failed | St::Cancelled))
        .collect();
    let done: Vec<&DownloadTask> = tasks
        .iter()
        .filter(|t| t.progress.status == St::Done)
        .collect();
    let in_progress = live.len() + paused.len() + queued.len();
    let groups = [in_progress, attention.len(), done.len()]
        .iter()
        .filter(|n| **n > 0)
        .count();
    let all_paused = !paused.is_empty() && live.is_empty();

    let mut out = surface;
    let mut first = true;
    let mut add = |out: El, row: El| {
        let r = if first {
            row
        } else {
            col()
                .w_full()
                .child(el().w_full().h(1.0).fill(p.line_soft))
                .child(row)
        };
        first = false;
        out.child(r)
    };
    if in_progress > 0 {
        if groups > 1 {
            let speed: f64 = tasks.iter().map(|t| t.progress.speed).sum();
            let total = if live.is_empty() {
                None
            } else {
                let sp = copy::format_speed(speed);
                let line = if sp.is_empty() {
                    live.len().to_string()
                } else {
                    format!("{} · {sp}", live.len())
                };
                Some(
                    row()
                        .gap(8.0)
                        .center_y()
                        .label(copy::transferring_aria(live.len(), &sp))
                        .child(icon(&icons::DOWNLOAD, 16.0).color(p.muted))
                        .child(text(line).font(mono(12.0, 400.0)).color(p.text2)),
                )
            };
            out = add(
                out,
                group_header(
                    p,
                    "group-progress",
                    copy::GROUP_IN_PROGRESS,
                    in_progress,
                    all_paused,
                    total,
                ),
            );
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
                    .child(
                        text(copy::up_next(queued.len()))
                            .font(mono(11.0, 400.0))
                            .color(p.muted),
                    ),
            );
        }
        let reorderable = queued.len() >= 2;
        for (i, t) in queued.iter().enumerate() {
            out = add(out, task_row(s, p, t, reorderable, reorderable && i > 0));
        }
    }
    if !attention.is_empty() {
        if groups > 1 {
            out = add(
                out,
                group_header(
                    p,
                    "group-attention",
                    copy::GROUP_NEEDS_ATTENTION,
                    attention.len(),
                    false,
                    None,
                ),
            );
        }
        for t in &attention {
            out = add(out, task_row(s, p, t, false, false));
        }
    }
    if !done.is_empty() {
        if groups > 1 {
            out = add(
                out,
                group_header(
                    p,
                    "group-done",
                    copy::GROUP_COMPLETED,
                    done.len(),
                    false,
                    None,
                ),
            );
        }
        for t in &done {
            out = add(out, task_row(s, p, t, false, false));
        }
    }
    out
}

fn group_header(
    p: &Pal,
    key: &str,
    name: &str,
    count: usize,
    paused: bool,
    total: Option<El>,
) -> El {
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
        .child(
            text(count.to_string())
                .font(mono(12.0, 400.0))
                .color(p.muted),
        );
    if let Some(t) = total {
        r = r.child(el().grow(1.0)).child(t);
    }
    r
}

fn empty_row(p: &Pal) -> El {
    let blank = el()
        .size(SLEEVE, SLEEVE)
        .radius(4.0)
        .border(1.0, p.control.alpha(0.6))
        .border_dash(4.0, 4.0)
        .center()
        .child(icon(&icons::DOWNLOAD, 16.0).color(p.muted));
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
                .child(
                    text(copy::EMPTY_NONE_TITLE)
                        .font(sans(15.0, 500.0))
                        .color(p.text),
                )
                .child(
                    text(copy::EMPTY_NONE_BODY)
                        .font(sans(13.0, 400.0))
                        .color(p.text2),
                ),
        )
        .child(
            ghost(p, "paste-a-link", None, copy::EMPTY_NONE_ACTION, p.text)
                .on_press(|s: &mut State| s.nav = Nav::Home),
        )
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
        St::Done if t.progress.already_on_disk == Some(true) => {
            (&icons::CHECK, copy::STATE_SKIPPED, p.success)
        }
        St::Done => (&icons::CHECK, copy::STATE_DONE, p.success),
        St::Failed if parts::is_permanent(t) => (&icons::ALERT, copy::STATE_CANT_DOWNLOAD, p.error),
        St::Failed => (&icons::ALERT, copy::STATE_FAILED, p.error),
        St::Cancelled => (&icons::CANCEL, copy::STATE_CANCELED, p.muted),
        St::Downloading => (&icons::DOWNLOAD, "", p.text),
    }
}

fn disc(t: &DownloadTask) -> Disc {
    let pct = (t.progress.percent / 100.0).clamp(0.0, 1.0) as f32;
    match t.progress.status {
        // The record waits beside its sleeve and slides in as the file arrives; done, it is filed.
        St::Queued | St::FetchingInfo => Disc::Out(1.0),
        St::Downloading | St::Paused | St::Retrying => Disc::Out(1.0 - pct),
        St::Converting | St::Embedding => Disc::Out(0.12),
        St::Done => Disc::Out(0.0),
        St::Failed | St::Cancelled => Disc::None,
    }
}

fn task_row(s: &State, p: &Pal, t: &DownloadTask, grip: bool, do_next: bool) -> El {
    let status = t.progress.status;
    let id = &t.task_id;
    let label = parts::label(t);
    let failed = status == St::Failed;
    let finished = matches!(status, St::Done | St::Cancelled);
    let reason = parts::reason(t);

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
    let slot = el()
        .w(84.0)
        .h(SLEEVE)
        .child(parts::sleeve(
            s,
            p,
            t,
            SLEEVE,
            disc(t),
            matches!(status, St::Failed | St::Cancelled),
        ));

    // 4. Title and detail.
    let show_link = failed;
    let mut title = vec![run(label.clone(), if finished { p.text2 } else { p.text })];
    if show_link && !t.title.is_empty() {
        title.push(run(copy::row_dot_part(&parts::link_label(&t.url)), p.muted));
    }
    let title_el = runs(title)
        .font(sans(13.0, if finished { 400.0 } else { 500.0 }))
        .single_line()
        .truncate()
        .help(if show_link {
            t.url.clone()
        } else {
            label.clone()
        });

    let source = parts::source_name(t.source);
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
        let delay = [5, 15, 45][k.min(2)];
        detail.push(run(copy::row_retrying(delay, k), p.warning));
    } else {
        if !t.uploader.is_empty() {
            detail.push(run(copy::row_dot_part(&t.uploader), p.text2));
        }
        let specs = parts::specs(t);
        if !specs.is_empty() {
            detail.push(run_mono(copy::row_dot_part(&specs), p.muted));
        }
    }
    let mut detail_el = runs(detail).font(sans(12.0, 400.0));
    detail_el = if wraps {
        detail_el.max_lines(2).wrap()
    } else {
        detail_el.single_line().truncate()
    };
    if (failed || status == St::Cancelled) && !t.progress.error.is_empty() {
        detail_el = detail_el.help(t.progress.error.clone());
    }
    let mut block = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .child(title_el)
        .child(detail_el);
    if done && !t.progress.filename.is_empty() {
        // The file you now own: its name, then the folder it sits in.
        let file = parts::file_name(&t.progress.filename).to_string();
        let folder = t
            .progress
            .filename
            .strip_suffix(&file)
            .unwrap_or_default()
            .trim_end_matches(['\\', '/'])
            .to_string();
        block = block.child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .help(t.progress.filename.clone())
                .child(icon(&icons::FOLDER, 16.0).color(p.muted))
                .child(
                    text(file)
                        .font(sans(12.0, 500.0))
                        .color(p.text)
                        .single_line()
                        .truncate()
                        .shrink(),
                )
                .child(
                    text(folder)
                        .font(sans(12.0, 400.0))
                        .color(p.muted)
                        .single_line()
                        .truncate()
                        .shrink(),
                ),
        );
    }

    // 5. State column.
    let state = if status == St::Downloading {
        let pct = (t.progress.percent.clamp(0.0, 100.0)).round() as u64;
        col()
            .w(128.0)
            .gap(0.0)
            .items_end()
            .child(
                text(format!("{pct}%"))
                    .font(mono(15.0, 500.0))
                    .color(p.text),
            )
            .child(
                text(copy::rate_line(t.progress.speed, t.progress.eta))
                    .font(mono(11.0, 400.0))
                    .color(p.muted)
                    .single_line(),
            )
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
    let mut act = |glyph: &'static Icon, name: &'static str, ink: Color| {
        actions = std::mem::replace(&mut actions, row()).child(icon_button(
            p,
            format!("{id}-{name}"),
            glyph,
            name,
            ink,
        ));
    };
    if do_next {
        act(&icons::TO_FRONT, copy::ACT_DO_THIS_NEXT, p.text2);
    }
    if matches!(status, St::Queued | St::FetchingInfo | St::Downloading) {
        act(&icons::PAUSE, copy::ACT_PAUSE, p.text2);
    }
    if status == St::Paused {
        act(&icons::PLAY, copy::ACT_RESUME, p.accent);
    }
    if is_active(status) {
        act(&icons::CANCEL, copy::ACT_CANCEL, p.text2);
    }
    if failed && !t.progress.error.is_empty() {
        act(&icons::COPY, copy::ACT_COPY_DETAILS, p.text2);
    }
    if fixable
        && t.error_code.is_some()
        && matches!(
            kind,
            ErrorCode::Private
                | ErrorCode::AgeRestricted
                | ErrorCode::LoginRequired
                | ErrorCode::DiskFull
        )
    {
        act(&icons::SETTINGS, copy::ACT_OPEN_SETTINGS, p.text2);
    }
    if fixable && t.error_code == Some(ErrorCode::Unknown) {
        act(&icons::REFRESH, copy::ACT_UPDATE_ENGINE, p.text2);
    }
    if done && t.progress.already_on_disk == Some(true) {
        act(&icons::DOWNLOAD, copy::ACT_DOWNLOAD_AGAIN, p.text2);
    }
    if fixable && !parts::is_permanent(t) {
        act(&icons::RETRY, copy::ACT_RETRY, p.error);
    }
    if done {
        act(&icons::EXTERNAL, copy::ACT_OPEN_FILE, p.text2);
        act(&icons::FOLDER, copy::ACT_SHOW_IN_FOLDER, p.text2);
    }
    if matches!(status, St::Done | St::Failed | St::Cancelled) {
        actions = actions.child(
            icon_button(
                p,
                format!("{id}-remove"),
                &icons::REMOVE,
                copy::ACT_REMOVE,
                p.muted,
            )
            .group_opacity(0.0, 1.0),
        );
    }

    let mut r = row()
        .key(format!("row-{id}"))
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
        .child(checkbox(p, format!("{id}-select"), copy::ROW_SELECT_ARIA))
        .child(slot)
        .child(block)
        .child(state)
        .child(actions);
    if failed {
        r = r.fill(p.error.alpha(0.05)).hover(p.error.alpha(0.08));
    }
    // 7. The progress trace along the bottom edge, for rows in flight.
    if matches!(
        status,
        St::FetchingInfo
            | St::Downloading
            | St::Converting
            | St::Embedding
            | St::Retrying
            | St::Paused
    ) {
        let pct = (t.progress.percent / 100.0).clamp(0.0, 1.0) as f32;
        let track = el()
            .h(2.0)
            .pin(Some(0.0), None, Some(0.0), Some(0.0))
            .label(copy::row_progress_aria(&t.title));
        let trace = match status {
            St::Downloading => row()
                .w_full()
                .h_full()
                .child(el().w_frac(pct).h_full().fill(p.accent)),
            St::Paused => row()
                .w_full()
                .h_full()
                .child(el().w_frac(pct).h_full().fill(p.warning.alpha(0.7))),
            // No meaningful percentage: a short travelling segment, drawn still.
            _ => row()
                .w_full()
                .h_full()
                .child(el().w_frac(0.38))
                .child(el().w_frac(0.24).h_full().fill(p.accent.alpha(0.55))),
        };
        r = r.child(track.child(trace));
    }
    r
}
