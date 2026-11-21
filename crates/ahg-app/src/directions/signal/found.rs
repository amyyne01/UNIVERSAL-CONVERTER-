//! What the field found: the one surface between a paste (or a search) and a
//! running download, shared by every preset that wears Signal. It is a desk
//! module: a header strip that names what came in and how to put it away, a
//! body (a channel list for an album, three source strips for a search, one
//! source with its routing for a video), and an output strip with the one lit
//! control, Download. A lookup reads as the instrument warming up: the waiting
//! ring over the shape of what is coming. The presets only place it.
use ahg_engine::tier::{self, BASIC_LIMITS, Plan, is_premium_format, is_premium_video_quality};
use ahg_engine::types::{MediaMetadata, Playlist, SearchResult, SourcePlatform, VideoQuality};
use rune::{Color, Cursor, El, Icon, ScrollSpec, col, el, icon, row, spacer};

use super::parts::{Line, RING, button, config_text, ghost, icon_button, keycap, label, mono, ring_material, sans};
use super::theme::{Pal, source_color};
use crate::actions;
use crate::copy::{self, found as f};
use crate::format;
use crate::gate::{Band, Card, Choice, Found, Matched, State};
use crate::icons;
use crate::keys;
use crate::ring;

const QUALITIES: [VideoQuality; 6] =
    [VideoQuality::P360, VideoQuality::P480, VideoQuality::P720, VideoQuality::P1080, VideoQuality::P2160, VideoQuality::Best];

/// What the choice chips show lit: the audio format, the audio quality and the video quality, as
/// places in `f::AUDIO_FORMATS`, `f::AUDIO_QUALITIES` and `f::VIDEO_QUALITIES`. Basic shows what it
/// will really make.
fn lit(s: &State) -> (usize, usize, usize) {
    let c = &s.choice;
    let format = f::AUDIO_FORMATS.iter().position(|n| n.eq_ignore_ascii_case(&c.format)).unwrap_or(0);
    let audio =
        f::AUDIO_QUALITIES.iter().position(|n| n.split(' ').next().is_some_and(|w| w.eq_ignore_ascii_case(&c.audio_quality))).unwrap_or(3);
    let video = QUALITIES.iter().position(|q| *q == tier::clamp_video_quality(c.video_quality, s.plan)).unwrap_or(5);
    (format, audio, video)
}

/// The surface, or `None` when the field found nothing yet. It hugs its
/// content and shrinks into the room it is given, scrolling inside.
pub fn view(s: &State, p: &Pal) -> Option<El> {
    let (head, body, foot) = match &s.found {
        Found::Nothing => return None,
        Found::LookingUp => (
            head(s, glyph(&icons::LINK, p.muted), label(format::link_label(&s.input), mono(12.0, 400.0), p.text2), p),
            looking_up(s, p),
            None,
        ),
        Found::LookupFailed(why) => (
            head(s, glyph(&icons::ALERT, p.error), label(format::link_label(&s.input), mono(12.0, 400.0), p.text2), p),
            failed(why, &s.input, p),
            None,
        ),
        Found::Media(m) => media(s, m, p),
        Found::Playlist { playlist, matched, selected } => album(s, playlist, matched, selected, p),
        Found::Search { query, bands } => (search_head(s, query, bands, p), search(s, bands, p), picked_bar(s, p)),
    };
    let c = col()
        .key(keys::FOUND)
        .w_full()
        .shrink()
        .min_h(0.0)
        .radius(12.0)
        .fill(p.surface)
        .border(1.0, p.line_strong)
        .clip_rounded(12.0)
        .role("region")
        .child(head);
    let mut c = c.child(body.shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }));
    if let Some(foot) = foot {
        c = c.child(foot);
    }
    Some(c)
}

/// "Available with Premium": a lock in ink, never the accent, which is Download's.
fn lock(p: &Pal) -> El {
    glyph(&icons::LOCK_SIGNAL, p.text2)
}

/// An open door to Premium, never the accent.
// TODO(shared): `actions::see_plans(s)`; the plans sheet has no action yet, so the button waits.
fn see_plans(p: &Pal) -> El {
    button(None, f::SEE_PLANS, p.text, p).key(keys::FOUND_SEE_PLANS)
}

fn glyph(i: &'static Icon, c: Color) -> El {
    el().size(16.0, 16.0).child(icon(i, 16.0).color(c))
}

/// The header strip: what came in on the left, the way out on the right.
fn head(s: &State, lead: El, words: El, p: &Pal) -> El {
    head_with(s, lead, words, None, p)
}

/// `aside`: what sits before the way out (Basic's line on a video).
fn head_with(_s: &State, lead: El, words: El, aside: Option<El>, p: &Pal) -> El {
    let mut r = row()
        .w_full()
        .h(48.0)
        .pl(16.0)
        .pr(12.0)
        .gap(12.0)
        .center_y()
        .fill(p.deep.alpha(0.6))
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
        .child(lead)
        .child(words.single_line().truncate().min_w(0.0).shrink())
        .child(spacer());
    if let Some(aside) = aside {
        r = r.child(aside).child(el().w(1.0).h(20.0).fill(p.line).mx(4.0));
    }
    r.child(keycap(copy::KEYCAP_ESC, p))
        .child(icon_button(&icons::CLOSE, copy::NOTICE_DISMISS, p.text2, p).key(keys::FOUND_CLOSE).on_press(actions::dismiss_found))
}

