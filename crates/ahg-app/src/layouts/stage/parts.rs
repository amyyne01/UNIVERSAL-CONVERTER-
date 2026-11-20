//! The player's small parts: its five type sizes, its buttons, keycaps and covers. Written
//! here rather than taken from Orbit's shell so every gap is on the 4 pt grid and every icon
//! is one size.
use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode};
use rune::{Color, Cursor, El, Icon, TextStyle, el, icon, row};

use crate::copy;
use crate::directions::orbit::look::{Pal, mix, txt};
use crate::directions::orbit::model;
use crate::format;
use crate::gate::State;
use crate::icons;
use crate::keys::RowAct;

/// The one icon size.
pub const ICON: f32 = 16.0;

// Five sizes, two weights. Space Grotesk for words, Space Mono for figures.
pub const HERO: TextStyle = TextStyle::new(28.0, 500.0).lh(32.0).tracking(-0.4);
pub const BODY: TextStyle = TextStyle::new(14.0, 400.0).lh(20.0);
pub const BODY_M: TextStyle = TextStyle::new(14.0, 500.0).lh(20.0);
pub const SMALL: TextStyle = TextStyle::new(12.0, 400.0).lh(16.0);
pub const SMALL_M: TextStyle = TextStyle::new(12.0, 500.0).lh(16.0);
pub const FIG: TextStyle = TextStyle::new(12.0, 400.0).lh(16.0).mono().features("'tnum' 1");
pub const FIG_L: TextStyle = TextStyle::new(20.0, 400.0).lh(24.0).mono().features("'tnum' 1");
pub const FIG_XL: TextStyle = TextStyle::new(44.0, 400.0).lh(48.0).mono().tracking(-1.0).features("'tnum' 1");

pub fn glyph(i: &'static Icon, c: Color) -> El {
    icon(i, ICON).color(c)
}

pub fn keycap(p: &Pal, label: &str) -> El {
    row().h(20.0).px(4.0).center().radius(4.0).fill(p.raised).border(1.0, p.line).child(txt(label.to_string(), FIG, p.text2))
}

pub fn keys(p: &Pal, caps: &[&str]) -> El {
    let mut r = row().gap(4.0).center_y();
    for c in caps {
        r = r.child(keycap(p, c));
    }
    r
}

/// A hairline-outlined button with a word; `lit` gives it the accent (one per window).
pub fn button(p: &Pal, key: String, label: &str, i: Option<&'static Icon>, lit: bool) -> El {
    tinted(p, key, label, i, lit.then_some((p.accent, p.on_accent)))
}

/// A button filled with `(fill, ink)`, or hairline-outlined with `None`; the red confirm of
/// an armed action is `(p.error, p.bg)`.
pub fn tinted(p: &Pal, key: String, label: &str, i: Option<&'static Icon>, tint: Option<(Color, Color)>) -> El {
    let lit = tint.is_some();
    let (fill, ink, edge) = match tint {
        Some((fill, ink)) => (fill, ink, fill),
        None => (Color::TRANSPARENT, p.text, p.track),
    };
    let mut b = row()
        .key(key)
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center()
        .radius(8.0)
        .fill(fill)
        .border(1.0, edge)
        .role("button")
        .label(label.to_string())
        .focusable(true)
        .cursor(Cursor::Pointer)
        .press_scale(0.97)
        .hover(if lit { mix(fill, p.text, 0.15) } else { p.raised });
    if let Some(i) = i {
        b = b.child(glyph(i, if lit { ink } else { p.text2 }));
    }
    b.child(txt(label.to_string(), BODY_M, ink))
}

/// A quiet word that acts, for headers.
pub fn text_button(p: &Pal, key: String, label: &str) -> El {
    row()
        .key(key)
        .h(24.0)
        .px(4.0)
        .center_y()
        .radius(4.0)
        .hover(p.hover)
        .cursor(Cursor::Pointer)
        .role("button")
        .focusable(true)
        .label(label.to_string())
        .child(txt(label.to_string(), SMALL_M, p.text2))
}

pub fn icon_button(p: &Pal, key: String, label: &str, i: &'static Icon, ink: Color) -> El {
    el().key(key)
        .size(32.0, 32.0)
        .center()
        .radius(8.0)
        .role("button")
        .label(label.to_string())
        .help(label.to_string())
        .focusable(true)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .press_scale(0.94)
        .child(glyph(i, ink))
}

