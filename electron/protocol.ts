import { app } from 'electron';
import path from 'node:path';
import { urlArg } from './security.js';
import { detectUrl } from './url-detector.js';
import type { UrlDetection } from '../shared/types.js';

// Shell integration (HOW-THE-APP-WORKS §14): an `ahg://` link opened anywhere on
// the OS is handed to THIS app rather than a browser, and — via single-instance
// forwarding in main.ts — to the one already-running window rather than a second
// copy of it.

export const PROTOCOL_SCHEME = 'ahg';
const PREFIX = `${PROTOCOL_SCHEME}://`;

/** Register as the OS handler for ahg:// links. In dev the executable IS
 *  Electron itself, so the real entry script must be passed explicitly or the
 *  OS would relaunch bare Electron with nothing to run. */
export function registerProtocolHandler(): void {
  if (process.defaultApp) {
    if (process.argv.length >= 2) {
      app.setAsDefaultProtocolClient(PROTOCOL_SCHEME, process.execPath, [path.resolve(process.argv[1])]);
    }
  } else {
    app.setAsDefaultProtocolClient(PROTOCOL_SCHEME);
  }
}

/** Pull the first ahg:// entry out of argv. Windows hands the launched (or
 *  relaunched-and-forwarded) process the link as a bare argument, both at cold
 *  start and on the 'second-instance' event — the same array shape either way. */
export function extractProtocolUrl(argv: string[]): string | null {
  return argv.find((a) => typeof a === 'string' && a.startsWith(PREFIX)) ?? null;
}

/** Untrusted OS input (§15): validated exactly like a pasted URL crossing the
 *  IPC boundary — bounded length, can't open with "-" — before it ever reaches
 *  detectUrl or anything that spawns. `null` means "not a link this app can
 *  use", never a thrown error into the caller. */
export function resolveProtocolLink(raw: string): UrlDetection | null {
  if (typeof raw !== 'string' || !raw.startsWith(PREFIX)) return null;
  let target: string;
  try {
    target = urlArg(raw.slice(PREFIX.length));
  } catch {
    return null;
  }
  if (!target) return null;
  const detection = detectUrl(target);
  return detection.platform === 'unknown' ? null : detection;
}
