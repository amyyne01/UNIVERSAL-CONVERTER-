//! Settings: the desk's patch sheet. The folder leads in a recessed well (the
//! one answer people come here for); then each section is a label column on
//! the left and its panel of rows on the right, so the eye runs down the
//! names. Panels vary by what they hold: a field grid for the choices, rows
//! for switches, status lines for the two updaters. Wording is
//! `copy::settings`, word for word; values are stand-ins (the gate has no settings).
use rune::{Color, El, ScrollSpec, col, el, icon, row};

use super::icons;
use super::model::{AUDIO_FORMAT, OUTPUT_DIR};
use super::parts::{Line, button, dot, label, mono, sans};
use super::theme::{self, Pal};
use crate::copy::settings as w;
use crate::gate::{self, State};

/// The section names' column.
const NAMES: f32 = 200.0;
/// A select's or a field's width in a row.
const CONTROL: f32 = 176.0;
/// Palette names, `s.palette` order: Signal's own, then the app's seven.
const PALETTES: [&str; 8] = ["Signal", "Aurora", "Slate", "Moss", "Ember", "Paper", "Lagoon", "Orchid"];

pub fn view(s: &State, p: &Pal) -> El {
    let head = row()
        .w_full()
        .items_end()
        .gap(16.0)
        .child(label(w::TITLE, sans(28.0, 500.0), p.text))
        .child(label(w::SAVES, sans(13.0, 400.0), p.muted).mb(4.0));
    let page = col()
        .w_full()
        .max_w(1120.0)
        .px(32.0)
        .pt(28.0)
        .pb(48.0)
        .gap(32.0)
        .child(head)
        .child(destination(p))
        .child(section(w::DEFAULTS, Some(w::DEFAULTS_HINT), defaults(p), p))
        .child(section(w::MORE, Some(w::MORE_HINT), more(p), p))
        .child(section(w::APPEARANCE, None, appearance(s, p), p))
        .child(section(w::SCHEDULE, Some(w::SCHEDULE_HINT), schedule(p), p))
        .child(section(w::SOFTWARE, Some(w::SOFTWARE_HINT), software(p), p))
        .child(section(w::ADVANCED, Some(w::ADVANCED_HINT), advanced(p), p))
        .child(section(w::PLAN, None, plan(p), p))
        .child(section(w::HELP, None, help(p), p));
    col().key("settings").w_full().h_full().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).items_center().child(page)
}

/// Destination has no heading: the folder, set like a tape label in the desk's well.
fn destination(p: &Pal) -> El {
    let (parent, leaf) = OUTPUT_DIR.rsplit_once('\\').map_or(("", OUTPUT_DIR), |(a, b)| (a, b));
    row()
        .w_full()
        .h(72.0)
        .pl(20.0)
        .pr(16.0)
        .gap(16.0)
        .center_y()
        .radius(12.0)
        .fill(p.deep)
        .border(1.0, p.line_strong)
        .child(
            el().size(40.0, 40.0)
                .center()
                .radius(8.0)
                .fill(p.surface)
                .border(1.0, p.line)
                .child(icon(&icons::FOLDER, 16.0).color(p.accent_ink)),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(label(w::SAVING_TO, sans(12.0, 400.0), p.text2))
                .child(Line::new().fig(&format!("{parent}\\"), p.text2).fig(leaf, p.text).one(mono(16.0, 500.0)).help(OUTPUT_DIR)),
        )
        .child(button(Some(&icons::FOLDER), w::CHANGE, p.text, p))
}

/// A section: its name and hint in the left column, its panel beside them.
fn section(title: &str, hint: Option<&str>, body: El, p: &Pal) -> El {
    let mut names = col().w(NAMES).pt(8.0).gap(4.0).child(label(title, sans(16.0, 500.0), p.text));
    if let Some(h) = hint {
        names = names.child(label(h, sans(12.0, 400.0).lh(16.0), p.text2).wrap());
    }
    row().w_full().gap(24.0).items_start().child(names).child(body.grow(1.0).min_w(0.0))
}

/// The panel a section's rows sit on; a hairline between them.
fn panel(parts: Vec<El>, p: &Pal) -> El {
    let mut c = col().radius(12.0).fill(p.surface).border(1.0, p.line).clip_rounded(12.0);
    for (i, part) in parts.into_iter().enumerate() {
        c = c.child(if i > 0 { part.border_sides([1.0, 0.0, 0.0, 0.0], p.line) } else { part });
    }
    c
}