/// A skeleton bone `at` points from the body's left edge: the shape of something still coming,
/// a graphite sheen crossing it.
fn bone(at: f32, w: Option<f32>, h: f32, p: &Pal) -> El {
    let b = el().h(h).radius(4.0).fill(p.raised).clip_rounded(4.0).child(sheen(at, p));
    match w {
        Some(w) => b.w(w),
        None => b.w_full(),
    }
}

/// Signal's shimmer: a narrow lit segment, like a meter's, in graphite.
fn sheen(at: f32, p: &Pal) -> El {
    let lit = if p.dark { p.text.alpha(0.08) } else { Color::WHITE.alpha(0.6) };
    crate::timeline::sheen(at, lit, 0.28)
}

/// The body's width last frame (the found surface less its border), for a bone set against
/// its right edge.
fn body_w() -> f32 {
    crate::timeline::found_w() - 2.0
}

/// The meter warming up: a run of lit segments, brightest at its head,
/// mid-sweep. A still picture reads as scanning, not as progress.
#[allow(dead_code)] // unused until cleanup (step 19)
fn sweep(n: usize, from: usize, len: usize, p: &Pal) -> El {
    let mut m = row().w_full().h(8.0).gap(2.0);
    for i in 0..n {
        let c = if i >= from && i < from + len { p.accent.alpha(0.2 + 0.8 * (i - from + 1) as f32 / len as f32) } else { p.unlit };
        m = m.child(el().grow(1.0).h(8.0).radius(1.0).fill(c));
    }
    m
}

/// `url`'s picture, over the look's placeholder (the source's tint and mark) until it is here.
fn cover(s: &State, url: &str, source: SourcePlatform, w: f32, h: f32, p: &Pal) -> El {
    let c = source_color(p, source);
    let placeholder =
        el().size(w, h).radius(4.0).fill(c.alpha(0.14)).center().child(icon(super::parts::platform_icon(source), 16.0).color(c));
    crate::covers::over(s, placeholder, url, 4.0)
}

/// The one accent control on the surface.
fn primary(words: &str, p: &Pal) -> El {
    row()
        .h(40.0)
        .pl(16.0)
        .pr(16.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.accent)
        .hover(p.deep.alpha(0.12))
        .press(p.deep.alpha(0.2))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .role("button")
        .child(icon(&icons::DOWNLOAD, 16.0).color(p.deep))
        .child(label(words, sans(14.0, 500.0), p.deep))
        .key(keys::FOUND_DOWNLOAD)
        .on_press(actions::download_found)
}

/// The output strip under the body.
fn strip(left: El, right: El, p: &Pal) -> El {
    row()
        .w_full()
        .min_h(64.0)
        .px(24.0)
        .py(12.0)
        .gap(16.0)
        .center_y()
        .fill(p.deep.alpha(0.6))
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .child(left.grow(1.0).min_w(0.0))
        .child(right)
}

// ── Looking up: the shape of a single video, still empty ───────────────────

fn looking_up(s: &State, p: &Pal) -> El {
    // TODO(motion): the real reduced-motion flag (step 52).
    // The waiting ring sits in the cover's box, where the picture will land (jury L-2).
    let waiting = ring::waiting("ring-waiting", RING, &ring_material(s, p, p.accent), false, 0);
    let (pad, cover_w, gap) = (24.0, 192.0, 24.0);
    let info = pad + cover_w + gap;
    let cover = bone(pad, Some(cover_w), 108.0, p).key(keys::FOUND_COVER).center().child(waiting);
    let card = row().w_full().gap(gap).child(cover).child(
        col()
            .grow(1.0)
            .min_w(0.0)
            .gap(12.0)
            .pt(4.0)
            // What is happening stands where the title will, not on a line of its own above.
            .child(label(f::NOTE_READING, mono(12.0, 400.0), p.text2))
            .child(bone(info, Some(440.0), 16.0, p))
            .child(bone(info, Some(320.0), 16.0, p))
            .child(bone(info, Some(160.0), 12.0, p))
            .child(bone(info, Some(120.0), 12.0, p).mt(8.0)),
    );
    // The well's own edge and padding: 1 + 16.
    let inner = pad + 17.0;
    let mut well =
        col().w_full().padding(16.0).gap(16.0).radius(8.0).fill(p.deep).border(1.0, p.line).child(bone(inner, Some(176.0), 28.0, p));
    for widths in [[56.0, 56.0, 56.0, 56.0, 56.0, 64.0], [72.0, 72.0, 72.0, 72.0, 72.0, 0.0]] {
        let mut r = row().w_full().gap(8.0).center_y().child(bone(inner, Some(96.0), 12.0, p).mr(24.0));
        let mut at = inner + 96.0 + 24.0 + 8.0;
        for w in widths.into_iter().filter(|w| *w > 0.0) {
            r = r.child(bone(at, Some(w), 28.0, p));
            at += w + 8.0;
        }
        well = well.child(r);
    }
    col().w_full().padding(pad).gap(24.0).role("status").label(f::NOTE_READING).child(card).child(well).child(
        row().w_full().center_y().child(bone(pad, Some(200.0), 12.0, p)).child(spacer()).child(bone(
            body_w() - pad - 128.0,
            Some(128.0),
            40.0,
            p,
        )),
    )
}

// ── The lookup failed ───────────────────────────────────────────────────────

