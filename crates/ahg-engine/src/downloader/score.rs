//! Ranking the YouTube search results for a Spotify track (§11). Ports
//! scoreCandidates from electron/downloader.ts; the arithmetic is kept in the
//! same order so the scores match to the last digit.
use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::types::{SearchResult, Track};

/// Beyond this many seconds apart a candidate is rejected.
const MAX_DURATION_DELTA_SEC: f64 = 12.0;
/// A full 12 seconds off (but not rejected) costs this.
const DURATION_PENALTY_WEIGHT: f64 = 0.4;
/// " - Topic" channel, or the artist's own channel.
const UPLOADER_BOOST: f64 = 0.3;
/// Per bad keyword in the title but not in the Spotify title.
const KEYWORD_PENALTY: f64 = 0.5;
const BAD_KEYWORDS: [&str; 6] = ["live", "cover", "remix", "sped", "8d", "nightcore"];

#[derive(Debug, Clone, PartialEq)]
pub struct ScoredCandidate {
    pub candidate: SearchResult,
    pub score: f64,
}

/// Lowercase word tokens (Unicode letters and digits), punctuation dropped.
fn tokenize(s: &str) -> Vec<String> {
    static NOT_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\p{L}\p{N}]+").expect("constant pattern"));
    // Only letters and digits survive the replacement, so the one separator left is the space.
    NOT_WORD.replace_all(&s.to_lowercase(), " ").split(' ').filter(|t| !t.is_empty()).map(str::to_owned).collect()
}

/// Rank candidates for a track, best first. Candidates more than 12 seconds off
/// (when both durations are known) are dropped; a missing duration is neutral.
pub fn score_candidates(track: &Track, candidates: &[SearchResult]) -> Vec<ScoredCandidate> {
    let track_sec = if track.duration_ms > 0.0 { track.duration_ms / 1000.0 } else { 0.0 };
    let both = format!("{} {}", track.artist, track.name);
    let wanted = tokenize(&both);
    let spotify_text = both.to_lowercase();
    let artist_tokens = tokenize(&track.artist);
    let mut out = Vec::new();

    for c in candidates {
        let mut duration_penalty = 0.0;
        if c.duration > 0.0 && track_sec > 0.0 {
            let delta = (c.duration - track_sec).abs();
            if delta > MAX_DURATION_DELTA_SEC {
                continue;
            }
            duration_penalty = (delta / MAX_DURATION_DELTA_SEC) * DURATION_PENALTY_WEIGHT;
        }

        let title_set: HashSet<String> = tokenize(&c.title).into_iter().collect();
        let overlap = if wanted.is_empty() {
            0.0
        } else {
            wanted.iter().filter(|t| title_set.contains(*t)).count() as f64 / wanted.len() as f64
        };

        let uploader = c.uploader.to_lowercase();
        let uploader_boost = if uploader.ends_with("- topic")
            || (!artist_tokens.is_empty() && artist_tokens.iter().all(|t| uploader.contains(t.as_str())))
        {
            UPLOADER_BOOST
        } else {
            0.0
        };

        let mut keyword_penalty = 0.0;
        for kw in BAD_KEYWORDS {
            if title_set.contains(kw) && !spotify_text.contains(kw) {
                keyword_penalty += KEYWORD_PENALTY;
            }
        }

        out.push(ScoredCandidate {
            candidate: c.clone(),
            score: overlap + uploader_boost - duration_penalty - keyword_penalty,
        });
    }

    // Stable, like Array.prototype.sort; a NaN comparison counts as a tie.
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    out
}
