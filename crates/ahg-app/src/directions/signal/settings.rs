//! Settings: the desk's patch sheet. The folder leads in a recessed well (the
//! one answer people come here for); then each section is a label column on
//! the left and its panel of rows on the right, so the eye runs down the
//! names. Panels vary by what they hold: a field grid for the choices, rows
//! for switches, status lines for the two updaters. Wording is
//! `copy::settings`, word for word; values come from `State.settings` through `settings_spec`.
use rune::{Color, El, ScrollSpec, col, el, icon, row};

use super::parts::{Line, Spec, button, dot, label, mono, sans};
use super::theme::{self, Pal};
use crate::copy::{self, settings as w};
use crate::gate::{self, Confirm, State};
use crate::icons;
use crate::keys;
use crate::palette;

/// The section names' column.
const NAMES: f32 = 200.0;
/// A select's or a field's width in a row.
const CONTROL: f32 = 176.0;

/// Palette names, `s.palette` order: Signal's own, then the app's seven.
fn palette_name(i: usize) -> &'static str {
    if i == 0 { copy::PRESET_SIGNAL } else { palette::NAMES[i - 1] }
}

pub fn view(s: &State, p: &Pal) -> El {
    let v = Spec::new(s);
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
        .child(destination(&v, p))
        .child(section(w::DEFAULTS, Some(w::DEFAULTS_HINT), defaults(&v, p), p))
        .child(section(w::MORE, Some(w::MORE_HINT), more(&v, p), p))
        .child(section(w::APPEARANCE, None, appearance(s, &v, p), p))
        .child(section(w::SCHEDULE, Some(w::SCHEDULE_HINT), schedule(&v, p), p))
        .child(section(w::SOFTWARE, Some(w::SOFTWARE_HINT), software(s, &v, p), p))
        .child(section(w::ADVANCED, Some(w::ADVANCED_HINT), advanced(&v, p), p))
        .child(section(w::PLAN, None, plan(p), p))
        .child(section(w::HELP, None, help(p), p));
    col().key("settings").w_full().h_full().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).items_center().child(page)
}

