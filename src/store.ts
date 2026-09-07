// Single Zustand store. No Immer — manual spreads; selections use fresh Sets
// so referential change triggers re-render (CLAUDE.md). Subscribe with narrow
// selectors. Persistence + theme side effects live in config actions.
import { create } from 'zustand';
import type { AppConfig, DownloadTask, DownloadProgress, Plan, Playlist, UrlDetection } from '@shared/types';
import type { TabKey } from '@/constants';
import { applyTheme } from '@/lib/theme';

export interface AppState {
  // ── downloads ──────────────────────────────────────────────
  downloads: Record<string, DownloadTask>;
  selectedDownloadIds: Set<string>;
  setDownloads: (tasks: DownloadTask[]) => void;
  addDownload: (task: DownloadTask) => void;
  updateDownloadProgress: (id: string, progress: DownloadProgress) => void;
  removeDownload: (id: string) => void;
  clearCompleted: () => void;
  /** Removes done+cancelled tasks from main's persisted history, then clears them locally. */
  clearCompletedAndPersist: () => void;
  toggleDownloadSelection: (id: string) => void;
  /** Selects the given ids, or every download if omitted. */
  selectAllDownloads: (ids?: string[]) => void;
  clearDownloadSelection: () => void;

  // ── config ─────────────────────────────────────────────────
  config: AppConfig | null;
  setConfig: (cfg: AppConfig) => void;
  updateConfig: (partial: Partial<AppConfig>) => void;

  // ── ui ─────────────────────────────────────────────────────
  activeTab: TabKey;
  setActiveTab: (tab: TabKey) => void;
  isWindowMaximized: boolean;
  setWindowMaximized: (v: boolean) => void;
  currentPlaylist: Playlist | null;
  setCurrentPlaylist: (p: Playlist | null) => void;
  /** URL handed from the Dashboard to a platform tab to auto-fetch on arrival. */
  pendingInput: string | null;
  setPendingInput: (v: string | null) => void;
  selectedTracks: Set<string>;
  toggleTrack: (id: string) => void;
  selectAllTracks: () => void;
  clearTracks: () => void;
  commandPaletteOpen: boolean;
  setCommandPaletteOpen: (v: boolean) => void;
  toggleCommandPalette: () => void;

  // ── tier ───────────────────────────────────────────────────
  /** Free by default; 'premium' only after a key is verified. */
  plan: Plan;
  /** Whether a key is bound to this machine — drives Settings, not feature access. */
  isActivated: boolean;
  setLicense: (v: { activated: boolean; plan: Plan }) => void;
  /** The upgrade sheet (pricing cards + key entry). */
  upgradeOpen: boolean;
  setUpgradeOpen: (v: boolean) => void;
  /** The one transient popup. Two tones, because "you found a ceiling" and
   *  "that didn't work" need different colour and urgency but identical mechanics. */
  notice: { message: string; tone: 'premium' | 'error' } | null;
  showPremiumNudge: (reason: string) => void;
  showError: (message: string) => void;
  dismissNotice: () => void;

  // ── clipboard watcher ──────────────────────────────────────
  /** The most recent link the clipboard watcher offered to download, or null. */
  clipboardOffer: UrlDetection | null;
  setClipboardOffer: (d: UrlDetection | null) => void;
}

