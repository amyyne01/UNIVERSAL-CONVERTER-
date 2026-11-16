//! Settings, the third place, laid out as a spec sheet: where files go leads, as the one
//! reading in large figures; then each section is a band across the page, its name and
//! purpose in a left margin, its settings as ruled lines on the right. No cards: the rules
//! do the grouping, the margin makes it scannable. Every word comes from
//! `copy::settings`; values (folder, versions, times) are stand-ins.
use rune::{Color, El, Icon, ScrollSpec, col, el, icon, row, spacer};

use super::look::{BODY, BODY_M, CELL_TITLE, FIG, FIG_L, PAGE_TITLE, Pal, SMALL, SMALL_M, mix, txt};
use super::{Tone, button, icons, keycap};
use crate::copy::settings as w;
use crate::gate::{self, State};

/// The left margin that carries each section's name.
const MARGIN: f32 = 216.0;

pub fn view(s: &State, p: &Pal) -> El {
    let head = col().w_full().gap(4.0).child(txt(w::TITLE, PAGE_TITLE, p.text)).child(txt(w::SAVES, SMALL, p.text2));
    let page = col().w_full().max_w(1120.0).px(40.0).pt(32.0).pb(48.0).gap(32.0).child(head).child(destination(p)).child(
        col()
            .w_full()
            .child(defaults(p))
            .child(more(p))
            .child(appearance(s, p))
            .child(schedule(p))
            .child(software(p))
            .child(advanced(p))
            .child(plan(p))
            .child(help(p)),
    );
    col().key("settings").scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0).w_full().child(page)
}

// ── Pieces ──────────────────────────────────────────────────────────────────

/// A band: a rule across, the name and hint in the margin, the lines on the right.
fn band(p: &Pal, title: &str, hint: Option<&str>, lines: Vec<El>) -> El {
    let mut name = col().w(MARGIN).gap(4.0).pt(20.0).child(txt(title.to_string(), CELL_TITLE, p.text));
    if let Some(h) = hint {
        name = name.child(txt(h.to_string(), SMALL, p.text2).wrap());
    }
    let mut right = col().grow(1.0).min_w(0.0);
    for (i, l) in lines.into_iter().enumerate() {
        right = right.child(l.border_sides([if i == 0 { 0.0 } else { 1.0 }, 0.0, 0.0, 0.0], p.line));
    }
    row().w_full().gap(32.0).pb(16.0).items_start().border_sides([1.0, 0.0, 0.0, 0.0], p.track).child(name).child(right)
}

/// A setting: its name and description, the control at the right.
fn line(p: &Pal, (label, desc): (&str, &str), control: El) -> El {
    let mut words = col().grow(1.0).min_w(0.0).gap(4.0).child(txt(label.to_string(), BODY_M, p.text));
    if !desc.is_empty() {
        words = words.child(txt(desc.to_string(), SMALL, p.text2).wrap());
    }
    row().w_full().min_h(64.0).py(12.0).gap(24.0).center_y().child(words).child(control)
}

fn toggle(p: &Pal, label: &str, on: bool) -> El {
    el().size(36.0, 20.0)
        .radius(10.0)
        .fill(if on { p.text } else { p.raised })
        .border(1.0, if on { p.text } else { p.track })
        .focusable(true)
        .role("switch")
        .label(label.to_string())
        .checked(on)
        .cursor(rune::Cursor::Pointer)
        .child(el().size(12.0, 12.0).radius(6.0).fill(if on { p.deep } else { p.muted }).absolute(if on { 19.0 } else { 3.0 }, 3.0))
}

/// A toggle row from a (label, description) pair.
fn switch(p: &Pal, pair: (&str, &str), on: bool) -> El {
    line(p, pair, toggle(p, pair.0, on))
}

/// A field showing its value: a select (sans, with a caret) or a typed value (mono).
/// A muted value is a placeholder.
fn field(p: &Pal, label: &str, value: &str, select: bool, muted: bool) -> El {
    let mut f = row()
        .h(32.0)
        .min_w(if select { 176.0 } else { 200.0 })
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.card)
        .border(1.0, p.track)
        .hover(p.raised)
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .role(if select { "combobox" } else { "textbox" })
        .label(label.to_string())
        .child(txt(value.to_string(), if select { SMALL } else { FIG }, if muted { p.muted } else { p.text }).single_line());
    if select {
        f = f.child(spacer()).child(icon(&icons::CARET_DOWN, 16.0).color(p.muted));
    }
    f
}

fn select(p: &Pal, label: &str, value: &str) -> El {
    field(p, label, value, true, false)
}

fn ghost(p: &Pal, label: &str, glyph: Option<&'static Icon>) -> El {
    button(p, &format!("set-{label}"), label, glyph, Tone::Ghost)
}

