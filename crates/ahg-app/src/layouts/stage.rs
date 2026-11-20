//! Layout "Now playing", wearing the Orbit look. The app is a player with three places,
//! switched by a dock built into the player's transport bar: three bodies on one orbit line
//! (Home, Downloads, Settings; the current one solid, the others hollow), always visible,
//! Ctrl+1, 2, 3. The bar also holds the paste field, play-pause and stop for everything in
//! flight and the overall progress. The title band carries the folder and the palette (Ctrl+K).
//!
//! Home is the stage: the download arriving now, its cover inside the orbit ring drawn large
//! (the window's one bright moment) with its title and figures; the other downloads in flight
//! are satellites under it, each on its own small orbit (pressing one brings it to the
//! stage); the queue and the problems sit in the rail on the right; the files that landed lie
//! on the shelf, square now that they are yours. With nothing in flight the stage shows the
//! last file that landed and the month; with nothing landed either, the planet and the month;
//! on a first launch, the planet and the invitation to paste. Downloads is the complete dense
//! list. Settings is every section side by side.
//!
//! Where the old app's screens and features live now:
//!
//! | Old (Electron 1.x)                              | Now playing                                                              |
//! |-------------------------------------------------|--------------------------------------------------------------------------|
//! | Rail: Home, Downloads, Settings (Ctrl+1/2/7)    | The dock in the transport bar: Home, Downloads, Settings (Ctrl+1/2/3)    |
//! | Home: paste field, Look up, Ctrl+V              | The transport's field, on every place; Look up is the one lit button     |
//! | Home: status line (reading, adding, not a link) | A line that rises above the transport field while it applies             |
//! | Home: rest caption (folder, format, hotkey)     | The title band's folder control (folder · format); first launch prints  |
//! |                                                 |   the Ctrl+V and hotkey caption under the headline                       |
//! | Home: search results, playlist preview          | A sheet that rises from the transport field, a band per source           |
//! | Home: Works with (first launch)                 | Home's first-launch stage, under the planet                              |
//! | Home: Speed cell                                | The stage's Speed reading, the speed trace on the stage's floor, the     |
//! |                                                 |   transport's summed speed                                               |
//! | Home: This month, Where it came from            | The idle stage (month figure, sentence, source split)                    |
//! | Home: Recent downloads, View all                | Home's shelf; View all is the dock's Downloads                           |
//! | Home: "N in flight" pill, rail badge            | The transport's `N in flight`; the count beside the dock's Downloads     |
//! | Downloads: the list, groups, rows, row actions, | Downloads place: the same groups and actions, a ring per row in flight,  |
//! |   filters, search, selection, bulk actions      |   a square tile once landed; filters, search, select, Retry all failed,  |
//! |                                                 |   Clear completed, Pause all, Cancel all in its header                   |
//! | Downloads: interrupted banner                   | Downloads' header, and the top of Home's rail                            |
//! | Downloads: in progress, up next, failed         | Also on Home: stage and satellites, rail's Up next, rail's Needs         |
//! |                                                 |   attention (each problem with its sentence and buttons)                 |
//! | Downloads: Pause all / Resume all, Cancel all   | Also the transport's two round controls, on every place                  |
//! | YouTube / Spotify / SoundCloud / Reels tabs     | Scopes of the transport field (search bands per source); the palette has |
//! |                                                 |   Go to each; per-source format overrides sit in Download defaults       |
//! | Settings: every section, Machine ID             | Settings place: three columns, every section open; Machine ID in its     |
//! |                                                 |   header                                                                 |
//! | Settings: Scheduled downloads, Software (app and| Settings place, middle column; Software, Plan & license and Help on the  |
//! |   engine updates), Plan & license, cookies      |   right; cookies under More download options; Check for updates also in  |
//! |                                                 |   the palette                                                            |
//! | Command palette (Ctrl+K)                        | Over any place; the title band's Search control opens it                 |
//! | Keyboard shortcuts sheet (Ctrl+/)               | Over any place, from the palette or Settings' Help                       |
//! | Notices and toasts                              | A line that rises above the transport's right end                        |
//!
//! Gate limits: `State` names no staged task, so the stage takes the first download by a
//! fixed order (downloading, starting, converting, retrying, paused). The dock's press sets
//! `State.nav`; Ctrl+1..3 need the app's `on_key`, which this layout does not own.
mod list;
mod parts;
mod rail;
mod sheet;

use ahg_engine::stats::DownloadStats;
use ahg_engine::types::{DownloadStatus as St, DownloadTask};
use rune::{Color, Cursor, Cx, El, Icon, canvas, col, el, row, spacer, text_input};

