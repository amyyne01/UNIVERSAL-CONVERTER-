//! Home: the paste box, the month in figures, and the latest files as sleeves
//! standing on a shelf.
use ahg_engine::config::default_config;
use ahg_engine::stats::DownloadStats;
use ahg_engine::types::{DownloadStatus as St, DownloadTask};
use rune::{Color, El, Icon, ScrollSpec, col, el, icon, row, text};

use super::icons;
use super::parts::{self, Disc, keycap, run, run_mono, runs};
use super::theme::{Pal, label_paper, mono, sans, serif, vinyl};
use crate::copy;
use crate::gate::{Screen, State};

pub fn view(s: &State, p: &Pal, active: usize) -> El {
    let finished: Vec<&DownloadTask> = s
        .tasks
        .iter()
        .filter(|t| t.progress.status == St::Done)
        .collect();
    let counted = s.stats.as_ref().is_some_and(|st| {
        st.current.files > 0.0 || st.previous.as_ref().is_some_and(|b| b.files > 0.0)
    });
    let quiet = !counted && finished.is_empty() && active == 0;

    let top = col()
        .w_full()
        .gap(16.0)
        .child(headline(p, active))
        .child(field(p, &s.input))
        .child(status_line(s, p));
    if quiet {
        // First launch: the field is the whole screen, set a little above the middle.
        return col()
            .key("home")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .justify_center()
            .px(56.0)
            .pb(96.0)
            .child(
                col()
                    .w_full()
                    .max_w(880.0)
                    .self_center()
                    .gap(32.0)
                    .child(top)
                    .child(works_with(p))
                    .child(empty_shelf(p)),
            );
    }
    let mut page = col()
        .w_full()
        .max_w(1480.0)
        .self_center()
        .px(40.0)
        .pt(28.0)
        .pb(40.0)
        .gap(24.0)
        .child(top);
    if active > 0 {
        page = page.child(speed(s, p));
    }
    if let Some(stats) = &s.stats {
        page = page.child(
            row()
                .w_full()
                .gap(0.0)
                .items_stretch()
                .child(this_month(p, stats))
                .child(el().w(1.0).fill(p.line_soft).mx(32.0))
                .child(where_from(p, stats)),
        );
    }
    if !finished.is_empty() {
        page = page.child(shelf(s, p, &finished));
    }
    col()
        .key("home")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(page)
}

// ── The top: headline, field, status ───────────────────────────────────────

fn headline(p: &Pal, active: usize) -> El {
    let mut r = row().w_full().items_end().child(
        text(copy::HOME_HEADLINE)
            .font(serif(42.0).lh(48.0))
            .color(p.text),
    );
    if active > 0 {
        let pill = row()
            .key("in-flight")
            .h(32.0)
            .pl(12.0)
            .pr(12.0)
            .gap(8.0)
            .mb(6.0)
            .center_y()
            .radius(16.0)
            .border(1.0, p.line)
            .hover(p.raised)
            .focusable(true)
            .role("button")
            .on_press(|s: &mut State| s.screen = Screen::Downloads)
            .child(el().size(6.0, 6.0).radius(3.0).fill(p.accent))
            .child(
                runs(vec![
                    run_mono(active.to_string(), p.text),
                    run(copy::HOME_IN_FLIGHT_SUFFIX, p.text2),
                ])
                .font(sans(13.0, 400.0))
                .single_line(),
            )
            .child(icon(&icons::FORWARD, 16.0).color(p.muted));
        r = r.child(el().grow(1.0)).child(pill);
    }
    r
}

fn field(p: &Pal, input: &str) -> El {
    let (shown, ink) = if input.is_empty() {
        (copy::FIELD_PLACEHOLDER, p.muted)
    } else {
        (input, p.text)
    };
    let button = row()
        .key("look-up")
        .h(48.0)
        .pl(20.0)
        .pr(16.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.text)
        .hover(p.text2)
        .focusable(true)
        .role("button")
        .label(copy::FIELD_BUTTON_LOOK_UP)
        .child(
            text(copy::FIELD_BUTTON_LOOK_UP)
                .font(sans(14.0, 500.0))
                .color(p.bg),
        )
        .child(icon(&icons::FORWARD, 16.0).color(p.bg));
    row()
        .key("paste")
        .w_full()
        .h(64.0)
        .pl(24.0)
        .pr(8.0)
        .gap(16.0)
        .center_y()
        .radius(16.0)
        .fill(p.card)
        .border(1.0, p.control.alpha(0.55))
        .role("textbox")
        .label(copy::FIELD_ARIA)
        .child(icon(&icons::LINK, 20.0).color(p.muted))
        .child(
            text(shown)
                .font(sans(17.0, 400.0))
                .color(ink)
                .grow(1.0)
                .single_line()
                .truncate(),
        )
        .child(button)
}

