//! What the field found, in the Orbit look: the step between a paste and a running download.
//! Both Orbit presets draw this same body; each frames it round its own paste field (classic
//! Orbit opens the field into it, Now playing raises it from the transport bar).
//!
//! Nothing here travels yet, so nothing is round and no progress ring shows. The one
//! exception is waiting: a pass still being planned is the dotted orbit, as a queued download
//! is, and it marks a lookup in progress and a track still matching. Skeleton shapes are
//! `bone`s, flat blocks in the raised colour that can shimmer later.
use ahg_engine::tier::{self, BASIC_LIMITS, Plan};
use ahg_engine::types::{AUDIO_ONLY_SOURCES, MediaMetadata, Playlist, SearchResult, SourcePlatform as Src, VideoQuality};
use rune::{El, Icon, PopupItem, ScrollSpec, TextStyle, col, el, icon, row, spacer};

use super::draw;
use super::look::{BODY, BODY_M, FIG, FIG_L, Pal, SMALL, SMALL_M, mix, txt};
use super::{Tone, button, icon_button, keycap, model, pick_list};
use crate::actions;
use crate::copy::{self, found as f};
use crate::format;
use crate::gate::{Band, Card, Choice, Found, Matched, State};
use crate::icons;
use crate::keys;

/// The found item's name: 20 on the look's scale.
const TITLE: TextStyle = TextStyle::new(20.0, 500.0).lh(28.0);
/// Rows a search band shows before "Show n more".
const BAND_ROWS: usize = 5;
// Unused until cleanup (step 19): the words are `copy::MATCH_*` and `copy::COL_MATCH` now.
#[allow(dead_code)]
const MATCHING: &str = "Matching…";
#[allow(dead_code)]
const CONFIRMED: &str = "Confirmed";
#[allow(dead_code)]
const BEST_GUESS: &str = "Best guess";
#[allow(dead_code)]
const NO_MATCH: &str = "No match found";
#[allow(dead_code)]
const COL_MATCH: &str = "Match";
/// The labels of `copy::found`, in the order of these values.
const AUDIO_FORMAT_VALUES: [&str; 8] = ["mp3", "aac", "m4a", "ogg", "opus", "flac", "wav", "alac"];
const AUDIO_QUALITY_VALUES: [&str; 5] = ["128", "192", "256", "320", "lossless"];
/// The labels run in the order of the engine's qualities and formats.
const QUALITIES: [VideoQuality; 6] =
    [VideoQuality::P360, VideoQuality::P480, VideoQuality::P720, VideoQuality::P1080, VideoQuality::P2160, VideoQuality::Best];
/// The video qualities' values, in the order of `QUALITIES`.
const VIDEO_QUALITY_VALUES: [&str; 6] = ["360p", "480p", "720p", "1080p", "2160p", "best"];

/// A view or like count as printed. TODO(shared): format::count_short (`1.2K`, `4.8M`,
/// a trailing `.0` dropped); the whole number stands in until it exists.
fn count(n: f64) -> String {
    format!("{}", n.round())
}

/// Where the pick sits in the label lists: audio format, audio quality, video quality. What
/// the plan would really run: on Basic a saved FLAC shows MP3, Lossless 320 and 4K 1080p.
fn picks(c: &Choice, plan: Plan) -> (usize, usize, usize) {
    let at = |list: &[&str], v: &str| list.iter().position(|x| *x == v).unwrap_or(0);
    let video = tier::clamp_video_quality(c.video_quality, plan);
    (
        at(&AUDIO_FORMAT_VALUES, tier::clamp_format(&c.format.to_lowercase(), plan)),
        at(&AUDIO_QUALITY_VALUES, tier::clamp_audio_quality(&c.audio_quality, plan)),
        QUALITIES.iter().position(|q| *q == video).unwrap_or(5),
    )
}

/// Where this download goes: its own folder, else the saved one.
fn folder(s: &State) -> String {
    s.choice.folder.clone().or_else(|| s.settings.get("outputDir").and_then(|v| v.as_str()).map(str::to_owned)).unwrap_or_default()
}

/// The found step's choice with one part changed, through `actions::set_choice` (which says what a locked pick needs).
fn set(s: &mut State, change: impl FnOnce(&mut Choice)) {
    let mut c = s.choice.clone();
    change(&mut c);
    actions::set_choice(s, c);
}

/// A pick list's entries, the one at `on` checked.
fn entries(labels: &[&str], values: &[&str], on: usize) -> Vec<PopupItem> {
    labels.iter().zip(values).enumerate().map(|(i, (l, v))| PopupItem::new(*v, *l).checked(i == on)).collect()
}

/// The way back: the Esc keycap and a close button.
pub(crate) fn dismiss(p: &Pal) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(keycap(p, copy::KEYCAP_ESC))
        .child(icon_button(p, keys::FOUND_CLOSE.into(), f::RESULTS_CLEAR, &icons::CLOSE, p.text2).on_press(actions::dismiss_found))
}

