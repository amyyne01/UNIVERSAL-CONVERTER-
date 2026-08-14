import { useEffect, type CSSProperties } from 'react';
import { MotionConfig, motion, useReducedMotion } from 'framer-motion';
import { useAppStore } from '@/store';
import { initTheme } from '@/lib/theme';
import { dismissSplash, splashStep } from '@/lib/splash';
import { NAV, PLATFORMS, TAB_ORDER, type NavItem, type TabKey } from '@/constants';
import { IconContext } from '@/components/ui/icons';
import WindowTitleBar from '@/components/WindowTitleBar';
import { CommandPalette } from '@/components/CommandPalette';
import { UpgradeSheet } from '@/components/UpgradeSheet';
import { Notice } from '@/components/Notice';
import { PlaylistPreview } from '@/components/PlaylistPreview';
import { Dashboard } from '@/components/tabs/Dashboard';
import { PlatformTab } from '@/components/tabs/PlatformTab';
import { DownloadsTab } from '@/components/tabs/DownloadsTab';
import { SettingsTab } from '@/components/tabs/SettingsTab';

/** Ceiling on how long the splash may hold the UI if a boot call never settles. */
const SPLASH_TIMEOUT_MS = 5000;

// App-wide icon weight — 'light' reads thinner and more refined than Phosphor's
// default 'regular', matching the glass/OKLCH premium aesthetic. One switch here
// re-tunes every icon in the app; call sites never set weight individually.
// One weight for the whole icon set. 'regular' rather than 'light': these render
// at 13–19px on a near-black canvas, where a lighter stroke stops holding its
// shape and drops below the contrast the rest of the UI is held to. The platform
// logos opt out to 'fill' in icons.ts — they are marks, not glyphs.
const ICON_CONTEXT = { weight: 'regular' as const };

export default function App() {
  return (
    <MotionConfig reducedMotion="user">
      <IconContext.Provider value={ICON_CONTEXT}>
        <Shell />
      </IconContext.Provider>
    </MotionConfig>
  );
}

