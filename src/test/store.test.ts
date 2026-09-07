import { useAppStore } from '@/store';
import type { AppConfig, DownloadProgress, DownloadTask, Playlist } from '@shared/types';

const defaultProgress: DownloadProgress = {
  status: 'queued',
  percent: 0,
  speed: 0,
  eta: 0,
  downloaded: 0,
  total: 0,
  filename: '',
  error: '',
  playlistIndex: 0,
  playlistTotal: 0,
};

const defaultConfig: AppConfig = {
  outputDir: '',
  defaultFormat: 'mp3',
  defaultQuality: '320',
  defaultVideoQuality: 'best',
  concurrentDownloads: 1,
  embedThumbnail: true,
  embedMetadata: true,
  skipExisting: true,
  theme: 'dark',
  rememberLastDir: true,
  autoPaste: true,
  clipboardWatch: false,
  showNotifications: true,
  showVisualizer: true,
  showAmbientParticles: true,
  rateLimit: '',
  proxy: '',
  ffmpegPath: '',
  minimizeToTray: false,
  globalHotkey: '',
  discordRichPresence: false,
  scheduleEnabled: false,
  scheduleTime: '00:00',
  scheduleDays: [],
  scheduleShutdown: false,
};

function makeTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    taskId: 'task_1',
    url: 'https://youtube.com/watch?v=test',
    outputDir: 'C:\\Downloads',
    format: 'mp3',
    quality: '320',
    videoQuality: 'best',
    isAudioOnly: true,
    isPlaylist: false,
    playlistName: '',
    embedThumbnail: true,
    embedMetadata: true,
    skipExisting: true,
    title: 'Test Song',
    thumbnailUrl: '',
    duration: 180,
    uploader: 'TestArtist',
    source: 'youtube',
    progress: { ...defaultProgress },
    ...overrides,
  };
}

// ── download slice ────────────────────────────────────────────────────────────