/// The rest-state caption: where files go, and the two ways to paste.
fn status_line(s: &State, p: &Pal) -> El {
    let config = default_config();
    let setting = |k: &str| {
        config
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    // ponytail: the gate has no settings, so the folder is the one the sample downloads went to, else the
    // engine's default (empty, so the folder part is left out); the format and hotkey are the engine's defaults.
    let folder = s
        .tasks
        .first()
        .map(|t| t.output_dir.clone())
        .unwrap_or_else(|| setting("outputDir"));
    let format = setting("defaultFormat");
    let hotkey = setting("globalHotkey");

    let mut left = row().gap(0.0).center_y().flex_wrap();
    if !folder.is_empty() {
        // Only `/` separates here, so a Windows path prints in full (CONTENT.md 2.2).
        let shown = folder.rsplit('/').next().unwrap_or(&folder).to_string();
        left = left
            .child(
                runs(vec![
                    run(
                        copy::rest_saves_as(if format.is_empty() { "mp3" } else { &format }),
                        p.text2,
                    ),
                    run(shown, p.text),
                ])
                .font(sans(13.0, 400.0))
                .single_line()
                .help(folder.clone()),
            )
            .child(
                text(copy::REST_CHANGE)
                    .key("change-folder")
                    .font(sans(13.0, 500.0))
                    .color(p.accent)
                    .ml(8.0)
                    .focusable(true)
                    .role("button")
                    .label(copy::REST_CHANGE_ARIA),
            );
    }
    let quiet = |t: &str| text(t).font(sans(13.0, 400.0)).color(p.muted).single_line();
    let mut right = row()
        .gap(4.0)
        .center_y()
        .child(keycap(p, copy::KEY_CTRL))
        .child(quiet(copy::REST_PLUS_SPACED.trim()))
        .child(keycap(p, copy::REST_KEY_V));
    if hotkey.is_empty() {
        right = right.child(quiet(copy::REST_PASTES_A_LINK));
    } else {
        right = right.child(quiet(&format!(
            "{}{}",
            copy::REST_PASTES_A_LINK,
            copy::REST_HOTKEY_LEAD.trim_end()
        )));
        for (i, key) in hotkey.split('+').enumerate() {
            if i > 0 {
                right = right.child(quiet("+"));
            }
            right = right.child(keycap(
                p,
                if key == "CommandOrControl" {
                    copy::KEY_CTRL
                } else {
                    key
                },
            ));
        }
        right = right.child(quiet(copy::REST_HOTKEY_TRAIL));
    }
    row()
        .w_full()
        .px(4.0)
        .gap(16.0)
        .center_y()
        .flex_wrap()
        .child(left)
        .child(el().grow(1.0))
        .child(right)
}

/// First launch: where links can come from.
fn works_with(p: &Pal) -> El {
    let site = |key: &'static str, glyph: &'static Icon, name: &'static str, hue: Color| {
        row()
            .key(key)
            .h(32.0)
            .px(10.0)
            .gap(8.0)
            .center_y()
            .radius(8.0)
            .border(1.0, p.line_soft)
            .hover(p.raised)
            .focusable(true)
            .role("button")
            .label(name)
            .child(icon(glyph, 16.0).color(hue))
            .child(text(name).font(sans(13.0, 500.0)).color(p.text))
    };
    row()
        .w_full()
        .px(4.0)
        .gap(8.0)
        .center_y()
        .flex_wrap()
        .child(
            text(copy::WORKS_WITH)
                .font(sans(13.0, 400.0))
                .color(p.text2)
                .mr(4.0),
        )
        .child(site(
            "works-youtube",
            &icons::YOUTUBE,
            copy::WORKS_WITH_YOUTUBE,
            p.youtube(),
        ))
        .child(site(
            "works-spotify",
            &icons::SPOTIFY,
            copy::WORKS_WITH_SPOTIFY,
            p.spotify(),
        ))
        .child(site(
            "works-soundcloud",
            &icons::SOUNDCLOUD,
            copy::WORKS_WITH_SOUNDCLOUD,
            p.soundcloud(),
        ))
        .child(site(
            "works-reels",
            &icons::REELS,
            copy::WORKS_WITH_REELS,
            p.reels(),
        ))
        .child(
            text(copy::WORKS_WITH_TAIL)
                .font(sans(13.0, 400.0))
                .color(p.text2)
                .ml(4.0),
        )
}

