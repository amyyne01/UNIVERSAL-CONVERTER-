import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('electron', () => ({
  globalShortcut: {
    register: vi.fn(() => true),
    unregister: vi.fn(),
    unregisterAll: vi.fn(),
  },
}));

import { registerGlobalHotkey, unregisterGlobalHotkey, unregisterAllHotkeys } from './hotkey';
import { globalShortcut } from 'electron';

const mockRegister = vi.mocked(globalShortcut.register);
const mockUnregister = vi.mocked(globalShortcut.unregister);
const mockUnregisterAll = vi.mocked(globalShortcut.unregisterAll);

beforeEach(() => vi.clearAllMocks());

describe('hotkey', () => {
  it('registers the accelerator with its handler and returns the success boolean', () => {
    const handler = vi.fn();
    expect(registerGlobalHotkey('CommandOrControl+Shift+D', handler)).toBe(true);
    expect(mockRegister).toHaveBeenCalledWith('CommandOrControl+Shift+D', handler);
  });

  it('propagates a failed registration', () => {
    mockRegister.mockReturnValueOnce(false);
    expect(registerGlobalHotkey('CommandOrControl+Shift+D', vi.fn())).toBe(false);
  });

  it('refuses an empty accelerator without touching globalShortcut', () => {
    expect(registerGlobalHotkey('', vi.fn())).toBe(false);
    expect(mockRegister).not.toHaveBeenCalled();
  });

  it('unregisters a single accelerator (and skips when empty)', () => {
    unregisterGlobalHotkey('CommandOrControl+Shift+D');
    expect(mockUnregister).toHaveBeenCalledWith('CommandOrControl+Shift+D');
    unregisterGlobalHotkey('');
    expect(mockUnregister).toHaveBeenCalledTimes(1);
  });

  it('unregisters everything', () => {
    unregisterAllHotkeys();
    expect(mockUnregisterAll).toHaveBeenCalledTimes(1);
  });
});
