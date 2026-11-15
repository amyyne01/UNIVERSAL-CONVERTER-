//! Settings, the third place: everything that is not a download, in two
//! columns (left, what a download produces; right, the app, its engine and the
//! licence), as Electron assigned them. Wording is the Electron Settings page's,
//! word for word (`SettingsTab.tsx`); `copy.rs` does not hold it yet. Values
//! are plausible stand-ins: the gate has no settings.
use rune::{Color, El, Icon, ScrollSpec, col, el, icon, row, text};

use crate::directions::shelf::icons;
use crate::directions::shelf::theme::{Pal, mono, sans, serif};

static CARET_DOWN: Icon = Icon {
    name: "collection-caret-down",
    view: 256.0,
    body: r#"<path d="M213.66,101.66l-80,80a8,8,0,0,1-11.32,0l-80-80A8,8,0,0,1,53.66,90.34L128,164.69l74.34-74.35a8,8,0,0,1,11.32,11.32Z" fill="currentColor"/>"#,
};
pub(super) static CARET_RIGHT: Icon = Icon {
    name: "collection-caret-right",
    view: 256.0,
    body: r#"<path d="M181.66,133.66l-80,80a8,8,0,0,1-11.32-11.32L164.69,128,90.34,53.66a8,8,0,0,1,11.32-11.32l80,80A8,8,0,0,1,181.66,133.66Z" fill="currentColor"/>"#,
};

pub fn view(p: &Pal) -> El {
    let left = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(16.0)
        .child(destination(p))
        .child(defaults(p))
        .child(more(p))
        .child(appearance(p));
    let right = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(16.0)
        .child(schedule(p))
        .child(software(p))
        .child(plan(p))
        .child(card(p).child(folded(p, "Advanced", "Network and shortcut settings most people never touch", false)))
        .child(help(p));
    let page = col()
        .w_full()
        .px(32.0)
        .pt(24.0)
        .pb(48.0)
        .child(
            row()
                .w_full()
                .gap(16.0)
                .items_end()
                .mb(24.0)
                .child(text("Settings").font(serif(44.0).lh(48.0)).color(p.text))
                .child(text("Changes save automatically").font(sans(13.0, 400.0)).color(p.text2).mb(8.0)),
        )
        .child(row().w_full().gap(16.0).items_start().child(left).child(right))
        .child(
            row()
                .mt(24.0)
                .gap(8.0)
                .help("What a key binds to on this computer")
                .child(text("Machine ID").font(sans(12.0, 400.0)).color(p.muted))
                .child(text("7f3a-91c2-e0d4-58b6").font(mono(12.0, 400.0)).color(p.muted)),
        );
    col()
        .key("settings-place")
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(page)
}

/// A section's paper: a card on the page.
fn card(p: &Pal) -> El {
    col().w_full().px(20.0).py(16.0).gap(8.0).radius(12.0).fill(p.card).border(1.0, p.line_soft)
}

fn section(p: &Pal, title: &str, hint: Option<&str>) -> El {
    let mut c = col().w_full().gap(4.0).mb(8.0).child(text(title.to_string()).font(sans(16.0, 500.0)).color(p.text));
    if let Some(h) = hint {
        c = c.child(text(h.to_string()).font(sans(12.0, 400.0)).color(p.text2).wrap());
    }
    card(p).child(c)
}

/// A setting: its name and description, the control on the right.
fn setting(p: &Pal, label: &str, desc: &str, control: El) -> El {
    let mut words = col().grow(1.0).min_w(0.0).gap(4.0).child(text(label.to_string()).font(sans(13.0, 500.0)).color(p.text));
    if !desc.is_empty() {
        words = words.child(text(desc.to_string()).font(sans(12.0, 400.0)).color(p.text2).wrap());
    }
    row().w_full().gap(16.0).py(4.0).center_y().child(words).child(control)
}

fn toggle(p: &Pal, label: &str, on: bool) -> El {
    el().size(32.0, 20.0)
        .radius(10.0)
        .fill(if on { p.accent } else { p.control.alpha(0.6) })
        .focusable(true)
        .role("switch")
        .label(label.to_string())
        .checked(on)
        .child(el().size(16.0, 16.0).radius(8.0).fill(if on { p.bg } else { p.card }).absolute(if on { 14.0 } else { 2.0 }, 2.0))
}

/// A bordered field showing its value (or its placeholder, muted): a select with a caret, or a typed value.
fn field(p: &Pal, label: &str, value: &str, select: bool, placeholder: bool) -> El {
    let mut f = row()
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .border(1.0, p.line)
        .hover(p.raised)
        .focusable(true)
        .role(if select { "combobox" } else { "textbox" })
        .label(label.to_string())
        .child(
            text(value.to_string())
                .font(if select { sans(13.0, 400.0) } else { mono(13.0, 400.0) })
                .color(if placeholder { p.muted } else { p.text })
                .single_line(),
        );
    if select {
        f = f.child(el().grow(1.0)).child(icon(&CARET_DOWN, 16.0).color(p.muted));
    }
    f
}

