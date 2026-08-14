import { ipcMain, dialog, shell } from 'electron';
import type { IpcMainInvokeEvent } from 'electron';
import { existsSync } from 'node:fs';
import { getMainWindow } from './window.js';
import { detectUrl } from './url-detector.js';
import { confineToRoot, secure } from './security.js';
import {
  createTask, enqueueTask, cancelTask, cancelAllTasks,
  pauseTask, resumeTask, getAllTasks, removeTasks,
} from './queue.js';
import { ConfigManager, defaultConfig } from './config.js';
import type { Downloader } from './downloader.js';
import type { StatsStore } from './stats.js';
import type { SpotifyHandler } from './spotify.js';
import type { LicenseManager } from './license.js';
import type { Updater } from './updater.js';
import type { YtdlpUpdater } from './ytdlp-updater.js';
import type { AppConfig, DownloadTask, RevealResult, Track, VideoQuality } from '../shared/types.js';
import {
  SOURCE_PLATFORMS, VIDEO_QUALITIES, BASIC_LIMITS,
  clampAudioQuality, clampFormat, clampVideoQuality,
} from '../shared/types.js';

// All ipcMain.handle() registrations live here, grouped by namespace.
// Every value crossing the bridge is untrusted (HOW-THE-APP-WORKS §15): string
// args are validated at the boundary before the engine acts on them.
const MAX_URL = 2048; // classic URL length ceiling — guards against oversized payloads.
const MAX_PATH = 4096;
const MAX_ARTISTS = 64; // cap the scraped-artist list so a hostile track can't balloon it.

/** Reject non-strings and over-long payloads at the trust boundary. */
function str(value: unknown, max = MAX_URL): string {
  if (typeof value !== 'string' || value.length > max) {
    throw new Error('Invalid input: expected a string within length limits');
  }
  return value;
}

/** A URL/positional arg for yt-dlp: a string that must not begin with "-", so it
 *  can never be parsed as an option (defense-in-depth with the downloader's `--`). */
function urlArg(value: unknown): string {
  const s = str(value);
  if (s.startsWith('-')) throw new Error('Invalid input: URL must not start with "-"');
  return s;
}

const MAX_QUERY = 256; // a search box realistically never exceeds this.

/** A read-only search query: non-strings become "" and over-long input is CLAMPED,
 *  not rejected. A giant paste (routed to search as plain text) degrades to a
 *  sensible search instead of throwing a boundary error that surfaces as a generic
 *  failure and logs in the main process. Search has no side effects, so this is safe. */
function queryStr(value: unknown, max = MAX_QUERY): string {
  return typeof value === 'string' ? value.slice(0, max) : '';
}

// Per-channel per-minute budgets (Seam #45). download:start spawns a real job so
// it's the tightest; url detection is bursty-but-cheap; everything else is generous.
const RATE_DEFAULT = 120;
const RATE_URL = 60;
const RATE_DOWNLOAD = 10;

