//! Home: the paste field is the launch point and the planet stands beside it, the product's
//! own mark at a size that means it.
use ahg_engine::stats::DownloadStats;
use ahg_engine::types::{DownloadStatus as St, DownloadTask};
use rune::{Color, El, Icon, ScrollSpec, TextEdit, canvas, col, el, icon, row, spacer, text_input};

use super::draw::{planet, trace};
use super::look::{BODY, BODY_M, CELL_TITLE, DISPLAY, FIG, FIG_L, FIG_S, FIG_XL, Pal, SMALL, SMALL_M, mix, txt};
use super::model;
use super::{Tone, button, cover, found, keycap};
use crate::actions;
use crate::copy;
use crate::format::{self, Bucket, is_active};
use crate::gate::{Nav, State};
use crate::icons;
use crate::keys;

pub fn view(s: &State, p: &Pal) -> El {
    let active: Vec<&DownloadTask> = s.tasks.iter().filter(|t| is_active(t)).collect();
    let done: Vec<&DownloadTask> = s.tasks.iter().filter(|t| t.progress.status == St::Done).collect();
    let counted = s.stats.as_ref().is_some_and(|st| st.current.files > 0.0 || st.previous.as_ref().is_some_and(|p| p.files > 0.0));
    let quiet = !counted && done.is_empty() && active.is_empty();
    if let Some(body) = found::body(s, p) {
        return found_page(s, p, active.len(), body);
    }

    let planet_w = if quiet { 380.0 } else { 260.0 };
    let mark = canvas(planet(planet_w, mix(p.text, p.bg, 0.08), p.bg)).key("planet").size(planet_w, planet_w * 0.592);

    let mut launch =
        col().grow(1.0).min_w(0.0).gap(20.0).child(headline(p, active.len())).child(field(p, &s.field, false)).child(status(s, p));
    if quiet {
        launch = launch.child(works_with(p).mt(12.0));
    }
    let top = row().w_full().gap(56.0).center_y().child(launch).child(mark);

    if quiet {
        // First launch: the field and the planet are the whole screen, a little above centre.
        return col().w_full().h_full().px(56.0).pb(56.0).justify_center().child(top);
    }
    let mut page = col().w_full().max_w(1600.0).px(40.0).pt(24.0).pb(48.0).gap(28.0).child(top);
    let mut bento = col().w_full().gap(32.0);
    if !active.is_empty() {
        bento = bento.child(speed(s, p));
    }
    if let Some(stats) = &s.stats {
        bento = bento.child(row().w_full().gap(48.0).items_start().child(this_month(p, stats)).child(sources(p, stats)));
    }
    if !done.is_empty() {
        bento = bento.child(recent(s, p, &done));
    }
    page = page.child(bento);
    col().key("home").scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0).w_full().child(page)
}

// ── The launch point ────────────────────────────────────────────────────────

fn headline(p: &Pal, in_flight: usize) -> El {
    let mut r = row().w_full().gap(16.0).center_y().child(txt(copy::HOME_HEADLINE, DISPLAY, p.text));
    if in_flight > 0 {
        r = r.child(
            row()
                .key("in-flight")
                .h(32.0)
                .px(12.0)
                .gap(8.0)
                .center_y()
                .radius(16.0)
                .border(1.0, p.track)
                .hover(p.raised)
                .cursor(rune::Cursor::Pointer)
                .role("button")
                .focusable(true)
                .on_press(|s: &mut State| actions::go(s, Nav::Downloads))
                .child(el().size(8.0, 8.0).radius(4.0).fill(p.text2))
                .child(row().center_y().child(txt(in_flight.to_string(), FIG, p.text)).child(txt(
                    copy::HOME_IN_FLIGHT_SUFFIX,
                    SMALL,
                    p.text2,
                )))
                .child(icon(&icons::FORWARD, 16.0).color(p.text2)),
        );
    }
    r
}

