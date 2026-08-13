// Drives the boot splash that index.html paints before this bundle exists.
// The markup lives outside #root, so React never owns it — these three calls are
// the whole contract, and every one of them is a no-op once the splash is gone.
const STEPS = ['Reading settings', 'Restoring downloads', 'Ready'] as const;

let done = 0;

/** Advance one boot step. Progress is real: 1/3 per completed step. */
export function splashStep(): void {
  const fill = document.getElementById('splash-fill');
  const status = document.getElementById('splash-status');
  if (!fill) return;
  done = Math.min(done + 1, STEPS.length);
  fill.style.width = `${Math.round((done / STEPS.length) * 100)}%`;
  if (status) status.textContent = STEPS[Math.min(done, STEPS.length - 1)];
}

/** Fade out and remove the splash. Safe to call twice (the node is gone after). */
export function dismissSplash(): void {
  const splash = document.getElementById('splash');
  if (!splash) return;
  const fill = document.getElementById('splash-fill');
  if (fill) fill.style.width = '100%';
  splash.dataset.leaving = 'true';
  // Remove on the fade's own event so a reduced-motion user (transition: none)
  // isn't left with a dead overlay swallowing clicks; the timeout is the floor
  // for browsers that skip transitionend when the property never animates.
  // Target-filtered: the fill's width sweep bubbles its own transitionend first
  // and must not cut the fade short.
  const remove = () => splash.remove();
  splash.addEventListener('transitionend', (e) => {
    if (e.target === splash) remove();
  });
  setTimeout(remove, 400);
}