use crate::actions;
use crate::copy;
use crate::directions::orbit::draw::{self, Pass, planet};
use crate::directions::orbit::found;
use crate::directions::orbit::look::{Pal, mix, txt};
use crate::directions::orbit::model;
use crate::directions::orbit::on_row;
use crate::format::{self, Bucket, is_active};
use crate::gate::{Confirm, Found, Nav, Overlay, State};
use crate::icons;
use crate::keys;
use parts::{BODY, BODY_M, FIG, FIG_L, FIG_XL, HERO, SMALL, SMALL_M, button, cover, glyph, icon_button, keys, state_word, text_button};

const TRANSPORT_H: f32 = 72.0;

/// A saved text setting, empty when it is missing.
fn setting<'a>(s: &'a State, key: &str) -> &'a str {
    s.settings.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

/// Sizes that follow the window: the full frame from 1100 x 760, a tighter one below.
#[derive(Clone, Copy)]
pub(crate) struct Fit {
    ring: f32,
    rail: f32,
    tile: f32,
    pad: f32,
    tall: bool,
    wide: bool,
}

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = Pal::new(s.palette, s.dark(cx));
    cx.titlebar_height(crate::TITLEBAR);
    cx.caption_color(p.text2);
    let insets = cx.chrome_insets();
    let (wide, tall) = cx.layout_with("stage-root", |r| (r[2] >= 1100.0, r[3] >= 760.0)).unwrap_or((true, true));
    let fit = Fit {
        ring: if tall { 296.0 } else { 192.0 },
        rail: if wide { 400.0 } else { 320.0 },
        tile: if tall { 80.0 } else { 56.0 },
        pad: if tall { 24.0 } else { 16.0 },
        tall,
        wide,
    };
    let first = s.tasks.is_empty() && s.stats.is_none();
    let home = |s: &State| if first { first_launch(s, &p).grow(1.0).min_h(0.0) } else { player(s, &p, fit) };
    let body = match s.nav {
        Nav::Home => match found::body(s, &p) {
            Some(answer) => found_sheet(s, &p, fit, home(s), answer),
            None => home(s),
        },
        Nav::Downloads => list::view(s, &p, fit),
        Nav::Settings => sheet::view(s, &p, fit),
    };
    col()
        .key("stage-root")
        .w_full()
        .h_full()
        .fill(p.bg)
        .child(titlebar(s, &p, fit, insets.leading, insets.trailing))
        .child(body)
        .child(transport(s, &p, fit))
}

fn titlebar(s: &State, p: &Pal, fit: Fit, leading: f32, trailing: f32) -> El {
    let mark = match &s.logo {
        Some(img) if img.is_ready() => el().size(16.0, 16.0).pixels(img.pixels()),
        _ => el().size(16.0, 16.0),
    };
    row()
        .key("titlebar")
        .w_full()
        .h(crate::TITLEBAR)
        .fill(p.deep)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
        .center_y()
        .window_drag()
        .pl(16.0 + leading)
        .gap(8.0)
        .child(mark)
        .child(row().child(txt(copy::BRAND, FIG, p.text)).child(txt(copy::BRAND_REST, FIG, p.muted)))
        .child(spacer())
        .child(tools(s, p, fit))
        .child(el().w(trailing + 8.0))
}

/// Where files go and the palette: quiet, in the title band's right end.
fn tools(s: &State, p: &Pal, fit: Fit) -> El {
    let dir = setting(s, "outputDir");
    let folder_name = format::file_name(dir).to_string();
    let folder = row()
        .key("folder")
        .h(32.0)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .hover(p.hover)
        .cursor(Cursor::Pointer)
        .role("button")
        .no_window_drag()
        .on_press_tx(actions::choose_folder)
        .label(copy::REST_CHANGE_ARIA)
        .help(dir.to_string())
        .child(glyph(&icons::FOLDER, p.text2));
    let folder = if fit.wide {
        folder.child(txt(folder_name, BODY_M, p.text)).child(txt(setting(s, "defaultFormat").to_uppercase(), FIG, p.muted))
    } else {
        folder
    };
    let search = row()
        .key("search")
        .h(32.0)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .hover(p.hover)
        .cursor(Cursor::Pointer)
        .role("button")
        .no_window_drag()
        .on_press(|s: &mut State| actions::open(s, Overlay::Palette))
        .label(copy::SEARCH_CAPSULE_ARIA)
        .help(copy::SEARCH_CAPSULE_ARIA)
        .child(glyph(&icons::SEARCH, p.text2))
        .child(keys(p, &[copy::KEY_CTRL, copy::KEY_K]));
    row().gap(8.0).center_y().child(folder).child(search)
}

// ── The player ─────────────────────────────────────────────────────────────

