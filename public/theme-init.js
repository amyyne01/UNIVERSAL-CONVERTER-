// Pre-paint theme bootstrap — runs before the bundle so there's no flash.
// External (not inline) so the production CSP can keep script-src 'self'.
//
// Order matters. The authority is ?theme=, which the main process appends when it
// loads this page: main owns config.theme (the real setting) and resolves 'system'
// against the OS there. localStorage is only a MIRROR the renderer writes after
// config arrives — trusting it first meant the splash guessed, and guessed wrong
// whenever the mirror was missing or stale (a fresh profile, a different origin
// between dev and the packaged app), painting a white splash in front of a dark app.
(function () {
  function fromQuery() {
    try {
      var t = new URLSearchParams(location.search).get('theme');
      return t === 'dark' || t === 'light' ? t : null;
    } catch (e) {
      return null;
    }
  }
  function osDark() {
    try {
      return matchMedia('(prefers-color-scheme: dark)').matches;
    } catch (e) {
      return true;
    }
  }
  var resolved = fromQuery();
  if (!resolved) {
    var stored = null;
    try {
      stored = localStorage.getItem('ahg-theme');
    } catch (e) {
      /* unavailable — fall through to the OS */
    }
    var dark = stored === 'dark' || ((!stored || stored === 'system') && osDark());
    resolved = dark ? 'dark' : 'light';
  }
  document.documentElement.dataset.theme = resolved;
})();