describe('Store — download slice', () => {
  beforeEach(() => {
    useAppStore.setState({ downloads: {}, selectedDownloadIds: new Set() });
  });

  it('addDownload stores the task', () => {
    const task = makeTask({ taskId: 'task_1' });
    useAppStore.getState().addDownload(task);
    expect(useAppStore.getState().downloads['task_1']).toEqual(task);
  });

  it('updateDownloadProgress merges new progress', () => {
    useAppStore.getState().addDownload(makeTask({ taskId: 'task_1' }));
    const next: DownloadProgress = { ...defaultProgress, status: 'downloading', percent: 50, speed: 1_048_576 };
    useAppStore.getState().updateDownloadProgress('task_1', next);
    const stored = useAppStore.getState().downloads['task_1'];
    expect(stored.progress.status).toBe('downloading');
    expect(stored.progress.percent).toBe(50);
  });

  it('updateDownloadProgress is a no-op for unknown id', () => {
    const snapshot = { ...useAppStore.getState().downloads };
    useAppStore.getState().updateDownloadProgress('ghost', defaultProgress);
    expect(useAppStore.getState().downloads).toEqual(snapshot);
  });

  it('removeDownload deletes the task', () => {
    useAppStore.getState().addDownload(makeTask({ taskId: 'task_1' }));
    useAppStore.getState().removeDownload('task_1');
    expect(useAppStore.getState().downloads['task_1']).toBeUndefined();
  });

  it('removeDownload cleans the task from selectedDownloadIds', () => {
    useAppStore.getState().addDownload(makeTask({ taskId: 'task_1' }));
    useAppStore.getState().toggleDownloadSelection('task_1');
    expect(useAppStore.getState().selectedDownloadIds.has('task_1')).toBe(true);
    useAppStore.getState().removeDownload('task_1');
    expect(useAppStore.getState().selectedDownloadIds.has('task_1')).toBe(false);
  });

  it('clearCompleted removes done and cancelled tasks', () => {
    useAppStore.getState().addDownload(makeTask({ taskId: 'done', progress: { ...defaultProgress, status: 'done', percent: 100 } }));
    useAppStore.getState().addDownload(makeTask({ taskId: 'cancelled', progress: { ...defaultProgress, status: 'cancelled' } }));
    useAppStore.getState().addDownload(makeTask({ taskId: 'active' }));
    useAppStore.getState().clearCompleted();
    const { downloads } = useAppStore.getState();
    expect(downloads['done']).toBeUndefined();
    expect(downloads['cancelled']).toBeUndefined();
    expect(downloads['active']).toBeDefined();
  });

  it('clearCompleted also cleans selectedDownloadIds for dropped tasks', () => {
    useAppStore.getState().addDownload(makeTask({ taskId: 'done', progress: { ...defaultProgress, status: 'done', percent: 100 } }));
    useAppStore.getState().toggleDownloadSelection('done');
    useAppStore.getState().clearCompleted();
    expect(useAppStore.getState().selectedDownloadIds.has('done')).toBe(false);
  });

  it('toggleDownloadSelection adds then removes an id', () => {
    useAppStore.getState().addDownload(makeTask({ taskId: 'task_1' }));
    useAppStore.getState().toggleDownloadSelection('task_1');
    expect(useAppStore.getState().selectedDownloadIds.has('task_1')).toBe(true);
    useAppStore.getState().toggleDownloadSelection('task_1');
    expect(useAppStore.getState().selectedDownloadIds.has('task_1')).toBe(false);
  });

  it('setDownloads hydrates from a task list, keyed by taskId', () => {
    useAppStore.getState().setDownloads([makeTask({ taskId: 'a' }), makeTask({ taskId: 'b' })]);
    const { downloads } = useAppStore.getState();
    expect(Object.keys(downloads).sort()).toEqual(['a', 'b']);
    expect(downloads['a'].taskId).toBe('a');
  });

  it('setDownloads replaces any prior state (boot hydration overwrite)', () => {
    useAppStore.getState().addDownload(makeTask({ taskId: 'stale' }));
    useAppStore.getState().setDownloads([makeTask({ taskId: 'fresh' })]);
    const { downloads } = useAppStore.getState();
    expect(downloads['stale']).toBeUndefined();
    expect(downloads['fresh']).toBeDefined();
  });

  it('selectAllDownloads with explicit ids selects only those', () => {
    useAppStore.getState().addDownload(makeTask({ taskId: 'a' }));
    useAppStore.getState().addDownload(makeTask({ taskId: 'b' }));
    useAppStore.getState().selectAllDownloads(['a']);
    const { selectedDownloadIds } = useAppStore.getState();
    expect(selectedDownloadIds.has('a')).toBe(true);
    expect(selectedDownloadIds.has('b')).toBe(false);
  });

  it('selectAllDownloads with no argument selects every download', () => {
    useAppStore.getState().addDownload(makeTask({ taskId: 'a' }));
    useAppStore.getState().addDownload(makeTask({ taskId: 'b' }));
    useAppStore.getState().selectAllDownloads();
    expect(useAppStore.getState().selectedDownloadIds.size).toBe(2);
  });

  it('clearDownloadSelection empties the selection', () => {
    useAppStore.getState().addDownload(makeTask({ taskId: 'a' }));
    useAppStore.getState().toggleDownloadSelection('a');
    useAppStore.getState().clearDownloadSelection();
    expect(useAppStore.getState().selectedDownloadIds.size).toBe(0);
  });

  it('clearCompletedAndPersist calls download.remove with done+cancelled ids, then drops them locally', () => {
    useAppStore.getState().addDownload(makeTask({ taskId: 'done', progress: { ...defaultProgress, status: 'done' } }));
    useAppStore.getState().addDownload(makeTask({ taskId: 'cancelled', progress: { ...defaultProgress, status: 'cancelled' } }));
    useAppStore.getState().addDownload(makeTask({ taskId: 'active' }));
    useAppStore.getState().clearCompletedAndPersist();
    expect(window.electronAPI.download.remove).toHaveBeenCalledWith(expect.arrayContaining(['done', 'cancelled']));
    const { downloads } = useAppStore.getState();
    expect(downloads['done']).toBeUndefined();
    expect(downloads['cancelled']).toBeUndefined();
    expect(downloads['active']).toBeDefined();
  });

  it('clearCompletedAndPersist skips the IPC call when nothing is done/cancelled', () => {
    vi.mocked(window.electronAPI.download.remove).mockClear();
    useAppStore.getState().addDownload(makeTask({ taskId: 'active' }));
    useAppStore.getState().clearCompletedAndPersist();
    expect(window.electronAPI.download.remove).not.toHaveBeenCalled();
  });
});

// ── config slice ──────────────────────────────────────────────────────────────

describe('Store — config slice', () => {
  beforeEach(() => {
    useAppStore.setState({ config: { ...defaultConfig } });
    vi.clearAllMocks();
  });

  it('setConfig replaces the entire config', () => {
    const next: AppConfig = { ...defaultConfig, theme: 'light', outputDir: 'X:\\Custom' };
    useAppStore.getState().setConfig(next);
    expect(useAppStore.getState().config?.theme).toBe('light');
    expect(useAppStore.getState().config?.outputDir).toBe('X:\\Custom');
  });

  it('updateConfig merges partial values and keeps the rest unchanged', () => {
    useAppStore.getState().updateConfig({ outputDir: 'D:\\Music', defaultQuality: '192' });
    const cfg = useAppStore.getState().config;
    expect(cfg?.outputDir).toBe('D:\\Music');
    expect(cfg?.defaultQuality).toBe('192');
    expect(cfg?.defaultFormat).toBe('mp3'); // unchanged
  });

  it('updateConfig calls window.electronAPI.config.update with the partial', () => {
    useAppStore.getState().updateConfig({ outputDir: 'D:\\Music' });
    expect(window.electronAPI.config.update).toHaveBeenCalledWith({ outputDir: 'D:\\Music' });
  });
});

// ── ui slice ──────────────────────────────────────────────────────────────────

