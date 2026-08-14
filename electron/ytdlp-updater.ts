// #26 yt-dlp engine self-update ("Binary Doctor").
//
// The bundled bin/yt-dlp.exe sits in the read-only resourcesPath of a portable EXE,
// so it freezes the day we package. When YouTube/TikTok change their player JS every
// download starts failing until a newer yt-dlp lands — this module fetches that newer
// engine into the writable userData/bin, which downloader.ts searches FIRST (§9).
//
// Security (§9/§12): we download an executable we will later spawn, so the release
// host is checked against the ONE trusted-host allowlist (updater.ts) and the payload
// must match the SHA-256 published in the release's SHA2-256SUMS asset. Verify first,
// then swap atomically (tmp + rename). The bundled copy is never touched — it stays
// the permanent fallback, and a corrupt swap still fails --version validation.
import { createHash } from 'node:crypto';
import { mkdirSync, writeFileSync, renameSync, rmSync } from 'node:fs';
import path from 'node:path';
import { app } from 'electron';
import type { BrowserWindow } from 'electron';
import type { YtdlpUpdateStatus } from '../shared/types.js';
import { isTrustedReleaseHost, parseChecksum } from './updater.js';
import type { Downloader } from './downloader.js';

const RELEASE_API = 'https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest';
const SUMS_ASSET = 'SHA2-256SUMS';
const CHECK_TTL_MS = 24 * 60 * 60 * 1000; // §26: cache checks 24h; never block startup

const ASSET_NAME = process.platform === 'win32' ? 'yt-dlp.exe' : 'yt-dlp';

/** yt-dlp stderr that means the ENGINE is stale (site changed / extractor broke) —
 *  the exact signal that an update, not a retry, is the fix. Deliberately narrow:
 *  a false positive costs one wasted download check, so only extractor-level text. */
export function isEngineStale(text: string): boolean {
  const t = (text || '').toLowerCase();
  return [
    'unable to extract',
    'failed to extract',
    'nsig extraction failed',
    'update to the latest version',
    'yt-dlp is out of date',
    'please report this issue',
  ].some((p) => t.includes(p));
}

interface YtdlpUpdaterDeps {
  downloader: Downloader;
  getWindow: () => BrowserWindow | null;
  isAutoUpdateEnabled: () => boolean;
  /** Injected for tests; defaults to the real userData path. */
  binDir?: string;
  now?: () => number;
  fetchFn?: typeof fetch;
}

export class YtdlpUpdater {
  private status: YtdlpUpdateStatus = { current: '', latest: '', state: 'idle' };
  private lastCheck = 0;
  private busy = false;
  private reactiveTried = false; // one automatic engine update per session

  constructor(private readonly deps: YtdlpUpdaterDeps) {}

  getStatus(): YtdlpUpdateStatus {
    return this.status;
  }

  /** Consult the release feed (24h-cached unless forced) and report what's available. */
  async check(force = false): Promise<YtdlpUpdateStatus> {
    const now = this.now();
    if (!force && this.lastCheck && now - this.lastCheck < CHECK_TTL_MS) return this.status;
    if (this.busy) return this.status;
    this.busy = true;
    this.emit({ ...this.status, current: this.currentVersion(), state: 'checking' });
    try {
      const latest = await this.latestRelease();
      this.lastCheck = this.now();
      const current = this.currentVersion();
      return this.emit({
        current,
        latest: latest.tag,
        state: current && latest.tag && current === latest.tag ? 'uptodate' : 'available',
      });
    } catch (err) {
      return this.emit({ ...this.status, state: 'error', message: message(err) });
    } finally {
      this.busy = false;
    }
  }

