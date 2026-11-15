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

/// The row to download: the first that agrees and carries Spotify's explicit
/// flag, else the first that agrees. `explicit` is `None` when Spotify's flag
/// is not known.
pub fn pick_row(track: &Track, rows: &[SongHit], explicit: Option<bool>) -> Option<Pick> {
    let mut agreeing = rows.iter().enumerate().filter(|(_, row)| row_agrees(track, &row.row()));
    let first = agreeing.next()?;
    let exact = match explicit {
        None => Some(first),
        Some(flag) => std::iter::once(first).chain(agreeing).find(|(_, row)| row.explicit == flag),
    };
    Some(Pick { index: exact.unwrap_or(first).0, explicit_agrees: exact.is_some() })
}
