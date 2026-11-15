//! Home: the desk's input. The paste field is the jack everything goes into;
//! the month's numbers are set like a counter, and every share is a meter.
use ahg_engine::stats::{DownloadStats, MonthBucket};
use ahg_engine::types::{DownloadStatus as St, DownloadTask, SourcePlatform};
use rune::{Color, El, ScrollSpec, col, el, icon, row, spacer};

use super::icons;
use super::model;
use super::parts::{Line, art, dot, keycap, label, meter, mono, platform_icon, sans, stack};
use super::theme::{Pal, source_color};
use crate::copy;
use crate::gate::{Found, Nav, State};
use model::{AUDIO_FORMAT, OUTPUT_DIR};

pub fn view(s: &State, p: &Pal) -> El {
    let active = model::active_count(&s.tasks);
    let done: Vec<&DownloadTask> = s.tasks.iter().filter(|t| t.progress.status == St::Done).take(7).collect();
    let has_counts = s.stats.as_ref().is_some_and(|st| st.current.files > 0.0 || st.previous.as_ref().is_some_and(|m| m.files > 0.0));
    let quiet = !has_counts && done.is_empty() && active == 0;

    if quiet {
        // First launch: the field is the whole screen, set a little above the middle.
        let hero = col()
            .w_full()
            .max_w(880.0)
            .gap(20.0)
            .child(label(copy::HOME_HEADLINE, sans(44.0, 500.0).tracking(-0.9), p.text))
            .child(stack(12.0).child(field(s, p)).child(status(p)))
            .child(works_with(p).mt(28.0));
        return col().w_full().h_full().px(40.0).pb(72.0).items_center().justify_center().child(hero);
    }

    let mut headline = row().w_full().center_y().justify_between().child(label(copy::HOME_HEADLINE, sans(30.0, 500.0).tracking(-0.5), p.text));
    if active > 0 {
        headline = headline.child(in_flight(active, p));
    }
    // What the field found takes the body's place, under the field it came out of.
    if let Some(found) = super::found::view(s, p) {
        let page = col().w_full().max_w(1600.0).h_full().min_h(0.0).px(40.0).pt(36.0).pb(40.0).gap(16.0).child(headline).child(stack(12.0).mt(4.0).child(field(s, p)).child(status(p))).child(found.mt(8.0));
        return col().w_full().h_full().min_h(0.0).items_center().child(page);
    }
    let mut page = col().w_full().max_w(1600.0).px(40.0).pt(36.0).pb(40.0).gap(16.0).child(headline).child(stack(12.0).mt(4.0).child(field(s, p)).child(status(p)));
    let mut bento = col().w_full().gap(16.0).mt(16.0);
    if active > 0 {
        bento = bento.child(speed_cell(&s.tasks, p));
    }
    bento = bento.child(row().w_full().gap(16.0).child(month_cell(s.stats.as_ref(), p).grow(3.0)).child(split_cell(s.stats.as_ref(), p).grow(2.0)));
    if !done.is_empty() {
        bento = bento.child(recent_cell(s, &done, p));
    }
    page = page.child(bento);
    col().w_full().h_full().scroll(ScrollSpec { stick_bottom: false }).min_h(0.0).items_center().child(page)
}

fn in_flight(n: usize, p: &Pal) -> El {
    row()
        .h(32.0)
        .pl(12.0)
        .pr(12.0)
        .gap(8.0)
        .center_y()
        .radius(16.0)
        .border(1.0, p.line_strong)
        .hover(p.hover)
        .press(p.press)
        .cursor(rune::Cursor::Pointer)
        .focusable(true)
        .role("button")
        .child(dot(p.accent, 8.0))
        .child(Line::new().fig(&n.to_string(), p.text).add(copy::HOME_IN_FLIGHT_SUFFIX, p.text2).one(sans(13.0, 500.0)))
        .child(icon(&icons::FORWARD, 16.0).color(p.muted))
        .on_press(|s: &mut State| s.nav = Nav::Downloads)
}

