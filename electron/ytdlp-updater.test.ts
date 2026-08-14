import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync, existsSync, readFileSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import type { BrowserWindow } from 'electron';

// The updater imports the trusted-host allowlist from updater.ts, which pulls in

import { YtdlpUpdater, isEngineStale } from './ytdlp-updater';
// One definition, in the module that owns release-asset handling.
import { parseChecksum } from './updater';
import type { Downloader } from './downloader';

const ASSET = process.platform === 'win32' ? 'yt-dlp.exe' : 'yt-dlp';
const API = 'https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest';
const PAYLOAD = Buffer.from('#!/fake-yt-dlp binary');
const SHA = createHash('sha256').update(PAYLOAD).digest('hex');

let dir: string;

/** Minimal fetch stub over a url → {json|text|bytes} table. */
function fetcher(table: Record<string, unknown>) {
  return vi.fn(async (url: string) => {
    const body = table[url];
    if (body === undefined) return { ok: false, status: 404 } as unknown as Response;
    return {
      ok: true,
      status: 200,
      json: async () => body,
      text: async () => String(body),
      arrayBuffer: async () => (body as Buffer).buffer.slice((body as Buffer).byteOffset, (body as Buffer).byteOffset + (body as Buffer).byteLength),
    } as unknown as Response;
  }) as unknown as typeof fetch;
}

function releaseFeed(assetUrl: string, sumsUrl: string, tag = '2026.07.01') {
  return {
    tag_name: tag,
    assets: [
      { name: ASSET, browser_download_url: assetUrl },
      { name: 'SHA2-256SUMS', browser_download_url: sumsUrl },
    ],
  };
}

const ASSET_URL = `https://github.com/yt-dlp/yt-dlp/releases/download/2026.07.01/${ASSET}`;
const SUMS_URL = 'https://github.com/yt-dlp/yt-dlp/releases/download/2026.07.01/SHA2-256SUMS';

function setup(opts: { fetchFn?: typeof fetch; current?: string; auto?: boolean } = {}) {
  const send = vi.fn();
  const resetBinaryCache = vi.fn();
  const downloader = {
    ytDlpVersion: () => opts.current ?? '2026.01.01',
    resetBinaryCache,
  } as unknown as Downloader;
  const updater = new YtdlpUpdater({
    downloader,
    getWindow: () => ({ webContents: { send } }) as unknown as BrowserWindow,
    isAutoUpdateEnabled: () => opts.auto ?? true,
    binDir: dir,
    fetchFn: opts.fetchFn,
  });
  return { updater, send, resetBinaryCache };
}