/// An updater or the plan: name, version in figures, and what it says, with a dot when it is good.
fn object(p: &Pal, name: &str, version: &str, say: &str, good: bool, control: El) -> El {
    let mut state = row().gap(8.0).center_y();
    if good {
        state = state.child(el().size(8.0, 8.0).radius(4.0).fill(p.success));
    }
    let mut words = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .child(row().gap(8.0).center_y().child(txt(name.to_string(), BODY_M, p.text)).child(txt(version.to_string(), FIG, p.text2)));
    if !say.is_empty() {
        words = words.child(state.child(txt(say.to_string(), SMALL, p.text2).wrap()));
    }
    row().w_full().min_h(64.0).py(12.0).gap(24.0).center_y().child(words).child(control)
}

// ── Sections ────────────────────────────────────────────────────────────────

/// The one large reading: where every file lands. The page's one bordered surface.
fn destination(p: &Pal) -> El {
    row()
        .w_full()
        .gap(24.0)
        .px(24.0)
        .py(20.0)
        .radius(12.0)
        .fill(p.card)
        .border(1.0, p.line)
        .center_y()
        .child(el().size(40.0, 40.0).radius(20.0).center().border(1.0, p.track).child(icon(&icons::FOLDER, 16.0).color(p.text2)))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(txt(w::SAVING_TO, SMALL, p.text2))
                .child(row().child(txt("C:\\Users\\Alex\\", FIG_L, p.text2)).child(txt("Music", FIG_L, p.text))),
        )
        .child(ghost(p, w::CHANGE, Some(&icons::FOLDER)))
}

fn defaults(p: &Pal) -> El {
    let labelled = |label: &str, value: &str| {
        col().grow(1.0).min_w(0.0).gap(8.0).child(txt(label.to_string(), SMALL, p.text2)).child(select(p, label, value).min_w(0.0))
    };
    let three = row()
        .w_full()
        .gap(12.0)
        .pt(16.0)
        .pb(16.0)
        .child(labelled(w::AUDIO_FORMAT, "MP3"))
        .child(labelled(w::AUDIO_QUALITY, "320 kbps"))
        .child(labelled(w::VIDEO_QUALITY, "1080p Full HD"));
    band(
        p,
        w::DEFAULTS,
        Some(w::DEFAULTS_HINT),
        vec![
            three,
            switch(p, w::SKIP_EXISTING, true),
            switch(p, w::EMBED_THUMBNAIL, true),
            switch(p, w::EMBED_METADATA, true),
            line(p, w::SIMULTANEOUS, select(p, w::SIMULTANEOUS.0, "3 at a time")),
        ],
    )
}

fn more(p: &Pal) -> El {
    band(
        p,
        w::MORE,
        Some(w::MORE_HINT),
        vec![
            line(p, (w::SUBTITLES, ""), select(p, w::SUBTITLES, "Off")),
            line(p, (w::SUBTITLE_LANGUAGES, ""), field(p, w::SUBTITLE_LANGUAGES, w::SUBTITLE_LANGUAGES_PLACEHOLDER, false, true)),
            switch(p, w::AUTO_CAPTIONS, false),
            line(p, (w::SPONSOR_SEGMENTS, ""), select(p, w::SPONSOR_SEGMENTS, "Off")),
            line(p, (w::VIDEO_CONTAINER, ""), select(p, w::VIDEO_CONTAINER, "MP4 (most compatible)")),
            line(p, (w::VIDEO_CODEC, ""), select(p, w::VIDEO_CODEC, "Best available")),
            line(p, (w::SIGN_IN_WITH, ""), select(p, w::SIGN_IN_WITH, "Chrome")),
            line(p, w::IMPORT_COOKIES, ghost(p, w::CHOOSE_FILE, Some(&icons::FOLDER))),
            switch(p, w::EMBED_CHAPTERS, true),
            switch(p, w::SPLIT_BY_CHAPTER, false),
            switch(p, w::SAVE_COVER_ART, false),
            line(p, w::FILE_NAMING, field(p, w::FILE_NAMING.0, w::FILE_NAMING_PLACEHOLDER, false, true)),
        ],
    )
}

