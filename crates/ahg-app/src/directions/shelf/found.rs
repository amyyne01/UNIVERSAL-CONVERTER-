//! What the field found, in the Shelf look: the liner notes of a record about to join the
//! collection, between a paste (or a search) and a running download. Shelf's Home sets it under
//! its field, The collection under its add field; both draw this same panel.
//!
//! A panel of paper: a header line naming what came in and the way to put it away, the notes
//! themselves, and a foot with the one ink control, Download. The sleeve's spine label prints
//! the format the file will have, so changing the choice re-prints it. A look-up is a blank
//! sleeve with its record beside it, the waiting ring on the rim.
//!
//! The controls carry their keys from `crate::keys` and call `crate::actions` (`super::handlers` routes the choice).
use ahg_engine::tier::{self, BASIC_LIMITS, Plan, is_premium_format};
use ahg_engine::types::{AUDIO_ONLY_SOURCES, LOSSLESS_FORMATS, MediaMetadata, Playlist, SearchResult, SourcePlatform as Src, VideoQuality};
use ahg_engine::url_detector::detect_url;
use rune::{Color, Cursor, El, Icon, ScrollSpec, col, el, icon, row, spacer, text};

use super::handlers;
use super::parts::{self, Run, ghost, icon_button, keycap, primary, run, run_mono, runs};
use super::theme::{self, Pal, mono, sans};
use crate::copy::{self, found as f};
use crate::gate::{Band, Card, Choice, Found, Matched, State};
use crate::{actions, format, icons, keys, ring};

/// The video qualities, in the order of `f::VIDEO_QUALITIES`.
const QUALITIES: [VideoQuality; 6] =
    [VideoQuality::P360, VideoQuality::P480, VideoQuality::P720, VideoQuality::P1080, VideoQuality::P2160, VideoQuality::Best];
/// Rows a search band shows before "Show n more".
const SHOWN: usize = 5;
/// The search bands, in the order of `Found::Search`.
const SOURCES: [(Src, &str); 3] =
    [(Src::Youtube, copy::SEARCH_BAND_YOUTUBE), (Src::Soundcloud, copy::SEARCH_BAND_SOUNDCLOUD), (Src::Spotify, copy::SEARCH_BAND_SPOTIFY)];
/// The format a video arrives in, as the spine label prints it.
const VIDEO_FORMAT: &str = "mp4";

/// The panel, or `None` while the field has found nothing.
pub(crate) fn view(s: &State, p: &Pal) -> Option<El> {
    let link = || head(p, glyph(&icons::LINK, p.muted), link_words(p, &s.input));
    let (head, body, foot) = match &s.found {
        Found::Nothing => return None,
        Found::LookingUp => (
            link(),
            looking_up(s, p),
            Some(foot(p, row().child(bone(p, 24.0, 200.0, 12.0)), bone(p, body_w() - 24.0 - 128.0, 128.0, 40.0))),
        ),
        Found::LookupFailed(why) => (head(p, glyph(&icons::ALERT, p.error), link_words(p, &s.input)), failed(p, why, &s.input), None),
        Found::Media(m) => {
            let (body, foot) = match &s.card {
                Card::Short { label } => short(s, p, m, label),
                Card::Single { matched } => single(s, p, m, matched.as_deref()),
            };
            (link(), body, Some(foot))
        }
        Found::Playlist { playlist, matched, selected } => {
            let (body, foot) = album(s, p, playlist, matched, selected);
            (link(), body, Some(foot))
        }
        Found::Search { query, bands } => search(s, p, query, bands),
    };
    let mut c = col()
        .key(keys::FOUND)
        .w_full()
        .shrink()
        .min_h(0.0)
        .radius(12.0)
        .fill(p.card)
        .border(1.0, p.line)
        .clip_rounded(12.0)
        .role("region")
        .child(head);
    c = c.child(body.shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }));
    if let Some(foot) = foot {
        c = c.child(foot);
    }
    Some(c)
}

// ── Pieces ──────────────────────────────────────────────────────────────────

fn glyph(i: &'static Icon, c: Color) -> El {
    el().size(16.0, 16.0).child(icon(i, 16.0).color(c))
}

/// "Available with Premium": a lock in ink, never the accent.
fn lock(ink: Color) -> El {
    glyph(&icons::LOCK_SIGNAL, ink)
}

fn link_words(p: &Pal, input: &str) -> El {
    text(format::link_label(input)).font(mono(12.0, 400.0)).color(p.text2)
}

/// The header line: what came in, then the way to put it away.
fn head(p: &Pal, lead: El, words: El) -> El {
    row()
        .w_full()
        .h(48.0)
        .pl(20.0)
        .pr(12.0)
        .gap(12.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line_soft)
        .child(lead)
        .child(words.single_line().truncate().min_w(0.0).shrink())
        .child(spacer())
        .child(keycap(p, copy::KEYCAP_ESC))
        .child(icon_button(p, keys::FOUND_CLOSE.into(), &icons::CLOSE, copy::NOTICE_DISMISS, p.text2).on_press(actions::dismiss_found))
}

/// The foot: the outcome on the left, the one ink button on the right.
fn foot(p: &Pal, left: El, right: El) -> El {
    row()
        .w_full()
        .min_h(64.0)
        .px(24.0)
        .py(12.0)
        .gap(16.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line_soft)
        .child(left.grow(1.0).min_w(0.0))
        .child(right)
}

fn download(p: &Pal, words: &str) -> El {
    primary(p, keys::FOUND_DOWNLOAD, Some(&icons::DOWNLOAD), words).h(40.0).px(16.0).on_press(actions::download_found)
}

/// A skeleton block `at` points from the body's left edge: the shape of something still
/// coming, a paper sheen crossing it.
fn bone(p: &Pal, at: f32, w: f32, h: f32) -> El {
    el().w(w).h(h).radius(4.0).fill(p.raised).clip_rounded(4.0).child(sheen(p, at))
}

