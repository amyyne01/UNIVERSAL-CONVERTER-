//! Matching a Spotify track to a YouTube Music song (D34): reading the song
//! search, choosing a row, and the verdict on a looked-up candidate. A port of
//! tools/match-model/match.mjs, the reference the rules were measured with.
use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

use crate::types::Track;

static BRACKETED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\((.*?)\)|\[(.*?)\]").expect("constant pattern"));
static NOT_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\p{L}\p{N}]+").expect("constant pattern"));

/// Lowercase, bracketed notes removed, every run of anything but letters and
/// digits a single space.
pub fn norm(s: &str) -> String {
    let lower = s.to_lowercase();
    let bare = BRACKETED.replace_all(&lower, " ");
    NOT_WORD.replace_all(&bare, " ").trim_matches(' ').to_owned()
}

fn tokens(s: &str) -> Vec<String> {
    norm(s).split(' ').filter(|t| !t.is_empty()).map(str::to_owned).collect()
}

/// Spotify appends edition notes after " - " ("Dreams - 2004 Remaster"); the
/// song's name is the part before.
fn base_name(name: &str) -> &str {
    name.split(" - ").next().unwrap_or("")
}

/// The share of `want`'s words found in `have`.
fn overlap(want: &str, have: &str) -> f64 {
    let want = tokens(want);
    if want.is_empty() {
        return 0.0;
    }
    let have: HashSet<String> = tokens(have).into_iter().collect();
    want.iter().filter(|t| have.contains(*t)).count() as f64 / want.len() as f64
}

/// Words inside brackets: where a title carries its version ("(2016 Remaster)").
fn notes(s: &str) -> Vec<String> {
    let lower = s.to_lowercase();
    BRACKETED
        .captures_iter(&lower)
        .flat_map(|c| {
            let inside = c.get(1).or_else(|| c.get(2)).map_or("", |m| m.as_str());
            NOT_WORD.split(inside).filter(|w| !w.is_empty()).map(str::to_owned).collect::<Vec<_>>()
        })
        .collect()
}

/// The separate artists in a credit ("A, B & C"), normalised, without a
/// leading "the".
fn people<'a>(credits: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    static BETWEEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*|\s+&\s+|\s+[xX]\s+").expect("constant pattern"));
    credits
        .into_iter()
        .flat_map(|credit| BETWEEN.split(credit).map(norm).collect::<Vec<_>>())
        .map(|name| name.strip_prefix("the ").map(str::to_owned).unwrap_or(name))
        .filter(|name| !name.is_empty())
        .collect()
}

/// A one-word name must match whole ("Nas" is not "Lil Nas X"); a longer one
/// may sit inside a longer credit.
fn within(hay: &str, needle: &str) -> bool {
    if hay == needle {
        return true;
    }
    let words: Vec<&str> = needle.split(' ').collect();
    let hay: HashSet<&str> = hay.split(' ').collect();
    words.len() >= 2 && words.iter().all(|w| hay.contains(w))
}

/// The two credits name the same people: every artist on one side is on the
/// other. YouTube often credits only the main artist and Spotify all of them,
/// so either side may be the shorter one. One shared name is not enough: a
/// composer is shared by every performer of the piece.
fn same_artist(ours: &[String], theirs: &[String]) -> bool {
    let inside = |few: &[String], many: &[String]| !few.is_empty() && few.iter().all(|x| many.iter().any(|y| within(y, x) || within(x, y)));
    inside(ours, theirs) || inside(theirs, ours)
}

fn credited(track: &Track) -> Vec<String> {
    if track.artists.is_empty() { people([track.artist.as_str()]) } else { people(track.artists.iter().map(String::as_str)) }
}

/// YouTube's title adds no version note of its own ("(2016 Remaster)").
fn no_new_note(track: &Track, title: &str) -> bool {
    let ours: HashSet<String> = tokens(&track.name.replace(['(', ')', '[', ']'], " ")).into_iter().collect();
    notes(title).iter().all(|w| ours.contains(w))
}

/// One row of a YouTube Music song search: unlike a plain search result it
/// carries the artist and the length.
#[derive(Debug, Clone, Copy)]
pub struct SongRow<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    /// Seconds, to the second.
    pub duration: Option<f64>,
}

