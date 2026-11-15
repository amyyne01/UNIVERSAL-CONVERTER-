//! The two places beside Home. Downloads: the whole history, filtered,
//! searched and acted on in bulk. Settings: the sections on the left, the
//! chosen one on the right, the full window.
use ahg_engine::types::DownloadStatus as St;
use rune::{Color, Cursor, El, ScrollSpec, col, el, icon, row};

use super::{SECTIONS, reach, section_count, stream};
use crate::copy;
use crate::copy::settings as w;
use crate::directions::signal::icons;
use crate::directions::signal::model;
use crate::directions::signal::parts::{button, ghost, label, mono, sans};
use crate::directions::signal::theme::Pal;
use crate::gate::State;

const WIDE: f32 = 1120.0;

pub fn downloads(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let mut filters = row().padding(4.0).gap(4.0).radius(8.0).fill(p.deep).border(1.0, p.line).role("radiogroup").label(copy::DL_FILTER_ARIA);
    for (i, name) in SECTIONS.into_iter().enumerate() {
        let on = i == 0;
        let mut f = row()
            .h(28.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(6.0)
            .hover(p.hover)
            .cursor(Cursor::Pointer)
            .role("radio")
            .child(label(name, sans(13.0, if on { 500.0 } else { 400.0 }), if on { p.text } else { p.text2 }))
            .child(label(section_count(tasks, i).to_string(), mono(12.0, 400.0), p.muted));
        f = if on { f.fill(p.raised).child(el().size(12.0, 2.0).radius(1.0).fill(p.accent).absolute(12.0, 24.0)) } else { f.fill(Color::TRANSPARENT) };
        filters = filters.child(f);
    }
    let search = row()
        .grow(1.0)
        .h(36.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.surface)
        .border(1.0, p.line_strong)
        .role("searchbox")
        .label(copy::DL_SEARCH_ARIA)
        .child(icon(&icons::SEARCH, 16.0).color(p.muted))
        .child(label(copy::DL_SEARCH_PLACEHOLDER, sans(13.0, 400.0), p.muted));
    let toolbar = row().w_full().gap(16.0).center_y().child(filters).child(search);

    let mut actions = row().gap(4.0).center_y();
    if tasks.iter().any(|t| t.progress.status == St::Failed && !model::is_permanent(t.error_code)) {
        actions = actions.child(ghost(Some(&icons::RETRY), copy::DL_RETRY_ALL_FAILED, p.text2, p));
    }
    if tasks.iter().any(|t| t.progress.status == St::Done) {
        actions = actions.child(ghost(Some(&icons::REMOVE), copy::DL_CLEAR_COMPLETED, p.text2, p));
    }
    let all_paused = model::all_paused(tasks);
    if tasks.iter().any(|t| matches!(t.progress.status, St::Queued | St::FetchingInfo | St::Downloading | St::Paused)) {
        actions = actions.child(if all_paused { button(Some(&icons::PLAY), copy::DL_RESUME_ALL, p.accent_ink, p) } else { button(Some(&icons::PAUSE), copy::DL_PAUSE_ALL, p.text, p) });
    }
    let active = model::active_count(tasks);
    if active > 0 {
        actions = actions.child(ghost(Some(&icons::CANCEL), copy::DL_CANCEL_ALL, p.text2, p));
    }
    let summary = col()
        .gap(4.0)
        .min_w(0.0)
        .child(label(copy::active_and_total(active, tasks.len(), all_paused), mono(12.0, 400.0), p.muted))
        .child(label(copy::downloads_summary(tasks.len(), &model::summary_parts(tasks)), sans(13.0, 400.0), p.text2).single_line().truncate());
    let bulk = row().w_full().gap(16.0).center_y().child(summary.grow(1.0)).child(actions);

    let column = col().w_full().max_w(WIDE).h_full().min_h(0.0).pt(24.0).gap(16.0).child(toolbar).child(bulk).child(stream::view(s, p, false).grow(1.0).min_h(0.0).mt(8.0));
    col().w_full().grow(1.0).min_h(0.0).px(32.0).items_center().child(column)
}

pub fn settings(s: &State, p: &Pal) -> El {
    let left = col()
        .w(296.0)
        .h_full()
        .min_h(0.0)
        .pr(24.0)
        .gap(8.0)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line)
        .child(label(w::TITLE, sans(16.0, 500.0), p.text).ml(8.0).mb(8.0))
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(reach::section_list(s, p)))
        .child(label(reach::SAVES_ITSELF, sans(12.0, 400.0), p.muted).ml(8.0));
    // One scroll of every section; the list marks the one in view. Scheduled downloads is scrolled to.
    let sections = col()
        .w_full()
        .max_w(640.0)
        .pt(32.0)
        .pb(32.0)
        .gap(48.0)
        .child(destination(p))
        .child(appearance(p))
        .child(reach::schedule_sheet(p, false))
        .child(software(p))
        .child(plan(p));
    let right = col().grow(1.0).h_full().min_w(0.0).pl(40.0).scroll(ScrollSpec { stick_bottom: false }).child(sections);
    let column = row().w_full().max_w(WIDE).h_full().min_h(0.0).pt(32.0).pb(32.0).child(left).child(right);
    col().w_full().grow(1.0).min_h(0.0).px(32.0).items_center().child(column)
}

/// Destination had no heading in the old app: one line, the folder and `Change`.
fn destination(p: &Pal) -> El {
    row()
        .w_full()
        .gap(12.0)
        .center_y()
        .child(icon(&icons::FOLDER, 16.0).color(p.muted))
        .child(label(w::SAVING_TO, sans(13.0, 400.0), p.text2))
        .child(label(super::OUTPUT_DIR, mono(13.0, 400.0), p.text).single_line().truncate().min_w(0.0).shrink())
        .child(el().grow(1.0))
        .child(button(None, w::CHANGE, p.text, p))
}

fn appearance(p: &Pal) -> El {
    section(w::APPEARANCE, "", p).child(reach::setting(w::LAYOUT, button(None, w::LAYOUT_CHOOSE, p.text, p).on_press(|s: &mut State| s.picker = true), p))
}

fn software(p: &Pal) -> El {
    section(w::SOFTWARE, w::SOFTWARE_HINT, p)
        .child(status_row(w::APP_NAME, "v2.0.0", button(Some(&icons::REFRESH), w::CHECK_FOR_UPDATES, p.text, p), p))
        .child(status_row(w::ENGINE_NAME, "2026.11.01", ghost(None, w::ENGINE_CHECK, p.text2, p), p))
        .child(reach::setting(w::AUTO_UPDATE_ENGINE, reach::toggle(true, p), p))
}

fn plan(p: &Pal) -> El {
    section(w::PLAN, "", p).child(status_row(w::YOUR_PLAN, w::PLAN_PREMIUM.0, label(w::PLAN_PREMIUM.1, sans(13.0, 400.0), p.text2), p))
}

fn section(title: &str, hint: &str, p: &Pal) -> El {
    let mut c = col().w_full().gap(4.0).child(label(title, sans(16.0, 500.0), p.text));
    if !hint.is_empty() {
        c = c.child(label(hint, sans(13.0, 400.0), p.text2));
    }
    c.child(el().h(12.0))
}

fn status_row(name: &str, figure: &str, control: El, p: &Pal) -> El {
    let mut words = col().gap(4.0).grow(1.0).min_w(0.0).child(label(name, sans(13.0, 500.0), p.text));
    if !figure.is_empty() {
        words = words.child(label(figure, mono(12.0, 400.0), p.muted));
    }
    row().w_full().min_h(56.0).py(12.0).gap(16.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], p.line).child(words).child(control)
}