/// Which in-flight task takes the stage: the first downloading one, else starting,
/// converting, retrying, paused.
fn staged(s: &State) -> Option<&DownloadTask> {
    let tasks = &s.tasks;
    let rank = |t: &DownloadTask| match t.progress.status {
        St::Downloading => Some(0),
        St::FetchingInfo => Some(1),
        St::Converting | St::Embedding => Some(2),
        St::Retrying => Some(3),
        St::Paused => Some(4),
        _ => None,
    };
    let chosen = s.staged.as_ref().and_then(|id| tasks.iter().find(|t| &t.task_id == id && rank(t).is_some()));
    chosen.or_else(|| tasks.iter().enumerate().filter_map(|(i, t)| rank(t).map(|r| ((r, i), t))).min_by_key(|(k, _)| *k).map(|(_, t)| t))
}

fn player(s: &State, p: &Pal, fit: Fit) -> El {
    let on_stage = staged(s);
    let landed: Vec<&DownloadTask> = s.tasks.iter().filter(|t| t.progress.status == St::Done).collect();
    let satellites: Vec<&DownloadTask> = s
        .tasks
        .iter()
        .filter(|t| is_active(t) && t.progress.status != St::Queued && on_stage.is_none_or(|o| o.task_id != t.task_id))
        .collect();

    let stage = match (on_stage, landed.first()) {
        (Some(t), _) => live_stage(s, p, t, fit),
        (None, Some(t)) => landed_stage(s, p, t, fit),
        (None, None) => month_stage(s, p, fit),
    };
    let mut left = col().key("stage").grow(1.0).min_w(0.0).h_full().px(fit.pad + 8.0).py(fit.pad);
    if satellites.is_empty() && landed.is_empty() {
        // Only the stage: hold it on the window's middle, not its top.
        left = left.child(spacer()).child(stage).child(spacer());
    } else {
        left = left.child(stage);
        if !satellites.is_empty() {
            left = left.child(orbiting(s, p, &satellites, fit).mt(fit.pad));
        }
        left = left.child(spacer());
        // With no shelf yet, the stage's floor carries the speed trace, a player's level meter.
        if landed.is_empty() && fit.tall && s.tasks.iter().any(|t| t.progress.speed > 0.0) {
            left = left.child(speed(s, p));
        }
    }
    if !landed.is_empty() {
        left = left.child(shelf(s, p, &landed, fit));
    }
    row().w_full().grow(1.0).min_h(0.0).child(left).child(rail::view(s, p, fit))
}

