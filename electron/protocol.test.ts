import { describe, it, expect, vi } from 'vitest';

vi.mock('electron', () => ({
  app: { setAsDefaultProtocolClient: vi.fn() },
}));

import { app } from 'electron';
import { registerProtocolHandler, extractProtocolUrl, resolveProtocolLink } from './protocol';

describe('extractProtocolUrl', () => {
  it('finds an ahg:// entry among ordinary argv', () => {
    expect(extractProtocolUrl(['C:\\app.exe', 'ahg://https://youtu.be/abc12345678'])).toBe(
      'ahg://https://youtu.be/abc12345678',
    );
  });

  it('returns null when argv carries no protocol link', () => {
    expect(extractProtocolUrl(['C:\\app.exe', '--some-flag'])).toBeNull();
  });

  it('ignores non-string entries without throwing', () => {
    expect(extractProtocolUrl(['C:\\app.exe', undefined as any, 'ahg://x.com'])).toBe('ahg://x.com');
  });
});

describe('resolveProtocolLink', () => {
  it('unwraps a real link and detects its platform', () => {
    const d = resolveProtocolLink('ahg://https://youtube.com/watch?v=abc12345678');
    expect(d?.platform).toBe('youtube');
    expect(d?.url).toBe('https://youtube.com/watch?v=abc12345678');
  });

  it('rejects a payload the detector cannot place (stays unknown)', () => {
    expect(resolveProtocolLink('ahg://not a link at all')).toBeNull();
  });

  it('rejects anything not carrying the ahg:// prefix', () => {
    expect(resolveProtocolLink('https://youtube.com/watch?v=abc12345678')).toBeNull();
  });

  it('rejects a payload starting with "-" (spawned-flag smuggling)', () => {
    expect(resolveProtocolLink('ahg://-exec=calc')).toBeNull();
  });

  it('rejects an oversized payload instead of throwing', () => {
    expect(resolveProtocolLink('ahg://' + 'a'.repeat(3000))).toBeNull();
  });

  it('rejects an empty payload', () => {
    expect(resolveProtocolLink('ahg://')).toBeNull();
  });
});

describe('registerProtocolHandler', () => {
  it('registers the scheme directly when not running as the bare Electron binary', () => {
    (process as any).defaultApp = false;
    registerProtocolHandler();
    expect(app.setAsDefaultProtocolClient).toHaveBeenCalledWith('ahg');
  });

  it('passes the script path explicitly in dev (process.defaultApp)', () => {
    (process as any).defaultApp = true;
    const originalArgv = process.argv;
    process.argv = ['C:\\electron.exe', 'C:\\app\\main.js'];
    registerProtocolHandler();
    expect(app.setAsDefaultProtocolClient).toHaveBeenCalledWith('ahg', process.execPath, [
      require('node:path').resolve('C:\\app\\main.js'),
    ]);
    process.argv = originalArgv;
    (process as any).defaultApp = false;
  });
});