/// Destination has no heading: the folder, set like a tape label in the desk's well.
fn destination(v: &Spec, p: &Pal) -> El {
    let dir = v.text("outputDir");
    let (parent, leaf) = dir.rsplit_once('\\').map_or(("", dir.as_str()), |(a, b)| (a, b));
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
                .child(Line::new().fig(&format!("{parent}\\"), p.text2).fig(leaf, p.text).one(mono(16.0, 500.0)).help(dir.clone())),
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

fn defaults(v: &Spec, p: &Pal) -> El {
    panel(
        vec![
            fields(
                vec![
                    (w::AUDIO_FORMAT, select(w::AUDIO_FORMAT, &v.pick("defaultFormat"), None, p)),
                    (w::AUDIO_QUALITY, select(w::AUDIO_QUALITY, &v.pick("defaultQuality"), None, p)),
                    (w::VIDEO_QUALITY, select(w::VIDEO_QUALITY, &v.pick("defaultVideoQuality"), None, p)),
                ],
                p,
            ),
            setting(w::SKIP_EXISTING, switch(w::SKIP_EXISTING.0, v.on("skipExisting"), p), p),
            setting(w::EMBED_THUMBNAIL, switch(w::EMBED_THUMBNAIL.0, v.on("embedThumbnail"), p), p),
            setting(w::EMBED_METADATA, switch(w::EMBED_METADATA.0, v.on("embedMetadata"), p), p),
            setting(w::SIMULTANEOUS, select(w::SIMULTANEOUS.0, &v.pick("maxConcurrentDownloads"), Some(CONTROL), p), p),
        ],
        p,
    )
}

fn more(v: &Spec, p: &Pal) -> El {
    // With subtitles off the language and captions settings sit a step back, as in Electron.
    let subtitles_off = v.text("subtitleMode") == "off";
    panel(
        vec![
            fields(
                vec![
                    (w::SUBTITLES, select(w::SUBTITLES, &v.pick("subtitleMode"), None, p)),
                    (
                        w::SUBTITLE_LANGUAGES,
                        input(w::SUBTITLE_LANGUAGES, &v.text("subtitleLangs"), w::SUBTITLE_LANGUAGES_PLACEHOLDER, None, p)
                            .opacity(if subtitles_off { 0.6 } else { 1.0 }),
                    ),
                    (w::SPONSOR_SEGMENTS, select(w::SPONSOR_SEGMENTS, &v.pick("sponsorBlock"), None, p)),
                ],
                p,
            ),
            fields(
                vec![
                    (w::VIDEO_CONTAINER, select(w::VIDEO_CONTAINER, &v.pick("videoContainer"), None, p)),
                    (w::VIDEO_CODEC, select(w::VIDEO_CODEC, &v.pick("videoCodec"), None, p)),
                    (w::SIGN_IN_WITH, select(w::SIGN_IN_WITH, &v.pick("cookieBrowser"), None, p)),
                ],
                p,
            ),
            setting(w::IMPORT_COOKIES, button(Some(&icons::FOLDER), w::CHOOSE_FILE, p.text, p), p),
            setting_dim(w::AUTO_CAPTIONS, switch(w::AUTO_CAPTIONS.0, v.on("subtitleAuto"), p), subtitles_off, p),
            setting(w::EMBED_CHAPTERS, switch(w::EMBED_CHAPTERS.0, v.on("embedChapters"), p), p),
            setting(w::SPLIT_BY_CHAPTER, switch(w::SPLIT_BY_CHAPTER.0, v.on("splitChapters"), p), p),
            setting(w::SAVE_COVER_ART, switch(w::SAVE_COVER_ART.0, v.on("writeThumbnail"), p), p),
            setting(w::FILE_NAMING, input(w::FILE_NAMING.0, &v.text("outputTemplate"), w::FILE_NAMING_PLACEHOLDER, Some(CONTROL), p), p),
        ],
        p,
    )
}

fn appearance(s: &State, v: &Spec, p: &Pal) -> El {
    let theme_on = ["light", "dark", "system"].iter().position(|t| *t == v.text("theme")).unwrap_or(2);
    let mut theme = row().padding(4.0).gap(4.0).radius(8.0).fill(p.deep).border(1.0, p.line).role("radiogroup").label(w::THEME.0);
    for (i, name) in w::THEMES.into_iter().enumerate() {
        let on = i == theme_on;
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
    for i in 0..=palette::NAMES.len() {
        let name = palette_name(i);
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
                .label(name)
                .help(name)
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
        .child(button(None, w::LAYOUT_CHOOSE, p.text, p).key(keys::LAYOUT_CHOOSE).on_press(|s: &mut State| s.picker = true));
    panel(
        vec![
            setting(w::THEME, theme, p),
            setting(w::PALETTE, swatches.max_w(280.0), p),
            setting(w::LAYOUT, layout, p),
            setting(w::CLIPBOARD_WATCHER, switch(w::CLIPBOARD_WATCHER.0, v.on("clipboardWatch"), p), p),
            setting(w::NOTIFICATIONS, switch(w::NOTIFICATIONS.0, v.on("showNotifications"), p), p),
            setting(w::DISCORD, switch(w::DISCORD.0, v.on("discordRichPresence"), p), p),
        ],
        p,
    )
}

fn schedule(v: &Spec, p: &Pal) -> El {
    let chosen = v.value("scheduleDays");
    let mut days = row().gap(4.0).role("group").label(w::ACTIVE_DAYS.0);
    for (i, day) in w::DAYS.iter().enumerate() {
        let on = chosen.as_array().is_some_and(|d| d.iter().any(|x| x.as_u64() == Some(i as u64)));
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
                .child(label(w::DAYS_SHORT[i], mono(12.0, 500.0), if on { p.text } else { p.muted })),
        );
    }
    panel(
        vec![
            setting((w::ENABLE_SCHEDULER, ""), switch(w::ENABLE_SCHEDULER, v.on("scheduleEnabled"), p), p),
            setting(w::START_TIME, input(w::START_TIME.0, &v.text("scheduleTime"), "", Some(80.0), p), p),
            setting(w::ACTIVE_DAYS, days, p),
            setting(w::SHUTDOWN, switch(w::SHUTDOWN.0, v.on("scheduleShutdown"), p), p),
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

fn software(s: &State, v: &Spec, p: &Pal) -> El {
    panel(
        vec![
            status(
                w::APP_NAME,
                &format!("v{}", env!("CARGO_PKG_VERSION")),
                w::APP_REST,
                p.muted,
                button(Some(&icons::REFRESH), w::CHECK_FOR_UPDATES, p.text, p),
                p,
            ),
            status(
                w::ENGINE_NAME,
                &s.engine.current,
                w::ENGINE_UP_TO_DATE,
                p.success,
                button(Some(&icons::REFRESH), w::ENGINE_CHECK, p.text, p),
                p,
            ),
            setting(w::AUTO_UPDATE_ENGINE, switch(w::AUTO_UPDATE_ENGINE.0, v.on("autoUpdateEngine"), p), p),
        ],
        p,
    )
}

fn advanced(v: &Spec, p: &Pal) -> El {
    panel(
        vec![
            setting(w::RATE_LIMIT, input(w::RATE_LIMIT.0, &v.text("rateLimit"), w::RATE_LIMIT_PLACEHOLDER, Some(CONTROL), p), p),
            setting(w::PROXY, input(w::PROXY.0, &v.text("proxy"), w::PROXY_PLACEHOLDER, Some(CONTROL), p), p),
            setting(
                w::GLOBAL_HOTKEY,
                input(w::GLOBAL_HOTKEY.0, &v.text("globalHotkey"), w::GLOBAL_HOTKEY_PLACEHOLDER, Some(CONTROL), p),
                p,
            ),
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
    panel(vec![yours, danger(keys::arm(Confirm::Deactivate), w::DEACTIVATE), danger(keys::arm(Confirm::Move), w::MOVE)], p)
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
