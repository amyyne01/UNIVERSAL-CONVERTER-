// Self-update — HOW-THE-APP-WORKS §12 (consult the release feed, download over an
// encrypted connection from trusted hosts only) + §9 (trusted-host gate).
//
// Why this is hand-rolled rather than electron-updater: electron-updater's Windows
// path is NsisUpdater, which downloads an installer and executes it. This app ships
// as a single portable exe and never an installer (see CLAUDE.md), and the library
// has no handling for that target at all — so it could never apply an update here.
// The flow it replaces is identical from the renderer's side: check → available →
// download with progress → "Restart to install".
//
// Applying an update means replacing the exe that is currently running. Windows will
// not let a running image be overwritten, so install() hands the swap to a detached
// script that outlives us, retries until the file unlocks, then relaunches.
//
// Activation is unaffected by any of this: the licence record and the machine id live
// in userData (%APPDATA%), which no part of the swap touches.
import { app } from 'electron';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import path from 'node:path';
import type { BrowserWindow } from 'electron';
import type { UpdateState } from '../shared/types.js';

const RELEASE_API = 'https://api.github.com/repos/amyyne01/UNIVERSAL-CONVERTER-/releases/latest';
/** Published beside the exe by scripts/release-assets.mjs; the update is refused without it. */
const SUMS_ASSET = 'SHA256SUMS.txt';

// §9/§12: releases ship as GitHub assets. GitHub serves the API from api.github.com
// and redirects asset bytes to objects.githubusercontent.com; anything else is
// refused outright. Exported so the engine updater reuses this one allowlist.
export const TRUSTED_HOSTS = new Set(['github.com', 'objects.githubusercontent.com', 'api.github.com']);
const GITHUB_BASE = 'https://github.com/';

/** §9/§12: a release URL is trusted only when it's https on an allowlisted host.
 *  Relative asset urls resolve against github.com (fail-closed on parse error). */
export function isTrustedReleaseHost(urlStr: string, base = GITHUB_BASE): boolean {
  try {
    const url = new URL(urlStr, base);
    return url.protocol === 'https:' && TRUSTED_HOSTS.has(url.hostname);
  } catch {
    return false;
  }
}

/** Numeric-segment compare; anything unparsable sorts as 0 so junk never looks newer. */
export function isNewerVersion(candidate: string, current: string): boolean {
  const parts = (v: string) => v.replace(/^v/, '').split('.').map((n) => parseInt(n, 10) || 0);
  const a = parts(candidate);
  const b = parts(current);
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const d = (a[i] ?? 0) - (b[i] ?? 0);
    if (d !== 0) return d > 0;
  }
  return false;
}

interface Available {
  version: string;
  assetName: string;
  assetUrl: string;
  sumsUrl: string;
}

interface UpdaterDeps {
  getWindow: () => BrowserWindow | null;
  /** Injected in tests. */
  fetchFn?: typeof fetch;
  currentVersion?: () => string;
  /** The portable exe the user actually launched — the file to replace. */
  targetExe?: () => string | null;
  updateDir?: string;
  spawnFn?: typeof spawn;
  quit?: () => void;
}

export class Updater {
  private available: Available | null = null;
  private downloaded: string | null = null;
  private state: UpdateState = { phase: 'idle' };

  constructor(private readonly deps: UpdaterDeps) {}

  /** The current state, readable at any time — see UpdateState on why this exists. */
  getState(): UpdateState {
    return this.state;
  }

  /** Ask the feed what the newest release is. Never throws — failures are events. */
  async check(): Promise<void> {
    this.state = { phase: 'checking' };
    try {
      const found = await this.latestRelease();
      if (!found || !isNewerVersion(found.version, this.currentVersion())) {
        this.available = null;
        this.emit({ phase: 'uptodate' }, 'update:not-available', {});
        return;
      }
      this.available = found;
      this.emit({ phase: 'available', version: found.version }, 'update:available', { version: found.version });
    } catch (err) {
      this.emit({ phase: 'error', message: message(err) }, 'update:error', { message: message(err) });
    }
  }

  /** Fetch the new exe, verify it against the published checksum, keep it in userData. */
  async download(): Promise<void> {
    const target = this.available;
    if (!target) throw new Error('Update refused: no available update to download');
    try {
      const expected = parseChecksum(await this.fetchText(target.sumsUrl), target.assetName);
      if (!expected) throw new Error('Update refused: no checksum published for this release');

      const bytes = await this.fetchWithProgress(target.assetUrl);
      const actual = createHash('sha256').update(bytes).digest('hex');
      if (actual !== expected.toLowerCase()) {
        throw new Error('Update refused: the download did not match its published checksum');
      }

      const dir = this.updateDir();
      mkdirSync(dir, { recursive: true });
      const file = path.join(dir, target.assetName);
      writeFileSync(file, bytes);
      this.downloaded = file;
      this.emit({ phase: 'ready', version: target.version }, 'update:downloaded', { version: target.version });
    } catch (err) {
      this.emit({ phase: 'error', message: message(err) }, 'update:error', { message: message(err) });
    }
  }

