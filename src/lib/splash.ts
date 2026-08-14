// Drives the boot splash that index.html paints before this bundle exists.
// The markup lives outside #root, so React never owns it.
//
// Division of labour, and it is deliberate:
//   index.html      — ramps the bar from 5% to 92% with a CSS animation. No JS, so
//                     it runs on the first painted frame and keeps running even if
//                     this bundle is slow, stalls, or never loads at all. A bar
//                     that freezes exactly when the app is struggling is the one
//                     case where it matters most.
//   splash-boot.js  — the wordmark reveal and the looping status line.
//   this module     — closes the bar to 100% and takes the splash away, and it is
//                     the ONLY thing that can. A full bar therefore always means a
//                     usable app, never "the animation ran out".
const START = 5;
/** The closing sweep once the app is ready. */
const COMPLETE_MS = 420;
/** How long a finished bar reads before the fade begins. */
const COMPLETE_HOLD_MS = 200;
/** Floor for removing the node when transitionend never arrives. */
const REMOVE_FLOOR_MS = 400;
/**
 * Minimum time the splash stays up. Boot is often under a second, so without this
 * the bar is created, finished and gone before it has drawn enough frames to read
 * as movement — indistinguishable from a bar that never animated.
 *
 * Deliberately longer than it needs to be. It covers the wordmark's ~2.2s reveal
 * AND at least one swap of the status line (the first lands at 1.5s), so the boot
 * screen is actually SEEN rather than glimpsed. This is a product decision, not a
 * technical floor: the app is ready well before this elapses, and the only thing
 * being waited on is the user's eye.
 */
const MIN_VISIBLE_MS = 3400;

/** Set by public/splash-boot.js, which runs before this bundle exists. */
declare global {
  interface Window {
    __ahgSplash?: { stop: () => void };
  }
}

let bootAt = 0;
let finishing = false;
let raf = 0;

const fillEl = () => document.getElementById('splash-fill');
const reduced = () =>
  typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

/** Current bar position as a percentage of its track, mid-CSS-animation. */
function currentPct(fill: HTMLElement): number {
  const track = fill.parentElement;
  if (!track) return START;
  const w = fill.getBoundingClientRect().width;
  const t = track.getBoundingClientRect().width;
  return t > 0 ? (w / t) * 100 : START;
}

/** Start the visibility clock. The CSS ramp is already running by now. */
export function startSplash(): void {
  if (!bootAt) bootAt = Date.now();
}

/**
 * Complete, then leave. The bar runs to 100% and is allowed to READ as finished
 * before the fade starts — the exit is "line resolves, then lights off", in that
 * order. Safe to call twice: the node is gone after, and `finishing` guards it.
 */
export function dismissSplash(): void {
  const splash = document.getElementById('splash');
  if (!splash || finishing) return;

  // Ready too soon: let the bar be seen before finishing it.
  const waited = bootAt ? Date.now() - bootAt : MIN_VISIBLE_MS;
  if (waited < MIN_VISIBLE_MS) {
    finishing = true;
    setTimeout(() => {
      finishing = false;
      dismissSplash();
    }, MIN_VISIBLE_MS - waited);
    return;
  }

  finishing = true;
  // Stop the message loop first: it schedules its own timers and rebuilds the
  // status line, and it must not animate a node that is on its way out.
  window.__ahgSplash?.stop();

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

  const fill = fillEl();
  if (!fill) {
    leave();
    return;
  }

  // Hand the bar over from CSS to here: freeze where the ramp got to, then close
  // the rest. Without pinning the width first, killing the animation would snap
  // the bar back to its 5% base for a frame.
  const from = currentPct(fill);
  fill.style.animation = 'none';
  fill.style.width = `${from}%`;

  if (reduced()) {
    fill.style.width = '100%';
    leave();
    return;
  }

  let t0 = 0;
  const sweep = (now: number) => {
    if (!t0) t0 = now;
    const p = Math.min((now - t0) / COMPLETE_MS, 1);
    const eased = 1 - Math.pow(1 - p, 3); // ease-out cubic, no overshoot
    fill.style.width = `${from + (100 - from) * eased}%`;
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
  if (raf) cancelAnimationFrame(raf);
  raf = 0;
  bootAt = 0;
  finishing = false;
}
