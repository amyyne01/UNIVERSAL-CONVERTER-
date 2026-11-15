//! Ordered URL-recognition cascade: trim, prepend `https://` when there is no
//! scheme, then walk a fixed list where the first match wins and specific
//! shapes come before general ones.
//!
//! The patterns are the ones in electron/url-detector.ts, with JavaScript's
//! meaning when its `u` flag is off: case folding, word and digit classes and
//! the word boundary are ASCII-only, so those are spelled out here.
use std::sync::LazyLock;

use fancy_regex::Regex;
use url::Url;

use crate::text::{js_trim, utf16_prefix};
use crate::types::{ContentType, SourcePlatform, UrlDetection};
use ContentType::{Album, Artist, Playlist, Track, Video};
use SourcePlatform::*;

/// Compiles a pattern with JavaScript's `i` flag as it behaves without `u`:
/// only ASCII letters fold. Every letter outside a character class or an
/// escape becomes a two-case class; classes are written with both cases.
fn re(pattern: &str) -> Regex {
    let mut out = String::with_capacity(pattern.len() * 2);
    let mut chars = pattern.chars();
    let mut in_class = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                out.push(c);
                out.extend(chars.next());
            }
            '[' => {
                in_class = true;
                out.push(c);
            }
            ']' => {
                in_class = false;
                out.push(c);
            }
            c if c.is_ascii_alphabetic() && !in_class => {
                out.extend(['[', c.to_ascii_lowercase(), c.to_ascii_uppercase(), ']']);
            }
            c => out.push(c),
        }
    }
    Regex::new(&out).expect("pattern is a compile-time constant")
}

/// The leftmost match's first participating capture group. The outer `Option`
/// says whether the pattern matched at all.
fn cap(re: &Regex, text: &str) -> Option<Option<String>> {
    let caps = re.captures(text).ok()??;
    Some((1..caps.len()).find_map(|i| caps.get(i)).map(|m| m.as_str().to_owned()))
}

fn is_match(re: &Regex, text: &str) -> bool {
    re.is_match(text).unwrap_or(false)
}

fn build(url: &str, platform: SourcePlatform, content_type: ContentType, id: Option<String>) -> UrlDetection {
    let label = match platform {
        Direct => "Direct media".to_owned(),
        Unknown => "Unknown link".to_owned(),
        // A generic hit carries its hostname as the id, which names the site
        // far better than the word "Link" does.
        Generic => format!("{} {}", id.as_deref().unwrap_or(Generic.label()), content_type.as_str()),
        _ => format!("{} {}", platform.label(), content_type.as_str()),
    };
    UrlDetection {
        url: url.to_owned(),
        platform,
        content_type,
        id,
        is_collection: matches!(content_type, Playlist | Album),
        label,
    }
}

static SPOTIFY: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?:open\.spotify\.com/(?:intl-[A-Za-z]{2}/)?|spotify:)(track|playlist|album|artist)[/:]([A-Za-z0-9]+)")
});

/// Recognises Spotify links in web (`open.spotify.com/[intl-xx/]<type>/<id>`)
/// and URI (`spotify:<type>:<id>`) notation. Returns the lower-cased type and the id.
pub fn parse_spotify_url(url: &str) -> Option<(String, String)> {
    let caps = SPOTIFY.captures(url).ok()??;
    Some((caps.get(1)?.as_str().to_ascii_lowercase(), caps.get(2)?.as_str().to_owned()))
}

