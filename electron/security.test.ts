import { describe, it, expect, vi } from 'vitest';
import path from 'node:path';

// assertTrustedSender reads app.isPackaged/getAppPath via security.ts. Dev branch
// (isPackaged:false) attests against the Vite origin — mirrors main.ts's loadURL.
vi.mock('electron', () => ({
  app: { isPackaged: false, getAppPath: vi.fn(() => 'C:\\app') },
  session: {},
}));

import { confineToRoot, assertTrustedSender, rateLimit, secure, urlArg, mediaUrlArg, CSP, playlistItemsArg, isNetscapeCookies } from './security';

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

  it('does not record rejected calls, so a burst cannot extend its own lockout', () => {
    const ch = 'test:ratelimit:noselfextend:' + Math.random();
    expect(rateLimit(ch, 1, 60)).toBe(true); // t≈0, admitted
    return new Promise<void>((resolve) => {
      setTimeout(() => {
        expect(rateLimit(ch, 1, 60)).toBe(false); // t≈40, rejected — must not be recorded
      }, 40);
      setTimeout(() => {
        // t≈80: the one admitted hit has aged out. If the rejection at t≈40 had been
        // recorded it would still be inside the window and the caller would stay
        // locked out purely for having retried.
        expect(rateLimit(ch, 1, 60)).toBe(true);
        resolve();
      }, 80);
    });
  });
});

// The exact untrusted-URL guard ipc.ts applies to a pasted link — reused by
// electron/protocol.ts so an ahg:// link from the OS is checked the same way.
describe('urlArg', () => {
  it('accepts an ordinary http(s) URL', () => {
    expect(urlArg('https://youtube.com/watch?v=abc12345678')).toBe('https://youtube.com/watch?v=abc12345678');
  });

  it('rejects a URL carrying a line break', () => {
    expect(() => urlArg('https://x.test/v.mp4\n[FILEPATH]C:/Windows/win.ini')).toThrow(/single line/);
  });

  it('rejects a non-string', () => {
    expect(() => urlArg(42)).toThrow(/string within length limits/);
  });

  it('rejects a URL over the length ceiling', () => {
    expect(() => urlArg('https://x.com/' + 'a'.repeat(3000))).toThrow(/length limits/);
  });

  it('rejects a value starting with "-" (spawned-flag smuggling)', () => {
    expect(() => urlArg('--exec=calc')).toThrow(/must not start with/);
  });

  it('honors a custom max length', () => {
    expect(() => urlArg('https://x.com/abc', 10)).toThrow(/length limits/);
  });
});

// A4: everything that spawns yt-dlp takes http(s) only, plus the Spotify bridge query.
describe('mediaUrlArg', () => {
  it.each(['file:///C:/Windows/win.ini', 'htp:/x', 'javascript:alert(1)', 'https://x.com/' + 'a'.repeat(5000), '', 'C:\\x.mp4'])(
    'rejects %j', (u) => {
      expect(() => mediaUrlArg(u)).toThrow(/Invalid input/);
    });
  it.each(['https://youtube.com/watch?v=abc', 'http://example.com/v.mp4', 'ytsearch:a - b'])('accepts %j', (u) => {
    expect(mediaUrlArg(u)).toBe(u);
  });
});

// A5: the packaged renderer talks to main over IPC only — it may open no connection.
describe('CSP', () => {
  it('allows the renderer no network connection at all', () => {
    expect(CSP).toContain("connect-src 'none'");
    expect(CSP).not.toContain("connect-src 'self'");
  });
});

describe('secure', () => {
  it('attests then rate-limits then delegates; a burst past the limit is rejected', () => {
    // The arg is typed so secure()'s rest-parameter generic infers [string] —
    // a bare `() => 'ok'` infers [] and every wrapped(event, arg) call is a type error.
    const inner = vi.fn((_event: any, _arg: string) => 'ok');
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

describe('playlistItemsArg', () => {
  it('accepts numbers, ranges and lists', () => {
    for (const ok of ['1', '1,3-5', '2-4,9,11-12']) expect(playlistItemsArg(ok)).toBe(ok);
  });
  it('rejects injection, empty, non-strings and over-length', () => {
    for (const bad of ['1;rm', '', '-1', '1,', ',1', '1-', 'a', '1 2', '--exec', 5, null]) {
      expect(() => playlistItemsArg(bad), String(bad)).toThrow();
    }
    expect(() => playlistItemsArg('1,'.repeat(200) + '1')).toThrow();
  });
});

describe('isNetscapeCookies', () => {
  const row = ['.a.com', 'TRUE', '/', 'TRUE', '0', 'k', 'v'].join('	');
  it('accepts a header plus 7-field rows', () => {
    expect(isNetscapeCookies(`# Netscape HTTP Cookie File
${row}
`)).toBe(true);
  });
  it('rejects empty, comment-only, malformed, NUL and oversized files', () => {
    expect(isNetscapeCookies('')).toBe(false);
    expect(isNetscapeCookies('# only a comment')).toBe(false);
    expect(isNetscapeCookies(`${row}
not a cookie line`)).toBe(false);
    expect(isNetscapeCookies(`${row} `)).toBe(false);
    expect(isNetscapeCookies(row.repeat(300_000))).toBe(false);
  });
});
