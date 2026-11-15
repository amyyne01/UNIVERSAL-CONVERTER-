// Trains and compares scorers for the Spotify bridge on data/examples.jsonl.
//
// Three scorers are compared on tracks none of them was fitted to:
//   - the hand-set scorer the Electron app ships (its own scoreCandidates),
//   - a single-layer weighted model (logistic regression),
//   - a small network with one hidden layer.
//
// A candidate's label comes from YouTube's own metadata for that video (a full
// lookup), compared with the Spotify track. Only looked-up candidates have a
// label. Labels are a rule over richer data than the scorers see, not a human
// verdict; the numbers below mean "agrees with YouTube's metadata".
//
// Run from this checkout's root (plain Node is fine, nothing here depends on its version):
//   node tools/match-model/train.mjs
import fs from 'node:fs';
import { register } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ELECTRON = path.resolve(process.env.AHG_ELECTRON ?? path.join(HERE, '..', '..', '..', 'converter-electron'));
const examples = fs.readFileSync(path.join(HERE, 'data', 'examples.jsonl'), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));

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

// ── Labels, from the full lookup of a candidate ─────────────────────────────
/** 1 = this video is the track, 0 = it is not, null = cannot tell from metadata. */
function label(track, look) {
  if (!look || look.duration == null) return null;
  const diff = Math.abs(look.duration - track.durationMs / 1000);
  const name = baseName(track.name);
  const titleSays = overlap(name, look.track ?? look.title) >= 0.99;
  const who = [look.artist, look.creator, look.uploader, look.channel, look.title].filter(Boolean).join(' ');
  const artistSays = track.artists.some((a) => overlap(a, who) >= 0.99) || overlap(track.artist, who) >= 0.99;
  const wrongVersion = BAD.some((k) => tokens(look.title).includes(k) && !tokens(track.name).includes(k));
  if (diff > 12 || !titleSays || wrongVersion) return 0;
  if (!artistSays) return null;
  return diff <= 3 ? 1 : null;
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

// One row per candidate of every track; `y` is null where there was no lookup or no verdict.
const tracks = examples.map((ex) => {
  const cands = [
    ...ex.songs.map((c, i) => ({ c, rank: i, fromSongs: true })),
    ...ex.plain.map((c, i) => ({ c, rank: i, fromSongs: false })),
  ].filter(({ c }) => c.id);
  return { ex, rows: cands.map(({ c, rank, fromSongs }) => ({ id: c.id, fromSongs, x: features(ex.track, c, rank, fromSongs), y: label(ex.track, ex.lookups[c.id]) })) };
}).filter((t) => t.rows.length);

// ── A fixed split by track, so no track is in both halves ───────────────────
let seed = 20261115;
const random = () => { seed = (seed * 48271) % 2147483647; return seed / 2147483647; };
const order = tracks.map((t, i) => [random(), i]).sort((a, b) => a[0] - b[0]).map(([, i]) => tracks[i]);
const cut = Math.floor(order.length * 0.8);
const [trainSet, testSet] = [order.slice(0, cut), order.slice(cut)];
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
  if (!scoreCandidates) return null;
  const track = { ...t.ex.track, thumbnailUrl: '', trackNumber: 0 };
  const cands = t.ex.plain.map((c) => ({ id: c.id, title: c.title, uploader: c.uploader, duration: c.duration ?? 0, thumbnailUrl: '', url: c.url, source: 'youtube' }));
  const best = scoreCandidates(track, cands)[0];
  return best ? t.rows.find((r) => r.id === best.candidate.id && !r.fromSongs) ?? null : null;
}

// ── Evaluation ──────────────────────────────────────────────────────────────
/** For each track: is the scorer's first choice a confirmed match? */
function judge(name, set, pick) {
  let right = 0, wrong = 0, unknown = 0, none = 0;
  for (const t of set) {
    const r = pick(t);
    if (!r) none++; else if (r.y === 1) right++; else if (r.y === 0) wrong++; else unknown++;
  }
  const known = right + wrong;
  return { scorer: name, tracks: set.length, right, wrong, noVerdict: unknown, noPick: none, accuracyWhereKnown: known ? +(right / known).toFixed(3) : null };
}
const best = (model) => (t) => t.rows.reduce((a, r) => (model.score(r.x) > model.score(a.x) ? r : a));
/** How well the score separates confirmed matches from confirmed non-matches. */
function separation(model, rows) {
  const pos = rows.filter((r) => r.y === 1).map((r) => model.score(r.x));
  const neg = rows.filter((r) => r.y === 0).map((r) => model.score(r.x));
  if (!pos.length || !neg.length) return null;
  let wins = 0;
  for (const p of pos) for (const q of neg) wins += p > q ? 1 : p === q ? 0.5 : 0;
  return +(wins / (pos.length * neg.length)).toFixed(3);
}

const trainRows = labelled(trainSet);
const testRows = labelled(testSet);
const logistic = fitLogistic(trainRows);
const network = fitNetwork(trainRows);

const count = (rows) => ({ rows: rows.length, matches: rows.filter((r) => r.y === 1).length, nonMatches: rows.filter((r) => r.y === 0).length });
console.log(`tracks: ${tracks.length} (train ${trainSet.length}, test ${testSet.length})`);
console.log('labelled candidates, train:', count(trainRows), 'test:', count(testRows));
const allLooked = tracks.flatMap((t) => t.rows.filter((r) => t.ex.lookups[r.id]));
console.log(`looked-up candidates with no verdict: ${allLooked.filter((r) => r.y === null).length} of ${allLooked.length}`);
console.table([
  judge('hand-set (Electron)', testSet, handPick),
  judge('first YouTube Music song', testSet, (t) => t.rows.find((r) => r.fromSongs) ?? null),
  judge('first plain result', testSet, (t) => t.rows.find((r) => !r.fromSongs) ?? null),
  judge('single-layer', testSet, best(logistic)),
  judge('network', testSet, best(network)),
]);
console.log('separation on held-out candidates (1 = perfect, 0.5 = chance):', { singleLayer: separation(logistic, testRows), network: separation(network, testRows) });
console.log('single-layer weights:', Object.fromEntries(FEATURES.map((f, i) => [f, +logistic.w[i].toFixed(2)])));
fs.writeFileSync(path.join(HERE, 'data', 'models.json'), JSON.stringify({ features: FEATURES, logistic: logistic.w, network: { w1: network.w1, w2: network.w2 }, tracks: tracks.length }, null, 1));