fn failed(why: &str, input: &str, p: &Pal) -> El {
    let panel = col()
        .w_full()
        .padding(16.0)
        .gap(12.0)
        .radius(8.0)
        .fill(p.error_soft)
        .border(1.0, p.error.alpha(0.35))
        .role("alert")
        .child(
            row()
                .w_full()
                .gap(12.0)
                .items_start()
                .child(glyph(&icons::ALERT, p.error).mt(2.0))
                .child(label(why, sans(14.0, 400.0), p.text).wrap().grow(1.0).min_w(0.0)),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .pl(28.0)
                .center_y()
                .child(label(input, mono(12.0, 400.0), p.muted).single_line().truncate().min_w(0.0).shrink())
                .child(spacer())
                .child(button(Some(&icons::RETRY), f::ERROR_TRY_AGAIN, p.text, p).key(keys::FOUND_RETRY).on_press_tx(actions::submit)),
        );
    // No ring after a dead lookup: a dark one would read as still waiting.
    col().w_full().padding(24.0).gap(20.0).child(panel)
}

// ── One video: the source, its routing, the output ──────────────────────────

fn source_of(m: &MediaMetadata) -> SourcePlatform {
    match m.extractor.as_deref().unwrap_or("") {
        "youtube" => SourcePlatform::Youtube,
        "soundcloud" => SourcePlatform::Soundcloud,
        "spotify" => SourcePlatform::Spotify,
        "instagram" | "Instagram" => SourcePlatform::Instagram,
        "tiktok" | "TikTok" => SourcePlatform::Tiktok,
        "facebook" | "Facebook" => SourcePlatform::Facebook,
        _ => SourcePlatform::Generic,
    }
}

/// A row of choices in a recessed well; the chosen one is raised with a
/// segment under it (in ink, not the accent: the accent is for Download).
/// `locked(i)`: the choice needs Premium. It stays pressable, so choosing it
/// can say what the plan adds; it carries a lock before its name.
/// `key(i)`: a choice's own key (the format chips); `None` keys the group instead.
/// `pick`: what choosing the i-th changes in the choice, which `actions::set_choice` then takes.
fn choices(
    items: &[&str],
    on: usize,
    tag: impl Fn(&str) -> bool,
    locked: impl Fn(usize) -> bool,
    key: impl Fn(usize) -> Option<String>,
    pick: fn(&mut Choice, usize),
    p: &Pal,
) -> El {
    let mut g = row().padding(4.0).gap(4.0).radius(8.0).fill(p.deep).border(1.0, p.line).role("radiogroup");
    for (i, name) in items.iter().enumerate() {
        let lit = i == on;
        let mut c = row()
            .h(28.0)
            .px(12.0)
            .gap(8.0)
            .center_y()
            .radius(6.0)
            .hover(p.hover)
            .cursor(Cursor::Pointer)
            .role("radio")
            .checked(lit)
            .on_press(move |s: &mut State| choose(s, |c| pick(c, i)));
        if let Some(k) = key(i) {
            c = c.key(k);
        }
        if locked(i) {
            c = c.pl(8.0).label(f::lock_chip_aria(name)).help(f::lock_chip_title(name)).child(lock(p));
        }
        c = c.child(label(*name, sans(13.0, if lit { 500.0 } else { 400.0 }), if lit { p.text } else { p.text2 }));
        if tag(name) {
            c = c.child(label(f::TAG_LOSSLESS, mono(12.0, 400.0), p.muted));
        }
        if lit {
            c = c.fill(p.raised).child(el().size(12.0, 2.0).radius(1.0).fill(p.text).absolute(12.0, 24.0));
        }
        g = g.child(c);
    }
    g
}

/// The found step's choice with one change, through the action that saves and checks it.
// TODO(shared): `actions::set_choice_with(s, |c| ..)`, or one action per chip (format, quality, video or audio).
fn choose(s: &mut State, change: impl FnOnce(&mut Choice)) {
    let mut c = s.choice.clone();
    change(&mut c);
    actions::set_choice(s, c);
}

