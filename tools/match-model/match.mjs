// The matching rules, in one place: the trainer fits against them, the golden
// file for the Rust port is generated from them, and
// crates/ahg-engine/src/downloader/bridge.rs is their port.

/** Lowercase, bracketed notes removed, every run of non-letters and non-digits a single space. */
export const norm = (s) => String(s ?? '').toLowerCase().replace(/\(.*?\)|\[.*?\]/g, ' ').replace(/[^\p{L}\p{N}]+/gu, ' ').trim();
export const tokens = (s) => norm(s).split(' ').filter(Boolean);
/** Spotify appends edition notes after " - " ("Dreams - 2004 Remaster"); the song's name is the part before. */
export const baseName = (name) => String(name ?? '').split(' - ')[0];
/** The share of `want`'s words found in `have`. */
export const overlap = (want, have) => {
  const w = tokens(want);
  if (!w.length) return 0;
  const h = new Set(tokens(have));
  return w.filter((t) => h.has(t)).length / w.length;
};
/** Words inside brackets: where a title carries its version ("(2016 Remaster)", "[Live]"). */
const notes = (s) => [...String(s ?? '').toLowerCase().matchAll(/\((.*?)\)|\[(.*?)\]/g)].flatMap((m) => (m[1] ?? m[2]).split(/[^\p{L}\p{N}]+/u).filter(Boolean));

export const BAD = ['live', 'cover', 'remix', 'sped', '8d', 'nightcore', 'karaoke', 'instrumental', 'slowed', 'reverb'];

/**
 * Is this looked-up video the Spotify track? Judged only from the music
 * metadata YouTube attaches (track, artist) and the length.
 * 'confirmed' | 'rejected' | 'unknown' (no metadata, or not enough to tell).
 */
export function verify(track, look) {
  if (!look || look.duration == null || !look.track || !(look.artist || look.creator)) return 'unknown';
  const diff = Math.abs(look.duration - track.durationMs / 1000);
  const name = overlap(baseName(track.name), look.track) >= 0.99;
  const artist = track.artists.some((a) => overlap(a, `${look.artist ?? ''} ${look.creator ?? ''}`) >= 0.99);
  if (name && artist) {
    if (diff <= 3) return 'confirmed';
    if (diff > 12) return 'rejected';
    // A few seconds apart is the same recording only when YouTube's title adds
    // no version note of its own ("(2016 Remaster)", "(With Horns Remix)").
    const ours = new Set(tokens(String(track.name).replace(/[()[\]]/g, ' ')));
    return diff <= 5 && notes(look.track).every((w) => ours.has(w)) ? 'confirmed' : 'unknown';
  }
  // A different name alone may be a translation; it counts with a second disagreement.
  if (!name && (!artist || diff > 3)) return 'rejected';
  if (name && !artist && diff > 12) return 'rejected';
  return 'unknown';
}

/** What the plain-video ranker sees: only what a flat search returns. */
export const FEATURES = ['bias', 'titleOverlap', 'artistInTitle', 'artistIsUploader', 'topicChannel', 'hasDuration', 'durationClose', 'durationFar',
  'badKeyword', 'rank', 'fromSongs', 'officialWord', 'lyricsWord', 'extraWords'];
export function features(track, cand, rank, fromSongs) {
  const name = baseName(track.name);
  const artist = track.artist || track.artists[0] || '';
  const titleTokens = tokens(cand.title);
  const hasDuration = cand.duration != null;
  const diff = hasDuration ? Math.abs(cand.duration - track.durationMs / 1000) : 0;
  const up = norm(cand.uploader);
  return [
    1,
    overlap(name, cand.title),
    overlap(artist, cand.title),
    up && (up === norm(artist) || up === `${norm(artist)} topic`) ? 1 : 0,
    / - topic$/i.test(cand.uploader ?? '') ? 1 : 0,
    hasDuration ? 1 : 0,
    hasDuration ? Math.max(0, 1 - diff / 12) : 0,
    hasDuration && diff > 12 ? 1 : 0,
    BAD.some((k) => titleTokens.includes(k) && !tokens(track.name).includes(k)) ? 1 : 0,
    1 / (1 + rank),
    fromSongs ? 1 : 0,
    titleTokens.includes('official') ? 1 : 0,
    titleTokens.includes('lyrics') || titleTokens.includes('lyric') ? 1 : 0,
    Math.min(1, Math.max(0, titleTokens.length - tokens(name).length - tokens(artist).length) / 8),
  ];
}
