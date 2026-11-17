//! Layout "One bar", wearing the Signal look. Three places (Home, Downloads,
//! Settings) sit as tabs in the title bar, always visible; Ctrl+1/2/3 go to
//! them. Home is the bar: a paste-and-command field over a readout deck
//! (this month's count on the left; on the right the master instrument, a
//! tape counter of the media carried across today over a level meter of
//! everything in flight), and under it what is in flight and what just
//! landed. Downloads is the full history; Settings lists its sections on the
//! left and shows the chosen one on the right. Ctrl+K still turns the bar
//! into a command line that reaches any section from anywhere.
//!
//! Where the old app's screens and features live now:
//!
//! | Old (Electron 1.x)                          | One bar                                                              |
//! |---------------------------------------------|----------------------------------------------------------------------|
//! | Rail: Home / Downloads / Settings           | Title-bar tabs Home / Downloads / Settings, Ctrl+1/2/3               |
//! | Rail badge on Downloads, carrier line       | Active count beside the Downloads tab; the title bar's edge lights    |
//! | Home: paste field, Look up, status line     | Home: the bar's upper deck; the status line is the caption under it  |
//! | Home: rest caption (folder, format, Ctrl+V) | Home: the caption; `Change` opens Settings › Destination             |
//! | Home: search results                        | Home: a panel dropped from the bar, one band per source              |
//! | Home: Speed cell                            | Home: the master instrument's speed figure                           |
//! | Home: This month                            | Home: the deck's left readout; Settings › This month for the split   |
//! | Home: Where it came from                    | Settings › This month                                                |
//! | Home: Recent downloads, View all            | Home: Recent downloads (latest three), View all goes to Downloads    |
//! | Home: Works with (first launch)             | Home, first launch: in the empty desk                                |
//! | Downloads: header counts, summary           | Downloads: the summary line under the toolbar                        |
//! | Downloads: filters All/Active/Done/Failed   | Downloads: segmented filter (by click; Ctrl+1..4 now go to places)   |
//! | Downloads: search, bulk actions             | Downloads: toolbar                                                   |
//! | Downloads: interrupted banner               | First line of the In progress well (Home and Downloads)              |
//! | Downloads: rows, row actions, reorder       | Downloads: the full list; Home shows in flight, problems, latest     |
//! | Downloads: selection (checkboxes)           | Ctrl/Shift-click a row; the toolbar swaps to the selection's actions |
//! | YouTube / Spotify / SoundCloud / Reels tabs | Search scopes of the bar (Ctrl+K, Navigate); playlist preview drops  |
//! |                                             |   from the bar like the search results                               |
//! | Settings: every section                     | Settings place: section list left, the section right                 |
//! | Settings: Software (updates, engine)        | Settings › Software; `Check for updates` is also a bar command       |
//! | Settings: Plan & license                    | Settings › Plan & license                                            |
//! | Settings: cookies import, Sign in with      | Settings › More download options                                     |
//! | Command palette (Ctrl+K)                    | The bar in command mode, from any place (jumps to Home's bar)        |
//! | Keyboard shortcuts sheet (Ctrl+/)           | Settings › Help, and a sheet from the bar                            |
//! | Notices / toasts                            | A line in the bar's lower deck                                       |
mod places;
mod reach;
mod stream;

use ahg_engine::types::{DownloadStatus as St, DownloadTask, SourcePlatform};
use rune::{Color, Cursor, Cx, El, SurfaceFit, col, el, icon, row, spacer};

use crate::copy;
use crate::directions::signal::found;
use crate::directions::signal::model;
use crate::directions::signal::parts::{Line, config_text, label, meter, meter_held, mono, platform_icon, sans};
use crate::directions::signal::theme::{self, Pal, source_color};
use crate::format;
use crate::gate::{Nav, State};
use crate::icons;