/// Shelf's shimmer: a wide, soft wash of warm paper light.
fn sheen(p: &Pal, at: f32) -> El {
    let lit = if p.dark { p.text.alpha(0.09) } else { p.card.alpha(0.75) };
    crate::timeline::sheen(at, lit, 0.5)
}

/// The body's width last frame (the card less its border), for a bone set against its right edge.
fn body_w() -> f32 {
    crate::timeline::found_w() - 2.0
}

/// A tick box. `can` false: the row cannot be picked (no match found).
fn tick(p: &Pal, on: bool, can: bool) -> El {
    let b = el().size(20.0, 20.0).radius(4.0).center().role("checkbox").checked(on);
    if !can {
        b.border(1.0, p.line).border_dash(2.0, 2.0)
    } else if on {
        b.fill(p.text).child(icon(&icons::CHECK_SHELF, 16.0).color(p.card))
    } else {
        b.border(1.0, p.control)
    }
}

/// A sleeve for something not yet downloaded: `url`'s picture on its face, a blank in the
/// source's tint until it is here, its spine printed with the format it will be.
fn blank_sleeve(s: &State, url: &str, p: &Pal, source: Src, size: f32, format: &str) -> El {
    parts::jacket(p, crate::covers::over(s, parts::blank_face(p, source, size), url, 0.0), size, format)
}

/// A figure on a sleeve's corner, as on the shelf at Home.
fn chip_on_art(words: String) -> El {
    el().h(20.0)
        .px(4.0)
        .center()
        .radius(4.0)
        .fill(theme::vinyl().alpha(0.78))
        .child(text(words).font(mono(11.0, 500.0)).color(theme::label_paper()))
}

// ── The choice: video or audio, format, quality ────────────────────────────

/// What the controls show lit, as places in `f::AUDIO_FORMATS`, `f::AUDIO_QUALITIES` and
/// `f::VIDEO_QUALITIES`: on Basic, what the plan will really make.
fn lit(s: &State) -> (usize, usize, usize) {
    let c = &s.choice;
    let saved = c.format.to_lowercase();
    let format = tier::clamp_format(&saved, s.plan);
    let audio = tier::clamp_audio_quality(&c.audio_quality, s.plan);
    (
        f::AUDIO_FORMATS.iter().position(|n| n.eq_ignore_ascii_case(format)).unwrap_or(0),
        f::AUDIO_QUALITIES.iter().position(|n| n.split(' ').next().is_some_and(|w| w.eq_ignore_ascii_case(audio))).unwrap_or(3),
        QUALITIES.iter().position(|q| *q == tier::clamp_video_quality(c.video_quality, s.plan)).unwrap_or(5),
    )
}

/// What the file will be: `MP3 · 320 kbps`, `MP4 · 1080p Full HD`.
fn outcome(s: &State, video: bool) -> String {
    let (fi, ai, vi) = lit(s);
    if video { f::readout_video(f::VIDEO_QUALITIES[vi]) } else { f::readout_audio(f::AUDIO_FORMATS[fi], f::AUDIO_QUALITIES[ai]) }
}

/// The format the spine label prints.
fn spine(s: &State, video: bool) -> String {
    if video { VIDEO_FORMAT.to_string() } else { f::AUDIO_FORMATS[lit(s).0].to_string() }
}

/// "You get" and the outcome in figures.
fn readout(p: &Pal, outcome: String) -> El {
    runs(vec![run(f::READOUT_LEAD, p.text2), run_mono(outcome, p.text)]).font(sans(13.0, 400.0)).single_line().truncate()
}

/// An audio format. A locked one (Basic) stays pressable, so choosing it can say what Premium
/// adds: a lock before its name and a dashed edge, in ink, never the accent.
fn chip(p: &Pal, value: &str, name: &str, on: bool, locked: bool) -> El {
    let pick = value.to_owned();
    let mut c = row()
        .key(keys::choice_format(value))
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(16.0)
        .border(1.0, if on { p.text } else { p.line })
        .focusable(true)
        .role("radio")
        .checked(on)
        .cursor(Cursor::Pointer)
        .on_press(handlers::choose(move |c: &mut Choice| c.format = pick.clone()));
    c = if on { c.fill(p.bg) } else { c.hover(p.raised) };
    if locked {
        c = c.pl(8.0).border_dash(3.0, 3.0).label(f::lock_chip_aria(name)).help(f::lock_chip_title(name)).child(lock(p.text2));
    } else {
        c = c.label(name);
    }
    c = c.child(text(name).font(sans(13.0, if on { 500.0 } else { 400.0 })).color(if on { p.text } else { p.text2 }).single_line());
    if LOSSLESS_FORMATS.contains(&value) {
        c = c.child(text(f::TAG_LOSSLESS).font(mono(11.0, 400.0)).color(p.muted));
    }
    c
}

