// Writes the golden file the Rust port of match.mjs is checked against:
// crates/ahg-engine/tests/golden/bridge.json. Cases come from the collected
// examples (data/, not in git), so the file holds real titles in many scripts.
//
//   node tools/match-model/golden.mjs
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { SONG_SEARCH, norm, pickRow, rowAgrees, songRows, verify } from './match.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const DATA = path.join(HERE, 'data');
const lines = (f) => fs.readFileSync(path.join(DATA, f), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));
const examples = fs.readdirSync(DATA).filter((f) => /^examples(-\d+)?\.jsonl$/.test(f)).sort().flatMap(lines);

const track = (t) => ({ name: t.name, artist: t.artist, artists: t.artists, durationMs: t.durationMs });
const text = new Set();
const verdicts = [];
examples.forEach((ex, i) => {
  // Every verdict that is not a plain confirmation, and a fifth of the rest.
  for (const look of Object.values(ex.lookups)) {
    const verdict = verify(ex.track, look);
    if (verdict === 'confirmed' && i % 5) continue;
    verdicts.push({ track: track(ex.track), look: { duration: look.duration, track: look.track, artist: look.artist, creator: look.creator }, verdict });
  }
  if (i % 6) return;
  [ex.track.name, ex.track.artist, ...ex.plain.slice(0, 3).map((c) => c.title)].forEach((s) => text.add(s));
});
// Hand-made edges the collected data may not hold.
for (const s of ['', '   ', '(only a note)', 'A (b) [c] D', 'Ünïcödé – ÉCOLE', '日本語 の タイトル', 'İstanbul', 'a_b-c.d', 'x (unclosed', 'one\n(two\nlines) three']) text.add(s);

// Rows of the direct YouTube Music song search, as saved by the measurement runs.
const byId = new Map(examples.map((ex) => [ex.track.id, ex]));
const searchRows = [];
const picks = [];
for (const f of fs.readdirSync(DATA).filter((x) => x.startsWith('direct-search'))) {
  for (const s of JSON.parse(fs.readFileSync(path.join(DATA, f), 'utf8'))) {
    const t = byId.get(s.trackId).track;
    const five = s.rows.slice(0, 5);
    for (const explicit of [s.explicit, null]) picks.push({ track: track(t), rows: five, explicit, pick: pickRow(t, five, explicit) });
    s.rows.slice(0, 5).forEach((r) => searchRows.push({ track: track(t), row: { title: r.title, artist: r.artist, duration: r.duration }, agrees: rowAgrees(t, r) }));
  }
}

// Real responses of the song search (capture-search.mjs), with the rows read from them.
const searches = JSON.parse(fs.readFileSync(path.join(DATA, 'ytm-responses.json'), 'utf8')).map(({ query, response }) => ({ query, body: SONG_SEARCH.body(query), response, rows: songRows(response) }));

const out = { searchUrl: SONG_SEARCH.url, searches, picks, rows: searchRows, norm: [...text].map((input) => ({ input, output: norm(input) })), verdicts };
const file = path.join(HERE, '..', '..', 'crates', 'ahg-engine', 'tests', 'golden', 'bridge.json');
fs.writeFileSync(file, JSON.stringify(out, null, 1) + '\n');
const tally = verdicts.reduce((t, v) => ({ ...t, [v.verdict]: (t[v.verdict] ?? 0) + 1 }), {});
console.log(`bridge.json: ${out.norm.length} texts, ${verdicts.length} verdicts ${JSON.stringify(tally)}, ${searches.length} searches, ${picks.length} picks (${picks.filter((x) => x.pick).length} found, ${picks.filter((x) => x.pick && !x.pick.explicitAgrees).length} with a different explicit flag), ${searchRows.length} search rows (${searchRows.filter((r) => r.agrees).length} agree), ${(fs.statSync(file).size / 1024).toFixed(0)} KB`);
