//! Layout "The collection", wearing the Shelf look: Home is the wall of what
//! you own. Every download is a sleeve on it; a new one joins the wall with
//! its record beside it, the record's rim filling as the file arrives. Three places (Home, Downloads, Settings) sit behind the shelf
//! dividers: a slim strip on the left that opens on hover and can be pinned.
//!
//! Where the old app's screens and features live now:
//!
//! | Old (Electron 1.x)                         | Here |
//! | --- | --- |
//! | Rail with seven tabs                       | The dividers strip: Home, Downloads, Settings (Ctrl+1, 2, 3); opens on hover, pin keeps it open |
//! | Home page                                  | Home: the wall, grouped "In progress" / "Needs attention" / "Completed" |
//! | Home paste field, Look up, Ctrl+V          | The add field in the title bar ("Paste a link or search", Ctrl V), on every place |
//! | Home headline, Works with, rest caption    | Home on first launch: the headline, the paste keys and four empty record slots, one per source |
//! | Home search results (unified search)       | Typing words in the add field: results join the top of the wall as a temporary group (not drawn) |
//! | YouTube / Spotify / SoundCloud / Reels tabs| Source filters on the wall; their search is the add field's; per-platform overrides sit in the add field's folder and format "Change" (not drawn) |
//! | Playlist and short-form previews           | Open under the add field when a pasted link is a collection (not drawn) |
//! | Clipboard offer                            | A quiet line under the add field (not drawn) |
//! | Home "This month", "Where it came from"    | Home's masthead: the month's file count as the headline, size and comparison as a sentence, the source split beside it |
//! | Home "Speed" cell                          | The "In progress" header (Home) and group header (Downloads): transferring count and total speed |
//! | Home "Recent downloads"                    | The "Completed" shelf on Home |
//! | Downloads page                             | Downloads: the full list, header counts and summary, filters, search, bulk actions, rows with their actions |
//! | Interrupted banner (Resume, Discard)       | Downloads above the list; Home's "In progress" header |
//! | Row details, error sentence, Spotify match | Home: a sleeve opens in place as liner notes (file, folder, source, match, actions); Downloads: the row |
//! | Select, multi-select, reorder, Do this next| Downloads rows (not drawn: checkboxes and the grip show on hover) |
//! | Toasts and the Notice popup                | Bottom-left over the place (not drawn) |
//! | Settings page                              | Settings: every section in two columns |
//! | Schedule, updates, licence, cookies import | Settings: Scheduled downloads, Software, Plan & license, More download options |
//! | Update modal, upgrade sheet                | Open from Settings' Software and Plan & license (not drawn) |
//! | Command palette (Ctrl+K)                   | Over any place; its keys sit in the title bar, as an extra |
//! | Shortcuts sheet (Ctrl+/)                   | The palette and Settings' Help |
mod downloads;
mod nav;
mod settings;

use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode, SourcePlatform};
use rune::{Color, Cx, El, Icon, ScrollSpec, Span, col, el, icon, rich, row, text};

use crate::directions::shelf::parts::{self, Disc, run, run_mono, runs};
use crate::directions::shelf::theme::{self, Pal, mono, sans, serif};
use crate::gate::{Confirm, Nav, State};
use crate::keys::{self, RowAct};
use crate::ring::Pass;
use crate::{copy, format, icons};

/// The wall's side margin.
const PAD: f32 = 32.0;
/// The sleeve whose liner notes are open on Home when only downloads in progress are shown.
const OPEN: &str = "get-lucky";

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = Pal::new(s.palette, cx.system_dark());
    cx.caption_color(p.text);
    let insets = cx.chrome_insets();
    let place = match s.nav {
        Nav::Home => wall(s, &p),
        Nav::Downloads => downloads::view(s, &p),
        Nav::Settings => settings::view(s, &p),
    };
    let body = row().grow(1.0).min_h(0.0).w_full().child(nav::view(s, &p)).child(place);
    col().w_full().h_full().fill(p.bg).child(titlebar(s, &p, insets.leading, insets.trailing)).child(body)
}