fn bordered(p: &Pal, glyph: &'static Icon, label: &str) -> El {
    row()
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .border(1.0, p.line)
        .hover(p.raised)
        .focusable(true)
        .role("button")
        .label(label.to_string())
        .child(icon(glyph, 16.0).color(p.text2))
        .child(text(label.to_string()).font(sans(13.0, 500.0)).color(p.text).single_line())
}

fn destination(p: &Pal) -> El {
    card(p).child(
        row()
            .w_full()
            .gap(16.0)
            .center_y()
            .child(
                col()
                    .grow(1.0)
                    .gap(4.0)
                    .child(text("Saving to").font(sans(12.0, 500.0)).color(p.text2))
                    .child(
                        row()
                            .child(text("C:\\Users\\Alex\\").font(mono(13.0, 400.0)).color(p.text2))
                            .child(text("Music").font(mono(13.0, 500.0)).color(p.text)),
                    ),
            )
            .child(bordered(p, &icons::FOLDER, "Change")),
    )
}

fn defaults(p: &Pal) -> El {
    let labelled = |label: &str, value: &str| {
        col()
            .grow(1.0)
            .min_w(0.0)
            .gap(8.0)
            .child(text(label.to_string()).font(sans(12.0, 500.0)).color(p.text2))
            .child(field(p, label, value, true, false))
    };
    section(p, "Download defaults", Some("What a new download starts with. Each platform tab can still override them."))
        .child(
            row()
                .w_full()
                .gap(12.0)
                .mb(8.0)
                .child(labelled("Audio format", "MP3"))
                .child(labelled("Audio quality", "320 kbps"))
                .child(labelled("Video quality", "1080p Full HD")),
        )
        .child(setting(p, "Skip existing", "Don't re-download files already present on disk", toggle(p, "Skip existing", true)))
        .child(setting(p, "Embed thumbnail", "Write cover art into the audio file", toggle(p, "Embed thumbnail", true)))
        .child(setting(p, "Embed metadata", "Write title, artist, and album tags", toggle(p, "Embed metadata", true)))
        .child(setting(p, "Simultaneous downloads", "", field(p, "Simultaneous downloads", "3", true, false)))
}

/// "More download options", open at the cookies: where a failed sleeve's "Open settings" lands.
fn more(p: &Pal) -> El {
    section(p, "More download options", Some("Subtitles, chapters, sponsor segments, container and file naming"))
        .child(setting(p, "Sign in with", "", field(p, "Sign in with", "Chrome", true, false)))
        .child(setting(
            p,
            "Import cookies.txt",
            "For sites your browser is signed in to but cannot be read directly",
            bordered(p, &icons::FOLDER, "Choose file"),
        ))
        .child(setting(p, "Subtitles", "", field(p, "Subtitles", "Off", true, false)))
        .child(setting(p, "Sponsor segments", "", field(p, "Sponsor segments", "Off", true, false)))
        .child(setting(p, "Video container", "", field(p, "Video container", "MP4 (most compatible)", true, false)))
        .child(setting(p, "File naming", "yt-dlp template, e.g. %(uploader)s/%(title)s. Leave empty for the title.", field(p, "File naming", "%(title)s", false, true)))
}

fn appearance(p: &Pal) -> El {
    let mut theme = row().h(32.0).padding(2.0).gap(2.0).radius(8.0).border(1.0, p.line).role("radiogroup").label("Theme");
    for (i, name) in ["Light", "Dark", "System"].into_iter().enumerate() {
        let on = i == 2;
        theme = theme.child(
            el().h(26.0)
                .px(12.0)
                .center()
                .radius(6.0)
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .role("radio")
                .checked(on)
                .child(text(name).font(sans(13.0, if on { 500.0 } else { 400.0 })).color(if on { p.text } else { p.text2 })),
        );
    }
    let mut swatches = row().gap(8.0).role("radiogroup").label("Color palette");
    for i in 0..8 {
        let sw = Pal::new(i, p.dark);
        swatches = swatches.child(
            el().size(24.0, 24.0)
                .radius(12.0)
                .center()
                .fill(sw.bg)
                .border(2.0, if i == 0 { p.text } else { sw.accent })
                .role("radio")
                .checked(i == 0)
                .child(el().size(8.0, 8.0).radius(4.0).fill(sw.accent)),
        );
    }
    section(p, "Appearance & behavior", None)
        .child(setting(p, "Theme", "Light, dark, or follow the system preference", theme))
        .child(setting(p, "Palette", "Accent and surface colors", swatches))
        .child(setting(p, "Clipboard watcher", "Offer a one-click download whenever you copy a link, even outside the app", toggle(p, "Clipboard watcher", true)))
        .child(setting(p, "Desktop notifications", "Notify when a download completes or fails", toggle(p, "Desktop notifications", true)))
        .child(setting(p, "Discord Rich Presence", "Show what you're downloading in your Discord status", toggle(p, "Discord Rich Presence", false)))
}

