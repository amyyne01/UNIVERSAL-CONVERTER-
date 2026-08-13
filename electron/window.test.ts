import { describe, it, expect, vi, beforeEach } from 'vitest';

// Minimal electron mock: capture the 'will-navigate' listener registered by createWindow().
const webContents = {
  setWindowOpenHandler: vi.fn(),
  on: vi.fn(),
  send: vi.fn(),
};

vi.mock('electron', () => ({
  app: {
    isPackaged: false, // dev branch — the one B16 fixes
    getAppPath: vi.fn(() => 'C:\\app'),
  },
  BrowserWindow: vi.fn().mockImplementation(() => ({
    webContents,
    on: vi.fn(),
    once: vi.fn(),
  })),
}));

import { createWindow } from './window';

function getWillNavigateHandler(): (e: { preventDefault: () => void }, url: string) => void {
  const call = webContents.on.mock.calls.find(([event]) => event === 'will-navigate');
  return call![1];
}

describe('createWindow will-navigate guard (dev branch)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('B16: rejects a URL whose prefix matches the dev origin string but whose real origin does not', () => {
    createWindow();
    const handler = getWillNavigateHandler();
    const preventDefault = vi.fn();
    // Pre-fix, `startsWith('http://localhost:5173')` treats this as allowed because the
    // string literally begins with that prefix; its real origin is http://evil.com.
    handler({ preventDefault }, 'http://localhost:5173@evil.com/');
    expect(preventDefault).toHaveBeenCalled();
  });

  it('still allows the real dev server origin', () => {
    createWindow();
    const handler = getWillNavigateHandler();
    const preventDefault = vi.fn();
    handler({ preventDefault }, 'http://localhost:5173/some/path');
    expect(preventDefault).not.toHaveBeenCalled();
  });
});