/// A setting: its name and description, the control on the right.
fn setting(words: (&str, &str), control: El, p: &Pal) -> El {
    setting_dim(words, control, false, p)
}

/// A setting that cannot be changed right now (it depends on another one): drawn a step back.
fn setting_dim(words: (&str, &str), control: El, dim: bool, p: &Pal) -> El {
    let (name, desc) = words;
    let mut text = col().grow(1.0).min_w(0.0).gap(4.0).child(label(name, sans(14.0, 400.0), if dim { p.muted } else { p.text }));
    if !desc.is_empty() {
        text = text.child(label(desc, sans(12.0, 400.0).lh(16.0), if dim { p.muted } else { p.text2 }).wrap());
    }
    let r = row().w_full().min_h(56.0).px(16.0).py(12.0).gap(16.0).center_y().child(text).child(control);
    if dim { r.opacity(0.6) } else { r }
}

/// A switch on the desk: a square-shouldered toggle; on, its track is the lit segment.
fn switch(name: &str, on: bool, p: &Pal) -> El {
    el().size(36.0, 20.0)
        .radius(6.0)
        .fill(if on { p.accent } else { p.deep })
        .border(1.0, if on { p.accent } else { p.line_strong })
        .cursor(rune::Cursor::Pointer)
        .focusable(true)
        .role("switch")
        .label(name.to_string())
        .checked(on)
        .child(el().size(12.0, 12.0).radius(4.0).fill(if on { p.deep } else { p.muted }).absolute(if on { 19.0 } else { 3.0 }, 3.0))
}

fn well(w_: Option<f32>, name: &str, role: &'static str) -> El {
    let r = row()
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .focusable(true)
        .role(role)
        .label(name.to_string())
        .cursor(rune::Cursor::Pointer);
    match w_ {
        Some(v) => r.w(v),
        None => r.w_full(),
    }
}

/// A closed select showing its value.
fn select(name: &str, value: &str, w_: Option<f32>, p: &Pal) -> El {
    well(w_, name, "combobox")
        .fill(p.deep)
        .border(1.0, p.line_strong)
        .hover(p.hover)
        .child(label(value, sans(13.0, 400.0), p.text).single_line().truncate().grow(1.0).min_w(0.0))
        .child(icon(&icons::CARET_DOWN, 16.0).color(p.muted))
}

/// A typed field: figures in mono; an empty one shows its placeholder.
fn input(name: &str, value: &str, placeholder: &str, w_: Option<f32>, p: &Pal) -> El {
    let (t, c) = if value.is_empty() { (placeholder, p.muted) } else { (value, p.text) };
    well(w_, name, "textbox")
        .fill(p.deep)
        .border(1.0, p.line_strong)
        .child(label(t, mono(13.0, 400.0), c).single_line().truncate().min_w(0.0))
}

/// Choices set side by side, each with its name above it.
fn fields(items: Vec<(&str, El)>, p: &Pal) -> El {
    let mut r = row().w_full().px(16.0).pt(16.0).pb(20.0).gap(12.0);
    for (name, control) in items {
        r = r.child(col().grow(1.0).min_w(0.0).gap(8.0).child(label(name, sans(12.0, 400.0), p.text2)).child(control));
    }
    r
}

fn defaults(p: &Pal) -> El {
    let format = AUDIO_FORMAT.to_uppercase();
    panel(
        vec![
            fields(
                vec![
                    (w::AUDIO_FORMAT, select(w::AUDIO_FORMAT, &format, None, p)),
                    (w::AUDIO_QUALITY, select(w::AUDIO_QUALITY, "320 kbps", None, p)),
                    (w::VIDEO_QUALITY, select(w::VIDEO_QUALITY, "1080p Full HD", None, p)),
                ],
                p,
            ),
            setting(w::SKIP_EXISTING, switch(w::SKIP_EXISTING.0, true, p), p),
            setting(w::EMBED_THUMBNAIL, switch(w::EMBED_THUMBNAIL.0, true, p), p),
            setting(w::EMBED_METADATA, switch(w::EMBED_METADATA.0, true, p), p),
            setting(w::SIMULTANEOUS, select(w::SIMULTANEOUS.0, "2 at a time", Some(CONTROL), p), p),
        ],
        p,
    )
}