/// Does this search row name the Spotify track? Most matches are settled
/// here, before any slow lookup.
pub fn row_agrees(track: &Track, row: &SongRow) -> bool {
    let Some(duration) = row.duration else { return false };
    if !same_artist(&credited(track), &people([row.artist])) {
        return false;
    }
    let diff = (duration - track.duration_ms / 1000.0).abs();
    if overlap(base_name(&track.name), row.title) >= 0.99 {
        return diff <= 3.5 || (diff <= 5.5 && no_new_note(track, row.title));
    }
    // Spotify puts the composer in front of a classical title ("Ravel: Pavane ..."); YouTube does not.
    tokens(row.title).len() >= 3 && overlap(row.title, &track.name) >= 0.99 && diff <= 2.0
}

/// What a full lookup says about a video: the music metadata YouTube attaches
/// to a song, absent on ordinary uploads.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Lookup {
    /// Seconds.
    pub duration: Option<f64>,
    pub track: Option<String>,
    pub artist: Option<String>,
    pub creator: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The same song by the same artist at the same length.
    Confirmed,
    /// A different song, artist or version.
    Rejected,
    /// No music metadata, or not enough to tell.
    Unknown,
}

/// Is this looked-up video the Spotify track? Judged only from the music
/// metadata and the length, never from the video's title.
pub fn verify(track: &Track, look: &Lookup) -> Verdict {
    fn filled(s: &Option<String>) -> Option<&str> {
        s.as_deref().filter(|s| !s.is_empty())
    }
    let (Some(duration), Some(title)) = (look.duration, filled(&look.track)) else { return Verdict::Unknown };
    if filled(&look.artist).or(filled(&look.creator)).is_none() {
        return Verdict::Unknown;
    }
    let diff = (duration - track.duration_ms / 1000.0).abs();
    let name = overlap(base_name(&track.name), title) >= 0.99;
    let artist = same_artist(&credited(track), &people([look.artist.as_deref().unwrap_or(""), look.creator.as_deref().unwrap_or("")]));
    if name && artist {
        if diff <= 3.0 {
            return Verdict::Confirmed;
        }
        if diff > 12.0 {
            return Verdict::Rejected;
        }
        // A few seconds apart is the same recording only when YouTube's title
        // adds no version note of its own ("(2016 Remaster)").
        return if diff <= 5.0 && no_new_note(track, title) { Verdict::Confirmed } else { Verdict::Unknown };
    }
    // A different name alone may be a translation; it counts with a second disagreement.
    if !name && (!artist || diff > 3.0) {
        return Verdict::Rejected;
    }
    if name && !artist && diff > 12.0 {
        return Verdict::Rejected;
    }
    Verdict::Unknown
}

// ── YouTube Music's own song search ─────────────────────────────────────────

/// Where the YouTube Music web page sends a search. No login, no key.
pub const SONG_SEARCH_URL: &str = "https://music.youtube.com/youtubei/v1/search?alt=json";

/// The body of a search limited to songs.
pub fn song_search_body(query: &str) -> String {
    serde_json::json!({
        "context": { "client": { "clientName": "WEB_REMIX", "clientVersion": "1.20240918.01.00", "hl": "en" } },
        "query": query,
        "params": "EgWKAQIIAWoKEAoQAxAEEAkQBQ==",
    })
    .to_string()
}

/// One song of a search response.
#[derive(Debug, Clone, PartialEq)]
pub struct SongHit {
    /// The video id.
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Seconds.
    pub duration: Option<f64>,
    /// Carries YouTube Music's "E" badge.
    pub explicit: bool,
}

impl SongHit {
    pub fn row(&self) -> SongRow<'_> {
        SongRow { title: &self.title, artist: &self.artist, duration: self.duration }
    }

    pub fn url(&self) -> String {
        format!("https://music.youtube.com/watch?v={}", self.id)
    }
}

/// The songs in a search response, in order. The response is outside input:
/// anything that is not the expected shape simply yields no rows.
pub fn song_rows(response: &str) -> Vec<SongHit> {
    fn collect<'a>(value: &'a Value, found: &mut Vec<&'a Value>) {
        match value {
            Value::Object(map) => {
                if let Some(item) = map.get("musicResponsiveListItemRenderer").filter(|v| v.is_object()) {
                    found.push(item);
                }
                map.values().for_each(|v| collect(v, found));
            }
            Value::Array(list) => list.iter().for_each(|v| collect(v, found)),
            _ => {}
        }
    }
    let Ok(response) = serde_json::from_str::<Value>(response) else { return Vec::new() };
    let mut items = Vec::new();
    collect(&response, &mut items);
    items.into_iter().filter_map(song_hit).collect()
}

