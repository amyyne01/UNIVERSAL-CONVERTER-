// Saves a few real responses of YouTube Music's song search, cut down to the
// parts the app reads, as cases for the golden file: data/ytm-responses.json.
//
//   node tools/match-model/capture-search.mjs
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { SONG_SEARCH } from './match.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const queries = ['Cardi B - WAP', 'Fleetwood Mac - Dreams', 'Kenshi Yonezu - Lemon', 'Bertrand Chamayou - Pavane pour une infante défunte', 'zzqqxx no such song qqzzxx'];
const out = [];
for (const query of queries) {
  const r = await fetch(SONG_SEARCH.url, { method: 'POST', headers: { 'Content-Type': 'application/json', Origin: 'https://music.youtube.com', 'User-Agent': 'Mozilla/5.0' }, body: JSON.stringify(SONG_SEARCH.body(query)) });
  const items = [];
  (function walk(o) {
    if (!o || typeof o !== 'object') return;
    if (o.musicResponsiveListItemRenderer) { const { flexColumns, badges, playlistItemData } = o.musicResponsiveListItemRenderer; items.push({ musicResponsiveListItemRenderer: { flexColumns, badges, playlistItemData } }); }
    for (const v of Object.values(o)) walk(v);
  })(await r.json());
  // Nested as the real response nests them, but without the bulk around them.
  out.push({ query, response: { contents: { tabbedSearchResultsRenderer: { tabs: [{ tabRenderer: { content: { sectionListRenderer: { contents: [{ musicShelfRenderer: { contents: items.slice(0, 8) } }] } } } }] } } } });
  console.log(`${query}: ${items.length} rows`);
}
fs.writeFileSync(path.join(HERE, 'data', 'ytm-responses.json'), JSON.stringify(out));