describe('Store — ui slice', () => {
  beforeEach(() => {
    useAppStore.setState({
      activeTab: 'home',
      selectedTracks: new Set(),
      currentPlaylist: null,
      commandPaletteOpen: false,
    });
  });

  it('setActiveTab changes the active tab', () => {
    useAppStore.getState().setActiveTab('settings');
    expect(useAppStore.getState().activeTab).toBe('settings');
  });

  it('setCurrentPlaylist sets playlist and clears selectedTracks', () => {
    useAppStore.setState({ selectedTracks: new Set(['t1', 't2']) });
    const playlist: Playlist = { id: 'pl_1', name: 'Test', description: '', owner: '', tracks: [], thumbnailUrl: '', url: '', trackCount: 0 };
    useAppStore.getState().setCurrentPlaylist(playlist);
    expect(useAppStore.getState().currentPlaylist?.id).toBe('pl_1');
    expect(useAppStore.getState().selectedTracks.size).toBe(0);
  });

  it('setCurrentPlaylist(null) clears the playlist and selectedTracks', () => {
    useAppStore.setState({ selectedTracks: new Set(['t1']) });
    useAppStore.getState().setCurrentPlaylist(null);
    expect(useAppStore.getState().currentPlaylist).toBeNull();
    expect(useAppStore.getState().selectedTracks.size).toBe(0);
  });

  it('toggleTrack adds and removes from selectedTracks', () => {
    useAppStore.getState().toggleTrack('t1');
    expect(useAppStore.getState().selectedTracks.has('t1')).toBe(true);
    useAppStore.getState().toggleTrack('t1');
    expect(useAppStore.getState().selectedTracks.has('t1')).toBe(false);
  });

  it('toggleCommandPalette flips commandPaletteOpen', () => {
    expect(useAppStore.getState().commandPaletteOpen).toBe(false);
    useAppStore.getState().toggleCommandPalette();
    expect(useAppStore.getState().commandPaletteOpen).toBe(true);
    useAppStore.getState().toggleCommandPalette();
    expect(useAppStore.getState().commandPaletteOpen).toBe(false);
  });

  it('setWindowMaximized sets isWindowMaximized', () => {
    useAppStore.getState().setWindowMaximized(true);
    expect(useAppStore.getState().isWindowMaximized).toBe(true);
    useAppStore.getState().setWindowMaximized(false);
    expect(useAppStore.getState().isWindowMaximized).toBe(false);
  });

  it('selectAllTracks selects every track id in the current playlist', () => {
    const playlist: Playlist = {
      id: 'pl_1', name: 'Test', description: '', owner: '', thumbnailUrl: '', url: '', trackCount: 2,
      tracks: [
        { id: 't1', name: 'A', artist: '', artists: [], album: '', trackNumber: 1, durationMs: 0, thumbnailUrl: '' },
        { id: 't2', name: 'B', artist: '', artists: [], album: '', trackNumber: 2, durationMs: 0, thumbnailUrl: '' },
      ],
    };
    useAppStore.setState({ currentPlaylist: playlist, selectedTracks: new Set() });
    useAppStore.getState().selectAllTracks();
    const { selectedTracks } = useAppStore.getState();
    expect(selectedTracks.has('t1')).toBe(true);
    expect(selectedTracks.has('t2')).toBe(true);
  });

  it('clearTracks empties selectedTracks', () => {
    useAppStore.setState({ selectedTracks: new Set(['t1']) });
    useAppStore.getState().clearTracks();
    expect(useAppStore.getState().selectedTracks.size).toBe(0);
  });
});

// ── tier slice ────────────────────────────────────────────────────────────────

describe('Store — tier slice', () => {
  beforeEach(() => {
    useAppStore.setState({ isActivated: false, plan: 'basic', upgradeOpen: false, notice: null });
  });

  it('defaults to the free tier — no key is a working install, not a locked one', () => {
    expect(useAppStore.getState().plan).toBe('basic');
    expect(useAppStore.getState().isActivated).toBe(false);
  });

  it('setLicense carries activation and plan together', () => {
    useAppStore.getState().setLicense({ activated: true, plan: 'premium' });
    expect(useAppStore.getState().isActivated).toBe(true);
    expect(useAppStore.getState().plan).toBe('premium');

    useAppStore.getState().setLicense({ activated: false, plan: 'basic' });
    expect(useAppStore.getState().isActivated).toBe(false);
    expect(useAppStore.getState().plan).toBe('basic');
  });

  it('opening the upgrade sheet clears a pending nudge', () => {
    useAppStore.getState().showPremiumNudge('4K needs Premium.');
    expect(useAppStore.getState().notice?.message).toBe('4K needs Premium.');

    useAppStore.getState().setUpgradeOpen(true);
    expect(useAppStore.getState().upgradeOpen).toBe(true);
    expect(useAppStore.getState().notice).toBeNull();
  });

  it('dismissing a nudge leaves the sheet closed', () => {
    useAppStore.getState().showPremiumNudge('Lossless is Premium.');
    useAppStore.getState().dismissNotice();
    expect(useAppStore.getState().notice).toBeNull();
    expect(useAppStore.getState().upgradeOpen).toBe(false);
  });
});
