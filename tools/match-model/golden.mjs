// Writes the golden file the Rust port of match.mjs is checked against:
// crates/ahg-engine/tests/golden/bridge.json. Cases come from the collected
// examples (data/, not in git), so the file holds real titles in many scripts.
//
//   node tools/match-model/golden.mjs
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { features, norm, verify } from './match.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const DATA = path.join(HERE, 'data');
const lines = (f) => fs.readFileSync(path.join(DATA, f), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));
const examples = fs.readdirSync(DATA).filter((f) => /^examples(-\d+)?\.jsonl$/.test(f)).sort().flatMap(lines);

const track = (t) => ({ name: t.name, artist: t.artist, artists: t.artists, durationMs: t.durationMs });
const text = new Set();
const verdicts = [];
const rows = [];
examples.forEach((ex, i) => {
  // Every verdict that is not a plain confirmation, and a fifth of the rest.
  for (const look of Object.values(ex.lookups)) {
    const verdict = verify(ex.track, look);
    if (verdict === 'confirmed' && i % 5) continue;
    verdicts.push({ track: track(ex.track), look: { duration: look.duration, track: look.track, artist: look.artist, creator: look.creator }, verdict });
  }
  if (i % 6) return;
  [ex.track.name, ex.track.artist, ...ex.plain.slice(0, 3).map((c) => c.title)].forEach((s) => text.add(s));
  const cands = [...ex.songs.map((c, rank) => ({ c, rank, fromSongs: true })), ...ex.plain.map((c, rank) => ({ c, rank, fromSongs: false }))];
  for (const { c, rank, fromSongs } of cands.filter((_, k) => k % 3 === 0)) {
    rows.push({ track: track(ex.track), cand: { title: c.title, uploader: c.uploader, duration: c.duration }, rank, fromSongs, x: features(ex.track, c, rank, fromSongs) });
  }
});
// Hand-made edges the collected data may not hold.
for (const s of ['', '   ', '(only a note)', 'A (b) [c] D', 'Ünïcödé – ÉCOLE', '日本語 の タイトル', 'İstanbul', 'a_b-c.d', 'x (unclosed', 'one\n(two\nlines) three']) text.add(s);

const out = { norm: [...text].map((input) => ({ input, output: norm(input) })), verdicts, features: rows };
const file = path.join(HERE, '..', '..', 'crates', 'ahg-engine', 'tests', 'golden', 'bridge.json');
fs.writeFileSync(file, JSON.stringify(out, null, 1) + '\n');
const tally = verdicts.reduce((t, v) => ({ ...t, [v.verdict]: (t[v.verdict] ?? 0) + 1 }), {});
console.log(`bridge.json: ${out.norm.length} texts, ${verdicts.length} verdicts ${JSON.stringify(tally)}, ${rows.length} feature rows, ${(fs.statSync(file).size / 1024).toFixed(0)} KB`);