/// The input of the desk: a jack, the line, the one filled button, and under
/// them the input's level, every segment dark until a link comes in.
fn field(s: &State, p: &Pal) -> El {
    let input = s.input.as_str();
    let words = if !input.is_empty() && !input.contains("://") { copy::FIELD_BUTTON_SEARCH } else { copy::FIELD_BUTTON_LOOK_UP };
    // A failed lookup leaves the input's level red, as the old field's strip did.
    let (lit, level_color) = if matches!(s.found, Found::LookupFailed(_)) { (1.0, p.error.alpha(0.6)) } else { (0.0, p.accent) };
    let jack = el().size(40.0, 40.0).radius(8.0).fill(p.deep).border(1.0, p.line).center().child(icon(&icons::LINK, 16.0).color(if input.is_empty() { p.muted } else { p.accent_ink }));
    let go = row()
        .h(44.0)
        .pl(16.0)
        .pr(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.text)
        .hover(p.bg.alpha(0.12))
        .press(p.bg.alpha(0.2))
        .cursor(rune::Cursor::Pointer)
        .focusable(true)
        .role("button")
        .child(label(words, sans(14.0, 500.0), p.bg))
        .child(icon(&icons::FORWARD, 16.0).color(p.bg));
    let typed = if input.is_empty() { label(copy::FIELD_PLACEHOLDER, sans(16.0, 400.0), p.muted) } else { label(input.to_string(), sans(16.0, 400.0), p.text) };
    let line = row().w_full().h(64.0).pl(12.0).pr(12.0).gap(12.0).center_y().child(jack).child(typed.grow(1.0).single_line().truncate().min_w(0.0)).child(go);
    let level = el().w_full().h(16.0).px(12.0).pb(8.0).child(meter(lit, 120, None, 8.0, 2.0, level_color, p.unlit));
    col()
        .w_full()
        .radius(12.0)
        .fill(p.surface)
        .border(1.0, p.line_strong)
        .role("textbox")
        .label(copy::FIELD_ARIA)
        .child(line)
        .child(level)
}

/// The rest-state status line (reading 8).
fn status(p: &Pal) -> El {
    let left = row()
        .gap(8.0)
        .center_y()
        .min_w(0.0)
        .child(Line::new().add(&copy::rest_saves_as(AUDIO_FORMAT), p.text2).add(OUTPUT_DIR, p.text).one(sans(13.0, 400.0)).help(OUTPUT_DIR))
        .child(label(copy::REST_CHANGE, sans(13.0, 500.0), p.accent_ink).cursor(rune::Cursor::Pointer).role("link").label(copy::REST_CHANGE_ARIA));
    let right = row()
        .gap(0.0)
        .center_y()
        .child(keycap(copy::KEY_CTRL, p))
        .child(label(copy::REST_PLUS_SPACED, sans(13.0, 400.0), p.muted))
        .child(keycap(copy::REST_KEY_V, p))
        .child(label(copy::REST_PASTES_A_LINK, sans(13.0, 400.0), p.text2));
    row().w_full().px(4.0).gap(16.0).center_y().justify_between().child(left).child(right)
}

fn works_with(p: &Pal) -> El {
    let chip = |source: SourcePlatform, name: &'static str| {
        row()
            .h(32.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(8.0)
            .border(1.0, p.line)
            .hover(p.hover)
            .press(p.press)
            .cursor(rune::Cursor::Pointer)
            .focusable(true)
            .role("button")
            .child(icon(platform_icon(source), 16.0).color(source_color(p, source)))
            .child(label(name, sans(13.0, 500.0), p.text))
    };
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .flex_wrap()
        .child(label(copy::WORKS_WITH, sans(13.0, 400.0), p.text2).mr(4.0))
        .child(chip(SourcePlatform::Youtube, copy::WORKS_WITH_YOUTUBE))
        .child(chip(SourcePlatform::Spotify, copy::WORKS_WITH_SPOTIFY))
        .child(chip(SourcePlatform::Soundcloud, copy::WORKS_WITH_SOUNDCLOUD))
        .child(chip(SourcePlatform::Instagram, copy::WORKS_WITH_REELS))
        .child(label(copy::WORKS_WITH_TAIL, sans(13.0, 400.0), p.text2).ml(4.0))
}

fn cell(title: &str, aside: Option<El>, body: El, p: &Pal) -> El {
    let mut head = row().w_full().center_y().child(label(title, sans(16.0, 500.0), p.text)).child(spacer());
    if let Some(aside) = aside {
        head = head.child(aside);
    }
    col().radius(12.0).fill(p.surface).border(1.0, p.line).padding(20.0).gap(16.0).min_w(0.0).child(head).child(body)
}

/// The next of 1, 2, 5 times a power of ten at or above `v`.
fn scale_for(v: f64) -> f64 {
    let mut step = 1.0;
    loop {
        for k in [1.0, 2.0, 5.0] {
            if k * step >= v {
                return k * step;
            }
        }
        step *= 10.0;
    }
}

fn speed_cell(tasks: &[DownloadTask], p: &Pal) -> El {
    let now = model::total_speed(tasks);
    let ceil = scale_for(now.max(1.0));
    let figure = stack(4.0)
        .w(160.0)
        .child(label(copy::CELL_SPEED, sans(16.0, 500.0), p.text).mb(8.0))
        .child(label(copy::speed_now(&copy::format_bytes(now)), mono(22.0, 500.0), p.text))
        .child(label(copy::speed_scale(&copy::format_bytes(ceil)), mono(11.0, 400.0), p.muted));
    let level = meter(now / ceil, 64, None, 16.0, 3.0, p.accent, p.unlit);
    row()
        .w_full()
        .radius(12.0)
        .fill(p.surface)
        .border(1.0, p.line)
        .padding(20.0)
        .gap(24.0)
        .center_y()
        .label(copy::speed_aria(&copy::format_bytes(now)))
        .child(figure)
        .child(col().grow(1.0).min_w(0.0).child(level))
}