export const useAppStore = create<AppState>()((set, get) => ({
  // ── downloads ──
  downloads: {},
  selectedDownloadIds: new Set(),
  setDownloads: (tasks) =>
    set(() => {
      const downloads: Record<string, DownloadTask> = {};
      for (const t of tasks) downloads[t.taskId] = t;
      return { downloads };
    }),
  addDownload: (task) =>
    set((s) => ({ downloads: { ...s.downloads, [task.taskId]: task } })),
  updateDownloadProgress: (id, progress) =>
    set((s) => {
      const t = s.downloads[id];
      if (!t) return s;
      return { downloads: { ...s.downloads, [id]: { ...t, progress } } };
    }),
  removeDownload: (id) =>
    set((s) => {
      const { [id]: _removed, ...downloads } = s.downloads;
      const sel = new Set(s.selectedDownloadIds);
      sel.delete(id);
      return { downloads, selectedDownloadIds: sel };
    }),
  clearCompleted: () =>
    set((s) => {
      const downloads: Record<string, DownloadTask> = {};
      const sel = new Set(s.selectedDownloadIds);
      for (const [id, t] of Object.entries(s.downloads)) {
        const done = t.progress.status === 'done' || t.progress.status === 'cancelled';
        if (done) sel.delete(id);
        else downloads[id] = t;
      }
      return { downloads, selectedDownloadIds: sel };
    }),
  clearCompletedAndPersist: () => {
    const ids = Object.values(get().downloads)
      .filter((t) => t.progress.status === 'done' || t.progress.status === 'cancelled')
      .map((t) => t.taskId);
    if (ids.length) void window.electronAPI.download.remove(ids);
    get().clearCompleted();
  },
  toggleDownloadSelection: (id) =>
    set((s) => {
      const sel = new Set(s.selectedDownloadIds);
      sel.has(id) ? sel.delete(id) : sel.add(id);
      return { selectedDownloadIds: sel };
    }),
  selectAllDownloads: (ids) =>
    set((s) => ({ selectedDownloadIds: new Set(ids ?? Object.keys(s.downloads)) })),
  clearDownloadSelection: () => set({ selectedDownloadIds: new Set() }),

  // ── config ──
  config: null,
  setConfig: (cfg) => {
    set({ config: cfg });
    applyTheme(cfg.theme);
  },
  updateConfig: (partial) => {
    const cur = get().config;
    if (!cur) return; // config not loaded yet — don't build a partial, incomplete AppConfig
    set({ config: { ...cur, ...partial } });
    void window.electronAPI.config.update(partial);
    if (partial.theme) applyTheme(partial.theme);
  },

  // ── ui ──
  activeTab: 'home',
  setActiveTab: (tab) => set({ activeTab: tab }),
  isWindowMaximized: false,
  setWindowMaximized: (v) => set({ isWindowMaximized: v }),
  currentPlaylist: null,
  setCurrentPlaylist: (p) => set({ currentPlaylist: p, selectedTracks: new Set() }),
  pendingInput: null,
  setPendingInput: (v) => set({ pendingInput: v }),
  selectedTracks: new Set(),
  toggleTrack: (id) =>
    set((s) => {
      const sel = new Set(s.selectedTracks);
      sel.has(id) ? sel.delete(id) : sel.add(id);
      return { selectedTracks: sel };
    }),
  selectAllTracks: () =>
    set((s) => ({ selectedTracks: new Set(s.currentPlaylist?.tracks.map((t) => t.id) ?? []) })),
  clearTracks: () => set({ selectedTracks: new Set() }),
  commandPaletteOpen: false,
  setCommandPaletteOpen: (v) => set({ commandPaletteOpen: v }),
  toggleCommandPalette: () => set((s) => ({ commandPaletteOpen: !s.commandPaletteOpen })),

  // ── tier ──
  plan: 'basic',
  isActivated: false,
  setLicense: ({ activated, plan }) => set({ isActivated: activated, plan }),
  upgradeOpen: false,
  // Opening the sheet clears any notice — the sheet answers what the nudge asked.
  setUpgradeOpen: (v) => set({ upgradeOpen: v, ...(v ? { notice: null } : {}) }),
  notice: null,
  showPremiumNudge: (message) => set({ notice: { message, tone: 'premium' } }),
  showError: (message) => set({ notice: { message, tone: 'error' } }),
  dismissNotice: () => set({ notice: null }),

  // ── clipboard watcher ──
  clipboardOffer: null,
  setClipboardOffer: (d) => set({ clipboardOffer: d }),
}));

/** One place that answers "may this install use premium features?". */
export const useIsPremium = (): boolean => useAppStore((s) => s.plan === 'premium');