/// The paste field. `open`: it has opened into the found surface, so it carries the way back
/// and Look up steps down to an outline (the surface's Download is the lit control then).
fn field(p: &Pal, edit: &TextEdit, open: bool) -> El {
    let (fill, ink, edge, over) =
        if open { (Color::TRANSPARENT, p.text, p.track, p.raised) } else { (p.accent, p.on_accent, p.accent, mix(p.accent, p.text, 0.15)) };
    let go = row()
        .key(keys::LOOK_UP)
        .h(44.0)
        .px(16.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(fill)
        .border(1.0, edge)
        .hover(over)
        .press_scale(0.97)
        .role("button")
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .on_press_tx(actions::submit)
        .child(txt(copy::FIELD_BUTTON_LOOK_UP, BODY_M, ink))
        .child(icon(&icons::FORWARD, 16.0).color(ink));
    let mut r = row()
        .key(keys::PASTE)
        .w_full()
        .h(64.0)
        .pl(20.0)
        .pr(12.0)
        .gap(16.0)
        .center_y()
        .label(copy::FIELD_ARIA)
        .child(icon(&icons::LINK, 16.0).color(p.muted))
        .child(
            text_input(edit, BODY.lh(22.0))
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
                .on_submit(actions::submit)
                .on_input_key(crate::gate::on_field_escape),
        );
    if open {
        r = r.child(found::dismiss(p));
    }
    r = r.child(go);
    if open { r } else { r.radius(16.0).fill(p.card).border(1.0, p.track) }
}

/// Home once something was looked up: the field opens downward into what it found, one
/// piece, so the answer reads as the field's own. A track list takes the height left and
/// scrolls inside.
fn found_page(s: &State, p: &Pal, in_flight: usize, body: El) -> El {
    let fills = found::fills(s);
    let mut inner = col().w_full().padding(24.0).border_sides([1.0, 0.0, 0.0, 0.0], p.line).child(body);
    let mut unit =
        col().key(keys::FOUND).w_full().min_h(0.0).radius(16.0).fill(p.card).border(1.0, p.track).child(field(p, &s.field, true));
    if fills {
        inner = inner.grow(1.0).min_h(0.0);
        unit = unit.grow(1.0);
    }
    // A track list needs every row of height it can get: the headline steps aside for it.
    let mut page = col().w_full().h_full().px(40.0).pt(24.0).pb(24.0).gap(20.0);
    if !fills {
        page = page.child(headline(p, in_flight));
    }
    page.child(unit.child(inner))
}

fn status(s: &State, p: &Pal) -> El {
    let setting = |key: &str| s.settings.get(key).and_then(|v| v.as_str()).unwrap_or("");
    let mut line = row().w_full().gaps(8.0, 24.0).center_y().flex_wrap();
    if let Some(folder) = s.tasks.first().map(|t| t.output_dir.clone()).filter(|f| !f.is_empty()) {
        // Only `/` separates here, as in Electron: a Windows path prints in full.
        let name = folder.rsplit('/').next().unwrap_or(&folder).to_string();
        line = line.child(
            row()
                .center_y()
                .child(txt(copy::rest_saves_as(setting("defaultFormat")), SMALL, p.text2))
                .child(txt(name, SMALL_M, p.text).help(folder))
                .child(txt(" ", SMALL, p.text2))
                .child(
                    txt(copy::REST_CHANGE, SMALL_M, p.text2)
                        .key(keys::CHANGE_FOLDER)
                        .label(copy::REST_CHANGE_ARIA)
                        .cursor(rune::Cursor::Pointer)
                        .on_press_tx(actions::choose_folder),
                ),
        );
    }
    // The strings carry their own spaces, so the pieces sit flush.
    let mut right = row()
        .center_y()
        .child(keycap(p, copy::KEY_CTRL))
        .child(txt(copy::REST_PLUS_SPACED, SMALL, p.muted))
        .child(keycap(p, copy::REST_KEY_V))
        .child(txt(copy::REST_PASTES_A_LINK, SMALL, p.text2));
    // Only when a global hotkey is set; `CommandOrControl` shows as Ctrl.
    let hotkey = setting("globalHotkey");
    if !hotkey.is_empty() {
        right = right.child(txt(copy::REST_HOTKEY_LEAD, SMALL, p.text2));
        for (i, part) in crate::format::hotkey_keys(hotkey).into_iter().enumerate() {
            if i > 0 {
                right = right.child(txt("+", SMALL, p.muted).mx(4.0));
            }
            right = right.child(keycap(p, part));
        }
        right = right.child(txt(copy::REST_HOTKEY_TRAIL, SMALL, p.text2));
    }
    line.child(right)
}

fn works_with(p: &Pal) -> El {
    let chip = |key: &str, glyph: &'static Icon, name: &'static str, color: Color| {
        row()
            .key(format!("works-{key}"))
            .h(32.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(8.0)
            .border(1.0, p.line)
            .hover(p.raised)
            .cursor(rune::Cursor::Pointer)
            .role("button")
            .focusable(true)
            .child(icon(glyph, 16.0).color(color))
            .child(txt(name, SMALL_M, p.text))
    };
    row()
        .gap(8.0)
        .center_y()
        .flex_wrap()
        .child(txt(copy::WORKS_WITH, SMALL, p.text2).mr(4.0))
        .child(chip("youtube", &icons::YOUTUBE, copy::WORKS_WITH_YOUTUBE, p.youtube))
        .child(chip("spotify", &icons::SPOTIFY, copy::WORKS_WITH_SPOTIFY, p.spotify))
        .child(chip("soundcloud", &icons::SOUNDCLOUD, copy::WORKS_WITH_SOUNDCLOUD, p.soundcloud))
        .child(chip("reels", &icons::REELS, copy::WORKS_WITH_REELS, p.reels))
        .child(txt(copy::WORKS_WITH_TAIL, SMALL, p.text2))
}

// ── The bento ───────────────────────────────────────────────────────────────

/// A section of the sheet: a hairline across, the title on it, the reading under it. No
/// boxes: the figures sit on the page like a plotted chart.
fn cell(p: &Pal, title: &'static str, aside: Option<El>, body: El) -> El {
    let mut head = row().w_full().h(28.0).center_y().child(txt(title, CELL_TITLE, p.text)).child(spacer());
    if let Some(a) = aside {
        head = head.child(a);
    }
    col().w_full().pt(16.0).gap(16.0).border_sides([1.0, 0.0, 0.0, 0.0], p.line).child(head).child(body)
}

/// The summed speed of every running download, the last samples (`State.speed`).
fn speed(s: &State, p: &Pal) -> El {
    let samples: Vec<f32> = s.speed.iter().copied().collect();
    let now = format::total_speed(&s.tasks);
    let peak = samples.iter().copied().fold(0.0f32, f32::max).max(1.0) as f64;
    let scale = format::axis_ceiling(peak);
    let figures = col().w(112.0).gap(4.0).child(txt(copy::speed_now(&format::bytes(now)), FIG_L, p.text)).child(txt(
        copy::speed_scale(&format::bytes(scale)),
        FIG_S,
        p.muted,
    ));
    let chart = canvas(trace(&samples, scale as f32, 600.0, 56.0, p.line, p.text2).view_box(600.0, 56.0))
        .grow(1.0)
        .h(56.0)
        .label(copy::speed_aria(&format::bytes(now)));
    cell(p, copy::CELL_SPEED, None, row().w_full().gap(24.0).center_y().child(figures).child(chart))
}

fn this_month(p: &Pal, st: &DownloadStats) -> El {
    let cur = &st.current;
    let files = cur.files as u64;
    let prev = st.previous.as_ref().filter(|b| b.files > 0.0);
    let month = format::month_name(&cur.month);
    let body = if files == 0 {
        let second = format::month_sentence(st);
        col().gap(8.0).child(txt(copy::MONTH_NOTHING_YET, BODY, p.text)).child(txt(second, SMALL, p.text2).wrap())
    } else {
        let ending = format::month_sentence(st);
        let figure = row()
            .gap(8.0)
            .items_end()
            .child(txt(files.to_string(), FIG_XL, p.text))
            .child(txt(copy::file_word(files), BODY, p.text2).mb(8.0));
        let line = row().flex_wrap().center_y().child(txt(format::bytes(cur.bytes), FIG, p.text2)).child(txt(ending, SMALL, p.text2));
        let mut b = col().gap(16.0).child(col().gap(8.0).child(figure).child(line));
        if let Some(prev) = prev {
            let most = cur.files.max(prev.files);
            let bar = |name: String, n: f64, color: Color| {
                col()
                    .w_full()
                    .gap(8.0)
                    .child(row().w_full().child(txt(name, SMALL, p.text2)).child(spacer()).child(txt((n as u64).to_string(), FIG, p.text2)))
                    .child(
                        el().w_full()
                            .h(8.0)
                            .radius(4.0)
                            .fill(p.raised)
                            .child(el().w_frac(((n / most) as f32).max(0.03)).h_full().radius(4.0).fill(color)),
                    )
            };
            b = b.child(col().w_full().max_w(520.0).gap(12.0).child(bar(month.clone(), cur.files, p.text2)).child(bar(
                format::month_name(&prev.month),
                prev.files,
                p.track,
            )));
        }
        b
    };
    cell(p, copy::CELL_THIS_MONTH, Some(txt(month, SMALL, p.muted)), body).grow(1.0).min_w(0.0)
}

fn sources(p: &Pal, st: &DownloadStats) -> El {
    let total = st.current.files as u64;
    let ranked = format::source_split(st);
    let colour = |b: Bucket| match b {
        Bucket::Youtube => p.youtube,
        Bucket::Spotify => p.spotify,
        Bucket::Soundcloud => p.soundcloud,
        Bucket::Reels => p.reels,
        Bucket::Other => p.muted,
    };
    let body = if total == 0 {
        txt(copy::SPLIT_EMPTY, SMALL, p.text2).wrap()
    } else {
        let rows = ranked.iter().enumerate().map(|(i, (bucket, n))| {
            let (name, color) = (bucket.name(), colour(*bucket));
            let pct = ((*n as f64 / total as f64) * 100.0).round() as u64;
            let ink = if i == 0 { p.text } else { p.text2 };
            col()
                .w_full()
                .gap(8.0)
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .center_y()
                        .child(el().size(8.0, 8.0).radius(4.0).fill(color))
                        .child(txt(name, SMALL, ink))
                        .child(spacer())
                        .child(txt(copy::split_row_figures(pct, *n), FIG, ink)),
                )
                .child(
                    el().w_full()
                        .h(8.0)
                        .radius(4.0)
                        .fill(p.raised)
                        .child(el().w_frac((*n as f32 / total as f32).max(0.02)).h_full().radius(4.0).fill(color)),
                )
        });
        col().w_full().max_w(560.0).gap(12.0).children(rows)
    };
    let aside = (total > 0).then(|| txt(copy::split_aside(total), FIG, p.muted));
    cell(p, copy::CELL_WHERE_FROM, aside, body).grow(1.0).min_w(0.0)
}

// ── Recent downloads: the files that landed ─────────────────────────────────

fn recent(s: &State, p: &Pal, done: &[&DownloadTask]) -> El {
    let tone = crate::palette::roles(0, p.dark);
    let (on_media, scrim) = (tone.on_media, tone.media_scrim);
    let size = |t: &DownloadTask| format::bytes(t.progress.downloaded);
    let title = |t: &DownloadTask| if t.title.is_empty() { copy::RECENT_UNTITLED.to_string() } else { t.title.clone() };
    let duration_chip = |t: &DownloadTask| {
        let d = format::duration(t.duration);
        (!d.is_empty()).then(|| {
            row().h(20.0).px(4.0).center().radius(4.0).fill(scrim).child(txt(d, FIG_S, on_media)).pin(None, Some(8.0), Some(8.0), None)
        })
    };
    let mut tiles = row().w_full().gap(16.0);
    for (i, t) in done.iter().take(4).enumerate() {
        let tile = if i == 0 {
            let caption = col()
                .w_full()
                .px(16.0)
                .pt(28.0)
                .pb(12.0)
                .gap(4.0)
                .gradient(scrim.alpha(0.0), scrim)
                .pin(Some(0.0), None, Some(0.0), Some(0.0))
                .child(txt(title(t), BODY_M, on_media).wrap().max_lines(2))
                .child(txt(copy::recent_poster_meta(&t.format, &size(t), &t.uploader), FIG_S, mix(on_media, scrim, 0.25)).truncate());
            let mut art = el().w_full().h(176.0).radius(12.0).clip_rounded(12.0).fill(p.raised).child(cover_fill(s, p, t)).child(caption);
            if let Some(chip) = duration_chip(t) {
                art = art.child(chip);
            }
            col().grow(2.0).min_w(0.0).child(art)
        } else {
            let mut art = el().w_full().h(120.0).radius(12.0).clip_rounded(12.0).fill(p.raised).child(cover_fill(s, p, t));
            if let Some(chip) = duration_chip(t) {
                art = art.child(chip);
            }
            col().grow(1.0).min_w(0.0).gap(8.0).child(art).child(txt(title(t), SMALL_M, p.text).truncate()).child(
                row().w_full().child(txt(t.format.to_uppercase(), FIG_S, p.text2)).child(spacer()).child(txt(size(t), FIG_S, p.muted)),
            )
        };
        let id = t.task_id.clone();
        tiles = tiles.child(
            tile.key(format!("recent-{id}"))
                .cursor(rune::Cursor::Pointer)
                .help(copy::recent_details_aria(&title(t)))
                .on_press(|s: &mut State| actions::go(s, Nav::Downloads)),
        );
    }
    for _ in done.len().min(4)..4 {
        tiles = tiles.child(el().grow(1.0));
    }
    let view_all =
        button(p, "view-all", copy::CELL_RECENT_VIEW_ALL, None, Tone::Ghost).on_press(|s: &mut State| actions::go(s, Nav::Downloads));
    cell(p, copy::CELL_RECENT, Some(view_all), tiles)
}

/// A cover that fills its tile, or the source's colour as a flat tint when there is none.
fn cover_fill(s: &State, p: &Pal, t: &DownloadTask) -> El {
    match s.cover(t) {
        Some(_) => cover(s, p, t, 0.0, false).w_full().h_full().radius(0.0),
        None => el().w_full().h_full().fill(mix(model::source(p, t.source).1, p.card, 0.6)),
    }
}