/// True when the body is tall enough to need all the height it can get (a track list).
pub(crate) fn fills(s: &State) -> bool {
    matches!(s.found, Found::Playlist { .. })
}

/// The surface's content, or `None` when nothing was looked up.
pub(crate) fn body(s: &State, p: &Pal) -> Option<El> {
    Some(match &s.found {
        Found::Nothing => return None,
        Found::LookingUp => looking_up(p, &s.input),
        Found::LookupFailed(why) => failed(p, &s.input, why),
        Found::Media(m) => media(p, m, s),
        Found::Playlist { playlist, matched, selected } => {
            tracks(p, playlist, matched, selected, &s.choice, &folder(s), s.plan == Plan::Basic)
        }
        Found::Search { bands, .. } => search(s, p, bands),
    })
}

// ── Pieces ──────────────────────────────────────────────────────────────────

/// A skeleton block: the shape of something still coming.
fn bone(p: &Pal, w: f32, h: f32) -> El {
    el().w(w).h(h).radius(4.0).fill(mix(p.raised, p.text, 0.04))
}

/// The ring at 16, waiting: something is being worked out.
fn waiting(p: &Pal) -> El {
    draw::waiting_ring(16.0, &draw::material(p, p.accent, p.card))
}

/// Artwork that has not arrived: a tile tinted by its source, its kind's glyph.
fn art(p: &Pal, src: Src, w: f32, h: f32, glyph: &'static Icon) -> El {
    el().size(w, h)
        .radius(if w >= 160.0 { 12.0 } else { 8.0 })
        .fill(mix(p.raised, model::source(p, src).1, 0.12))
        .center()
        .child(icon(glyph, 16.0).color(p.muted))
}

/// The link the field holds, as a short mono line over the answer.
fn subject(p: &Pal, input: &str) -> El {
    row().gap(8.0).center_y().child(icon(&icons::LINK, 16.0).color(p.muted)).child(txt(format::link_label(input), FIG, p.text2).truncate())
}

/// A choice. A locked one (Basic) stays pressable, so choosing it can say what Premium adds:
/// a lock before the word and a dashed edge, in the neutral inks, never the accent.
fn chip(p: &Pal, label: &str, on: bool, tag: Option<&str>, locked: bool) -> El {
    let mut c = row()
        .h(28.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .border(
            1.0,
            if on {
                p.text2
            } else if locked {
                p.track
            } else {
                p.line
            },
        )
        .role("radio")
        .checked(on)
        .focusable(true)
        .cursor(rune::Cursor::Pointer);
    if locked {
        c = c
            .border_dash(3.0, 3.0)
            .label(f::lock_chip_aria(label))
            .help(f::lock_chip_title(label))
            .child(icon(&icons::LOCK_ORBIT, 16.0).color(p.text2));
    } else {
        c = c.label(label.to_string());
    }
    c = c.child(txt(label.to_string(), if on { SMALL_M } else { SMALL }, if on { p.text } else { p.text2 }));
    c = if on { c.fill(p.raised) } else { c.hover(p.hover) };
    if let Some(t) = tag {
        c = c.child(txt(t, FIG, p.muted));
    }
    c
}

/// A labelled row of choices; `locked` says which need Premium.
fn choices(
    p: &Pal,
    (label, group): (&str, &str),
    items: &[&str],
    on: usize,
    lossless_from: Option<usize>,
    locked: &dyn Fn(usize) -> bool,
    at: &dyn Fn(El, usize) -> El,
) -> El {
    let mut r = row().key(group.to_string()).gap(8.0).flex_wrap().role("radiogroup").label(label.to_string());
    for (i, item) in items.iter().enumerate() {
        r = r.child(at(chip(p, item, i == on, lossless_from.filter(|from| i >= *from).map(|_| f::TAG_LOSSLESS), locked(i)), i));
    }
    col().gap(8.0).child(txt(label.to_string(), SMALL, p.text2)).child(r)
}

/// What Basic includes, with the way to see the plans: the positive side of a lock.
fn basic_line(p: &Pal, words: &str) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(icon(&icons::LOCK_ORBIT, 16.0).color(p.muted))
        .child(txt(words.to_string(), SMALL, p.text2))
        // TODO(shared): `actions::see_plans(s)`; the notice's See plans is `Act::SeePlans`, a no-op until the plans sheet.
        .child(txt(f::SEE_PLANS, SMALL_M, p.text).key(keys::FOUND_SEE_PLANS).role("button").focusable(true).cursor(rune::Cursor::Pointer))
}

