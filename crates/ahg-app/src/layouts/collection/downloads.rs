//! Downloads, the second place: every download as a dense list, with the
//! filters, the search and the bulk actions. The sleeve shrinks to a thumbnail
//! with its record beside it, so the list still reads as the same collection.
use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode};
use rune::{Color, El, Icon, ScrollSpec, col, el, icon, row, text};

use super::{PAD, disc, icon_button, interrupted_line, quiet, tab};
use crate::copy;
use crate::directions::shelf::icons;
use crate::directions::shelf::parts::{self, is_active, is_live, run, run_mono, runs};
use crate::directions::shelf::theme::{Pal, mono, sans, serif};
use crate::gate::State;

const THUMB: f32 = 40.0;

pub fn view(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let count = |f: &dyn Fn(St) -> bool| tasks.iter().filter(|t| f(t.progress.status)).count();
    let active = count(&is_active);
    let done = count(&|st| st == St::Done);
    let failed = count(&|st| matches!(st, St::Failed | St::Cancelled));
    let mut page = col().w_full().px(PAD).pt(16.0).pb(48.0).child(header(p, tasks, active));
    if tasks.is_empty() {
        return shell(page.child(empty(p)));
    }
    let search = row()
        .key("dl-find")
        .w(280.0)
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .border(1.0, p.line)
        .hover(p.raised)
        .focusable(true)
        .role("searchbox")
        .label(copy::DL_SEARCH_ARIA)
        .child(icon(&icons::SEARCH, 16.0).color(p.muted))
        .child(text(copy::DL_SEARCH_PLACEHOLDER).font(sans(13.0, 400.0)).color(p.muted).single_line().truncate().shrink());
    page = page.child(
        row()
            .w_full()
            .mb(16.0)
            .gap(16.0)
            .center_y()
            .border_sides([0.0, 0.0, 1.0, 0.0], p.line_soft)
            .child(
                row()
                    .gap(16.0)
                    .role("tablist")
                    .label(copy::DL_FILTER_ARIA)
                    .child(tab(p, "d-all", copy::DL_FILTER_ALL, Some(tasks.len()), None, 0, true))
                    .child(tab(p, "d-active", copy::DL_FILTER_ACTIVE, Some(active), None, 0, false))
                    .child(tab(p, "d-done", copy::DL_FILTER_DONE, Some(done), None, 0, false))
                    .child(tab(p, "d-failed", copy::DL_FILTER_FAILED, Some(failed), None, 0, false)),
            )
            .child(el().grow(1.0))
            .child(search.mb(4.0)),
    );
    let interrupted = tasks.iter().filter(|t| t.progress.status == St::Paused && t.interrupted == Some(true)).count();
    if interrupted > 0 {
        page = page.child(
            row().w_full().h(48.0).px(12.0).mb(16.0).center_y().radius(8.0).fill(p.soft(p.warning)).child(interrupted_line(p, interrupted)),
        );
    }
    let running: Vec<&DownloadTask> = tasks.iter().filter(|t| is_active(t.progress.status) && t.progress.status != St::Queued).collect();
    let waiting: Vec<&DownloadTask> = tasks.iter().filter(|t| t.progress.status == St::Queued).collect();
    let trouble: Vec<&DownloadTask> = tasks.iter().filter(|t| matches!(t.progress.status, St::Failed | St::Cancelled)).collect();
    let finished: Vec<&DownloadTask> = tasks.iter().filter(|t| t.progress.status == St::Done).collect();
    if !running.is_empty() {
        let live = running.iter().filter(|t| is_live(t.progress.status)).count();
        let speed: f64 = running.iter().filter(|t| t.progress.status == St::Downloading).map(|t| t.progress.speed).sum();
        let aside = row()
            .gap(8.0)
            .center_y()
            .label(copy::transferring_aria(live, &copy::format_speed(speed)))
            .child(icon(&icons::DOWNLOAD, 16.0).color(p.muted))
            .child(text(format!("{live} · {}", copy::format_speed(speed))).font(mono(12.0, 400.0)).color(p.text2));
        page = page.child(group(s, p, copy::GROUP_IN_PROGRESS, &running, Some(aside)));
    }
    if !waiting.is_empty() {
        page = page.child(group(s, p, &copy::up_next(waiting.len()), &waiting, None));
    }
    if !trouble.is_empty() {
        page = page.child(group(s, p, copy::GROUP_NEEDS_ATTENTION, &trouble, None));
    }
    if !finished.is_empty() {
        page = page.child(group(s, p, copy::GROUP_COMPLETED, &finished, None));
    }
    shell(page)
}

