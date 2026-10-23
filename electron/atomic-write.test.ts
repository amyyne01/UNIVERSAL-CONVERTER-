import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { writeFileAtomic } from './atomic-write';

let dir: string;
beforeEach(() => { dir = mkdtempSync(path.join(tmpdir(), 'aw-')); });
afterEach(() => { rmSync(dir, { recursive: true, force: true }); });

describe('writeFileAtomic', () => {
  it('writes the file and leaves no tmp behind', () => {
    const f = path.join(dir, 'a.json');
    writeFileAtomic(f, '{"a":1}');
    expect(readFileSync(f, 'utf-8')).toBe('{"a":1}');
    expect(readdirSync(dir)).toEqual(['a.json']);
  });

  it('overwrites an existing file', () => {
    const f = path.join(dir, 'a.json');
    writeFileSync(f, 'old');
    writeFileAtomic(f, 'new');
    expect(readFileSync(f, 'utf-8')).toBe('new');
  });

  it('keeps the original intact and cleans tmp when the swap fails', () => {
    const f = path.join(dir, 'target');
    mkdirSync(f); // rename of a file onto a directory fails
    expect(() => writeFileAtomic(f, 'x')).toThrow();
    expect(readdirSync(dir)).toEqual(['target']);
  });
});