// ── Title bar: the brand, the add field, the palette and the sheet ─────────

fn titlebar(s: &State, p: &Pal, leading: f32, trailing: f32) -> El {
    let side = 240.0_f32.max(trailing + 96.0);
    let mark = match &s.logo {
        Some(logo) => el().size(16.0, 16.0).pixels(logo.pixels()),
        None => el().size(16.0, 16.0),
    };
    let wordmark = rich(
        format!("{}{}", copy::BRAND, copy::BRAND_REST),
        vec![
            Span::new(0, copy::BRAND.len()).weight(500.0).color(p.text),
            Span::new(copy::BRAND.len(), copy::BRAND.len() + copy::BRAND_REST.len()).color(p.muted),
        ],
    )
    .font(sans(13.0, 400.0))
    .single_line();
    let add = row()
        .key(keys::FIELD)
        .w(440.0)
        .h(32.0)
        .pl(12.0)
        .pr(4.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.card)
        .border(1.0, p.line)
        .hover(p.raised)
        .no_window_drag()
        .focusable(true)
        .role("textbox")
        .label(copy::FIELD_ARIA)
        .child(icon(&icons::LINK, 16.0).color(p.muted))
        .child(text(copy::FIELD_PLACEHOLDER).font(sans(13.0, 400.0)).color(p.muted).grow(1.0).single_line().truncate())
        .child(keys(p, &[copy::KEY_CTRL, copy::REST_KEY_V]));
    let palette = row()
        .key("palette")
        .h(32.0)
        .px(8.0)
        .center_y()
        .radius(8.0)
        .hover(p.raised)
        .no_window_drag()
        .focusable(true)
        .role("button")
        .label(copy::PALETTE_ARIA)
        .help(copy::PALETTE_ARIA)
        .child(keys(p, &[copy::KEY_CTRL, copy::KEY_K]));
    row()
        .key("titlebar")
        .w_full()
        .h(crate::TITLEBAR)
        .center_y()
        .fill(p.bg)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line_soft)
        .window_drag()
        .child(row().w(side).pl(leading + 16.0).gap(8.0).center_y().child(mark).child(wordmark))
        .child(row().grow(1.0).justify_center().child(add))
        .child(row().w(side).pr(trailing + 8.0).gap(4.0).justify_end().center_y().child(palette))
}

/// Keys as printed on the keyboard, side by side.
fn keys(p: &Pal, caps: &[&str]) -> El {
    let mut r = row().gap(4.0).center_y();
    for k in caps {
        r = r.child(keycap(p, k));
    }
    r
}

fn keycap(p: &Pal, k: &str) -> El {
    el().h(20.0)
        .px(4.0)
        .center()
        .radius(4.0)
        .fill(p.raised)
        .border(1.0, p.line)
        .child(text(k.to_string()).font(mono(12.0, 400.0)).color(p.text2).single_line())
}

/// A quiet text button: the default for every action on the wall.
fn quiet(p: &Pal, key: &str, glyph: Option<&'static Icon>, label: &str, ink: Color) -> El {
    let mut b = row()
        .key(key.to_string())
        .h(32.0)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .hover(p.raised)
        .press(p.line)
        .focusable(true)
        .role("button")
        .label(label.to_string());
    if let Some(g) = glyph {
        b = b.child(icon(g, 16.0).color(ink));
    }
    b.child(text(label.to_string()).font(sans(13.0, 500.0)).color(ink).single_line())
}

// ── The wall ────────────────────────────────────────────────────────────────

