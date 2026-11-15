//! Layout "One bar", wearing the Signal look. There are no pages and no
//! sidebar: the window is one paste-and-command bar, and every download
//! streams under it. The bar has two decks. The upper one takes a link, a
//! search or a command; the lower one is the desk's readout: the stream's
//! sections on the left, the master instrument (a tape counter of the media
//! carried across today, over a level meter of everything in flight) on the
//! right. Whatever is not a download opens as a panel that drops out of the
//! bar over the stream: a list of what matches on the left, the chosen
//! sheet on the right.
//!
//! Where the old app's screens and features live now:
//!
//! | Old (Electron 1.x)                         | One bar                                                             |
//! |--------------------------------------------|---------------------------------------------------------------------|
//! | Home: paste field, Look up, status line    | The bar's upper deck; the status line is the caption under the bar  |
//! | Home: rest caption (folder, format, Ctrl+V)| The caption under the bar; `Change` opens the Destination sheet     |
//! | Home: search results                       | A panel dropped from the bar, bands per source                      |
//! | Home: Speed cell                           | The master instrument's speed figure, and the In progress header    |
//! | Home: This month, Where it came from       | `This month` sheet in the bar (its row shows the file count)        |
//! | Home: Recent downloads                     | The stream's Completed section                                      |
//! | Home: Works with (first launch)            | Under the bar on first launch                                       |
//! | Downloads: header counts and summary       | The deck's section tabs (count per section)                         |
//! | Downloads: filters All/Active/Done/Failed  | The deck's section tabs, Ctrl+1..4                                  |
//! | Downloads: search                          | Typing a non-link in the bar also matches downloads                 |
//! | Downloads: Pause/Resume/Cancel all, Retry  | Each section header carries its own; all are bar commands too       |
//! |   all failed, Clear completed              |                                                                     |
//! | Downloads: interrupted banner              | The first line of the In progress well                              |
//! | Downloads: rows, row actions, reorder      | The stream (live rows sit in the recessed well)                     |
//! | Downloads: selection (checkboxes)          | Ctrl/Shift-click a row; the selection's actions show in the deck    |
//! | YouTube / Spotify / SoundCloud / Reels tabs| Search scopes in the bar (Navigate group); a playlist preview drops |
//! |                                            |   from the bar like a sheet                                         |
//! | Settings: every section                    | One sheet each, from the bar (`Settings` group)                     |
//! | Settings: Scheduled downloads              | `Scheduled downloads` sheet                                         |
//! | Settings: Software (app update, engine)    | `Software` sheet; `Check for updates` is also a bar command         |
//! | Settings: Plan & license, deactivate/move  | `Plan & license` sheet                                              |
//! | Settings: cookies import, sign in with     | `More download options` sheet                                       |
//! | Settings: theme and palette                | `Appearance & behavior` sheet; `Switch to dark mode` command        |
//! | Command palette (Ctrl+K)                   | The bar itself: Ctrl+K focuses it, typing filters                   |
//! | Keyboard shortcuts sheet (Ctrl+/)          | A sheet from the bar                                                |
//! | Notices / toasts                           | A line in the bar's lower deck, where the sections sit              |
//! | Rail shortcuts Ctrl+1..7                   | Ctrl+1..4 pick a section; Ctrl+7 still opens Settings in the bar    |
//! | Badge on Downloads, carrier line           | The titlebar's lower edge lights while anything is in flight        |
mod reach;
mod stream;

use ahg_engine::types::{DownloadStatus as St, DownloadTask, SourcePlatform};
use rune::{Color, Cursor, Cx, El, col, el, icon, row, spacer};

use crate::copy;
use crate::directions::signal::icons;
use crate::directions::signal::model;
use crate::directions::signal::parts::{Line, label, meter, meter_held, mono, platform_icon, sans};
use crate::directions::signal::theme::{self, Pal, source_color};
use crate::gate::State;

/// The bar's measure, and the stream's.
pub(crate) const MEASURE: f32 = 960.0;
const INPUT_H: f32 = 64.0;
const DECK_H: f32 = 56.0;
/// The shared state carries no settings: the folder and format the samples use.
pub(crate) const OUTPUT_DIR: &str = "C:\\Users\\Alex\\Music";
pub(crate) const AUDIO_FORMAT: &str = "mp3";

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = theme::pal(s.palette, cx.system_dark());
    cx.caption_color(if p.dark { Color::WHITE } else { Color::BLACK });
    let trailing = cx.chrome_insets().trailing;
    let first = s.tasks.is_empty() && s.stats.is_none() && !s.reach;
    let body = if first { first_launch(&p) } else { desk(s, &p) };
    col().w_full().h_full().fill(p.bg).child(titlebar(s, &p, trailing)).child(body)
}