fn media(s: &State, m: &MediaMetadata, p: &Pal) -> (El, El, Option<El>) {
    let source = source_of(m);
    let basic = s.plan == Plan::Basic;
    let video = s.choice.video;
    let (format_on, audio_on, video_on) = lit(s);
    let short = match &s.card {
        Card::Short { label } => Some(label.as_str()),
        Card::Single { .. } => None,
    };
    let eyebrow = short.map_or_else(|| f::single_eyebrow_fallback(format::source_name(source), !video), str::to_owned);
    let readout = if video {
        f::readout_video(f::VIDEO_QUALITIES[video_on])
    } else {
        f::readout_audio(f::AUDIO_FORMATS[format_on], f::AUDIO_QUALITIES[audio_on])
    };
    // On Basic the strip says what Basic includes here, with the way to lift it.
    let aside = basic.then(|| {
        row()
            .gap(8.0)
            .center_y()
            .child(lock(p))
            .child(label(if video { f::BASIC_VIDEO } else { f::BASIC_AUDIO }, sans(13.0, 400.0), p.text2))
            .child(see_plans(p).h(28.0))
    });
    let h = head_with(
        s,
        glyph(super::parts::platform_icon(source), source_color(p, source)),
        label(eyebrow, mono(12.0, 400.0), source_color(p, source)),
        aside,
        p,
    );

    let mut facts = row().gap(8.0).center_y();
    let length = format::duration(m.duration);
    if !length.is_empty() {
        facts = facts.child(glyph(&icons::CLOCK, p.muted)).child(label(length, mono(12.0, 400.0), p.text2)).child(label(
            "·",
            mono(12.0, 400.0),
            p.muted,
        ));
    }
    facts = facts.child(label(readout.clone(), mono(12.0, 400.0), p.text2));
    let mut info = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(8.0)
        .child(label(m.title.clone(), sans(16.0, 500.0), p.text).wrap().max_lines(2))
        .child(label(m.uploader.clone(), sans(13.0, 400.0), p.text2).single_line().truncate())
        .child(facts.mt(4.0));
    if let Card::Single { matched: Some(name) } = &s.card {
        // A Spotify track is matched by search: the name it will look for, said plainly.
        info = info.child(
            Line::new()
                .add(f::MATCHED_LEAD, p.text2)
                .fig(name, p.text)
                .add(f::MATCHED_TRAIL, p.text2)
                .wrap(sans(13.0, 400.0).lh(20.0))
                .mt(4.0),
        );
    }
    let top = match short {
        Some(_) => poster(s, m, source, p),
        None => row()
            .w_full()
            .gap(24.0)
            .items_start()
            .child(cover(s, &m.thumbnail_url, source, 192.0, 108.0, p).key(keys::FOUND_COVER))
            .child(info),
    };

    // The routing: Video or Audio, then the channels each one uses. The ones
    // the current choice does not use sit dark, one press away.
    let capsule = {
        let side = |i: &'static Icon, words: &str, on: bool, video: bool| {
            let mut c = row()
                .h(28.0)
                .px(12.0)
                .gap(8.0)
                .center_y()
                .radius(6.0)
                .hover(p.hover)
                .cursor(Cursor::Pointer)
                .role("radio")
                .checked(on)
                .on_press(move |s: &mut State| choose(s, |c| c.video = video))
                .child(icon(i, 16.0).color(if on { p.text } else { p.muted }))
                .child(label(words, sans(13.0, if on { 500.0 } else { 400.0 }), if on { p.text } else { p.text2 }));
            if on {
                c = c.fill(p.raised).child(el().size(12.0, 2.0).radius(1.0).fill(p.text).absolute(12.0, 24.0));
            }
            c
        };
        row()
            .padding(4.0)
            .gap(4.0)
            .radius(8.0)
            .fill(p.deep)
            .border(1.0, p.line)
            .role("radiogroup")
            .child(side(&icons::VIDEO, f::MEDIA_VIDEO, video, true).key(keys::CHOICE_VIDEO))
            .child(side(&icons::TRACK, f::MEDIA_AUDIO, !video, false).key(keys::CHOICE_AUDIO))
    };
    let named = |name: &str, control: El, routed: bool| {
        let r = row().w_full().gap(16.0).center_y().child(label(name, sans(13.0, 400.0), p.text2).w(112.0)).child(control);
        if routed { r } else { r.opacity(0.4) }
    };
    let lossless = |n: &str| matches!(n, "FLAC" | "WAV" | "ALAC");
    let mut well = col()
        .w_full()
        .padding(16.0)
        .gap(12.0)
        .radius(8.0)
        .fill(p.bg)
        .border(1.0, p.line)
        .child(named("", capsule, true))
        .child(named(
            f::VIDEO_QUALITY_LABEL,
            choices(
                &f::VIDEO_QUALITIES,
                video_on,
                |_| false,
                |i| basic && is_premium_video_quality(QUALITIES[i]),
                |_| None,
                |c, i| c.video_quality = QUALITIES[i],
                p,
            )
            .key(keys::CHOICE_VIDEO_QUALITY),
            video,
        ))
        .child(named(
            f::PLAYLIST_FORMAT,
            choices(
                &f::AUDIO_FORMATS,
                format_on,
                lossless,
                |i| basic && is_premium_format(&f::AUDIO_FORMATS[i].to_lowercase()),
                |i| Some(keys::choice_format(&f::AUDIO_FORMATS[i].to_lowercase())),
                |c, i| c.format = f::AUDIO_FORMATS[i].to_lowercase(),
                p,
            ),
            !video,
        ))
        .child(named(
            f::AUDIO_QUALITY_LABEL,
            choices(
                &f::AUDIO_QUALITIES,
                audio_on,
                |_| false,
                |i| basic && i == f::AUDIO_QUALITIES.len() - 1,
                |_| None,
                |c, i| c.audio_quality = f::AUDIO_QUALITIES[i].split(' ').next().unwrap_or_default().to_lowercase(),
                p,
            )
            .key(keys::CHOICE_AUDIO_QUALITY),
            !video,
        ));
    if basic {
        // What the locks mean: the old dropdowns' title, on the routing as a whole.
        well = well.help(f::LOCK_SELECT_TITLE);
    }
    let body = col().w_full().padding(24.0).gap(24.0).child(top).child(well);
    let out = Line::new().add(f::READOUT_LEAD, p.text2).fig(&readout, p.text).one(sans(13.0, 400.0));
    let go = if short.is_some() { f::SHORT_DOWNLOAD } else { f::SINGLE_DOWNLOAD };
    (h, body, Some(strip(out, primary(go, p), p)))
}

