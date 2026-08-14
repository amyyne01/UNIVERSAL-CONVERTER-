import { app, BrowserWindow, Menu, clipboard, Notification } from 'electron';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { createWindow, getMainWindow } from './window.js';
import { registerIpcHandlers } from './ipc.js';
import { ConfigManager } from './config.js';
import { Downloader } from './downloader.js';
import { SpotifyHandler } from './spotify.js';
import { LicenseManager } from './license.js';
import { StatsStore, statsFilePath } from './stats.js';
import { loadSecrets, type Secrets } from './secrets.js';
import { detectUrl } from './url-detector.js';
import {
  setDownloader, createTask, enqueueTask,
  getRecentDownloads, onQueueChange, pauseAllDownloads, resumeAllDownloads,
  initHistory, flushHistory, hasUnfinishedWork, onDownloadFailure, onDownloadCompleted,
} from './queue.js';
import { initTray } from './tray.js';
import { registerGlobalHotkey, unregisterGlobalHotkey, unregisterAllHotkeys } from './hotkey.js';
import { DiscordPresence } from './presence.js';
import { Scheduler } from './scheduler.js';
import { Updater } from './updater.js';
import { YtdlpUpdater } from './ytdlp-updater.js';
import { installContentSecurityPolicy } from './security.js';
import type { DownloadStatus } from '../shared/types.js';

let config: ConfigManager | null = null;
let tray: { update(): void; destroy(): void } | null = null;
let presence: DiscordPresence | null = null;
let scheduler: Scheduler | null = null;
let lastHotkey = '';
// Set when a scheduled run with "shutdown after" fires; the OS shutdown is armed
// and only executed once the queue actually drains (not 60s after it starts).
let shutdownArmed = false;
/** Ceiling on how long the ambient layer waits for the renderer to finish loading. */
const SERVICES_WATCHDOG_MS = 4000;

function runScheduledShutdown(): void {
  try { spawn('shutdown', ['/s', '/t', '60'], { detached: true }).unref(); } catch { /* best-effort */ }
}

const ACTIVE_STATES = new Set<DownloadStatus>(['fetching_info', 'downloading', 'converting', 'embedding']);

function loadApp(win: BrowserWindow): void {
  // loadFile/loadURL REJECT on a failed navigation (missing/corrupt dist, an AV
  // handle, the dev server not up yet). Unhandled, that rejection takes down the
  // main process under Node's default — same class of bug updater.ts guards.
  const load = app.isPackaged
    ? win.loadFile(path.join(app.getAppPath(), 'dist', 'index.html'))
    : win.loadURL('http://localhost:5173');
  void load.catch((err: unknown) => console.error('[main] failed to load the app window', err));
}

function showWindow(): void {
  const w = getMainWindow();
  if (!w) return;
  if (w.isMinimized()) w.restore();
  w.show();
  w.focus();
}

function toggleWindow(): void {
  const w = getMainWindow();
  if (!w) return;
  if (w.isVisible() && !w.isMinimized()) w.hide();
  else showWindow();
}

// §14: read the clipboard, classify with the same cascade as the UI, and if it's a
// known link create+enqueue a task with config defaults and bring the window forward.
function pasteAndDownload(): void {
  if (!config) return;
  const text = clipboard.readText().trim();
  if (!text) return;
  const det = detectUrl(text);
  if (det.platform === 'unknown') return;
  enqueueTask(createTask({
    url: det.url,
    source: det.platform,
    isPlaylist: det.isCollection,
    outputDir: config.get('outputDir'),
    format: config.get('defaultFormat'),
    quality: config.get('defaultQuality'),
    videoQuality: config.get('defaultVideoQuality'),
    embedThumbnail: config.get('embedThumbnail'),
    embedMetadata: config.get('embedMetadata'),
    skipExisting: config.get('skipExisting'),
  }));
  showWindow();
}

