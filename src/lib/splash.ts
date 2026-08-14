// Drives the boot splash that index.html paints before this bundle exists.
// The markup lives outside #root, so React never owns it — these calls are the
// whole contract, and every one of them is a no-op once the splash is gone.
//
// Progress has two parts, and both are honest:
//   MILESTONES — splashStep() on each boot step that actually completed. A step
//                raises the ceiling; it never paints the bar directly.
//   CREEP      — the bar eases toward that ceiling continuously and never crosses
//                it. A boot step can take an unknown time (cold disk, slow IPC),
//                and a bar frozen at 67% reads as hung — the one thing a loading
//                screen must never do. Because the approach decelerates, it can
//                chase the boundary forever while only a real completed step is
//                allowed past it.
//
// rAF is the SOLE owner of the width. index.html deliberately carries no CSS
// transition on it: a per-frame write plus a 340ms transition would restart that
// transition every frame, and the bar would crawl instead of track.
const STEPS = ['Reading settings', 'Restoring downloads', 'Ready'] as const;

/** Where the bar starts, matching #splash-fill's initial width in index.html. */
const START = 5;
/** Fraction of the remaining gap closed per second while waiting. */
const CREEP_RATE = 0.9;
/** The closing sweep to 100%. */
const COMPLETE_MS = 320;
/** How long a finished bar reads before the fade begins. */
const COMPLETE_HOLD_MS = 200;
/** Floor for removing the node when transitionend never arrives. */
const REMOVE_FLOOR_MS = 400;

let done = 0;
let shown = START;
let raf = 0;
let last = 0;
let finishing = false;

const fillEl = () => document.getElementById('splash-fill');
const reduced = () =>
  typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

/** Upper bound the creep may approach: the boundary of the step now in progress. */
function ceiling(): number {
  return START + ((100 - START) * Math.min(done + 1, STEPS.length)) / STEPS.length;
}

/** Where the bar sits once `done` steps have genuinely completed. */
function milestone(): number {
  return START + ((100 - START) * done) / STEPS.length;
}

function paint(pct: number): void {
  const fill = fillEl();
  if (fill) fill.style.width = `${pct}%`;
}

function tick(now: number): void {
  if (finishing) return;
  const dt = last ? Math.min((now - last) / 1000, 0.25) : 0;
  last = now;
  // Exponential approach: quick while far, imperceptible when close.
  shown += (ceiling() - shown) * (1 - Math.pow(1 - CREEP_RATE, dt));
  paint(shown);
  raf = requestAnimationFrame(tick);
}

function startCreep(): void {
  if (raf || finishing || !fillEl()) return;
  if (reduced()) return; // no motion: milestones paint directly instead
  last = 0;
  raf = requestAnimationFrame(tick);
}

function stopCreep(): void {
  if (raf) cancelAnimationFrame(raf);
  raf = 0;
}

/** Begin moving before the first step lands, so the bar is alive from frame one. */
export function startSplash(): void {
  if (!fillEl()) return;
  startCreep();
}

/** Record one completed boot step. The creep carries the bar to the new ceiling. */
export function splashStep(): void {
  if (!fillEl() || finishing) return;
  done = Math.min(done + 1, STEPS.length);
  const status = document.getElementById('splash-status');
  if (status) status.textContent = STEPS[Math.min(done, STEPS.length - 1)];
  if (reduced()) {
    shown = milestone();
    paint(shown);
    return;
  }
  startCreep();
}

/**
 * Complete, then leave. The bar runs to 100% and is allowed to READ as finished
 * before the fade starts — the exit is "line resolves, then lights off", in that
 * order. Safe to call twice: `finishing` guards it and the node is gone after.
 */
export function dismissSplash(): void {
  const splash = document.getElementById('splash');
  if (!splash || finishing) return;
  finishing = true;
  stopCreep();

  const status = document.getElementById('splash-status');
  if (status) status.textContent = STEPS[STEPS.length - 1];

  const leave = () => {
    splash.dataset.leaving = 'true';
    // Remove on the fade's own event so a reduced-motion user (transition: none)
    // isn't left with a dead overlay swallowing clicks; the timeout is the floor
    // for browsers that skip transitionend when the property never animates.
    // Target-filtered: only the overlay's own fade ends the splash.
    const remove = () => splash.remove();
    splash.addEventListener('transitionend', (e) => {
      if (e.target === splash) remove();
    });
    setTimeout(remove, REMOVE_FLOOR_MS);
  };

  if (reduced()) {
    paint(100);
    leave();
    return;
  }

  const from = shown;
  let t0 = 0;
  const sweep = (now: number) => {
    if (!t0) t0 = now;
    const p = Math.min((now - t0) / COMPLETE_MS, 1);
    const eased = 1 - Math.pow(1 - p, 3); // ease-out cubic, no overshoot
    shown = from + (100 - from) * eased;
    paint(shown);
    if (p < 1) raf = requestAnimationFrame(sweep);
    else {
      raf = 0;
      setTimeout(leave, COMPLETE_HOLD_MS);
    }
  };
  raf = requestAnimationFrame(sweep);
}

/** Test seam: this module holds boot state that must not leak between cases. */
export function __resetSplash(): void {
  stopCreep();
  done = 0;
  shown = START;
  last = 0;
  finishing = false;
}