/// A Reels, TikTok or Facebook clip: its own 9:16 shape as a poster, the facts beside it.
fn poster(s: &State, m: &MediaMetadata, source: SourcePlatform, p: &Pal) -> El {
    let tint = source_color(p, source);
    let length = format::duration(m.duration);
    let frame = col()
        .size(152.0, 270.0)
        .radius(8.0)
        .fill(tint.alpha(0.14))
        .border(1.0, p.line)
        .center()
        .child(icon(super::parts::platform_icon(source), 28.0).color(tint));
    let mut frame = crate::covers::over(s, frame, &m.thumbnail_url, 8.0);
    if !length.is_empty() {
        frame =
            frame.child(el().h(20.0).px(6.0).center().radius(4.0).fill(p.media_scrim).pin(None, None, Some(8.0), Some(8.0)).child(label(
                length.clone(),
                mono(12.0, 500.0),
                p.on_media,
            )));
    }
    let mut facts = Line::new();
    let size = match (m.width, m.height) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => f::short_size(w as u32, h as u32),
        _ => String::new(),
    };
    // TODO(shared): `format::count` (1.2K, 4.8M) for `f::short_views` / `f::short_likes`, and the short upload date.
    for (i, part) in [length, size].into_iter().filter(|x| !x.is_empty()).enumerate() {
        if i > 0 {
            facts = facts.add(&copy::row_dot_part(""), p.muted);
        }
        facts = facts.fig(&part, p.text2);
    }
    let mut info = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(8.0)
        .child(label(m.title.clone(), sans(20.0, 500.0).lh(26.0), p.text).wrap().max_lines(2))
        .child(label(m.uploader.clone(), sans(13.0, 400.0), p.text2).single_line().truncate());
    if let Some(caption) = m.description.as_ref().filter(|d| !d.trim().is_empty() && **d != m.title) {
        info = info.child(label(caption.clone(), sans(13.0, 400.0).lh(20.0), p.text2).wrap().max_lines(3).mt(4.0));
    }
    let info = info.child(facts.one(mono(12.0, 400.0)).mt(4.0));
    row().w_full().gap(24.0).items_start().child(frame).child(info)
}

// ── An album: a channel list ────────────────────────────────────────────────

/// Signal strength, four bars: how sure the match is. Read by count, not colour.
fn bars(lit: usize, c: Color, p: &Pal) -> El {
    let mut b = row().w(16.0).h(16.0).gap(2.0).items_end().pb(1.0);
    for (i, h) in [5.0, 8.0, 11.0, 14.0].into_iter().enumerate() {
        b = b.child(el().w(2.0).h(h).radius(1.0).fill(if i < lit { c } else { p.line_strong }));
    }
    b
}

fn match_cell(m: Matched, p: &Pal) -> El {
    let (lead, words, ink, extra) = match m {
        Matched::Confirmed => (bars(4, p.text, p), copy::MATCH_CONFIRMED, p.text2, None),
        Matched::BestGuess(c) => {
            (bars(3, p.warning, p), copy::MATCH_BEST_GUESS, p.warning, Some(format!("{}%", format::round_half_up(c * 100.0))))
        }
        Matched::Matching => (glyph(&icons::LOADING, p.muted), copy::MATCH_MATCHING, p.muted, None),
        Matched::NoMatch => (glyph(&icons::ALERT, p.error), copy::MATCH_NONE, p.error, None),
    };
    let mut r = row().w(176.0).gap(8.0).center_y().child(lead).child(label(words, sans(13.0, 400.0), ink));
    if let Some(x) = extra {
        r = r.child(label(x, mono(12.0, 400.0), p.muted));
    }
    // Still matching: the cell shimmers like a bone.
    // Its column: the row's inset, the length cell and the gap after it, from the right.
    if m == Matched::Matching { r.clip_rounded(4.0).child(sheen(body_w() - 24.0 - 48.0 - 16.0 - 176.0, p)) } else { r }
}

fn tick(on: bool, can: bool, p: &Pal) -> El {
    let b = el().size(20.0, 20.0).radius(4.0).center().role("checkbox").checked(on);
    if !can {
        b.border(1.0, p.line_strong).border_dash(2.0, 2.0)
    } else if on {
        b.fill(p.text).child(icon(&icons::CHECK_SIGNAL, 16.0).color(p.surface))
    } else {
        b.border(1.0, p.line_strong).hover(p.hover)
    }
}

/// A tick that Basic holds: on, in a quieter ink, with no hover.
fn tick_fixed(on: bool, p: &Pal) -> El {
    let b = el().size(20.0, 20.0).radius(4.0).center().role("checkbox").checked(on).border(1.0, p.line_strong);
    if on { b.fill(p.raised).child(icon(&icons::CHECK_SIGNAL, 16.0).color(p.muted)) } else { b }
}