// Non-core sources. Each rule is tested against `hostname + pathname`, anchored
// at the start, so a lookalike (twitch.tv.evil.com, evil.com/twitch.tv/…) can
// never match. A single-item shape precedes its collection rule.
static SOURCE_RULES: LazyLock<Vec<(Regex, SourcePlatform, ContentType)>> = LazyLock::new(|| {
    const W: &str = r"(?:www\.|m\.|mobile\.)?";
    const RESERVED: &str = "discover|select|upload|settings|about|pro|categories|search|dashboard|terms";
    const SEG: &str = "[^/]"; // one character of a path segment
    let rules: Vec<(String, SourcePlatform, ContentType)> = vec![
        (format!(r"^{W}(?:twitter|x)\.com/(?:i/web|{SEG}+)/status/([0-9]+)"), Twitter, Video),
        (r"^player\.vimeo\.com/video/([0-9]+)".into(), Vimeo, Video),
        (format!(r"^{W}vimeo\.com/(?:channels/{SEG}+|groups/{SEG}+/videos)/([0-9]+)"), Vimeo, Video),
        (format!(r"^{W}vimeo\.com/(?:showcase|album)/([0-9]+)/?$"), Vimeo, Playlist),
        (format!(r"^{W}vimeo\.com/(?:channels|groups)/({SEG}+)/?$"), Vimeo, Playlist),
        (format!(r"^{W}vimeo\.com/([0-9]+)(?:/[A-Fa-f0-9]+)?/?$"), Vimeo, Video),
        (format!(r"^{W}twitch\.tv/videos/([0-9]+)"), Twitch, Video),
        (format!(r"^{W}twitch\.tv/{SEG}+/v(?:ideo)?/([0-9]+)"), Twitch, Video),
        (r"^clips\.twitch\.tv/([A-Za-z0-9_-]+)".into(), Twitch, Video),
        (format!(r"^{W}twitch\.tv/{SEG}+/clip/([A-Za-z0-9_-]+)"), Twitch, Video),
        // Channel archive only; a bare twitch.tv/<name> is a live stream and never ends.
        (format!(r"^{W}twitch\.tv/({SEG}+)/(?:videos|clips)/?$"), Twitch, Playlist),
        (r"^[A-Za-z0-9-]+\.bandcamp\.com/track/([^/?#]+)".into(), Bandcamp, Track),
        (r"^[A-Za-z0-9-]+\.bandcamp\.com/album/([^/?#]+)".into(), Bandcamp, Album),
        (r"^(?!www\.)([A-Za-z0-9-]+)\.bandcamp\.com(?:/music)?/?$".into(), Bandcamp, Album),
        (format!(r"^(?:[A-Za-z]+\.)?reddit\.com/(?:(?:r|user)/{SEG}+/)?comments/([A-Za-z0-9_]+)"), Reddit, Video),
        (r"^(?:v\.)?redd\.it/([A-Za-z0-9_]+)".into(), Reddit, Video),
        (format!(r"^{W}dailymotion\.com/(?:embed/)?video/([A-Za-z0-9_]+)"), Dailymotion, Video),
        (r"^dai\.ly/([A-Za-z0-9_]+)".into(), Dailymotion, Video),
        (format!(r"^{W}dailymotion\.com/playlist/([A-Za-z0-9_]+)"), Dailymotion, Playlist),
        (
            format!(
                r"^{W}mixcloud\.com/(?!(?:{RESERVED})/){SEG}+/(?!(?:stream|uploads|favorites|listens|playlists|followers|following)(?:/|$))({SEG}+)/?$"
            ),
            Mixcloud,
            Track,
        ),
        (format!(r"^{W}mixcloud\.com/(?!(?:{RESERVED})(?:/|$))({SEG}+)/?$"), Mixcloud, Playlist),
        (format!(r"^{W}rumble\.com/(v(?!ideos)[A-Za-z0-9_.-]+)"), Rumble, Video),
    ];
    rules.into_iter().map(|(p, platform, kind)| (re(&p), platform, kind)).collect()
});

static YT_PLAYLIST: LazyLock<Regex> = LazyLock::new(|| {
    // `.` in the original excludes JavaScript's four line terminators.
    re(r"youtube\.com/playlist\?[^\n\r\x{2028}\x{2029}]*(?<![A-Za-z0-9_])list=([A-Za-z0-9_-]+)")
});
static YT_VIDEO: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?:youtube\.com/(?:watch\?(?:[^#]*&)?v=|embed/|shorts/|live/)|youtu\.be/)([A-Za-z0-9_-]{11})")
});
static SC_SET: LazyLock<Regex> = LazyLock::new(|| re(r"soundcloud\.com/[^/]+/sets/([^/?#]+)"));
static SC_TRACK: LazyLock<Regex> = LazyLock::new(|| {
    re(r"soundcloud\.com/[^/]+/(?!(?:sets|likes|reposts|followers|following|tracks|albums|you)(?:[/?#]|$))([^/?#]+)")
});
static INSTAGRAM: LazyLock<Regex> = LazyLock::new(|| re(r"instagram\.com/(?:reel|reels|p|tv)/([^/?#]+)"));
static TIKTOK: LazyLock<Regex> = LazyLock::new(|| re(r"tiktok\.com"));
static TIKTOK_CANONICAL: LazyLock<Regex> = LazyLock::new(|| re(r"@[^/]+/video/([0-9]+)"));
static TIKTOK_SHORT: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?:vm\.|vt\.)tiktok\.com/([A-Za-z0-9]+)|tiktok\.com/t/([A-Za-z0-9]+)"));
static FACEBOOK: LazyLock<Regex> = LazyLock::new(|| re(r"(?:facebook\.com|fb\.watch)"));
static FACEBOOK_VIDEO: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?:/videos/|/reel/|[?&]v=|fb\.watch/)([A-Za-z0-9]+)"));
static DIRECT: LazyLock<Regex> = LazyLock::new(|| re(r"\.(?:mp4|mkv|webm|avi|mov)(?:[?#]|$)"));
static REAL_HOST: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$"));

