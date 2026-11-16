//! What both screens share: facts read off a task (in CONTENT.md's terms) and
//! the small pieces drawn the same way everywhere: the sleeve with its spine
//! label, keycaps, buttons and rich text runs.
use ahg_engine::types::{DownloadStatus as St, DownloadTask, ErrorCode, SourcePlatform};
use rune::{Color, El, Icon, Span, SurfaceFit, el, icon, rich, row, text};

use super::theme::{self, Pal, mono, sans};
use crate::copy;
use crate::gate::State;

// ── Task facts ──────────────────────────────────────────────────────────────

/// Counts toward "active": everything not finished.
pub fn is_active(st: St) -> bool {
    matches!(st, St::Queued | St::FetchingInfo | St::Downloading | St::Converting | St::Embedding | St::Paused | St::Retrying)
}

/// Transferring or about to: the rows that sweep.
pub fn is_live(st: St) -> bool {
    matches!(st, St::FetchingInfo | St::Downloading | St::Converting | St::Embedding | St::Retrying)
}

pub fn source_name(p: SourcePlatform) -> &'static str {
    use SourcePlatform::*;
    match p {
        Youtube => copy::SOURCE_YOUTUBE,
        Spotify => copy::SOURCE_SPOTIFY,
        Soundcloud => copy::SOURCE_SOUNDCLOUD,
        Instagram => copy::SOURCE_INSTAGRAM,
        Tiktok => copy::SOURCE_TIKTOK,
        Facebook => copy::SOURCE_FACEBOOK,
        Twitter => copy::SOURCE_TWITTER,
        Vimeo => copy::SOURCE_VIMEO,
        Twitch => copy::SOURCE_TWITCH,
        Bandcamp => copy::SOURCE_BANDCAMP,
        Reddit => copy::SOURCE_REDDIT,
        Dailymotion => copy::SOURCE_DAILYMOTION,
        Mixcloud => copy::SOURCE_MIXCLOUD,
        Rumble => copy::SOURCE_RUMBLE,
        Direct => copy::SOURCE_DIRECT,
        Generic => copy::SOURCE_GENERIC,
        Unknown => copy::SOURCE_UNKNOWN,
    }
}

/// The text after the last `\` or `/`.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// `linkLabel(url)` (CONTENT.md 4.7): the stand-in title for a task with none.
pub fn link_label(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let (before_hash, _) = rest.split_once('#').unwrap_or((rest, ""));
    let (host_path, query) = before_hash.split_once('?').unwrap_or((before_hash, ""));
    let (host, path) = host_path.split_once('/').map(|(h, p)| (h, format!("/{p}"))).unwrap_or((host_path, String::new()));
    let host = host.strip_prefix("www.").or_else(|| host.strip_prefix("m.")).unwrap_or(host);
    if let Some(v) = query.split('&').find_map(|kv| kv.strip_prefix("v=")).filter(|v| !v.is_empty()) {
        return format!("{host} · {v}");
    }
    let label = format!("{host}{}", path.trim_end_matches('/'));
    if label.chars().count() > 48 { format!("{}…", label.chars().take(47).collect::<String>()) } else { label }
}

/// A row's label: the title, else the saved file's name, else the link.
pub fn label(t: &DownloadTask) -> String {
    if !t.title.is_empty() {
        t.title.clone()
    } else if !t.progress.filename.is_empty() {
        file_name(&t.progress.filename).to_string()
    } else {
        link_label(&t.url)
    }
}

pub fn is_permanent(t: &DownloadTask) -> bool {
    matches!(t.error_code, Some(ErrorCode::Removed | ErrorCode::Unsupported | ErrorCode::Drm | ErrorCode::GeoBlocked))
}

/// The readable sentence on a failed or cancelled row (CONTENT.md 3.7), or the raw error.
pub fn reason(t: &DownloadTask) -> Option<String> {
    use ErrorCode::*;
    if !matches!(t.progress.status, St::Failed | St::Cancelled) {
        return None;
    }
    let Some(kind) = t.error_code.filter(|_| t.progress.status == St::Failed) else {
        return Some(t.progress.error.clone()).filter(|e| !e.is_empty());
    };
    if t.source == SourcePlatform::Generic && kind == Removed {
        return Some(copy::ERR_GENERIC_DEAD_END.to_string());
    }
    let (cause, remedy) = match kind {
        Private => (copy::ERR_PRIVATE_CAUSE, copy::ERR_PRIVATE_REMEDY),
        Removed => (copy::ERR_REMOVED_CAUSE, copy::ERR_REMOVED_REMEDY),
        AgeRestricted => (copy::ERR_AGE_RESTRICTED_CAUSE, copy::ERR_AGE_RESTRICTED_REMEDY),
        LoginRequired => (copy::ERR_LOGIN_REQUIRED_CAUSE, copy::ERR_LOGIN_REQUIRED_REMEDY),
        GeoBlocked => (copy::ERR_GEO_BLOCKED_CAUSE, copy::ERR_GEO_BLOCKED_REMEDY),
        Drm => (copy::ERR_DRM_CAUSE, copy::ERR_DRM_REMEDY),
        RateLimited => (copy::ERR_RATE_LIMITED_CAUSE, copy::ERR_RATE_LIMITED_REMEDY),
        Network => (copy::ERR_NETWORK_CAUSE, copy::ERR_NETWORK_REMEDY),
        Unsupported => (copy::ERR_UNSUPPORTED_CAUSE, copy::ERR_UNSUPPORTED_REMEDY),
        FfmpegMissing => (copy::ERR_FFMPEG_MISSING_CAUSE, copy::ERR_FFMPEG_MISSING_REMEDY),
        DiskFull => (copy::ERR_DISK_FULL_CAUSE, copy::ERR_DISK_FULL_REMEDY),
        Unknown => (copy::ERR_UNKNOWN_CAUSE, copy::ERR_UNKNOWN_REMEDY),
    };
    let remedy = if t.source == SourcePlatform::Spotify && matches!(kind, Unknown | Removed | Unsupported) {
        copy::ERR_SPOTIFY_REMEDY
    } else {
        remedy
    };
    let video = matches!(
        t.source,
        SourcePlatform::Youtube | SourcePlatform::Vimeo | SourcePlatform::Dailymotion | SourcePlatform::Rumble | SourcePlatform::Twitch
    );
    Some(copy::error_text(cause, remedy, video))
}

