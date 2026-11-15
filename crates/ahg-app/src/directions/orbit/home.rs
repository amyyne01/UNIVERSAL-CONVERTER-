//! Home: the paste field is the launch point and the planet stands beside it, the product's
//! own mark at a size that means it.
use ahg_engine::stats::{DownloadStats, MonthBucket};
use ahg_engine::types::{DownloadStatus as St, DownloadTask};
use rune::{Color, El, Icon, ScrollSpec, canvas, col, el, icon, oklch, oklcha, row, spacer};

use super::draw::{planet, trace};
use super::look::{BODY, BODY_M, CELL_TITLE, DISPLAY, FIG, FIG_L, FIG_S, FIG_XL, Pal, SMALL, SMALL_M, mix, txt};
use super::model::{self, is_active};
use super::{Tone, button, cover, icons, keycap};
use crate::copy;
use crate::gate::{Nav, State};

/// The global hotkey as Electron stores it by default; split on `+` for its keycaps.
const HOTKEY: &str = "CommandOrControl+Shift+D";
/// The saved default audio format; `mp3` when unset.
const AUDIO_FORMAT: &str = "mp3";

pub fn view(s: &State, p: &Pal) -> El {
    let active: Vec<&DownloadTask> = s.tasks.iter().filter(|t| is_active(t)).collect();
    let done: Vec<&DownloadTask> = s.tasks.iter().filter(|t| t.progress.status == St::Done).collect();
    let counted = s.stats.as_ref().is_some_and(|st| st.current.files > 0.0 || st.previous.as_ref().is_some_and(|p| p.files > 0.0));
    let quiet = !counted && done.is_empty() && active.is_empty();

    let planet_w = if quiet { 380.0 } else { 260.0 };
    let mark = canvas(planet(planet_w, mix(p.text, p.bg, 0.08), p.bg)).key("planet").size(planet_w, planet_w * 0.592);

    let mut launch = col().grow(1.0).min_w(0.0).gap(20.0).child(headline(p, active.len())).child(field(p, &s.input)).child(status(s, p));
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
        bento = bento.child(row().w_full().gap(48.0).items_start().child(this_month(p, stats)).child(sources(p, &stats.current)));
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
                .on_press(|s: &mut State| s.nav = Nav::Downloads)
                .child(el().size(8.0, 8.0).radius(4.0).fill(p.text2))
                .child(row().center_y().child(txt(in_flight.to_string(), FIG, p.text)).child(txt(copy::HOME_IN_FLIGHT_SUFFIX, SMALL, p.text2)))
                .child(icon(&icons::FORWARD, 16.0).color(p.text2)),
        );
    }
    r
}

fn field(p: &Pal, input: &str) -> El {
    let go = row()
        .key("look-up")
        .h(44.0)
        .px(16.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.accent)
        .hover(mix(p.accent, p.text, 0.15))
        .press_scale(0.97)
        .role("button")
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .child(txt(copy::FIELD_BUTTON_LOOK_UP, BODY_M, p.on_accent))
        .child(icon(&icons::FORWARD, 16.0).color(p.on_accent));
    row()
        .key("paste")
        .w_full()
        .h(64.0)
        .pl(20.0)
        .pr(12.0)
        .gap(16.0)
        .center_y()
        .radius(16.0)
        .fill(p.card)
        .border(1.0, p.track)
        .label(copy::FIELD_ARIA)
        .child(icon(&icons::LINK, 16.0).color(p.muted))
        .child(if input.is_empty() { txt(copy::FIELD_PLACEHOLDER, BODY.lh(22.0), p.muted) } else { txt(input.to_string(), BODY.lh(22.0), p.text) }.grow(1.0).truncate())
        .child(go)
}