fn appearance(s: &State, p: &Pal) -> El {
    // System is the stand-in choice: it follows the window's own light or dark.
    let mut theme = row().gap(4.0).padding(4.0).radius(8.0).border(1.0, p.track).role("radiogroup").label(w::THEME.0);
    for (i, name) in w::THEMES.into_iter().enumerate() {
        let on = i == 2;
        let mut seg = el().h(24.0).px(12.0).center().radius(4.0).role("radio").checked(on).focusable(true).cursor(rune::Cursor::Pointer);
        seg = if on { seg.fill(p.raised) } else { seg.hover(p.hover) };
        theme = theme.child(seg.child(txt(name, if on { SMALL_M } else { SMALL }, if on { p.text } else { p.text2 })));
    }
    // Orbit's own colours first, then the app's seven palettes in Settings' order.
    const NAMES: [&str; 8] = ["Orbit", "Aurora", "Slate", "Moss", "Ember", "Paper", "Lagoon", "Orchid"];
    let mut swatches = row().gap(4.0).role("radiogroup").label(w::PALETTE_ARIA);
    for (i, name) in NAMES.iter().enumerate() {
        let sw = Pal::new(i, p.dark);
        let on = i == s.palette;
        swatches = swatches.child(
            el().size(28.0, 28.0)
                .radius(14.0)
                .center()
                .border(1.0, if on { p.text } else { Color::TRANSPARENT })
                .role("radio")
                .checked(on)
                .label(name.to_string())
                .help(name.to_string())
                .focusable(true)
                .cursor(rune::Cursor::Pointer)
                .child(
                    el().size(20.0, 20.0)
                        .radius(10.0)
                        .center()
                        .fill(sw.bg)
                        .border(1.0, mix(sw.bg, sw.text, 0.2))
                        .child(el().size(8.0, 8.0).radius(4.0).fill(sw.accent)),
                ),
        );
    }
    // The preset on show, then the picker's shortcut and the button that opens it.
    let layout = row()
        .gap(12.0)
        .center_y()
        .child(txt(gate::PRESETS[gate::preset(s)].0, SMALL_M, p.text))
        .child(row().center_y().gap(4.0).child(keycap(p, crate::copy::KEY_CTRL)).child(keycap(p, ".")))
        .child(ghost(p, w::LAYOUT_CHOOSE, None).key("set-layout").on_press(|s: &mut State| s.picker = true));
    band(
        p,
        w::APPEARANCE,
        None,
        vec![
            line(p, w::THEME, theme),
            line(p, w::PALETTE, swatches),
            line(p, w::LAYOUT, layout),
            switch(p, w::CLIPBOARD_WATCHER, true),
            switch(p, w::NOTIFICATIONS, true),
            switch(p, w::DISCORD, false),
        ],
    )
}

fn schedule(p: &Pal) -> El {
    // Electron's order: Sunday first. The stand-in runs on weekdays.
    let mut days = row().gap(4.0).role("group").label(w::ACTIVE_DAYS.0);
    for (i, full) in w::DAYS.iter().enumerate() {
        let on = (1..=5).contains(&i);
        days = days.child(
            el().size(28.0, 28.0)
                .radius(14.0)
                .center()
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .border(1.0, if on { p.track } else { p.line })
                .focusable(true)
                .role("checkbox")
                .checked(on)
                .label(full.to_string())
                .help(full.to_string())
                .cursor(rune::Cursor::Pointer)
                .child(txt(&full[..1], FIG, if on { p.text } else { p.muted })),
        );
    }
    band(
        p,
        w::SCHEDULE,
        Some(w::SCHEDULE_HINT),
        vec![
            line(p, (w::ENABLE_SCHEDULER, ""), toggle(p, w::ENABLE_SCHEDULER, true)),
            line(p, w::START_TIME, field(p, w::START_TIME.0, "03:00", false, false).min_w(96.0)),
            line(p, w::ACTIVE_DAYS, days),
            switch(p, w::SHUTDOWN, false),
        ],
    )
}

fn software(p: &Pal) -> El {
    band(
        p,
        w::SOFTWARE,
        Some(w::SOFTWARE_HINT),
        vec![
            object(p, w::APP_NAME, "v2.0.0", w::APP_REST, false, ghost(p, w::CHECK_FOR_UPDATES, Some(&icons::REFRESH))),
            object(p, w::ENGINE_NAME, "2026.11.01", "", false, ghost(p, w::ENGINE_CHECK, Some(&icons::REFRESH))),
            switch(p, w::AUTO_UPDATE_ENGINE, true),
        ],
    )
}

fn advanced(p: &Pal) -> El {
    band(
        p,
        w::ADVANCED,
        Some(w::ADVANCED_HINT),
        vec![
            line(p, w::RATE_LIMIT, field(p, w::RATE_LIMIT.0, w::RATE_LIMIT_PLACEHOLDER, false, true)),
            line(p, w::PROXY, field(p, w::PROXY.0, w::PROXY_PLACEHOLDER, false, true)),
            line(p, w::GLOBAL_HOTKEY, field(p, w::GLOBAL_HOTKEY.0, "Ctrl+Shift+D", false, false)),
        ],
    )
}

fn plan(p: &Pal) -> El {
    let danger = |(label, desc): (&str, &str)| {
        row()
            .w_full()
            .min_h(64.0)
            .py(12.0)
            .gap(24.0)
            .center_y()
            .focusable(true)
            .role("button")
            .label(label.to_string())
            .cursor(rune::Cursor::Pointer)
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(4.0)
                    .child(txt(label.to_string(), BODY, p.error))
                    .child(txt(desc.to_string(), SMALL, p.text2).wrap()),
            )
            .child(icon(&icons::CARET_RIGHT, 16.0).color(p.muted))
    };
    band(
        p,
        w::PLAN,
        None,
        vec![object(p, w::YOUR_PLAN, w::PLAN_PREMIUM.0, w::PLAN_PREMIUM.1, true, el()), danger(w::DEACTIVATE), danger(w::MOVE)],
    )
}

fn help(p: &Pal) -> El {
    band(
        p,
        w::HELP,
        None,
        vec![
            line(p, w::SHORTCUTS, ghost(p, w::SHOW_SHORTCUTS, None)),
            line(p, w::REPORT, ghost(p, w::GET_SUPPORT, Some(&icons::EXTERNAL))),
        ],
    )
}
