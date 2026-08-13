// System-tray presence (HOW-THE-APP-WORKS §14): a background icon whose menu is
// rebuilt on download-state changes, showing recent items with a status glyph
// plus quick controls. Self-contained, dependency-injected — no queue/window imports.
import { Tray, Menu, nativeImage, type MenuItemConstructorOptions } from 'electron';
import type { DownloadStatus } from '../shared/types.js';

export interface TrayDeps {
  iconPath: string;
  getRecent: () => { title: string; status: DownloadStatus }[];
  actions: {
    pauseAll(): void;
    resumeAll(): void;
    toggleWindow(): void;
    pasteAndDownload(): void;
    quit(): void;
  };
}

// Small glyph per state so the queue is readable at a glance (§14).
const STATUS_GLYPH: Record<DownloadStatus, string> = {
  queued: '◷',
  fetching_info: '◌',
  downloading: '↓',
  converting: '⚙',
  embedding: '♪',
  done: '✓',
  paused: '⏸',
  retrying: '↻',
  failed: '✗',
  cancelled: '⊘',
};

function buildMenu(deps: TrayDeps): MenuItemConstructorOptions[] {
  const recent = deps.getRecent();
  const recentItems: MenuItemConstructorOptions[] = recent.length
    ? recent.map((r) => ({ label: `${STATUS_GLYPH[r.status]}  ${r.title}`, enabled: false }))
    : [{ label: 'No recent downloads', enabled: false }];
  return [
    ...recentItems,
    { type: 'separator' },
    { label: 'Pause all', click: deps.actions.pauseAll },
    { label: 'Resume all', click: deps.actions.resumeAll },
    { label: 'Show / Hide', click: deps.actions.toggleWindow },
    { label: 'Paste & Download', click: deps.actions.pasteAndDownload },
    { type: 'separator' },
    { label: 'Quit', click: deps.actions.quit },
  ];
}

export function initTray(deps: TrayDeps): { update(): void; destroy(): void } {
  const tray = new Tray(nativeImage.createFromPath(deps.iconPath));
  tray.setToolTip('AHG Universal Converter');
  const update = (): void => tray.setContextMenu(Menu.buildFromTemplate(buildMenu(deps)));
  update();
  return { update, destroy: () => tray.destroy() };
}
