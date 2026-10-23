import { writeFileSync, renameSync, rmSync } from 'node:fs';

/** Write via `<path>.tmp` then rename, so a crash or full disk never leaves a
 *  truncated file at `path`. Throws on failure (callers decide to swallow); the
 *  tmp file is cleaned up and the original left intact. */
export function writeFileAtomic(path: string, data: string): void {
  const tmp = `${path}.tmp`;
  try {
    writeFileSync(tmp, data, 'utf-8');
    renameSync(tmp, path);
  } catch (err) {
    try { rmSync(tmp, { force: true }); } catch { /* nothing to clean */ }
    throw err;
  }
}
