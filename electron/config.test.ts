import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('electron', () => ({
  app: { getPath: vi.fn((name: string) => (name === 'downloads' ? 'C:\\Downloads' : 'C:\\userData')) },
}));

vi.mock('node:fs', () => {
  const readFileSync = vi.fn();
  const writeFileSync = vi.fn();
  const renameSync = vi.fn();
  return { default: { readFileSync, writeFileSync, renameSync }, readFileSync, writeFileSync, renameSync };
});

import { ConfigManager, defaultConfig, migrateConfig } from './config';
import { readFileSync, writeFileSync, renameSync } from 'node:fs';

const mockRead = vi.mocked(readFileSync);
const mockWrite = vi.mocked(writeFileSync);
const mockRename = vi.mocked(renameSync);

beforeEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
  // default: no saved file on disk → ConfigManager falls back to defaults
  mockRead.mockImplementation(() => {
    throw new Error('ENOENT');
  });
});

describe('ConfigManager', () => {
  it('fills missing keys with defaults', () => {
    mockRead.mockReturnValue(JSON.stringify({ outputDir: 'D:\\Music' }));
    const cfg = new ConfigManager();
    expect(cfg.get('outputDir')).toBe('D:\\Music'); // saved value wins
    expect(cfg.get('defaultFormat')).toBe('mp3'); // missing → default
    expect(cfg.get('theme')).toBe('system');
    expect(cfg.get('globalHotkey')).toBe('CommandOrControl+Shift+D');
    expect(cfg.get('scheduleDays')).toEqual([]);
  });

  it('falls back to defaults when the file is missing/corrupt', () => {
    const cfg = new ConfigManager();
    // outputDir is anchored to the Downloads folder rather than left empty.
    expect(cfg.getAll()).toEqual({ ...defaultConfig, outputDir: 'C:\\Downloads' });
  });

  it('anchors an empty outputDir to the OS Downloads folder', () => {
    mockRead.mockReturnValue(JSON.stringify({ outputDir: '' }));
    const cfg = new ConfigManager();
    expect(cfg.get('outputDir')).toBe('C:\\Downloads');
  });

  it('rejects set/update values whose type mismatches the default', () => {
    const cfg = new ConfigManager();
    cfg.set('scheduleDays', 'mon' as unknown as number[]); // string, want array
    expect(cfg.get('scheduleDays')).toEqual([]);
    cfg.set('scheduleTime', 300 as unknown as string); // number, want string
    expect(cfg.get('scheduleTime')).toBe('03:00');
    cfg.update({ skipExisting: 'yes' as unknown as boolean, defaultFormat: 'flac' });
    expect(cfg.get('skipExisting')).toBe(true); // rejected
    expect(cfg.get('defaultFormat')).toBe('flac'); // valid, applied
  });

  it('strips unknown keys on load', () => {
    mockRead.mockReturnValue(JSON.stringify({ outputDir: 'X', bogusKey: 'nope' }));
    const cfg = new ConfigManager();
    expect(cfg.get('outputDir')).toBe('X');
    expect((cfg.getAll() as Record<string, unknown>).bogusKey).toBeUndefined();
  });

  it('rejects __proto__ to prevent prototype pollution', () => {
    mockRead.mockReturnValue('{"__proto__":{"polluted":true},"constructor":{"x":1},"outputDir":"Y"}');
    const cfg = new ConfigManager();
    expect(({} as Record<string, unknown>).polluted).toBeUndefined();
    expect((cfg.getAll() as Record<string, unknown>).polluted).toBeUndefined();
    expect(cfg.get('outputDir')).toBe('Y');
  });

  it('update merges partial values and strips unknown/dangerous keys', () => {
    const cfg = new ConfigManager();
    cfg.update({ outputDir: 'Z', defaultQuality: '192' });
    expect(cfg.get('outputDir')).toBe('Z');
    expect(cfg.get('defaultQuality')).toBe('192');
    expect(cfg.get('defaultFormat')).toBe('mp3'); // untouched

    cfg.update({ bogus: 1, ['__proto__']: { polluted: true } } as unknown as Partial<typeof defaultConfig>);
    expect((cfg.getAll() as Record<string, unknown>).bogus).toBeUndefined();
    expect(({} as Record<string, unknown>).polluted).toBeUndefined();
  });

  it('saveNow writes config to disk immediately', () => {
    const cfg = new ConfigManager();
    cfg.set('outputDir', 'W');
    cfg.saveNow();
    expect(mockWrite).toHaveBeenCalledTimes(1);
    const [, content] = mockWrite.mock.calls[0];
    expect(content).toContain('"outputDir": "W"');
  });

  it('debounces set writes (~400ms)', () => {
    vi.useFakeTimers();
    const cfg = new ConfigManager();
    cfg.set('proxy', 'http://proxy');
    expect(mockWrite).not.toHaveBeenCalled(); // not written yet
    vi.advanceTimersByTime(400);
    expect(mockWrite).toHaveBeenCalledTimes(1);
  });

  it('B2/B3: onChange fires after set() and update() so services can re-sync', () => {
    const cfg = new ConfigManager();
    const cb = vi.fn();
    cfg.onChange(cb);
    cfg.set('globalHotkey', 'CommandOrControl+Shift+X');
    expect(cb).toHaveBeenCalledTimes(1);
    cfg.update({ discordRichPresence: true });
    expect(cb).toHaveBeenCalledTimes(2);
  });

  it('B2/B3: the unsubscribe returned by onChange stops further notifications', () => {
    const cfg = new ConfigManager();
    const cb = vi.fn();
    const unsubscribe = cfg.onChange(cb);
    unsubscribe();
    cfg.set('globalHotkey', 'CommandOrControl+Shift+X');
    expect(cb).not.toHaveBeenCalled();
  });

  it('B12: set() re-anchors an empty outputDir to the OS Downloads folder', () => {
    const cfg = new ConfigManager();
    cfg.set('outputDir', 'D:\\Music');
    cfg.set('outputDir', ''); // a compromised renderer (or UI bug) clearing it
    expect(cfg.get('outputDir')).toBe('C:\\Downloads');
  });

  it('B12: update() re-anchors an empty outputDir to the OS Downloads folder', () => {
    const cfg = new ConfigManager();
    cfg.update({ outputDir: '' });
    expect(cfg.get('outputDir')).toBe('C:\\Downloads');
  });

  it('B13: write() is atomic — writes to a .tmp file then renames it over config.json', () => {
    const cfg = new ConfigManager();
    cfg.set('outputDir', 'W');
    cfg.saveNow();
    expect(mockWrite).toHaveBeenCalledTimes(1);
    const [tmpPath] = mockWrite.mock.calls[0];
    expect(tmpPath).toMatch(/\.tmp$/);
    expect(mockRename).toHaveBeenCalledTimes(1);
    const [renameFrom, renameTo] = mockRename.mock.calls[0];
    expect(renameFrom).toBe(tmpPath);
    expect(renameTo).not.toMatch(/\.tmp$/);
  });

  it('B14: rejects array values whose elements are not the expected type', () => {
    const cfg = new ConfigManager();
    // Hand-edited config with string days instead of numbers — must be rejected so
    // `.includes(getDay())` (a number) can't silently never match.
    cfg.set('scheduleDays', ['1', '3', '5'] as unknown as number[]);
    expect(cfg.get('scheduleDays')).toEqual([]); // rejected, still default
    cfg.set('scheduleDays', [1, 3, 5]);
    expect(cfg.get('scheduleDays')).toEqual([1, 3, 5]); // valid, applied
  });

  it('seam #43: write() rotates config.json -> backup.1, shifting 1->2->3', () => {
    // simulate an existing config.json plus a pre-existing backup.1
    mockRead.mockImplementation((p: unknown) => {
      const filePath = String(p);
      if (filePath.endsWith('config.backup.1.json')) return JSON.stringify({ outputDir: 'OldBackup1' });
      if (filePath.endsWith('config.json')) return JSON.stringify({ outputDir: 'CurrentOnDisk' });
      throw new Error('ENOENT'); // backup.2, backup.3 don't exist yet
    });
    const cfg = new ConfigManager();
    cfg.set('outputDir', 'NewValue');
    cfg.saveNow();

    const writes = mockWrite.mock.calls.map(([p, content]) => [String(p), content]);
    const backup2 = writes.find(([p]) => p.endsWith('config.backup.2.json'));
    const backup1 = writes.find(([p]) => p.endsWith('config.backup.1.json'));
    expect(backup2?.[1]).toBe(JSON.stringify({ outputDir: 'OldBackup1' })); // old backup.1 -> backup.2
    expect(backup1?.[1]).toBe(JSON.stringify({ outputDir: 'CurrentOnDisk' })); // old config.json -> backup.1
  });

  it('seam #43: a corrupt config.json is restored from the backup ring, not reset to defaults', () => {
    mockRead.mockImplementation((p: unknown) => {
      const filePath = String(p);
      if (filePath.endsWith('config.backup.1.json')) throw new Error('corrupt');
      if (filePath.endsWith('config.backup.2.json')) {
        return JSON.stringify({ outputDir: 'FromRing', schemaVersion: 1 });
      }
      if (filePath.endsWith('config.json')) return '{not valid json';
      throw new Error('ENOENT');
    });
    const cfg = new ConfigManager();
    expect(cfg.get('outputDir')).toBe('FromRing');
    expect(cfg.wasRestoredFromBackup()).toBe(true);
  });

  it('seam #43: falls back to defaults (not restored) when config.json and the whole ring are unreadable', () => {
    const cfg = new ConfigManager(); // beforeEach: every readFileSync throws ENOENT
    expect(cfg.wasRestoredFromBackup()).toBe(false);
    expect(cfg.getAll()).toEqual({ ...defaultConfig, outputDir: 'C:\\Downloads' });
  });

  it('seam #43: a config saved before schemaVersion existed loads and is stamped to the current version', () => {
    mockRead.mockReturnValue(JSON.stringify({ outputDir: 'X' })); // no schemaVersion field
    const cfg = new ConfigManager();
    expect(cfg.get('schemaVersion')).toBe(1);
  });

  it('seam #43: migrateConfig runs migrations in order from the loaded schemaVersion (absent => 0)', () => {
    expect(migrateConfig({ outputDir: 'X' }).schemaVersion).toBe(1); // no field -> treated as v0
    expect(migrateConfig({ outputDir: 'X', schemaVersion: 0 }).schemaVersion).toBe(1);
    expect(migrateConfig({ outputDir: 'X', schemaVersion: 1 }).schemaVersion).toBe(1); // already current, no-op
  });
});