/// `[position] · [duration] · [size] · [format]`, empty parts left out.
pub fn specs(t: &DownloadTask) -> String {
    let p = &t.progress;
    let size = if p.total > 0.0 { p.total } else { p.downloaded };
    let position = (t.is_playlist && p.playlist_total > 0.0).then(|| format!("{}/{}", p.playlist_index, p.playlist_total));
    [position, Some(copy::format_duration(t.duration)), (size > 0.0).then(|| copy::format_bytes(size)), Some(t.format.clone())]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// A full month name from `YYYY-MM` (CONTENT.md 4.6); anything else as is.
pub fn month_name(ym: &str) -> String {
    const NAMES: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    ym.split_once('-')
        .and_then(|(_, m)| m.parse::<usize>().ok())
        .and_then(|m| NAMES.get(m.wrapping_sub(1)))
        .map_or_else(|| ym.to_string(), |n| n.to_string())
}

// ── Text ────────────────────────────────────────────────────────────────────

/// One run of a line made of several styles.
pub struct Run {
    pub text: String,
    pub color: Color,
    pub mono: bool,
    pub weight: Option<f32>,
}

pub fn run(text: impl Into<String>, color: Color) -> Run {
    Run { text: text.into(), color, mono: false, weight: None }
}

pub fn run_mono(text: impl Into<String>, color: Color) -> Run {
    Run { text: text.into(), color, mono: true, weight: None }
}

/// Runs joined into one text, each keeping its colour, weight and family.
pub fn runs(parts: Vec<Run>) -> El {
    let mut content = String::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut parts_out = Vec::new();
    for p in parts {
        // Set in Instrument Sans, the spaces either side of a middle dot came out closed up (rendered,
        // cause not isolated), so a leading ` · ` is set in the figures face.
        if !p.mono && p.text.starts_with(" · ") && p.text.len() > 4 {
            let (dot, rest) = p.text.split_at(" · ".len());
            parts_out.push(Run { text: dot.to_string(), color: p.color, mono: true, weight: None });
            parts_out.push(Run { text: rest.to_string(), ..p });
        } else {
            parts_out.push(p);
        }
    }
    for p in parts_out {
        let start = content.len();
        content.push_str(&p.text);
        let mut span = Span::new(start, content.len()).color(p.color);
        if p.mono {
            span = span.mono();
        }
        if let Some(w) = p.weight {
            span = span.weight(w);
        }
        spans.push(span);
    }
    rich(content, spans)
}

// ── Controls ────────────────────────────────────────────────────────────────

/// A key, as printed on the keyboard.
pub fn keycap(p: &Pal, key: &str) -> El {
    el().h(20.0).px(4.0).center().radius(4.0).fill(p.raised).border(1.0, p.line).child(text(key).font(mono(11.0, 400.0)).color(p.text2))
}

/// A quiet button with an optional icon: the default for every secondary action.
pub fn ghost(p: &Pal, key: &str, glyph: Option<&'static Icon>, label: &str, ink: Color) -> El {
    let mut b = row()
        .key(key)
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .border(1.0, p.line)
        .hover(p.raised)
        .press(p.line)
        .focusable(true)
        .role("button")
        .label(label);
    if let Some(g) = glyph {
        b = b.child(icon(g, 16.0).color(ink));
    }
    b.child(text(label).font(sans(13.0, 500.0)).color(ink))
}

/// The one filled button of a group.
pub fn primary(p: &Pal, key: &str, glyph: Option<&'static Icon>, label: &str) -> El {
    let mut b = row()
        .key(key)
        .h(32.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.text)
        .hover(p.text2)
        .focusable(true)
        .role("button")
        .label(label);
    if let Some(g) = glyph {
        b = b.child(icon(g, 16.0).color(p.bg));
    }
    b.child(text(label).font(sans(13.0, 500.0)).color(p.bg))
}

/// A square icon button; its label is the tooltip and the accessible name.
pub fn icon_button(p: &Pal, key: String, glyph: &'static Icon, label: &str, ink: Color) -> El {
    el().key(key)
        .size(28.0, 28.0)
        .center()
        .radius(6.0)
        .hover(p.raised)
        .press(p.line)
        .focusable(true)
        .role("button")
        .label(label)
        .help(label)
        .child(icon(glyph, 16.0).color(ink))
}

// ── The sleeve ──────────────────────────────────────────────────────────────

/// Where the record sits relative to its sleeve: tucked in, sliding out with
/// the download, or out of reach (failed and cancelled rows have none).
#[derive(Clone, Copy, PartialEq)]
pub enum Disc {
    None,
    /// 0 is fully in the sleeve, 1 fully out.
    Out(f32),
}

/// How far the record comes out, as a share of the sleeve.
pub const DISC_TRAVEL: f32 = 0.7;

/// A record sleeve: the cover (or a plain company sleeve with a die-cut hole
/// when there is none), the record behind it, and the spine label down its
/// left edge. The spine label is Shelf's signature: a strip of paper printed
/// with the file's format, on every sleeve and nowhere else.
pub fn sleeve(s: &State, p: &Pal, t: &DownloadTask, size: f32, disc: Disc, faded: bool) -> El {
    let travel = (size * DISC_TRAVEL).round();
    let width = if disc == Disc::None { size } else { size + travel };
    let mut holder = el().size(width, size);
    if let Disc::Out(out) = disc {
        holder = holder.child(record(p, t.source, size * 0.94).absolute(size * 0.03 + travel * out.clamp(0.0, 1.0), size * 0.03));
    }
    let r = (size * 0.06).round().max(3.0);
    let face = match s.cover(&t.task_id) {
        Some(img) => el().size(size, size).pixels_fit(img.pixels(), SurfaceFit::Fill),
        None => blank_face(p, t.source, size),
    };
    let spine_w = (size * 0.25).round().clamp(14.0, 24.0);
    let format = t.format.to_uppercase();
    let label_size = (spine_w * 0.64).round().clamp(9.0, 13.0);
    let printed = el()
        .size(size, spine_w)
        .center()
        .absolute(spine_w / 2.0 - size / 2.0, size / 2.0 - spine_w / 2.0)
        .rotate(-std::f32::consts::FRAC_PI_2)
        .child(text(format).font(mono(label_size, 500.0).tracking(label_size * 0.12)).color(theme::label_ink()).single_line());
    let spine = el()
        .size(spine_w, size)
        .absolute(0.0, 0.0)
        .fill(theme::label_paper())
        .border_sides([0.0, 1.0, 0.0, 0.0], theme::label_ink().alpha(0.18));
    let mut cover = el().size(size, size).absolute(0.0, 0.0).clip_rounded(r).fill(p.raised).child(face).child(spine).child(printed);
    if !p.dark {
        cover = cover.shadow(1.0, 3.0, theme::label_ink().alpha(0.18));
    }
    if faded {
        cover = cover.opacity(0.5);
    }
    holder.child(cover)
}

/// A plain company sleeve: a tint of the source with a die-cut hole showing the record's label.
fn blank_face(p: &Pal, source: SourcePlatform, size: f32) -> El {
    let hue = p.source(source);
    let hole = (size * 0.56).round();
    let label = (size * 0.30).round();
    let shift = (size * 0.22).round() / 2.0;
    el().size(size, size).center().fill(p.card.mix(hue, 0.16)).child(
        el().size(hole, hole).ml(shift).radius(hole / 2.0).center().fill(theme::vinyl()).child(
            el().size(label, label).radius(label / 2.0).center().fill(hue).child(el().size(3.0, 3.0).radius(1.5).fill(theme::vinyl())),
        ),
    )
}

/// The record: vinyl with a groove and a label in the source's colour.
fn record(p: &Pal, source: SourcePlatform, d: f32) -> El {
    let outer = (d * 0.86).round();
    let groove = (d * 0.66).round();
    let label = (d * 0.40).round();
    // Vinyl catches the light along one diagonal.
    let sheen = theme::groove().mix(theme::vinyl(), 0.55);
    let ring = |size: f32| el().size(size, size).radius(size / 2.0).center().border(1.0, theme::groove().alpha(0.7));
    el().size(d, d)
        .radius(d / 2.0)
        .center()
        .fill(theme::vinyl())
        .linear_gradient(135.0, [(0.0, theme::vinyl()), (0.48, sheen), (0.52, sheen), (1.0, theme::vinyl())])
        .border(1.0, theme::rim())
        .child(
            ring(outer).child(
                ring(groove).child(
                    el().size(label, label)
                        .radius(label / 2.0)
                        .center()
                        .fill(p.source(source))
                        .child(el().size(3.0, 3.0).radius(1.5).fill(theme::vinyl())),
                ),
            ),
        )
}
