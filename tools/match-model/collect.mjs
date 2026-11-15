// Collects examples for the Spotify match scorer.
//
// For about 500 Spotify tracks across genres it records, per track, what the
// app's two searches return and what YouTube itself says about the leading
// candidates. Metadata only: nothing is downloaded. Output is one JSON line per
// track in data/examples.jsonl, and a rerun skips tracks already there.
//
// A later batch takes different songs: MATCH_BATCH=2 writes examples-2.jsonl
// and leaves out every song an earlier batch holds. `--fill` looks up the
// videos train.mjs listed in data/needed-lookups.json (its scorers' own picks).
//
// It uses the Electron app's own credential-free Spotify reader, from the
// Electron checkout (`../converter-electron` by default, or AHG_ELECTRON; see
// scripts/rust-golden.mjs for how to set that up). Run from this checkout's root:
//   ELECTRON_RUN_AS_NODE=1 ../converter-electron/node_modules/electron/dist/electron.exe tools/match-model/collect.mjs
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import { register } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(HERE, '..', '..');
const DATA = path.join(HERE, 'data');
const BATCH = Number(process.env.MATCH_BATCH ?? 1);
const OUT = path.join(DATA, BATCH === 1 ? 'examples.jsonl' : `examples-${BATCH}.jsonl`);
const YTDLP = path.join(ROOT, 'bin', 'yt-dlp.exe');
const TARGET = Number(process.env.MATCH_TRACKS ?? 500);
const WORKERS = 3;
const PAUSE_MS = 700;

// Public playlists chosen for spread across genres, eras and languages.
const PLAYLISTS = [
  '37i9dQZF1DXcBWIGoYBM5M', '37i9dQZEVXbMDoHDwVN2tF', '37i9dQZF1DWXRqgorJj26U', '37i9dQZF1DX0XUsuxWHRQd', '37i9dQZF1DX10zKzsJ2jva',
  '37i9dQZF1DX4dyzvuaRJ0n', '37i9dQZF1DX1lVhptIYRda', '37i9dQZF1DX4sWSpwq3LiO', '37i9dQZF1DX9tPFwDMOaN1', '37i9dQZF1DX4UtSsGT1Sbe',
  '37i9dQZF1DX4o1oenSJRJd', '37i9dQZF1DXbITWG1ZJKYt', '37i9dQZF1DWWEJlAGA9gs0', '37i9dQZF1DX4SBhb3fqCJd', '37i9dQZF1DWYkaDif7Ztbp',
  '37i9dQZF1DWVuV87wUBNwc', '37i9dQZF1DXd8cOUiye1o2', '37i9dQZF1DWWOaP4H0w5b0', '37i9dQZF1DX2Nc3B70tvx0', '37i9dQZF1DXbSbnqxMTGx9',
  '37i9dQZF1DWWQRwui0ExPn', '37i9dQZF1DXafb0IuPwJyF', '37i9dQZF1DX5cO1uP1XC1g', '37i9dQZF1DX6z20IXmBjWI', '37i9dQZF1DWTwnEm1IYyoj',
  '37i9dQZF1DX4WYpdgoIcn6', '37i9dQZF1DXaXB8fQg7xif', '37i9dQZF1DX5Ejj0EkURtP',
];

register('data:text/javascript,' + encodeURIComponent(
  `export async function resolve(s, c, n) { return s === 'electron' ? { url: 'data:text/javascript,export const app={};export default {};', shortCircuit: true } : n(s, c); }`));
const ELECTRON = path.resolve(process.env.AHG_ELECTRON ?? path.join(ROOT, '..', 'converter-electron'));
const { SpotifyHandler } = await import(pathToFileURL(path.join(ELECTRON, 'dist-electron', 'electron', 'spotify.js')).href);
const { scoreCandidates } = await import(pathToFileURL(path.join(ELECTRON, 'dist-electron', 'electron', 'downloader.js')).href);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const readLines = (file) => (fs.existsSync(file) ? fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)) : []);

/** Runs yt-dlp in inspect mode and returns its JSON records (empty on failure or timeout). */
function ytJson(args, timeoutMs = 60_000) {
  return new Promise((resolve) => {
    const proc = spawn(YTDLP, ['--socket-timeout', '20', '--retries', '2', '--dump-json', '--no-warnings', ...args], { windowsHide: true });
    let out = '';
    const timer = setTimeout(() => { proc.kill(); }, timeoutMs);
    proc.stdout.on('data', (d) => { out += d; });
    proc.on('error', () => { clearTimeout(timer); resolve([]); });
    proc.on('close', () => {
      clearTimeout(timer);
      const items = [];
      for (const line of out.split('\n')) { try { if (line.trim()) items.push(JSON.parse(line)); } catch { /* chatter */ } }
      resolve(items);
    });
  });
}

const flat = (j) => ({ id: j.id ?? '', title: j.title ?? '', uploader: j.uploader ?? j.channel ?? '', duration: j.duration ?? null, url: j.url ?? j.webpage_url ?? '' });
/** What YouTube says about one video: the fields that identify a music upload. */
const rich = (j) => ({
  id: j.id ?? '', title: j.title ?? '', uploader: j.uploader ?? '', channel: j.channel ?? '', duration: j.duration ?? null,
  artist: j.artist ?? null, track: j.track ?? null, album: j.album ?? null, creator: j.creator ?? null, releaseYear: j.release_year ?? null,
  categories: j.categories ?? [], artTrack: /Provided to YouTube by/i.test(j.description ?? ''), viewCount: j.view_count ?? null,
});
const lookup = async (id) => (await ytJson(['--no-playlist', '--', `https://www.youtube.com/watch?v=${id}`]))[0];