/// An `http(s)` URL's hostname and path, parsed the way `new URL()` does.
fn web_parts(url: &str) -> Option<(String, String)> {
    let u = Url::parse(url).ok()?;
    matches!(u.scheme(), "http" | "https").then(|| (u.host_str().unwrap_or("").to_owned(), u.path().to_owned()))
}

/// The fixed ordered cascade; the first rule that matches wins.
fn cascade(url: &str) -> Option<UrlDetection> {
    // YouTube playlist, before the single-video rule.
    if let Some(id) = cap(&YT_PLAYLIST, url) {
        return Some(build(url, Youtube, Playlist, id));
    }
    // YouTube video: an 11-character id after watch?v= / youtu.be/ / embed/ / shorts/ / live/.
    if let Some(id) = cap(&YT_VIDEO, url) {
        return Some(build(url, Youtube, Video, id));
    }
    if let Some((kind, id)) = parse_spotify_url(url) {
        let kind = match kind.as_str() {
            "track" => Track,
            "playlist" => Playlist,
            "album" => Album,
            _ => Artist,
        };
        return Some(build(url, Spotify, kind, Some(id)));
    }
    // A SoundCloud set is a playlist, checked before the looser track rule.
    if let Some(id) = cap(&SC_SET, url) {
        return Some(build(url, Soundcloud, Playlist, id));
    }
    // SoundCloud track: a slug, with profile sub-pages excluded.
    if let Some(id) = cap(&SC_TRACK, url) {
        return Some(build(url, Soundcloud, Track, id));
    }
    if let Some(id) = cap(&INSTAGRAM, url) {
        return Some(build(url, Instagram, Video, id));
    }
    // TikTok: only a real video URL. A bare profile or search page falls through.
    if is_match(&TIKTOK, url)
        && let Some(id) = cap(&TIKTOK_CANONICAL, url).or_else(|| cap(&TIKTOK_SHORT, url))
    {
        return Some(build(url, Tiktok, Video, id));
    }
    // Facebook: must carry a video id, not just the domain.
    if is_match(&FACEBOOK, url)
        && let Some(id) = cap(&FACEBOOK_VIDEO, url)
    {
        return Some(build(url, Facebook, Video, id));
    }
    // First-class non-core sources (http/https only, host-anchored).
    let web = web_parts(url);
    if let Some((host, path)) = &web {
        let subject = format!("{host}{path}");
        for (re, platform, kind) in SOURCE_RULES.iter() {
            if let Some(id) = cap(re, &subject) {
                return Some(build(url, *platform, *kind, id));
            }
        }
    }
    // Direct media file by extension.
    if is_match(&DIRECT, url) {
        return Some(build(url, Direct, Video, None));
    }
    // Terminal rule: hand any other real http(s) address to the engine, which
    // supports far more sites than are named above. It is strict about what
    // counts as a link, because a scheme-less search phrase has had `https://`
    // put in front of it and must not reach a spawn as if it were a URL.
    let (host, _) = web?;
    is_match(&REAL_HOST, &host).then(|| build(url, Generic, Video, Some(host)))
}

fn has_scheme(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))) == Some(':')
}

pub fn detect_url(input: &str) -> UrlDetection {
    // The length bound limits regex work on clipboard text.
    let trimmed = utf16_prefix(js_trim(input), 2048);
    // A bare address gets https://; spotify: and http(s): keep their scheme.
    let url = if has_scheme(trimmed) { trimmed.to_owned() } else { format!("https://{trimmed}") };
    cascade(&url).unwrap_or_else(|| build(&url, Unknown, ContentType::Unknown, None))
}