fn wall(s: &State, p: &Pal) -> El {
    let mut page = col().w_full().px(PAD).pt(16.0).pb(48.0);
    if s.tasks.is_empty() && s.stats.is_none() {
        page = page.child(first_run(s, p));
    } else {
        page = page.child(masthead(s, p));
        let active: Vec<&DownloadTask> = s.tasks.iter().filter(|t| format::is_active(t)).collect();
        let attention: Vec<&DownloadTask> = s.tasks.iter().filter(|t| matches!(t.progress.status, St::Failed | St::Cancelled)).collect();
        let done: Vec<&DownloadTask> = s.tasks.iter().filter(|t| t.progress.status == St::Done).collect();
        if !active.is_empty() {
            page = page.child(in_progress(s, p, &active, attention.is_empty() && done.is_empty()));
        }
        if !attention.is_empty() {
            page = page.child(needs_attention(s, p, &attention));
        }
        if !done.is_empty() {
            // A shelf on its own gets the room; under the others it steps down.
            let size = if active.is_empty() && attention.is_empty() { 240.0 } else { 120.0 };
            page = page.child(completed(s, p, &done, size));
        }
    }
    col().key("wall").grow(1.0).h_full().min_w(0.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(page)
}

// ── First launch: an empty wall that asks for the first link ───────────────

fn first_run(s: &State, p: &Pal) -> El {
    let hotkey = s.settings.get("globalHotkey").and_then(|v| v.as_str()).unwrap_or_default();
    let caps: Vec<&str> = hotkey.split('+').map(|k| if k == "CommandOrControl" { copy::KEY_CTRL } else { k }).collect();
    let paste = row()
        .gap(4.0)
        .center_y()
        .child(keys(p, &[copy::KEY_CTRL]))
        .child(text(copy::REST_PLUS_SPACED).font(mono(12.0, 400.0)).color(p.muted))
        .child(keys(p, &[copy::REST_KEY_V]))
        .child(text(format!("{}{}", copy::REST_PASTES_A_LINK, copy::REST_HOTKEY_LEAD.trim_end())).font(sans(13.0, 400.0)).color(p.text2))
        .child(el().w(4.0))
        .child(keys(p, &caps))
        .child(text(copy::REST_HOTKEY_TRAIL).font(sans(13.0, 400.0)).color(p.text2));
    let slots = [
        (copy::WORKS_WITH_YOUTUBE, p.youtube()),
        (copy::WORKS_WITH_SPOTIFY, p.spotify()),
        (copy::WORKS_WITH_SOUNDCLOUD, p.soundcloud()),
        (copy::WORKS_WITH_REELS, p.reels()),
    ];
    let mut shelf = row().w_full().gap(24.0).items_end();
    for (name, hue) in slots {
        shelf = shelf.child(
            col()
                .gap(12.0)
                .child(el().size(200.0, 200.0).center().radius(12.0).border(1.0, p.line).border_dash(4.0, 4.0).child(record(hue, 128.0)))
                .child(text(name).font(sans(13.0, 500.0)).color(p.text2)),
        );
    }
    shelf = shelf.child(col().w(160.0).pb(28.0).child(text(copy::WORKS_WITH_TAIL).font(sans(13.0, 400.0)).color(p.muted).wrap()));
    col()
        .w_full()
        .pt(64.0)
        .child(text(copy::HOME_HEADLINE).font(serif(44.0).lh(48.0)).color(p.text))
        .child(paste.mt(12.0))
        .child(text(copy::WORKS_WITH).font(sans(13.0, 400.0)).color(p.muted).mt(80.0).mb(16.0))
        .child(shelf)
}

/// A bare record waiting for its sleeve: vinyl, one groove, the source's label.
fn record(hue: Color, d: f32) -> El {
    let groove = (d * 0.68).round();
    let label = (d * 0.36).round();
    el().size(d, d).radius(d / 2.0).center().fill(theme::vinyl()).border(1.0, theme::rim()).child(
        el().size(groove, groove).radius(groove / 2.0).center().border(1.0, theme::groove()).child(
            el().size(label, label).radius(label / 2.0).center().fill(hue).child(el().size(4.0, 4.0).radius(2.0).fill(theme::vinyl())),
        ),
    )
}

// ── Masthead: the collection's own facts ───────────────────────────────────

fn masthead(s: &State, p: &Pal) -> El {
    let Some(stats) = &s.stats else {
        return col().pb(32.0).child(text(copy::HOME_HEADLINE).font(serif(44.0).lh(48.0)).color(p.text));
    };
    let cur = &stats.current;
    let files = cur.files as u64;
    let month = runs(vec![run(copy::CELL_THIS_MONTH, p.text2), run_mono(" · ", p.muted), run(format::month_name(&cur.month), p.text2)])
        .font(sans(13.0, 400.0))
        .single_line();
    let compare = format::month_sentence(stats);
    let (headline, sentence) = if files == 0 {
        (text(copy::MONTH_NOTHING_YET), runs(vec![run(copy::MONTH_NOTHING_YET_NO_PREV, p.text2)]))
    } else {
        (text(format!("{files} {}", copy::file_word(files))), runs(vec![run_mono(format::bytes(cur.bytes), p.text), run(compare, p.text2)]))
    };
    let left = col()
        .gap(4.0)
        .child(row().gap(16.0).items_end().child(headline.font(serif(44.0).lh(48.0)).color(p.text)).child(month.mb(8.0)))
        .child(sentence.font(sans(13.0, 400.0)).single_line());

    // Where it came from: the month's files by source, as one bar.
    let split: Vec<(&str, f64, Color)> =
        format::source_split(stats).into_iter().map(|(bucket, n)| (bucket.name(), n as f64, parts::bucket_hue(p, bucket))).collect();
    let total: f64 = split.iter().map(|g| g.1).sum();
    let mut right = col().w(320.0).gap(12.0);
    if total > 0.0 {
        let mut bar = row().w_full().h(8.0).gap(2.0);
        let mut legend = row().w_full().flex_wrap().gaps(4.0, 16.0);
        for (name, n, hue) in &split {
            bar = bar.child(el().grow(*n as f32).h(8.0).radius(2.0).fill(*hue));
            let pct = (n / total * 100.0).round() as u64;
            legend = legend.child(
                row()
                    .w(152.0)
                    .h(24.0)
                    .radius(4.0)
                    .hover(p.raised)
                    .focusable(true)
                    .role("button")
                    .label(name.to_string())
                    .gap(8.0)
                    .center_y()
                    .child(el().size(8.0, 8.0).radius(4.0).fill(*hue))
                    .child(text(*name).font(sans(12.0, 400.0)).color(p.text2).single_line())
                    .child(el().grow(1.0))
                    .child(text(copy::split_row_figures(pct, *n as u64)).font(mono(12.0, 400.0)).color(p.muted)),
            );
        }
        right = right
            .child(
                runs(vec![run(copy::CELL_WHERE_FROM, p.text2), run_mono(" · ", p.muted), run(copy::split_aside(total as u64), p.muted)])
                    .font(sans(13.0, 400.0))
                    .single_line(),
            )
            .child(bar)
            .child(legend);
    }
    row().w_full().pb(32.0).items_end().justify_between().gap(32.0).child(left).child(right)
}

// ── Filters (Downloads uses them) ──────────────────────────────────────────

fn tab(p: &Pal, key: &str, label: &str, count: Option<usize>, dot: Option<Color>, n: usize, on: bool) -> El {
    let mut t = row()
        .key(key.to_string())
        .h(40.0)
        .gap(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 2.0, 0.0], if on { p.text } else { Color::TRANSPARENT })
        .focusable(true)
        .role("tab")
        .label(label.to_string());
    if n > 0 {
        t = t.help(copy::nav_tooltip(label, n));
    }
    if let Some(hue) = dot {
        t = t.child(el().size(8.0, 8.0).radius(4.0).fill(hue));
    }
    t = t.child(
        text(label.to_string()).font(sans(13.0, if on { 500.0 } else { 400.0 })).color(if on { p.text } else { p.text2 }).single_line(),
    );
    if let Some(c) = count {
        t = t.child(text(c.to_string()).font(mono(12.0, 400.0)).color(p.muted));
    }
    t
}

