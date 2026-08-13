// Layer 3 of the IPC contract (CLAUDE.md): the renderer-side type of the preload
// bridge. Channel names + signatures here mirror electron/ipc.ts + preload.cjs —
// interface declaration, not logic duplication. Domain shapes come from shared/.
import type {
  AppConfig,
  UrlDetection,
  SearchResult,
  DownloadTask,
  DownloadProgress,
  Track,
  Playlist,
  ActivationResult,
  LicenseCheck,
  Plan,
  ReleaseResult,
  MediaMetadata,
  YtdlpUpdateStatus,
} from '@shared/types';

export type { MediaMetadata };

export interface ElectronAPI {
  window: {
    minimize: () => Promise<void>;
    maximize: () => Promise<void>;
    close: () => Promise<void>;
    isMaximized: () => Promise<boolean>;
  };
  config: {
    get: <K extends keyof AppConfig>(key: K) => Promise<AppConfig[K]>;
    set: <K extends keyof AppConfig>(key: K, value: AppConfig[K]) => Promise<boolean>;
    getAll: () => Promise<AppConfig>;
    update: (data: Partial<AppConfig>) => Promise<boolean>;
  };
  url: {
    detect: (url: string) => Promise<UrlDetection>;
    fetchMetadata: (url: string) => Promise<MediaMetadata>;
  };
  youtube: {
    search: (query: string) => Promise<SearchResult[]>;
  };
  soundcloud: {
    search: (query: string) => Promise<SearchResult[]>;
  };
  spotify: {
    parseUrl: (url: string) => Promise<{ type: 'track' | 'playlist' | 'album' | 'artist' | null; id: string | null }>;
    fetchTrack: (idOrUrl: string) => Promise<Track | null>;
    fetchPlaylist: (idOrUrl: string) => Promise<Playlist | null>;
    fetchAlbum: (idOrUrl: string) => Promise<Playlist | null>;
    search: (query: string) => Promise<Track[]>;
  };
  download: {
    start: (taskData: Partial<DownloadTask>) => Promise<DownloadTask>;
    cancel: (taskId: string) => Promise<boolean>;
    cancelAll: () => Promise<void>;
    pause: (taskId: string) => Promise<boolean>;
    resume: (taskId: string) => Promise<boolean>;
    list: () => Promise<DownloadTask[]>;
    remove: (taskIds: string[]) => Promise<boolean>;
  };
  dialog: {
    selectDir: () => Promise<string | null>;
  };
  shell: {
    showItemInFolder: (path: string) => Promise<void>;
  };
  license: {
    check: () => Promise<LicenseCheck>;
    activate: (key: string) => Promise<ActivationResult>;
    deactivate: () => Promise<{ success: boolean; message?: string }>;
    release: () => Promise<ReleaseResult>;
  };
  app: {
    openSupport: () => Promise<void>;
  };
  update: {
    check: () => Promise<void>;
    download: () => Promise<void>;
    install: () => Promise<void>;
  };
  // #26 yt-dlp engine self-update.
  ytdlp: {
    version: (force?: boolean) => Promise<YtdlpUpdateStatus>;
    update: () => Promise<YtdlpUpdateStatus>;
  };
  // Event listeners (main → renderer); each returns an unsubscribe function.
  onDownloadQueued: (callback: (taskId: string, task: DownloadTask) => void) => () => void;
  onDownloadProgress: (callback: (taskId: string, progress: DownloadProgress) => void) => () => void;
  onDownloadDone: (callback: (taskId: string, progress: DownloadProgress) => void) => () => void;
  onDownloadError: (callback: (taskId: string, error: string) => void) => () => void;
  onDownloadCancelled: (callback: (taskId: string) => void) => () => void;
  onWindowMaximized: (cb: (maximized: boolean) => void) => () => void;
  onLicenseStateChanged: (cb: (d: { activated: boolean; plan: Plan }) => void) => () => void;
  onUpdateAvailable: (cb: (info: { version: string }) => void) => () => void;
  onUpdateNotAvailable: (cb: () => void) => () => void;
  onUpdateProgress: (cb: (info: { percent: number }) => void) => () => void;
  onUpdateDownloaded: (cb: (info: { version: string }) => void) => () => void;
  onUpdateError: (cb: (info: { message: string }) => void) => () => void;
  onYtdlpStatus: (cb: (status: YtdlpUpdateStatus) => void) => () => void;
}

declare global {
  interface Window {
    electronAPI: ElectronAPI;
  }
}