/// The orbit a task is on and the ink it is drawn in; `None` once it stopped.
#[allow(dead_code)] // unused until cleanup (step 19)
pub(crate) fn pass(p: &Pal, t: &DownloadTask) -> Option<(Pass, Color)> {
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

/// A cover on its orbit: `size` is the ring's square, the cover sits `inset` inside it.
pub(crate) fn on_orbit(s: &State, p: &Pal, t: &DownloadTask, size: f32, inset: f32) -> El {
    let mut ring = el().size(size, size).center();
    if draw::has_ring(s, t) {
        ring = ring.child(draw::task_ring(s, t, size, &draw::material(p, crate::ring::tint(t, p.dark), p.bg)).absolute(0.0, 0.0));
    }
    ring.child(cover(s, p, t, size - 2.0 * inset, true))
}

/// `Spotify · Daft Punk`: the source in its colour, then the uploader.
fn byline(p: &Pal, t: &DownloadTask) -> El {
    let (name, color) = model::source(p, t.source);
    let mut r = row().min_w(0.0).center_y().child(txt(name, SMALL_M, color));
    if !t.uploader.is_empty() {
        r = r.child(txt(copy::row_dot_part(&t.uploader), SMALL, p.text2).truncate().shrink());
    }
    r
}

fn reading(p: &Pal, label: &str, value: String) -> El {
    col().gap(4.0).child(txt(label.to_string(), SMALL, p.muted)).child(txt(value, FIG_L, p.text).single_line())
}

fn live_stage(s: &State, p: &Pal, t: &DownloadTask, fit: Fit) -> El {
    let pr = &t.progress;
    let or_dash = |v: String| if v.is_empty() { copy::RATE_UNKNOWN.to_string() } else { v };
    let size = if pr.total > 0.0 {
        format!("{} of {}", format::bytes(pr.downloaded), format::bytes(pr.total))
    } else {
        copy::RATE_UNKNOWN.to_string()
    };
    let figure = if pr.status == St::Downloading {
        txt(format!("{}%", pr.percent.clamp(0.0, 100.0).round()), FIG_XL, p.text)
    } else {
        let (word, tone) = state_word(p, t);
        txt(word, FIG_XL, tone)
    };
    let gap = if fit.tall { 32.0 } else { 16.0 };
    let readings = row()
        .gap(gap)
        .child(reading(p, copy::CELL_SPEED, or_dash(format::speed(pr.speed))))
        .child(reading(p, copy::LABEL_TIME_LEFT, or_dash(format::eta(pr.eta))))
        .child(reading(p, copy::LABEL_SIZE, size))
        .label(copy::speed_aria(&format::bytes(pr.speed)));
    let mut acts = row().gap(8.0);
    for (label, i) in parts::actions(t) {
        acts = acts.child(act_button(p, t, label, i));
    }
    let info = col()
        .grow(1.0)
        .min_w(0.0)
        .max_w(520.0)
        .child(byline(p, t))
        .child(txt(format::label(t), HERO, p.text).wrap().max_lines(if fit.tall { 2 } else { 1 }).mt(8.0))
        .child(txt(format::specs(t), FIG, p.muted).mt(8.0))
        .child(figure.mt(if fit.tall { 24.0 } else { 12.0 }))
        .child(readings.mt(8.0))
        .child(acts.mt(if fit.tall { 24.0 } else { 16.0 }));
    row()
        .w_full()
        .min_h(fit.ring)
        .gap(if fit.tall { 48.0 } else { 32.0 })
        .center_y()
        .child(on_orbit(s, p, t, fit.ring, 28.0).key("stage-ring").label(copy::row_progress_aria(&t.title)))
        .child(info)
}

/// One of a task's buttons: its shared key, its action.
fn act_button(p: &Pal, t: &DownloadTask, label: &str, i: &'static Icon) -> El {
    let act = parts::act(label);
    on_row(button(p, keys::row_action(act, &t.task_id), label, Some(i), false), act, &t.task_id)
}

/// Nothing in flight: the last file that landed, square, and the month under it.
fn landed_stage(s: &State, p: &Pal, t: &DownloadTask, fit: Fit) -> El {
    let (word, tone) = state_word(p, t);
    let file = format::file_name(&t.progress.filename).to_string();
    let mut acts = row().gap(8.0);
    for (label, i) in parts::actions(t) {
        acts = acts.child(act_button(p, t, label, i));
    }
    let mut info = col()
        .grow(1.0)
        .min_w(0.0)
        .max_w(520.0)
        .child(row().gap(8.0).center_y().child(glyph(&icons::CHECK_SHELF, tone)).child(txt(word, SMALL_M, tone)).child(byline(p, t)))
        .child(txt(format::label(t), HERO, p.text).wrap().max_lines(if fit.tall { 2 } else { 1 }).mt(8.0))
        .child(txt(file, FIG, p.text2).truncate().mt(8.0).help(t.progress.filename.clone()))
        .child(acts.mt(16.0));
    if let Some(st) = &s.stats {
        info = info.child(month(p, st, fit.tall, true).mt(if fit.tall { 24.0 } else { 16.0 }));
    }
    let art = el().size(fit.ring, fit.ring).center().child(cover(s, p, t, fit.ring - 56.0, false));
    row().w_full().min_h(fit.ring).gap(if fit.tall { 48.0 } else { 32.0 }).center_y().child(art).child(info)
}

/// Nothing in flight and nothing landed: the planet, and the month's figures.
fn month_stage(s: &State, p: &Pal, fit: Fit) -> El {
    let w = fit.ring;
    let art = el().size(w, w).center().child(canvas(planet(w - 16.0, mix(p.text, p.bg, 0.08), p.bg)).size(w - 16.0, (w - 16.0) * 0.592));
    let info = match &s.stats {
        Some(st) => month(p, st, true, false),
        None => txt(copy::MONTH_NOTHING_YET_NO_PREV, BODY, p.text2),
    };
    row()
        .w_full()
        .min_h(fit.ring)
        .gap(if fit.tall { 48.0 } else { 32.0 })
        .center_y()
        .child(art)
        .child(col().grow(1.0).min_w(0.0).max_w(520.0).child(info))
}

/// This month: the count, the sentence against last month, and where it came from.
fn month(p: &Pal, st: &DownloadStats, split: bool, rule: bool) -> El {
    let cur = &st.current;
    let files = cur.files as u64;
    let head =
        row().gap(8.0).child(txt(copy::CELL_THIS_MONTH, SMALL_M, p.text2)).child(txt(format::month_name(&cur.month), SMALL, p.muted));
    let mut c = col().w_full().child(head);
    if rule {
        c = c.pt(16.0).border_sides([1.0, 0.0, 0.0, 0.0], p.line);
    }
    if files == 0 {
        let second = format::month_sentence(st);
        return c.child(txt(copy::MONTH_NOTHING_YET, BODY, p.text).mt(8.0)).child(txt(second, SMALL, p.text2).wrap().mt(4.0));
    }
    let ending = format::month_sentence(st);
    c = c
        .child(
            row()
                .gap(8.0)
                .items_end()
                .mt(8.0)
                .child(txt(files.to_string(), FIG_XL, p.text))
                .child(txt(copy::file_word(files), BODY, p.text2).mb(8.0)),
        )
        .child(row().flex_wrap().child(txt(format::bytes(cur.bytes), FIG, p.text2)).child(txt(ending, SMALL, p.text2)));
    if split {
        c = c.child(sources(p, st).mt(16.0));
    }
    c
}

/// Where it came from: one bar split by source, and its legend.
fn sources(p: &Pal, st: &DownloadStats) -> El {
    let cur = &st.current;
    let total = cur.files.max(1.0);
    let colour = |b: Bucket| match b {
        Bucket::Youtube => p.youtube,
        Bucket::Spotify => p.spotify,
        Bucket::Soundcloud => p.soundcloud,
        Bucket::Reels => p.reels,
        Bucket::Other => p.muted,
    };
    let mut bar = row().w_full().h(4.0).gap(4.0);
    let mut legend = row().w_full().gaps(4.0, 16.0).flex_wrap();
    for (bucket, n) in format::source_split(st) {
        let color = colour(bucket);
        bar = bar.child(el().grow(n as f32).h_full().radius(2.0).fill(color));
        let pct = (n as f64 / total * 100.0).round() as u64;
        legend = legend.child(
            row()
                .gap(8.0)
                .center_y()
                .child(el().size(8.0, 8.0).radius(4.0).fill(color))
                .child(txt(bucket.name(), SMALL, p.text2))
                .child(txt(copy::split_row_figures(pct, n), FIG, p.muted)),
        );
    }
    col()
        .w_full()
        .gap(12.0)
        .child(row().child(txt(copy::CELL_WHERE_FROM, SMALL_M, p.text2)).child(spacer()).child(txt(
            copy::split_aside(cur.files as u64),
            FIG,
            p.muted,
        )))
        .child(bar)
        .child(legend)
}

/// The summed speed of the running downloads, the last samples (`State.speed`).
fn speed(s: &State, p: &Pal) -> El {
    let samples: Vec<f32> = s.speed.iter().copied().collect();
    let now = format::total_speed(&s.tasks);
    let peak = samples.iter().copied().fold(0.0f32, f32::max).max(1.0) as f64;
    let scale = format::axis_ceiling(peak);
    let head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(txt(copy::CELL_SPEED, BODY_M, p.text2))
        .child(txt(copy::speed_now(&format::bytes(now)), FIG, p.text))
        .child(spacer())
        .child(txt(copy::speed_scale(&format::bytes(scale)), FIG, p.muted));
    let chart =
        canvas(rune_trace(&samples, scale as f32, p).view_box(800.0, 64.0)).w_full().h(64.0).label(copy::speed_aria(&format::bytes(now)));
    col().w_full().gap(12.0).child(head).child(chart)
}

fn rune_trace(samples: &[f32], scale: f32, p: &Pal) -> rune::Shape {
    crate::directions::orbit::draw::trace(samples, scale, 800.0, 64.0, p.line, p.text2)
}

// ── Satellites ─────────────────────────────────────────────────────────────

/// `2 downloading · 1 converting · 1 retrying · 1 paused · 1 queued`.
fn summary(tasks: &[DownloadTask]) -> String {
    let converting = copy::STATE_CONVERTING.to_lowercase();
    let words = [copy::SUMMARY_WORDS[0], converting.as_str(), copy::SUMMARY_WORDS[1], copy::SUMMARY_WORDS[3], copy::SUMMARY_WORDS[2]];
    let mut n = [0usize; 5];
    for t in tasks {
        let at = match t.progress.status {
            St::FetchingInfo | St::Downloading => 0,
            St::Converting | St::Embedding => 1,
            St::Retrying => 2,
            St::Paused => 3,
            St::Queued => 4,
            _ => continue,
        };
        n[at] += 1;
    }
    n.iter().zip(words).filter(|(c, _)| **c > 0).map(|(c, w)| format!("{c} {w}")).collect::<Vec<_>>().join(" · ")
}

fn orbiting(s: &State, p: &Pal, sats: &[&DownloadTask], fit: Fit) -> El {
    let head = row().w_full().center_y().child(txt(copy::GROUP_IN_PROGRESS, BODY_M, p.text2)).child(spacer()).child(txt(
        summary(&s.tasks),
        FIG,
        p.muted,
    ));
    let mut grid = col().w_full().gap(8.0);
    for pair in sats.chunks(2) {
        let mut r = row().w_full().gap(24.0);
        for t in pair {
            r = r.child(satellite(s, p, t));
        }
        if pair.len() == 1 {
            r = r.child(el().grow(1.0));
        }
        grid = grid.child(r);
    }
    col()
        .w_full()
        .gap(if fit.tall { 12.0 } else { 8.0 })
        .pt(if fit.tall { 16.0 } else { 12.0 })
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .child(head)
        .child(grid)
}

/// What a moving task reads as on one line.
pub(crate) fn motion_line(p: &Pal, t: &DownloadTask) -> El {
    let pr = &t.progress;
    match pr.status {
        St::Downloading => {
            let rate = format::rate(pr.speed, pr.eta);
            row().gap(8.0).child(txt(format!("{}%", pr.percent.round()), FIG, p.text)).child(txt(rate, FIG, p.muted))
        }
        St::Retrying => row().gap(8.0).center_y().child(glyph(&icons::RETRY, p.warning)).child(txt(parts::retry_line(t), SMALL, p.warning)),
        St::Paused => row()
            .gap(8.0)
            .center_y()
            .child(glyph(&icons::PAUSE, p.warning))
            .child(txt(copy::STATE_PAUSED, SMALL_M, p.warning))
            .child(txt(format!("{}%", pr.percent.round()), FIG, p.muted)),
        _ => {
            let (word, tone) = state_word(p, t);
            row().gap(8.0).child(txt(word, SMALL_M, tone)).child(txt(format::specs(t), FIG, p.muted).truncate().shrink())
        }
    }
}

fn satellite(s: &State, p: &Pal, t: &DownloadTask) -> El {
    row()
        .key(format!("sat-{}", t.task_id))
        .grow(1.0)
        .min_w(0.0)
        .h(48.0)
        .gap(12.0)
        .center_y()
        .cursor(Cursor::Pointer)
        .role("button")
        .focusable(true)
        .label(format::label(t))
        // TODO(shared): `actions::stage(s, id)`.
        .on_press({
            let id = t.task_id.clone();
            move |s: &mut State| s.staged = Some(id.clone())
        })
        .child(on_orbit(s, p, t, 48.0, 8.0))
        .child(col().grow(1.0).min_w(0.0).gap(4.0).child(txt(format::label(t), BODY_M, p.text).truncate()).child(motion_line(p, t)))
}

// ── The shelf ──────────────────────────────────────────────────────────────

fn shelf(s: &State, p: &Pal, landed: &[&DownloadTask], fit: Fit) -> El {
    let head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(txt(copy::GROUP_COMPLETED, BODY_M, p.text2))
        .child(txt(landed.len().to_string(), FIG, p.muted))
        .child(spacer())
        .child(
            text_button(p, keys::arm(Confirm::Clear).into(), copy::DL_CLEAR_COMPLETED)
                .on_press(|s: &mut State| actions::arm(s, Confirm::Clear)),
        );
    let mut tiles = row().w_full().gap(16.0);
    for t in landed.iter().take(7) {
        let size = format::bytes(t.progress.downloaded);
        let id = t.task_id.clone();
        let mut tile = col()
            .key(format!("tile-{}", t.task_id))
            .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| actions::show_in_folder(s, tx, &id))
            .w(fit.tile + 24.0)
            .gap(8.0)
            .cursor(Cursor::Pointer)
            .help(copy::recent_open_aria(&format::label(t)))
            .child(cover(s, p, t, fit.tile, false));
        tile = tile.child(
            col()
                .w_full()
                .gap(0.0)
                .child(txt(format::label(t), SMALL_M, p.text).truncate())
                .child(txt(format!("{} · {size}", t.format.to_uppercase()), FIG, p.muted).truncate()),
        );
        tiles = tiles.child(tile);
    }
    col()
        .w_full()
        .gap(12.0)
        .pt(if fit.tall { 16.0 } else { 12.0 })
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .child(head)
        .child(tiles.clip_rounded(0.0))
}