/// A drop-down showing its value, its name over it.
fn select(p: &Pal, key: &str, name: &str, aria: &str, value: El, w: f32) -> El {
    col().gap(4.0).child(text(name).font(sans(12.0, 400.0)).color(p.muted)).child(
        row()
            .key(key)
            .w(w)
            .h(32.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(8.0)
            .fill(p.bg)
            .border(1.0, p.line)
            .hover(p.raised)
            .cursor(Cursor::Pointer)
            .focusable(true)
            .role("combobox")
            .label(aria)
            .child(value.grow(1.0).min_w(0.0))
            .child(icon(&icons::CARET_DOWN, 16.0).color(p.muted)),
    )
}

fn value(p: &Pal, words: &str) -> El {
    text(words).font(mono(13.0, 400.0)).color(p.text).single_line().truncate()
}

/// Video or Audio, as two halves of one control.
fn capsule(p: &Pal, video: bool) -> El {
    let side = |key: &'static str, glyph_: &'static Icon, words: &'static str, on: bool, to_video: bool| {
        let c = row()
            .key(key)
            .h(24.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(6.0)
            .cursor(Cursor::Pointer)
            .focusable(true)
            .role("radio")
            .checked(on)
            .label(words)
            .on_press(handlers::choose(move |c: &mut Choice| c.video = to_video))
            .child(icon(glyph_, 16.0).color(if on { p.text } else { p.muted }))
            .child(text(words).font(sans(13.0, if on { 500.0 } else { 400.0 })).color(if on { p.text } else { p.text2 }));
        if on { c.fill(p.card).border(1.0, p.line) } else { c.hover(p.bg) }
    };
    row()
        .padding(4.0)
        .gap(4.0)
        .radius(8.0)
        .fill(p.raised)
        .role("radiogroup")
        .child(side(keys::CHOICE_VIDEO, &icons::VIDEO, f::MEDIA_VIDEO, video, true))
        .child(side(keys::CHOICE_AUDIO, &icons::MUSIC, f::MEDIA_AUDIO, !video, false))
}

/// What Basic includes here, with the way to see the plans: the positive side of a lock.
fn basic_line(p: &Pal, words: &str) -> El {
    row()
        .gap(8.0)
        .center_y()
        .flex_wrap()
        .child(lock(p.muted))
        .child(text(words).font(sans(13.0, 400.0)).color(p.text2))
        // TODO(shared): `actions::see_plans` (the plans sheet); no action opens it yet.
        .child(ghost(p, keys::FOUND_SEE_PLANS, None, f::SEE_PLANS, p.text).ml(4.0))
}

/// The choice for one video or track: the kind, then that kind's quality and formats.
fn options(s: &State, p: &Pal, source: Src) -> El {
    let basic = s.plan == Plan::Basic;
    let audio_only = AUDIO_ONLY_SOURCES.contains(&source);
    let video = s.choice.video && !audio_only;
    let (fi, ai, vi) = lit(s);
    let mut top = row().w_full().gap(16.0).items_end().flex_wrap();
    if !audio_only {
        top = top.child(capsule(p, video));
    }
    let quality = if video {
        select(p, keys::CHOICE_VIDEO_QUALITY, f::VIDEO_QUALITY_LABEL, f::VIDEO_QUALITY_LABEL, value(p, f::VIDEO_QUALITIES[vi]), 176.0)
    } else {
        select(p, keys::CHOICE_AUDIO_QUALITY, f::AUDIO_QUALITY_LABEL, f::AUDIO_QUALITY_LABEL, value(p, f::AUDIO_QUALITIES[ai]), 144.0)
    };
    // On Basic the drop-down holds locked options: its title says what they mean.
    top = top.child(if basic { quality.help(f::LOCK_SELECT_TITLE) } else { quality });
    let mut c = col().w_full().gap(16.0).child(top);
    if !video {
        let mut chips = row().w_full().gap(8.0).flex_wrap().role("radiogroup").label(f::PLAYLIST_FORMAT);
        for (i, name) in f::AUDIO_FORMATS.iter().enumerate() {
            let v = name.to_lowercase();
            chips = chips.child(chip(p, &v, name, i == fi, basic && is_premium_format(&v)));
        }
        c = c.child(col().w_full().gap(8.0).child(text(f::PLAYLIST_FORMAT).font(sans(12.0, 400.0)).color(p.muted)).child(chips));
    }
    if basic {
        c = c.child(basic_line(p, if video { f::BASIC_VIDEO } else { f::BASIC_AUDIO }));
    }
    c
}

// ── A link is being read ────────────────────────────────────────────────────

/// The final card's own shape, still blank: the same sleeve (its face a bone, the waiting ring
/// where the picture will land, the spine already printed), the notes' lines at the same place
/// and the controls that kind of link will get (jury L-1: no record beside the sleeve, no chips
/// a video never shows).
fn looking_up(s: &State, p: &Pal) -> El {
    let source = detect_url(&s.input).platform;
    let audio_only = AUDIO_ONLY_SOURCES.contains(&source);
    let video = s.choice.video && !audio_only;
    // As `single` lays it out: the padding, the sleeve, the gap, then the notes.
    let (pad, size, gap) = (24.0, 176.0, 32.0);
    let at = pad + size + gap;
    // TODO(motion): `reduced` comes from the motion context at step 52.
    let waiting = ring::waiting(keys::RING_WAITING, 40.0, &ring::shelf(s.palette, p.dark, p.source(source)), false, 0);
    let face = el().size(size, size).fill(p.raised).clip_rounded(0.0).center().child(sheen(p, pad)).child(waiting);
    let art = el().key(keys::FOUND_COVER).size(size, size).child(parts::jacket(p, face, size, &spine(s, video)));
    let words = col()
        .w_full()
        .gap(4.0)
        .child(text(f::NOTE_READING).font(mono(12.0, 400.0)).color(p.text2))
        .child(bone(p, at, 360.0, 20.0).my(4.0))
        .child(bone(p, at, 200.0, 14.0).mt(2.0))
        .child(bone(p, at, 160.0, 12.0).mt(10.0));
    let mut top = row().w_full().gap(16.0).items_end().flex_wrap();
    let mut x = at;
    if !audio_only {
        top = top.child(bone(p, x, 168.0, 32.0).radius(8.0).clip_rounded(8.0));
        x += 168.0 + 16.0;
    }
    let pick_w = if video { 176.0 } else { 144.0 };
    top = top.child(col().gap(4.0).child(bone(p, x, 88.0, 12.0)).child(bone(p, x, pick_w, 32.0).radius(8.0).clip_rounded(8.0)));
    let mut options = col().w_full().gap(16.0).child(top);
    if !video {
        let mut chips = row().w_full().gap(8.0).flex_wrap();
        let mut x = at;
        for w in [56.0, 56.0, 56.0, 56.0, 60.0, 104.0, 100.0, 104.0] {
            chips = chips.child(bone(p, x, w, 32.0).radius(16.0).clip_rounded(16.0));
            x += w + 8.0;
        }
        options = options.child(col().w_full().gap(8.0).child(bone(p, at, 96.0, 12.0)).child(chips));
    }
    if s.plan == Plan::Basic {
        options = options.child(bone(p, at, 280.0, 16.0));
    }
    let notes = col().grow(1.0).min_w(0.0).gap(24.0).child(words).child(options);
    row().w_full().padding(pad).gap(gap).items_start().role("status").label(f::NOTE_READING).child(art).child(notes)
}

// ── The look-up failed ──────────────────────────────────────────────────────

fn failed(p: &Pal, why: &str, input: &str) -> El {
    let panel = row()
        .w_full()
        .padding(16.0)
        .gap(12.0)
        .items_start()
        .radius(8.0)
        .fill(p.soft(p.error))
        .border(1.0, p.error.alpha(0.3))
        .role("alert")
        .child(glyph(&icons::ALERT, p.error).mt(2.0))
        .child(
            col().grow(1.0).min_w(0.0).gap(12.0).child(text(why).font(sans(14.0, 400.0)).color(p.text).wrap()).child(
                row()
                    .w_full()
                    .gap(12.0)
                    .center_y()
                    .child(text(input).font(mono(12.0, 400.0)).color(p.muted).single_line().truncate().min_w(0.0).shrink())
                    .child(spacer())
                    .child(ghost(p, keys::FOUND_RETRY, Some(&icons::RETRY), f::ERROR_TRY_AGAIN, p.text).on_press_tx(actions::submit)),
            ),
        );
    // No ring after a dead look-up: one on the rim would read as still waiting.
    col().w_full().padding(24.0).child(panel)
}

// ── One video or track ──────────────────────────────────────────────────────

/// Where the item comes from: the extractor that read it, else the link's own detection.
fn source_of(m: &MediaMetadata) -> Src {
    match m.extractor.as_deref().map(str::to_lowercase).as_deref() {
        Some("youtube") => Src::Youtube,
        Some("soundcloud") => Src::Soundcloud,
        Some("spotify") => Src::Spotify,
        Some("instagram") => Src::Instagram,
        Some("tiktok") => Src::Tiktok,
        Some("facebook") => Src::Facebook,
        _ => detect_url(&m.url).platform,
    }
}

/// The detector's label, or `{source} · Audio` where it gives none (a Spotify track) or names
/// another source than the one that read it.
fn eyebrow(m: &MediaMetadata, source: Src, video: bool) -> String {
    let det = detect_url(&m.url);
    if det.label.is_empty() || det.platform != source || source == Src::Spotify {
        f::single_eyebrow_fallback(format::source_name(source), !video)
    } else {
        det.label
    }
}

fn single(s: &State, p: &Pal, m: &MediaMetadata, matched: Option<&str>) -> (El, El) {
    let source = source_of(m);
    let video = s.choice.video && !AUDIO_ONLY_SOURCES.contains(&source);
    let eyebrow = eyebrow(m, source, video);
    let outcome = outcome(s, video);
    let size = 176.0;
    let length = format::duration(m.duration);
    let mut art = el().key(keys::FOUND_COVER).size(size, size).child(blank_sleeve(s, &m.thumbnail_url, p, source, size, &spine(s, video)));
    if !length.is_empty() {
        art = art.child(chip_on_art(length.clone()).pin(None, Some(8.0), Some(8.0), None));
    }
    let mut meta = row().gap(8.0).center_y();
    if !length.is_empty() {
        meta = meta
            .child(glyph(&icons::CLOCK, p.muted))
            .child(text(length).font(mono(12.0, 400.0)).color(p.text2))
            .child(text("·").font(mono(12.0, 400.0)).color(p.muted));
    }
    meta = meta.child(text(outcome.clone()).font(mono(12.0, 400.0)).color(p.text2));
    let mut words = col()
        .w_full()
        .gap(4.0)
        .child(text(eyebrow).font(mono(12.0, 400.0)).color(p.source(source)))
        .child(text(m.title.clone()).font(sans(20.0, 500.0).lh(28.0)).color(p.text).wrap().max_lines(2));
    if !m.uploader.is_empty() {
        words = words.child(text(m.uploader.clone()).font(sans(14.0, 400.0)).color(p.text2).single_line().truncate());
    }
    words = words.child(meta.mt(8.0));
    if let Some(name) = matched {
        words = words.child(
            runs(vec![run(f::MATCHED_LEAD, p.text2), run_mono(name, p.text), run(f::MATCHED_TRAIL, p.text2)])
                .font(sans(13.0, 400.0))
                .wrap()
                .mt(4.0),
        );
    }
    let notes = col().grow(1.0).min_w(0.0).gap(24.0).child(words).child(options(s, p, source));
    let body = row().w_full().padding(24.0).gap(32.0).items_start().child(art).child(notes);
    (body, foot(p, readout(p, outcome), download(p, f::SINGLE_DOWNLOAD)))
}

// ── A short-form post ───────────────────────────────────────────────────────

fn short(s: &State, p: &Pal, m: &MediaMetadata, label: &str) -> (El, El) {
    let source = source_of(m);
    let video = s.choice.video;
    let tint = p.reels();
    let length = format::duration(m.duration);
    let (w, h) = (152.0, 272.0);
    let poster =
        el().size(w, h).radius(8.0).center().fill(p.card.mix(tint, 0.16)).border(1.0, p.line).child(icon(&icons::REELS, 24.0).color(tint));
    let mut poster = crate::covers::over(s, poster, &m.thumbnail_url, 8.0);
    if !length.is_empty() {
        poster = poster.child(chip_on_art(length.clone()).pin(None, None, Some(8.0), Some(8.0)));
    }
    let eyebrow = if label.is_empty() { copy::WORKS_WITH_REELS.to_string() } else { label.to_string() };
    let mut words = col()
        .w_full()
        .gap(4.0)
        .child(text(eyebrow).font(mono(12.0, 400.0)).color(tint))
        .child(text(m.title.clone()).font(sans(20.0, 500.0).lh(28.0)).color(p.text).wrap().max_lines(2));
    if !m.uploader.is_empty() {
        words = words.child(text(m.uploader.clone()).font(sans(14.0, 400.0)).color(p.text2).single_line().truncate());
    }
    if let Some(caption) = m.description.as_deref().filter(|d| !d.is_empty() && *d != m.title) {
        words = words.child(text(caption).font(sans(13.0, 400.0)).color(p.text2).wrap().max_lines(3).mt(8.0));
    }
    // The facts, each left out when unknown.
    let facts: Vec<String> = [
        Some(length).filter(|l| !l.is_empty()),
        m.width.zip(m.height).map(|(w, h)| f::short_size(w as u32, h as u32)),
        m.view_count.map(|n| f::short_views(&format::short_count(n))),
        m.like_count.map(|n| f::short_likes(&format::short_count(n))),
        m.upload_date.as_deref().and_then(format::short_date),
    ]
    .into_iter()
    .flatten()
    .collect();
    if let Some((first, rest)) = facts.split_first() {
        let mut line = vec![run_mono(first.clone(), p.text2)];
        line.extend(rest.iter().map(|x| run_mono(copy::row_dot_part(x), p.text2)));
        words = words.child(runs(line).font(mono(12.0, 400.0)).single_line().truncate().mt(8.0));
    }
    let notes = col().grow(1.0).min_w(0.0).gap(24.0).child(words).child(options(s, p, source));
    let body = row().w_full().padding(24.0).gap(32.0).items_start().child(poster).child(notes);
    (body, foot(p, readout(p, outcome(s, video)), download(p, f::SHORT_DOWNLOAD)))
}

// ── A playlist or album: the tracklist on the back of the sleeve ───────────

/// Where a track's match stands: a mark and a word, so it reads without its colour.
fn match_cell(s: &State, p: &Pal, m: Matched) -> El {
    let (lead, words, ink) = match m {
        Matched::Confirmed => (glyph(&icons::CHECK_SHELF, p.success), copy::MATCH_CONFIRMED, p.text2),
        Matched::BestGuess(_) => (glyph(&icons::QUESTION, p.warning), copy::MATCH_BEST_GUESS, p.text2),
        Matched::Matching => {
            // TODO(motion): `reduced` comes from the motion context at step 52.
            (ring::waiting(keys::RING_WAITING, 16.0, &ring::shelf(s.palette, p.dark, p.spotify()), false, 0), copy::MATCH_MATCHING, p.muted)
        }
        Matched::NoMatch => (glyph(&icons::CANCEL, p.error), copy::MATCH_NONE, p.text2),
    };
    let mut r = row().w(176.0).gap(8.0).center_y().child(lead).child(text(words).font(sans(13.0, 400.0)).color(ink).single_line());
    if let Matched::BestGuess(c) = m {
        r = r.child(text(format!("{}%", format::round_half_up(c * 100.0))).font(mono(12.0, 400.0)).color(p.muted));
    }
    // Still matching: the cell shimmers like a bone.
    // Its column: the row's inset, the length cell and the gap before it, from the right.
    if m == Matched::Matching { r.clip_rounded(4.0).child(sheen(p, body_w() - 24.0 - 48.0 - 16.0 - 176.0)) } else { r }
}

/// A tick Basic holds: on, in a quieter ink, with no hover.
fn tick_fixed(p: &Pal, on: bool) -> El {
    let b = el().size(20.0, 20.0).radius(4.0).center().role("checkbox").checked(on).border(1.0, p.line);
    if on { b.fill(p.raised).child(icon(&icons::CHECK_SHELF, 16.0).color(p.muted)) } else { b }
}

fn album(s: &State, p: &Pal, pl: &Playlist, matched: &[Matched], selected: &[bool]) -> (El, El) {
    // A Spotify album or playlist matches each track; a YouTube playlist is one job.
    let spotify = pl.url.contains("spotify");
    let source = if spotify { Src::Spotify } else { Src::Youtube };
    let tint = p.source(source);
    let eyebrow = if !spotify {
        format::source_name(source)
    } else if pl.url.contains("/album/") {
        f::PLAYLIST_EYEBROW_ALBUM
    } else {
        f::PLAYLIST_EYEBROW_PLAYLIST
    };
    let basic = s.plan == Plan::Basic;
    // On Basic a YouTube playlist is taken from the top: its rows cannot be picked.
    let fixed = basic && !spotify;
    let cap = BASIC_LIMITS.max_collection_tracks as usize;

    let tracks = &pl.tracks;
    let picked: Vec<usize> = (0..tracks.len()).filter(|i| selected.get(*i).copied().unwrap_or(false)).collect();
    let span_of = |rows: &mut dyn Iterator<Item = usize>| f::playlist_span(rows.map(|i| tracks[i].duration_ms).sum());
    let runtime = span_of(&mut (0..tracks.len()));
    let picked_span = span_of(&mut picked.iter().copied());
    // A YouTube playlist ticked through counts every video, listed or not.
    let whole = !spotify && picked.len() == tracks.len();
    let (picked_n, total_n) = if whole { (pl.track_count as usize, pl.track_count as usize) } else { (picked.len(), tracks.len()) };
    let matched_n = picked.iter().filter(|i| matched.get(**i) != Some(&Matched::NoMatch)).count() + picked_n - picked.len();
    let queued = if basic { matched_n.min(cap) } else { matched_n };
    let (fi, ai, _) = lit(s);

    let mut counter = col()
        .items_end()
        .gap(4.0)
        .child(
            row()
                .items_end()
                .gap(4.0)
                .child(text(picked_n.to_string()).font(mono(32.0, 500.0).lh(32.0)).color(p.text))
                .child(text(f::playlist_selected_total(total_n)).font(mono(16.0, 400.0)).color(p.muted).pb(2.0)),
        )
        .child(
            text(if picked.is_empty() { f::PLAYLIST_NONE_SELECTED.to_string() } else { f::playlist_selected_span(&picked_span) })
                .font(mono(12.0, 400.0))
                .color(p.muted),
        );
    if basic && picked_n <= cap {
        counter = counter.child(text(f::BASIC_COLLECTION).font(sans(12.0, 400.0)).color(p.muted));
    }
    let all = picked.len() == tracks.len();
    let mut top = row()
        .w_full()
        .px(24.0)
        .py(16.0)
        .gap(16.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line_soft)
        .child(blank_sleeve(s, &pl.thumbnail_url, p, source, 64.0, f::AUDIO_FORMATS[fi]))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(text(eyebrow).font(mono(12.0, 400.0)).color(tint))
                .child(text(pl.name.clone()).font(sans(18.0, 500.0)).color(p.text).single_line().truncate())
                .child(text(f::playlist_meta(pl.track_count as usize, &runtime, &pl.owner)).font(mono(12.0, 400.0)).color(p.muted)),
        );
    if !fixed {
        top = top.child(
            row()
                .key(keys::FOUND_SELECT_ALL)
                .h(32.0)
                .px(12.0)
                .gap(8.0)
                .center_y()
                .radius(8.0)
                .border(1.0, p.line)
                .hover(p.raised)
                .cursor(Cursor::Pointer)
                .focusable(true)
                .role("button")
                .on_press(actions::toggle_all_tracks)
                .child(tick(p, all, true))
                .child(text(if all { f::PLAYLIST_DESELECT_ALL } else { f::PLAYLIST_SELECT_ALL }).font(sans(13.0, 500.0)).color(p.text)),
        );
    }
    let mut body = col().w_full().child(top.child(counter));
    // A strip under the sleeve's head: what Basic holds back, and why.
    let strip = |words: El, button: Option<El>| {
        let r = row()
            .w_full()
            .px(24.0)
            .py(12.0)
            .gap(12.0)
            .center_y()
            .fill(p.raised)
            .border_sides([0.0, 0.0, 1.0, 0.0], p.line_soft)
            .child(lock(p.text2))
            .child(words.grow(1.0).min_w(0.0));
        match button {
            Some(b) => r.child(b),
            None => r,
        }
    };
    if basic && picked_n > cap {
        let skipped = runs(vec![
            run(f::LIMIT_SKIPPED_LEAD, p.text2),
            run_mono((picked_n - cap).to_string(), p.text2),
            run(f::LIMIT_SKIPPED_TRAIL, p.text2),
        ])
        .font(sans(13.0, 400.0))
        .wrap();
        let words = col().gap(4.0).child(text(f::LIMIT_COLLECTION).font(sans(13.0, 500.0)).color(p.text).wrap()).child(skipped);
        body = body.child(strip(words, Some(ghost(p, keys::FOUND_SEE_PLANS, None, f::SEE_PLANS, p.text))));
    }
    if fixed {
        body = body.child(strip(text(f::LOCK_PICK_PLAYLIST).font(sans(13.0, 400.0)).color(p.text2).wrap(), None));
    }

    let cols = |a: El, b: El, t: El, ar: El, m: Option<El>, time: El| {
        let r = row()
            .w_full()
            .px(24.0)
            .gap(16.0)
            .center_y()
            .child(a.w(20.0))
            .child(b.w(24.0))
            .child(t.grow(2.0).min_w(0.0))
            .child(ar.grow(1.0).min_w(0.0));
        match m {
            Some(m) => r.child(m),
            None => r,
        }
        .child(time.w(48.0))
    };
    let heading = |w: &str| text(w).font(sans(12.0, 500.0)).color(p.muted).single_line();
    let mut title_head = row().gap(12.0).center_y().child(heading(f::PLAYLIST_COL_TITLE));
    if !fixed {
        title_head = title_head.child(text(f::PLAYLIST_COL_HINT).font(sans(12.0, 400.0)).color(p.muted).single_line().truncate().shrink());
    }
    let header = cols(
        el(),
        heading(f::PLAYLIST_COL_NUMBER),
        title_head,
        heading(f::PLAYLIST_COL_ARTIST),
        spotify.then(|| heading(copy::COL_MATCH).w(176.0)),
        row().justify_end().child(heading(f::PLAYLIST_COL_TIME)),
    )
    .h(32.0)
    .fill(p.bg)
    .border_sides([0.0, 0.0, 1.0, 0.0], p.line_soft);

    let mut list = col().w_full().role("list").label(pl.name.clone());
    let mut rank = 0;
    for (i, t) in tracks.iter().enumerate() {
        let m = matched.get(i).copied().unwrap_or(Matched::Matching);
        let can = m != Matched::NoMatch;
        let on = can && selected.get(i).copied().unwrap_or(false);
        // Past the Basic limit: the ticked tracks after the 20th, in list order.
        let capped = basic && spotify && on && rank >= cap;
        rank += usize::from(on);
        let length = format::duration(t.duration_ms / 1000.0);
        let ink = if can { p.text } else { p.muted };
        let number =
            if capped { glyph(&icons::LOCK_SIGNAL, p.warning) } else { text((i + 1).to_string()).font(mono(12.0, 400.0)).color(p.muted) };
        let mut r = cols(
            if fixed { tick_fixed(p, on) } else { tick(p, on, can) },
            number,
            text(t.name.clone()).font(sans(13.0, if on { 500.0 } else { 400.0 })).color(ink).single_line().truncate(),
            text(t.artist.clone()).font(sans(13.0, 400.0)).color(if can { p.text2 } else { p.muted }).single_line().truncate(),
            spotify.then(|| match_cell(s, p, m)),
            row().justify_end().child(text(length.clone()).font(mono(12.0, 400.0)).color(if can { p.text2 } else { p.muted })),
        )
        .key(keys::track(i))
        .h(40.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line_soft)
        .role("checkbox")
        .checked(on)
        .label(f::playlist_row_aria(&t.name, &t.artist, &length, capped));
        if can {
            r = r.on_press(move |s: &mut State| actions::toggle_track(s, i));
        }
        if can && !fixed {
            r = r.cursor(Cursor::Pointer).hover(p.raised);
        }
        if on && !fixed {
            r = r.fill(p.soft(p.text));
        }
        if capped {
            r = r.opacity(0.55);
        }
        list = list.child(r);
    }
    let body = body.child(header).child(list);

    // The foot: format, quality and folder for the whole batch, then Download.
    let folder = s
        .choice
        .folder
        .clone()
        .or_else(|| s.settings.get("outputDir").and_then(|v| v.as_str()).map(str::to_owned))
        .filter(|x| !x.is_empty());
    let folder_value = row().gap(8.0).center_y().child(glyph(&icons::FOLDER, p.muted)).child(
        text(folder.as_deref().unwrap_or(f::PLAYLIST_CHOOSE_FOLDER))
            .font(mono(13.0, 400.0))
            .color(p.text)
            .single_line()
            .truncate_middle()
            .min_w(0.0)
            .shrink(),
    );
    let mut folder_pick = select(p, keys::CHOICE_FOLDER, f::PLAYLIST_FOLDER, f::PLAYLIST_FOLDER, folder_value, 240.0);
    if let Some(full) = &folder {
        folder_pick = folder_pick.help(full.clone());
    }
    let mut format_pick = select(p, keys::CHOICE_FORMAT, f::PLAYLIST_FORMAT, f::PLAYLIST_FORMAT_ARIA, value(p, f::AUDIO_FORMATS[fi]), 96.0);
    let mut quality_pick =
        select(p, keys::CHOICE_AUDIO_QUALITY, f::PLAYLIST_QUALITY, f::PLAYLIST_QUALITY_ARIA, value(p, f::AUDIO_QUALITIES[ai]), 128.0);
    if basic {
        format_pick = format_pick.help(f::LOCK_SELECT_TITLE);
        quality_pick = quality_pick.help(f::LOCK_SELECT_TITLE);
    }
    let sum = if queued == 0 {
        f::PLAYLIST_NOTHING_SELECTED.to_string()
    } else {
        f::playlist_readout(queued, f::AUDIO_FORMATS[fi], f::AUDIO_QUALITIES[ai], &picked_span)
    };
    let left = row()
        .gap(12.0)
        .items_end()
        .child(format_pick)
        .child(quality_pick)
        .child(folder_pick)
        .child(text(sum).font(mono(12.0, 400.0)).color(p.text2).single_line().truncate().min_w(0.0).shrink().pb(8.0));
    let go = download(p, &f::playlist_download_label(matched_n, queued));
    // Nothing ticked: faded and deaf, as Shelf's other buttons that can't be used now.
    let go = if actions::can_download_playlist(selected) { go } else { go.disabled(true).inert().opacity(0.4) };
    (body, foot(p, left, go))
}

