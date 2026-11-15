// Trains and compares scorers for the Spotify bridge on data/examples*.jsonl.
//
// Scorers compared on songs none of them was fitted to:
//   - the hand-set scorer the Electron app ships (its own scoreCandidates),
//   - taking the first result of each search,
//   - a single-layer weighted model (logistic regression),
//   - a small network with one hidden layer.
//
// A candidate's label comes from the music metadata YouTube attaches to a
// video (track, artist: fields no scorer sees), found by a full lookup and
// compared with the Spotify track. A video without that metadata has no label:
// its title would be the only evidence, and the scorers read the title too.
// The length of the video is the one thing labels and scorers share, so the
// models are also fitted without it, to show how much rests on it.
// Labels are a rule, not a human verdict; the numbers mean "agrees with
// YouTube's music metadata".
//
// Run from this checkout's root (plain Node is fine, nothing here depends on its version):
//   node tools/match-model/train.mjs
import fs from 'node:fs';
import { register } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ELECTRON = path.resolve(process.env.AHG_ELECTRON ?? path.join(HERE, '..', '..', '..', 'converter-electron'));
const DATA = path.join(HERE, 'data');
const lines = (file) => (fs.existsSync(file) ? fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)) : []);
// Every batch (examples.jsonl, examples-2.jsonl, ...), or only batch MATCH_BATCH when that is set.
const ONLY = process.env.MATCH_BATCH && (process.env.MATCH_BATCH === '1' ? 'examples.jsonl' : `examples-${process.env.MATCH_BATCH}.jsonl`);
const examples = fs.readdirSync(DATA).filter((f) => (ONLY ? f === ONLY : /^examples(-\d+)?\.jsonl$/.test(f))).sort().flatMap((f) => lines(path.join(DATA, f)));
// Lookups made afterwards for the scorers' own picks (collect.mjs --fill).
const extra = new Map(lines(path.join(DATA, 'lookups-extra.jsonl')).map(({ trackId, lookup }) => [`${trackId} ${lookup.id}`, lookup]));
for (const ex of examples) {
  for (const c of [...ex.songs, ...ex.plain]) {
    const found = extra.get(`${ex.track.id} ${c.id}`);
    if (found) ex.lookups[c.id] = found;
  }
}

// ── Text helpers ────────────────────────────────────────────────────────────
const norm = (s) => String(s ?? '').toLowerCase().normalize('NFKD').replace(/[̀-ͯ]/g, '')
  .replace(/\(.*?\)|\[.*?\]/g, ' ').replace(/[^\p{L}\p{N}]+/gu, ' ').trim();
const tokens = (s) => norm(s).split(' ').filter(Boolean);
/** Spotify appends edition notes after " - " ("Dreams - 2004 Remaster"); the song's name is the part before. */
const baseName = (name) => String(name ?? '').split(' - ')[0];
const BAD = ['live', 'cover', 'remix', 'sped', '8d', 'nightcore', 'karaoke', 'instrumental', 'slowed', 'reverb'];
const overlap = (want, have) => {
  const w = tokens(want);
  if (!w.length) return 0;
  const h = new Set(tokens(have));
  return w.filter((t) => h.has(t)).length / w.length;
};

// ── Labels, from the music metadata of a looked-up candidate ────────────────
/** 1 = this video is the track, 0 = it is not, null = the metadata cannot tell. */
function label(track, look) {
  if (!look || look.duration == null || !look.track || !(look.artist || look.creator)) return null;
  const diff = Math.abs(look.duration - track.durationMs / 1000);
  const name = overlap(baseName(track.name), look.track) >= 0.99;
  const artist = track.artists.some((a) => overlap(a, `${look.artist ?? ''} ${look.creator ?? ''}`) >= 0.99);
  // Same song by the same artist: the length says whether it is this recording.
  if (name && artist) return diff <= 3 ? 1 : diff > 12 ? 0 : null;
  // A different name alone may be a translation; it counts with a second disagreement.
  if (!name && (!artist || diff > 3)) return 0;
  if (name && !artist && diff > 12) return 0;
  return null;
}

