// Pre-paint theme bootstrap — runs before the bundle so there's no flash.
// External (not inline) so the production CSP can keep script-src 'self'.
(function () {
  try {
    var t = localStorage.getItem('ahg-theme') || 'system';
    var dark = t === 'dark' || (t === 'system' && matchMedia('(prefers-color-scheme: dark)').matches);
    document.documentElement.dataset.theme = dark ? 'dark' : 'light';
  } catch (e) {
    // localStorage unavailable — still follow the OS (system is the default everywhere).
    var osDark = true;
    try { osDark = matchMedia('(prefers-color-scheme: dark)').matches; } catch (e2) { /* no matchMedia */ }
    document.documentElement.dataset.theme = osDark ? 'dark' : 'light';
  }
})();
