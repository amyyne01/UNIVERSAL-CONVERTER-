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

/** The separate artists in a credit ("A, B & C"), normalised, without a leading "the". */
export const people = (list) => list.flatMap((a) => String(a ?? '').split(/\s*,\s*|\s+&\s+|\s+x\s+/i)).map((a) => norm(a).replace(/^the /, '')).filter(Boolean);
// A one-word name must match whole ("Nas" is not "Lil Nas X"); a longer one may sit inside a longer credit.
const within = (hay, needle) => { if (hay === needle) return true; const n = needle.split(' '); const h = new Set(hay.split(' ')); return n.length >= 2 && n.every((w) => h.has(w)); };
/**
 * The two credits name the same people: every artist on one side is on the
 * other. YouTube often credits only the main artist and Spotify all of them,
 * so either side may be the shorter one. One shared name is not enough: a
 * composer is shared by every performer of the piece.
 */
export const sameArtist = (ours, theirs) => {
  const a = people(ours);
  const b = people(theirs);
  const inside = (few, many) => few.length > 0 && few.every((x) => many.some((y) => within(y, x) || within(x, y)));
  return inside(a, b) || inside(b, a);
};
/** YouTube's title adds no version note of its own ("(2016 Remaster)", "(With Horns Remix)"). */
const noNewNote = (track, title) => { const ours = new Set(tokens(String(track.name).replace(/[()[\]]/g, ' '))); return notes(title).every((w) => ours.has(w)); };

/**
 * Does this row of a YouTube Music song search name the Spotify track? Rows
 * carry title, artist and length, so most matches are settled here, before
 * any slow lookup.
 */
export function rowAgrees(track, row) {
  if (row.duration == null || !sameArtist(track.artists.length ? track.artists : [track.artist], [row.artist])) return false;
  const diff = Math.abs(row.duration - track.durationMs / 1000);
  if (overlap(baseName(track.name), row.title) >= 0.99) return diff <= 3.5 || (diff <= 5.5 && noNewNote(track, row.title));
  // Spotify puts the composer in front of a classical title ("Ravel: Pavane ..."); YouTube does not.
  return tokens(row.title).length >= 3 && overlap(row.title, track.name) >= 0.99 && diff <= 2;
}

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
  const artist = sameArtist(track.artists.length ? track.artists : [track.artist], [look.artist, look.creator]);
  if (name && artist) {
    if (diff <= 3) return 'confirmed';
    if (diff > 12) return 'rejected';
    // A few seconds apart is the same recording only when YouTube's title adds
    // no version note of its own ("(2016 Remaster)", "(With Horns Remix)").
    return diff <= 5 && noNewNote(track, look.track) ? 'confirmed' : 'unknown';
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