// ── First launch ───────────────────────────────────────────────────────────

fn first_launch(s: &State, p: &Pal) -> El {
    let w = 400.0;
    let mark = canvas(planet(w, mix(p.text, p.bg, 0.08), p.bg)).key("planet").size(w, w * 0.592);
    let mut caption = row()
        .center_y()
        .child(keys(p, &[copy::KEY_CTRL]))
        .child(txt(copy::REST_PLUS_SPACED, SMALL, p.muted))
        .child(keys(p, &[copy::REST_KEY_V]))
        .child(txt(copy::REST_PASTES_A_LINK, SMALL, p.text2));
    // Only when a global hotkey is set; `CommandOrControl` shows as Ctrl.
    let hotkey = setting(s, "globalHotkey");
    if !hotkey.is_empty() {
        caption = caption.child(txt(copy::REST_HOTKEY_LEAD, SMALL, p.text2));
        for (i, part) in hotkey.split('+').enumerate() {
            if i > 0 {
                caption = caption.child(txt("+", SMALL, p.muted).mx(4.0));
            }
            caption = caption.child(keys(p, &[if part == "CommandOrControl" { copy::KEY_CTRL } else { part }]));
        }
        caption = caption.child(txt(copy::REST_HOTKEY_TRAIL, SMALL, p.text2));
    }
    let source =
        |i: &'static Icon, name: &'static str, c: Color| row().gap(8.0).center_y().child(glyph(i, c)).child(txt(name, SMALL_M, p.text));
    let works = row()
        .gap(16.0)
        .center_y()
        .child(txt(copy::WORKS_WITH, SMALL, p.muted))
        .child(source(&icons::YOUTUBE, copy::WORKS_WITH_YOUTUBE, p.youtube))
        .child(source(&icons::SPOTIFY, copy::WORKS_WITH_SPOTIFY, p.spotify))
        .child(source(&icons::SOUNDCLOUD, copy::WORKS_WITH_SOUNDCLOUD, p.soundcloud))
        .child(source(&icons::REELS, copy::WORKS_WITH_REELS, p.reels))
        .child(txt(copy::WORKS_WITH_TAIL, SMALL, p.muted));
    col().w_full().center().child(mark).child(txt(copy::HOME_HEADLINE, HERO, p.text).mt(40.0)).child(caption.mt(16.0)).child(works.mt(32.0))
}