fn album(s: &State, pl: &Playlist, matched: &[Matched], selected: &[bool], p: &Pal) -> (El, El, Option<El>) {
    // A Spotify album or playlist matches each track; a YouTube playlist is one job.
    let spotify = pl.url.contains("spotify");
    let source = if spotify { SourcePlatform::Spotify } else { SourcePlatform::Youtube };
    let tint = source_color(p, source);
    let eyebrow = if !spotify {
        format::source_name(source)
    } else if pl.url.contains("/album/") {
        f::PLAYLIST_EYEBROW_ALBUM
    } else {
        f::PLAYLIST_EYEBROW_PLAYLIST
    };
    let h = head(s, glyph(super::parts::platform_icon(source), tint), label(eyebrow, mono(12.0, 400.0), tint), p);
    let basic = s.plan == Plan::Basic;
    // On Basic a YouTube playlist is taken from the top: its rows cannot be picked.
    let fixed = basic && !spotify;

    let tracks = &pl.tracks;
    let picked: Vec<usize> = (0..tracks.len()).filter(|i| selected.get(*i).copied().unwrap_or(false)).collect();
    let span_of = |rows: &mut dyn Iterator<Item = usize>| f::playlist_span(rows.map(|i| tracks[i].duration_ms).sum());
    let runtime = span_of(&mut (0..tracks.len()));
    let picked_span = span_of(&mut picked.iter().copied());
    // A YouTube playlist ticked through counts every video, listed or not.
    let whole = !spotify && picked.len() == tracks.len();
    let (picked_n, total_n) = if whole { (pl.track_count as usize, pl.track_count as usize) } else { (picked.len(), tracks.len()) };
    let matched_n = picked.iter().filter(|i| matched.get(**i) != Some(&Matched::NoMatch)).count() + picked_n - picked.len();
    let cap = BASIC_LIMITS.max_collection_tracks as usize;
    let queued = if basic { matched_n.min(cap) } else { matched_n };

    let mut counter = col()
        .items_end()
        .gap(4.0)
        .child(
            row()
                .items_end()
                .gap(4.0)
                .child(label(picked_n.to_string(), mono(32.0, 500.0).lh(32.0), p.text))
                .child(label(f::playlist_selected_total(total_n), mono(16.0, 400.0), p.muted).pb(2.0)),
        )
        .child(label(
            if picked.is_empty() { f::PLAYLIST_NONE_SELECTED.to_string() } else { f::playlist_selected_span(&picked_span) },
            mono(12.0, 400.0),
            p.muted,
        ));
    if basic && picked_n <= cap {
        counter = counter.child(label(f::BASIC_COLLECTION, sans(12.0, 400.0), p.muted));
    }
    let all = picked.len() == tracks.len();
    let selectors = if fixed {
        row()
            .h(32.0)
            .pl(12.0)
            .gap(8.0)
            .center_y()
            .child(lock(p))
            .child(label(f::LOCK_PICK_PLAYLIST, sans(13.0, 400.0), p.text2).single_line().truncate().min_w(0.0))
    } else {
        row()
            .gap(4.0)
            .child(
                ghost(Some(&icons::CHECK_SIGNAL), f::PLAYLIST_SELECT_ALL, if all { p.muted } else { p.text }, p)
                    .key(keys::FOUND_SELECT_ALL)
                    .on_press(actions::toggle_all_tracks),
            )
            // TODO(shared): `actions::deselect_all_tracks(s)`; `toggle_all_tracks` would select all when only some are ticked.
            .child(ghost(None, f::PLAYLIST_DESELECT_ALL, if picked.is_empty() { p.muted } else { p.text2 }, p))
    };
    let top = row()
        .w_full()
        .px(24.0)
        .py(16.0)
        .gap(16.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
        .child(cover(s, &pl.thumbnail_url, source, 64.0, 64.0, p))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(8.0)
                .child(label(pl.name.clone(), sans(16.0, 500.0), p.text).single_line().truncate())
                .child(label(f::playlist_meta(pl.track_count as usize, &runtime, &pl.owner), mono(12.0, 400.0), p.muted))
                .child(selectors.ml(-12.0)),
        )
        .child(counter);

    let cols = |a: El, b: El, c: El, d: El, e: El, t: El| {
        row()
            .w_full()
            .px(24.0)
            .gap(16.0)
            .center_y()
            .child(a.w(20.0))
            .child(b.w(24.0))
            .child(t.grow(1.0).min_w(0.0))
            .child(c.w(160.0))
            .child(d)
            .child(e.w(48.0))
    };
    let hdr = |w: &str| label(w, sans(12.0, 500.0), p.muted);
    let header =
        cols(
            el(),
            hdr(f::PLAYLIST_COL_NUMBER),
            hdr(f::PLAYLIST_COL_ARTIST),
            if spotify { el().w(176.0) } else { el() },
            row().justify_end().child(hdr(f::PLAYLIST_COL_TIME)),
            row().gap(12.0).center_y().child(hdr(f::PLAYLIST_COL_TITLE)).child(
                label(if fixed { "" } else { f::PLAYLIST_COL_HINT }, sans(12.0, 400.0), p.muted.alpha(0.7)).single_line().truncate(),
            ),
        )
        .h(32.0)
        .fill(p.deep.alpha(0.6))
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line);

    let mut list = col().w_full().role("list").label(pl.name.clone());
    for (i, t) in tracks.iter().enumerate() {
        let m = matched.get(i).copied().unwrap_or(Matched::Matching);
        let can = m != Matched::NoMatch;
        let on = can && selected.get(i).copied().unwrap_or(false);
        let ink = if can { p.text } else { p.muted };
        let mut r = cols(
            if fixed { tick_fixed(on, p) } else { tick(on, can, p) },
            label(format!("{:02}", i + 1), mono(12.0, 400.0), p.muted),
            label(t.artist.clone(), sans(13.0, 400.0), if can { p.text2 } else { p.muted }).single_line().truncate(),
            if spotify { match_cell(m, p) } else { el() },
            row().justify_end().child(label(
                format::duration(t.duration_ms / 1000.0),
                mono(12.0, 400.0),
                if can { p.text2 } else { p.muted },
            )),
            label(t.name.clone(), sans(13.0, if on { 500.0 } else { 400.0 }), ink).single_line().truncate(),
        )
        .key(keys::track(i))
        .h(40.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
        .role("checkbox")
        .checked(on)
        .label(f::playlist_row_aria(&t.name, &t.artist, &format::duration(t.duration_ms / 1000.0), false));
        r = r.on_press(move |s: &mut State| actions::toggle_track(s, i));
        if can && !fixed {
            r = r.cursor(Cursor::Pointer).hover(p.hover);
        }
        if on && !fixed {
            r = r.fill(p.raised.alpha(0.5));
        }
        list = list.child(r);
    }
    let mut body = col().w_full().child(top);
    if basic && picked_n > cap {
        // The cap strip: what Basic will take, what it leaves, the way to take it all.
        let skipped = Line::new()
            .add(f::LIMIT_SKIPPED_LEAD, p.text2)
            .fig(&(picked_n - cap).to_string(), p.text2)
            .add(f::LIMIT_SKIPPED_TRAIL, p.text2)
            .wrap(sans(13.0, 400.0));
        let words = col().gap(4.0).child(label(f::LIMIT_COLLECTION, sans(13.0, 500.0), p.text).wrap()).child(skipped);
        body = body.child(
            row()
                .w_full()
                .px(24.0)
                .py(12.0)
                .gap(12.0)
                .center_y()
                .fill(p.raised)
                .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
                .child(lock(p))
                .child(words.grow(1.0).min_w(0.0))
                .child(see_plans(p)),
        );
    }
    let body = body.child(header).child(list);

    // The output strip: format, quality and folder for the whole batch, then Download.
    let pick = |name: &str, value: El, w: f32| {
        col().gap(4.0).child(label(name, sans(12.0, 400.0), p.muted)).child(
            row()
                .w(w)
                .h(32.0)
                .px(12.0)
                .gap(8.0)
                .center_y()
                .radius(6.0)
                .fill(p.surface)
                .border(1.0, p.line_strong)
                .hover(p.hover)
                .cursor(Cursor::Pointer)
                .role("button")
                .child(value.grow(1.0).min_w(0.0))
                .child(icon(&icons::CARET_DOWN, 16.0).color(p.muted)),
        )
    };
    let (format_on, audio_on, _) = lit(s);
    let folder = s.choice.folder.clone().unwrap_or_else(|| config_text(s, "outputDir"));
    let readout = if queued == 0 {
        f::PLAYLIST_NOTHING_SELECTED.to_string()
    } else {
        f::playlist_readout(queued, f::AUDIO_FORMATS[format_on], f::AUDIO_QUALITIES[audio_on], &picked_span)
    };
    let left = row()
        .gap(12.0)
        .items_end()
        .child(pick(f::PLAYLIST_FORMAT, label(f::AUDIO_FORMATS[format_on], mono(13.0, 400.0), p.text), 96.0).label(f::PLAYLIST_FORMAT_ARIA))
        .child(
            pick(f::PLAYLIST_QUALITY, label(f::AUDIO_QUALITIES[audio_on], mono(13.0, 400.0), p.text), 120.0)
                .label(f::PLAYLIST_QUALITY_ARIA),
        )
        .child(
            pick(
                f::PLAYLIST_FOLDER,
                row()
                    .gap(8.0)
                    .center_y()
                    .child(glyph(&icons::FOLDER, p.muted))
                    .child(label(folder.clone(), mono(13.0, 400.0), p.text).single_line().truncate().min_w(0.0)),
                208.0,
            )
            .key(keys::CHOICE_FOLDER)
            .help(folder.clone()),
        )
        .child(label(readout, mono(12.0, 400.0), p.text2).single_line().truncate().min_w(0.0).shrink().pb(8.0));
    (h, body, Some(strip(left, primary(&f::playlist_download_label(matched_n, queued), p), p)))
}