/// The bar's measure, and the stream's.
pub(crate) const MEASURE: f32 = 960.0;
const INPUT_H: f32 = 64.0;
const DECK_H: f32 = 56.0;

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = theme::pal(s.palette, s.dark(cx));
    cx.caption_color(if p.dark { Color::WHITE } else { Color::BLACK });
    let trailing = cx.chrome_insets().trailing;
    let body = match s.nav {
        Nav::Home if s.tasks.is_empty() && s.stats.is_none() => first_launch(s, &p),
        Nav::Home => home(s, &p),
        Nav::Downloads => places::downloads(s, &p),
        Nav::Settings => places::settings(s, &p),
    };
    col().w_full().h_full().fill(p.bg).child(titlebar(s, &p, trailing)).child(body)
}

/// The title bar: wordmark, then the three places as tabs. The current one is
/// a single lit segment under its name.
fn titlebar(s: &State, p: &Pal, trailing: f32) -> El {
    let logo = match &s.logo {
        Some(img) if img.is_ready() => el().size(16.0, 16.0).pixels(img.pixels()),
        _ => el().size(16.0, 16.0),
    };
    let mark = row()
        .w(240.0)
        .pl(16.0)
        .gap(8.0)
        .center_y()
        .child(logo)
        .child(Line::new().strong(copy::BRAND, p.text).add(copy::BRAND_REST, p.muted).one(mono(12.0, 400.0)));
    let active = format::active_count(&s.tasks);
    let mut tabs = row().h_full().gap(4.0).center_y().role("tablist").label(copy::RAIL_ARIA).no_window_drag();
    for (i, (place, name)) in
        [(Nav::Home, copy::NAV_HOME), (Nav::Downloads, copy::NAV_DOWNLOADS), (Nav::Settings, copy::NAV_SETTINGS)].into_iter().enumerate()
    {
        let on = s.nav == place;
        let mut tab = row()
            .h(32.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(6.0)
            .hover(p.hover)
            .press(p.press)
            .cursor(Cursor::Pointer)
            .focusable(true)
            .role("tab")
            .help(copy::nav_tooltip(name, i + 1))
            .on_press(move |s: &mut State| s.nav = place)
            .child(label(name, sans(13.0, 500.0), if on { p.text } else { p.muted }));
        if place == Nav::Downloads && active > 0 {
            tab = tab.child(label(active.to_string(), mono(12.0, 400.0), p.accent_ink));
        }
        if on {
            tab = tab.child(el().size(16.0, 2.0).radius(1.0).fill(p.accent).absolute(12.0, 36.0));
        }
        tabs = tabs.child(tab);
    }
    // While anything is in flight the band's lower edge carries the accent: the desk is live.
    let edge = if active > 0 { p.accent.alpha(0.5) } else { p.line };
    row()
        .key("titlebar")
        .w_full()
        .h(crate::TITLEBAR)
        .fill(p.deep)
        .border_sides([0.0, 0.0, 1.0, 0.0], edge)
        .center_y()
        .window_drag()
        .child(mark)
        .child(spacer())
        .child(tabs)
        .child(spacer())
        .child(el().w(240.0f32.max(trailing + 12.0)))
}

/// Home with history: the bar, its caption, what is in flight and what landed.
/// In the `nav` scene the bar is in command mode with its panel dropped.
fn home(s: &State, p: &Pal) -> El {
    let command = s.nav_open;
    // What the field found drops from the bar like the command panel, over the stream.
    let found = if command { None } else { found::view(s, p) };
    let mut top = col().w_full().child(bar(p, &s.input, command, command || found.is_some(), deck(s, p)));
    if command {
        top = top.child(reach::panel(s, p).pin(Some(0.0), Some(INPUT_H + DECK_H + 8.0), Some(0.0), None).raise());
    }
    let mut stream = stream::view(s, p, true).grow(1.0).min_h(0.0).mt(24.0);
    if command || found.is_some() {
        stream = stream.opacity(0.4);
    }
    let mut column = col().w_full().max_w(MEASURE).h_full().min_h(0.0).pt(32.0).child(top).child(caption(s, p).mt(12.0)).child(stream);
    if let Some(found) = found {
        // The drop runs to the window's foot; a press beside it puts it away.
        let drop = col()
            .pin(Some(0.0), Some(32.0 + INPUT_H + DECK_H + 8.0), Some(0.0), Some(24.0))
            .on_press(found::dismiss)
            .child(found.on_press(|_: &mut State| {}));
        column = column.child(drop.raise());
    }
    col().w_full().grow(1.0).min_h(0.0).px(32.0).items_center().child(column)
}

/// First launch: the same frame as Home, the desk switched on with nothing
/// in it. The empty well takes the whole stream: the promise, where it works,
/// and five channels at zero waiting for a link.
fn first_launch(s: &State, p: &Pal) -> El {
    let mut channels = col().w_full();
    for i in 0..5 {
        let fade = [1.0, 0.8, 0.6, 0.4, 0.2][i];
        let ch = row()
            .w_full()
            .h(48.0)
            .px(24.0)
            .gap(16.0)
            .center_y()
            .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
            .opacity(fade)
            .child(label(format!("{:02}", i + 1), mono(12.0, 400.0), p.muted).w(20.0))
            .child(el().size(32.0, 32.0).radius(4.0).border(1.0, p.line))
            .child(label(model::timecode(0.0), mono(13.0, 500.0), p.muted).w(72.0))
            .child(el().grow(1.0).child(meter(0.0, 64, None, 8.0, 2.0, p.accent, p.unlit)));
        channels = channels.child(ch);
    }
    let promise = col()
        .w_full()
        .px(24.0)
        .pt(32.0)
        .pb(32.0)
        .gap(16.0)
        .child(label(copy::HOME_HEADLINE, sans(32.0, 500.0), p.text))
        .child(works_with(p));
    let well = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .mt(24.0)
        .mb(32.0)
        .radius(8.0)
        .fill(p.deep)
        .border(1.0, p.line)
        .clip_rounded(8.0)
        .child(promise)
        .child(channels)
        .child(spacer())
        .child(legend(p).h(48.0).border_sides([1.0, 0.0, 0.0, 0.0], p.line));
    let column = col()
        .w_full()
        .max_w(MEASURE)
        .h_full()
        .min_h(0.0)
        .pt(32.0)
        .child(bar(p, &s.input, false, true, deck(s, p)))
        .child(caption(s, p).mt(12.0))
        .child(well);
    col().w_full().grow(1.0).min_h(0.0).px(32.0).items_center().child(column)
}

/// The bar: the input deck over the readout deck, one surface.
/// `command`: Ctrl+K was pressed. `listening`: the bar has the caret.
fn bar(p: &Pal, input: &str, command: bool, listening: bool, lower: El) -> El {
    let jack = el()
        .size(40.0, 40.0)
        .radius(8.0)
        .fill(p.deep)
        .border(1.0, p.line)
        .center()
        .child(icon(if command { &icons::SEARCH } else { &icons::LINK }, 16.0).color(if listening { p.accent_ink } else { p.muted }));
    let words = if command { copy::PALETTE_PLACEHOLDER } else { copy::FIELD_PLACEHOLDER };
    let mut typed = row().grow(1.0).min_w(0.0).gap(4.0).center_y();
    if listening {
        // The caret is the only lit thing on the line: the bar is listening.
        typed = typed.child(el().size(2.0, 20.0).radius(1.0).fill(p.accent));
    }
    let typed = if command || input.is_empty() {
        typed.child(label(words, sans(16.0, 400.0), p.muted))
    } else {
        typed.child(label(input, sans(16.0, 400.0), p.text))
    };
    let typed = typed.single_line().truncate().min_w(0.0);
    let go = if !input.is_empty() && !input.contains("://") { copy::FIELD_BUTTON_SEARCH } else { copy::FIELD_BUTTON_LOOK_UP };
    let end = if command {
        keycap(copy::PALETTE_ESC_CAP, p)
    } else {
        row().gap(12.0).center_y().child(row().gap(4.0).child(keycap(copy::KEY_CTRL, p)).child(keycap(copy::KEY_K, p))).child(
            row()
                .h(40.0)
                .pl(16.0)
                .pr(12.0)
                .gap(8.0)
                .center_y()
                .radius(8.0)
                .fill(p.text)
                .hover(p.bg.alpha(0.12))
                .press(p.bg.alpha(0.2))
                .cursor(Cursor::Pointer)
                .focusable(true)
                .role("button")
                .child(label(go, sans(14.0, 500.0), p.bg))
                .child(icon(&icons::FORWARD, 16.0).color(p.bg)),
        )
    };
    let input =
        row().w_full().h(INPUT_H).px(12.0).gap(12.0).center_y().role("textbox").label(copy::FIELD_ARIA).child(jack).child(typed).child(end);
    col()
        .w_full()
        .radius(12.0)
        .fill(p.surface)
        .border(1.0, if command { p.accent.alpha(0.6) } else { p.line_strong })
        .clip_rounded(12.0)
        .child(input)
        .child(lower.border_sides([1.0, 0.0, 0.0, 0.0], p.line))
}

/// A key as printed on the board.
pub(crate) fn keycap(key: &str, p: &Pal) -> El {
    el().h(20.0).px(4.0).min_w(20.0).center().radius(4.0).fill(p.raised).border(1.0, p.line_strong).child(label(
        key,
        mono(12.0, 400.0),
        p.text2,
    ))
}

/// A task's cover, or a flat tint of its source with its mark at the one icon size.
pub(crate) fn cover(s: &State, t: &DownloadTask, size: f32, p: &Pal) -> El {
    let base = el().size(size, size).radius(4.0);
    match s.cover(&t.task_id) {
        Some(img) => base.fill(p.raised).pixels_fit(img.pixels(), SurfaceFit::Fill),
        None => {
            let c = source_color(p, t.source);
            base.fill(c.alpha(0.14)).center().child(icon(platform_icon(t.source), 16.0).color(c))
        }
    }
}

/// The four filters of the Downloads list.
pub(crate) const SECTIONS: [&str; 4] = [copy::DL_FILTER_ALL, copy::DL_FILTER_ACTIVE, copy::DL_FILTER_DONE, copy::DL_FILTER_FAILED];

/// The readout deck: this month on the left, the master instrument on the right.
fn deck(s: &State, p: &Pal) -> El {
    let month = match &s.stats {
        Some(st) if st.current.files > 0.0 => {
            let files = st.current.files as u64;
            Line::new()
                .add(&format::month_name(&st.current.month), p.text2)
                .add("  ", p.muted)
                .fig(&files.to_string(), p.text)
                .add(&format!(" {}", copy::file_word(files)), p.text2)
                .add(&copy::row_dot_part(""), p.muted)
                .fig(&format::bytes(st.current.bytes), p.text)
                .one(sans(13.0, 400.0))
        }
        _ => label(copy::MONTH_NOTHING_YET, sans(13.0, 400.0), p.muted),
    };
    row()
        .w_full()
        .h(DECK_H)
        .px(12.0)
        .gap(16.0)
        .center_y()
        .fill(p.deep.alpha(0.6))
        .child(row().pl(4.0).min_w(0.0).child(month))
        .child(spacer())
        .child(instrument(s, p))
}

/// Media carried across today: whole lengths of finished files, the done
/// share of those in flight. The bar's tape counter.
fn carried(tasks: &[DownloadTask]) -> f64 {
    tasks
        .iter()
        .map(|t| match t.progress.status {
            St::Done => t.duration,
            St::Failed | St::Cancelled => 0.0,
            _ => t.duration * t.progress.percent.clamp(0.0, 100.0) / 100.0,
        })
        .sum()
}

/// The master instrument: the tape counter over a level meter of every byte
/// in flight. At rest the meter is dark and the counter holds the day's total.
fn instrument(s: &State, p: &Pal) -> El {
    let tasks = &s.tasks;
    let flying: Vec<&DownloadTask> = tasks.iter().filter(|t| format::is_active(t) && t.progress.total > 0.0).collect();
    let total: f64 = flying.iter().map(|t| t.progress.total).sum();
    let got: f64 = flying.iter().map(|t| t.progress.downloaded).sum();
    let (live, flow) = format::in_flight(tasks);
    let frac = if total > 0.0 { got / total } else { 0.0 };
    let speed = format::speed(flow);
    let figure = if live > 0 && !speed.is_empty() {
        format!("{live}{}", copy::row_dot_part(&speed))
    } else if live > 0 {
        live.to_string()
    } else {
        String::new()
    };
    let ink = if live > 0 { p.text } else { p.text2 };
    let held = live == 0 && format::active_count(tasks) > 0;
    let lit = if held { p.muted } else { p.accent };
    col()
        .w(280.0)
        .h(40.0)
        .px(12.0)
        .gap(4.0)
        .justify_center()
        .radius(6.0)
        .fill(p.deep)
        .border(1.0, p.line)
        .label(copy::transferring_aria(live, &speed))
        .child(
            row().w_full().center_y().child(label(model::timecode(carried(tasks)), mono(13.0, 500.0), ink)).child(spacer()).child(label(
                figure,
                mono(12.0, 400.0),
                p.muted,
            )),
        )
        .child(meter_held(frac, 48, None, 6.0, 2.0, lit, p.unlit, held))
}

/// The rest-state caption under the bar.
fn caption(s: &State, p: &Pal) -> El {
    let (folder, audio) = (config_text(s, "outputDir"), config_text(s, "defaultFormat"));
    let left = row()
        .gap(8.0)
        .center_y()
        .min_w(0.0)
        .child(Line::new().add(&copy::rest_saves_as(&audio), p.text2).add(&folder, p.text).one(sans(13.0, 400.0)).help(folder.clone()))
        .child(
            label(copy::REST_CHANGE, sans(13.0, 500.0), p.accent_ink).cursor(Cursor::Pointer).role("link").label(copy::REST_CHANGE_ARIA),
        );
    let right = row()
        .gap(4.0)
        .center_y()
        .child(keycap(copy::KEY_CTRL, p))
        .child(label(copy::REST_PLUS_SPACED.trim(), sans(13.0, 400.0), p.muted))
        .child(keycap(copy::REST_KEY_V, p))
        .child(label(copy::REST_PASTES_A_LINK, sans(13.0, 400.0), p.text2));
    row().w_full().px(4.0).gap(16.0).center_y().justify_between().child(left).child(right)
}

fn works_with(p: &Pal) -> El {
    let mark = |source: SourcePlatform, name: &'static str| {
        row().gap(8.0).center_y().child(icon(platform_icon(source), 16.0).color(source_color(p, source))).child(label(
            name,
            sans(13.0, 500.0),
            p.text,
        ))
    };
    row()
        .w_full()
        .gap(20.0)
        .center_y()
        .flex_wrap()
        .child(label(copy::WORKS_WITH, sans(13.0, 400.0), p.muted))
        .child(mark(SourcePlatform::Youtube, copy::WORKS_WITH_YOUTUBE))
        .child(mark(SourcePlatform::Spotify, copy::WORKS_WITH_SPOTIFY))
        .child(mark(SourcePlatform::Soundcloud, copy::WORKS_WITH_SOUNDCLOUD))
        .child(mark(SourcePlatform::Instagram, copy::WORKS_WITH_REELS))
        .child(label(copy::WORKS_WITH_TAIL, sans(13.0, 400.0), p.text2))
}

/// The keys that are the whole app, along the foot of the empty desk.
fn legend(p: &Pal) -> El {
    let pair = |keys: &[&str], words: &str| {
        let mut caps = row().gap(4.0);
        for k in keys {
            caps = caps.child(keycap(k, p));
        }
        row().gap(8.0).center_y().child(caps).child(label(words, sans(13.0, 400.0), p.text2))
    };
    row()
        .w_full()
        .px(24.0)
        .gap(32.0)
        .center_y()
        .child(pair(&[copy::KEY_CTRL, copy::KEY_K], copy::SHORTCUT_PALETTE))
        .child(pair(&[copy::KEY_CTRL, &copy::keycap_tab_range(3)], copy::RAIL_ARIA))
        .child(pair(&[copy::KEY_CTRL, copy::KEYCAP_SLASH], copy::SHORTCUTS_TITLE))
}
