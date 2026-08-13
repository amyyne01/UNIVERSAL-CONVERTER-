// HOW-THE-APP-WORKS §14: a global keyboard shortcut triggers paste-and-download
// from anywhere. Thin wrapper over Electron's globalShortcut — the handler wiring
// (clipboard read, classification, window foreground) lives in the integration layer.
import { globalShortcut } from 'electron';

/** Register a system-wide shortcut. Returns false for an empty accelerator or a
 *  failed registration (e.g. the combo is already taken by another app). */
export function registerGlobalHotkey(accelerator: string, handler: () => void): boolean {
  if (!accelerator) return false;
  return globalShortcut.register(accelerator, handler) ?? false;
}

export function unregisterGlobalHotkey(accelerator: string): void {
  if (!accelerator) return;
  globalShortcut.unregister(accelerator);
}

export function unregisterAllHotkeys(): void {
  globalShortcut.unregisterAll();
}