/// A task's artwork: round while it travels, a square tile once it is yours.
pub fn cover(s: &State, p: &Pal, t: &DownloadTask, size: f32, round: bool) -> El {
    let r = if round {
        size / 2.0
    } else if size >= 160.0 {
        12.0
    } else {
        8.0
    };
    match s.cover(t) {
        Some(img) => el().size(size, size).radius(r).fill(p.raised).child(crate::covers::art(s, img, r)),
        None => {
            let tint = mix(p.raised, model::source(p, t.source).1, 0.12);
            el().size(size, size)
                .radius(r)
                .fill(tint)
                .center()
                .child(glyph(if t.is_audio_only { &icons::MUSIC } else { &icons::VIDEO }, p.muted))
        }
    }
}

/// The buttons a task offers, in Electron's order (Remove left out: it waits for hover).
pub fn actions(t: &DownloadTask) -> Vec<(&'static str, &'static Icon)> {
    let st = t.progress.status;
    let mut a = Vec::new();
    if matches!(st, St::Queued | St::FetchingInfo | St::Downloading) {
        a.push((copy::ACT_PAUSE, &icons::PAUSE));
    }
    if st == St::Paused {
        a.push((copy::ACT_RESUME, &icons::PLAY));
    }
    if format::is_active(t) {
        a.push((copy::ACT_CANCEL, &icons::CANCEL));
    }
    if st == St::Failed && !t.progress.error.is_empty() {
        a.push((copy::ACT_COPY_DETAILS, &icons::COPY));
    }
    if matches!(st, St::Failed | St::Cancelled) {
        if matches!(t.error_code, Some(ErrorCode::Private | ErrorCode::AgeRestricted | ErrorCode::LoginRequired | ErrorCode::DiskFull)) {
            a.push((copy::ACT_OPEN_SETTINGS, &icons::SETTINGS));
        }
        if t.error_code == Some(ErrorCode::Unknown) {
            a.push((copy::ACT_UPDATE_ENGINE, &icons::REFRESH));
        }
        if !format::is_permanent(t.error_code) {
            a.push((copy::ACT_RETRY, &icons::RETRY));
        }
    }
    if st == St::Done {
        a.push((copy::ACT_OPEN_FILE, &icons::EXTERNAL));
        a.push((copy::ACT_SHOW_IN_FOLDER, &icons::FOLDER));
    }
    a
}

/// The word for where a task is, and its colour. Paused and retrying share the warning
/// colour (they are held); converting and tagging stay neutral (the orbit is closed, it is
/// only being finished).
pub fn state_word(p: &Pal, t: &DownloadTask) -> (&'static str, Color) {
    match t.progress.status {
        St::Queued => (copy::STATE_QUEUED, p.muted),
        St::FetchingInfo => (copy::STATE_STARTING, p.text2),
        St::Downloading => ("", p.text2),
        St::Converting => (copy::STATE_CONVERTING, p.text2),
        St::Embedding => (copy::STATE_TAGGING, p.text2),
        St::Paused => (copy::STATE_PAUSED, p.warning),
        St::Retrying => (copy::STATE_RETRYING, p.warning),
        St::Done if t.progress.already_on_disk == Some(true) => (copy::STATE_SKIPPED, p.success),
        St::Done => (copy::STATE_DONE, p.success),
        St::Failed if format::is_permanent(t.error_code) => (copy::STATE_CANT_DOWNLOAD, p.error),
        St::Failed => (copy::STATE_FAILED, p.error),
        St::Cancelled => (copy::STATE_CANCELED, p.muted),
    }
}

/// `Retrying in 5s (1 of 3)` without its leading dot.
pub fn retry_line(t: &DownloadTask) -> String {
    let k = t.retry_count.unwrap_or(0.0) as usize;
    copy::row_retrying(format::retry_wait(t), k).trim_start_matches(" · ").to_string()
}

/// Which of a row's actions a button of `actions` is, for its shared key.
pub fn act(label: &str) -> RowAct {
    match label {
        copy::ACT_PAUSE => RowAct::Pause,
        copy::ACT_RESUME => RowAct::Resume,
        copy::ACT_CANCEL => RowAct::Cancel,
        copy::ACT_COPY_DETAILS => RowAct::CopyDetails,
        copy::ACT_OPEN_SETTINGS | copy::ACT_UPDATE_ENGINE => RowAct::Fix,
        copy::ACT_RETRY => RowAct::Retry,
        copy::ACT_OPEN_FILE => RowAct::OpenFile,
        _ => RowAct::ShowInFolder,
    }
}