fn titlebar(s: &State, p: &Pal, trailing: f32) -> El {
    let logo = match &s.logo {
        Some(img) if img.is_ready() => el().size(16.0, 16.0).pixels(img.pixels()),
        _ => el().size(16.0, 16.0),
    };
    let mark = row().gap(8.0).center_y().child(logo).child(Line::new().strong(copy::BRAND, p.text).add(copy::BRAND_REST, p.muted).one(mono(12.0, 400.0)));
    // While anything is in flight the band's lower edge carries the accent: the desk is live.
    let edge = if model::active_count(&s.tasks) > 0 { p.accent.alpha(0.5) } else { p.line };
    row()
        .key("titlebar")
        .w_full()
        .h(crate::TITLEBAR)
        .fill(p.deep)
        .border_sides([0.0, 0.0, 1.0, 0.0], edge)
        .center_y()
        .window_drag()
        .child(row().pl(16.0).child(mark))
        .child(spacer())
        .child(el().w(trailing + 12.0))
}

/// The window with history: the bar, its caption, the stream.
fn desk(s: &State, p: &Pal) -> El {
    let mut top = col().w_full().child(bar(p, s.reach, s.reach, deck(s, p)));
    if s.reach {
        top = top.child(reach::panel(s, p).pin(Some(0.0), Some(INPUT_H + DECK_H + 8.0), Some(0.0), None).raise());
    }
    let mut stream = stream::view(s, p).grow(1.0).min_h(0.0).mt(24.0);
    if s.reach {
        stream = stream.opacity(0.4);
    }
    let column = col().w_full().max_w(MEASURE).h_full().min_h(0.0).pt(32.0).child(top).child(caption(p).mt(12.0)).child(stream);
    col().w_full().grow(1.0).min_h(0.0).px(32.0).items_center().child(column)
}

/// First launch: nothing downloaded, nothing counted. The bar stands alone
/// with the desk at rest under it; the line says what goes in.
fn first_launch(p: &Pal) -> El {
    let headline = label(copy::HOME_HEADLINE, sans(32.0, 500.0).tracking(-0.6), p.text);
    let column = col()
        .w_full()
        .max_w(MEASURE)
        .child(headline.mb(24.0))
        .child(bar(p, false, true, rest_deck(p)))
        .child(caption(p).mt(12.0))
        .child(works_with(p).mt(40.0));
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(32.0)
        .pb(48.0)
        .items_center()
        .justify_center()
        .child(column)
        .child(legend(p).absolute(0.0, 0.0).pin(Some(32.0), None, Some(32.0), Some(24.0)))
}

/// The bar: the input deck over the readout deck, one surface.
/// `command`: Ctrl+K was pressed. `listening`: the bar has the caret.
fn bar(p: &Pal, command: bool, listening: bool, lower: El) -> El {
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
    let typed = typed.child(label(words, sans(16.0, 400.0), p.muted).single_line().truncate().min_w(0.0));
    let end = if command {
        keycap(copy::PALETTE_ESC_CAP, p)
    } else {
        row()
            .gap(12.0)
            .center_y()
            .child(row().gap(4.0).child(keycap(copy::KEY_CTRL, p)).child(keycap(copy::KEY_K, p)))
            .child(
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
                    .child(label(copy::FIELD_BUTTON_LOOK_UP, sans(14.0, 500.0), p.bg))
                    .child(icon(&icons::FORWARD, 16.0).color(p.bg)),
            )
    };
    let input = row().w_full().h(INPUT_H).px(12.0).gap(12.0).center_y().role("textbox").label(copy::FIELD_ARIA).child(jack).child(typed).child(end);
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
    el().h(20.0).px(4.0).min_w(20.0).center().radius(4.0).fill(p.raised).border(1.0, p.line_strong).child(label(key, mono(12.0, 400.0), p.text2))
}

/// The four sections of the stream, Ctrl+1..4.
pub(crate) const SECTIONS: [&str; 4] = [copy::DL_FILTER_ALL, copy::DL_FILTER_ACTIVE, copy::DL_FILTER_DONE, copy::DL_FILTER_FAILED];

