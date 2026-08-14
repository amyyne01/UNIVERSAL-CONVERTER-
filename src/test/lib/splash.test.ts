import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { startSplash, dismissSplash, __resetSplash } from '@/lib/splash';

// The module animates on rAF, so the test drives the frames itself rather than
// waiting on wall-clock time — otherwise these assertions are races.
let queue: FrameRequestCallback[] = [];
let clock = 0;

function frame(ms = 16): void {
  clock += ms;
  const due = queue;
  queue = [];
  due.forEach((cb) => cb(clock));
}
function settle(seconds = 2): void {
  for (let i = 0; i < seconds * 60; i++) frame();
}

const fill = () => document.getElementById('splash-fill') as HTMLElement;
const width = () => parseFloat(fill().style.width);
const leaving = () => document.getElementById('splash')?.dataset.leaving;

/** jsdom has no layout, so getBoundingClientRect is 0 — pin a track/fill size. */
function stubGeometry(fillPx: number, trackPx = 400): void {
  const track = fill().parentElement as HTMLElement;
  track.getBoundingClientRect = () => ({ width: trackPx }) as DOMRect;
  fill().getBoundingClientRect = () => ({ width: fillPx }) as DOMRect;
}

beforeEach(() => {
  clock = 0;
  queue = [];
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
    queue.push(cb);
    return queue.length;
  });
  vi.stubGlobal('cancelAnimationFrame', () => { queue = []; });
  vi.useFakeTimers();
  document.body.innerHTML = `
    <div id="splash">
      <div class="sp-trace"><div id="splash-fill"></div></div>
      <div id="splash-status"></div>
    </div>`;
  stubGeometry(200);
  __resetSplash();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('dismissSplash', () => {
  // The bar is ramped by a CSS animation in index.html so it runs without JS,
  // even if this bundle stalls. This module only takes it over at the right time.
  it('holds the splash up long enough for the bar to be seen', () => {
    startSplash();
    dismissSplash(); // ready almost immediately
    settle(1);
    expect(leaving()).toBeUndefined(); // still visible, deliberately

    vi.advanceTimersByTime(2500); // past MIN_VISIBLE_MS
    settle(1);
    vi.advanceTimersByTime(300);
    expect(leaving()).toBe('true');
  });

  it('freezes the CSS ramp where it got to, without snapping backwards', () => {
    startSplash();
    vi.advanceTimersByTime(2500);
    stubGeometry(240); // ramp reached 60% of a 400px track
    dismissSplash();

    // Pinned to the ramp's position before the sweep starts — never back to 5%.
    expect(width()).toBeCloseTo(60, 0);
    expect(fill().style.animation).toBe('none');
  });

  it('completes to 100% BEFORE it starts fading', () => {
    startSplash();
    vi.advanceTimersByTime(2500);
    dismissSplash();

    frame();
    expect(leaving()).toBeUndefined(); // mid-sweep, not leaving yet

    settle(1);
    expect(width()).toBe(100);
    expect(leaving()).toBeUndefined(); // completion is allowed to read first

    vi.advanceTimersByTime(250);
    expect(leaving()).toBe('true');
  });

  it('removes the overlay so it cannot swallow clicks', () => {
    startSplash();
    vi.advanceTimersByTime(2500);
    dismissSplash();
    settle(1);
    vi.advanceTimersByTime(1000);
    expect(document.getElementById('splash')).toBeNull();
  });

  it('is safe to call twice — the watchdog and the ready path both fire it', () => {
    startSplash();
    vi.advanceTimersByTime(2500);
    dismissSplash();
    expect(() => dismissSplash()).not.toThrow();
    settle(1);
    vi.advanceTimersByTime(1000);
    expect(document.getElementById('splash')).toBeNull();
  });

  it('stops the message loop before dismissing, so it cannot animate a removed node', () => {
    const stop = vi.fn();
    (window as unknown as { __ahgSplash: { stop: () => void } }).__ahgSplash = { stop };
    startSplash();
    vi.advanceTimersByTime(2500);
    dismissSplash();
    expect(stop).toHaveBeenCalledOnce();
  });

  it('still completes the bar under prefers-reduced-motion', () => {
    vi.stubGlobal('matchMedia', vi.fn(() => ({ matches: true })));
    __resetSplash();
    startSplash();
    vi.advanceTimersByTime(2500);
    dismissSplash();
    expect(width()).toBe(100);
    vi.advanceTimersByTime(1000);
    expect(document.getElementById('splash')).toBeNull();
  });

  it('does nothing when the splash is already gone', () => {
    document.body.innerHTML = '';
    expect(() => { startSplash(); dismissSplash(); }).not.toThrow();
  });
});
