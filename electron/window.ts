import { app, BrowserWindow } from 'electron';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const dir = path.dirname(fileURLToPath(import.meta.url));

let mainWindow: BrowserWindow | null = null;

/**
 * The taskbar and the Alt-Tab switcher take their icon from the window, not from
 * the exe — without this they fall back to Electron's own default. A multi-size
 * .ico is used rather than the 4000x4000 source png: Windows picks the right size
 * per surface, and nothing has to decode a 2.5 MB bitmap on the first-paint path.
 */
function windowIcon(): string {
  const base = app.isPackaged ? process.resourcesPath : app.getAppPath();
  return path.join(base, 'assets', 'icon.ico');
}

export function createWindow(theme: 'light' | 'dark' = 'dark'): BrowserWindow {
  mainWindow = new BrowserWindow({
    icon: windowIcon(),
    width: 1240,
    height: 820,
    minWidth: 960,
    minHeight: 640,
    frame: false,
    titleBarStyle: 'hidden',
    // The native fill shown before the first frame. Same resolved theme the splash
    // paints with, so a light setup never flashes dark (or the reverse).
    backgroundColor: theme === 'dark' ? '#0d1117' : '#f7f9fb',
    show: false,
    webPreferences: {
      preload: path.join(dir, 'preload.cjs'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      // A shipped build is an application, not a web page: no inspector, no
      // console, nothing to poke the renderer with. Development keeps them.
      devTools: !app.isPackaged,
    },
  });

  // devTools:false stops the API, but the accelerators come from Electron's
  // default menu and fire before it — so swallow them at the input layer too.
  // Without this, F12 in a packaged build still opens nothing but is handled,
  // which is the kind of half-state that invites someone to keep trying.
  if (app.isPackaged) {
    mainWindow.webContents.on('before-input-event', (event, input) => {
      const key = input.key.toLowerCase();
      const inspector =
        key === 'f12' ||
        (input.control && input.shift && (key === 'i' || key === 'j' || key === 'c'));
      if (inspector) event.preventDefault();
    });
  }

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