fn song_hit(item: &Value) -> Option<SongHit> {
    let id = item["playlistItemData"]["videoId"].as_str().filter(|id| !id.is_empty())?;
    let column = |n: usize| -> String {
        let runs = item["flexColumns"][n]["musicResponsiveListItemFlexColumnRenderer"]["text"]["runs"].as_array();
        runs.into_iter().flatten().filter_map(|run| run["text"].as_str()).collect()
    };
    // The second column reads "Artist • Album • 3:08".
    let second = column(1);
    let parts: Vec<&str> = second.split(" • ").collect();
    let clock: Option<Vec<f64>> = parts.last().and_then(|last| {
        last.split(':').map(|p| if p.trim().is_empty() { Some(0.0) } else { p.trim().parse::<f64>().ok().filter(|n| n.is_finite()) }).collect()
    });
    let clock = clock.filter(|c| c.len() >= 2);
    let (artist, album) = match &clock {
        Some(_) if parts.len() >= 3 => (parts[..parts.len() - 2].join(" • "), parts[parts.len() - 2].to_owned()),
        Some(_) => (parts[..parts.len() - 1].join(" • "), String::new()),
        None => (second.clone(), String::new()),
    };
    Some(SongHit {
        id: id.to_owned(),
        title: column(0),
        artist,
        album,
        duration: clock.map(|c| c.iter().fold(0.0, |total, part| total * 60.0 + part)),
        explicit: item.get("badges").is_some_and(|badges| badges.to_string().contains("MUSIC_EXPLICIT_BADGE")),
    })
}

/// Which row to download, and whether it carries Spotify's explicit flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pick {
    pub index: usize,
    pub explicit_agrees: bool,
}

/// An album's name for comparing: lowercase, punctuation as spaces, edition
/// notes kept ("Rumours (Deluxe Edition)" is not "Rumours").
fn album_key(album: &str) -> String {
    NOT_WORD.replace_all(&album.to_lowercase(), " ").trim_matches(' ').to_owned()
}

/// The row to download. Among the rows that agree with the track, those
/// carrying Spotify's explicit flag come first; among those, one from the
/// track's own album, when Spotify named it; else the first. `explicit` is
/// `None` when Spotify's flag is not known.
pub fn pick_row(track: &Track, rows: &[SongHit], explicit: Option<bool>) -> Option<Pick> {
    let agreeing: Vec<(usize, &SongHit)> = rows.iter().enumerate().filter(|(_, row)| row_agrees(track, &row.row())).collect();
    let flagged: Vec<(usize, &SongHit)> = agreeing.iter().copied().filter(|(_, row)| explicit.is_none_or(|flag| row.explicit == flag)).collect();
    let pool = if flagged.is_empty() { &agreeing } else { &flagged };
    let album = album_key(&track.album);
    let same_album = pool.iter().find(|(_, row)| !album.is_empty() && album_key(&row.album) == album);
    let (index, _) = same_album.or(pool.first())?;
    Some(Pick { index: *index, explicit_agrees: !flagged.is_empty() })
}

/// The same song search as a page yt-dlp can read, for when the direct
/// request cannot be made. Its rows carry only an id and a title.
pub fn song_search_page(query: &str) -> String {
    let query: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    format!("https://music.youtube.com/search?q={query}#songs")
}

// ── The match itself ────────────────────────────────────────────────────────

/// The confidence of a confirmed match: artist, name, length and explicit
/// flag all agree. Anything lower is a best guess.
pub const CONFIRMED: f64 = 1.0;
/// A song that agrees with the track but carries the other explicit flag.
pub const EXPLICIT_DIFFERS: f64 = 0.9;
/// The most a match picked by score alone, or with no stated confidence, may claim.
pub const GUESS_CEILING: f64 = 0.89;

/// Why a Spotify track was not downloaded when only confirmed matches may be
/// ("needs review"). Unlisted in the error phrases, so it is never retried.
pub const NO_CONFIRMED_MATCH: &str = "No confirmed match for this Spotify track.";

/// Why a Spotify track was not downloaded at all: no song agreed and no
/// video resembled it enough to be offered even as a guess.
pub const NO_MATCH_FOUND: &str = "No match was found for this Spotify track.";
/// Below this score a plain video is not offered even as a guess. A judgment
/// call: the videos that can be judged from YouTube's music labels are mostly
/// official uploads, so the collected songs say little about covers, live
/// versions and fan uploads. On them it would have dropped 3 right picks of 79
/// and no wrong one.
pub const GUESS_FLOOR: f64 = 0.5;