// ── What the field found ───────────────────────────────────────────────────

/// The answer rises out of the transport bar as a sheet over the stage, which stays faintly
/// behind it (what is playing goes on playing). The sheet stands on the bar: its foot is open
/// to the bar's hairline. Short answers are as tall as they need; a track list takes the
/// stage's height and scrolls inside. Esc or the close button puts the stage back.
fn found_sheet(s: &State, p: &Pal, fit: Fit, under: El, answer: El) -> El {
    let side = fit.pad + 8.0;
    let head = row().w_full().h(32.0).center_y().child(spacer()).child(found::dismiss(p));
    let mut sheet = col()
        .key(keys::FOUND)
        .px(24.0)
        .pt(16.0)
        .pb(24.0)
        .gap(8.0)
        .fill(p.card)
        .border_sides([1.0, 1.0, 0.0, 1.0], p.track)
        .corners([16.0, 16.0, 0.0, 0.0])
        .shadow(-8.0, 32.0, Color::BLACK.alpha(if p.dark { 0.45 } else { 0.10 }))
        .child(head);
    sheet = if found::fills(s) {
        sheet.child(answer.grow(1.0).min_h(0.0)).pin(Some(side), Some(fit.pad), Some(side), Some(0.0))
    } else {
        sheet.child(answer).pin(Some(side), None, Some(side), Some(0.0))
    };
    col().w_full().grow(1.0).min_h(0.0).child(under.opacity(0.24).inert()).child(sheet)
}

