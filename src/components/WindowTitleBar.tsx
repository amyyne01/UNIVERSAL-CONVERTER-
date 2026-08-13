import { Minus, Square, Copy, X, Search } from 'lucide-react';
import { useAppStore } from '@/store';
// 64px variant — the full 4000px icon.png is 2.5 MB and ships for tray/installer only.
import iconUrl from '../../assets/icon-64.png';

export default function WindowTitleBar() {
  const isMax = useAppStore((s) => s.isWindowMaximized);
  const toggleCommandPalette = useAppStore((s) => s.toggleCommandPalette);

  // Fire-and-forget: the actual maximized state is pushed back via the
  // window:maximized event (subscribed in App), so it stays correct after
  // native maximize/restore/snap too — no optimistic flip here.
  const onMaximize = () => void window.electronAPI.window.maximize();

  const ctrl =
    'grid place-items-center w-11 h-11 text-text-muted hover:text-text-primary hover:bg-bg-hover transition-colors';
  const kbd =
    'font-mono text-[9.5px] leading-none tracking-[0.08em] px-1 py-0.5 rounded bg-bg-secondary border border-border-soft text-text-muted';

  return (
    <header className="drag-region relative flex items-center h-11 pl-4 pr-1 bg-bg-secondary border-b border-border-soft select-none">
      {/* Left — rack-unit wordmark (Micro-mono, instrument label not window caption) */}
      <div className="flex items-center gap-2.5">
        <img src={iconUrl} alt="" className="w-4 h-4 rounded-[5px]" draggable={false} />
        <span className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted">
          <b className="text-text-primary font-semibold">AHG</b> Universal Converter
        </span>
      </div>

      {/* Center — command capsule (replaces the old TopBar), absolute-centered so
          the flanking space stays draggable */}
      <button
        onClick={toggleCommandPalette}
        aria-label="Search or run a command"
        className="no-drag absolute left-1/2 -translate-x-1/2 flex items-center gap-2 w-[420px] max-w-[38vw] h-7 px-2.5 rounded-md bg-bg-tertiary border border-border-soft text-text-muted hover:border-accent transition-colors"
      >
        <Search size={13} />
        <span className="flex-1 text-left text-[12px] truncate">Search or run…</span>
        <span className="flex items-center gap-1">
          <kbd className={kbd}>Ctrl</kbd>
          <kbd className={kbd}>K</kbd>
        </span>
      </button>

      <div className="flex-1" />

      {/* Right — window controls */}
      <div className="no-drag flex">
        <button onClick={() => window.electronAPI.window.minimize()} aria-label="Minimize" className={ctrl}>
          <Minus size={15} />
        </button>
        <button onClick={onMaximize} aria-label={isMax ? 'Restore' : 'Maximize'} className={ctrl}>
          {isMax ? <Copy size={12} /> : <Square size={12} />}
        </button>
        <button
          onClick={() => window.electronAPI.window.close()}
          aria-label="Close"
          className="grid place-items-center w-11 h-11 text-text-muted hover:text-text-primary hover:bg-error transition-colors"
        >
          <X size={15} />
        </button>
      </div>
    </header>
  );
}