// ── Shelves ─────────────────────────────────────────────────────────────────

fn shelf_header(p: &Pal, title: &str, count: usize, aside: Option<El>, actions: El) -> El {
    let mut left = row()
        .gap(8.0)
        .center_y()
        .child(text(title.to_string()).font(sans(16.0, 500.0)).color(p.text))
        .child(text(count.to_string()).font(mono(13.0, 400.0)).color(p.muted));
    if let Some(a) = aside {
        left = left.child(a.ml(8.0));
    }
    row().w_full().mb(8.0).gaps(8.0, 16.0).flex_wrap().center_y().justify_between().child(left).child(actions)
}

fn in_progress(s: &State, p: &Pal, tasks: &[&DownloadTask], alone: bool) -> El {
    let (live, speed) = format::in_flight(&s.tasks);
    let aside = row()
        .gap(8.0)
        .center_y()
        .label(copy::transferring_aria(live, &format::speed(speed)))
        .child(icon(&icons::DOWNLOAD, 16.0).color(p.muted))
        .child(
            text(if speed > 0.0 { format!("{live} · {}", format::speed(speed)) } else { live.to_string() })
                .font(mono(13.0, 400.0))
                .color(p.text2),
        );
    let interrupted = tasks.iter().filter(|t| t.progress.status == St::Paused && t.interrupted == Some(true)).count();
    // Two units, so a narrow window breaks between them and never inside one.
    let mut actions = row().gaps(4.0, 16.0).flex_wrap().shrink().min_w(0.0).center_y();
    if interrupted > 0 {
        actions = actions.child(interrupted_line(p, interrupted));
    }
    actions = actions.child(
        row().gap(4.0).center_y().child(quiet(p, keys::PAUSE_ALL, Some(&icons::PAUSE), copy::DL_PAUSE_ALL, p.text2)).child(quiet(
            p,
            keys::CANCEL_ALL,
            Some(&icons::CANCEL),
            copy::DL_CANCEL_ALL,
            p.text2,
        )),
    );

    // Arriving now leads, large; what waits stands smaller beside it.
    let mut order: Vec<&DownloadTask> = tasks.to_vec();
    order.sort_by_key(|t| !arriving(t.progress.status));
    let mut wall = row().w_full().flex_wrap().gaps(32.0, 12.0).items_end();
    for t in order {
        let size = if arriving(t.progress.status) {
            if alone { 200.0 } else { 120.0 }
        } else if alone {
            120.0
        } else {
            80.0
        };
        wall = wall.child(if alone && t.task_id == OPEN {
            notes(s, p, t)
        } else {
            item(s, p, t, size, if alone { size + 8.0 } else { 128.0 }, 144.0)
        });
    }
    col().w_full().mb(24.0).child(shelf_header(p, copy::GROUP_IN_PROGRESS, tasks.len(), Some(aside), actions)).child(wall)
}