fn status(s: &State, p: &Pal) -> El {
    let mut line = row().w_full().gaps(8.0, 24.0).center_y().flex_wrap();
    if let Some(folder) = s.tasks.first().map(|t| t.output_dir.clone()).filter(|f| !f.is_empty()) {
        // Only `/` separates here, as in Electron: a Windows path prints in full.
        let name = folder.rsplit('/').next().unwrap_or(&folder).to_string();
        line = line.child(row()
            .center_y()
            .child(txt(copy::rest_saves_as(AUDIO_FORMAT), SMALL, p.text2))
            .child(txt(name, SMALL_M, p.text).help(folder))
            .child(txt(" ", SMALL, p.text2))
            .child(txt(copy::REST_CHANGE, SMALL_M, p.text2).label(copy::REST_CHANGE_ARIA).cursor(rune::Cursor::Pointer)));
    }
    // The strings carry their own spaces, so the pieces sit flush.
    let mut right = row()
        .center_y()
        .child(keycap(p, copy::KEY_CTRL))
        .child(txt(copy::REST_PLUS_SPACED, SMALL, p.muted))
        .child(keycap(p, copy::REST_KEY_V))
        .child(txt(copy::REST_PASTES_A_LINK, SMALL, p.text2))
        .child(txt(copy::REST_HOTKEY_LEAD, SMALL, p.text2));
    for (i, part) in HOTKEY.split('+').enumerate() {
        if i > 0 {
            right = right.child(txt("+", SMALL, p.muted).mx(4.0));
        }
        right = right.child(keycap(p, if part == "CommandOrControl" { copy::KEY_CTRL } else { part }));
    }
    right = right.child(txt(copy::REST_HOTKEY_TRAIL, SMALL, p.text2));
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

/// The summed speed of every running download over the last two minutes, rebuilt from when
/// each sample download started (the gate has no live samples).
fn speed(s: &State, p: &Pal) -> El {
    let running: Vec<&DownloadTask> = s.tasks.iter().filter(|t| t.progress.status == St::Downloading).collect();
    let now_ms = s.tasks.iter().filter_map(|t| t.created_at).fold(0.0, f64::max) + 12_000.0;
    let samples: Vec<f32> = (0..120)
        .map(|i| {
            let ago = (119 - i) as f64;
            running
                .iter()
                .map(|t| {
                    let age = (now_ms - t.created_at.unwrap_or(now_ms)) / 1000.0;
                    let ramp = ((age - ago) / 4.0).clamp(0.0, 1.0);
                    let wobble = 1.0 + 0.06 * ((i as f64) * 0.7 + age).sin() + 0.03 * ((i as f64) * 2.3).cos();
                    t.progress.speed * ramp * wobble
                })
                .sum::<f64>() as f32
        })
        .collect();
    let now: f64 = running.iter().map(|t| t.progress.speed).sum();
    let peak = samples.iter().copied().fold(0.0f32, f32::max).max(1.0) as f64;
    let scale = (0..16).flat_map(|k| [1.0, 2.0, 5.0].map(|m| m * 10f64.powi(k))).find(|v| *v >= peak).unwrap_or(peak);
    let figures = col().w(112.0).gap(4.0).child(txt(copy::speed_now(&copy::format_bytes(now)), FIG_L, p.text)).child(txt(copy::speed_scale(&copy::format_bytes(scale)), FIG_S, p.muted));
    let chart = canvas(trace(&samples, scale as f32, 600.0, 56.0, p.line, p.text2).view_box(600.0, 56.0)).grow(1.0).h(56.0).label(copy::speed_aria(&copy::format_bytes(now)));
    cell(p, copy::CELL_SPEED, None, row().w_full().gap(24.0).center_y().child(figures).child(chart))
}

fn this_month(p: &Pal, st: &DownloadStats) -> El {
    let cur = &st.current;
    let files = cur.files as u64;
    let prev = st.previous.as_ref().filter(|b| b.files > 0.0);
    let month = model::month_name(&cur.month);
    let body = if files == 0 {
        let second = match prev {
            Some(b) => copy::month_last_month_kept(b.files as u64, &copy::format_bytes(b.bytes)),
            None => copy::MONTH_NOTHING_YET_NO_PREV.to_string(),
        };
        col().gap(8.0).child(txt(copy::MONTH_NOTHING_YET, BODY, p.text)).child(txt(second, SMALL, p.text2).wrap())
    } else {
        let ending = match prev {
            Some(b) if b.files as u64 == files => copy::month_same_count(&model::month_name(&b.month)),
            Some(b) => copy::month_diff(files as i64 - b.files as i64, &model::month_name(&b.month)),
            None => copy::MONTH_FIRST_ON_RECORD.to_string(),
        };
        let figure = row().gap(8.0).items_end().child(txt(files.to_string(), FIG_XL, p.text)).child(txt(copy::file_word(files), BODY, p.text2).mb(8.0));
        let line = row().flex_wrap().center_y().child(txt(copy::format_bytes(cur.bytes), FIG, p.text2)).child(txt(ending, SMALL, p.text2));
        let mut b = col().gap(16.0).child(col().gap(8.0).child(figure).child(line));
        if let Some(prev) = prev {
            let most = cur.files.max(prev.files);
            let bar = |name: String, n: f64, color: Color| {
                col()
                    .w_full()
                    .gap(8.0)
                    .child(row().w_full().child(txt(name, SMALL, p.text2)).child(spacer()).child(txt((n as u64).to_string(), FIG, p.text2)))
                    .child(el().w_full().h(8.0).radius(4.0).fill(p.raised).child(el().w_frac(((n / most) as f32).max(0.03)).h_full().radius(4.0).fill(color)))
            };
            b = b.child(col().w_full().max_w(520.0).gap(12.0).child(bar(month.clone(), cur.files, p.text2)).child(bar(model::month_name(&prev.month), prev.files, p.track)));
        }
        b
    };
    cell(p, copy::CELL_THIS_MONTH, Some(txt(month, SMALL, p.muted)), body).grow(1.0).min_w(0.0)
}

fn sources(p: &Pal, cur: &MonthBucket) -> El {
    let total = cur.files as u64;
    let mut groups: [(&'static str, Color, u64); 5] = [(copy::SPLIT_YOUTUBE, p.youtube, 0), (copy::SPLIT_SPOTIFY, p.spotify, 0), (copy::SPLIT_SOUNDCLOUD, p.soundcloud, 0), (copy::SPLIT_REELS, p.reels, 0), (copy::SPLIT_OTHER, p.muted, 0)];
    for (key, n) in &cur.by_platform {
        let at = match key.as_str() {
            "youtube" => 0,
            "spotify" => 1,
            "soundcloud" => 2,
            "instagram" | "tiktok" | "facebook" => 3,
            _ => 4,
        };
        groups[at].2 += n.as_f64().unwrap_or(0.0) as u64;
    }
    let mut ranked: Vec<_> = groups.into_iter().filter(|g| g.2 > 0).collect();
    ranked.sort_by(|a, b| b.2.cmp(&a.2));
    let body = if total == 0 {
        txt(copy::SPLIT_EMPTY, SMALL, p.text2).wrap()
    } else {
        let rows = ranked.iter().enumerate().map(|(i, (name, color, n))| {
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
                        .child(el().size(8.0, 8.0).radius(4.0).fill(*color))
                        .child(txt(*name, SMALL, ink))
                        .child(spacer())
                        .child(txt(copy::split_row_figures(pct, *n), FIG, ink)),
                )
                .child(el().w_full().h(8.0).radius(4.0).fill(p.raised).child(el().w_frac((*n as f32 / total as f32).max(0.02)).h_full().radius(4.0).fill(*color)))
        });
        col().w_full().max_w(560.0).gap(12.0).children(rows)
    };
    let aside = (total > 0).then(|| txt(copy::split_aside(total), FIG, p.muted));
    cell(p, copy::CELL_WHERE_FROM, aside, body).grow(1.0).min_w(0.0)
}

// ── Recent downloads: the files that landed ─────────────────────────────────

fn recent(s: &State, p: &Pal, done: &[&DownloadTask]) -> El {
    let on_media = oklch(0.955, 0.008, 220.0);
    let scrim = oklcha(0.12, 0.012, 240.0, 0.88);
    let size = |t: &DownloadTask| copy::format_bytes(t.progress.downloaded);
    let title = |t: &DownloadTask| if t.title.is_empty() { copy::RECENT_UNTITLED.to_string() } else { t.title.clone() };
    let duration_chip = |t: &DownloadTask| {
        let d = copy::format_duration(t.duration);
        (!d.is_empty()).then(|| row().h(20.0).px(4.0).center().radius(4.0).fill(scrim).child(txt(d, FIG_S, on_media)).pin(None, Some(8.0), Some(8.0), None))
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
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(8.0)
                .child(art)
                .child(txt(title(t), SMALL_M, p.text).truncate())
                .child(row().w_full().child(txt(t.format.to_uppercase(), FIG_S, p.text2)).child(spacer()).child(txt(size(t), FIG_S, p.muted)))
        };
        tiles = tiles.child(tile.key(format!("recent-{}", t.task_id)).cursor(rune::Cursor::Pointer).help(copy::recent_details_aria(&title(t))));
    }
    for _ in done.len().min(4)..4 {
        tiles = tiles.child(el().grow(1.0));
    }
    let view_all = button(p, "view-all", copy::CELL_RECENT_VIEW_ALL, None, Tone::Ghost).on_press(|s: &mut State| s.nav = Nav::Downloads);
    cell(p, copy::CELL_RECENT, Some(view_all), tiles)
}

/// A cover that fills its tile, or the source's colour as a flat tint when there is none.
fn cover_fill(s: &State, p: &Pal, t: &DownloadTask) -> El {
    match s.cover(&t.task_id) {
        Some(_) => cover(s, p, t, 0.0, false).w_full().h_full().radius(0.0),
        None => el().w_full().h_full().fill(mix(model::source(p, t.source).1, p.card, 0.6)),
    }
}