// ── Features, from what the two flat searches give for free ─────────────────
const FEATURES = ['bias', 'titleOverlap', 'artistInTitle', 'artistIsUploader', 'topicChannel', 'hasDuration', 'durationClose', 'durationFar',
  'badKeyword', 'rank', 'fromSongs', 'officialWord', 'lyricsWord', 'extraWords'];
function features(track, cand, rank, fromSongs) {
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
/** Feature positions that carry the video's length. */
const LENGTH = ['hasDuration', 'durationClose', 'durationFar'].map((f) => FEATURES.indexOf(f));
const noLength = (x) => x.map((v, i) => (LENGTH.includes(i) ? 0 : v));

// One row per candidate of every track; `y` is null where there was no lookup or no verdict.
const tracks = examples.map((ex) => {
  const cands = [
    ...ex.songs.map((c, i) => ({ c, rank: i, fromSongs: true })),
    ...ex.plain.map((c, i) => ({ c, rank: i, fromSongs: false })),
  ].filter(({ c }) => c.id);
  return { ex, rows: cands.map(({ c, rank, fromSongs }) => ({ id: c.id, fromSongs, x: features(ex.track, c, rank, fromSongs), y: label(ex.track, ex.lookups[c.id]) })) };
}).filter((t) => t.rows.length);

// ── A fixed split by song, so no song is in both halves ─────────────────────
// The same recording can sit under two Spotify ids; artist plus name keeps both on one side.
let seed = 20261115;
const random = () => { seed = (seed * 48271) % 2147483647; return seed / 2147483647; };
const songKey = (t) => `${norm(t.ex.track.artist)}|${norm(baseName(t.ex.track.name))}`;
const songs = new Map();
for (const t of tracks) songs.set(songKey(t), [...(songs.get(songKey(t)) ?? []), t]);
const order = [...songs.values()].map((group) => [random(), group]).sort((a, b) => a[0] - b[0]).map(([, group]) => group);
const cut = Math.floor(order.length * 0.8);
const [trainSet, testSet] = [order.slice(0, cut).flat(), order.slice(cut).flat()];
const labelled = (set) => set.flatMap((t) => t.rows.filter((r) => r.y !== null));

// ── Models ──────────────────────────────────────────────────────────────────
const sigmoid = (z) => 1 / (1 + Math.exp(-z));
const dot = (w, x) => w.reduce((s, wi, i) => s + wi * x[i], 0);

function fitLogistic(rows, epochs = 4000, rate = 0.5, decay = 1e-3) {
  const w = FEATURES.map(() => 0);
  for (let e = 0; e < epochs; e++) {
    const g = w.map(() => 0);
    for (const r of rows) { const err = sigmoid(dot(w, r.x)) - r.y; r.x.forEach((xi, i) => { g[i] += err * xi; }); }
    w.forEach((_, i) => { w[i] -= rate * (g[i] / rows.length + (i ? decay * w[i] : 0)); });
  }
  return { w, score: (x) => sigmoid(dot(w, x)) };
}

function fitNetwork(rows, hidden = 6, epochs = 6000, rate = 0.3, decay = 1e-3) {
  const n = FEATURES.length;
  const w1 = Array.from({ length: hidden }, () => Array.from({ length: n }, () => (random() - 0.5) * 0.6));
  const w2 = Array.from({ length: hidden + 1 }, () => (random() - 0.5) * 0.6);
  const forward = (x) => { const h = w1.map((row) => Math.tanh(dot(row, x))); return { h, p: sigmoid(w2[hidden] + h.reduce((s, hi, j) => s + hi * w2[j], 0)) }; };
  for (let e = 0; e < epochs; e++) {
    const g1 = w1.map((row) => row.map(() => 0));
    const g2 = w2.map(() => 0);
    for (const r of rows) {
      const { h, p } = forward(r.x);
      const err = p - r.y;
      g2[hidden] += err;
      h.forEach((hi, j) => {
        g2[j] += err * hi;
        const back = err * w2[j] * (1 - hi * hi);
        r.x.forEach((xi, i) => { g1[j][i] += back * xi; });
      });
    }
    w2.forEach((_, j) => { w2[j] -= rate * (g2[j] / rows.length + (j < hidden ? decay * w2[j] : 0)); });
    w1.forEach((row, j) => row.forEach((_, i) => { row[i] -= rate * (g1[j][i] / rows.length + decay * row[i]); }));
  }
  return { w1, w2, score: (x) => forward(x).p };
}

// The scorer the Electron app ships, on the plain search results only (its real
// behaviour). Its module imports 'electron', which plain Node answers with a stand-in.
register('data:text/javascript,' + encodeURIComponent(
  `export async function resolve(s, c, n) { return s === 'electron' ? { url: 'data:text/javascript,export const app={};export default {};', shortCircuit: true } : n(s, c); }`));
const { scoreCandidates } = await import(pathToFileURL(path.join(ELECTRON, 'dist-electron', 'electron', 'downloader.js')).href);
function handPick(t) {
  const track = { ...t.ex.track, thumbnailUrl: '', trackNumber: 0 };
  const cands = t.ex.plain.map((c) => ({ id: c.id, title: c.title, uploader: c.uploader, duration: c.duration ?? 0, thumbnailUrl: '', url: c.url, source: 'youtube' }));
  const best = scoreCandidates(track, cands)[0];
  return best ? t.rows.find((r) => r.id === best.candidate.id && !r.fromSongs) ?? null : null;
}

// ── Evaluation ──────────────────────────────────────────────────────────────
const first = (fromSongs) => (t) => t.rows.find((r) => r.fromSongs === fromSongs) ?? null;
const best = (model, view = (x) => x) => (t) => t.rows.reduce((a, r) => (model.score(view(r.x)) > model.score(view(a.x)) ? r : a));
/** How well the score separates confirmed matches from confirmed non-matches. */
function separation(model, rows, view = (x) => x) {
  const pos = rows.filter((r) => r.y === 1).map((r) => model.score(view(r.x)));
  const neg = rows.filter((r) => r.y === 0).map((r) => model.score(view(r.x)));
  if (!pos.length || !neg.length) return null;
  let wins = 0;
  for (const p of pos) for (const q of neg) wins += p > q ? 1 : p === q ? 0.5 : 0;
  return +(wins / (pos.length * neg.length)).toFixed(3);
}

const trainRows = labelled(trainSet);
const testRows = labelled(testSet);
if (!trainRows.some((r) => r.y === 1) || !trainRows.some((r) => r.y === 0)) throw new Error('nothing to learn from: the training half needs both matches and non-matches');
const blind = (rows) => rows.map((r) => ({ ...r, x: noLength(r.x) }));
const logistic = fitLogistic(trainRows);
const network = fitNetwork(trainRows);
const logisticBlind = fitLogistic(blind(trainRows));
const networkBlind = fitNetwork(blind(trainRows));

const SCORERS = [
  ['hand-set (Electron)', handPick],
  ['first YouTube Music song', first(true)],
  ['first plain result', first(false)],
  ['single-layer', best(logistic)],
  ['network', best(network)],
  ['single-layer, no length', best(logisticBlind, noLength)],
  ['network, no length', best(networkBlind, noLength)],
];

// A pick that was never looked up has no verdict. List those, so
// `collect.mjs --fill` can look them up and the next run judges every scorer.
const picks = testSet.map((t) => SCORERS.map(([, pick]) => pick(t)));
const needed = new Map();
testSet.forEach((t, i) => {
  for (const r of picks[i]) if (r && !t.ex.lookups[r.id]) needed.set(`${t.ex.track.id} ${r.id}`, { trackId: t.ex.track.id, id: r.id });
});
fs.writeFileSync(path.join(DATA, 'needed-lookups.json'), JSON.stringify([...needed.values()]));

/** Every scorer on the same tracks: those where each one's pick has a verdict. */
const judged = picks.filter((row) => row.every((r) => r && r.y !== null));
const table = SCORERS.map(([name], i) => {
  const own = picks.map((row) => row[i]);
  const right = judged.filter((row) => row[i].y === 1).length;
  return {
    scorer: name,
    right, wrong: judged.length - right, accuracy: judged.length ? +(right / judged.length).toFixed(3) : null,
    'own right': own.filter((r) => r?.y === 1).length, 'own wrong': own.filter((r) => r?.y === 0).length,
    'no verdict': own.filter((r) => r && r.y === null).length, 'no pick': own.filter((r) => !r).length,
  };
});

const count = (rows) => ({ rows: rows.length, matches: rows.filter((r) => r.y === 1).length, nonMatches: rows.filter((r) => r.y === 0).length });
console.log(`tracks: ${tracks.length} (train ${trainSet.length}, test ${testSet.length})`);
console.log('labelled candidates, train:', count(trainRows), 'test:', count(testRows));
const allLooked = tracks.flatMap((t) => t.rows.filter((r) => t.ex.lookups[r.id]));
console.log(`looked-up candidates with no verdict: ${allLooked.filter((r) => r.y === null).length} of ${allLooked.length}`);
console.log(`picks still to look up: ${needed.size} (run collect.mjs --fill, then this again)`);
console.log(`"right/wrong/accuracy": the ${judged.length} of ${testSet.length} held-out tracks where every scorer's pick has a verdict. "own": each scorer over all its picks.`);
console.table(table);
console.log('separation on held-out candidates (1 = perfect, 0.5 = chance):', {
  singleLayer: separation(logistic, testRows), network: separation(network, testRows),
  singleLayerNoLength: separation(logisticBlind, testRows, noLength), networkNoLength: separation(networkBlind, testRows, noLength),
});
console.log('single-layer weights:', Object.fromEntries(FEATURES.map((f, i) => [f, +logistic.w[i].toFixed(2)])));
fs.writeFileSync(path.join(DATA, 'models.json'), JSON.stringify({ features: FEATURES, logistic: logistic.w, network: { w1: network.w1, w2: network.w2 }, tracks: tracks.length }, null, 1));

// ── The fallback job: ranking plain videos only ─────────────────────────────
// When no song is confirmed the app falls back to ordinary videos. Compare the
// models with the hand-set scorer on held-out plain candidates that have a verdict.
{
  const hand = new Map();
  for (const t of testSet) {
    const track = { ...t.ex.track, thumbnailUrl: '', trackNumber: 0 };
    const cands = t.ex.plain.map((c) => ({ id: c.id, title: c.title, uploader: c.uploader, duration: c.duration ?? 0, thumbnailUrl: '', url: c.url, source: 'youtube' }));
    for (const s of scoreCandidates(track, cands)) hand.set(`${t.ex.track.id} ${s.candidate.id}`, s.score);
  }
  const rows = testSet.flatMap((t) => t.rows.filter((r) => !r.fromSongs && r.y !== null).map((r) => ({ ...r, hand: hand.get(`${t.ex.track.id} ${r.id}`) ?? -1 })));
  const handModel = { score: (x) => x };
  const asHand = rows.map((r) => ({ y: r.y, x: r.hand }));
  console.log('plain videos only, held-out:', count(rows), 'separation:', {
    handSet: separation(handModel, asHand), singleLayer: separation(logistic, rows), network: separation(network, rows),
  });
}

// ── How far apart in length are confirmed same-song, same-artist videos? ────
{
  const gaps = { 'up to 3s': 0, '3 to 5s': 0, '5 to 12s': 0, 'over 12s': 0 };
  const near = [];
  for (const ex of examples) {
    for (const look of Object.values(ex.lookups)) {
      if (!look?.track || look.duration == null || !(look.artist || look.creator)) continue;
      const name = overlap(baseName(ex.track.name), look.track) >= 0.99;
      const artist = ex.track.artists.some((a) => overlap(a, `${look.artist ?? ''} ${look.creator ?? ''}`) >= 0.99);
      if (!name || !artist) continue;
      const diff = Math.abs(look.duration - ex.track.durationMs / 1000);
      gaps[diff <= 3 ? 'up to 3s' : diff <= 5 ? '3 to 5s' : diff <= 12 ? '5 to 12s' : 'over 12s']++;
      if (diff > 3 && diff <= 12 && near.length < 14) near.push(`${diff.toFixed(1)}s | ${ex.track.name} [${ex.track.album}] -> ${look.track} [${look.album}]`);
    }
  }
  console.log('length gap where name and artist agree:', gaps);
  console.log(near.join('\n'));
}