// ── A search: one band per source ───────────────────────────────────────────

fn search(s: &State, p: &Pal, query: &str, bands: &[Band; 3]) -> (El, El, Option<El>) {
    // A scoped search asks one source: only its band is shown.
    let shown: Vec<usize> = (0..3).filter(|i| s.scope.is_none_or(|sc| actions::band_of(sc) == *i)).collect();
    let loading = shown.iter().any(|i| matches!(bands[*i], Band::Loading));
    let mut words = vec![Run { weight: Some(500.0), ..run(query, p.text) }];
    if loading {
        words.push(run(copy::row_dot_part(copy::STATUS_SEARCHING), p.muted));
    }
    let head = head(p, glyph(&icons::SEARCH, p.muted), runs(words).font(sans(13.0, 400.0))).label(copy::search_region_aria(query));

    let body = if shown.iter().all(|i| matches!(bands[*i], Band::Failed)) {
        // Nothing could be reached: one sentence and one way back.
        col().w_full().padding(24.0).child(
            row()
                .w_full()
                .padding(16.0)
                .gap(12.0)
                .center_y()
                .radius(8.0)
                .fill(p.soft(p.error))
                .border(1.0, p.error.alpha(0.3))
                .role("alert")
                .child(glyph(&icons::ALERT, p.error))
                .child(text(copy::SEARCH_ALL_FAILED).font(sans(14.0, 400.0)).color(p.text).wrap().grow(1.0).min_w(0.0))
                .child(ghost(p, keys::FOUND_RETRY, Some(&icons::RETRY), copy::SEARCH_TRY_AGAIN, p.text).on_press_tx(actions::submit)),
        )
    } else if shown.iter().all(|i| matches!(bands[*i], Band::Empty)) {
        col()
            .w_full()
            .padding(24.0)
            .gap(4.0)
            .child(text(copy::search_nothing_matched(query)).font(sans(14.0, 500.0)).color(p.text).wrap())
            .child(text(copy::SEARCH_NOTHING_HINT).font(sans(13.0, 400.0)).color(p.text2).wrap())
    } else {
        // Where each band starts, for its bones' shimmer: equal shares of the body after its insets.
        let n = shown.len() as f32;
        let share = (body_w() - 48.0 - 24.0 * (n - 1.0)) / n;
        shown.iter().enumerate().fold(row().w_full().px(24.0).pt(8.0).pb(16.0).gap(24.0).items_start(), |r, (j, i)| {
            r.child(band(s, p, *i, &bands[*i], 24.0 + j as f32 * (share + 24.0)))
        })
    };

    // The picked results, sent together.
    let picked =
        bands.iter().flat_map(|b| if let Band::Results(rs) = b { rs.as_slice() } else { &[] }).filter(|r| s.picked.contains(&r.id)).count();
    let foot = (picked > 0).then(|| {
        foot(
            p,
            row().gap(12.0).center_y().child(text(f::results_selected(picked)).font(sans(13.0, 500.0)).color(p.text)).child(ghost(
                p,
                // TODO(shared): `actions::clear_picked` (nothing empties `State.picked` but ticking each row again).
                keys::RESULTS_CLEAR,
                None,
                f::RESULTS_CLEAR,
                p.text2,
            )),
            download(p, &f::results_download_n(picked)),
        )
    });
    (head, body, foot)
}