/// What is searched for: "Artist - Name". The text reaches yt-dlp as an
/// argument, with no shell, so quotes are removed.
pub fn song_query(track: &Track) -> String {
    let clean = |s: &str| crate::text::js_trim(&s.replace('"', "")).to_owned();
    let artist = if track.artist.is_empty() { track.artists.first().map_or("", String::as_str) } else { &track.artist };
    format!("{} - {}", clean(artist), clean(&track.name))
}

/// The song chosen for a track, with what the search said about it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongMatch {
    pub url: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Seconds.
    pub duration: Option<f64>,
    pub explicit: bool,
    /// [`CONFIRMED`], or less for a best guess.
    pub confidence: f64,
}

/// The song of these search rows that is the track, if one agrees.
pub fn match_song(track: &Track, rows: &[SongHit]) -> Option<SongMatch> {
    let pick = pick_row(track, rows, track.explicit)?;
    let hit = &rows[pick.index];
    Some(SongMatch {
        url: hit.url(),
        title: hit.title.clone(),
        artist: hit.artist.clone(),
        album: hit.album.clone(),
        duration: hit.duration,
        explicit: hit.explicit,
        confidence: if pick.explicit_agrees { CONFIRMED } else { EXPLICIT_DIFFERS },
    })
}

/// A match named from outside (a request, a saved task) is only used when it
/// is a plain YouTube address: it becomes an argument to yt-dlp.
pub fn song_url(url: &str) -> Option<&str> {
    let parsed = url::Url::parse(crate::validate::media_url_arg(url).ok()?).ok()?;
    let host = parsed.host_str()?;
    (parsed.scheme() == "https" && ["music.youtube.com", "www.youtube.com", "youtube.com", "youtu.be"].contains(&host)).then_some(url)
}

// ── Naming and tagging the file ─────────────────────────────────────────────

/// D34: a Spotify download is named and tagged from the Spotify track, not
/// from the video it was matched to. Adds to `args` (as built for the task,
/// the link last): the tags, when tags are embedded at all, and the file name
/// "Artist - Name", unless the task or a naming template already names it.
pub fn name_and_tag_from_spotify(task: &crate::types::DownloadTask, args: &mut Vec<String>) {
    use crate::downloader::args::{named_after_the_video, sanitize};
    let Some(track) = &task.track else { return };
    if args.is_empty() {
        return;
    }
    if task.embed_metadata {
        let artists = if track.artists.is_empty() { track.artist.clone() } else { track.artists.join(", ") };
        let number = if track.track_number >= 1.0 { format!("{}", track.track_number as u64) } else { String::new() };
        let tags = [("title", track.name.as_str()), ("artist", &artists), ("album", &track.album), ("track", &number), ("date", track.album_year.as_deref().unwrap_or(""))];
        let flags: Vec<String> = tags.iter().filter_map(|(field, text)| fixed_tag(field, text)).flat_map(|tag| ["--parse-metadata".to_owned(), tag]).collect();
        // Before the link, and before the "--" that ends the options if there is one.
        let link = args.len() - 1;
        let at = if link > 0 && args[link - 1] == "--" { link - 1 } else { link };
        args.splice(at..at, flags);
    }
    if named_after_the_video(task) {
        let artist = if track.artist.is_empty() { track.artists.first().map_or("", String::as_str) } else { &track.artist };
        let both = if artist.is_empty() { track.name.clone() } else { format!("{artist} - {}", track.name) };
        // "%" is doubled so yt-dlp fills nothing into the name.
        let stem = sanitize(&both).replace('%', "%%");
        if let Some(output) = args.iter().position(|a| a == "-o").and_then(|at| args.get_mut(at + 1))
            && let Some(at) = output.rfind("%(title)s").filter(|_| !stem.is_empty())
        {
            output.replace_range(at..at + "%(title)s".len(), &stem);
        }
    }
}

/// `--parse-metadata` value that sets a tag to this exact text, or `None` for
/// nothing to set. yt-dlp reads the value in a small language of its own: ":"
/// separates and "%(...)s" is filled in. Measured against the real yt-dlp:
/// doubling "%" and escaping ":" carries any text through unchanged, except a
/// trailing backslash, which would swallow the separator and is dropped.
fn fixed_tag(field: &str, text: &str) -> Option<String> {
    let text = text.trim_end_matches('\\');
    (!text.trim().is_empty()).then(|| format!("{}:%(meta_{field})s", text.replace('%', "%%").replace(':', "\\:")))
}
