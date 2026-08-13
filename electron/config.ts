import { app } from 'electron';
import { readFileSync, writeFileSync, renameSync } from 'node:fs';
import path from 'node:path';
import type { AppConfig } from '../shared/types.js';

// HOW-THE-APP-WORKS §10: persisted, human-readable settings in the per-user
// app-data folder. Defaults laid first, saved values merged on top, unknown
// keys stripped, debounced save with one immediate flush at shutdown.
export const defaultConfig: AppConfig = {
  schemaVersion: 1,
  outputDir: '',
  defaultFormat: 'mp3',
  defaultQuality: '320',
  defaultVideoQuality: 'best',
  embedThumbnail: true,
  embedMetadata: true,
  skipExisting: true,
  theme: 'system',
  rememberLastDir: true,
  autoPaste: true,
  showNotifications: true,
  rateLimit: '',
  proxy: '',
  ffmpegPath: '',
  globalHotkey: 'CommandOrControl+Shift+D',
  discordRichPresence: false,
  scheduleEnabled: false,
  scheduleTime: '03:00',
  scheduleDays: [],
  scheduleShutdown: false,
  autoUpdateEngine: true,
};

const DEBOUNCE_MS = 400;
const DANGEROUS_KEYS = new Set(['__proto__', 'constructor', 'prototype']);
const BACKUP_COUNT = 3;

type Migration = (obj: Record<string, unknown>) => Record<string, unknown>;

/** Ordered config-shape migrations, index i takes schemaVersion i -> i+1. Run
 *  from the loaded object's schemaVersion (absent => 0) before the
 *  defaults-merge/key-strip below. Register future config-key changes here —
 *  no renames exist yet, so this seeds the harness with a single stamping step. */
const MIGRATIONS: Migration[] = [
  (obj) => ({ ...obj, schemaVersion: 1 }), // v0 -> v1: establish schemaVersion field
];

/** Exported for direct unit testing of the migration harness. */
export function migrateConfig(obj: Record<string, unknown>): Record<string, unknown> {
  let result = obj;
  const startVersion = typeof result.schemaVersion === 'number' ? result.schemaVersion : 0;
  for (let v = startVersion; v < MIGRATIONS.length; v++) {
    result = MIGRATIONS[v](result);
  }
  return result;
}

export class ConfigManager {
  private config: AppConfig;
  private readonly filePath: string;
  private saveTimer: NodeJS.Timeout | null = null;
  private readonly changeListeners = new Set<() => void>();
  private restoredFromBackup = false;

  constructor() {
    this.filePath = path.join(app.getPath('userData'), 'config.json');
    this.config = this.load();
    // An empty outputDir would make yt-dlp write into the process CWD; anchor it
    // to the OS Downloads folder so first-run (or a hand-cleared value) is sane.
    if (!this.config.outputDir) this.config.outputDir = app.getPath('downloads');
  }

  /** True if the most recent load() had to fall back to the backup ring
   *  because config.json was missing/corrupt but a prior snapshot parsed cleanly. */
  wasRestoredFromBackup(): boolean {
    return this.restoredFromBackup;
  }

  get<K extends keyof AppConfig>(key: K): AppConfig[K] {
    return this.config[key];
  }

  /** Notify on every set()/update() so main-process services (hotkey, presence, …)
   *  can re-sync to a Settings-driven change instead of only reading config at startup. */
  onChange(cb: () => void): () => void {
    this.changeListeners.add(cb);
    return () => this.changeListeners.delete(cb);
  }

  private notifyChange(): void {
    for (const cb of this.changeListeners) {
      try { cb(); } catch { /* one listener's failure shouldn't break the others */ }
    }
  }

  set<K extends keyof AppConfig>(key: K, value: AppConfig[K]): void {
    if (!this.isSafeKey(key as string) || !this.isValidValue(key as string, value)) return;
    this.config[key] = this.normalize(key as string, value) as AppConfig[K];
    this.scheduleSave();
    this.notifyChange();
  }

  update(partial: Partial<AppConfig>): void {
    const src = partial as unknown as Record<string, unknown>;
    for (const key of Object.keys(partial)) {
      if (!this.isSafeKey(key) || !this.isValidValue(key, src[key])) continue;
      (this.config as unknown as Record<string, unknown>)[key] = this.normalize(key, src[key]);
    }
    this.scheduleSave();
    this.notifyChange();
  }

  getAll(): AppConfig {
    return { ...this.config };
  }

  /** Immediate, non-debounced write — call at shutdown. */
  saveNow(): void {
    if (this.saveTimer) {
      clearTimeout(this.saveTimer);
      this.saveTimer = null;
    }
    this.write();
  }

