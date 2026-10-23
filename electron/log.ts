import { appendFileSync, statSync, renameSync, rmSync } from 'node:fs';
import path from 'node:path';

// Small rotating crash/diagnostic log in userData: ahg.log + ahg.log.1, ~512 KB each.
// Everything is redacted on the way in — a log the user may attach to a bug report
// must never carry a token, cookie or auth header.
export const LOG_CAP_BYTES = 512 * 1024;
const TAIL_CHARS = 2000;

let file = '';

export function initLog(dir: string): void {
  file = path.join(dir, 'ahg.log');
}

/** Masks credentials: GitHub tokens, Bearer/Basic auth, cookie headers, token-ish query/JSON values. */
export function redact(text: string): string {
  return text
    .replace(/\b(?:gh[pousr]_|github_pat_)[A-Za-z0-9_]{10,}/g, '[redacted]')
    .replace(/\b(Bearer|Basic)\s+[A-Za-z0-9._~+/=-]+/gi, '$1 [redacted]')
    .replace(/\b((?:set-)?cookie|authorization)\s*[:=]\s*[^\r\n]*/gi, '$1: [redacted]')
    .replace(/((?:access_|refresh_|api_)?(?:token|secret|key|sig|password)["']?\s*[=:]\s*["']?)[^&\s"',}]+/gi, '$1[redacted]');
}

export function logLine(level: 'info' | 'error', msg: string): void {
  if (!file) return;
  try {
    try {
      if (statSync(file).size >= LOG_CAP_BYTES) {
        rmSync(`${file}.1`, { force: true });
        renameSync(file, `${file}.1`);
      }
    } catch { /* no log yet */ }
    appendFileSync(file, `${new Date().toISOString()} [${level}] ${redact(msg)}\n`, 'utf-8');
  } catch { /* logging must never throw */ }
}

/** Last chunk of a long error (e.g. yt-dlp stderr) — the failure reason sits at the end. */
export const tail = (s: string): string => (s.length > TAIL_CHARS ? `...${s.slice(-TAIL_CHARS)}` : s);

/** Host only — a log line never carries a full URL, its path or its query. */
export const hostOf = (u: string): string => { try { return new URL(u).host; } catch { return ''; } };
export const scrubUrls = (s: string): string => s.replace(/\b[a-z][a-z0-9+.-]*:\/\/[^\s"'<>]+/gi, hostOf);

export const errText =(e: unknown): string => (e instanceof Error ? e.stack || e.message : String(e));