fn shell(page: El) -> El {
    col().key("downloads-place").grow(1.0).h_full().min_w(0.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(page)
}

fn header(p: &Pal, tasks: &[DownloadTask], active: usize) -> El {
    let all_paused = active > 0 && tasks.iter().filter(|t| is_active(t.progress.status)).all(|t| t.progress.status == St::Paused);
    let n = |st: &[St]| tasks.iter().filter(|t| st.contains(&t.progress.status)).count();
    let counts = [
        n(&[St::Downloading, St::FetchingInfo, St::Converting, St::Embedding]),
        n(&[St::Retrying]),
        n(&[St::Queued]),
        n(&[St::Paused]),
        n(&[St::Done]),
        n(&[St::Failed]),
        n(&[St::Cancelled]),
    ];
    let parts: Vec<String> = counts.iter().zip(copy::SUMMARY_WORDS).filter(|(c, _)| **c > 0).map(|(c, w)| format!("{c} {w}")).collect();
    let mut words = col().gap(4.0).child(
        row()
            .gap(16.0)
            .items_end()
            .child(text(copy::DL_TITLE).font(serif(44.0).lh(48.0)).color(p.text))
            .child(text(copy::active_and_total(active, tasks.len(), all_paused)).font(mono(13.0, 400.0)).color(p.text2).mb(8.0)),
    );
    if !tasks.is_empty() {
        words = words.child(text(copy::downloads_summary(tasks.len(), &parts)).font(sans(13.0, 400.0)).color(p.text2).single_line());
    }
    let mut actions = row().gap(4.0).center_y();
    if !tasks.is_empty() {
        actions = actions
            .child(quiet(p, "d-retry-all", Some(&icons::RETRY), copy::DL_RETRY_ALL_FAILED, p.text2))
            .child(quiet(p, "d-clear", Some(&icons::REMOVE), copy::DL_CLEAR_COMPLETED, p.text2))
            .child(quiet(p, "d-pause-all", Some(&icons::PAUSE), copy::DL_PAUSE_ALL, p.text2))
            .child(quiet(p, "d-cancel-all", Some(&icons::CANCEL), copy::DL_CANCEL_ALL, p.text2));
    }
    row().w_full().pb(16.0).items_end().justify_between().gap(24.0).child(words).child(actions)
}

fn empty(p: &Pal) -> El {
    col()
        .w_full()
        .py(48.0)
        .gap(8.0)
        .items_center()
        .child(text(copy::EMPTY_NONE_TITLE).font(sans(16.0, 500.0)).color(p.text))
        .child(text(copy::EMPTY_NONE_BODY).font(sans(13.0, 400.0)).color(p.text2))
}

fn group(s: &State, p: &Pal, title: &str, tasks: &[&DownloadTask], aside: Option<El>) -> El {
    let mut head = row()
        .w_full()
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .child(match title.split_once(" · ") {
            // "Up next · 1" carries its own count; the dot is set in the figures face.
            Some((a, n)) => runs(vec![run(a, p.text2), run_mono(" · ", p.muted), run_mono(n, p.muted)]).font(sans(13.0, 500.0)),
            None => runs(vec![run(title, p.text2), run_mono(format!("  {}", tasks.len()), p.muted)]).font(sans(13.0, 500.0)),
        })
        .child(el().grow(1.0));
    if let Some(a) = aside {
        head = head.child(a);
    }
    let mut g = col().w_full().mb(16.0).child(head);
    for t in tasks {
        g = g.child(task_row(s, p, t));
    }
    g
}

/// The state column: the word for what is happening, in its tone.
fn state_word(p: &Pal, t: &DownloadTask) -> (&'static Icon, &'static str, Color) {
    match t.progress.status {
        St::Queued => (&icons::CLOCK, copy::STATE_QUEUED, p.muted),
        St::FetchingInfo => (&icons::SPINNER, copy::STATE_STARTING, p.text2),
        St::Converting => (&icons::SPINNER, copy::STATE_CONVERTING, p.warning),
        St::Embedding => (&icons::SPINNER, copy::STATE_TAGGING, p.warning),
        St::Paused => (&icons::PAUSE, copy::STATE_PAUSED, p.warning),
        St::Retrying => (&icons::RETRY, copy::STATE_RETRYING, p.warning),
        St::Done if t.progress.already_on_disk == Some(true) => (&icons::FOLDER, copy::STATE_SKIPPED, p.success),
        St::Done => (&icons::FOLDER, copy::STATE_DONE, p.success),
        St::Failed if parts::is_permanent(t) => (&icons::ALERT, copy::STATE_CANT_DOWNLOAD, p.error),
        St::Failed => (&icons::ALERT, copy::STATE_FAILED, p.error),
        St::Cancelled => (&icons::CANCEL, copy::STATE_CANCELED, p.muted),
        St::Downloading => (&icons::DOWNLOAD, "", p.text),
    }
}

/// The row's actions, as Electron chose them per state (CONTENT.md 3.5).
fn actions(t: &DownloadTask) -> Vec<(&'static str, &'static Icon, &'static str)> {
    use ErrorCode::*;
    match t.progress.status {
        St::Queued => vec![
            ("next", &icons::TO_FRONT, copy::ACT_DO_THIS_NEXT),
            ("pause", &icons::PAUSE, copy::ACT_PAUSE),
            ("cancel", &icons::CANCEL, copy::ACT_CANCEL),
        ],
        St::Paused => vec![("resume", &icons::PLAY, copy::ACT_RESUME), ("cancel", &icons::CANCEL, copy::ACT_CANCEL)],
        St::Downloading | St::FetchingInfo | St::Retrying => {
            vec![("pause", &icons::PAUSE, copy::ACT_PAUSE), ("cancel", &icons::CANCEL, copy::ACT_CANCEL)]
        }
        St::Converting | St::Embedding => vec![("cancel", &icons::CANCEL, copy::ACT_CANCEL)],
        St::Done => vec![
            ("open", &icons::EXTERNAL, copy::ACT_OPEN_FILE),
            ("folder", &icons::FOLDER, copy::ACT_SHOW_IN_FOLDER),
            ("remove", &icons::REMOVE, copy::ACT_REMOVE),
        ],
        St::Failed | St::Cancelled => {
            let mut a = vec![("copy", &icons::COPY, copy::ACT_COPY_DETAILS)];
            match t.error_code {
                Some(Private | AgeRestricted | LoginRequired | DiskFull) => a.push(("settings", &icons::SETTINGS, copy::ACT_OPEN_SETTINGS)),
                Some(Unknown) => a.push(("engine", &icons::REFRESH, copy::ACT_UPDATE_ENGINE)),
                _ => {}
            }
            if !parts::is_permanent(t) {
                a.push(("retry", &icons::RETRY, copy::ACT_RETRY));
            }
            a.push(("remove", &icons::REMOVE, copy::ACT_REMOVE));
            a
        }
    }
}

fn task_row(s: &State, p: &Pal, t: &DownloadTask) -> El {
    let st = t.progress.status;
    let label = parts::label(t);
    let troubled = matches!(st, St::Failed | St::Cancelled);
    let thumb = el().w((THUMB * (1.0 + parts::DISC_TRAVEL)).round()).child(parts::sleeve(s, p, t, THUMB, disc(t), st == St::Cancelled));

    let mut detail = vec![run(parts::source_name(t.source), p.source(t.source))];
    let reason = parts::reason(t);
    if let Some(r) = &reason {
        detail.push(run(copy::row_dot_part(r), p.error));
    } else if st == St::Retrying {
        let k = t.retry_count.unwrap_or(0.0) as usize;
        detail.push(run(copy::row_retrying([5, 15, 45][k.min(2)], k), p.warning));
    } else {
        if !t.uploader.is_empty() {
            detail.push(run(copy::row_dot_part(&t.uploader), p.text2));
        }
        let specs = parts::specs(t);
        if !specs.is_empty() {
            detail.push(run_mono(copy::row_dot_part(&specs), p.muted));
        }
    }
    let detail = runs(detail).font(sans(12.0, 400.0));
    let detail = if reason.is_some() { detail.wrap().max_lines(2) } else { detail.single_line().truncate() };
    let words = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .child(
            text(label.clone())
                .font(sans(13.0, 500.0))
                .color(if matches!(st, St::Done | St::Cancelled) { p.text2 } else { p.text })
                .single_line()
                .truncate()
                .help(label.clone()),
        )
        .child(detail);

    let state = if st == St::Downloading {
        col()
            .w(160.0)
            .gap(4.0)
            .items_end()
            .child(text(format!("{}%", t.progress.percent.round())).font(mono(13.0, 500.0)).color(p.text))
            .child(text(copy::rate_line(t.progress.speed, t.progress.eta)).font(mono(12.0, 400.0)).color(p.muted))
    } else {
        let (g, w, c) = state_word(p, t);
        let mut r = row().gap(8.0).center_y().child(icon(g, 16.0).color(c)).child(text(w).font(sans(13.0, 500.0)).color(c));
        if st == St::Paused {
            r = r.child(text(format!("{}%", t.progress.percent.round())).font(mono(12.0, 400.0)).color(p.muted));
        }
        col().w(160.0).items_end().child(r)
    };
    let mut acts = row().w(124.0).gap(4.0).justify_end();
    for (key, glyph, name) in actions(t) {
        acts = acts.child(icon_button(p, format!("d-{key}-{}", t.task_id), glyph, name, if key == "retry" { p.error } else { p.text2 }));
    }
    row()
        .key(format!("row-{}", t.task_id))
        .w_full()
        .min_h(64.0)
        .px(12.0)
        .py(12.0)
        .gap(16.0)
        .center_y()
        .radius(8.0)
        .hover(p.raised)
        .focusable(true)
        .role("listitem")
        .label(copy::row_name_with_link(&label, &t.url))
        .fill(if troubled && st == St::Failed { p.error.alpha(0.05) } else { Color::TRANSPARENT })
        .child(thumb)
        .child(words)
        .child(state)
        .child(acts)
}