// ── The transport ──────────────────────────────────────────────────────────

fn transport(s: &State, p: &Pal, fit: Fit) -> El {
    let active: Vec<&DownloadTask> = s.tasks.iter().filter(|t| is_active(t)).collect();
    let all_paused = format::all_paused(&s.tasks);

    let field = row()
        .key(keys::PASTE)
        .grow(1.0)
        .min_w(0.0)
        .max_w(480.0)
        .h(48.0)
        .pl(16.0)
        .pr(8.0)
        .gap(12.0)
        .center_y()
        .radius(12.0)
        .fill(p.card)
        .border(1.0, p.track)
        .label(copy::FIELD_ARIA)
        .child(glyph(&icons::LINK, p.muted))
        .child(
            text_input(&s.field, BODY)
                .key(keys::FIELD)
                .label(copy::FIELD_ARIA)
                .single_line()
                .color(p.text)
                .caret(p.text, p.accent.alpha(0.3))
                .placeholder(copy::FIELD_PLACEHOLDER, p.muted)
                .grow(1.0)
                .min_w(0.0)
                .on_edit(|s: &mut State| &mut s.field)
                .on_input_change(|s: &mut State, _| s.input = s.field.text.clone())
                .on_submit(actions::submit),
        )
        // While an answer is open its Download is the lit control, so Look up steps down.
        .child(
            button(p, keys::LOOK_UP.into(), copy::FIELD_BUTTON_LOOK_UP, None, matches!(s.found, Found::Nothing))
                .on_press_tx(actions::submit)
                .radius(4.0),
        );

    // Play-pause and stop, as on any player: one for everything in flight.
    let (pp_label, pp_icon) = if all_paused { (copy::DL_RESUME_ALL, &icons::PLAY) } else { (copy::DL_PAUSE_ALL, &icons::PAUSE) };
    let ink = if active.is_empty() { p.muted } else { p.text };
    let play = el()
        .key(keys::PAUSE_ALL)
        .size(40.0, 40.0)
        .center()
        .radius(20.0)
        .border(1.0, p.track)
        .hover(p.raised)
        .cursor(Cursor::Pointer)
        .role("button")
        .label(pp_label)
        .help(pp_label)
        .focusable(true)
        .on_press(actions::pause_or_resume_all)
        .child(glyph(pp_icon, ink));
    let stop =
        icon_button(p, keys::CANCEL_ALL.into(), copy::DL_CANCEL_ALL, &icons::CANCEL, if active.is_empty() { p.muted } else { p.text2 })
            .on_press(actions::cancel_all);

    // The whole flight: everything known in bytes, as one line.
    let (done, total) =
        active.iter().filter(|t| t.progress.total > 0.0).fold((0.0, 0.0), |(d, n), t| (d + t.progress.downloaded, n + t.progress.total));
    let (flying, speed) = format::in_flight(&s.tasks);
    let overall = if active.is_empty() {
        col().w(if fit.tall { 240.0 } else { 160.0 }).child(txt(copy::EMPTY_FILTER_ACTIVE, SMALL, p.muted).truncate())
    } else {
        let frac = if total > 0.0 { (done / total) as f32 } else { 0.0 };
        let mut figures = row()
            .grow(1.0)
            .min_w(0.0)
            .gap(8.0)
            .child(txt(active.len().to_string(), FIG, p.text))
            .child(txt(copy::HOME_IN_FLIGHT_SUFFIX.trim_start(), SMALL, p.text2))
            .child(spacer());
        if speed > 0.0 {
            figures = figures.child(txt(format::speed(speed), FIG, p.text2));
        }
        figures = figures.child(txt(format!("{}%", (frac * 100.0).round()), FIG, p.muted));
        row()
            .w(if fit.tall { 240.0 } else { 160.0 })
            .gap(12.0)
            .center_y()
            .label(copy::transferring_aria(flying, &format::speed(speed)))
            .child(draw::overall_ring(s, "transport-ring", 28.0, &draw::material(p, p.accent, p.deep)))
            .child(figures)
    };

    row()
        .key("transport")
        .w_full()
        .h(TRANSPORT_H)
        .px(fit.pad)
        .gap(if fit.wide { 24.0 } else { 16.0 })
        .center_y()
        .fill(p.deep)
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .child(dock(s, p, active.len()))
        .child(el().size(1.0, 32.0).fill(p.line))
        .child(field)
        .child(row().gap(8.0).center_y().child(play).child(stop))
        .child(overall)
}

