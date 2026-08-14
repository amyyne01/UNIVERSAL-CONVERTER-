import { describe, it, expect, vi, beforeEach } from 'vitest';
import { createHash } from 'node:crypto';

// In-memory fs. `default` too: anything in the import graph doing
// `import fs from 'node:fs'` must get the same object, or the module fails to
// resolve and the whole file silently collects zero tests.
vi.mock('node:fs', async (importOriginal) => {
  const actual = await importOriginal<typeof import('node:fs')>();
  const files = new Map<string, unknown>();
  const api = {
    ...actual,
    existsSync: vi.fn((p: string) => files.has(p)),
    mkdirSync: vi.fn(() => undefined),
    writeFileSync: vi.fn((p: string, data: unknown) => { files.set(p, data); }),
    rmSync: vi.fn((p: string) => { files.delete(p); }),
    _files: files,
  };
  return { ...api, default: api };
});

vi.mock('electron', () => ({
  app: { getVersion: () => '1.0.0', getPath: () => 'C:\\userData', quit: vi.fn() },
}));

import * as fs from 'node:fs';
import { Updater, isTrustedReleaseHost, isNewerVersion, parseChecksum, swapScript } from './updater';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const files = (fs as any)._files as Map<string, unknown>;

const EXE = 'AHG-Converter-1.2.0.exe';
const BYTES = Buffer.from('pretend this is a portable exe');
const SHA = createHash('sha256').update(BYTES).digest('hex');
const ASSET_URL = `https://github.com/amyyne01/UNIVERSAL-CONVERTER-/releases/download/v1.2.0/${EXE}`;
const SUMS_URL = 'https://github.com/amyyne01/UNIVERSAL-CONVERTER-/releases/download/v1.2.0/SHA256SUMS.txt';

const release = (over: Record<string, unknown> = {}) => ({
  tag_name: 'v1.2.0',
  assets: [
    { name: EXE, browser_download_url: ASSET_URL },
    { name: 'SHA256SUMS.txt', browser_download_url: SUMS_URL },
  ],
  ...over,
});

/** A fetch stub that answers by URL; the exe is streamed so progress is exercised. */
function stubFetch(opts: { feed?: unknown; sums?: string; bytes?: Buffer } = {}) {
  const sums = opts.sums ?? `${SHA}  ${EXE}\n`;
  const bytes = opts.bytes ?? BYTES;
  return vi.fn(async (url: string) => {
    if (url.includes('api.github.com')) {
      return { ok: true, status: 200, json: async () => opts.feed ?? release() } as unknown as Response;
    }
    if (url === SUMS_URL) {
      return { ok: true, status: 200, text: async () => sums } as unknown as Response;
    }
    const body = new ReadableStream<Uint8Array>({
      start(c) {
        c.enqueue(new Uint8Array(bytes.subarray(0, 10)));
        c.enqueue(new Uint8Array(bytes.subarray(10)));
        c.close();
      },
    });
    return {
      ok: true, status: 200, body,
      headers: { get: (h: string) => (h === 'content-length' ? String(bytes.length) : null) },
    } as unknown as Response;
  });
}

function make(over: Record<string, unknown> = {}) {
  const sent: { channel: string; payload: any }[] = [];
  const win = { webContents: { send: (channel: string, payload: unknown) => sent.push({ channel, payload }) } };
  const spawnFn = vi.fn(() => ({ unref: vi.fn() }));
  const quit = vi.fn();
  const updater = new Updater({
    getWindow: () => win as never,
    fetchFn: stubFetch() as never,
    currentVersion: () => '1.0.0',
    targetExe: () => 'D:\\Apps\\AHG-Converter.exe',
    updateDir: 'C:\\userData\\updates',
    spawnFn: spawnFn as never,
    quit,
    ...over,
  });
  const of = (channel: string) => sent.filter((s) => s.channel === channel);
  return { updater, sent, of, spawnFn, quit };
}

beforeEach(() => { vi.clearAllMocks(); files.clear(); });