export function registerIpcHandlers(config: ConfigManager, downloader: Downloader, spotify: SpotifyHandler, license: LicenseManager, updater: Updater, ytdlp: YtdlpUpdater, stats: StatsStore): void {
  // Route EVERY channel through secure(): sender attestation + a per-channel rate
  // budget run before the handler. Additive — no channel's args/returns change.
  const handle = (
    channel: string,
    limit: number,
    fn: (event: IpcMainInvokeEvent, ...args: any[]) => unknown,
  ) => ipcMain.handle(channel, secure(channel, limit, fn));

  // ── window:* ──────────────────────────────────────────────────────────────
  handle('window:minimize', RATE_DEFAULT, () => getMainWindow()?.minimize());
  handle('window:maximize', RATE_DEFAULT, () => {
    const w = getMainWindow();
    if (!w) return;
    if (w.isMaximized()) w.unmaximize();
    else w.maximize();
  });
  handle('window:close', RATE_DEFAULT, () => getMainWindow()?.close());
  handle('window:isMaximized', RATE_DEFAULT, () => getMainWindow()?.isMaximized() ?? false);

  // ── config:* ──────────────────────────────────────────────────────────────
  // Keys are whitelisted/prototype-guarded inside ConfigManager (§10). config:get
  // reads through, so gate the key here too — a renderer asking for '__proto__' /
  // 'toString' must get undefined, never a prototype member.
  handle('config:get', RATE_DEFAULT, (_e, key: unknown) => {
    if (typeof key !== 'string' || !Object.prototype.hasOwnProperty.call(defaultConfig, key)) return undefined;
    return config.get(key as keyof AppConfig);
  });
  handle('config:set', RATE_DEFAULT, (_e, key: keyof AppConfig, value: AppConfig[keyof AppConfig]) => {
    config.set(key, value);
    return true;
  });
  handle('config:getAll', RATE_DEFAULT, () => config.getAll());
  handle('config:update', RATE_DEFAULT, (_e, data: unknown) => {
    if (typeof data !== 'object' || data === null) throw new Error('Invalid input: expected a config object');
    config.update(data as Partial<AppConfig>);
    return true;
  });

  // ── url:* ─────────────────────────────────────────────────────────────────
  handle('url:detect', RATE_URL, (_e, url: unknown) => detectUrl(urlArg(url)));
  handle('url:fetchMetadata', RATE_URL, (_e, url: unknown) => downloader.fetchMetadata(urlArg(url)));

  // ── youtube:* / soundcloud:* (search via the engine, §3.1) ──────────────────
  handle('youtube:search', RATE_DEFAULT, (_e, query: unknown) => downloader.searchYouTube(queryStr(query)));
  handle('soundcloud:search', RATE_DEFAULT, (_e, query: unknown) => downloader.searchSoundCloud(queryStr(query)));

  // ── spotify:* (credential-free metadata pipeline, §3.2) ─────────────────────
  handle('spotify:parseUrl', RATE_DEFAULT, (_e, url: unknown) => spotify.parseUrl(str(url)));
  handle('spotify:fetchTrack', RATE_DEFAULT, (_e, idOrUrl: unknown) => spotify.fetchTrack(str(idOrUrl)));
  handle('spotify:fetchPlaylist', RATE_DEFAULT, (_e, idOrUrl: unknown) => spotify.fetchPlaylist(str(idOrUrl)));
  handle('spotify:fetchAlbum', RATE_DEFAULT, (_e, idOrUrl: unknown) => spotify.fetchAlbum(str(idOrUrl)));
  handle('spotify:search', RATE_DEFAULT, (_e, query: unknown) => spotify.search(queryStr(query)));

  // ── download:* ──────────────────────────────────────────────────────────────
  handle('download:start', RATE_DOWNLOAD, (_e, taskData: Partial<DownloadTask>): DownloadTask => {
    if (typeof taskData !== 'object' || taskData === null) throw new Error('Invalid input: expected a task object');
    // Validate every renderer-supplied field that reaches the engine (not just url).
    // Strings are length-bounded; enums are whitelisted; booleans coerced. Anything
    // malformed is rejected at the boundary instead of throwing deep in spawn().
    const optStr = (v: unknown, max = 512): string | undefined =>
      v === undefined ? undefined : str(v, max);
    const optBool = (v: unknown): boolean | undefined =>
      v === undefined ? undefined : Boolean(v);
    const enumOf = <T extends string>(v: unknown, allowed: readonly T[], field: string): T | undefined => {
      if (v === undefined) return undefined;
      if (typeof v !== 'string' || !allowed.includes(v as T)) throw new Error(`Invalid input: ${field}`);
      return v as T;
    };
    // The scraped Spotify track drives the ytsearch query + file tagging, so it
    // crosses into spawn/tagging too — rebuild it from bounded fields (drops any
    // extraneous/prototype-polluting keys) rather than trusting it wholesale.
    const validateTrack = (v: unknown): Track => {
      if (typeof v !== 'object' || v === null) throw new Error('Invalid input: track');
      const t = v as Record<string, unknown>;
      return {
        id: optStr(t.id, 256) ?? '',
        name: str(t.name, 512),
        artist: str(t.artist, 512),
        artists: Array.isArray(t.artists) ? t.artists.slice(0, MAX_ARTISTS).map((a) => str(a, 512)) : [],
        album: optStr(t.album, 512) ?? '',
        albumYear: optStr(t.albumYear, 16),
        trackNumber: typeof t.trackNumber === 'number' ? t.trackNumber : 0,
        durationMs: typeof t.durationMs === 'number' ? t.durationMs : 0,
        thumbnailUrl: optStr(t.thumbnailUrl, MAX_URL) ?? '',
        spotifyUrl: optStr(t.spotifyUrl, MAX_URL),
      };
    };
    // Tier enforcement lives HERE, not in the renderer: the UI locks premium
    // controls, but the boundary is what actually decides what runs. Basic input
    // is clamped down to the free ceiling rather than rejected, so a stale/edited
    // renderer degrades to a working download instead of a failed one.
    const plan = license.getPlan();
    const basic = plan === 'basic';
    const format = optStr(taskData.format, 16) ?? config.get('defaultFormat');
    const quality = optStr(taskData.quality, 16) ?? config.get('defaultQuality');
    const videoQuality: VideoQuality =
      enumOf(taskData.videoQuality, VIDEO_QUALITIES, 'videoQuality') ?? config.get('defaultVideoQuality');
    const isPlaylist = optBool(taskData.isPlaylist);

    // outputDir is NEVER taken from the renderer — always the dialog-chosen config
    // dir — so a compromised renderer can't redirect writes to an arbitrary path.
    const task = createTask({
      url: urlArg(taskData.url ?? ''),
      source: enumOf(taskData.source, SOURCE_PLATFORMS, 'source'),
      outputDir: config.get('outputDir'),
      format: clampFormat(format, plan),
      quality: clampAudioQuality(quality, plan),
      videoQuality: clampVideoQuality(videoQuality, plan),
      ...(basic && isPlaylist ? { playlistLimit: BASIC_LIMITS.maxCollectionTracks } : {}),
      isAudioOnly: optBool(taskData.isAudioOnly),
      isPlaylist,
      playlistName: optStr(taskData.playlistName),
      title: optStr(taskData.title),
      thumbnailUrl: optStr(taskData.thumbnailUrl, MAX_URL),
      duration: typeof taskData.duration === 'number' ? taskData.duration : undefined,
      uploader: optStr(taskData.uploader),
      embedThumbnail: optBool(taskData.embedThumbnail) ?? config.get('embedThumbnail'),
      embedMetadata: optBool(taskData.embedMetadata) ?? config.get('embedMetadata'),
      skipExisting: optBool(taskData.skipExisting) ?? config.get('skipExisting'),
      ...(taskData.track !== undefined ? { track: validateTrack(taskData.track) } : {}),
    });
    enqueueTask(task);
    return task;
  });
  handle('download:cancel', RATE_DEFAULT, (_e, taskId: string) => cancelTask(str(taskId, MAX_PATH)));
  handle('download:cancelAll', RATE_DEFAULT, () => cancelAllTasks());
  handle('download:pause', RATE_DEFAULT, (_e, taskId: unknown) => pauseTask(str(taskId, MAX_PATH)));
  handle('download:resume', RATE_DEFAULT, (_e, taskId: unknown) => resumeTask(str(taskId, MAX_PATH)));
  handle('download:list', RATE_DEFAULT, (): DownloadTask[] => getAllTasks());
  handle('download:remove', RATE_DEFAULT, (_e, ids: unknown): boolean => {
    if (!Array.isArray(ids)) throw new Error('Invalid input: expected an array of task ids');
    return removeTasks(ids.map((id) => str(id, MAX_PATH)));
  });

  // ── dialog:* ────────────────────────────────────────────────────────────────
  handle('dialog:selectDir', RATE_DEFAULT, async (): Promise<string | null> => {
    const win = getMainWindow();
    const { canceled, filePaths } = win
      ? await dialog.showOpenDialog(win, { properties: ['openDirectory'] })
      : await dialog.showOpenDialog({ properties: ['openDirectory'] });
    return canceled || filePaths.length === 0 ? null : filePaths[0];
  });

  // ── shell:* ─────────────────────────────────────────────────────────────────
  // Reveal a downloaded file. The path is CONFINED to the configured output dir —
  // a compromised renderer must not be able to reveal/launch arbitrary files (§15).
  // Returns whether the file was actually there. A download recorded months ago
  // may have been moved, renamed or deleted outside the app — showItemInFolder on
  // a missing path opens nothing and reports nothing, so the click just dies. The
  // renderer needs the answer to say so out loud.
  handle('shell:showItemInFolder', RATE_DEFAULT, (_e, p: unknown): RevealResult => {
    const raw = str(p, MAX_PATH);
    if (!raw.trim()) return { ok: false, reason: 'no-path' };
    const abs = confineToRoot(raw, config.get('outputDir'));
    if (!abs) throw new Error('Invalid input: path escapes the output directory');
    if (!existsSync(abs)) return { ok: false, reason: 'missing' };
    shell.showItemInFolder(abs);
    return { ok: true };
  });

  // ── stats:* ─────────────────────────────────────────────────────────────────
  // Read-only: the renderer can never write a counter, only ask what it says.
  handle('stats:get', RATE_DEFAULT, () => stats.get());

  // ── license:* / app:* (LICENSE-ACTIVATION-SYSTEM.md §7) ──────────────────────
  // Renderer only ever sees { activated } / ActivationResult — secrets stay in main.
  handle('license:check', RATE_DEFAULT, () => license.checkLicense());
  handle('license:activate', RATE_DEFAULT, (_e, key: unknown) => license.activateLicense(str(key)));
  handle('license:deactivate', RATE_DEFAULT, () => license.deactivateLicense());
  handle('license:release', RATE_DEFAULT, () => license.releaseLicense());
  // Open the support link. The URL is a MAIN-SIDE CONSTANT — the renderer supplies
  // nothing, so there's no path/URL injection surface; openExternal, not openPath.
  handle('app:openSupport', RATE_DEFAULT, () => {
    const url = license.getSupportLink();
    if (/^https:\/\//i.test(url)) shell.openExternal(url).catch(() => {});
  });

  // ── update:* (self-update, §12) — delegate to the Updater; host-vetting lives there.
  handle('update:check', RATE_DEFAULT, () => updater.check());
  // Readable state, so a tab mounted after the startup check still sees its result.
  handle('update:state', RATE_DEFAULT, () => updater.getState());
  handle('update:download', RATE_DEFAULT, () => updater.download());
  handle('update:install', RATE_DEFAULT, () => updater.install());

  // ── ytdlp:* (#26 engine self-update) — host/checksum vetting lives in the updater.
  handle('ytdlp:version', RATE_DEFAULT, (_e, force: unknown) => ytdlp.check(force === true));
  handle('ytdlp:update', RATE_DEFAULT, () => ytdlp.update());
}
