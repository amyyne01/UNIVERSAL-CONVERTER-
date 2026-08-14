// CommonJS — NOT compiled by tsc. Copied verbatim into dist-electron/electron/ by the build.
// The ONLY bridge between the sandboxed renderer and the privileged main process.
const { contextBridge, ipcRenderer } = require('electron');

// Subscribe to a main→renderer event; returns an unsubscribe fn (for useEffect cleanup).
const on = (channel, callback) => {
  const handler = (_event, ...args) => callback(...args);
  ipcRenderer.on(channel, handler);
  return () => ipcRenderer.removeListener(channel, handler);
};

contextBridge.exposeInMainWorld('electronAPI', {
  window: {
    minimize: () => ipcRenderer.invoke('window:minimize'),
    maximize: () => ipcRenderer.invoke('window:maximize'),
    close: () => ipcRenderer.invoke('window:close'),
    isMaximized: () => ipcRenderer.invoke('window:isMaximized'),
  },
  config: {
    get: (key) => ipcRenderer.invoke('config:get', key),
    set: (key, value) => ipcRenderer.invoke('config:set', key, value),
    getAll: () => ipcRenderer.invoke('config:getAll'),
    update: (data) => ipcRenderer.invoke('config:update', data),
  },
  url: {
    detect: (url) => ipcRenderer.invoke('url:detect', url),
    fetchMetadata: (url) => ipcRenderer.invoke('url:fetchMetadata', url),
  },
  youtube: {
    search: (query) => ipcRenderer.invoke('youtube:search', query),
  },
  soundcloud: {
    search: (query) => ipcRenderer.invoke('soundcloud:search', query),
  },
  spotify: {
    parseUrl: (url) => ipcRenderer.invoke('spotify:parseUrl', url),
    fetchTrack: (idOrUrl) => ipcRenderer.invoke('spotify:fetchTrack', idOrUrl),
    fetchPlaylist: (idOrUrl) => ipcRenderer.invoke('spotify:fetchPlaylist', idOrUrl),
    fetchAlbum: (idOrUrl) => ipcRenderer.invoke('spotify:fetchAlbum', idOrUrl),
    search: (query) => ipcRenderer.invoke('spotify:search', query),
  },
  download: {
    start: (taskData) => ipcRenderer.invoke('download:start', taskData),
    cancel: (taskId) => ipcRenderer.invoke('download:cancel', taskId),
    cancelAll: () => ipcRenderer.invoke('download:cancelAll'),
    pause: (taskId) => ipcRenderer.invoke('download:pause', taskId),
    resume: (taskId) => ipcRenderer.invoke('download:resume', taskId),
    list: () => ipcRenderer.invoke('download:list'),
    remove: (taskIds) => ipcRenderer.invoke('download:remove', taskIds),
  },
  dialog: {
    selectDir: () => ipcRenderer.invoke('dialog:selectDir'),
  },
  shell: {
    showItemInFolder: (path) => ipcRenderer.invoke('shell:showItemInFolder', path),
  },
  stats: {
    get: () => ipcRenderer.invoke('stats:get'),
  },
  license: {
    check: () => ipcRenderer.invoke('license:check'),
    activate: (key) => ipcRenderer.invoke('license:activate', key),
    deactivate: () => ipcRenderer.invoke('license:deactivate'),
    release: () => ipcRenderer.invoke('license:release'),
  },
  app: {
    openSupport: () => ipcRenderer.invoke('app:openSupport'),
  },
  update: {
    check: () => ipcRenderer.invoke('update:check'),
    download: () => ipcRenderer.invoke('update:download'),
    install: () => ipcRenderer.invoke('update:install'),
  },
  ytdlp: {
    version: (force) => ipcRenderer.invoke('ytdlp:version', force === true),
    update: () => ipcRenderer.invoke('ytdlp:update'),
  },
  // Event listeners (main → renderer) each return an unsubscribe function.
  onDownloadQueued: (callback) => on('download:queued', callback),
  onDownloadProgress: (callback) => on('download:progress', callback),
  onDownloadDone: (callback) => on('download:done', callback),
  onDownloadError: (callback) => on('download:error', callback),
  onDownloadCancelled: (callback) => on('download:cancelled', callback),
  onWindowMaximized: (callback) => on('window:maximized', callback),
  onLicenseStateChanged: (callback) => on('license:state-changed', callback),
  onUpdateAvailable: (callback) => on('update:available', callback),
  onUpdateNotAvailable: (callback) => on('update:not-available', callback),
  onUpdateProgress: (callback) => on('update:progress', callback),
  onUpdateDownloaded: (callback) => on('update:downloaded', callback),
  onUpdateError: (callback) => on('update:error', callback),
  onYtdlpStatus: (callback) => on('ytdlp:status', callback),
});
