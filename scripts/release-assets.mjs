// Writes SHA256SUMS.txt next to the built exe(s) in release/.
// The in-app updater refuses any download whose hash is not published here, so this
// asset must be uploaded alongside the exe or clients simply will not update.
//
//   node scripts/release-assets.mjs [dir]     (default: release)
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dir = process.argv[2] ?? 'release';
const exes = readdirSync(dir).filter((f) => f.toLowerCase().endsWith('.exe'));

if (exes.length === 0) {
  console.error(`No .exe found in ${dir}/ — build first (npm run dist).`);
  process.exit(1);
}

const lines = exes.map((name) => {
  const hash = createHash('sha256').update(readFileSync(path.join(dir, name))).digest('hex');
  console.log(`${hash}  ${name}`);
  return `${hash}  ${name}`;
});

const out = path.join(dir, 'SHA256SUMS.txt');
writeFileSync(out, lines.join('\n') + '\n', 'utf-8');
console.log(`\nwrote ${out}`);