/** Run `job` over `items`, a few at a time, pausing between calls. */
async function paced(items, job) {
  let next = 0;
  let finished = 0;
  await Promise.all(Array.from({ length: WORKERS }, async () => {
    for (;;) {
      const item = items[next++];
      if (!item) return;
      await job(item);
      if (++finished % 10 === 0) console.log(`${finished}/${items.length}`);
      await sleep(PAUSE_MS);
    }
  }));
  return finished;
}

fs.mkdirSync(DATA, { recursive: true });

if (process.argv.includes('--fill')) {
  const EXTRA = path.join(DATA, 'lookups-extra.jsonl');
  const have = new Set(readLines(EXTRA).map((l) => `${l.trackId} ${l.lookup.id}`));
  const todo = readLines(path.join(DATA, 'needed-lookups.json')).flat().filter((n) => !have.has(`${n.trackId} ${n.id}`));
  console.log(`${todo.length} videos to look up`);
  await paced(todo, async ({ trackId, id }) => {
    const j = await lookup(id);
    if (j) fs.appendFileSync(EXTRA, JSON.stringify({ trackId, lookup: rich(j) }) + '\n');
  });
  console.log('done');
  process.exit(0);
}

async function example(track) {
  const clean = (s) => String(s ?? '').replace(/"/g, '').trim();
  const query = `${clean(track.artist || track.artists?.[0])} - ${clean(track.name)}`;
  const [plain, songs] = await Promise.all([
    ytJson(['--flat-playlist', `ytsearch10:${query}`]),
    ytJson(['--flat-playlist', '--playlist-end', '5', `https://music.youtube.com/search?q=${encodeURIComponent(query)}#songs`]),
  ]);
  // Full lookups are the slow part (several seconds each), so not every candidate gets one:
  // the first three songs (they carry music metadata, so they give verdicts both ways),
  // the first two plain results, the Electron scorer's own pick, and one of the rest,
  // so the labelled candidates are not only the leaders.
  const hand = scoreCandidates({ ...track, thumbnailUrl: '', trackNumber: 0 },
    plain.map((j) => ({ ...flat(j), duration: j.duration ?? 0, thumbnailUrl: '', source: 'youtube' })))[0]?.candidate.id;
  const rest = [...songs.slice(3), ...plain.slice(2)];
  const other = rest[[...track.id].reduce((sum, ch) => sum + ch.charCodeAt(0), 0) % (rest.length || 1)]?.id;
  const leaders = [...new Set([...songs.slice(0, 3).map((j) => j.id), ...plain.slice(0, 2).map((j) => j.id), hand, other].filter(Boolean))];
  const lookups = {};
  for (const id of leaders) {
    await sleep(PAUSE_MS);
    const j = await lookup(id);
    if (j) lookups[id] = rich(j);
  }
  return {
    track: { id: track.id, name: track.name, artist: track.artist, artists: track.artists, album: track.album, durationMs: track.durationMs },
    query, plain: plain.map(flat), songs: songs.map(flat), lookups,
  };
}

const done = new Set(readLines(OUT).map((l) => l.track.id));
// Songs another batch holds, by id and by artist plus name: the same recording can sit under two Spotify ids.
const songKey = (t) => `${t.artist} | ${String(t.name).split(' - ')[0]}`.toLowerCase();
const elsewhere = new Set(fs.readdirSync(DATA).filter((f) => /^examples(-\d+)?\.jsonl$/.test(f) && path.join(DATA, f) !== OUT)
  .flatMap((f) => readLines(path.join(DATA, f))).flatMap((l) => [l.track.id, songKey(l.track)]));

// An even sample from each playlist, so no genre dominates.
const spotify = new SpotifyHandler();
const perList = Math.ceil(TARGET / PLAYLISTS.length) + 2;
const tracks = [];
const seen = new Set();
const sources = [];
for (const id of PLAYLISTS) {
  try {
    const p = await spotify.fetchPlaylist(id);
    const fresh = p.tracks.filter((t) => t?.id && !elsewhere.has(t.id) && !elsewhere.has(songKey(t)));
    const step = Math.max(1, Math.floor(fresh.length / perList));
    let taken = 0;
    for (let i = 0; i < fresh.length && taken < perList; i += step) {
      const t = fresh[i];
      if (!t.name || !t.durationMs || seen.has(t.id)) continue;
      seen.add(t.id);
      tracks.push({ ...t, playlist: p.name });
      taken++;
    }
    sources.push(`${p.name}: ${taken}`);
  } catch (e) {
    sources.push(`${id}: unavailable`);
  }
  await sleep(400);
}
fs.writeFileSync(path.join(DATA, BATCH === 1 ? 'sources.txt' : `sources-${BATCH}.txt`), sources.join('\n') + '\n');
const todo = tracks.slice(0, TARGET).filter((t) => !done.has(t.id));
console.log(`${tracks.length} tracks sampled from ${sources.filter((s) => !s.endsWith('unavailable')).length} playlists; ${done.size} already collected; ${todo.length} to do`);

const finished = await paced(todo, async (t) => {
  try {
    const ex = await example(t);
    ex.playlist = t.playlist;
    fs.appendFileSync(OUT, JSON.stringify(ex) + '\n');
  } catch (e) {
    console.log(`skipped ${t.artist} - ${t.name}: ${String(e.message).slice(0, 80)}`);
  }
});
console.log(`done: ${done.size + finished} examples in ${OUT}`);
process.exit(0);