fn month_cell(stats: Option<&DownloadStats>, p: &Pal) -> El {
    let Some(st) = stats else {
        return cell(copy::CELL_THIS_MONTH, None, label(copy::MONTH_FAILED, sans(13.0, 400.0), p.text2), p);
    };
    let cur = &st.current;
    let prev = st.previous.as_ref().filter(|m| m.files > 0.0);
    let aside = label(model::month_name(&cur.month), sans(13.0, 400.0), p.muted);
    let files = cur.files.max(0.0) as u64;
    let body = if files == 0 {
        let second = match prev {
            Some(m) => copy::month_last_month_kept(m.files as u64, &copy::format_bytes(m.bytes)),
            None => copy::MONTH_NOTHING_YET_NO_PREV.to_string(),
        };
        stack(8.0).child(label(copy::MONTH_NOTHING_YET, sans(16.0, 500.0), p.text)).child(label(second, sans(13.0, 400.0), p.text2))
    } else {
        let ending = match prev {
            Some(m) => {
                let diff = files as i64 - m.files as i64;
                let name = model::month_name(&m.month);
                if diff == 0 { copy::month_same_count(&name) } else { copy::month_diff(diff, &name) }
            }
            None => copy::MONTH_FIRST_ON_RECORD.to_string(),
        };
        let counter = row()
            .gap(12.0)
            .items_end()
            .child(label(files.to_string(), mono(60.0, 400.0).tracking(-2.0).lh(60.0), p.text))
            .child(label(copy::file_word(files), sans(16.0, 400.0), p.text2).pb(8.0));
        let mut b = stack(16.0).child(stack(12.0).child(counter).child(Line::new().fig(&copy::format_bytes(cur.bytes), p.text).add(&ending, p.text2).wrap(sans(13.0, 400.0))));
        if let Some(m) = prev {
            let top = cur.files.max(m.files);
            b = b.child(
                stack(12.0)
                    .max_w(520.0)
                    .label(copy::month_bars_aria(&model::month_name(&cur.month), files, &model::month_name(&m.month), m.files as u64))
                    .child(month_bar(cur, top, p.accent, p))
                    .child(month_bar(m, top, p.muted.alpha(0.7), p)),
            );
        }
        b
    };
    cell(copy::CELL_THIS_MONTH, Some(aside), body, p)
}

fn month_bar(m: &MonthBucket, top: f64, color: Color, p: &Pal) -> El {
    let frac = if top > 0.0 { (m.files / top).max(0.03) } else { 0.0 };
    stack(8.0)
        .w_full()
        .child(row().w_full().child(label(model::month_name(&m.month), sans(12.0, 400.0), p.text2)).child(spacer()).child(label((m.files as u64).to_string(), mono(12.0, 500.0), p.text)))
        .child(meter(frac, 48, None, 8.0, 2.0, color, p.unlit))
}

/// Source groups: the four rail tabs plus Other, biggest first.
fn split(by: &serde_json::Map<String, serde_json::Value>) -> Vec<(Option<SourcePlatform>, &'static str, u64)> {
    let mut groups: Vec<(Option<SourcePlatform>, &'static str, u64)> = vec![
        (Some(SourcePlatform::Youtube), copy::SPLIT_YOUTUBE, 0),
        (Some(SourcePlatform::Spotify), copy::SPLIT_SPOTIFY, 0),
        (Some(SourcePlatform::Soundcloud), copy::SPLIT_SOUNDCLOUD, 0),
        (Some(SourcePlatform::Instagram), copy::SPLIT_REELS, 0),
        (None, copy::SPLIT_OTHER, 0),
    ];
    for (key, v) in by {
        let n = v.as_f64().unwrap_or(0.0).max(0.0) as u64;
        let at = match key.as_str() {
            "youtube" => 0,
            "spotify" => 1,
            "soundcloud" => 2,
            "instagram" | "tiktok" | "facebook" => 3,
            _ => 4,
        };
        groups[at].2 += n;
    }
    groups.retain(|g| g.2 > 0);
    groups.sort_by(|a, b| b.2.cmp(&a.2));
    groups
}

