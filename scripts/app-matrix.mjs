// Headless picture matrix of the whole UI: six presets x scenes x two themes.
//   node scripts/app-matrix.mjs capture <outdir> [--exe <path>]
//   node scripts/app-matrix.mjs compare <dirA> <dirB>
// Pictures are PNG (--snap) and element trees JSON (--tree), one pair per run.
import { execFile, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const defaultExe = join(root, 'target', 'debug', 'AHG-Converter.exe');
// Three classic looks (--direction) and three layouts (--layout, which wins over --direction).
const PRESETS = [
  ['signal', ['--direction', 'signal']],
  ['shelf', ['--direction', 'shelf']],
  ['orbit', ['--direction', 'orbit']],
  ['bar', ['--layout', 'bar']],
  ['collection', ['--layout', 'collection']],
  ['stage', ['--layout', 'stage']],
];
const THEMES = ['dark', 'light'];

// Runs `jobs` at a time: each worker takes the next job index until none is left.
async function capture(outdir, exe, jobs) {
  const list = spawnSync(exe, ['--list-scenes'], { encoding: 'utf8' });
  if (list.status !== 0) throw new Error(`--list-scenes failed: ${list.stderr}`);
  const scenes = list.stdout.split(/\r?\n/).filter(Boolean);
  const todo = [];
  for (const [preset, flags] of PRESETS) {
    mkdirSync(join(outdir, preset), { recursive: true });
    for (const scene of scenes) {
      for (const theme of THEMES) todo.push({ preset, flags, scene, theme });
    }
  }
  let failed = 0;
  let next = 0;
  const one = (job) => new Promise((done) => {
    const base = join(outdir, job.preset, `${job.scene}-${job.theme}`);
    const args = [...job.flags, '--scene', job.scene, '--theme', job.theme, '--snap', `${base}.png`, '--tree', `${base}.json`];
    execFile(exe, args, { encoding: 'utf8' }, (err, _out, stderr) => {
      if (err || !existsSync(`${base}.png`) || !existsSync(`${base}.json`)) {
        failed++;
        console.error(`FAILED ${job.preset} ${job.scene} ${job.theme}: ${(stderr || err?.message || '').trim()}`);
      }
      done();
    });
  });
  const worker = async () => {
    while (next < todo.length) await one(todo[next++]);
  };
  await Promise.all(Array.from({ length: jobs }, worker));
  console.log(`${todo.length} runs: ${todo.length - failed} ok, ${failed} failed (${PRESETS.length} presets x ${scenes.length} scenes x ${THEMES.length} themes, ${jobs} at a time)`);
  return failed === 0;
}

function listFiles(dir) {
  const out = [];
  const walk = (d) => {
    for (const entry of readdirSync(d, { withFileTypes: true })) {
      const path = join(d, entry.name);
      if (entry.isDirectory()) walk(path);
      else out.push(relative(dir, path));
    }
  };
  walk(dir);
  return out.sort();
}

function compare(a, b) {
  const diffs = [];
  const aFiles = listFiles(a);
  for (const rel of aFiles) {
    const other = join(b, rel);
    if (!existsSync(other)) {
      diffs.push(`missing in B: ${rel}`);
    } else if (rel.endsWith('.json')) {
      if (readFileSync(join(a, rel), 'utf8') !== readFileSync(other, 'utf8')) diffs.push(`differs (tree): ${rel}`);
    } else if (!readFileSync(join(a, rel)).equals(readFileSync(other))) {
      diffs.push(`differs (bytes): ${rel}`);
    }
  }
  for (const rel of listFiles(b)) {
    if (!existsSync(join(a, rel))) diffs.push(`extra in B: ${rel}`);
  }
  for (const line of diffs) console.log(line);
  console.log(`${aFiles.length} files compared, ${diffs.length} differences`);
  return diffs.length === 0;
}

const [mode, ...rest] = process.argv.slice(2);
// Pulls `--name value` out of the arguments; the rest stay positional.
const option = (name, fallback) => {
  const at = rest.indexOf(name);
  if (at < 0) return fallback;
  const value = rest[at + 1];
  rest.splice(at, 2);
  return value;
};
const exe = resolve(option('--exe', defaultExe));
const jobs = Number(option('--jobs', 4));
const positional = rest;

let ok;
if (mode === 'capture' && positional.length === 1 && jobs >= 1 && jobs <= 4) {
  ok = await capture(resolve(positional[0]), exe, jobs);
} else if (mode === 'compare' && positional.length === 2) {
  ok = compare(resolve(positional[0]), resolve(positional[1]));
} else {
  console.error('usage: app-matrix.mjs capture <outdir> [--exe <path>] [--jobs 1-4] | compare <dirA> <dirB>');
  process.exit(2);
}
process.exit(ok ? 0 : 1);
