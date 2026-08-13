import { describe, it, expect, vi, beforeEach } from 'vitest';
import { EventEmitter } from 'node:events';
import type { BrowserWindow } from 'electron';

// Mock electron-updater: autoUpdater is an EventEmitter exposing the three methods
// the wrapper drives. The wrapper default-imports then destructures autoUpdater
// (named ESM imports fail at runtime with this CJS module), so expose BOTH a
// default and a named export backed by the SAME instance the test drives.
vi.mock('electron-updater', () => {
  const autoUpdater = new EventEmitter() as EventEmitter & Record<string, unknown>;
  autoUpdater.autoDownload = true; // wrapper must flip this to false
  autoUpdater.autoInstallOnAppQuit = false; // …and this to true
  autoUpdater.checkForUpdates = vi.fn(async () => null);
  autoUpdater.downloadUpdate = vi.fn(async () => []);
  autoUpdater.quitAndInstall = vi.fn();
  return { default: { autoUpdater }, autoUpdater };
});

import { Updater } from './updater';
import { autoUpdater } from 'electron-updater';

const emitter = autoUpdater as unknown as EventEmitter;
const au = autoUpdater as unknown as {
  autoDownload: boolean;
  autoInstallOnAppQuit: boolean;
  checkForUpdates: ReturnType<typeof vi.fn>;
  downloadUpdate: ReturnType<typeof vi.fn>;
  quitAndInstall: ReturnType<typeof vi.fn>;
};

const GOOD = 'https://github.com/owner/repo/releases/download/v1.2.3/App-Setup.exe';
const BAD = 'https://evil.example.com/App-Setup.exe';

function setup() {
  const send = vi.fn();
  const win = { webContents: { send } } as unknown as BrowserWindow;
  const updater = new Updater({ getWindow: () => win });
  return { updater, send };
}

beforeEach(() => {
  vi.clearAllMocks();
  emitter.removeAllListeners();
});

describe('Updater', () => {
  it('configures autoUpdater: no auto-download, install on app quit', () => {
    setup();
    expect(au.autoDownload).toBe(false);
    expect(au.autoInstallOnAppQuit).toBe(true);
  });

  it('forwards updater events to the renderer window', () => {
    const { send } = setup();
    emitter.emit('update-available', { version: '1.2.3', files: [{ url: GOOD }] });
    emitter.emit('download-progress', { percent: 42 });
    emitter.emit('update-downloaded', { version: '1.2.3', files: [{ url: GOOD }] });
    expect(send).toHaveBeenCalledWith('update:available', { version: '1.2.3' });
    expect(send).toHaveBeenCalledWith('update:progress', { percent: 42 });
    expect(send).toHaveBeenCalledWith('update:downloaded', { version: '1.2.3' });
  });

  it('check() delegates to autoUpdater.checkForUpdates', () => {
    setup().updater.check();
    expect(au.checkForUpdates).toHaveBeenCalledOnce();
  });

  it('download() fetches when the installer host is trusted', async () => {
    const { updater } = setup();
    emitter.emit('update-available', { version: '1.2.3', files: [{ url: GOOD }] });
    await updater.download();
    expect(au.downloadUpdate).toHaveBeenCalledOnce();
  });

  it('download() refuses an untrusted installer host', async () => {
    const { updater } = setup();
    emitter.emit('update-available', { version: '1.2.3', files: [{ url: BAD }] });
    await expect(updater.download()).rejects.toThrow(/untrusted/i);
    expect(au.downloadUpdate).not.toHaveBeenCalled();
  });

  it('install() delegates to autoUpdater.quitAndInstall', () => {
    setup().updater.install();
    expect(au.quitAndInstall).toHaveBeenCalledOnce();
  });
});