function Shell() {
  const setLicense = useAppStore((s) => s.setLicense);
  const upgradeOpen = useAppStore((s) => s.upgradeOpen);
  const setUpgradeOpen = useAppStore((s) => s.setUpgradeOpen);
  const setConfig = useAppStore((s) => s.setConfig);
  const setWindowMaximized = useAppStore((s) => s.setWindowMaximized);
  const activeTab = useAppStore((s) => s.activeTab);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const currentPlaylist = useAppStore((s) => s.currentPlaylist);
  const setCurrentPlaylist = useAppStore((s) => s.setCurrentPlaylist);
  const activeCount = useAppStore(
    (s) =>
      Object.values(s.downloads).filter((t) => {
        const st = t.progress.status;
        return st !== 'done' && st !== 'failed' && st !== 'cancelled';
      }).length,
  );

  // Bootstrap: theme, config, license, window state, IPC subscriptions.
  useEffect(() => {
    initTheme();
    // A permanently-null config soft-locks the UI, so retry once on failure.
    const loadConfig = () =>
      window.electronAPI.config.getAll().then(setConfig).catch(() => {
        setTimeout(() => void window.electronAPI.config.getAll().then(setConfig).catch(() => {}), 1500);
      });
    // The splash waits on config + history only. The tier check can hit the
    // network, so it is never on the path that unblocks the UI — an offline
    // launch must not stall behind it.
    const configReady = loadConfig().then(splashStep);
    void window.electronAPI.license
      .check()
      .then((r) => setLicense({ activated: r.activated, plan: r.plan }))
      .catch(() => {});
    void window.electronAPI.window.isMaximized().then(setWindowMaximized).catch(() => {});

    const get = useAppStore.getState;
    // Subscribe BEFORE hydrating so live events during load aren't dropped; the
    // history merge below keeps live events (they override the persisted snapshot).
    const unsubs = [
      window.electronAPI.onDownloadQueued((_id, task) => get().addDownload(task)),
      window.electronAPI.onDownloadProgress((id, p) => get().updateDownloadProgress(id, p)),
      window.electronAPI.onDownloadDone((id, p) => get().updateDownloadProgress(id, p)),
      window.electronAPI.onDownloadError((id, error) => {
        const t = get().downloads[id];
        if (t) get().updateDownloadProgress(id, { ...t.progress, status: 'failed', error });
      }),
      window.electronAPI.onDownloadCancelled((id) => {
        const t = get().downloads[id];
        if (t) get().updateDownloadProgress(id, { ...t.progress, status: 'cancelled' });
      }),
      window.electronAPI.onWindowMaximized(setWindowMaximized),
      window.electronAPI.onLicenseStateChanged((d) => get().setLicense(d)),
    ];

    // Hydrate persisted download history, merging live events on top (last wins).
    const historyReady = window.electronAPI.download
      .list()
      .then((tasks) => get().setDownloads([...tasks, ...Object.values(get().downloads)]))
      .catch(() => {})
      .then(splashStep);

    // Whichever comes first: both boot steps landing, or the watchdog. A hung
    // IPC call must never leave the user staring at the splash forever.
    const watchdog = setTimeout(dismissSplash, SPLASH_TIMEOUT_MS);
    void Promise.all([configReady, historyReady]).finally(() => {
      clearTimeout(watchdog);
      dismissSplash();
    });

    return () => {
      clearTimeout(watchdog);
      unsubs.forEach((u) => u());
    };
  }, [setLicense, setConfig, setWindowMaximized]);

  // Keyboard: Ctrl+1..n tab switch. (Ctrl+K is owned by CommandPalette.)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey) return;
      if (useAppStore.getState().commandPaletteOpen) return; // palette owns keys while open
      const n = Number(e.key);
      if (Number.isInteger(n) && n >= 1 && n <= TAB_ORDER.length) {
        e.preventDefault();
        setCurrentPlaylist(null);
        setActiveTab(TAB_ORDER[n - 1]);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [setActiveTab, setCurrentPlaylist]);

  const platform = PLATFORMS.find((p) => p.key === activeTab);
  const contentStyle = platform
    ? ({ '--color-accent': `var(--color-${platform.accent})` } as CSSProperties)
    : undefined;

  return (
    <div className="flex flex-col h-screen bg-bg-primary text-text-primary">
      <a
        href="#main-content"
        className="no-drag sr-only focus:not-sr-only focus:absolute focus:z-50 focus:left-3 focus:top-3 focus:px-3 focus:py-2 focus:rounded-md focus:bg-bg-surface focus:text-text-primary focus:border focus:border-accent"
      >
        Skip to content
      </a>
      <WindowTitleBar />
      <div className="flex-1 grid grid-cols-[68px_1fr] min-h-0">
        <Rail
          activeTab={activeTab}
          activeCount={activeCount}
          onSelect={(t) => {
            setCurrentPlaylist(null);
            setActiveTab(t);
          }}
        />
        {/* Content column — contentStyle swaps --color-accent per platform, so the
            carrier line, focus rings, buttons and soft tints all inherit it. */}
        <div className="flex flex-col min-h-0" style={contentStyle}>
          <CarrierLine active={activeCount > 0} />
          {/* `@container` makes this column — not the viewport — the unit every
              tab measures against. The rail is a fixed 68px and the scrollbar
              takes its own slice, so the viewport always over-reports the space
              a tab actually has; a tab that reflows on `md:` reflows a rail-width
              too early. Tabs cap and reflow with `@min-[…]:` off this element. */}
          <main id="main-content" tabIndex={-1} className="@container flex-1 overflow-y-auto">
            {currentPlaylist ? <PlaylistPreview /> : <Screen tab={activeTab} />}
          </main>
        </div>
      </div>
      <CommandPalette />
      {/* Upgrade sheet — opened from Settings or any locked control. Nothing
          blocks launch any more: no key simply means the basic tier. */}
      {upgradeOpen && <UpgradeSheet onClose={() => setUpgradeOpen(false)} />}
      <Notice />
    </div>
  );
}