// ponytail: presence counts are derived from the recent window (cap ~8); exact
// queue totals aren't exported. Fine for an ambient "Downloading N / M queued".
function presenceActivity(): { downloading: number; queued: number } {
  const recent = getRecentDownloads();
  return {
    downloading: recent.filter((r) => ACTIVE_STATES.has(r.status)).length,
    queued: recent.filter((r) => r.status === 'queued').length,
  };
}

// ponytail: minimize-to-tray and the on-blur mini-player are both removed.
// Close quits normally; no window hides on blur. To restore tray minimize,
// re-add a `win.on('close')` handler gated on config.minimizeToTray.
function initServices(secrets: Secrets, license: LicenseManager): void {
  // The 4000×4000/2.5MB icon.png is for the title bar's largest use; the tray
  // renders at 16-32px, so use the pre-shrunk 64px asset (see tray.ts).
  const iconPath = app.isPackaged
    ? path.join(process.resourcesPath, 'assets', 'icon-64.png')
    : path.join(app.getAppPath(), 'assets', 'icon-64.png');

  tray = initTray({
    iconPath,
    getRecent: getRecentDownloads,
    actions: {
      pauseAll: pauseAllDownloads,
      resumeAll: resumeAllDownloads,
      toggleWindow,
      pasteAndDownload,
      quit: () => app.quit(),
    },
  });
  onQueueChange(() => tray?.update());
  // Scheduled-shutdown drain watcher: fire the OS shutdown only once the queue
  // has actually emptied, so a run longer than 60s isn't cut off mid-download.
  onQueueChange(() => {
    if (shutdownArmed && !hasUnfinishedWork()) { shutdownArmed = false; runScheduledShutdown(); }
  });

  lastHotkey = config!.get('globalHotkey');
  registerGlobalHotkey(lastHotkey, pasteAndDownload);

  // Registered unconditionally (no-op via `presence?.` while off) so a later
  // Settings toggle-on picked up by syncServices() still gets live activity updates.
  onQueueChange(() => presence?.setActivity(presenceActivity()));

  // §14: presence only when enabled AND a client id is configured — otherwise silent.
  if (config!.get('discordRichPresence') && secrets.discordClientId) {
    presence = new DiscordPresence();
    presence.start(secrets.discordClientId);
    presence.setActivity(presenceActivity());
  }

  // §13: poll-based scheduler + startup missed-run catch-up. Shutdown is OS-level
  // and only reachable past the scheduler's own once-per-day trigger gate.
  scheduler = new Scheduler({
    getConfig: () => config!.getAll(),
    isPremium: () => license.getPlan() === 'premium',
    onTrigger: () => {
      resumeAllDownloads();
      if (config!.get('showNotifications') && Notification.isSupported()) {
        new Notification({ title: 'AHG Universal Converter', body: 'Scheduled downloads started.' }).show();
      }
    },
    onShutdown: () => {
      // Arm, don't execute: the drain watcher (onQueueChange) runs the OS shutdown
      // when the queue empties. Edge — nothing was queued to resume, so no change
      // event will come: check shortly, after downloads have had time to spin up.
      shutdownArmed = true;
      setTimeout(() => {
        if (shutdownArmed && !hasUnfinishedWork()) { shutdownArmed = false; runScheduledShutdown(); }
      }, 10_000).unref?.();
    },
  });
  scheduler.start();
  scheduler.checkMissedRun();
}

// B2+B3 root-cause fix: ConfigManager has no other way to tell main-process services
// a Settings change happened, so re-sync the hotkey + presence to current config on
// every config:set/update instead of only registering them once at startup.
function syncServices(secrets: Secrets): void {
  if (!config) return;

  const hotkey = config.get('globalHotkey');
  if (hotkey !== lastHotkey) {
    if (lastHotkey) unregisterGlobalHotkey(lastHotkey);
    if (hotkey) registerGlobalHotkey(hotkey, pasteAndDownload);
    lastHotkey = hotkey;
  }

  const wantPresence = config.get('discordRichPresence') && !!secrets.discordClientId;
  if (wantPresence && !presence) {
    presence = new DiscordPresence();
    presence.start(secrets.discordClientId);
    presence.setActivity(presenceActivity());
  } else if (!wantPresence && presence) {
    presence.stop();
    presence = null;
  }
}

