// Theme resolution. The effective theme is also set pre-paint in index.html;
// this keeps it in sync at runtime and tracks the OS when set to 'system'.
import type { ThemeName } from '@shared/types';

const KEY = 'ahg-theme';
// One cached MediaQueryList: add/removeEventListener must target the SAME object,
// or the old listener is never removed (a fresh matchMedia() each call wouldn't match).
let mqCache: MediaQueryList | null = null;
const mq = () => (mqCache ??= window.matchMedia('(prefers-color-scheme: dark)'));

export function resolveTheme(theme: ThemeName): 'light' | 'dark' {
  return theme === 'system' ? (mq().matches ? 'dark' : 'light') : theme;
}

export function applyTheme(theme: ThemeName): void {
  document.documentElement.dataset.theme = resolveTheme(theme);
  localStorage.setItem(KEY, theme);
}

let listener: (() => void) | null = null;

export function initTheme(): void {
  // Deliberately does NOT apply a theme here. public/theme-init.js already set
  // one pre-paint from the value MAIN resolved out of config.theme — the real
  // setting. localStorage is only a mirror this module writes afterwards, so
  // re-applying it on mount would repaint the app from a stale copy and flash
  // the wrong theme over a correct splash. The authoritative apply happens when
  // config arrives (store.setConfig -> applyTheme(cfg.theme)).
  if (listener) mq().removeEventListener('change', listener);
  listener = () => {
    const current = (localStorage.getItem(KEY) as ThemeName | null) ?? 'system';
    if (current === 'system') applyTheme('system');
  };
  mq().addEventListener('change', listener);
}