fn arriving(st: St) -> bool {
    matches!(st, St::Downloading | St::FetchingInfo | St::Converting | St::Embedding)
}

fn needs_attention(s: &State, p: &Pal, tasks: &[&DownloadTask]) -> El {
    let mut wall = row().w_full().flex_wrap().gaps(32.0, 24.0).items_start();
    for t in tasks {
        wall = wall.child(item(s, p, t, 80.0, 88.0, 256.0));
    }
    col()
        .w_full()
        .mb(24.0)
        .child(shelf_header(
            p,
            copy::GROUP_NEEDS_ATTENTION,
            tasks.len(),
            None,
            quiet(p, keys::RETRY_ALL_FAILED, Some(&icons::RETRY), copy::DL_RETRY_ALL_FAILED, p.text2),
        ))
        .child(wall)
}

fn completed(s: &State, p: &Pal, tasks: &[&DownloadTask], size: f32) -> El {
    let mut wall = row().w_full().flex_wrap().gaps(32.0, 24.0).items_start();
    for t in tasks {
        wall = wall.child(item(s, p, t, size, size + 8.0, size.max(200.0)));
    }
    col()
        .w_full()
        .child(shelf_header(
            p,
            copy::GROUP_COMPLETED,
            tasks.len(),
            None,
            quiet(p, keys::arm(Confirm::Clear), Some(&icons::REMOVE), copy::DL_CLEAR_COMPLETED, p.text2),
        ))
        .child(wall)
}

