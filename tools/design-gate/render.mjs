// Renders every state of the design gate for the three directions and builds
// the comparison page: docs/design-gate/index.html, a plain file to open in a
// browser. Build the app first (cargo build -p ahg-app), or one direction on its own
// (cargo build -p ahg-app --no-default-features --features shelf --target-dir target/gate-shelf), then:
//
//   node tools/design-gate/render.mjs            all three directions
//   node tools/design-gate/render.mjs shelf      one direction only (the page is rebuilt from what exists)
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
/** A direction built on its own (`--target-dir target/gate-<id>`) is used when it exists and is newer; else the full build. */
const exeFor = (id) => {
  const own = path.join(ROOT, 'target', `gate-${id}`, 'debug', 'AHG-Converter.exe');
  const all = path.join(ROOT, 'target', 'debug', 'AHG-Converter.exe');
  const age = (f) => (fs.existsSync(f) ? fs.statSync(f).mtimeMs : 0);
  return age(own) >= age(all) ? own : all;
};
const OUT = path.join(ROOT, 'docs', 'design-gate');
const DIRECTIONS = [
  ['shelf', 'Shelf', 'A record collection: cover art leads, warm paper-and-ink neutrals. Instrument Sans, IBM Plex Mono, a serif headline.'],
  ['signal', 'Signal', 'A transfer desk: meters and timecode, cool graphite. Geist and Geist Mono.'],
  ['orbit', 'Orbit', "The planet logo's world: a download coming in to land, deep ink. Space Grotesk and Space Mono."],
];
const SCENES = [
  ['home-first', 'Home, first launch'],
  ['home-history', 'Home, with history'],
  ['downloads-empty', 'Downloads, empty'],
  ['downloads-active', 'Downloads, in progress'],
  ['downloads-finished', 'Downloads, finished'],
  ['downloads-failed', 'Downloads, problems'],
  ['downloads-everything', 'Downloads, a mixed day'],
];
// Two of the app's existing palettes, to show a direction survives a switch.
const PALETTE_PROOFS = [2, 5];

const only = process.argv[2];
const shots = [];
for (const [id] of DIRECTIONS.filter(([d]) => !only || d === only)) {
  const dir = path.join(OUT, 'shots', id);
  fs.mkdirSync(dir, { recursive: true });
  const snap = (name, args) => {
    const file = path.join(dir, `${name}.png`);
    const r = spawnSync(exeFor(id), ['--direction', id, ...args, '--snap', file], { encoding: 'utf8', windowsHide: true });
    shots.push(`${id}/${name}: ${r.status === 0 && fs.existsSync(file) ? 'ok' : `FAILED ${(r.stderr ?? '').trim().split('\n').pop()?.slice(0, 120)}`}`);
  };
  for (const [scene] of SCENES) for (const theme of ['dark', 'light']) snap(`${scene}-${theme}`, ['--scene', scene, '--theme', theme]);
  for (const palette of PALETTE_PROOFS) snap(`palette-${palette}`, ['--scene', 'downloads-everything', '--theme', 'dark', '--palette', String(palette)]);
}
console.log(shots.join('\n'));

// ── The page: one row per state, the three directions side by side ──────────
const img = (id, name) => {
  const rel = `shots/${id}/${name}.png`;
  return fs.existsSync(path.join(OUT, rel)) ? `<a href="${rel}" target="_blank"><img loading="lazy" src="${rel}" alt="${id} ${name}"></a>` : '<div class="missing">not rendered</div>';
};
const row = (title, name) => `<section><h2>${title}</h2><div class="three">${DIRECTIONS.map(([id]) => `<figure>${img(id, name)}</figure>`).join('')}</div></section>`;
const html = `<!doctype html>
<meta charset="utf-8">
<title>AHG Universal Converter: design gate</title>
<style>
  :root { color-scheme: dark; }
  body { margin: 0; padding: 32px; background: #101012; color: #e9e9ec; font: 15px/1.5 system-ui, sans-serif; }
  h1 { font-size: 24px; font-weight: 600; margin: 0 0 4px; }
  p.lead { color: #a0a0a8; margin: 0 0 28px; max-width: 78ch; }
  .heads, .three { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }
  .heads { position: sticky; top: 0; background: #101012; padding: 12px 0; z-index: 1; border-bottom: 1px solid #26262b; }
  .heads h3 { margin: 0; font-size: 16px; }
  .heads p { margin: 2px 0 0; color: #a0a0a8; font-size: 13px; }
  section { margin: 28px 0 0; }
  h2 { font-size: 13px; font-weight: 500; color: #a0a0a8; margin: 0 0 8px; }
  figure { margin: 0; }
  img { width: 100%; display: block; border-radius: 8px; border: 1px solid #26262b; }
  .missing { aspect-ratio: 1240 / 820; display: grid; place-items: center; border: 1px dashed #3a3a42; border-radius: 8px; color: #70707a; }
</style>
<h1>Design gate: Home and Downloads, three ways</h1>
<p class="lead">The same prepared downloads in every direction. Click a picture to open it full size. The last rows show each direction in two of the app's existing palettes.</p>
<div class="heads">${DIRECTIONS.map(([, name, about]) => `<div><h3>${name}</h3><p>${about}</p></div>`).join('')}</div>
${SCENES.flatMap(([scene, title]) => [row(`${title} · dark`, `${scene}-dark`), row(`${title} · light`, `${scene}-light`)]).join('\n')}
${PALETTE_PROOFS.map((p) => row(`A mixed day in existing palette ${p} · dark`, `palette-${p}`)).join('\n')}
`;
fs.writeFileSync(path.join(OUT, 'index.html'), html);
console.log(`page: ${path.join(OUT, 'index.html')}`);
