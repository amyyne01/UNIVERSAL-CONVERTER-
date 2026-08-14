import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock electron + the modules ipc.ts pulls in at runtime (same specifiers it imports).
// Only existsSync is stubbed — config.ts imports node:fs's DEFAULT export, so a
// wholesale replacement breaks the module for everything else in the graph.
vi.mock('node:fs', async (importOriginal) => {
  const actual = await importOriginal<typeof import('node:fs')>();
  const existsSync = vi.fn(() => true);
  return { ...actual, existsSync, default: { ...actual, existsSync } };
});

vi.mock('electron', () => ({
  // `app` is read by secure()/assertTrustedSender via security.ts (dev branch → no
  // getAppPath call, but stub it anyway). isPackaged:false picks the Vite-origin path.
  app: { isPackaged: false, getAppPath: vi.fn(() => 'C:\\app') },
  ipcMain: { handle: vi.fn() },
  dialog: { showOpenDialog: vi.fn().mockResolvedValue({ canceled: false, filePaths: ['C:\\Picked'] }) },
  shell: { openExternal: vi.fn().mockResolvedValue(undefined), showItemInFolder: vi.fn() },
}));

// Every handler is now wrapped by secure(), which attests the sender frame first.
// Supply a valid app MAIN frame (dev origin, top === itself) so the real
// attestation path runs and passes — never a no-op stub.
const evt = () => {
  const frame: any = { url: 'http://localhost:5173/' };
  frame.top = frame;
  return { senderFrame: frame } as any;
};

vi.mock('./window.js', () => ({
  getMainWindow: vi.fn(() => ({
    webContents: { send: vi.fn() },
    minimize: vi.fn(),
    maximize: vi.fn(),
    unmaximize: vi.fn(),
    close: vi.fn(),
    isMaximized: vi.fn().mockReturnValue(false),
  })),
}));

vi.mock('./queue.js', () => ({
  createTask: vi.fn((data) => ({ taskId: 'mock-task', ...data })),
  enqueueTask: vi.fn(),
  cancelTask: vi.fn().mockReturnValue(true),
  cancelAllTasks: vi.fn(),
  pauseTask: vi.fn().mockReturnValue(true),
  resumeTask: vi.fn().mockReturnValue(true),
  getAllTasks: vi.fn().mockReturnValue([{ taskId: 't1' }]),
  removeTasks: vi.fn().mockReturnValue(true),
}));

import { registerIpcHandlers } from './ipc';
import { ipcMain, dialog, shell } from 'electron';
import { existsSync } from 'node:fs';