/// First launch: the shelf the files will stand on, still bare.
fn empty_shelf(p: &Pal) -> El {
    let ghost = |size: f32| el().size(size, size).radius(4.0).fill(p.raised.alpha(0.45));
    let sleeves = row()
        .gap(28.0)
        .items_end()
        .pl(8.0)
        .child(ghost(136.0))
        .child(ghost(104.0))
        .child(ghost(104.0));
    col().w_full().mt(24.0).child(sleeves).child(board(p))
}

/// The shelf itself: a board with a lit top edge.
fn board(p: &Pal) -> El {
    col()
        .w_full()
        .child(el().w_full().h(1.0).fill(p.line))
        .child(el().w_full().h(7.0).fill(p.raised))
        .child(el().w_full().h(1.0).fill(p.line_soft))
}

// ── The figures ─────────────────────────────────────────────────────────────

fn heading(p: &Pal, title: &str, aside: Option<El>) -> El {
    let mut r = row()
        .w_full()
        .h(20.0)
        .center_y()
        .child(text(title).font(sans(15.0, 500.0)).color(p.text));
    if let Some(a) = aside {
        r = r.child(el().grow(1.0)).child(a);
    }
    r
}

/// The next of 1, 2, 5 times a power of ten at or above `v`.
fn nice_ceiling(v: f64) -> f64 {
    if v <= 1.0 {
        return 1.0;
    }
    let mut step = 10f64.powi(v.log10().floor() as i32);
    loop {
        for m in [1.0, 2.0, 5.0] {
            if m * step >= v {
                return m * step;
            }
        }
        step *= 10.0;
    }
}

/// Speed: the summed rate now, against the chart's scale.
fn speed(s: &State, p: &Pal) -> El {
    let now: f64 = s
        .tasks
        .iter()
        .filter(|t| t.progress.status == St::Downloading)
        .map(|t| t.progress.speed)
        .sum();
    let ceil = nice_ceiling(now);
    let figure = col()
        .w(152.0)
        .gap(4.0)
        .child(
            text(copy::speed_now(&copy::format_bytes(now)))
                .font(mono(22.0, 500.0))
                .color(p.text)
                .single_line(),
        )
        .child(
            text(copy::speed_scale(&copy::format_bytes(ceil)))
                .font(mono(12.0, 400.0))
                .color(p.muted)
                .single_line(),
        );
    // The chart starts empty: one sample so far, at the right edge (the app gathers one a second).
    let level = (now / ceil).clamp(0.0, 1.0) as f32;
    let h = 40.0;
    let grid = |top: f32| el().h(1.0).w_full().fill(p.line_soft).absolute(0.0, top);
    let chart = el()
        .grow(1.0)
        .h(h)
        .label(copy::speed_aria(&copy::format_bytes(now)))
        .child(grid(0.0))
        .child(grid(h / 2.0))
        .child(grid(h - 1.0))
        .child(el().size(7.0, 7.0).radius(3.5).fill(p.accent).pin(
            None,
            Some((h - 1.0) * (1.0 - level) - 3.0),
            Some(-3.0),
            None,
        ));
    let title = text(copy::CELL_SPEED)
        .font(sans(15.0, 500.0))
        .color(p.text)
        .w(120.0);
    row()
        .w_full()
        .gap(24.0)
        .center_y()
        .child(title)
        .child(figure)
        .child(chart)
}