// ── A search: three source strips side by side ─────────────────────────────

const SOURCES: [(SourcePlatform, &str); 3] = [
    (SourcePlatform::Youtube, copy::SEARCH_BAND_YOUTUBE),
    (SourcePlatform::Soundcloud, copy::SEARCH_BAND_SOUNDCLOUD),
    (SourcePlatform::Spotify, copy::SEARCH_BAND_SPOTIFY),
];
/// Rows a band shows before `Show n more`.
const SHOWN: usize = 5;

fn search_head(s: &State, query: &str, bands: &[Band; 3], p: &Pal) -> El {
    let words = if bands.iter().any(|b| matches!(b, Band::Loading)) {
        Line::new().strong(query, p.text).add("  ", p.muted).add(copy::STATUS_SEARCHING, p.muted).one(sans(13.0, 400.0))
    } else {
        label(query, sans(13.0, 500.0), p.text)
    };
    head(s, glyph(&icons::SEARCH, p.muted), words, p).label(copy::search_region_aria(query))
}

fn search(s: &State, bands: &[Band; 3], p: &Pal) -> El {
    let mut strips = row().w_full().items_stretch();
    for (i, ((source, name), band)) in SOURCES.iter().zip(bands).enumerate() {
        let mut c =
            col().grow(1.0).min_w(0.0).pb(8.0).child(band_head(*source, name, band, p)).child(band_body(s, i, *source, name, band, p));
        if i < 2 {
            c = c.border_sides([0.0, 1.0, 0.0, 0.0], p.line);
        }
        strips = strips.child(c);
    }
    strips
}

