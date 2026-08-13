import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { MenuItemConstructorOptions } from 'electron';
import type { DownloadStatus } from '../shared/types.js';

const { trayInstance } = vi.hoisted(() => ({
  trayInstance: { setToolTip: vi.fn(), setContextMenu: vi.fn(), destroy: vi.fn() },
}));

vi.mock('electron', () => ({
  Tray: vi.fn(() => trayInstance),
  Menu: { buildFromTemplate: vi.fn((tpl) => tpl) }, // identity → template is inspectable
  nativeImage: { createFromPath: vi.fn((p: string) => ({ path: p })) },
}));

import { initTray } from './tray';
import { Tray, nativeImage } from 'electron';

const lastTemplate = (): MenuItemConstructorOptions[] =>
  vi.mocked(trayInstance.setContextMenu).mock.lastCall![0] as MenuItemConstructorOptions[];
const item = (label: string) => lastTemplate().find((i) => i.label === label);

describe('initTray', () => {
  const actions = {
    pauseAll: vi.fn(), resumeAll: vi.fn(), toggleWindow: vi.fn(),
    pasteAndDownload: vi.fn(), quit: vi.fn(),
  };
  let recent: { title: string; status: DownloadStatus }[];
  const getRecent = vi.fn(() => recent);

  beforeEach(() => {
    vi.clearAllMocks();
    recent = [{ title: 'Song A', status: 'downloading' }];
  });

  it('builds a tray from the icon path and installs a menu on init', () => {
    initTray({ iconPath: 'C:/icon.png', getRecent, actions });
    expect(nativeImage.createFromPath).toHaveBeenCalledWith('C:/icon.png');
    expect(Tray).toHaveBeenCalledWith({ path: 'C:/icon.png' });
    expect(trayInstance.setContextMenu).toHaveBeenCalledTimes(1);
  });

  it('lists recent downloads with a status glyph, then the controls', () => {
    initTray({ iconPath: 'i', getRecent, actions });
    const tpl = lastTemplate();
    expect(tpl[0].label).toBe('↓  Song A'); // downloading glyph + title
    for (const label of ['Pause all', 'Resume all', 'Show / Hide', 'Paste & Download', 'Quit']) {
      expect(item(label), `missing control: ${label}`).toBeTruthy();
    }
  });

  it('update() rebuilds the menu from the latest recent state', () => {
    const tray = initTray({ iconPath: 'i', getRecent, actions });
    recent = [{ title: 'Song B', status: 'done' }];
    tray.update();
    expect(trayInstance.setContextMenu).toHaveBeenCalledTimes(2);
    expect(lastTemplate()[0].label).toBe('✓  Song B');
  });

  it('menu controls fire the injected actions', () => {
    initTray({ iconPath: 'i', getRecent, actions });
    item('Pause all')!.click!(undefined as never, undefined, undefined as never);
    item('Resume all')!.click!(undefined as never, undefined, undefined as never);
    item('Quit')!.click!(undefined as never, undefined, undefined as never);
    expect(actions.pauseAll).toHaveBeenCalledOnce();
    expect(actions.resumeAll).toHaveBeenCalledOnce();
    expect(actions.quit).toHaveBeenCalledOnce();
  });

  it('destroy() tears down the tray', () => {
    initTray({ iconPath: 'i', getRecent, actions }).destroy();
    expect(trayInstance.destroy).toHaveBeenCalledOnce();
  });
});
