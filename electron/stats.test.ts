import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// In-memory fs; statSync returns a fixed size per path so byte sums are checkable.
vi.mock('node:fs', () => {
  const store = new Map<string, string>();
  const sizes = new Map<string, number>();
  const api = {
    existsSync: vi.fn((p: string) => store.has(p)),
    readFileSync: vi.fn((p: string) => {
      if (!store.has(p)) throw new Error('ENOENT');
      return store.get(p);
    }),
    writeFileSync: vi.fn((p: string, data: unknown) => { store.set(p, String(data)); }),
    statSync: vi.fn((p: string) => {
      if (!sizes.has(p)) throw new Error('ENOENT');
      return { size: sizes.get(p) };
    }),
  };
  // `default` too: anything in the import graph that uses `import fs from 'node:fs'`
  // gets the same object, instead of failing to resolve the module entirely.
  return { ...api, _store: store, _sizes: sizes, default: api };
});

import { StatsStore, monthKey } from './stats';
// eslint-disable-next-line @typescript-eslint/no-explicit-any
import * as fs from 'node:fs';

const store = (fs as any)._store as Map<string, string>;
const sizes = (fs as any)._sizes as Map<string, number>;
const FILE = 'C:\\userData\\download-stats.json';

beforeEach(() => {
  vi.clearAllMocks();
  store.clear();
  sizes.clear();
  vi.useRealTimers();
});
afterEach(() => vi.useRealTimers());

const at = (iso: string) => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(iso));
};

describe('StatsStore', () => {
  it('counts FILES, not tasks — a collection writes many', () => {
    const s = new StatsStore();
    s.init(FILE);
    sizes.set('a.mp3', 1000);
    sizes.set('b.mp3', 2000);
    sizes.set('c.mp3', 3000);

    s.record('spotify', ['a.mp3', 'b.mp3', 'c.mp3']);

    const { current } = s.get();
    expect(current.files).toBe(3);
    expect(current.bytes).toBe(6000);
    expect(current.byPlatform.spotify).toBe(3);
  });

  it('falls back to the reported total when a path cannot be stat-ed', () => {
    const s = new StatsStore();
    s.init(FILE);
    s.record('youtube', ['gone.mp4'], 4242); // never added to `sizes`
    expect(s.get().current.bytes).toBe(4242);
    expect(s.get().current.files).toBe(1);
  });

  it('an instant finish with no paths still counts as one download', () => {
    const s = new StatsStore();
    s.init(FILE);
    s.record('youtube', [], 0);
    expect(s.get().current.files).toBe(1);
  });

  // The whole reason this file exists: totals must never shrink, and must never
  // be derived from the capped history.
  it('accumulates across many records within a month', () => {
    const s = new StatsStore();
    s.init(FILE);
    sizes.set('x.mp3', 500);
    for (let i = 0; i < 250; i++) s.record('soundcloud', ['x.mp3']);
    expect(s.get().current.files).toBe(250); // past HISTORY_CAP, still counted
    expect(s.get().current.bytes).toBe(125_000);
  });

  it('rolls at the month boundary and keeps the old month as previous', () => {
    at('2026-08-20T10:00:00');
    const s = new StatsStore();
    s.init(FILE);
    sizes.set('x.mp3', 100);
    s.record('youtube', ['x.mp3']);
    expect(s.get().current.month).toBe('2026-08');

    at('2026-09-02T09:00:00');
    const rolled = s.get();
    expect(rolled.current.month).toBe('2026-09');
    expect(rolled.current.files).toBe(0);
    expect(rolled.previous?.month).toBe('2026-08');
    expect(rolled.previous?.files).toBe(1); // last month survives for the delta
  });

  it('treats the last month with activity as previous, even after a long gap', () => {
    at('2026-03-10T10:00:00');
    const s = new StatsStore();
    s.init(FILE);
    sizes.set('x.mp3', 100);
    s.record('youtube', ['x.mp3']);

    at('2026-08-01T10:00:00'); // five idle months
    const rolled = s.get();
    expect(rolled.current.month).toBe('2026-08');
    expect(rolled.previous?.month).toBe('2026-03');
    expect(rolled.previous?.files).toBe(1);
  });

  it('persists across restarts', () => {
    at('2026-08-20T10:00:00');
    const first = new StatsStore();
    first.init(FILE);
    sizes.set('x.mp3', 700);
    first.record('spotify', ['x.mp3']);

    const second = new StatsStore();
    second.init(FILE);
    expect(second.get().current.files).toBe(1);
    expect(second.get().current.bytes).toBe(700);
  });

  it('starts clean on a corrupt file instead of throwing at boot', () => {
    store.set(FILE, '{ not json');
    const s = new StatsStore();
    expect(() => s.init(FILE)).not.toThrow();
    expect(s.get().current.files).toBe(0);
  });

  it('monthKey uses local time, zero-padded', () => {
    expect(monthKey(new Date(2026, 0, 15))).toBe('2026-01');
    expect(monthKey(new Date(2026, 11, 1))).toBe('2026-12');
  });
});