beforeEach(() => {
  dir = mkdtempSync(path.join(tmpdir(), 'ahg-engine-'));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

describe('isEngineStale', () => {
  it('matches extractor-level failures only', () => {
    expect(isEngineStale('ERROR: Unable to extract player response')).toBe(true);
    expect(isEngineStale('nsig extraction failed; please report this issue')).toBe(true);
    expect(isEngineStale('ERROR: Video unavailable')).toBe(false);
    expect(isEngineStale('')).toBe(false);
  });
});

describe('parseChecksum', () => {
  it('pulls the hash for the requested file and ignores others', () => {
    const sums = `${'a'.repeat(64)}  yt-dlp\n${SHA}  ${ASSET}\n`;
    expect(parseChecksum(sums, ASSET)).toBe(SHA);
    expect(parseChecksum(sums, 'nope.exe')).toBeNull();
    expect(parseChecksum('garbage line', ASSET)).toBeNull();
  });
});

describe('check', () => {
  it('reports an available engine and caches for 24h', async () => {
    const fetchFn = fetcher({ [API]: releaseFeed(ASSET_URL, SUMS_URL) });
    const { updater, send } = setup({ fetchFn, current: '2026.01.01' });

    const status = await updater.check();
    expect(status).toMatchObject({ current: '2026.01.01', latest: '2026.07.01', state: 'available' });
    expect(send).toHaveBeenCalledWith('ytdlp:status', expect.objectContaining({ state: 'available' }));

    const calls = (fetchFn as unknown as ReturnType<typeof vi.fn>).mock.calls.length;
    await updater.check();                       // inside the TTL — no network
    expect((fetchFn as unknown as ReturnType<typeof vi.fn>).mock.calls.length).toBe(calls);
    await updater.check(true);                   // forced — hits the feed again
    expect((fetchFn as unknown as ReturnType<typeof vi.fn>).mock.calls.length).toBeGreaterThan(calls);
  });

  it('reports uptodate when the installed tag matches', async () => {
    const fetchFn = fetcher({ [API]: releaseFeed(ASSET_URL, SUMS_URL) });
    const { updater } = setup({ fetchFn, current: '2026.07.01' });
    expect((await updater.check()).state).toBe('uptodate');
  });

  it('surfaces a network failure as an error status instead of throwing', async () => {
    const { updater } = setup({ fetchFn: fetcher({}) });
    const status = await updater.check();
    expect(status.state).toBe('error');
  });
});

describe('update', () => {
  it('verifies the checksum, swaps atomically and invalidates the binary cache', async () => {
    const fetchFn = fetcher({
      [API]: releaseFeed(ASSET_URL, SUMS_URL),
      [ASSET_URL]: PAYLOAD,
      [SUMS_URL]: `${SHA}  ${ASSET}\n`,
    });
    const { updater, resetBinaryCache, send } = setup({ fetchFn });

    const status = await updater.update();
    expect(status.state).toBe('updated');
    expect(readFileSync(path.join(dir, ASSET))).toEqual(PAYLOAD);
    expect(resetBinaryCache).toHaveBeenCalled();
    expect(send).toHaveBeenCalledWith('ytdlp:status', expect.objectContaining({ state: 'updated' }));
  });

  it('refuses a payload whose hash does not match and writes no binary', async () => {
    const fetchFn = fetcher({
      [API]: releaseFeed(ASSET_URL, SUMS_URL),
      [ASSET_URL]: PAYLOAD,
      [SUMS_URL]: `${'b'.repeat(64)}  ${ASSET}\n`,
    });
    const { updater, resetBinaryCache } = setup({ fetchFn });

    const status = await updater.update();
    expect(status.state).toBe('error');
    expect(status.message).toMatch(/checksum mismatch/i);
    expect(existsSync(path.join(dir, ASSET))).toBe(false);
    expect(readdirSync(dir)).toEqual([]); // the .tmp is cleaned up too
    expect(resetBinaryCache).not.toHaveBeenCalled();
  });

  it('refuses an off-allowlist asset host before fetching a byte', async () => {
    const evil = 'https://evil.example.com/yt-dlp.exe';
    const fetchFn = fetcher({ [API]: releaseFeed(evil, SUMS_URL), [evil]: PAYLOAD });
    const { updater } = setup({ fetchFn });

    const status = await updater.update();
    expect(status.state).toBe('error');
    expect(status.message).toMatch(/untrusted host/i);
    expect((fetchFn as unknown as ReturnType<typeof vi.fn>).mock.calls.map((c) => c[0])).not.toContain(evil);
  });
});

describe('handleDownloadFailure (reactive trigger)', () => {
  it('updates once per session on a stale-extractor error', async () => {
    const fetchFn = fetcher({
      [API]: releaseFeed(ASSET_URL, SUMS_URL),
      [ASSET_URL]: PAYLOAD,
      [SUMS_URL]: `${SHA}  ${ASSET}\n`,
    });
    const { updater } = setup({ fetchFn });
    const spy = vi.spyOn(updater, 'update');

    updater.handleDownloadFailure('ERROR: Unable to extract player response');
    updater.handleDownloadFailure('ERROR: Unable to extract player response');
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('ignores ordinary failures and stays off when auto-update is disabled', () => {
    const { updater } = setup({ auto: false });
    const spy = vi.spyOn(updater, 'update');
    updater.handleDownloadFailure('ERROR: Unable to extract player response');
    expect(spy).not.toHaveBeenCalled();

    const { updater: on } = setup({ auto: true });
    const spy2 = vi.spyOn(on, 'update');
    on.handleDownloadFailure('ERROR: Video unavailable');
    expect(spy2).not.toHaveBeenCalled();
  });
});