/// The three places, as three bodies on one orbit line: the current one is a solid body
/// with its name in full ink, the others hollow. Always shown, Ctrl+1, 2, 3.
fn dock(s: &State, p: &Pal, active: usize) -> El {
    let places: [(Nav, &str, &'static Icon); 3] = [
        (Nav::Home, copy::NAV_HOME, &icons::HOME),
        (Nav::Downloads, copy::NAV_DOWNLOADS, &icons::DOWNLOAD),
        (Nav::Settings, copy::NAV_SETTINGS, &icons::SETTINGS),
    ];
    let mut items = row().gap(4.0).role("tablist").label(copy::RAIL_ARIA);
    for (i, (nav, name, i_con)) in places.into_iter().enumerate() {
        let on = s.nav == nav;
        let body = if on {
            el().size(8.0, 8.0).radius(4.0).fill(p.text)
        } else {
            el().size(8.0, 8.0).radius(4.0).fill(p.deep).border(1.0, p.track)
        };
        let mut label = row().gap(8.0).center_y().child(glyph(i_con, if on { p.text } else { p.muted })).child(txt(
            name,
            BODY_M,
            if on { p.text } else { p.text2 },
        ));
        if nav == Nav::Downloads && active > 0 {
            label = label.child(txt(active.to_string(), FIG, p.muted));
        }
        let mut item = col()
            .key(keys::nav(nav))
            .h(44.0)
            .px(12.0)
            .py(4.0)
            .gap(8.0)
            .items_center()
            .radius(8.0)
            .cursor(Cursor::Pointer)
            .role("tab")
            .checked(on)
            .focusable(true)
            .label(name)
            .help(copy::nav_tooltip(name, i + 1))
            .on_press(move |s: &mut State| actions::go(s, nav))
            .child(el().h(8.0).center().child(body))
            .child(label);
        if !on {
            item = item.hover(p.hover);
        }
        items = items.child(item);
    }
    // The orbit line runs through the three bodies, behind them.
    el().child(el().h(1.0).fill(p.track).pin(Some(24.0), Some(8.0), Some(24.0), None)).child(items)
}
