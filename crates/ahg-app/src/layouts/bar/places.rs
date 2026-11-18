//! The two places beside Home. Downloads: the whole history, filtered,
//! searched and acted on in bulk. Settings: the sections on the left, the
//! chosen one on the right, the full window.
use ahg_engine::types::DownloadStatus as St;
use rune::{Color, Cursor, El, ScrollSpec, col, el, icon, row, text_input};

use super::{SECTIONS, reach, stream};
use crate::actions;
use crate::copy;
use crate::copy::settings as w;
use crate::directions::signal::parts::{self, button, confirm, ghost, label, mono, sans};
use crate::directions::signal::theme::Pal;
use crate::format;
use crate::gate::{Confirm, Filter, State};
use crate::icons;
use crate::keys;
use crate::settings_spec;

const WIDE: f32 = 1120.0;

pub fn downloads(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let counts = format::filter_counts(tasks);
    let mut filters =
        row().padding(4.0).gap(4.0).radius(8.0).fill(p.deep).border(1.0, p.line).role("radiogroup").label(copy::DL_FILTER_ARIA);
    for (i, name) in SECTIONS.into_iter().enumerate() {
        let on = Filter::ALL[i] == s.filter;
        let mut f = row()
            .key(keys::filter(Filter::ALL[i]))
            .on_press(move |s: &mut State| actions::set_filter(s, Filter::ALL[i]))
            .h(28.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(6.0)
            .hover(p.hover)
            .cursor(Cursor::Pointer)
            .role("radio")
            .checked(on)
            .child(label(name, sans(13.0, if on { 500.0 } else { 400.0 }), if on { p.text } else { p.text2 }))
            .child(label(counts[i].to_string(), mono(12.0, 400.0), p.muted));
        f = if on {
            f.fill(p.raised).child(el().size(12.0, 2.0).radius(1.0).fill(p.accent).absolute(12.0, 24.0))
        } else {
            f.fill(Color::TRANSPARENT)
        };
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
        );
    let toolbar = row().w_full().gap(16.0).center_y().child(filters).child(search);
    let bulk = if s.selected.is_empty() { totals(s, p) } else { selection(s, p) };

    let column = col()
        .w_full()
        .max_w(WIDE)
        .h_full()
        .min_h(0.0)
        .pt(24.0)
        .gap(16.0)
        .child(toolbar)
        .child(bulk.h(40.0))
        .child(stream::view(s, p, false).grow(1.0).min_h(0.0).mt(8.0));
    col().w_full().grow(1.0).min_h(0.0).px(32.0).items_center().child(column)
}

/// The counts and the whole list's actions, when nothing is selected.
fn totals(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let mut actions = row().gap(4.0).center_y();
    if tasks.iter().any(|t| t.progress.status == St::Failed && !format::is_permanent(t.error_code)) {
        actions = actions.child(
            ghost(Some(&icons::RETRY), copy::DL_RETRY_ALL_FAILED, p.text2, p)
                .key(keys::RETRY_ALL_FAILED)
                .on_press(actions::retry_all_failed),
        );
    }
    let done = tasks.iter().filter(|t| t.progress.status == St::Done).count();
    if s.confirm == Some(Confirm::Clear) && done > 0 {
        actions = actions.child(confirm(&copy::clear_confirm(done), copy::DL_CLEAR_CONFIRM_ARIA, p));
    } else if done > 0 {
        actions = actions.child(
            ghost(Some(&icons::REMOVE), copy::DL_CLEAR_COMPLETED, p.text2, p)
                .key(keys::arm(Confirm::Clear))
                .on_press(|s: &mut State| actions::arm(s, Confirm::Clear)),
        );
    }
    let all_paused = format::all_paused(tasks);
    if tasks.iter().any(|t| matches!(t.progress.status, St::Queued | St::FetchingInfo | St::Downloading | St::Paused)) {
        actions = actions.child(
            if all_paused {
                button(Some(&icons::PLAY), copy::DL_RESUME_ALL, p.accent_ink, p)
            } else {
                button(Some(&icons::PAUSE), copy::DL_PAUSE_ALL, p.text, p)
            }
            .key(keys::PAUSE_ALL)
            .on_press(actions::pause_or_resume_all),
        );
    }
    let active = format::active_count(tasks);
    if active > 0 {
        actions =
            actions.child(ghost(Some(&icons::CANCEL), copy::DL_CANCEL_ALL, p.text2, p).key(keys::CANCEL_ALL).on_press(actions::cancel_all));
    }
    let mut summary =
        col().gap(4.0).min_w(0.0).child(label(copy::active_and_total(active, tasks.len(), all_paused), mono(12.0, 400.0), p.muted));
    if tasks.len() >= 2 {
        summary = summary.child(
            label(copy::downloads_summary(tasks.len(), &format::summary_parts(tasks)), sans(13.0, 400.0), p.text2).single_line().truncate(),
        );
    }
    row().w_full().gap(16.0).center_y().child(summary.grow(1.0)).child(actions)
}

/// The toolbar swapped to the selection's actions; the selection's count lit like a channel.
fn selection(s: &State, p: &Pal) -> El {
    let (active, finished) = parts::selection(s);
    let count = row().gap(8.0).center_y().child(el().size(2.0, 16.0).radius(1.0).fill(p.accent)).child(label(
        copy::selected_count(s.selected.len()),
        mono(13.0, 500.0),
        p.text,
    ));
    let mut actions = row().gap(4.0).center_y();
    let cancel =
        ghost(Some(&icons::CANCEL), copy::DL_SELECTION_CANCEL, p.text2, p).key(keys::CANCEL_SELECTED).on_press(actions::cancel_selected);
    let armed = s.confirm == Some(Confirm::Remove) && finished > 0;
    if !armed {
        actions = actions.child(if active > 0 { cancel } else { cancel.opacity(0.4) });
    }
    if s.confirm == Some(Confirm::Remove) && finished > 0 {
        actions = actions.child(confirm(&copy::remove_confirm(finished), copy::DL_REMOVE_CONFIRM_ARIA, p));
    } else {
        let remove = ghost(Some(&icons::REMOVE), copy::DL_SELECTION_REMOVE, p.text2, p)
            .key(keys::arm(Confirm::Remove))
            .on_press(|s: &mut State| actions::arm(s, Confirm::Remove));
        actions = actions.child(if finished > 0 { remove } else { remove.opacity(0.4) });
    }
    // TODO(shared): `actions::clear_selection(s)`; the selection is let go here directly.
    actions = actions
        .child(ghost(None, copy::DL_SELECTION_CLEAR, p.text2, p).key(keys::CLEAR_SELECTION).on_press(|s: &mut State| s.selected.clear()));
    row().w_full().pl(4.0).gap(16.0).center_y().child(count).child(el().grow(1.0)).child(actions)
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
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(reach::section_list(s, true, p)))
        .child(label(w::SAVES, sans(12.0, 400.0), p.muted).ml(8.0));
    // One scroll of every section; the list marks the one in view.
    let mut sections = col().key("settings-sections").w_full().max_w(640.0).pt(32.0).pb(32.0).gap(40.0);
    for sec in settings_spec::sections(s) {
        sections = sections.child(reach::sheet(s, &sec, p));
    }
    let right = col().grow(1.0).h_full().min_w(0.0).pl(40.0).scroll(ScrollSpec { stick_bottom: false }).child(sections);
    let column = row().w_full().max_w(WIDE).h_full().min_h(0.0).pt(32.0).pb(32.0).child(left).child(right);
    col().w_full().grow(1.0).min_h(0.0).px(32.0).items_center().child(column)
}