fn band_head(source: SourcePlatform, name: &str, band: &Band, p: &Pal) -> El {
    let mut r = row()
        .w_full()
        .h(40.0)
        .px(16.0)
        .gap(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
        .child(icon(super::parts::platform_icon(source), 16.0).color(source_color(p, source)))
        .child(label(name, sans(13.0, 500.0), p.text));
    r = match band {
        Band::Results(rs) => r.child(label(rs.len().to_string(), mono(12.0, 400.0), p.muted)),
        Band::Loading => r.child(spacer()).child(glyph(&icons::LOADING, p.muted)),
        _ => r,
    };
    r
}

fn band_body(s: &State, at: usize, source: SourcePlatform, name: &str, band: &Band, p: &Pal) -> El {
    let c = col().w_full().pt(4.0);
    match band {
        Band::Loading => {
            let mut c = c.role("status");
            // The three strips share the body: this one starts a third along per place.
            let x0 = at as f32 * body_w() / 3.0;
            let x1 = x0 + 16.0 + art_w(source) + 12.0;
            for w in [[168.0, 104.0], [136.0, 88.0]] {
                c = c.child(
                    row()
                        .w_full()
                        .h(56.0)
                        .px(16.0)
                        .gap(12.0)
                        .center_y()
                        .child(bone(x0 + 16.0, Some(art_w(source)), 36.0, p))
                        .child(col().gap(8.0).child(bone(x1, Some(w[0]), 12.0, p)).child(bone(x1, Some(w[1]), 8.0, p))),
                );
            }
            c
        }
        Band::Empty => c.padding(16.0).child(label(copy::search_no_matches(name), sans(13.0, 400.0), p.text2)),
        Band::Failed => c
            .padding(16.0)
            .gap(12.0)
            .role("alert")
            .child(row().gap(8.0).center_y().child(glyph(&icons::ALERT, p.error)).child(label(
                copy::search_source_failed(name),
                sans(13.0, 400.0),
                p.text,
            )))
            .child(
                row().ml(-12.0).child(
                    ghost(Some(&icons::RETRY), copy::SEARCH_RETRY, p.text, p)
                        .key(keys::band_retry(at))
                        .on_press(move |s: &mut State| actions::retry_band(s, at)),
                ),
            ),
        Band::Results(rs) => {
            let mut c = c;
            let open = s.expanded.get(at).copied().unwrap_or(false);
            for r in rs.iter().take(if open { rs.len() } else { SHOWN }) {
                c = c.child(result_row(s, r, s.picked.contains(&r.id), p));
            }
            if rs.len() > SHOWN && !open {
                c = c.child(
                    row().px(4.0).child(
                        ghost(Some(&icons::CARET_DOWN), &copy::search_show_more(rs.len() - SHOWN), p.text2, p)
                            .key(keys::band_more(at))
                            .on_press(move |s: &mut State| actions::show_more(s, at)),
                    ),
                );
            }
            if source == SourcePlatform::Spotify {
                c = c.child(label(copy::SEARCH_SPOTIFY_NOTE, mono(12.0, 400.0), p.muted).px(16.0).pt(8.0));
            }
            c
        }
    }
}

/// The ticked results: how many, the way to let go, and Download for all of them.
fn picked_bar(s: &State, p: &Pal) -> Option<El> {
    let n = s.picked.len();
    if n == 0 {
        return None;
    }
    // TODO(shared): `actions::clear_results(s)`; the ticks are let go here.
    let left = row()
        .gap(12.0)
        .center_y()
        .child(label(f::results_selected(n), mono(13.0, 500.0), p.text))
        .child(ghost(None, f::RESULTS_CLEAR, p.text2, p).key(keys::RESULTS_CLEAR).on_press(|s: &mut State| s.picked.clear()));
    Some(strip(left, primary(&f::results_download_n(n), p), p))
}

/// YouTube's pictures are 16:9; SoundCloud's and Spotify's are square.
fn art_w(source: SourcePlatform) -> f32 {
    if source == SourcePlatform::Youtube { 64.0 } else { 36.0 }
}

/// A result: its tick, its picture, its two lines. A ticked row sits on the raised surface.
fn result_row(s: &State, r: &SearchResult, on: bool, p: &Pal) -> El {
    let length = format::duration(r.duration);
    let mut sub = Line::new().add(&r.uploader, p.text2);
    if !length.is_empty() {
        sub = sub.add(" · ", p.muted).fig(&length, p.muted);
    }
    let id = r.id.clone();
    let tick = el()
        .key(keys::result_tick(&r.id))
        .size(16.0, 16.0)
        .radius(4.0)
        .center()
        .role("checkbox")
        .checked(on)
        .label(f::results_pick_aria(&r.title));
    let tick =
        if on { tick.fill(p.text).child(icon(&icons::CHECK_SIGNAL, 12.0).color(p.surface)) } else { tick.border(1.0, p.line_strong) };
    row()
        .key(keys::result(&r.id))
        .fill(if on { p.raised } else { Color::TRANSPARENT })
        .w_full()
        .h(56.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .hover(p.hover)
        .press(p.press)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .role("button")
        .label(copy::search_row_aria(&r.title, &r.uploader))
        .on_press(move |s: &mut State| actions::toggle_result(s, &id))
        .pl(12.0)
        .child(tick)
        .child(cover(s, &r.thumbnail_url, r.source, art_w(r.source), 36.0, p))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(label(r.title.clone(), sans(13.0, 500.0), p.text).single_line().truncate())
                .child(sub.one(sans(12.0, 400.0))),
        )
        .child(glyph(&icons::DOWNLOAD, p.muted).help(copy::SEARCH_ROW_DOWNLOAD))
}