// ── A sleeve on the wall ────────────────────────────────────────────────────

/// Where the record is: beside the sleeve while its rim is the ring (decision 9a);
/// landed, failed and cancelled sleeves have none.
fn disc(s: &State, t: &DownloadTask) -> Disc {
    match (t.progress.status, s.rings.pass(&t.task_id)) {
        (St::Failed | St::Cancelled, _) | (St::Done, Some(Pass::Landed) | None) => Disc::None,
        _ => Disc::Out(1.0),
    }
}

/// The sticker on a sleeve's corner, like a shop's price sticker: the state at
/// a glance, without motion. Downloading and done sleeves carry none.
fn sticker(p: &Pal, t: &DownloadTask) -> Option<(&'static Icon, Color, Color)> {
    Some(match t.progress.status {
        St::Queued => (&icons::CLOCK, p.raised, p.text2),
        St::Converting | St::Embedding | St::FetchingInfo => (&icons::SPINNER, p.warning, p.bg),
        St::Paused => (&icons::PAUSE, p.warning, p.bg),
        St::Retrying => (&icons::RETRY, p.warning, p.bg),
        St::Failed => (&icons::ALERT, p.error, p.bg),
        St::Cancelled => (&icons::CANCEL, p.raised, p.text2),
        St::Downloading | St::Done => return None,
    })
}

fn sleeve(s: &State, p: &Pal, t: &DownloadTask, size: f32) -> El {
    let faded = matches!(t.progress.status, St::Cancelled);
    let mut e = el().child(parts::sleeve(s, p, t, size, disc(s, t), faded));
    if let Some((g, fill, ink)) = sticker(p, t) {
        e = e.child(
            el().size(28.0, 28.0)
                .radius(14.0)
                .center()
                .fill(fill)
                .border(2.0, p.bg)
                .absolute(size - 20.0, -8.0)
                .child(icon(g, 16.0).color(ink)),
        );
    }
    e
}

/// One sleeve standing on its shelf, its caption under it.
fn item(s: &State, p: &Pal, t: &DownloadTask, size: f32, stand: f32, width: f32) -> El {
    let label = format::label(t);
    let travel = if disc(s, t) == Disc::None { 1.0 } else { 1.0 + parts::DISC_TRAVEL };
    let slot = (size * travel).round().max(width);
    col()
        .key(format!("sleeve-{}", t.task_id))
        .w(slot)
        .gap(4.0)
        .focusable(true)
        .role("button")
        .label(label.clone())
        .child(el().w(slot).h(stand).items_end().mb(8.0).child(sleeve(s, p, t, size)))
        .child(
            text(label.clone())
                .font(sans(13.0, 500.0))
                .color(if matches!(t.progress.status, St::Done | St::Cancelled) { p.text2 } else { p.text })
                .single_line()
                .truncate()
                .help(label),
        )
        .child(caption(p, t))
}

