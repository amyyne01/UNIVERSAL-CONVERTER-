//! Matching a Spotify track to a YouTube video (D34): the verdict on a
//! looked-up candidate and the numbers the plain-video ranker reads. A port of
//! tools/match-model/match.mjs, which the ranker's weights were fitted against,
//! so the two must agree exactly.
use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::types::Track;

static BRACKETED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\((.*?)\)|\[(.*?)\]").expect("constant pattern"));
static NOT_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\p{L}\p{N}]+").expect("constant pattern"));

/// Words that mark a different version, unless the Spotify title has them too.
const BAD: [&str; 10] = ["live", "cover", "remix", "sped", "8d", "nightcore", "karaoke", "instrumental", "slowed", "reverb"];

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

/// A search result as the ranker sees it: only what a flat search returns.
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub title: &'a str,
    pub uploader: &'a str,
    /// Seconds; song results carry none.
    pub duration: Option<f64>,
}

/// How many numbers describe a candidate.
pub const FEATURES: usize = 14;

/// The numbers the ranker reads, in the order its weights expect: bias, title
/// overlap, artist in title, artist is uploader, topic channel, has duration,
/// duration close, duration far, bad keyword, rank, from songs, "official",
/// "lyrics", extra words.
pub fn features(track: &Track, cand: &Candidate, rank: usize, from_songs: bool) -> [f64; FEATURES] {
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    let name = base_name(&track.name);
    let artist = if track.artist.is_empty() { track.artists.first().map_or("", String::as_str) } else { &track.artist };
    let title = tokens(cand.title);
    let diff = cand.duration.map_or(0.0, |d| (d - track.duration_ms / 1000.0).abs());
    let uploader = norm(cand.uploader);
    let artist_norm = norm(artist);
    let ours = tokens(&track.name);
    let extra = title.len() as f64 - tokens(name).len() as f64 - tokens(artist).len() as f64;
    [
        1.0,
        overlap(name, cand.title),
        overlap(artist, cand.title),
        flag(!uploader.is_empty() && (uploader == artist_norm || uploader == format!("{artist_norm} topic"))),
        flag(cand.uploader.to_ascii_lowercase().ends_with(" - topic")),
        flag(cand.duration.is_some()),
        if cand.duration.is_some() { (1.0 - diff / 12.0).max(0.0) } else { 0.0 },
        flag(cand.duration.is_some() && diff > 12.0),
        flag(BAD.iter().any(|k| title.iter().any(|t| t == k) && !ours.iter().any(|t| t == k))),
        1.0 / (1.0 + rank as f64),
        flag(from_songs),
        flag(title.iter().any(|t| t == "official")),
        flag(title.iter().any(|t| t == "lyrics" || t == "lyric")),
        (extra.max(0.0) / 8.0).min(1.0),
    ]
}