fn more(p: &Pal) -> El {
    // Subtitles are off, so the language and captions settings sit a step back, as in Electron.
    panel(
        vec![
            fields(
                vec![
                    (w::SUBTITLES, select(w::SUBTITLES, "Off", None, p)),
                    (w::SUBTITLE_LANGUAGES, input(w::SUBTITLE_LANGUAGES, "", w::SUBTITLE_LANGUAGES_PLACEHOLDER, None, p).opacity(0.6)),
                    (w::SPONSOR_SEGMENTS, select(w::SPONSOR_SEGMENTS, "Off", None, p)),
                ],
                p,
            ),
            fields(
                vec![
                    (w::VIDEO_CONTAINER, select(w::VIDEO_CONTAINER, "MP4 (most compatible)", None, p)),
                    (w::VIDEO_CODEC, select(w::VIDEO_CODEC, "Best available", None, p)),
                    (w::SIGN_IN_WITH, select(w::SIGN_IN_WITH, "Chrome", None, p)),
                ],
                p,
            ),
            setting(w::IMPORT_COOKIES, button(Some(&icons::FOLDER), w::CHOOSE_FILE, p.text, p), p),
            setting_dim(w::AUTO_CAPTIONS, switch(w::AUTO_CAPTIONS.0, false, p), true, p),
            setting(w::EMBED_CHAPTERS, switch(w::EMBED_CHAPTERS.0, true, p), p),
            setting(w::SPLIT_BY_CHAPTER, switch(w::SPLIT_BY_CHAPTER.0, false, p), p),
            setting(w::SAVE_COVER_ART, switch(w::SAVE_COVER_ART.0, false, p), p),
            setting(w::FILE_NAMING, input(w::FILE_NAMING.0, "", w::FILE_NAMING_PLACEHOLDER, Some(CONTROL), p), p),
        ],
        p,
    )
}

fn appearance(s: &State, p: &Pal) -> El {
    // The theme follows the system in the gate (the picture's `--theme` stands for the OS).
    let mut theme = row().padding(4.0).gap(4.0).radius(8.0).fill(p.deep).border(1.0, p.line).role("radiogroup").label(w::THEME.0);
    for (i, name) in w::THEMES.into_iter().enumerate() {
        let on = i == 2;
        theme = theme.child(
            el().h(28.0)
                .px(12.0)
                .center()
                .radius(6.0)
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .hover(p.hover)
                .cursor(rune::Cursor::Pointer)
                .role("radio")
                .checked(on)
                .child(label(name, sans(13.0, if on { 500.0 } else { 400.0 }), if on { p.text } else { p.text2 })),
        );
    }
    let mut swatches = row().gap(8.0).flex_wrap().justify_end().role("radiogroup").label(w::PALETTE_ARIA);
    for (i, name) in PALETTES.iter().enumerate() {
        let sw = theme::pal(i, p.dark);
        let on = s.palette == i;
        swatches = swatches.child(
            el().size(28.0, 28.0)
                .radius(8.0)
                .center()
                .fill(sw.deep)
                .border(if on { 2.0 } else { 1.0 }, if on { p.text } else { p.line_strong })
                .cursor(rune::Cursor::Pointer)
                .role("radio")
                .label(*name)
                .help(*name)
                .checked(on)
                .on_press(move |s: &mut State| s.palette = i)
                // The palette's accent as one lit segment.
                .child(el().size(12.0, 4.0).radius(1.0).fill(sw.accent)),
        );
    }
    let preset = gate::PRESETS[gate::preset(s)].0;
    let layout = row()
        .gap(12.0)
        .center_y()
        .child(label(preset, mono(13.0, 400.0), p.text2))
        .child(button(None, w::LAYOUT_CHOOSE, p.text, p).key("layout-choose").on_press(|s: &mut State| s.picker = true));
    panel(
        vec![
            setting(w::THEME, theme, p),
            setting(w::PALETTE, swatches.max_w(280.0), p),
            setting(w::LAYOUT, layout, p),
            setting(w::CLIPBOARD_WATCHER, switch(w::CLIPBOARD_WATCHER.0, false, p), p),
            setting(w::NOTIFICATIONS, switch(w::NOTIFICATIONS.0, true, p), p),
            setting(w::DISCORD, switch(w::DISCORD.0, false, p), p),
        ],
        p,
    )
}

fn schedule(p: &Pal) -> El {
    let mut days = row().gap(4.0).role("group").label(w::ACTIVE_DAYS.0);
    for (i, day) in w::DAYS.iter().enumerate() {
        let on = (1..=5).contains(&i);
        days = days.child(
            el().size(32.0, 28.0)
                .center()
                .radius(6.0)
                .fill(if on { p.raised } else { p.deep })
                .border(1.0, if on { p.line_strong } else { p.line })
                .cursor(rune::Cursor::Pointer)
                .focusable(true)
                .role("checkbox")
                .label(*day)
                .checked(on)
                .child(label(&day[..2], mono(12.0, 500.0), if on { p.text } else { p.muted })),
        );
    }
    panel(
        vec![
            setting((w::ENABLE_SCHEDULER, ""), switch(w::ENABLE_SCHEDULER, true, p), p),
            setting(w::START_TIME, input(w::START_TIME.0, "03:00", "", Some(80.0), p), p),
            setting(w::ACTIVE_DAYS, days, p),
            setting(w::SHUTDOWN, switch(w::SHUTDOWN.0, false, p), p),
        ],
        p,
    )
}