fn this_month(p: &Pal, stats: &DownloadStats) -> El {
    let cur = &stats.current;
    let name = parts::month_name(&cur.month);
    let prev = stats.previous.as_ref().filter(|b| b.files > 0.0);
    let files = cur.files as u64;
    let body = if files == 0 {
        let second = match prev {
            Some(b) => copy::month_last_month_kept(b.files as u64, &copy::format_bytes(b.bytes)),
            None => copy::MONTH_NOTHING_YET_NO_PREV.to_string(),
        };
        col()
            .gap(6.0)
            .child(
                text(copy::MONTH_NOTHING_YET)
                    .font(sans(15.0, 400.0))
                    .color(p.text),
            )
            .child(text(second).font(sans(13.0, 400.0)).color(p.text2).wrap())
    } else {
        let ending = match prev {
            Some(b) => {
                let prev_name = parts::month_name(&b.month);
                let diff = files as i64 - b.files as i64;
                if diff == 0 {
                    copy::month_same_count(&prev_name)
                } else {
                    copy::month_diff(diff, &prev_name)
                }
            }
            None => copy::MONTH_FIRST_ON_RECORD.to_string(),
        };
        let figure = row()
            .gap(8.0)
            .items_end()
            .child(
                text(files.to_string())
                    .font(mono(44.0, 400.0).lh(46.0))
                    .color(p.text),
            )
            .child(
                text(copy::file_word(files))
                    .font(sans(15.0, 400.0))
                    .color(p.text2)
                    .mb(6.0),
            );
        let line = runs(vec![
            run_mono(copy::format_bytes(cur.bytes), p.text),
            run(ending, p.text2),
        ])
        .font(sans(13.0, 400.0));
        let mut c = col().gap(8.0).child(figure).child(line);
        if let Some(b) = prev {
            let top = cur.files.max(b.files).max(1.0);
            let bar = |key: &'static str, month: String, n: f64, fill: Color| {
                let share = ((n / top) as f32).max(if n > 0.0 { 0.03 } else { 0.0 });
                col()
                    .key(key)
                    .w_full()
                    .gap(4.0)
                    .child(
                        row()
                            .w_full()
                            .child(text(month).font(sans(12.0, 400.0)).color(p.text2))
                            .child(el().grow(1.0))
                            .child(
                                text((n as u64).to_string())
                                    .font(mono(12.0, 400.0))
                                    .color(p.text2),
                            ),
                    )
                    .child(
                        el().w_full()
                            .h(6.0)
                            .radius(3.0)
                            .fill(p.raised)
                            .child(el().w_frac(share).h_full().radius(3.0).fill(fill)),
                    )
            };
            c = c.child(
                col()
                    .w_full()
                    .max_w(520.0)
                    .mt(2.0)
                    .gap(8.0)
                    .label(copy::month_bars_aria(
                        &name,
                        files,
                        &parts::month_name(&b.month),
                        b.files as u64,
                    ))
                    .child(bar("bar-now", name.clone(), cur.files, p.accent))
                    .child(bar(
                        "bar-prev",
                        parts::month_name(&b.month),
                        b.files,
                        p.muted.alpha(0.55),
                    )),
            );
        }
        c
    };
    col()
        .grow(1.0)
        .min_w(0.0)
        .gap(12.0)
        .child(heading(
            p,
            copy::CELL_THIS_MONTH,
            Some(text(name.clone()).font(sans(13.0, 400.0)).color(p.muted)),
        ))
        .child(body)
}

fn where_from(p: &Pal, stats: &DownloadStats) -> El {
    let cur = &stats.current;
    let total = cur.files as u64;
    let mut groups: Vec<(&'static str, u64, Color)> = vec![
        (copy::SPLIT_YOUTUBE, 0, p.youtube()),
        (copy::SPLIT_SPOTIFY, 0, p.spotify()),
        (copy::SPLIT_SOUNDCLOUD, 0, p.soundcloud()),
        (copy::SPLIT_REELS, 0, p.reels()),
        (copy::SPLIT_OTHER, 0, p.muted),
    ];
    for (source, n) in &cur.by_platform {
        let at = match source.as_str() {
            "youtube" => 0,
            "spotify" => 1,
            "soundcloud" => 2,
            "instagram" | "tiktok" | "facebook" => 3,
            _ => 4,
        };
        groups[at].1 += n.as_u64().unwrap_or(0);
    }
    groups.retain(|g| g.1 > 0);
    groups.sort_by(|a, b| b.1.cmp(&a.1));
    let aside = (total > 0).then(|| {
        let words = copy::split_aside(total);
        let n = total.to_string();
        let at = words.find(&n).unwrap_or(0);
        runs(vec![
            run(&words[..at], p.muted),
            run_mono(n.clone(), p.text2),
            run(&words[at + n.len()..], p.muted),
        ])
        .font(sans(13.0, 400.0))
        .single_line()
    });
    let mut list = col().w_full().max_w(560.0).gap(12.0);
    if total == 0 {
        list = list.child(
            text(copy::SPLIT_EMPTY)
                .font(sans(13.0, 400.0))
                .color(p.text2)
                .wrap(),
        );
    }
    for (i, (name, n, hue)) in groups.iter().enumerate() {
        let pct = ((*n as f64 / total.max(1) as f64) * 100.0).round() as u64;
        let figures = copy::split_row_figures(pct, *n);
        let cut = figures.find(" · ").unwrap_or(figures.len());
        let lead = if i == 0 { p.text } else { p.text2 };
        let share = ((*n as f32) / (total.max(1) as f32)).max(0.02);
        list = list.child(
            col()
                .w_full()
                .gap(6.0)
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .center_y()
                        .child(el().size(6.0, 6.0).radius(3.0).fill(*hue))
                        .child(text(*name).font(sans(13.0, 400.0)).color(lead))
                        .child(el().grow(1.0))
                        .child(
                            runs(vec![
                                run_mono(&figures[..cut], lead),
                                run_mono(&figures[cut..], p.muted),
                            ])
                            .font(mono(12.0, 400.0))
                            .single_line(),
                        ),
                )
                .child(
                    el().w_full()
                        .h(4.0)
                        .radius(2.0)
                        .fill(p.raised)
                        .child(el().w_frac(share).h_full().radius(2.0).fill(*hue)),
                ),
        );
    }
    col()
        .grow(1.0)
        .min_w(0.0)
        .gap(12.0)
        .child(heading(p, copy::CELL_WHERE_FROM, aside))
        .child(list)
}