fn band(s: &State, p: &Pal, i: usize, band: &Band, x0: f32) -> El {
    let (source, name) = SOURCES[i];
    let tint = p.source(source);
    let mut head = row()
        .w_full()
        .h(40.0)
        .gap(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line_soft)
        .child(el().size(8.0, 8.0).radius(4.0).fill(tint))
        .child(text(name).font(sans(13.0, 500.0)).color(p.text));
    head = match band {
        Band::Results(rs) => head.child(text(rs.len().to_string()).font(mono(12.0, 400.0)).color(p.muted)),
        // TODO(motion): `reduced` comes from the motion context at step 52.
        Band::Loading => {
            head.child(spacer()).child(ring::waiting(keys::RING_WAITING, 16.0, &ring::shelf(s.palette, p.dark, tint), false, 0))
        }
        _ => head,
    };
    let c = col().w_full().pt(4.0);
    let body = match band {
        Band::Loading => [[168.0, 104.0], [136.0, 88.0], [152.0, 96.0]].into_iter().fold(c.role("status"), |c, w| {
            c.child(row().w_full().h(56.0).gap(12.0).center_y().child(bone(p, x0, art_w(source), 36.0)).child(
                col().gap(8.0).child(bone(p, x0 + art_w(source) + 12.0, w[0], 12.0)).child(bone(p, x0 + art_w(source) + 12.0, w[1], 8.0)),
            ))
        }),
        Band::Empty => c.pt(12.0).child(text(copy::search_no_matches(name)).font(sans(13.0, 400.0)).color(p.text2).wrap()),
        Band::Failed => c
            .pt(12.0)
            .gap(12.0)
            .role("alert")
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(glyph(&icons::ALERT, p.error))
                    .child(text(copy::search_source_failed(name)).font(sans(13.0, 400.0)).color(p.text).wrap().min_w(0.0).shrink()),
            )
            .child(
                ghost(p, &keys::band_retry(i), Some(&icons::RETRY), copy::SEARCH_RETRY, p.text)
                    .on_press(move |s: &mut State| actions::retry_band(s, i))
                    .self_start(),
            ),
        Band::Results(rs) => {
            let open = s.expanded[i];
            let mut c = rs.iter().take(if open { rs.len() } else { SHOWN }).fold(c, |c, r| c.child(result_row(s, p, r)));
            if rs.len() > SHOWN && !open {
                c = c.child(
                    ghost(p, &keys::band_more(i), Some(&icons::CARET_DOWN), &copy::search_show_more(rs.len() - SHOWN), p.text2)
                        .on_press(move |s: &mut State| actions::show_more(s, i))
                        .self_start()
                        .mt(8.0),
                );
            }
            if source == Src::Spotify {
                c = c.child(text(copy::SEARCH_SPOTIFY_NOTE).font(mono(12.0, 400.0)).color(p.muted).pt(8.0));
            }
            c
        }
    };
    col().grow(1.0).min_w(0.0).child(head).child(body)
}