/// An updater: name and version, its state on a tally light, its one button.
fn status(name: &str, version: &str, state: &str, tone: Color, action: El, p: &Pal) -> El {
    let words = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .child(row().gap(8.0).center_y().child(label(name, sans(14.0, 400.0), p.text)).child(label(version, mono(12.0, 400.0), p.muted)))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(dot(tone, 8.0))
                .child(label(state, sans(12.0, 400.0), p.text2).single_line().truncate().min_w(0.0)),
        );
    row().w_full().min_h(64.0).px(16.0).py(12.0).gap(16.0).center_y().child(words).child(action)
}

/// Not in `copy::settings`: Electron's rest line for an engine that is up to date (SettingsTab.tsx L364).
const ENGINE_UP_TO_DATE: &str = "yt-dlp is up to date";

fn software(p: &Pal) -> El {
    panel(
        vec![
            status(w::APP_NAME, "v2.0.0", w::APP_REST, p.muted, button(Some(&icons::REFRESH), w::CHECK_FOR_UPDATES, p.text, p), p),
            status(
                w::ENGINE_NAME,
                "2026.11.01",
                ENGINE_UP_TO_DATE,
                p.success,
                button(Some(&icons::REFRESH), w::ENGINE_CHECK, p.text, p),
                p,
            ),
            setting(w::AUTO_UPDATE_ENGINE, switch(w::AUTO_UPDATE_ENGINE.0, true, p), p),
        ],
        p,
    )
}

fn advanced(p: &Pal) -> El {
    panel(
        vec![
            setting(w::RATE_LIMIT, input(w::RATE_LIMIT.0, "", w::RATE_LIMIT_PLACEHOLDER, Some(CONTROL), p), p),
            setting(w::PROXY, input(w::PROXY.0, "", w::PROXY_PLACEHOLDER, Some(CONTROL), p), p),
            setting(w::GLOBAL_HOTKEY, input(w::GLOBAL_HOTKEY.0, "Ctrl+Shift+D", w::GLOBAL_HOTKEY_PLACEHOLDER, Some(CONTROL), p), p),
        ],
        p,
    )
}

fn plan(p: &Pal) -> El {
    // The licence actions cost the user something: their names in the error colour.
    let danger = |key: &str, (name, desc): (&str, &str)| {
        row()
            .key(key.to_string())
            .w_full()
            .min_h(56.0)
            .px(16.0)
            .py(12.0)
            .gap(16.0)
            .center_y()
            .hover(p.hover)
            .cursor(rune::Cursor::Pointer)
            .focusable(true)
            .role("button")
            .label(name.to_string())
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(4.0)
                    .child(label(name, sans(14.0, 400.0), p.error))
                    .child(label(desc, sans(12.0, 400.0).lh(16.0), p.text2).wrap()),
            )
            .child(icon(&icons::CARET_RIGHT, 16.0).color(p.muted))
    };
    let (premium, desc) = w::PLAN_PREMIUM;
    let lit = el().h(24.0).px(8.0).center().radius(4.0).fill(p.accent).child(label(premium, sans(12.0, 500.0), p.deep));
    let yours = row()
        .w_full()
        .min_h(64.0)
        .px(16.0)
        .py(12.0)
        .gap(16.0)
        .center_y()
        .child(col().grow(1.0).min_w(0.0).gap(4.0).child(label(w::YOUR_PLAN, sans(14.0, 400.0), p.text)).child(label(
            desc,
            sans(12.0, 400.0),
            p.text2,
        )))
        .child(lit);
    panel(vec![yours, danger("plan-deactivate", w::DEACTIVATE), danger("plan-move", w::MOVE)], p)
}

fn help(p: &Pal) -> El {
    panel(
        vec![
            setting(w::SHORTCUTS, button(None, w::SHOW_SHORTCUTS, p.text, p), p),
            setting(w::REPORT, button(Some(&icons::EXTERNAL), w::GET_SUPPORT, p.text, p), p),
        ],
        p,
    )
}
