// Self-update wrapper — HOW-THE-APP-WORKS §12 (consult the release feed, download
// over an encrypted connection from trusted hosts only) + §9 (trusted-host gate).
//
// A thin shell around electron-updater's autoUpdater: it never auto-downloads, it
// forwards the updater's lifecycle events to the renderer, and — before fetching a
// single byte — it refuses any installer whose URL is not https on a trusted host.
// electron-updater is CommonJS — under the ESM main process it must be
// default-imported, then destructured (named ESM imports fail at runtime).
import electronUpdater from 'electron-updater';
const { autoUpdater } = electronUpdater;
import type { UpdateInfo, ProgressInfo, UpdateDownloadedEvent } from 'electron-updater';
import type { BrowserWindow } from 'electron';

interface UpdaterDeps {
  getWindow: () => BrowserWindow | null;
}

// §9/§12: installers + the yt-dlp engine (#26) ship as GitHub release assets. GitHub
// serves the release page/API from github.com / api.github.com and redirects the asset
// bytes to objects.githubusercontent.com; anything else is refused outright. Exported so
// the engine updater reuses this one allowlist instead of forking its own.
export const TRUSTED_HOSTS = new Set(['github.com', 'objects.githubusercontent.com', 'api.github.com']);
// ponytail: the release feed lists asset urls relative to the repo; resolve them
// against github.com so a plain filename stays on a trusted host while a
// protocol-relative ("//evil/…") or absolute off-host url is still caught.
const GITHUB_BASE = 'https://github.com/';

/** §9/§12: a release URL is trusted only when it's https on an allowlisted host.
 *  Relative asset urls resolve against github.com (fail-closed on parse error). */
export function isTrustedReleaseHost(urlStr: string, base = GITHUB_BASE): boolean {
  try {
    const url = new URL(urlStr, base);
    return url.protocol === 'https:' && TRUSTED_HOSTS.has(url.hostname);
  } catch {
    return false;
  }
}

export class Updater {
  private latestInfo: UpdateInfo | null = null;

  constructor(private readonly deps: UpdaterDeps) {
    autoUpdater.autoDownload = false; // §12: vet the host before any download.
    autoUpdater.autoInstallOnAppQuit = true;

    autoUpdater.on('update-available', (info: UpdateInfo) => {
      this.latestInfo = info;
      this.send('update:available', { version: info.version });
    });
    autoUpdater.on('update-not-available', () => {
      this.send('update:not-available', {});
    });
    autoUpdater.on('download-progress', (progress: ProgressInfo) => {
      this.send('update:progress', { percent: progress.percent });
    });
    autoUpdater.on('update-downloaded', (event: UpdateDownloadedEvent) => {
      this.send('update:downloaded', { version: event.version });
    });
    // Without this, autoUpdater's 'error' event is unhandled and crashes the main
    // process (it throws in dev with no feed). Forward it so the UI can recover.
    autoUpdater.on('error', (err: Error) => {
      this.send('update:error', { message: err?.message ?? String(err) });
    });
  }

  check(): void {
    // The 'error' event above reports failures to the renderer; catch the promise
    // too so a rejected check never becomes an unhandled rejection.
    void autoUpdater.checkForUpdates().catch(() => { /* surfaced via 'error' */ });
  }

  async download(): Promise<void> {
    if (!this.latestInfo) throw new Error('Update refused: no available update to download');
    this.verifyTrustedHost(this.latestInfo);
    await autoUpdater.downloadUpdate();
  }

  install(): void {
    autoUpdater.quitAndInstall();
  }

  /** §9: every installer file must be https on a trusted host; refuse otherwise (fail-closed). */
  private verifyTrustedHost(info: UpdateInfo): void {
    const files = info.files ?? [];
    if (files.length === 0) throw new Error('Update refused: no installer file in update info');
    for (const file of files) {
      if (!isTrustedReleaseHost(file.url)) {
        throw new Error(`Update refused: untrusted installer host "${file.url}"`);
      }
    }
  }

  private send(channel: string, payload: unknown): void {
    this.deps.getWindow()?.webContents.send(channel, payload);
  }
}