/// YouTube's pictures are 16:9; SoundCloud's and Spotify's are square.
fn art_w(source: Src) -> f32 {
    if source == Src::Youtube { 64.0 } else { 36.0 }
}

fn result_row(s: &State, p: &Pal, r: &SearchResult) -> El {
    let on = s.picked.contains(&r.id);
    let (row_id, tick_id) = (r.id.clone(), r.id.clone());
    let tint = p.source(r.source);
    let length = format::duration(r.duration);
    let mut sub = Vec::new();
    if !r.uploader.is_empty() {
        sub.push(run(r.uploader.clone(), p.text2));
    }
    if !length.is_empty() {
        sub.push(run_mono(if sub.is_empty() { length } else { copy::row_dot_part(&length) }, p.muted));
    }
    // The picture that has not arrived: the source's tint with a record label at its middle.
    let art = el()
        .size(art_w(r.source), 36.0)
        .radius(4.0)
        .center()
        .fill(p.raised.mix(tint, 0.18))
        .child(el().size(8.0, 8.0).radius(4.0).fill(tint));
    let art = crate::covers::over(s, art, &r.thumbnail_url, 4.0);
    let mut c = row()
        .key(keys::result(&r.id))
        .w_full()
        .h(56.0)
        .px(8.0)
        .gap(12.0)
        .center_y()
        .radius(8.0)
        .hover(p.raised)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .role("button")
        .label(copy::search_row_aria(&r.title, &r.uploader))
        .on_press(move |s: &mut State| actions::toggle_result(s, &row_id))
        .child(
            tick(p, on, true)
                .key(keys::result_tick(&r.id))
                .label(f::results_pick_aria(&r.title))
                .on_press(move |s: &mut State| actions::toggle_result(s, &tick_id)),
        )
        .child(art)
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(text(r.title.clone()).font(sans(13.0, 500.0)).color(p.text).single_line().truncate())
                .child(runs(sub).font(sans(12.0, 400.0)).single_line().truncate()),
        )
        .child(glyph(&icons::DOWNLOAD, p.muted).help(copy::SEARCH_ROW_DOWNLOAD));
    if on {
        c = c.fill(p.soft(p.text));
    }
    c
}