describe('isTrustedReleaseHost', () => {
  it('accepts https on allowlisted hosts and refuses everything else', () => {
    expect(isTrustedReleaseHost('https://github.com/a/b')).toBe(true);
    expect(isTrustedReleaseHost('https://objects.githubusercontent.com/x')).toBe(true);
    expect(isTrustedReleaseHost('http://github.com/a/b')).toBe(false);  // not https
    expect(isTrustedReleaseHost('https://evil.example/x')).toBe(false);
    expect(isTrustedReleaseHost('//evil.example/x')).toBe(false);       // protocol-relative
  });

  it('resolves a relative asset path onto github.com rather than rejecting it', () => {
    // Feeds list assets relative to the repo, so a bare name is legitimate and
    // lands on a trusted host. What must NOT slip through is an off-host absolute
    // url wearing a relative-looking prefix.
    expect(isTrustedReleaseHost('releases/download/v1/app.exe')).toBe(true);
    expect(isTrustedReleaseHost('https://evil.example/releases/download/v1/app.exe')).toBe(false);
  });
});

describe('isNewerVersion', () => {
  it('compares numerically, not lexically', () => {
    expect(isNewerVersion('1.10.0', '1.9.0')).toBe(true);   // the classic string-compare trap
    expect(isNewerVersion('v1.2.0', '1.0.0')).toBe(true);
    expect(isNewerVersion('1.0.0', '1.0.0')).toBe(false);
    expect(isNewerVersion('0.9.9', '1.0.0')).toBe(false);
    expect(isNewerVersion('garbage', '1.0.0')).toBe(false); // junk never looks newer
  });
});

describe('parseChecksum', () => {
  it('finds the hash for a name and ignores the rest', () => {
    const sums = `deadbeef  other.exe\n${SHA}  ${EXE}\n`;
    expect(parseChecksum(sums, EXE)).toBe(SHA);
    expect(parseChecksum(sums, 'nope.exe')).toBeNull();
    expect(parseChecksum('garbage line', EXE)).toBeNull();
  });

  it('tolerates the binary-mode asterisk', () => {
    expect(parseChecksum(`${SHA} *${EXE}`, EXE)).toBe(SHA);
  });
});

describe('Updater.check', () => {
  it('reports an available update when the feed is newer', async () => {
    const { updater, of } = make();
    await updater.check();
    expect(of('update:available')[0].payload).toEqual({ version: '1.2.0' });
  });

  it('reports not-available when the feed matches the running version', async () => {
    const { updater, of } = make({ currentVersion: () => '1.2.0' });
    await updater.check();
    expect(of('update:available')).toHaveLength(0);
    expect(of('update:not-available')).toHaveLength(1);
  });

  it('refuses a feed pointing at an untrusted host, before downloading anything', async () => {
    const feed = release({
      assets: [
        { name: EXE, browser_download_url: 'https://evil.example/payload.exe' },
        { name: 'SHA256SUMS.txt', browser_download_url: SUMS_URL },
      ],
    });
    const { updater, of } = make({ fetchFn: stubFetch({ feed }) as never });
    await updater.check();
    expect(of('update:error')[0].payload.message).toMatch(/untrusted host/);
    expect(of('update:available')).toHaveLength(0);
  });

  it('surfaces a network failure as an error event rather than throwing', async () => {
    const fetchFn = vi.fn(async () => { throw new Error('offline'); });
    const { updater, of } = make({ fetchFn: fetchFn as never });
    await expect(updater.check()).resolves.toBeUndefined();
    expect(of('update:error')).toHaveLength(1);
  });
});