/// A drop-down showing its value, its name over it.
fn select(p: &Pal, label: &str, value: &str, glyph: Option<&'static Icon>, at: impl FnOnce(El) -> El) -> El {
    let mut b = row()
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .border(1.0, p.track)
        .hover(p.raised)
        .role("combobox")
        .label(label.to_string())
        .focusable(true)
        .cursor(rune::Cursor::Pointer);
    if let Some(g) = glyph {
        b = b.child(icon(g, 16.0).color(p.text2));
    }
    b = b.child(txt(value.to_string(), FIG, p.text)).child(icon(&icons::CARET_DOWN, 16.0).color(p.muted));
    col().gap(8.0).child(txt(label.to_string(), SMALL, p.text2)).child(at(b))
}

fn readout(p: &Pal, outcome: String) -> El {
    row().center_y().child(txt(f::READOUT_LEAD, SMALL, p.text2)).child(txt(outcome, FIG, p.text))
}

// ── A link is being read ────────────────────────────────────────────────────

/// The skeleton of a single item (the common case), under the note and the link.
fn looking_up(p: &Pal, input: &str) -> El {
    let chips = (0..6).fold(row().gap(8.0), |r, i| r.child(bone(p, [48.0, 48.0, 64.0, 104.0, 80.0, 104.0][i], 28.0)));
    let words = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(12.0)
        .child(bone(p, 112.0, 12.0))
        .child(bone(p, 440.0, 20.0))
        .child(bone(p, 280.0, 20.0))
        .child(bone(p, 160.0, 12.0))
        .child(bone(p, 168.0, 32.0).mt(12.0))
        .child(chips)
        .child(row().w_full().mt(12.0).child(bone(p, 200.0, 16.0)).child(spacer()).child(bone(p, 120.0, 32.0)));
    let note = row().gap(8.0).center_y().child(waiting(p)).child(txt(f::NOTE_READING, FIG, p.text2));
    col()
        .w_full()
        .gap(20.0)
        .label(f::NOTE_READING)
        .child(row().w_full().gap(24.0).center_y().child(note).child(spacer()).child(subject(p, input)))
        .child(row().w_full().gap(32.0).items_start().child(bone(p, 320.0, 180.0).radius(12.0)).child(words))
}

// ── The lookup failed ───────────────────────────────────────────────────────

fn failed(p: &Pal, input: &str, why: &str) -> El {
    let mark =
        el().size(40.0, 40.0).radius(20.0).center().border(1.0, mix(p.error, p.card, 0.5)).child(icon(&icons::ALERT, 16.0).color(p.error));
    row().w_full().gap(16.0).items_start().role("alert").child(mark).child(
        col().grow(1.0).min_w(0.0).gap(8.0).child(subject(p, input)).child(txt(why.to_string(), BODY, p.text).wrap().max_w(640.0)).child(
            row()
                .mt(8.0)
                .child(button(p, keys::FOUND_RETRY, f::ERROR_TRY_AGAIN, Some(&icons::RETRY), Tone::Ghost).on_press_tx(actions::submit)),
        ),
    )
}

// ── One video or track ──────────────────────────────────────────────────────