// ── The shelf ───────────────────────────────────────────────────────────────

/// The newest finished files as sleeves standing on a shelf: the newest one
/// faces out larger, the rest stand beside it.
fn shelf(s: &State, p: &Pal, finished: &[&DownloadTask]) -> El {
    let view_all = row()
        .key("view-all")
        .gap(6.0)
        .center_y()
        .focusable(true)
        .role("button")
        .on_press(|s: &mut State| s.screen = Screen::Downloads)
        .child(
            text(copy::CELL_RECENT_VIEW_ALL)
                .font(sans(13.0, 500.0))
                .color(p.text2),
        )
        .child(icon(&icons::FORWARD, 16.0).color(p.text2));
    let gap = 28.0;
    let mut sleeves = row().gap(gap).items_end().pl(8.0);
    let mut captions = row().gap(gap).items_start().pl(8.0);
    // ponytail: four shown, the 1240-point window's width; the full app shows up to seven as the window widens.
    for (i, t) in finished.iter().take(4).enumerate() {
        let size = if i == 0 { 136.0 } else { 104.0 };
        let mut art = el()
            .size(size, size)
            .child(parts::sleeve(s, p, t, size, Disc::None, false));
        let duration = copy::format_duration(t.duration);
        if !duration.is_empty() {
            art = art.child(
                el().h(18.0)
                    .px(5.0)
                    .center()
                    .radius(4.0)
                    .fill(vinyl().alpha(0.78))
                    .pin(None, Some(6.0), Some(6.0), None)
                    .child(text(duration).font(mono(10.0, 500.0)).color(label_paper())),
            );
        }
        let title = if t.title.is_empty() {
            copy::RECENT_UNTITLED.to_string()
        } else {
            t.title.clone()
        };
        let size_text = copy::format_bytes(t.progress.downloaded);
        let caption = if i == 0 {
            col()
                .w(size)
                .gap(4.0)
                .child(
                    text(title.clone())
                        .font(sans(13.0, 500.0))
                        .color(p.text)
                        .max_lines(2)
                        .wrap(),
                )
                .child(
                    text(copy::recent_poster_meta(&t.format, &size_text, &t.uploader))
                        .font(mono(11.0, 400.0))
                        .color(p.muted)
                        .truncate(),
                )
        } else {
            col()
                .w(size)
                .gap(4.0)
                .child(
                    text(title.clone())
                        .font(sans(13.0, 400.0))
                        .color(p.text)
                        .truncate(),
                )
                .child(
                    row()
                        .w_full()
                        .child(
                            text(t.format.to_uppercase())
                                .font(mono(11.0, 400.0))
                                .color(p.muted),
                        )
                        .child(el().grow(1.0))
                        .child(text(size_text).font(mono(11.0, 400.0)).color(p.muted)),
                )
        };
        let tile = el()
            .key(format!("recent-{}", t.task_id))
            .focusable(true)
            .role("button")
            .label(copy::recent_details_aria(&title))
            .on_press(|s: &mut State| s.screen = Screen::Downloads)
            .child(art);
        sleeves = sleeves.child(tile);
        captions = captions.child(caption);
    }
    col()
        .w_full()
        .gap(0.0)
        .child(heading(p, copy::CELL_RECENT, Some(view_all)).mb(14.0))
        .child(sleeves)
        .child(board(p))
        .child(captions.mt(12.0))
}
