import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { startSplash, splashStep, dismissSplash, __resetSplash } from '@/lib/splash';

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
/** Run enough frames that the exponential creep has effectively settled. */
function settle(seconds = 6): void {
  for (let i = 0; i < seconds * 60; i++) frame();
}

const width = () => parseFloat((document.getElementById('splash-fill') as HTMLElement).style.width);
const status = () => document.getElementById('splash-status')?.textContent;

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
      <div id="splash-fill" style="width:5%"></div>
      <div id="splash-status"></div>
    </div>`;
  __resetSplash();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('splash progress', () => {
  it('keeps moving while a boot step is still running', () => {
    startSplash();
    const before = width();
    for (let i = 0; i < 20; i++) frame();
    // The bug this guards: a bar parked at its last milestone reads as hung.
    expect(width()).toBeGreaterThan(before);
  });

  it('never crosses into a step that has not completed', () => {
    startSplash();
    settle(); // creep forever with zero steps done
    // One of three steps in progress => ceiling is the 1/3 boundary (5 + 95/3).
    expect(width()).toBeLessThanOrEqual(5 + 95 / 3 + 0.01);
  });

  it('a completed step raises the ceiling the creep may reach', () => {
    startSplash();
    settle();
    const afterFirst = width();
    splashStep();
    settle();
    expect(width()).toBeGreaterThan(afterFirst);
    expect(width()).toBeLessThanOrEqual(5 + (95 * 2) / 3 + 0.01);
  });

  it('names the step it is on', () => {
    startSplash();
    splashStep();
    expect(status()).toBe('Restoring downloads');
  });
});

describe('dismissSplash', () => {
  it('completes the bar to 100% BEFORE it starts fading', () => {
    startSplash();
    splashStep();
    dismissSplash();

    // Mid-sweep: bar is climbing and the overlay has not begun to leave.
    frame();
    expect(document.getElementById('splash')?.dataset.leaving).toBeUndefined();

    settle(1);
    expect(width()).toBe(100);
    // The completion is allowed to read before the fade is triggered.
    expect(document.getElementById('splash')?.dataset.leaving).toBeUndefined();

    vi.advanceTimersByTime(250);
    expect(document.getElementById('splash')?.dataset.leaving).toBe('true');
  });

  it('removes the overlay so it cannot swallow clicks', () => {
    startSplash();
    dismissSplash();
    settle(1);
    vi.advanceTimersByTime(1000);
    expect(document.getElementById('splash')).toBeNull();
  });

  it('is safe to call twice — the watchdog and the ready path both fire it', () => {
    startSplash();
    dismissSplash();
    expect(() => dismissSplash()).not.toThrow();
    settle(1);
    vi.advanceTimersByTime(1000);
    expect(document.getElementById('splash')).toBeNull();
  });

  it('does nothing when the splash is already gone', () => {
    document.body.innerHTML = '';
    expect(() => { startSplash(); splashStep(); dismissSplash(); }).not.toThrow();
  });
});
