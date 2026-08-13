import { app, BrowserWindow } from 'electron';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const dir = path.dirname(fileURLToPath(import.meta.url));

let mainWindow: BrowserWindow | null = null;

export function createWindow(): BrowserWindow {
  mainWindow = new BrowserWindow({
    width: 1240,
    height: 820,
    minWidth: 960,
    minHeight: 640,
    frame: false,
    titleBarStyle: 'hidden',
    backgroundColor: '#15131c',
    show: false,
    webPreferences: {
      preload: path.join(dir, 'preload.cjs'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  // Harden navigation: the renderer is a fixed SPA that never opens windows or
  // navigates off its own origin. Deny both so an injected/compromised page can't
  // load a remote origin that would inherit the preload bridge (window.electronAPI).
  mainWindow.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  // Only the app's own entrypoint may be navigated to: the dev server in dev, the
  // packaged index.html file URL when packaged. Any other file:// (or origin) is denied,
  // so a compromised page can't load a URL that inherits the preload bridge.
  const indexUrl = pathToFileURL(path.join(app.getAppPath(), 'dist', 'index.html')).href;
  const allowed = (url: string) =>
    app.isPackaged
      ? url === indexUrl || url.startsWith(indexUrl + '#') || url.startsWith(indexUrl + '?')
      : (() => { try { return new URL(url).origin === 'http://localhost:5173'; } catch { return false; } })();
  mainWindow.webContents.on('will-navigate', (e, url) => {
    if (!allowed(url)) e.preventDefault();
  });

  // Keep the renderer's maximize/restore icon in sync with native maximize/snap.
  const sendMaxState = (maximized: boolean) =>
    mainWindow?.webContents.send('window:maximized', maximized);
  mainWindow.on('maximize', () => sendMaxState(true));
  mainWindow.on('unmaximize', () => sendMaxState(false));

  mainWindow.once('ready-to-show', () => mainWindow?.show());
  mainWindow.on('closed', () => {
    mainWindow = null;
  });
  return mainWindow;
}

export function getMainWindow(): BrowserWindow | null {
  return mainWindow;
}
