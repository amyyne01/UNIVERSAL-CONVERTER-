import { describe, it, expect } from 'vitest';
import type { DownloadStatus } from '../shared/types.js';
import { transitions, canTransition, assertTransition } from './state-machine';

const ALL = Object.keys(transitions) as DownloadStatus[];

describe('state-machine', () => {
  it('canTransition matches the transition table for every (from, to) pair', () => {
    for (const from of ALL) {
      const allowed = new Set(transitions[from]);
      for (const to of ALL) {
        expect(canTransition(from, to)).toBe(allowed.has(to));
      }
    }
  });

  it('allows the full forward path queued -> ... -> done', () => {
    const path: DownloadStatus[] = [
      'queued', 'fetching_info', 'downloading', 'converting', 'embedding', 'done',
    ];
    for (let i = 0; i < path.length - 1; i++) {
      expect(canTransition(path[i], path[i + 1])).toBe(true);
    }
  });

  it('treats done and cancelled as terminal (no exits)', () => {
    expect(transitions.done).toEqual([]);
    expect(transitions.cancelled).toEqual([]);
  });

  it('never lists a self-transition', () => {
    for (const s of ALL) expect(transitions[s]).not.toContain(s);
  });

  it('assertTransition throws only on illegal transitions', () => {
    expect(() => assertTransition('downloading', 'converting')).not.toThrow();
    expect(() => assertTransition('done', 'downloading')).toThrow(/Illegal/);
  });
});
