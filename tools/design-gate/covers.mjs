// Fetches cover art for the design gate's sample downloads into
// docs/design-gate/covers/<task id>.jpg. Thumbnails only; nothing else is
// downloaded. The folder is not in git: the pictures are other people's.
//
//   node tools/design-gate/covers.mjs
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const OUT = path.join(ROOT, 'docs', 'design-gate', 'covers');
const YT = path.join(ROOT, 'bin', 'yt-dlp.exe');
fs.mkdirSync(OUT, { recursive: true });

// Task ids from crates/ahg-app/src/sample.rs, each with a search that finds a fitting picture.
const WANTED = {
  'get-lucky': 'ytsearch1:Daft Punk Get Lucky official audio',
  'big-buck-bunny': 'ytsearch1:Big Buck Bunny 60fps 4K official Blender Foundation short film',
  'tiny-desk': 'ytsearch1:Khruangbin Tiny Desk Concert NPR Music',
  'soundcloud-set': 'ytsearch1:Four Tet Boiler Room London DJ set',
  'vimeo-film': 'ytsearch1:The Last Ice Merchant documentary',
  lecture: 'ytsearch1:Feynman The Character of Physical Law lecture 1',
  dreams: 'ytsearch1:Fleetwood Mac Dreams official audio',
  humble: 'ytsearch1:Kendrick Lamar HUMBLE official audio',
  reel: 'ytsearch1:sourdough bread scoring close up',
  artemis: 'ytsearch1:NASA Artemis II around the Moon',
  cancelled: 'ytsearch1:how a mechanical watch works animation',
};

for (const [id, query] of Object.entries(WANTED)) {
  if (fs.existsSync(path.join(OUT, `${id}.jpg`))) { console.log(`${id}: already there`); continue; }
  const r = spawnSync(YT, ['--no-warnings', '--skip-download', '--write-thumbnail', '--convert-thumbnails', 'jpg', '--ffmpeg-location', path.join(ROOT, 'bin'), '-o', path.join(OUT, id), query], { encoding: 'utf8', windowsHide: true });
  console.log(`${id}: ${fs.existsSync(path.join(OUT, `${id}.jpg`)) ? 'ok' : `failed ${(r.stderr ?? '').trim().split('\n').pop()?.slice(0, 100)}`}`);
}