describe('registerIpcHandlers', () => {
  let config: any;
  let downloader: any;
  let spotify: any;
  let license: any;
  let updater: any;
  let ytdlp: any;
  let handlers: Map<string, (...a: any[]) => any>;

  beforeEach(() => {
    vi.clearAllMocks();

    config = {
      get: vi.fn((key: string) => {
        const defaults: Record<string, any> = {
          outputDir: 'C:\\Downloads',
          defaultFormat: 'mp3',
          defaultQuality: '320',
          defaultVideoQuality: 'best',
          embedThumbnail: true,
          embedMetadata: true,
          skipExisting: true,
        };
        return defaults[key];
      }),
      set: vi.fn(),
      getAll: vi.fn().mockReturnValue({ outputDir: 'C:\\Downloads' }),
      update: vi.fn(),
    };

    downloader = {
      fetchMetadata: vi.fn().mockResolvedValue({ title: 'X', entries: [] }),
      searchYouTube: vi.fn().mockResolvedValue([{ id: 'yt1' }]),
      searchSoundCloud: vi.fn().mockResolvedValue([{ id: 'sc1' }]),
    };

    spotify = {
      parseUrl: vi.fn().mockReturnValue({ type: 'track', id: 'sp1' }),
      fetchTrack: vi.fn().mockResolvedValue(null),
      fetchPlaylist: vi.fn().mockResolvedValue(null),
      fetchAlbum: vi.fn().mockResolvedValue(null),
      search: vi.fn().mockResolvedValue([]),
    };

    license = {
      // Default to premium so the pre-existing download:start expectations read
      // the unclamped path; the tier tests below flip it to basic explicitly.
      getPlan: vi.fn().mockReturnValue('premium'),
      checkLicense: vi.fn().mockResolvedValue({ activated: true, plan: 'premium' }),
      activateLicense: vi.fn().mockResolvedValue({ success: true }),
      deactivateLicense: vi.fn().mockResolvedValue({ success: true }),
      releaseLicense: vi.fn().mockResolvedValue({ success: true }),
      getSupportLink: vi.fn().mockReturnValue('https://discord.gg/NMRgaSQVNx'),
    };

    updater = {
      check: vi.fn(),
      download: vi.fn().mockResolvedValue(undefined),
      install: vi.fn(),
    };

    ytdlp = {
      check: vi.fn().mockResolvedValue({ current: '2026.01.01', latest: '2026.07.01', state: 'available' }),
      update: vi.fn().mockResolvedValue({ current: '2026.07.01', latest: '2026.07.01', state: 'updated' }),
    };

    handlers = new Map();
    vi.mocked(ipcMain.handle).mockImplementation((channel: string, handler: any) => {
      handlers.set(channel, handler);
    });

    registerIpcHandlers(config, downloader, spotify, license, updater, ytdlp);
  });

  it('registers exactly the expected channels', () => {
    const expected = [
      'window:minimize', 'window:maximize', 'window:close', 'window:isMaximized',
      'config:get', 'config:set', 'config:getAll', 'config:update',
      'url:detect', 'url:fetchMetadata',
      'youtube:search', 'soundcloud:search',
      'spotify:parseUrl', 'spotify:fetchTrack', 'spotify:fetchPlaylist',
      'spotify:fetchAlbum', 'spotify:search',
      'download:start', 'download:cancel', 'download:cancelAll',
      'download:pause', 'download:resume', 'download:list', 'download:remove',
      'dialog:selectDir',
      'shell:showItemInFolder',
      'license:check', 'license:activate', 'license:deactivate', 'license:release', 'app:openSupport',
      'update:check', 'update:download', 'update:install',
      'ytdlp:version', 'ytdlp:update',
    ];
    for (const ch of expected) {
      expect(handlers.has(ch), `Missing handler: ${ch}`).toBe(true);
    }
  });

  it('search handlers clamp an over-long query instead of throwing at the boundary', () => {
    // A 5000-char paste routed to search must NOT throw (which would reject the invoke
    // and log a main-process error) — it degrades to a clamped, sensible search.
    const huge = 'a'.repeat(5000);
    expect(() => handlers.get('youtube:search')!(evt(), huge)).not.toThrow();
    const passed = downloader.searchYouTube.mock.calls[0][0];
    expect(typeof passed).toBe('string');
    expect(passed.length).toBeLessThanOrEqual(256);
    // A non-string query degrades to '' rather than rejecting at the boundary.
    handlers.get('soundcloud:search')!(evt(), { not: 'a string' });
    expect(downloader.searchSoundCloud).toHaveBeenCalledWith('');
    handlers.get('spotify:search')!(evt(), 12345);
    expect(spotify.search).toHaveBeenCalledWith('');
  });

  it('config:get delegates to config.get', () => {
    const result = handlers.get('config:get')!(evt(), 'outputDir');
    expect(config.get).toHaveBeenCalledWith('outputDir');
    expect(result).toBe('C:\\Downloads');
  });

  it('config:get returns undefined for prototype/unknown keys without touching config', () => {
    for (const bad of ['__proto__', 'toString', 'constructor', 'notAKey']) {
      expect(handlers.get('config:get')!(evt(), bad)).toBeUndefined();
    }
    expect(config.get).not.toHaveBeenCalled();
  });

  it('config:set delegates and returns true', () => {
    const result = handlers.get('config:set')!(evt(), 'defaultFormat', 'flac');
    expect(config.set).toHaveBeenCalledWith('defaultFormat', 'flac');
    expect(result).toBe(true);
  });

  it('config:getAll returns the full config', () => {
    expect(handlers.get('config:getAll')!(evt())).toEqual({ outputDir: 'C:\\Downloads' });
  });

  it('config:update delegates and returns true', () => {
    const result = handlers.get('config:update')!(evt(), { outputDir: 'D:\\Music' });
    expect(config.update).toHaveBeenCalledWith({ outputDir: 'D:\\Music' });
    expect(result).toBe(true);
  });

  it('B7: config:update rejects null/non-object payloads instead of throwing a raw TypeError', () => {
    // Pre-fix, config.update(null) reaches Object.keys(null) inside ConfigManager and
    // throws an uncaught "Cannot convert undefined or null to object" TypeError.
    expect(() => handlers.get('config:update')!(evt(), null)).toThrow('Invalid input');
    expect(() => handlers.get('config:update')!(evt(), 'nope')).toThrow('Invalid input');
    expect(config.update).not.toHaveBeenCalled();
  });

  it('url:detect runs the detection cascade', () => {
    const det = handlers.get('url:detect')!(evt(), 'https://youtu.be/dQw4w9WgXcQ');
    expect(det.platform).toBe('youtube');
    expect(det.contentType).toBe('video');
  });

  it('url:detect rejects non-string input at the boundary', async () => {
    await expect((async () => handlers.get('url:detect')!(evt(), 123))()).rejects.toThrow();
  });

  it('url:detect rejects over-long input at the boundary', async () => {
    const huge = 'h'.repeat(3000);
    await expect((async () => handlers.get('url:detect')!(evt(), huge))()).rejects.toThrow();
  });

  it('url:fetchMetadata delegates to downloader.fetchMetadata', async () => {
    await handlers.get('url:fetchMetadata')!(evt(), 'https://x.test/v');
    expect(downloader.fetchMetadata).toHaveBeenCalledWith('https://x.test/v');
  });

  it('youtube:search and soundcloud:search delegate to the engine', async () => {
    await handlers.get('youtube:search')!(evt(), 'lofi');
    expect(downloader.searchYouTube).toHaveBeenCalledWith('lofi');
    await handlers.get('soundcloud:search')!(evt(), 'house');
    expect(downloader.searchSoundCloud).toHaveBeenCalledWith('house');
  });

  it('url:fetchMetadata rejects non-string and leading-dash input', () => {
    expect(() => handlers.get('url:fetchMetadata')!(evt(), 123)).toThrow();
    expect(() => handlers.get('url:fetchMetadata')!(evt(), '--rm-rf')).toThrow(/must not start/);
  });

  // URL args reject invalid input (above); read-only SEARCH args degrade to an empty
  // search instead — an over-long/garbage paste must never throw a boundary error.
  it('youtube:search and spotify:search degrade non-string input to an empty search (no throw)', () => {
    expect(() => handlers.get('youtube:search')!(evt(), {})).not.toThrow();
    expect(downloader.searchYouTube).toHaveBeenCalledWith('');
    expect(() => handlers.get('spotify:search')!(evt(), 5)).not.toThrow();
    expect(spotify.search).toHaveBeenCalledWith('');
  });

  it('download:start applies config defaults, enqueues, and returns the task', async () => {
    const { createTask, enqueueTask } = await import('./queue.js');
    const task = handlers.get('download:start')!(evt(), { url: 'https://youtu.be/abc' });
    expect(vi.mocked(createTask)).toHaveBeenCalledWith(
      expect.objectContaining({
        url: 'https://youtu.be/abc',
        outputDir: 'C:\\Downloads',
        format: 'mp3',
        quality: '320',
      }),
    );
    expect(vi.mocked(enqueueTask)).toHaveBeenCalledWith(task);
    expect(task.taskId).toBe('mock-task');
  });

  // Tier enforcement is a boundary concern: the renderer locks these controls,
  // but a stale or tampered renderer must still be clamped here, not rejected —
  // a clamped download works, a rejected one just fails.
  it('download:start clamps video quality, lossless format and playlists on basic', async () => {
    const { createTask } = await import('./queue.js');
    license.getPlan.mockReturnValue('basic');

    handlers.get('download:start')!(evt(), {
      url: 'https://youtu.be/abc',
      videoQuality: 'best',
      format: 'flac',
      quality: 'lossless',
      isPlaylist: true,
    });

    expect(vi.mocked(createTask)).toHaveBeenCalledWith(
      expect.objectContaining({
        videoQuality: '1080p',
        format: 'mp3',
        quality: '320',
        playlistLimit: 20,
      }),
    );
  });

  it('download:start leaves premium requests untouched and sets no playlist cap', async () => {
    const { createTask } = await import('./queue.js');
    license.getPlan.mockReturnValue('premium');

    handlers.get('download:start')!(evt(), {
      url: 'https://youtu.be/abc',
      videoQuality: '2160p',
      format: 'flac',
      quality: 'lossless',
      isPlaylist: true,
    });

    const arg = vi.mocked(createTask).mock.calls.at(-1)![0];
    expect(arg).toMatchObject({ videoQuality: '2160p', format: 'flac', quality: 'lossless' });
    expect(arg).not.toHaveProperty('playlistLimit');
  });

  it('download:start rebuilds a supplied track from bounded fields', async () => {
    const { createTask } = await import('./queue.js');
    handlers.get('download:start')!(evt(), {
      url: 'https://open.spotify.com/track/x',
      track: { name: 'N', artist: 'A', artists: ['A', 'B'], album: 'Al', evil: '<script>' },
    });
    const arg = vi.mocked(createTask).mock.calls.at(-1)![0];
    expect(arg.track).toEqual(expect.objectContaining({ name: 'N', artist: 'A', artists: ['A', 'B'], album: 'Al' }));
    expect(arg.track).not.toHaveProperty('evil');
  });

  it('download:start rejects a malformed track at the boundary', () => {
    expect(() => handlers.get('download:start')!(evt(), {
      url: 'https://open.spotify.com/track/x',
      track: { name: 123 },
    })).toThrow();
    expect(() => handlers.get('download:start')!(evt(), {
      url: 'https://open.spotify.com/track/x',
      track: 'not-an-object',
    })).toThrow(/track/);
  });

  it('download:cancel delegates to cancelTask', async () => {
    const { cancelTask } = await import('./queue.js');
    expect(handlers.get('download:cancel')!(evt(), 'mock-task')).toBe(true);
    expect(vi.mocked(cancelTask)).toHaveBeenCalledWith('mock-task');
  });

  it('download:cancelAll delegates to cancelAllTasks', async () => {
    const { cancelAllTasks } = await import('./queue.js');
    handlers.get('download:cancelAll')!(evt());
    expect(vi.mocked(cancelAllTasks)).toHaveBeenCalled();
  });

  it('download:pause and download:resume delegate to the queue', async () => {
    const { pauseTask, resumeTask } = await import('./queue.js');
    expect(handlers.get('download:pause')!(evt(), 'mock-task')).toBe(true);
    expect(vi.mocked(pauseTask)).toHaveBeenCalledWith('mock-task');
    expect(handlers.get('download:resume')!(evt(), 'mock-task')).toBe(true);
    expect(vi.mocked(resumeTask)).toHaveBeenCalledWith('mock-task');
  });

  it('download:pause rejects non-string ids at the boundary', () => {
    expect(() => handlers.get('download:pause')!(evt(), 123)).toThrow();
  });

  it('download:list returns the queue snapshot', async () => {
    const { getAllTasks } = await import('./queue.js');
    expect(handlers.get('download:list')!(evt())).toEqual([{ taskId: 't1' }]);
    expect(vi.mocked(getAllTasks)).toHaveBeenCalled();
  });

  it('download:remove maps a validated id array to removeTasks', async () => {
    const { removeTasks } = await import('./queue.js');
    expect(handlers.get('download:remove')!(evt(), ['a', 'b'])).toBe(true);
    expect(vi.mocked(removeTasks)).toHaveBeenCalledWith(['a', 'b']);
  });

  it('download:remove rejects a non-array payload', () => {
    expect(() => handlers.get('download:remove')!(evt(), 'a')).toThrow(/array/);
  });

  it('download:remove rejects an array with a non-string id', () => {
    expect(() => handlers.get('download:remove')!(evt(), ['ok', 42])).toThrow();
  });

  it('dialog:selectDir returns the chosen path', async () => {
    const result = await handlers.get('dialog:selectDir')!(evt());
    expect(result).toBe('C:\\Picked');
  });

  it('dialog:selectDir returns null when cancelled', async () => {
    vi.mocked(dialog.showOpenDialog).mockResolvedValueOnce({ canceled: true, filePaths: [] });
    expect(await handlers.get('dialog:selectDir')!(evt())).toBeNull();
  });

  it('shell:showItemInFolder reveals a file confined to the output dir', () => {
    vi.mocked(existsSync).mockReturnValueOnce(true);
    const result = handlers.get('shell:showItemInFolder')!(evt(), 'C:\\Downloads\\song.mp3');
    expect(shell.showItemInFolder).toHaveBeenCalledWith('C:\\Downloads\\song.mp3');
    expect(result).toEqual({ ok: true });
  });

  // A file recorded weeks ago may be gone. showItemInFolder on a missing path
  // opens nothing and reports nothing, so the renderer needs the answer to say so.
  it('shell:showItemInFolder reports a missing file instead of silently doing nothing', () => {
    vi.mocked(existsSync).mockReturnValueOnce(false);
    const result = handlers.get('shell:showItemInFolder')!(evt(), 'C:\\Downloads\\gone.mp3');
    expect(shell.showItemInFolder).not.toHaveBeenCalled();
    expect(result).toEqual({ ok: false, reason: 'missing' });
  });

  it('shell:showItemInFolder reports an empty path without touching the shell', () => {
    expect(handlers.get('shell:showItemInFolder')!(evt(), '   ')).toEqual({ ok: false, reason: 'no-path' });
    expect(shell.showItemInFolder).not.toHaveBeenCalled();
  });

  it('shell:showItemInFolder rejects a path escaping the output dir', () => {
    expect(() => handlers.get('shell:showItemInFolder')!(evt(), 'C:\\Windows\\System32\\cmd.exe'))
      .toThrow(/output directory/);
    expect(shell.showItemInFolder).not.toHaveBeenCalled();
  });

  it('app:openSupport opens the main-side https support link (no renderer input)', () => {
    handlers.get('app:openSupport')!(evt());
    expect(shell.openExternal).toHaveBeenCalledWith('https://discord.gg/NMRgaSQVNx');
  });

  it('license:release delegates to releaseLicense', async () => {
    await handlers.get('license:release')!(evt());
    expect(license.releaseLicense).toHaveBeenCalledOnce();
  });

  it('update:* delegate to the Updater', () => {
    handlers.get('update:check')!(evt());
    handlers.get('update:download')!(evt());
    handlers.get('update:install')!(evt());
    expect(updater.check).toHaveBeenCalledOnce();
    expect(updater.download).toHaveBeenCalledOnce();
    expect(updater.install).toHaveBeenCalledOnce();
  });

  // Seam #45 wiring: every registered handler is the secured wrapper, so an invoke
  // from a spoofed sub-frame is rejected before the handler body runs.
  it('a secured handler rejects a spoofed sub-frame sender before delegating', () => {
    const parent: any = { url: 'http://localhost:5173/' };
    parent.top = parent;
    const subFrame = { url: 'http://localhost:5173/', top: parent }; // top !== self → nested
    expect(() => handlers.get('config:getAll')!({ senderFrame: subFrame }))
      .toThrow(/main frame/);
    expect(config.getAll).not.toHaveBeenCalled();
  });
});