  /** Download + checksum-verify the latest engine and swap it into userData/bin. */
  async update(): Promise<YtdlpUpdateStatus> {
    if (this.busy) return this.status;
    this.busy = true;
    try {
      this.emit({ ...this.status, state: 'downloading' });
      const latest = await this.latestRelease();
      const bytes = await this.fetchBinary(latest.assetUrl);
      const expected = parseChecksum(await this.fetchText(latest.sumsUrl), ASSET_NAME);
      if (!expected) throw new Error('Engine update refused: no published checksum');
      const actual = createHash('sha256').update(bytes).digest('hex');
      if (actual !== expected) throw new Error('Engine update refused: checksum mismatch');

      const dir = this.binDir();
      mkdirSync(dir, { recursive: true });
      const target = path.join(dir, ASSET_NAME);
      const tmp = `${target}.tmp`;
      writeFileSync(tmp, bytes);
      try {
        renameSync(tmp, target); // atomic swap; EBUSY if a download is spawned from it
      } catch (err) {
        try { rmSync(tmp, { force: true }); } catch { /* best effort */ }
        throw err;
      }
      // The new binary lives ahead of the bundled one in the search order — drop the
      // cached path so the next spawn re-resolves (and re-validates) it.
      this.deps.downloader.resetBinaryCache();
      this.lastCheck = this.now();
      return this.emit({ current: this.currentVersion(), latest: latest.tag, state: 'updated' });
    } catch (err) {
      return this.emit({ ...this.status, state: 'error', message: message(err) });
    } finally {
      this.busy = false;
    }
  }

  /** Reactive trigger: a stale-extractor failure is the signal yt-dlp itself is old.
   *  Fires at most once per session, and only when the user left auto-update on. */
  handleDownloadFailure(error: string): void {
    if (this.reactiveTried || !isEngineStale(error) || !this.deps.isAutoUpdateEnabled()) return;
    this.reactiveTried = true;
    void this.update();
  }

  // ── internals ────────────────────────────────────────────────────────────

  private async latestRelease(): Promise<{ tag: string; assetUrl: string; sumsUrl: string }> {
    const json = (await this.fetchJson(RELEASE_API)) as {
      tag_name?: string;
      assets?: { name?: string; browser_download_url?: string }[];
    };
    const tag = String(json.tag_name ?? '').trim();
    const assets = json.assets ?? [];
    const assetUrl = assets.find((a) => a.name === ASSET_NAME)?.browser_download_url ?? '';
    const sumsUrl = assets.find((a) => a.name === SUMS_ASSET)?.browser_download_url ?? '';
    if (!tag || !assetUrl || !sumsUrl) throw new Error('Engine update refused: incomplete release feed');
    // Fail-closed host gate BEFORE a single byte is fetched (§9/§12).
    for (const url of [assetUrl, sumsUrl]) {
      if (!isTrustedReleaseHost(url)) throw new Error(`Engine update refused: untrusted host "${url}"`);
    }
    return { tag, assetUrl, sumsUrl };
  }

  private async request(url: string): Promise<Response> {
    if (!isTrustedReleaseHost(url)) throw new Error(`Engine update refused: untrusted host "${url}"`);
    const fetchFn = this.deps.fetchFn ?? fetch;
    const res = await fetchFn(url, { headers: { 'User-Agent': 'AHG-Universal-Converter' } });
    if (!res.ok) throw new Error(`Engine update failed: HTTP ${res.status}`);
    return res;
  }

  private async fetchJson(url: string): Promise<unknown> {
    return (await this.request(url)).json();
  }

  private async fetchText(url: string): Promise<string> {
    return (await this.request(url)).text();
  }

  private async fetchBinary(url: string): Promise<Buffer> {
    return Buffer.from(await (await this.request(url)).arrayBuffer());
  }

  private currentVersion(): string {
    return this.deps.downloader.ytDlpVersion();
  }

  private binDir(): string {
    if (this.deps.binDir) return this.deps.binDir;
    return path.join(app.getPath('userData'), 'bin');
  }

  private now(): number {
    return this.deps.now ? this.deps.now() : Date.now();
  }

  private emit(status: YtdlpUpdateStatus): YtdlpUpdateStatus {
    this.status = status;
    this.deps.getWindow()?.webContents.send('ytdlp:status', status);
    return status;
  }
}

function message(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
