// Report-only WCAG check for every palette x theme. `node scripts/check-contrast.mjs`
// Exits 1 on any failure: <4.5:1 for text, <3:1 for UI (accent edge, control border).
import { readFileSync } from 'node:fs';

const css = readFileSync('src/index.css', 'utf8') + '\n' + readFileSync('src/palettes.css', 'utf8');

function parseVars(body) {
  const v = {};
  for (const m of body.matchAll(/--color-([\w-]+):\s*oklch\(([^)]*)\)/g)) v[m[1]] = m[2];
  return v;
}
const block = (sel) => {
  const i = css.indexOf(sel + ' {');
  return i < 0 ? null : parseVars(css.slice(i, css.indexOf('}', i)));
};
// Aurora: base @theme (dark) and :root[data-theme="light"] (light), second occurrence wins for dark.
const base = { dark: parseVars(css.slice(css.indexOf('@theme {'), css.indexOf('}', css.indexOf('@theme {')))), light: block(':root[data-theme="light"]') };
const ids = [...new Set([...css.matchAll(/\[data-palette="(\w+)"\]/g)].map((m) => m[1]))];

const parse = (s) => { const [l, c, h] = s.split('/')[0].trim().split(/\s+/).map(Number); return { l, c, h, a: s.includes('/') ? Number(s.split('/')[1]) : 1 }; };
function toRgb({ l, c, h }) {
  const a = c * Math.cos((h * Math.PI) / 180), b = c * Math.sin((h * Math.PI) / 180);
  const L = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3, M = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3, S = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [4.0767416621 * L - 3.3077115913 * M + 0.2309699292 * S, -1.2684380046 * L + 2.6097574011 * M - 0.3413193965 * S, -0.0041960863 * L - 0.7034186147 * M + 1.707614701 * S].map((x) => Math.min(1, Math.max(0, x)));
}
const lum = (s) => { const [r, g, b] = toRgb(parse(s)); return 0.2126 * r + 0.7152 * g + 0.0722 * b; }; // already linear
const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };

const REQUIRED = ['bg-secondary', 'bg-primary', 'bg-tertiary', 'bg-hover', 'bg-surface', 'bg-glass', 'scrim', 'text-primary', 'text-secondary', 'text-muted', 'accent', 'accent-hover', 'accent-deep', 'on-accent', 'border', 'border-soft', 'border-control'];
const SURF = ['bg-primary', 'bg-secondary', 'bg-tertiary', 'bg-surface', 'bg-hover'];
const failures = [];
let checked = 0;
const check = (name, tok, a, b, min) => {
  checked++;
  const r = ratio(a, b);
  if (r < min) failures.push(`${name}: ${tok} ${r.toFixed(2)} < ${min}`);
};

for (const id of ['aurora', ...ids]) for (const mode of ['dark', 'light']) {
  const own = id === 'aurora' ? {} : block(`:root[data-palette="${id}"][data-theme="${mode}"]`);
  const name = `${id}/${mode}`;
  if (!own) { failures.push(`${name}: block missing`); continue; }
  if (id !== 'aurora') for (const k of REQUIRED) if (!(k in own)) failures.push(`${name}: token --color-${k} not overridden`);
  const t = { ...base[mode], ...own };
  for (const s of SURF) {
    for (const x of ['text-primary', 'text-secondary', 'text-muted', 'accent', 'success', 'warning', 'error', 'youtube', 'spotify', 'soundcloud', 'reels'])
      check(name, `${x} on ${s}`, t[x], t[s], 4.5);
  }
  check(name, 'on-accent on accent', t['on-accent'], t.accent, 4.5);
  check(name, 'on-accent on accent-hover', t['on-accent'], t['accent-hover'], 4.5);
  for (const s of ['bg-primary', 'bg-tertiary', 'bg-surface']) {
    check(name, `accent(UI) on ${s}`, t.accent, t[s], 3);
    if (t['border-control']) check(name, `border-control on ${s}`, t['border-control'], t[s], 3);
  }
}
console.log(`${checked} pairs checked across ${ids.length + 1} palettes x 2 themes`);
console.log(failures.length ? 'FAILURES:\n' + failures.join('\n') : 'zero failures');
process.exit(failures.length ? 1 : 0);