/// One item: the single card, or the short-form poster (`State.card`).
fn media(p: &Pal, m: &MediaMetadata, s: &State) -> El {
    let (choice, basic) = (&s.choice, s.plan == Plan::Basic);
    let (af, aq, vq) = picks(choice, s.plan);
    // The extractor names YouTube even for a link the detector cannot read; else the link's own source.
    let src = if m.extractor.as_deref().is_some_and(|e| e.starts_with("youtube")) {
        Src::Youtube
    } else {
        ahg_engine::url_detector::detect_url(&m.url).platform
    };
    // Spotify and SoundCloud are audio only: no Video or Audio to choose.
    let audio_only = AUDIO_ONLY_SOURCES.contains(&src);
    let video = choice.video && !audio_only;
    let (name, color) = model::source(p, src);
    let duration = format::duration(m.duration);
    // A short-form post keeps its own shape: a 9:16 poster, its detector label for an eyebrow.
    let short = match &s.card {
        Card::Short { label } => Some(label.as_str()),
        Card::Single { .. } => None,
    };
    let mut cover = match short {
        Some(_) => art(p, src, 152.0, 272.0, &icons::REELS),
        None => art(p, src, 320.0, 180.0, &icons::VIDEO),
    };
    if !duration.is_empty() {
        cover = cover.child(row().h(20.0).px(4.0).center().radius(4.0).fill(p.deep).child(txt(duration.clone(), FIG, p.text)).pin(
            None,
            None,
            Some(8.0),
            Some(8.0),
        ));
    }
    let outcome =
        if video { f::readout_video(f::VIDEO_QUALITIES[vq]) } else { f::readout_audio(f::AUDIO_FORMATS[af], f::AUDIO_QUALITIES[aq]) };
    let mut meta = row().gap(8.0).center_y();
    if !duration.is_empty() {
        meta =
            meta.child(icon(&icons::CLOCK, 16.0).color(p.muted)).child(txt(duration.clone(), FIG, p.text2)).child(txt("·", FIG, p.muted));
    }
    meta = meta.child(txt(outcome.clone(), FIG, p.text2));
    let eyebrow = match short {
        Some(label) => txt(label.to_string(), SMALL_M, p.reels),
        None => txt(f::single_eyebrow_fallback(name, !video), SMALL_M, color),
    };
    let mut words = col().grow(1.0).min_w(0.0).gap(4.0).child(eyebrow).child(txt(m.title.clone(), TITLE, p.text).wrap().max_lines(2));
    if !m.uploader.is_empty() {
        words = words.child(txt(m.uploader.clone(), BODY, p.text2).truncate());
    }
    if short.is_some() {
        // The caption only when it says more than the title; then the facts, each dropped
        // when unknown. TODO(shared): the upload date, short (`Mar 9, 2024`), needs a formatter.
        if let Some(caption) = m.description.as_ref().filter(|d| !d.is_empty() && **d != m.title) {
            words = words.child(txt(caption.clone(), SMALL, p.text2).wrap().max_lines(3).mt(4.0));
        }
        let mut facts: Vec<String> = Vec::new();
        if !duration.is_empty() {
            facts.push(duration.clone());
        }
        if let (Some(w), Some(h)) = (m.width, m.height) {
            facts.push(f::short_size(w as u32, h as u32));
        }
        facts.extend(m.view_count.map(|n| f::short_views(&count(n))));
        facts.extend(m.like_count.map(|n| f::short_likes(&count(n))));
        let mut line = row().gap(8.0).center_y().flex_wrap();
        for (i, fact) in facts.into_iter().enumerate() {
            if i > 0 {
                line = line.child(txt("·", FIG, p.muted));
            }
            line = line.child(txt(fact, FIG, p.text2));
        }
        words = words.child(line.mt(8.0));
    } else {
        words = words.child(meta.mt(8.0));
    }
    // A Spotify track: what the audio was matched to, with the advice to check the name.
    if let Card::Single { matched: Some(found) } = &s.card {
        words = words.child(
            row()
                .flex_wrap()
                .center_y()
                .mt(4.0)
                .child(txt(f::MATCHED_LEAD, SMALL, p.text2))
                .child(txt(found.clone(), FIG, p.text))
                .child(txt(f::MATCHED_TRAIL, SMALL, p.text2)),
        );
    }

    // Video or audio, then that kind's own choices.
    let mut kind = row().self_start().gap(4.0).padding(4.0).radius(8.0).border(1.0, p.track).role("radiogroup");
    for (label, glyph, on, key) in
        [(f::MEDIA_VIDEO, &icons::VIDEO, video, keys::CHOICE_VIDEO), (f::MEDIA_AUDIO, &icons::MUSIC, !video, keys::CHOICE_AUDIO)]
    {
        let mut seg = row()
            .key(key)
            .on_press(move |s: &mut State| set(s, |c| c.video = key == keys::CHOICE_VIDEO))
            .h(24.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(4.0)
            .role("radio")
            .checked(on)
            .focusable(true)
            .label(label)
            .cursor(rune::Cursor::Pointer);
        seg = if on { seg.fill(p.raised) } else { seg.hover(p.hover) };
        kind = kind.child(seg.child(icon(glyph, 16.0).color(if on { p.text } else { p.muted })).child(txt(
            label,
            if on { SMALL_M } else { SMALL },
            if on { p.text } else { p.text2 },
        )));
    }
    let mut options = col().w_full().gap(16.0);
    if !audio_only {
        options = options.child(kind);
    }
    if video {
        options = options.child(choices(
            p,
            (f::VIDEO_QUALITY_LABEL, keys::CHOICE_VIDEO_QUALITY),
            &f::VIDEO_QUALITIES,
            vq,
            None,
            &|i| basic && tier::is_premium_video_quality(QUALITIES[i]),
            // TODO(shared): keys::choice_video_quality(value), one key per chip.
            &|c, i| {
                c.key(format!("{}-{}", keys::CHOICE_VIDEO_QUALITY, VIDEO_QUALITY_VALUES[i]))
                    .on_press(move |s: &mut State| set(s, |c| c.video_quality = QUALITIES[i]))
            },
        ));
        if basic {
            options = options.child(basic_line(p, f::BASIC_VIDEO));
        }
    } else {
        options = options
            .child(choices(
                p,
                (f::PLAYLIST_FORMAT, keys::CHOICE_FORMAT),
                &f::AUDIO_FORMATS,
                af,
                Some(5),
                &|i| basic && tier::is_premium_format(&f::AUDIO_FORMATS[i].to_lowercase()),
                &|c, i| {
                    c.key(keys::choice_format(AUDIO_FORMAT_VALUES[i]))
                        .on_press(move |s: &mut State| set(s, |c| c.format = AUDIO_FORMAT_VALUES[i].into()))
                },
            ))
            .child(choices(
                p,
                (f::AUDIO_QUALITY_LABEL, keys::CHOICE_AUDIO_QUALITY),
                &f::AUDIO_QUALITIES,
                aq,
                None,
                &|i| basic && !BASIC_LIMITS.lossless && i == f::AUDIO_QUALITIES.len() - 1,
                // TODO(shared): keys::choice_audio_quality(value), one key per chip.
                &|c, i| {
                    c.key(format!("{}-{}", keys::CHOICE_AUDIO_QUALITY, AUDIO_QUALITY_VALUES[i]))
                        .on_press(move |s: &mut State| set(s, |c| c.audio_quality = AUDIO_QUALITY_VALUES[i].into()))
                },
            ));
        if basic {
            options = options.child(basic_line(p, f::BASIC_AUDIO));
        }
    }
    let foot = row()
        .w_full()
        .gap(16.0)
        .center_y()
        .pt(16.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .child(readout(p, outcome))
        .child(spacer())
        .child(
            button(p, keys::FOUND_DOWNLOAD, f::SINGLE_DOWNLOAD, Some(&icons::DOWNLOAD), Tone::Primary).on_press(actions::download_found),
        );
    col()
        .w_full()
        .gap(24.0)
        .child(row().w_full().gap(32.0).items_start().child(cover).child(col().grow(1.0).min_w(0.0).gap(24.0).child(words).child(options)))
        .child(foot)
}

// ── A playlist or album ─────────────────────────────────────────────────────

fn tick(p: &Pal, on: bool, usable: bool) -> El {
    let b = el().size(20.0, 20.0).radius(4.0).center().role("checkbox").checked(on);
    if !usable {
        b.border(1.0, p.line).border_dash(2.0, 2.0)
    } else if on {
        b.fill(p.text).focusable(true).child(icon(&icons::CHECK_SHELF, 16.0).color(p.card))
    } else {
        b.border(1.0, p.track).focusable(true)
    }
}

/// Where a track's match stands: a mark and a word, so it reads without its colour.
fn match_cell(p: &Pal, m: Matched) -> El {
    let mark = |glyph: &'static Icon, ink| icon(glyph, 16.0).color(ink);
    let (lead, word, ink) = match m {
        Matched::Matching => (waiting(p), copy::MATCH_MATCHING, p.muted),
        Matched::Confirmed => (mark(&icons::CHECK_SHELF, p.success), copy::MATCH_CONFIRMED, p.text2),
        Matched::BestGuess(_) => (mark(&icons::QUESTION, p.warning), copy::MATCH_BEST_GUESS, p.text2),
        Matched::NoMatch => (mark(&icons::CANCEL, p.error), copy::MATCH_NONE, p.text2),
    };
    let mut r = row().w(176.0).gap(8.0).center_y().child(lead).child(txt(word, SMALL, ink));
    if let Matched::BestGuess(c) = m {
        r = r.child(txt(format!("{}%", format::round_half_up(c * 100.0)), FIG, p.muted));
    }
    r
}

fn tracks(p: &Pal, pl: &Playlist, matched: &[Matched], selected: &[bool], choice: &Choice, folder: &str, basic: bool) -> El {
    let (af, aq, _) = picks(choice, if basic { Plan::Basic } else { Plan::Premium });
    let n = pl.tracks.len();
    let spotify = pl.url.contains("spotify");
    // A YouTube playlist on Basic: its videos cannot be picked, it downloads from the top.
    let pick_locked = basic && !spotify;
    let cap = BASIC_LIMITS.max_collection_tracks as usize;
    let span = |rows: &mut dyn Iterator<Item = usize>| f::playlist_span(rows.map(|i| pl.tracks[i].duration_ms).sum());
    let picked: Vec<usize> = (0..n).filter(|i| selected[*i]).collect();
    // Only the listed rows are known; when every one is ticked the whole collection is
    // (a YouTube playlist starts all ticked), so the count is the collection's own.
    let total = (pl.track_count as usize).max(n);
    let picked_n = if picked.len() == n && total > n { total } else { picked.len() };
    let unmatched = picked.iter().filter(|i| matched[**i] == Matched::NoMatch).count();
    let queued = if basic { (picked_n - unmatched).min(cap) } else { picked_n - unmatched };
    let picked_span = span(&mut picked.iter().copied());

    let eyebrow = if pl.url.contains("/album/") { f::PLAYLIST_EYEBROW_ALBUM } else { f::PLAYLIST_EYEBROW_PLAYLIST };
    let src = if spotify { Src::Spotify } else { Src::Youtube };
    let mut counter = col()
        .items_end()
        .gap(4.0)
        .child(
            row()
                .items_end()
                .child(txt(picked_n.to_string(), FIG_L, p.text))
                .child(txt(f::playlist_selected_total(total), FIG, p.muted).mb(4.0)),
        )
        .child(txt(
            if picked.is_empty() { f::PLAYLIST_NONE_SELECTED.to_string() } else { f::playlist_selected_span(&picked_span) },
            FIG,
            p.text2,
        ));
    if basic && picked_n <= cap {
        counter = counter.child(txt(f::BASIC_COLLECTION, SMALL, p.muted));
    }
    let head =
        row().w_full().gap(16.0).center_y().child(art(p, src, 64.0, 64.0, if spotify { &icons::MUSIC } else { &icons::VIDEO })).child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(txt(eyebrow, SMALL_M, model::source(p, src).1))
                .child(txt(pl.name.clone(), TITLE, p.text).truncate())
                .child(txt(f::playlist_meta(pl.track_count as usize, &span(&mut (0..n)), &pl.owner), FIG, p.text2).truncate()),
        );

    let all = picked.len() == n;
    let mut select_all = row()
        .key(keys::FOUND_SELECT_ALL)
        .on_press(actions::toggle_all_tracks)
        .h(28.0)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .role("button")
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .child(if pick_locked {
            icon(&icons::LOCK_ORBIT, 16.0).color(p.text2)
        } else {
            tick(p, all, true).on_press(actions::toggle_all_tracks)
        })
        .child(txt(
            if all { f::PLAYLIST_DESELECT_ALL } else { f::PLAYLIST_SELECT_ALL },
            SMALL_M,
            if pick_locked { p.text2 } else { p.text },
        ));
    // Picking open: the chip sits on the column heading's line. Picking locked: a line of its
    // own, with the sentence that says why.
    let mut tools = None;
    let mut chip_at_head = None;
    if pick_locked {
        select_all = select_all.help(f::LOCK_PICK_PLAYLIST);
        tools = Some(
            row().w_full().gap(16.0).center_y().child(select_all).child(txt(f::LOCK_PICK_PLAYLIST, SMALL, p.text2).truncate().shrink()),
        );
    } else {
        chip_at_head = Some(select_all.hover(p.hover));
    }
    let mut head = head;
    if let Some(chip) = chip_at_head {
        head = head.child(chip.mr(16.0));
    }
    let head = head.child(counter);

    let cells = |a: El, b: El, c: El, d: El, e: Option<El>, g: El| {
        let r = row().w_full().gap(16.0).center_y().child(a).child(b.w(24.0)).child(c.grow(2.0).min_w(0.0)).child(d.grow(1.0).min_w(0.0));
        match e {
            Some(e) => r.child(e),
            None => r,
        }
        .child(g.w(48.0))
    };
    let heading = |s: &str| txt(s.to_string(), SMALL, p.muted);
    let mut title_head = row().gap(8.0).child(heading(f::PLAYLIST_COL_TITLE));
    if !pick_locked {
        title_head = title_head.child(txt(f::PLAYLIST_COL_HINT, SMALL, p.muted).truncate().shrink());
    }
    let columns = cells(
        el().w(20.0),
        heading(f::PLAYLIST_COL_NUMBER),
        title_head,
        heading(f::PLAYLIST_COL_ARTIST),
        spotify.then(|| heading(copy::COL_MATCH).w(176.0)),
        heading(f::PLAYLIST_COL_TIME).align(rune::Align::End),
    )
    .h(32.0)
    .px(12.0)
    .border_sides([0.0, 0.0, 1.0, 0.0], p.line);

    let mut list = col().key("found-tracks").w_full().scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0);
    for (i, t) in pl.tracks.iter().enumerate() {
        let usable = matched[i] != Matched::NoMatch;
        let on = selected[i] && usable;
        let ink = if usable { p.text } else { p.muted };
        let time = format::duration(t.duration_ms / 1000.0);
        let mut r = cells(
            if pick_locked {
                el().size(20.0, 20.0).center().child(icon(&icons::LOCK_ORBIT, 16.0).color(p.muted))
            } else {
                tick(p, on, usable).on_press(move |s: &mut State| actions::toggle_track(s, i))
            },
            txt((i + 1).to_string(), FIG, p.muted),
            txt(t.name.clone(), BODY, ink).truncate(),
            txt(t.artist.clone(), BODY, if usable { p.text2 } else { p.muted }).truncate(),
            spotify.then(|| match_cell(p, matched[i])),
            txt(time.clone(), FIG, p.text2).align(rune::Align::End),
        )
        .key(keys::track(i))
        .on_press(move |s: &mut State| actions::toggle_track(s, i))
        .h(40.0)
        .px(12.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
        .role("checkbox")
        .checked(on)
        .label(f::playlist_row_aria(&t.name, &t.artist, &time, false));
        if pick_locked {
            r = r.help(f::LOCK_PICK_PLAYLIST);
        } else if usable {
            r = r.cursor(rune::Cursor::Pointer).hover(p.hover);
        }
        if on && !pick_locked {
            r = r.fill(mix(p.card, p.text, 0.035));
        }
        list = list.child(r);
    }

    let sum = if queued == 0 {
        f::PLAYLIST_NOTHING_SELECTED.to_string()
    } else {
        f::playlist_readout(queued, f::AUDIO_FORMATS[af], f::AUDIO_QUALITIES[aq], &picked_span)
    };
    let bar = row()
        .w_full()
        .gap(16.0)
        .items_end()
        .pt(16.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .child(
            select(p, f::PLAYLIST_FORMAT, f::AUDIO_FORMATS[af], None, |b| {
                let items = entries(&f::AUDIO_FORMATS, &AUDIO_FORMAT_VALUES, af);
                pick_list(b.key(keys::CHOICE_FORMAT), keys::CHOICE_FORMAT, items, AUDIO_FORMAT_VALUES[af], |s, id| {
                    set(s, |c| c.format = id.into())
                })
            })
            .label(f::PLAYLIST_FORMAT_ARIA),
        )
        .child(
            select(p, f::PLAYLIST_QUALITY, f::AUDIO_QUALITIES[aq], None, |b| {
                let items = entries(&f::AUDIO_QUALITIES, &AUDIO_QUALITY_VALUES, aq);
                pick_list(b.key(keys::CHOICE_AUDIO_QUALITY), keys::CHOICE_AUDIO_QUALITY, items, AUDIO_QUALITY_VALUES[aq], |s, id| {
                    set(s, |c| c.audio_quality = id.into())
                })
            })
            .label(f::PLAYLIST_QUALITY_ARIA),
        )
        // TODO(shared): a download's own folder (`Choice.folder`); `actions::choose_folder` sets the saved `outputDir`.
        .child(select(p, f::PLAYLIST_FOLDER, folder, Some(&icons::FOLDER), |b| b.key(keys::CHOICE_FOLDER)).help(folder))
        .child(spacer())
        .child(txt(sum, FIG, p.text2).mb(8.0))
        .child(
            button(p, keys::FOUND_DOWNLOAD, &f::playlist_download_label(picked_n, queued), Some(&icons::DOWNLOAD), Tone::Primary)
                .on_press(actions::download_found),
        );

    let mut c = col().w_full().grow(1.0).min_h(0.0).gap(16.0).child(head);
    if basic && picked_n > cap {
        // The cap strip: what Basic takes, what is left behind, and the way to the plans.
        let words = row()
            .grow(1.0)
            .min_w(0.0)
            .flex_wrap()
            .center_y()
            .child(txt(f::LIMIT_COLLECTION, SMALL, p.text))
            .child(txt(format!(" {}", f::LIMIT_SKIPPED_LEAD.trim_start()), SMALL, p.text2))
            .child(txt((picked_n - cap).to_string(), FIG, p.text2))
            .child(txt(f::LIMIT_SKIPPED_TRAIL, SMALL, p.text2));
        c = c.child(
            row()
                .key("found-cap")
                .w_full()
                .min_h(48.0)
                .pl(16.0)
                .pr(8.0)
                .py(8.0)
                .gap(12.0)
                .center_y()
                .radius(12.0)
                .fill(p.raised)
                .child(icon(&icons::LOCK_ORBIT, 16.0).color(p.text2))
                .child(words)
                .child(button(p, "found-cap-plans", f::SEE_PLANS, None, Tone::Ghost)),
        );
    }
    if let Some(t) = tools {
        c = c.child(t);
    }
    c.child(col().w_full().grow(1.0).min_h(0.0).child(columns).child(list)).child(bar)
}

// ── A search: one band per source ───────────────────────────────────────────

fn search(s: &State, p: &Pal, bands: &[Band; 3]) -> El {
    let sources: [(&str, &'static Icon, rune::Color); 3] = [
        (copy::SEARCH_BAND_YOUTUBE, &icons::YOUTUBE, p.youtube),
        (copy::SEARCH_BAND_SOUNDCLOUD, &icons::SOUNDCLOUD, p.soundcloud),
        (copy::SEARCH_BAND_SPOTIFY, &icons::SPOTIFY, p.spotify),
    ];
    let mut cols = row().w_full().gap(24.0).items_start();
    for (i, band) in bands.iter().enumerate() {
        cols = cols.child(band_view(s, p, i, sources[i], band));
    }
    if s.picked.is_empty() {
        return cols;
    }
    // Anything ticked: the bar that sends it, under the bands.
    let n = s.picked.len();
    let bar = row()
        .w_full()
        .gap(16.0)
        .center_y()
        .pt(16.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .child(txt(f::results_selected(n), FIG, p.text2))
        // TODO(shared): `actions::clear_picked(s)`.
        .child(button(p, keys::RESULTS_CLEAR, f::RESULTS_CLEAR, None, Tone::Ghost).on_press(|s: &mut State| s.picked.clear()))
        .child(spacer())
        .child(
            button(p, keys::FOUND_DOWNLOAD, &f::results_download_n(n), Some(&icons::DOWNLOAD), Tone::Primary)
                .on_press(actions::download_found),
        );
    col().w_full().gap(16.0).child(cols).child(bar)
}

fn band_view(s: &State, p: &Pal, i: usize, (name, glyph, color): (&str, &'static Icon, rune::Color), band: &Band) -> El {
    let mut head =
        row().w_full().h(28.0).gap(8.0).center_y().child(icon(glyph, 16.0).color(color)).child(txt(name.to_string(), BODY_M, p.text));
    match band {
        Band::Results(r) => head = head.child(txt(r.len().to_string(), FIG, p.muted)),
        Band::Loading => head = head.child(waiting(p)),
        _ => {}
    }
    if i == 2 {
        head = head.child(spacer()).child(txt(copy::SEARCH_SPOTIFY_NOTE, FIG, p.muted).truncate().shrink());
    }
    let body =
        match band {
            Band::Loading => (0..2).fold(col().w_full(), |c, k| {
                let w = if i == 0 { 64.0 } else { 40.0 };
                c.child(row().w_full().h(56.0).px(8.0).gap(12.0).center_y().child(bone(p, w, 36.0).radius(8.0)).child(
                    col().grow(1.0).min_w(0.0).gap(8.0).child(bone(p, [200.0, 160.0][k], 12.0)).child(bone(p, [112.0, 136.0][k], 12.0)),
                ))
            }),
            Band::Results(r) => {
                let mut c = r.iter().take(BAND_ROWS).fold(col().w_full(), |c, r| c.child(result_row(p, r, s.picked.contains(&r.id))));
                if r.len() > BAND_ROWS {
                    c = c.child(
                        row()
                            .key(keys::band_more(i))
                            .on_press(move |s: &mut State| actions::show_more(s, i))
                            .h(32.0)
                            .px(8.0)
                            .gap(8.0)
                            .center_y()
                            .radius(8.0)
                            .hover(p.hover)
                            .role("button")
                            .focusable(true)
                            .cursor(rune::Cursor::Pointer)
                            .child(icon(&icons::CARET_DOWN, 16.0).color(p.text2))
                            .child(txt(copy::search_show_more(r.len() - BAND_ROWS), SMALL_M, p.text2)),
                    );
                }
                c
            }
            Band::Empty => col().w_full().py(16.0).px(8.0).child(txt(copy::search_no_matches(name), SMALL, p.text2).wrap()),
            Band::Failed => row()
                .w_full()
                .py(12.0)
                .px(8.0)
                .gap(8.0)
                .center_y()
                .role("alert")
                .child(icon(&icons::ALERT, 16.0).color(p.error))
                .child(txt(copy::search_source_failed(name), SMALL, p.text).grow(1.0).min_w(0.0).truncate())
                .child(
                    button(p, &keys::band_retry(i), copy::SEARCH_RETRY, Some(&icons::RETRY), Tone::Ghost)
                        .on_press(move |s: &mut State| actions::retry_band(s, i)),
                ),
        };
    col().grow(1.0).min_w(0.0).gap(8.0).child(head.pb(8.0).border_sides([0.0, 0.0, 1.0, 0.0], p.line)).child(body)
}

/// A result: its tick, its art and words, and the download mark. A ticked row is tinted in ink.
fn result_row(p: &Pal, r: &SearchResult, on: bool) -> El {
    let (w, glyph) = if r.source == Src::Youtube { (64.0, &icons::VIDEO) } else { (36.0, &icons::MUSIC) };
    let duration = format::duration(r.duration);
    let subtitle = [r.uploader.as_str(), duration.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
    let mut second = row().gap(8.0).center_y();
    if !r.uploader.is_empty() {
        second = second.child(txt(r.uploader.clone(), SMALL, p.text2).truncate().shrink());
    }
    if !duration.is_empty() {
        second = second.child(txt(duration, FIG, p.muted));
    }
    let (row_id, tick_id) = (r.id.clone(), r.id.clone());
    let row_el = row()
        .key(keys::result(&r.id))
        .on_press(move |s: &mut State| actions::toggle_result(s, &row_id))
        .w_full()
        .h(56.0)
        .px(8.0)
        .gap(12.0)
        .center_y()
        .radius(8.0)
        .role("button")
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .label(copy::search_row_aria(&r.title, &subtitle));
    let row_el = if on { row_el.fill(mix(p.card, p.text, 0.06)) } else { row_el.hover(p.hover) };
    row_el
        .child(
            tick(p, on, true)
                .key(keys::result_tick(&r.id))
                .on_press(move |s: &mut State| actions::toggle_result(s, &tick_id))
                .label(f::results_pick_aria(&r.title)),
        )
        .child(art(p, r.source, w, 36.0, glyph))
        .child(col().grow(1.0).min_w(0.0).gap(4.0).child(txt(r.title.clone(), SMALL_M, p.text).truncate()).child(second))
        .child(icon(&icons::DOWNLOAD, 16.0).color(p.text2).help(copy::SEARCH_ROW_DOWNLOAD))
}