pub(crate) fn section_count(tasks: &[DownloadTask], i: usize) -> usize {
    tasks
        .iter()
        .filter(|t| match i {
            0 => true,
            1 => model::is_active(t),
            2 => t.progress.status == St::Done,
            _ => matches!(t.progress.status, St::Failed | St::Cancelled),
        })
        .count()
}

/// The readout deck: sections on the left, the master instrument on the right.
fn deck(s: &State, p: &Pal) -> El {
    let mut tabs = row().gap(4.0).center_y().role("radiogroup").label(copy::DL_FILTER_ARIA);
    for (i, name) in SECTIONS.into_iter().enumerate() {
        let on = i == 0;
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
            .role("radio")
            .help(copy::nav_tooltip(name, i + 1))
            .child(label(name, sans(13.0, if on { 500.0 } else { 400.0 }), if on { p.text } else { p.text2 }))
            .child(label(section_count(&s.tasks, i).to_string(), mono(12.0, 400.0), p.muted));
        if on {
            // The chosen section is one lit segment, like the meter's head.
            tab = tab.fill(p.raised).child(el().size(12.0, 2.0).radius(1.0).fill(p.accent).absolute(12.0, 28.0));
        }
        tabs = tabs.child(tab);
    }
    row().w_full().h(DECK_H).px(12.0).gap(16.0).center_y().fill(p.deep.alpha(0.6)).child(tabs).child(spacer()).child(instrument(s, p))
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
    let flying: Vec<&DownloadTask> = tasks.iter().filter(|t| model::is_active(t) && t.progress.total > 0.0).collect();
    let total: f64 = flying.iter().map(|t| t.progress.total).sum();
    let got: f64 = flying.iter().map(|t| t.progress.downloaded).sum();
    let live = tasks.iter().filter(|t| model::is_live(t)).count();
    let frac = if total > 0.0 { got / total } else { 0.0 };
    let speed = copy::format_speed(model::total_speed(tasks));
    let figure = if live > 0 && !speed.is_empty() { format!("{live}{}", copy::row_dot_part(&speed)) } else if live > 0 { live.to_string() } else { String::new() };
    let ink = if live > 0 { p.text } else { p.text2 };
    let held = live == 0 && model::active_count(tasks) > 0;
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
        .child(row().w_full().center_y().child(label(model::timecode(carried(tasks)), mono(13.0, 500.0), ink)).child(spacer()).child(label(figure, mono(12.0, 400.0), p.muted)))
        .child(meter_held(frac, 48, None, 6.0, 2.0, lit, p.unlit, held))
}

/// First launch's lower deck: the input's level, every segment dark, and the
/// counter at zero. Nothing has come in yet.
fn rest_deck(p: &Pal) -> El {
    row()
        .w_full()
        .h(DECK_H)
        .px(12.0)
        .gap(12.0)
        .center_y()
        .fill(p.deep.alpha(0.6))
        .child(label(model::timecode(0.0), mono(13.0, 500.0), p.muted))
        .child(el().grow(1.0).child(meter(0.0, 96, None, 8.0, 2.0, p.accent, p.unlit)))
}

/// The rest-state caption under the bar.
fn caption(p: &Pal) -> El {
    let left = row()
        .gap(8.0)
        .center_y()
        .min_w(0.0)
        .child(Line::new().add(&copy::rest_saves_as(AUDIO_FORMAT), p.text2).add(OUTPUT_DIR, p.text).one(sans(13.0, 400.0)).help(OUTPUT_DIR))
        .child(label(copy::REST_CHANGE, sans(13.0, 500.0), p.accent_ink).cursor(Cursor::Pointer).role("link").label(copy::REST_CHANGE_ARIA));
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
    let mark = |source: SourcePlatform, name: &'static str| row().gap(8.0).center_y().child(icon(platform_icon(source), 16.0).color(source_color(p, source))).child(label(name, sans(13.0, 500.0), p.text));
    row()
        .w_full()
        .px(4.0)
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

/// The three keys that are the whole app, set along the window's foot.
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
        .gap(32.0)
        .center()
        .child(pair(&[copy::KEY_CTRL, copy::KEY_K], copy::SHORTCUT_PALETTE))
        .child(pair(&[copy::KEY_CTRL, copy::KEYCAP_SLASH], copy::SHORTCUT_SHOW))
        .child(pair(&[copy::KEYCAP_ESC], copy::SHORTCUT_CLOSE))
}