/// The line under a sleeve: what is happening to it, in words.
fn caption(p: &Pal, t: &DownloadTask) -> El {
    let pr = &t.progress;
    let small = |r: Vec<parts::Run>| runs(r).font(sans(12.0, 400.0)).single_line().truncate();
    let pct = format!("{}%", format::percent(t));
    match pr.status {
        St::Downloading => small(vec![run_mono(pct, p.text), run_mono(copy::row_dot_part(&copy::rate_line(pr.speed, pr.eta)), p.muted)]),
        St::FetchingInfo => small(vec![run(copy::STATE_STARTING, p.text2)]),
        St::Converting => small(vec![run(copy::STATE_CONVERTING, p.warning)]),
        St::Embedding => small(vec![run(copy::STATE_TAGGING, p.warning)]),
        St::Queued => small(vec![run(copy::STATE_QUEUED, p.muted)]),
        St::Paused => small(vec![run(copy::STATE_PAUSED, p.warning), run_mono(copy::row_dot_part(&pct), p.muted)]),
        St::Retrying => {
            let k = t.retry_count.unwrap_or(0.0) as usize;
            let line = copy::row_retrying(format::retry_wait(t), k);
            small(vec![run(line.trim_start_matches(" · ").to_string(), p.warning)])
        }
        St::Done => {
            let meta = format!("{} · {}", t.format.to_uppercase(), format::bytes(pr.total));
            let mut r = Vec::new();
            if !t.uploader.is_empty() {
                r.push(run(t.uploader.clone(), p.text2));
                r.push(run_mono(copy::row_dot_part(&meta), p.muted));
            } else {
                r.push(run_mono(meta, p.muted));
            }
            small(r)
        }
        St::Failed | St::Cancelled => trouble(p, t),
    }
}

/// A failed or cancelled sleeve's caption: the state, the reason, one way out.
fn trouble(p: &Pal, t: &DownloadTask) -> El {
    let failed = t.progress.status == St::Failed;
    let word = if !failed {
        copy::STATE_CANCELED
    } else if format::is_permanent(t.error_code) {
        copy::STATE_CANT_DOWNLOAD
    } else {
        copy::STATE_FAILED
    };
    let (act, glyph, label) = way_out(t);
    let state = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            runs(vec![
                run(word, if failed { p.error } else { p.muted }),
                run(copy::row_dot_part(format::source_name(t.source)), p.source(t.source)),
            ])
            .font(sans(12.0, 500.0))
            .single_line()
            .grow(1.0),
        )
        .child(icon_button(p, keys::row_action(act, &t.task_id), glyph, label, p.text2));
    let mut c = col().w_full().gap(4.0).mt(-4.0).child(state);
    if let Some(r) = format::reason(t) {
        c = c.child(text(r).font(sans(12.0, 400.0)).color(p.text2).wrap().max_lines(3).help(t.progress.error.clone()));
    }
    c
}

/// A square icon button; its label is the tooltip and the accessible name.
fn icon_button(p: &Pal, key: String, glyph: &'static Icon, label: &str, ink: Color) -> El {
    el().key(key)
        .size(28.0, 28.0)
        .center()
        .radius(8.0)
        .hover(p.raised)
        .press(p.line)
        .focusable(true)
        .role("button")
        .label(label.to_string())
        .help(label.to_string())
        .child(icon(glyph, 16.0).color(ink))
}

/// "1 download interrupted when the app closed. Finish it now?" with Resume and Discard.
fn interrupted_line(p: &Pal, n: usize) -> El {
    row()
        .gap(4.0)
        .center_y()
        .child(icon(&icons::PAUSE, 16.0).color(p.warning).mr(8.0))
        .child(
            runs(vec![run_mono(n.to_string(), p.text), run(copy::interrupted_sentence(n), p.text2)]).font(sans(13.0, 400.0)).single_line(),
        )
        .child(quiet(p, keys::RESUME_INTERRUPTED, None, copy::DL_INTERRUPTED_RESUME, p.text).ml(8.0))
        .child(quiet(p, keys::DISCARD_INTERRUPTED, None, copy::DL_INTERRUPTED_DISCARD, p.text2))
}

/// The one action a troubled sleeve offers on the wall (the rest are in its notes).
fn way_out(t: &DownloadTask) -> (RowAct, &'static Icon, &'static str) {
    use ErrorCode::*;
    if t.progress.status == St::Cancelled {
        return (RowAct::DownloadAgain, &icons::DOWNLOAD, copy::ACT_DOWNLOAD_AGAIN);
    }
    match t.error_code {
        Some(Private | AgeRestricted | LoginRequired | DiskFull) => (RowAct::Fix, &icons::SETTINGS, copy::ACT_OPEN_SETTINGS),
        Some(Unknown) => (RowAct::Fix, &icons::REFRESH, copy::ACT_UPDATE_ENGINE),
        _ if format::is_permanent(t.error_code) => (RowAct::Remove, &icons::REMOVE, copy::ACT_REMOVE),
        _ => (RowAct::Retry, &icons::RETRY, copy::ACT_RETRY),
    }
}

