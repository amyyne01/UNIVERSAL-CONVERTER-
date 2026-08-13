import { describe, it, expect, vi } from 'vitest';
import path from 'node:path';

// assertTrustedSender reads app.isPackaged/getAppPath via security.ts. Dev branch
// (isPackaged:false) attests against the Vite origin — mirrors main.ts's loadURL.
vi.mock('electron', () => ({
  app: { isPackaged: false, getAppPath: vi.fn(() => 'C:\\app') },
  session: {},
}));

import { confineToRoot, assertTrustedSender, rateLimit, secure } from './security';

// A valid app main frame: dev origin, top === itself.
const mainFrameEvent = (url = 'http://localhost:5173/') => {
  const frame: any = { url };
  frame.top = frame;
  return { senderFrame: frame } as any;
};

// confineToRoot is the sole guard keeping shell:showItemInFolder from acting on a
// path outside the output dir (the trust boundary). Exercise its edge cases directly
// rather than only indirectly through ipc.test.ts. Windows-only app → win32 paths.
describe('confineToRoot', () => {
  const ROOT = 'C:\\Downloads';

  it('accepts a file directly inside the root', () => {
    expect(confineToRoot('song.mp3', ROOT)).toBe(path.resolve(ROOT, 'song.mp3'));
  });

  it('accepts a nested file inside the root', () => {
    expect(confineToRoot('sub\\song.mp3', ROOT)).toBe(path.resolve(ROOT, 'sub\\song.mp3'));
  });

  it('accepts an already-absolute path that sits inside the root', () => {
    expect(confineToRoot('C:\\Downloads\\song.mp3', ROOT)).toBe('C:\\Downloads\\song.mp3');
  });

  it('accepts the root itself', () => {
    expect(confineToRoot('.', ROOT)).toBe(path.resolve(ROOT));
  });

  it('rejects .. traversal that escapes the root', () => {
    expect(confineToRoot('..\\Windows\\System32\\cmd.exe', ROOT)).toBeNull();
  });

  it('rejects a sibling dir that shares the root as a name prefix', () => {
    // C:\Downloads-evil must not pass the C:\Downloads guard.
    expect(confineToRoot('..\\Downloads-evil\\x.mp3', ROOT)).toBeNull();
  });

  it('rejects an absolute path on a different drive', () => {
    expect(confineToRoot('D:\\Music\\x.mp3', ROOT)).toBeNull();
  });

  it('rejects a UNC path', () => {
    expect(confineToRoot('\\\\server\\share\\x.mp3', ROOT)).toBeNull();
  });
});

// Seam #45 — the secure() IPC wrapper: sender attestation + per-channel rate limit.
describe('assertTrustedSender', () => {
  it('accepts the app main frame on its own origin', () => {
    expect(() => assertTrustedSender(mainFrameEvent())).not.toThrow();
  });

  it('rejects a missing sender frame', () => {
    expect(() => assertTrustedSender({ senderFrame: null } as any)).toThrow(/no sender frame/);
  });

  it('rejects a spoofed sub-frame (top !== self) even on the app origin', () => {
    const top: any = { url: 'http://localhost:5173/' };
    top.top = top;
    const sub = { url: 'http://localhost:5173/', top }; // nested frame
    expect(() => assertTrustedSender({ senderFrame: sub } as any)).toThrow(/main frame/);
  });

  it('rejects a wrong-origin main frame', () => {
    expect(() => assertTrustedSender(mainFrameEvent('http://evil.example/'))).toThrow(/off-origin/);
    // A look-alike userinfo trick must not pass the origin check either.
    expect(() => assertTrustedSender(mainFrameEvent('http://localhost:5173@evil.com/'))).toThrow(/off-origin/);
  });
});

describe('rateLimit', () => {
  it('allows up to maxPerWindow then rejects the overflow', () => {
    const ch = 'test:ratelimit:' + Math.random();
    for (let i = 0; i < 3; i++) expect(rateLimit(ch, 3)).toBe(true);
    expect(rateLimit(ch, 3)).toBe(false); // 4th within the window
  });

  it('prunes hits older than the window so the counter recovers', () => {
    const ch = 'test:ratelimit:prune:' + Math.random();
    expect(rateLimit(ch, 1, 50)).toBe(true);
    expect(rateLimit(ch, 1, 50)).toBe(false);
    return new Promise<void>((resolve) => setTimeout(() => {
      expect(rateLimit(ch, 1, 50)).toBe(true); // old hit aged out
      resolve();
    }, 60));
  });
});

describe('secure', () => {
  it('attests then rate-limits then delegates; a burst past the limit is rejected', () => {
    const inner = vi.fn(() => 'ok');
    const wrapped = secure('test:secure:burst:' + Math.random(), 2, inner);
    expect(wrapped(mainFrameEvent(), 'a')).toBe('ok');
    expect(wrapped(mainFrameEvent(), 'b')).toBe('ok');
    expect(() => wrapped(mainFrameEvent(), 'c')).toThrow(/rate limit exceeded/);
    expect(inner).toHaveBeenCalledTimes(2); // the rejected 3rd call never reached the handler
  });

  it('rejects a spoofed sender before the rate check or the handler runs', () => {
    const inner = vi.fn();
    const wrapped = secure('test:secure:spoof:' + Math.random(), 100, inner);
    const sub = { url: 'http://localhost:5173/', top: { url: 'http://localhost:5173/' } };
    expect(() => wrapped({ senderFrame: sub } as any)).toThrow(/main frame/);
    expect(inner).not.toHaveBeenCalled();
  });
});