  // ponytail: AppConfig is flat (scheduleDays is the only array, replaced
  // wholesale), so "deep merge" reduces to defaults + sanitized saved keys —
  // no nested recursion needed. Add recursion only if a nested shape lands here.
  private load(): AppConfig {
    const merged: AppConfig = { ...defaultConfig };
    let saved: unknown;
    try {
      saved = JSON.parse(readFileSync(this.filePath, 'utf-8'));
    } catch {
      // missing or corrupt file → walk the backup ring newest-first for the
      // first snapshot that still parses, instead of falling straight to defaults
      const restored = this.restoreFromBackupRing();
      if (restored === undefined) return merged;
      saved = restored;
      this.restoredFromBackup = true;
    }
    if (!saved || typeof saved !== 'object') return merged;
    const migrated = migrateConfig(saved as Record<string, unknown>);
    for (const key of Object.keys(migrated)) {
      if (!this.isSafeKey(key)) continue;
      const value = migrated[key];
      if (!this.isValidValue(key, value)) continue;
      (merged as unknown as Record<string, unknown>)[key] = value;
    }
    return merged;
  }

  private backupPath(n: number): string {
    return path.join(path.dirname(this.filePath), `config.backup.${n}.json`);
  }

  /** Walk config.backup.1.json..N newest-first, returning the first one that parses. */
  private restoreFromBackupRing(): Record<string, unknown> | undefined {
    for (let n = 1; n <= BACKUP_COUNT; n++) {
      try {
        const parsed = JSON.parse(readFileSync(this.backupPath(n), 'utf-8'));
        if (parsed && typeof parsed === 'object') return parsed as Record<string, unknown>;
      } catch { /* this ring slot is missing/corrupt too — try the next */ }
    }
    return undefined;
  }

  /** Shift backup.1→2→3 (oldest dropped), then the about-to-be-overwritten
   *  config.json becomes the new backup.1. Called just before write(). */
  private rotateBackups(): void {
    for (let n = BACKUP_COUNT; n >= 2; n--) {
      try {
        const content = readFileSync(this.backupPath(n - 1), 'utf-8');
        writeFileSync(this.backupPath(n), content, 'utf-8');
      } catch { /* that ring slot doesn't exist yet */ }
    }
    try {
      const current = readFileSync(this.filePath, 'utf-8');
      writeFileSync(this.backupPath(1), current, 'utf-8');
    } catch { /* no config.json yet (first write) */ }
  }

  /** Whitelist + prototype-pollution guard: only known, non-dangerous keys. */
  private isSafeKey(key: string): boolean {
    return !DANGEROUS_KEYS.has(key) && Object.prototype.hasOwnProperty.call(defaultConfig, key);
  }

  /** Accept a value only when its runtime type matches the default's. The config
   *  file is user-hand-editable and set/update take renderer input, so a mistyped
   *  `"scheduleDays": "mon"` or numeric hotkey must be rejected here rather than
   *  throw deep in the scheduler / globalShortcut. */
  private isValidValue(key: string, value: unknown): boolean {
    const def = (defaultConfig as unknown as Record<string, unknown>)[key];
    if (Array.isArray(def) !== Array.isArray(value)) return false;
    if (Array.isArray(value)) return value.every((v) => typeof v === 'number');
    return typeof value === typeof def;
  }

  /** Re-anchor an empty outputDir to the OS Downloads folder on every write, not just
   *  construction — a compromised renderer (or a future UI bug) could otherwise empty
   *  it via set()/update(), sending downloads to the process CWD. */
  private normalize(key: string, value: unknown): unknown {
    if (key === 'outputDir' && !value) return app.getPath('downloads');
    return value;
  }

  private scheduleSave(): void {
    if (this.saveTimer) clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => {
      this.saveTimer = null;
      this.write();
    }, DEBOUNCE_MS);
  }

  private write(): void {
    // config only ever holds whitelisted keys (load/set/update strip), so the
    // whole object is already the curated save whitelist. Swallow write failures
    // (disk full, locked file) — this runs from a debounce timer and on quit,
    // where an uncaught throw would crash the main process.
    try {
      // Rotate the backup ring before overwriting, so a later corrupt config.json
      // can be restored from a recent-good snapshot instead of resetting to defaults.
      this.rotateBackups();
      // Atomic write: a crash mid-write to the tmp file leaves the real config.json
      // untouched, instead of truncated (which load() would silently factory-reset).
      const tmpPath = `${this.filePath}.tmp`;
      writeFileSync(tmpPath, JSON.stringify(this.config, null, 2), 'utf-8');
      renameSync(tmpPath, this.filePath);
    } catch { /* best-effort persistence */ }
  }
}