fn split_cell(stats: Option<&DownloadStats>, p: &Pal) -> El {
    let Some(st) = stats else {
        return cell(copy::CELL_WHERE_FROM, None, label(copy::SPLIT_FAILED, sans(13.0, 400.0), p.text2), p);
    };
    let groups = split(&st.current.by_platform);
    let total: u64 = groups.iter().map(|g| g.2).sum();
    if total == 0 {
        return cell(copy::CELL_WHERE_FROM, None, label(copy::SPLIT_EMPTY, sans(13.0, 400.0), p.text2).wrap(), p);
    }
    let mut rows = stack(16.0).max_w(560.0);
    for (i, (source, name, n)) in groups.iter().enumerate() {
        let c = source.map_or(p.muted, |s| source_color(p, s));
        let lead = i == 0;
        let pct = (*n as f64 / total as f64 * 100.0).round() as u64;
        let figures = copy::split_row_figures(pct, *n);
        let (pct_part, count_part) = figures.split_at(figures.find(' ').unwrap_or(figures.len()));
        rows = rows.child(
            stack(8.0)
                .w_full()
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .center_y()
                        .child(dot(c, 6.0))
                        .child(label(*name, sans(13.0, if lead { 500.0 } else { 400.0 }), if lead { p.text } else { p.text2 }))
                        .child(spacer())
                        .child(Line::new().fig(pct_part, if lead { p.text } else { p.text2 }).fig(count_part, p.muted).one(mono(12.0, 400.0))),
                )
                .child(meter((*n as f64 / total as f64).max(0.02), 40, None, 4.0, 2.0, c, p.unlit)),
        );
    }
    cell(copy::CELL_WHERE_FROM, Some(label(copy::split_aside(total), sans(13.0, 400.0), p.muted)), rows, p)
}

fn recent_cell(s: &State, done: &[&DownloadTask], p: &Pal) -> El {
    let view_all = row()
        .gap(8.0)
        .center_y()
        .cursor(rune::Cursor::Pointer)
        .role("button")
        .child(label(copy::CELL_RECENT_VIEW_ALL, sans(13.0, 500.0), p.text2))
        .child(icon(&icons::FORWARD, 16.0).color(p.text2))
        .on_press(|s: &mut State| s.nav = Nav::Downloads);
    // Under 1500 points of content: five columns, the poster takes two, four tiles at most.
    let mut grid = row().w_full().gap(12.0).items_start();
    let shown = done.len().min(4);
    for (i, t) in done.iter().take(shown).enumerate() {
        grid = grid.child(if i == 0 { poster(s, t, p) } else { tile(s, t, p) });
    }
    for _ in shown..4 {
        grid = grid.child(el().grow(1.0));
    }
    cell(copy::CELL_RECENT, Some(view_all), grid, p)
}

fn title_of(t: &DownloadTask) -> &str {
    if t.title.trim().is_empty() { copy::RECENT_UNTITLED } else { &t.title }
}

fn duration_chip(t: &DownloadTask, p: &Pal) -> Option<El> {
    (t.duration > 0.0).then(|| el().h(20.0).px(8.0).center().radius(4.0).fill(p.media_scrim).child(label(copy::format_duration(t.duration), mono(11.0, 500.0), p.on_media)).pin(None, Some(8.0), Some(8.0), None))
}

const ART_H: f32 = 104.0;

fn poster(s: &State, t: &DownloadTask, p: &Pal) -> El {
    let caption = stack(4.0)
        .padding(12.0)
        .fill(p.media_scrim)
        .pin(Some(0.0), None, Some(0.0), Some(0.0))
        .child(label(title_of(t), sans(14.0, 500.0), p.on_media).single_line().truncate())
        .child(label(copy::recent_poster_meta(&t.format, &copy::format_bytes(t.progress.downloaded), &t.uploader), mono(11.0, 400.0), p.on_media.alpha(0.78)).single_line().truncate());
    let mut a = art(s, t, None, ART_H + 46.0, 10.0, p).clip_rounded(10.0).child(caption);
    if let Some(chip) = duration_chip(t, p) {
        a = a.child(chip);
    }
    col().grow(2.0).min_w(0.0).hover_group().cursor(rune::Cursor::Pointer).child(a)
}

fn tile(s: &State, t: &DownloadTask, p: &Pal) -> El {
    let mut a = art(s, t, None, ART_H, 8.0, p);
    if let Some(chip) = duration_chip(t, p) {
        a = a.child(chip);
    }
    stack(8.0)
        .grow(1.0)
        .cursor(rune::Cursor::Pointer)
        .child(a)
        .child(label(title_of(t), sans(13.0, 500.0), p.text).single_line().truncate().min_w(0.0))
        .child(row().w_full().child(label(t.format.to_uppercase(), mono(11.0, 400.0), p.muted)).child(spacer()).child(label(copy::format_bytes(t.progress.downloaded), mono(11.0, 400.0), p.muted)))
}
