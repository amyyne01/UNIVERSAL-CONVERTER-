import '@testing-library/jest-dom/vitest';

// ResizeObserver — used by framer-motion internally
global.ResizeObserver = class {
  observe() {}
  unobserve() {}
  disconnect() {}
};

// matchMedia — jsdom omits it; needed by applyTheme (resolveTheme) in the store
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});

// window.electronAPI — mirrors ElectronAPI in src/types.ts exactly
const mockElectronAPI = {
  window: {
    minimize: vi.fn().mockResolvedValue(undefined),
    maximize: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    isMaximized: vi.fn().mockResolvedValue(false),
  },
  config: {
    get: vi.fn().mockResolvedValue(null),
    set: vi.fn().mockResolvedValue(true),
    getAll: vi.fn().mockResolvedValue({
      outputDir: '',
      defaultFormat: 'mp3',
      defaultQuality: '320',
      defaultVideoQuality: 'best',
      embedThumbnail: true,
      embedMetadata: true,
      skipExisting: true,
      theme: 'dark',
      rememberLastDir: true,
      autoPaste: true,
      showNotifications: true,
      rateLimit: '',
      proxy: '',
      ffmpegPath: '',
      globalHotkey: '',
      discordRichPresence: false,
      scheduleEnabled: false,
      scheduleTime: '00:00',
      scheduleDays: [],
      scheduleShutdown: false,
    }),
    update: vi.fn().mockResolvedValue(true),
  },
  url: {
    detect: vi.fn().mockResolvedValue({
      url: '',
      platform: 'unknown',
      contentType: 'unknown',
      isCollection: false,
      label: '',
    }),
    fetchMetadata: vi.fn().mockResolvedValue(null),
  },
  youtube: { search: vi.fn().mockResolvedValue([]) },
  soundcloud: { search: vi.fn().mockResolvedValue([]) },
  spotify: {
    parseUrl: vi.fn().mockResolvedValue({ type: null, id: null }),
    fetchTrack: vi.fn().mockResolvedValue(null),
    fetchPlaylist: vi.fn().mockResolvedValue(null),
    fetchAlbum: vi.fn().mockResolvedValue(null),
    search: vi.fn().mockResolvedValue([]),
  },
  download: {
    start: vi.fn().mockResolvedValue({ taskId: 'mock-task' }),
    cancel: vi.fn().mockResolvedValue(true),
    cancelAll: vi.fn().mockResolvedValue(undefined),
    pause: vi.fn().mockResolvedValue(true),
    resume: vi.fn().mockResolvedValue(true),
    list: vi.fn().mockResolvedValue([]),
    remove: vi.fn().mockResolvedValue(true),
  },
  dialog: { selectDir: vi.fn().mockResolvedValue(null) },
  shell: {
    showItemInFolder: vi.fn().mockResolvedValue(undefined),
  },
  license: {
    check: vi.fn().mockResolvedValue({ activated: false, plan: 'basic' }),
    activate: vi.fn().mockResolvedValue({ success: false }),
    deactivate: vi.fn().mockResolvedValue({ success: true }),
    release: vi.fn().mockResolvedValue({ success: true }),
  },
  app: { openSupport: vi.fn().mockResolvedValue(undefined) },
  update: {
    check: vi.fn().mockResolvedValue(undefined),
    download: vi.fn().mockResolvedValue(undefined),
    install: vi.fn().mockResolvedValue(undefined),
  },
  // Event listeners — each returns an unsubscribe fn
  onDownloadQueued: vi.fn().mockReturnValue(vi.fn()),
  onDownloadProgress: vi.fn().mockReturnValue(vi.fn()),
  onDownloadDone: vi.fn().mockReturnValue(vi.fn()),
  onDownloadError: vi.fn().mockReturnValue(vi.fn()),
  onDownloadCancelled: vi.fn().mockReturnValue(vi.fn()),
  onWindowMaximized: vi.fn().mockReturnValue(vi.fn()),
  onLicenseStateChanged: vi.fn().mockReturnValue(vi.fn()),
  onUpdateAvailable: vi.fn().mockReturnValue(vi.fn()),
  onUpdateNotAvailable: vi.fn().mockReturnValue(vi.fn()),
  onUpdateProgress: vi.fn().mockReturnValue(vi.fn()),
  onUpdateDownloaded: vi.fn().mockReturnValue(vi.fn()),
  onUpdateError: vi.fn().mockReturnValue(vi.fn()),
};

Object.defineProperty(window, 'electronAPI', {
  value: mockElectronAPI,
  writable: true,
  configurable: true,
});

// framer-motion mock — render motion.* as their underlying tag and STRIP
// motion-only props so they don't leak onto DOM elements (kills the noisy
// "React does not recognize `whileTap`" warnings). Centralized here so test
// files don't each re-mock framer-motion.
vi.mock('framer-motion', async () => {
  const React = await import('react');
  const MOTION_ONLY = new Set([
    'initial', 'animate', 'exit', 'transition', 'variants', 'custom',
    'whileHover', 'whileTap', 'whileFocus', 'whileDrag', 'whileInView',
    'layout', 'layoutId', 'layoutScroll', 'layoutDependency', 'viewport',
    'drag', 'dragConstraints', 'dragElastic', 'dragMomentum', 'dragControls', 'dragListener',
    'onAnimationStart', 'onAnimationComplete', 'onHoverStart', 'onHoverEnd',
    'onTap', 'onTapStart', 'onTapCancel', 'onDrag', 'onDragStart', 'onDragEnd',
    'onViewportEnter', 'onViewportLeave',
  ]);
  const strip = (props) => {
    const out = {};
    for (const k in props) if (!MOTION_ONLY.has(k)) out[k] = props[k];
    return out;
  };
  const motion = new Proxy(
    {},
    {
      get: (_t, tag) =>
        typeof tag === 'string'
          ? React.forwardRef((props, ref) => React.createElement(tag, { ...strip(props), ref }))
          : undefined,
    },
  );
  return {
    __esModule: true,
    motion,
    AnimatePresence: ({ children }) => React.createElement(React.Fragment, null, children),
    LayoutGroup: ({ children }) => React.createElement(React.Fragment, null, children),
    useMotionValue: (v) => ({ get: () => v, set: () => {}, on: () => () => {} }),
    useTransform: () => ({ get: () => 0 }),
    useSpring: (v) => v,
    useAnimation: () => ({ start: () => {}, stop: () => {}, set: () => {} }),
    useReducedMotion: () => true,
    useInView: () => false,
  };
});