// The signature ambient moment — a 1px phosphor "carrier line" under the title
// bar, the app's only glow. It inherits the per-platform --color-accent swap and
// shimmers a slow 2.4s sweep while any download is live.
function CarrierLine({ active }: { active: boolean }) {
  const reduce = useReducedMotion();
  const shimmer = active && !reduce;
  return (
    <motion.div
      aria-hidden
      className="h-px shrink-0"
      style={{
        background: 'linear-gradient(90deg, transparent, var(--color-accent), transparent)',
        backgroundSize: shimmer ? '250% 100%' : '100% 100%',
        boxShadow: 'var(--shadow-glow)',
        opacity: active ? 0.85 : 0.35,
      }}
      animate={shimmer ? { backgroundPositionX: ['-125%', '125%'] } : { backgroundPositionX: '0%' }}
      transition={shimmer ? { duration: 2.4, ease: 'linear', repeat: Infinity } : { duration: 0 }}
    />
  );
}

function Screen({ tab }: { tab: TabKey }) {
  switch (tab) {
    case 'home':
      return <Dashboard />;
    case 'queue':
      return <DownloadsTab />;
    case 'settings':
      return <SettingsTab />;
    case 'youtube':
    case 'spotify':
    case 'soundcloud':
    case 'reels':
      return <PlatformTab platform={tab} />;
    default:
      return <Dashboard />;
  }
}

// 68px icon rail — replaces the 260px sidebar. The hairline divider + pinned
// Settings carry the taxonomy, so the group headers are gone.
function Rail({
  activeTab,
  activeCount,
  onSelect,
}: {
  activeTab: TabKey;
  activeCount: number;
  onSelect: (t: TabKey) => void;
}) {
  const lib = NAV.filter((n) => n.group === 'Library');
  const sources = NAV.filter((n) => n.group === 'Sources');
  const system = NAV.filter((n) => n.group === 'System');
  const item = (n: NavItem) => (
    <RailItem
      key={n.key}
      nav={n}
      active={activeTab === n.key}
      badge={n.key === 'queue' ? activeCount : 0}
      onSelect={onSelect}
    />
  );
  return (
    <aside className="flex flex-col items-center gap-1 py-3 bg-bg-secondary border-r border-border-soft overflow-y-auto">
      {lib.map(item)}
      <div className="w-8 h-px my-1.5 shrink-0 bg-border-soft" />
      {sources.map(item)}
      <div className="flex-1" />
      {system.map(item)}
    </aside>
  );
}

function RailItem({
  nav,
  active,
  badge,
  onSelect,
}: {
  nav: NavItem;
  active: boolean;
  badge: number;
  onSelect: (t: TabKey) => void;
}) {
  // shrink-0 below: the rail is a scrolling flex column, so at the 640px minimum
  // window height eight items would squash themselves flat rather than scroll.
  const Icon = nav.icon;
  const platform = PLATFORMS.find((p) => p.key === nav.key);
  const dot = platform ? `var(--color-${platform.accent})` : undefined;
  return (
    <button
      onClick={() => onSelect(nav.key)}
      aria-label={nav.label}
      aria-current={active ? 'page' : undefined}
      title={nav.label}
      className={`no-drag relative grid place-items-center w-11 h-11 shrink-0 rounded-lg transition-colors ${
        active
          ? 'bg-bg-surface text-text-primary'
          : 'text-text-muted hover:text-text-primary hover:bg-bg-hover'
      }`}
    >
      {active && (
        <motion.span
          layoutId="rail-active"
          className="absolute -left-3 top-2 bottom-2 w-[3px] rounded-r-full"
          style={{ background: dot ?? 'var(--color-accent)' }}
        />
      )}
      <Icon size={20} />
      {platform && (
        <span
          className="absolute bottom-1.5 right-1.5 w-1.5 h-1.5 rounded-full"
          style={{ background: dot }}
        />
      )}
      {badge > 0 && (
        <span className="absolute -top-0.5 -right-0.5 grid place-items-center min-w-[15px] h-[15px] px-1 rounded-full bg-accent text-on-accent font-mono text-[9px] leading-none tabular-nums">
          {badge}
        </span>
      )}
    </button>
  );
}