  /**
   * Restart into the new version. The running exe cannot overwrite itself, so a
   * detached script waits for this process to release the file, swaps it, and
   * starts it again.
   */
  install(): void {
    const staged = this.downloaded;
    if (!staged) throw new Error('Update refused: nothing downloaded to install');
    const exe = this.targetExe();
    if (!exe) {
      throw new Error('Update refused: this build cannot replace itself — download the new version instead');
    }

    const script = path.join(this.updateDir(), 'apply-update.cmd');
    writeFileSync(script, swapScript(staged, exe), 'utf-8');
    const spawnFn = this.deps.spawnFn ?? spawn;
    spawnFn('cmd.exe', ['/c', script], { detached: true, stdio: 'ignore', windowsHide: true }).unref();
    (this.deps.quit ?? (() => app.quit()))();
  }

  /** Clear a staged download that a previous run already applied. */
  cleanup(): void {
    const dir = this.updateDir();
    if (existsSync(dir)) rmSync(dir, { recursive: true, force: true });
  }

  // ── internals ──────────────────────────────────────────────────────────────

  private async latestRelease(): Promise<Available | null> {
    const json = (await this.fetchJson(RELEASE_API)) as {
      tag_name?: string;
      assets?: { name?: string; browser_download_url?: string }[];
    };
    const version = String(json.tag_name ?? '').replace(/^v/, '').trim();
    const assets = json.assets ?? [];
    const exe = assets.find((a) => (a.name ?? '').toLowerCase().endsWith('.exe'));
    const sums = assets.find((a) => a.name === SUMS_ASSET);
    if (!version || !exe?.browser_download_url || !exe.name || !sums?.browser_download_url) return null;
    // Fail-closed host gate BEFORE a single byte is fetched (§9/§12).
    for (const url of [exe.browser_download_url, sums.browser_download_url]) {
      if (!isTrustedReleaseHost(url)) throw new Error(`Update refused: untrusted host "${url}"`);
    }
    return {
      version,
      assetName: exe.name,
      assetUrl: exe.browser_download_url,
      sumsUrl: sums.browser_download_url,
    };
  }

  /** Streams so the renderer gets a real percentage, not a spinner for 120 MB. */
  private async fetchWithProgress(url: string): Promise<Buffer> {
    const res = await this.request(url);
    const total = Number(res.headers.get('content-length')) || 0;
    const body = res.body;
    if (!body) return Buffer.from(await res.arrayBuffer());

    const reader = body.getReader();
    const chunks: Uint8Array[] = [];
    let received = 0;
    let lastSent = -1;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value);
      received += value.length;
      if (total > 0) {
        const percent = Math.floor((received / total) * 100);
        // Only on change: a 120 MB download would otherwise flood the bridge.
        if (percent !== lastSent) {
          lastSent = percent;
          this.emit({ phase: 'downloading', percent, version: this.available?.version }, 'update:progress', { percent });
        }
      }
    }
    return Buffer.concat(chunks);
  }

  private async request(url: string): Promise<Response> {
    if (!isTrustedReleaseHost(url)) throw new Error(`Update refused: untrusted host "${url}"`);
    const fetchFn = this.deps.fetchFn ?? fetch;
    const res = await fetchFn(url, { headers: { 'User-Agent': 'AHG-Universal-Converter' } });
    if (!res.ok) throw new Error(`Update failed: HTTP ${res.status}`);
    return res;
  }

  private async fetchJson(url: string): Promise<unknown> {
    return (await this.request(url)).json();
  }

  private async fetchText(url: string): Promise<string> {
    return (await this.request(url)).text();
  }

  private currentVersion(): string {
    return this.deps.currentVersion ? this.deps.currentVersion() : app.getVersion();
  }

  /**
   * The portable launcher extracts the app to a temp dir and runs it from there, so
   * process.execPath is NOT the file the user double-clicked — electron-builder puts
   * that one in PORTABLE_EXECUTABLE_FILE. Absent it (dev, or an unpacked build),
   * there is no single file to swap and install() refuses.
   */
  private targetExe(): string | null {
    if (this.deps.targetExe) return this.deps.targetExe();
    return process.env.PORTABLE_EXECUTABLE_FILE || null;
  }

  private updateDir(): string {
    return this.deps.updateDir ?? path.join(app.getPath('userData'), 'updates');
  }

  /** One place: record the state AND push the event, so a late subscriber that
   *  reads getState() sees exactly what an early subscriber was told. */
  private emit(state: UpdateState, channel: string, payload: unknown): void {
    this.state = state;
    this.deps.getWindow()?.webContents.send(channel, payload);
  }
}

/** Checksum line lookup, `<sha256>  <filename>`, tolerant of the `*name` binary marker. */
export function parseChecksum(sums: string, name: string): string | null {
  for (const line of sums.split(/\r?\n/)) {
    const m = line.trim().match(/^([a-fA-F0-9]{64})\s+\*?(.+)$/);
    if (m && m[2].trim() === name) return m[1].toLowerCase();
  }
  return null;
}

/**
 * Retries the move for ~60s: the swap can only succeed once this process has fully
 * exited and released the image, and that is not instant. Deletes itself last so a
 * failed update never leaves a script that runs again on the next boot.
 */
export function swapScript(staged: string, target: string): string {
  return [
    '@echo off',
    'setlocal',
    `set "SRC=${staged}"`,
    `set "DST=${target}"`,
    'for /l %%i in (1,1,30) do (',
    '  move /y "%SRC%" "%DST%" >nul 2>&1 && goto done',
    '  ping -n 3 127.0.0.1 >nul',
    ')',
    'goto cleanup',
    ':done',
    'start "" "%DST%"',
    ':cleanup',
    '(goto) 2>nul & del "%~f0"',
    '',
  ].join('\r\n');
}

function message(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