describe('Updater.getState', () => {
  // The regression this exists for: the startup check fires before the lazy
  // Settings tab mounts, so its event is missed. A late reader must still be able
  // to learn that an update is waiting.
  it('still reports an available update to a reader that missed the event', async () => {
    const { updater } = make();
    expect(updater.getState()).toEqual({ phase: 'idle' });
    await updater.check();
    expect(updater.getState()).toEqual({ phase: 'available', version: '1.2.0' });
  });

  it('tracks the phase through download and failure', async () => {
    const { updater } = make();
    await updater.check();
    await updater.download();
    expect(updater.getState()).toEqual({ phase: 'ready', version: '1.2.0' });

    const bad = make({ fetchFn: stubFetch({ sums: `${'0'.repeat(64)}  ${EXE}\n` }) as never });
    await bad.updater.check();
    await bad.updater.download();
    expect(bad.updater.getState().phase).toBe('error');
  });

  it('reports uptodate when the feed is not newer', async () => {
    const { updater } = make({ currentVersion: () => '9.9.9' });
    await updater.check();
    expect(updater.getState()).toEqual({ phase: 'uptodate' });
  });
});

describe('Updater.download', () => {
  it('refuses when nothing is available', async () => {
    const { updater } = make();
    await expect(updater.download()).rejects.toThrow(/no available update/);
  });

  it('verifies the checksum, writes the file, and reports progress', async () => {
    const { updater, of } = make();
    await updater.check();
    await updater.download();

    expect(files.has('C:\\userData\\updates\\' + EXE)).toBe(true);
    expect(of('update:downloaded')[0].payload).toEqual({ version: '1.2.0' });
    const percents = of('update:progress').map((p) => p.payload.percent);
    expect(percents.at(-1)).toBe(100);
  });

  it('refuses a payload that does not match the published checksum, and writes nothing', async () => {
    const { updater, of } = make({
      fetchFn: stubFetch({ sums: `${'0'.repeat(64)}  ${EXE}\n` }) as never,
    });
    await updater.check();
    await updater.download();

    expect(files.size).toBe(0);
    expect(of('update:downloaded')).toHaveLength(0);
    expect(of('update:error')[0].payload.message).toMatch(/checksum/);
  });

  it('refuses when the release publishes no checksum at all', async () => {
    const { updater, of } = make({ fetchFn: stubFetch({ sums: '' }) as never });
    await updater.check();
    await updater.download();
    expect(of('update:error')[0].payload.message).toMatch(/no checksum/);
    expect(files.size).toBe(0);
  });
});

describe('Updater.install', () => {
  it('refuses before anything has been downloaded', () => {
    const { updater } = make();
    expect(() => updater.install()).toThrow(/nothing downloaded/);
  });

  it('refuses when there is no single exe to replace (dev / unpacked build)', async () => {
    const { updater } = make({ targetExe: () => null });
    await updater.check();
    await updater.download();
    expect(() => updater.install()).toThrow(/cannot replace itself/);
  });

  it('writes the swap script, spawns it detached, and quits', async () => {
    const { updater, spawnFn, quit } = make();
    await updater.check();
    await updater.download();
    updater.install();

    const script = files.get('C:\\userData\\updates\\apply-update.cmd') as string;
    expect(script).toContain('D:\\Apps\\AHG-Converter.exe');
    expect(script).toContain(EXE);

    expect(spawnFn).toHaveBeenCalledOnce();
    const [cmd, args, opts] = spawnFn.mock.calls[0] as unknown as [string, string[], Record<string, unknown>];
    expect(cmd).toBe('cmd.exe');
    expect(args).toContain('C:\\userData\\updates\\apply-update.cmd');
    // Detached, or it dies with us before it can swap the file it is waiting on.
    expect(opts.detached).toBe(true);
    expect(quit).toHaveBeenCalledOnce();
  });
});

describe('swapScript', () => {
  it('retries the move, relaunches, and deletes itself', () => {
    const s = swapScript('C:\\staged\\new.exe', 'D:\\Apps\\app.exe');
    expect(s).toMatch(/for \/l/);        // retries rather than assuming the file is free
    expect(s).toMatch(/move \/y/);
    expect(s).toMatch(/start ""/);       // relaunches
    expect(s).toMatch(/del "%~f0"/);     // leaves nothing behind to re-run at next boot
  });
});