app.whenReady().then(() => {
  // The window is frameless with its own title bar, so Electron's default menu is
  // invisible — but its accelerators (inspector, reload, force-reload, zoom) are
  // still live. Removing it entirely is what makes this behave like an app rather
  // than a browser wearing one.
  Menu.setApplicationMenu(null);
  if (app.isPackaged) installContentSecurityPolicy();
  config = new ConfigManager();
  // Restore download history; interrupted tasks come back paused + resumable.
  initHistory(path.join(app.getPath('userData'), 'downloads-history.json'));
  // Monthly counters, kept outside the (capped, evicting) history so the totals
  // can never shrink while the user keeps downloading.
  const stats = new StatsStore();
  stats.init(statsFilePath(app.getPath('userData')));
  onDownloadCompleted((task, filepaths) => stats.record(task.source, filepaths, task.progress.total));
  const downloader = new Downloader();
  setDownloader(downloader);
  const spotify = new SpotifyHandler();
  const secrets = loadSecrets();
  const license = new LicenseManager(secrets);
  // ponytail: don't auto-check on startup — autoUpdater throws in dev (no feed) and
  // emits an unhandled 'error'. The renderer drives update:check when appropriate.
  const updater = new Updater({ getWindow: () => getMainWindow() });
  // #26: the yt-dlp engine is the thing that actually goes stale — let it self-update
  // into userData/bin (checksum-verified) and react to extractor failures.
  const ytdlp = new YtdlpUpdater({
    downloader,
    getWindow: () => getMainWindow(),
    isAutoUpdateEnabled: () => config?.get('autoUpdateEngine') ?? true,
  });
  onDownloadFailure((error) => ytdlp.handleDownloadFailure(error));
  registerIpcHandlers(config, downloader, spotify, license, updater, ytdlp, stats);

  const win = createWindow();
  loadApp(win);
  // Nothing below is needed to paint the first frame, and some of it is slow
  // (tray icon I/O, a global shortcut, a Discord socket, a scheduler catch-up
  // that can start downloads). Deferring it to after the renderer has loaded
  // keeps the window's first paint on the critical path by itself.
  // …but the ambient layer owns the tray, the global hotkey and the scheduler's
  // missed-run catch-up, so it must come up even when that load never finishes.
  // The watchdog covers every failure shape at once — failed navigation, a window
  // destroyed mid-load, a dev server that never answers — and the flag makes the
  // two paths idempotent, since double-init would mean a second tray and a second
  // set of scheduler timers.
  let servicesStarted = false;
  const startServices = () => {
    if (servicesStarted) return;
    servicesStarted = true;
    initServices(secrets, license);
    config?.onChange(() => syncServices(secrets));
  };
  win.webContents.once('did-finish-load', startServices);
  setTimeout(startServices, SERVICES_WATCHDOG_MS).unref?.();
  // Resolve/validate the yt-dlp + ffmpeg binaries shortly after launch so the
  // user's first search doesn't eat the one-time cold-start cost.
  setTimeout(() => downloader.warmUp(), 1500);
  // #26 ambient channel: one deferred, non-blocking engine check per launch (the
  // updater's own 24h TTL caps the real network traffic); only auto-installs when
  // the user left auto-update on. Failures are surfaced as status, never thrown.
  setTimeout(() => {
    if (!config?.get('autoUpdateEngine')) return;
    void ytdlp.check().then((s) => (s.state === 'available' ? ytdlp.update() : s));
  }, 8000);

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      loadApp(createWindow());
    }
  });
});

// §10: one final, immediate, non-debounced save; then tear down the ambient layer.
app.on('before-quit', () => {
  config?.saveNow();
  flushHistory();
  unregisterAllHotkeys();
  presence?.stop();
  scheduler?.stop();
  tray?.destroy();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});
