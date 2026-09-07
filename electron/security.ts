import { app, session } from 'electron';
import type { IpcMainInvokeEvent } from 'electron';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

/**
 * Confine a renderer-supplied path to an allowed root (HOW-THE-APP-WORKS §15).
 * Resolves to absolute and confirms the result sits inside `root`; returns the
 * absolute path on success, or null if it escapes (…/.., UNC, absolute elsewhere).
 * The trust boundary depends on this: `shell.*` must never act on an unconfined
 * renderer path, or a compromised renderer could launch arbitrary executables.
 */
export function confineToRoot(candidate: string, root: string): string | null {
  const absRoot = path.resolve(root);
  const abs = path.resolve(absRoot, candidate);
  if (abs !== absRoot && !abs.startsWith(absRoot + path.sep)) return null;
  return abs;
}

const MAX_URL = 2048; // classic URL length ceiling — mirrors ipc.ts's own MAX_URL.

/** The exact untrusted-input guard ipc.ts applies to a pasted URL before it
 *  reaches detectUrl or anything that spawns (§15): a bounded string that
 *  can't open with "-" and be read as a spawned flag. Shared here so the
 *  ahg:// protocol handler (electron/protocol.ts) validates an OS-delivered
 *  link exactly as strictly as a renderer-pasted one, instead of growing a
 *  second copy of the same check. */
export function urlArg(value: unknown, maxLen = MAX_URL): string {
  if (typeof value !== 'string' || value.length > maxLen) {
    throw new Error('Invalid input: expected a string within length limits');
  }
  if (value.startsWith('-')) throw new Error('Invalid input: URL must not start with "-"');
  return value;
}

// ── IPC trust boundary: sender attestation + rate limiting (Seam #45) ─────────
// secure() wraps every invoke handler so each channel — current and future —
// inherits these two checks. The renderer is a fixed SPA served from the app's
// own origin; an invoke from a nested/injected sub-frame or an off-origin
// document is hostile and rejected before any handler logic runs.

/** True only for the app's own top-frame document URL. Mirrors window.ts's
 *  navigation guard exactly: packaged → the bundled dist/index.html file URL
 *  (bare, or with a #hash / ?query for client routing); dev → the Vite origin
 *  the window is loaded from in main.ts (`http://localhost:5173`). */
function isAppFrameUrl(url: string): boolean {
  if (app.isPackaged) {
    const indexUrl = pathToFileURL(path.join(app.getAppPath(), 'dist', 'index.html')).href;
    return url === indexUrl || url.startsWith(indexUrl + '#') || url.startsWith(indexUrl + '?');
  }
  try { return new URL(url).origin === 'http://localhost:5173'; } catch { return false; }
}

/** Attest that an IPC invoke came from the app's own MAIN frame. Rejects a
 *  missing sender frame, a nested sub-frame (`frame.top !== frame`), or an
 *  off-origin document. Throws before the handler runs — this is the boundary. */
export function assertTrustedSender(event: IpcMainInvokeEvent): void {
  const frame = event.senderFrame;
  if (!frame) throw new Error('IPC rejected: no sender frame');
  if (frame.top !== frame) throw new Error('IPC rejected: sender is not the main frame');
  if (!isAppFrameUrl(frame.url)) throw new Error('IPC rejected: sender is off-origin');
}

// Per-channel sliding-window call log. In-memory, one process, one desktop
// client — a plain Map is the whole store.
const rateBuckets = new Map<string, number[]>();

/** Sliding-window rate limit for a channel: prunes hits older than windowMs, then
 *  admits the call only while the window holds fewer than maxPerWindow.
 *  A REJECTED call is not recorded — counting it would let a burst keep pushing its
 *  own window forward, so the lockout outlived the burst by however long the caller
 *  kept retrying instead of draining after windowMs. */
export function rateLimit(channel: string, maxPerWindow: number, windowMs = 60000): boolean {
  const now = Date.now();
  const hits = (rateBuckets.get(channel) ?? []).filter((t) => now - t < windowMs);
  const admitted = hits.length < maxPerWindow;
  if (admitted) hits.push(now);
  rateBuckets.set(channel, hits);
  return admitted;
}

/** Wrap an ipcMain.handle callback so it inherits sender attestation, then a
 *  per-channel rate check, before delegating. Guards invoke handlers only —
 *  never webContents.send. */
export function secure<A extends unknown[], R>(
  channel: string,
  limitPerMin: number,
  handler: (event: IpcMainInvokeEvent, ...args: A) => R,
): (event: IpcMainInvokeEvent, ...args: A) => R {
  return (event, ...args) => {
    assertTrustedSender(event);
    if (!rateLimit(channel, limitPerMin)) {
      throw new Error(`IPC rate limit exceeded: ${channel}`);
    }
    return handler(event, ...args);
  };
}

// HOW-THE-APP-WORKS §15: constrain what the renderer may load and connect to.
// Applied in packaged builds only — the Vite dev server needs a looser policy
// (inline scripts, eval, ws: HMR), so dev is left alone.
const CSP = [
  "default-src 'self'",
  "script-src 'self'",
  "style-src 'self' 'unsafe-inline'",
  "font-src 'self' data:",
  "img-src 'self' data: https:",
  "media-src 'self' https:",
  "connect-src 'self'",
  "object-src 'none'",
  "base-uri 'self'",
  "frame-src 'none'",
].join('; ');

export function installContentSecurityPolicy(): void {
  session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
    callback({
      responseHeaders: {
        ...details.responseHeaders,
        'Content-Security-Policy': [CSP],
      },
    });
  });
}