// ── Liner notes: a sleeve opened in place ──────────────────────────────────

fn notes(s: &State, p: &Pal, t: &DownloadTask) -> El {
    let size = 160.0;
    let pr = &t.progress;
    let label = format::label(t);
    let mut source = vec![run(format::source_name(t.source), p.source(t.source))];
    if let Some(c) = t.match_confidence.filter(|_| t.source == SourcePlatform::Spotify) {
        // ponytail: the old app has no wording for the confidence itself; the figure stands beside its note.
        source.push(run(copy::row_dot_part(copy::SEARCH_SPOTIFY_NOTE), p.text2));
        source.push(run_mono(copy::row_dot_part(&format!("{}%", (c * 100.0).round())), p.text));
    }
    let mut text_col = col()
        .w(320.0)
        .gap(4.0)
        .child(text(label).font(sans(16.0, 500.0)).color(p.text).wrap().max_lines(2))
        .child(text(t.uploader.clone()).font(sans(13.0, 400.0)).color(p.text2).single_line())
        .child(runs(source).font(sans(13.0, 400.0)).wrap().mt(8.0));
    if pr.status == St::Downloading {
        text_col = text_col.child(
            runs(vec![
                run_mono(format!("{}%", format::percent(t)), p.text),
                run_mono(copy::row_dot_part(&copy::rate_line(pr.speed, pr.eta)), p.muted),
            ])
            .font(sans(13.0, 400.0))
            .single_line(),
        );
    }
    let folder = if pr.filename.is_empty() { t.output_dir.clone() } else { pr.filename.clone() };
    text_col = text_col.child(text(format::specs(t)).font(mono(12.0, 400.0)).color(p.muted).single_line().mt(8.0)).child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .help(folder.clone())
            .child(icon(&icons::FOLDER, 16.0).color(p.muted))
            .child(text(folder).font(mono(12.0, 400.0)).color(p.text2).single_line().truncate_middle().shrink()),
    );
    if let Some(r) = format::reason(t) {
        text_col = text_col.child(text(r).font(sans(13.0, 400.0)).color(p.error).wrap().mt(8.0));
    }
    let actions = if pr.status == St::Done {
        row()
            .child(quiet(p, &keys::row_action(RowAct::OpenFile, &t.task_id), Some(&icons::EXTERNAL), copy::ACT_OPEN_FILE, p.text))
            .child(quiet(p, &keys::row_action(RowAct::ShowInFolder, &t.task_id), Some(&icons::FOLDER), copy::ACT_SHOW_IN_FOLDER, p.text))
    } else {
        row().child(quiet(p, &keys::row_action(RowAct::Pause, &t.task_id), Some(&icons::PAUSE), copy::ACT_PAUSE, p.text)).child(quiet(
            p,
            &keys::row_action(RowAct::Cancel, &t.task_id),
            Some(&icons::CANCEL),
            copy::ACT_CANCEL,
            p.text,
        ))
    };
    text_col = text_col.child(actions.gap(4.0).mt(12.0).ml(-8.0).child(quiet(
        p,
        &keys::row_action(RowAct::CopyDetails, &t.task_id),
        Some(&icons::COPY),
        copy::ACT_COPY_DETAILS,
        p.text2,
    )));
    row()
        .key(format!("notes-{}", t.task_id))
        .padding(16.0)
        .gap(24.0)
        .items_start()
        .radius(12.0)
        .fill(p.card)
        .border(1.0, p.line)
        .child(el().pt(8.0).child(sleeve(s, p, t, size)))
        .child(text_col)
}
