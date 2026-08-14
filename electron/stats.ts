// Monthly download counters.
//
// WHY THIS EXISTS: every number the dashboard could show was derived from the
// download history, which is capped at 200 entries and evicts completed rows
// oldest-first (queue.ts). A "total downloaded" built on that starts DECREASING
// once eviction kicks in — a statistic that silently forgets is worse than none.
//
// So counting happens here instead, at the moment a download finishes, into a
// bucket keyed by month. Two buckets are kept: the current one and the previous,
// which is what lets the tile say "-18% vs last month" and gives it something to
// show on the 1st instead of a lonely zero. Older months are dropped — this is a
// counter, not an archive, and it must never grow without bound.
//
// It counts FILES, not tasks: one playlist task can write twenty files, and the
// engine tells us exactly which ones (yt-dlp's `--print after_move:filepath`,
// collected by downloader.ts and passed through on completion).
import { existsSync, readFileSync, writeFileSync, statSync } from 'node:fs';
import path from 'node:path';
import type { DownloadStats, MonthBucket, SourcePlatform } from '../shared/types.js';

const emptyBucket = (month: string): MonthBucket => ({
  month,
  files: 0,
  bytes: 0,
  byPlatform: {},
});

/** 'YYYY-MM' in LOCAL time — the user's month, not UTC's. */
export function monthKey(at: Date = new Date()): string {
  return `${at.getFullYear()}-${String(at.getMonth() + 1).padStart(2, '0')}`;
}

interface StatsFile {
  current: MonthBucket;
  previous: MonthBucket | null;
}

export class StatsStore {
  private file = '';
  private data: StatsFile = { current: emptyBucket(monthKey()), previous: null };

  /** Load (or start) the counters. Corrupt/missing file simply starts fresh —
   *  statistics are never worth failing a launch over. */
  init(file: string): void {
    this.file = file;
    try {
      if (existsSync(file)) {
        const raw = JSON.parse(readFileSync(file, 'utf-8')) as Partial<StatsFile>;
        if (raw.current?.month) {
          this.data = {
            current: { ...emptyBucket(raw.current.month), ...raw.current },
            previous: raw.previous?.month ? { ...emptyBucket(raw.previous.month), ...raw.previous } : null,
          };
        }
      }
    } catch {
      // unreadable / malformed → start clean rather than throw at boot
    }
    this.roll();
  }

  /** Move to the current month if the calendar has passed the stored one. */
  private roll(at: Date = new Date()): void {
    const key = monthKey(at);
    if (this.data.current.month === key) return;
    // A gap of more than one month means the app wasn't used; the stored current
    // month is still the honest "previous" — it is the last month with activity.
    this.data = { current: emptyBucket(key), previous: this.data.current };
    this.save();
  }

  private save(): void {
    if (!this.file) return;
    try {
      writeFileSync(this.file, JSON.stringify(this.data), 'utf-8');
    } catch {
      // best-effort: a lost counter must never break a download
    }
  }

  /**
   * Record a finished download. `filepaths` is every file the engine actually
   * wrote — one entry for a single track, N for an expanded collection. Sizes
   * are read from disk because a collection's per-file bytes were never
   * reported as progress; `fallbackBytes` covers the case where the paths are
   * unavailable (an instant skip-existing, or a path we cannot stat).
   */
  record(platform: SourcePlatform, filepaths: string[], fallbackBytes = 0): void {
    this.roll();
    const paths = filepaths.filter((p) => p.trim());
    let bytes = 0;
    for (const p of paths) {
      try {
        bytes += statSync(p).size;
      } catch {
        // moved/renamed between finishing and counting — skip its size only
      }
    }
    const files = paths.length || 1; // a completion always counts as at least one
    const cur = this.data.current;
    cur.files += files;
    cur.bytes += bytes || fallbackBytes;
    cur.byPlatform[platform] = (cur.byPlatform[platform] ?? 0) + files;
    this.save();
  }

  /** Snapshot for the renderer. Always rolls first, so a month boundary crossed
   *  while the app sat open is reflected the next time the dashboard asks. */
  get(): DownloadStats {
    this.roll();
    return { current: this.data.current, previous: this.data.previous };
  }
}

/** Default location beside the rest of the app's user data. */
export const statsFilePath = (userData: string): string => path.join(userData, 'download-stats.json');
