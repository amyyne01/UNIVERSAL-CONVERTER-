import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { initLog, logLine, redact, LOG_CAP_BYTES } from './log';

let dir: string;
beforeEach(() => { dir = mkdtempSync(path.join(tmpdir(), 'log-')); initLog(dir); });
afterEach(() => { rmSync(dir, { recursive: true, force: true }); });

describe('log', () => {
  it('rotates at the cap: current + one previous file, both bounded', () => {
    const line = 'x'.repeat(1000);
    for (let i = 0; i < 1500; i++) logLine('info', line); // ~1.5 MB
    expect(readdirSync(dir).sort()).toEqual(['ahg.log', 'ahg.log.1']);
    expect(statSync(path.join(dir, 'ahg.log')).size).toBeLessThan(LOG_CAP_BYTES + 2000);
    expect(statSync(path.join(dir, 'ahg.log.1')).size).toBeLessThan(LOG_CAP_BYTES + 2000);
  });

  it('never writes tokens, auth headers or cookies', () => {
    const secrets = [
      'ghp_' + 'abcdefghijklmnopqrstuvwxyz0123456789',
      'github_pat_11ABCDEFG0abcdefghij_klmnop',
      'Bearer abc.def.ghi',
      'Cookie: SID=supersecret; HSID=other',
      'https://x.test/a?access_token=topsecret&b=1',
      '{"githubToken":"zzzsecretzzz"}',
    ];
    for (const s of secrets) logLine('error', `boom ${s}`);
    const out = readFileSync(path.join(dir, 'ahg.log'), 'utf-8');
    for (const bad of ['ghp_abc', 'github_pat_11', 'abc.def.ghi', 'supersecret', 'topsecret', 'zzzsecretzzz']) {
      expect(out).not.toContain(bad);
    }
    expect(redact('plain failure text')).toBe('plain failure text');
  });
});