fn schedule(p: &Pal) -> El {
    // Electron's order: Sunday first.
    const DAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
    let mut days = row().gap(4.0).role("group").label("Active days");
    for (i, d) in DAYS.iter().enumerate() {
        let on = (1..=5).contains(&i);
        days = days.child(
            el().size(32.0, 28.0)
                .center()
                .radius(8.0)
                .fill(if on { p.raised } else { Color::TRANSPARENT })
                .border(1.0, if on { p.control } else { p.line })
                .focusable(true)
                .role("checkbox")
                .checked(on)
                .child(text(*d).font(mono(12.0, 500.0)).color(if on { p.text } else { p.muted })),
        );
    }
    // As in Electron: the section's own heading carries the master switch.
    let head = row()
        .w_full()
        .gap(16.0)
        .mb(8.0)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .gap(4.0)
                .child(text("Scheduled downloads").font(sans(16.0, 500.0)).color(p.text))
                .child(text("Start downloads automatically at a set time").font(sans(12.0, 400.0)).color(p.text2)),
        )
        .child(toggle(p, "Enable scheduler", true));
    card(p)
        .child(head)
        .child(setting(p, "Start time", "Local time the schedule starts", field(p, "Start time", "02:00", false, false)))
        .child(setting(p, "Active days", "Days of the week the schedule is active", days))
        .child(setting(p, "Shutdown after queue", "Power off the PC when scheduled downloads finish", toggle(p, "Shutdown after queue", false)))
}

/// An updater: name and version, its state in a sentence, its one button.
fn status_object(p: &Pal, name: &str, version: &str, desc: &str, button: El) -> El {
    row()
        .w_full()
        .gap(16.0)
        .py(4.0)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(text(name.to_string()).font(sans(13.0, 500.0)).color(p.text))
                        .child(text(version.to_string()).font(mono(12.0, 400.0)).color(p.muted)),
                )
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(el().size(8.0, 8.0).radius(4.0).fill(p.success))
                        .child(text(desc.to_string()).font(sans(12.0, 400.0)).color(p.text2)),
                ),
        )
        .child(button)
}

fn software(p: &Pal) -> El {
    section(p, "Software", Some("Two moving parts: the app itself, and the yt-dlp engine that does the downloading."))
        .child(status_object(p, "AHG Universal Converter", "v2.0.0", "You're on the latest version", bordered(p, &icons::REFRESH, "Check for updates")))
        .child(status_object(p, "Download engine", "2026.11.01", "yt-dlp is up to date", bordered(p, &icons::REFRESH, "Check for engine update")))
        .child(setting(p, "Auto-update engine", "Keep yt-dlp current, and repair it when a site change breaks downloads", toggle(p, "Auto-update engine", true)))
}

fn plan(p: &Pal) -> El {
    let danger = |label: &str, desc: &str| {
        row()
            .w_full()
            .py(8.0)
            .gap(16.0)
            .center_y()
            .border_sides([1.0, 0.0, 0.0, 0.0], p.line_soft)
            .focusable(true)
            .role("button")
            .label(label.to_string())
            .child(
                col()
                    .grow(1.0)
                    .gap(4.0)
                    .child(text(label.to_string()).font(sans(13.0, 500.0)).color(p.error))
                    .child(text(desc.to_string()).font(sans(12.0, 400.0)).color(p.text2).wrap()),
            )
            .child(icon(&CARET_RIGHT, 16.0).color(p.muted))
    };
    section(p, "Plan & license", None)
        .child(status_object(p, "Your plan", "Premium", "Every feature is included on this machine", el()))
        .child(danger("Deactivate license", "Release this machine so you can activate on another device"))
        .child(danger("Move to another PC", "Deactivate here and release the key so it can be activated on a different computer"))
}

fn help(p: &Pal) -> El {
    section(p, "Help", None)
        .child(setting(p, "Keyboard shortcuts", "Ctrl+K opens the command palette; the full list is one click away", bordered(p, &icons::EXTERNAL, "Show shortcuts")))
        .child(setting(
            p,
            "Report a problem",
            "Say what you pasted and the message you saw. If asked, attach ahg.log, found in the app's folder under %APPDATA%.",
            bordered(p, &icons::EXTERNAL, "Get support"),
        ))
}

/// A section kept closed: its title and Electron's hint, one press to open.
fn folded(p: &Pal, title: &str, hint: &str, rule: bool) -> El {
    row()
        .w_full()
        .gap(12.0)
        .center_y()
        .border_sides([if rule { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0], p.line_soft)
        .focusable(true)
        .role("button")
        .label(title.to_string())
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(text(title.to_string()).font(sans(16.0, 500.0)).color(p.text))
                .child(text(hint.to_string()).font(sans(12.0, 400.0)).color(p.text2).wrap()),
        )
        .child(icon(&CARET_RIGHT, 16.0).color(p.muted))
}
