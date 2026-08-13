import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import {
  Clipboard,
  Moon,
  RefreshCw,
  Search,
  Sun,
  Trash2,
  XCircle,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useAppStore } from '@/store';
import { NAV, TAB_ORDER } from '@/constants';
import { buildDownloadRequest } from '@/lib/download';

// ─── types ───────────────────────────────────────────────────────────────────

interface Cmd {
  id: string;
  label: string;
  icon: LucideIcon;
  group: string;
  hint?: string;
  action: () => void | Promise<void>;
}

// ─── tiny keyboard badge (used in input row, command hints, footer) ───────────

function Kbd({ children }: { children: string }) {
  return (
    <span className="font-mono text-[10px] text-text-muted border border-border rounded px-1.5 py-0.5 leading-none shrink-0">
      {children}
    </span>
  );
}

// ─── animation ───────────────────────────────────────────────────────────────

// ease-out-expo from index.css
const EASE: [number, number, number, number] = [0.16, 1, 0.3, 1];

// ─── component ───────────────────────────────────────────────────────────────

export function CommandPalette() {
  const isOpen        = useAppStore((s) => s.commandPaletteOpen);
  const setOpen       = useAppStore((s) => s.setCommandPaletteOpen);
  const togglePalette = useAppStore((s) => s.toggleCommandPalette);
  const setActiveTab  = useAppStore((s) => s.setActiveTab);
  const clearCompletedAndPersist = useAppStore((s) => s.clearCompletedAndPersist);
  const updateConfig  = useAppStore((s) => s.updateConfig);
  const config        = useAppStore((s) => s.config);

  const [query, setQuery]           = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef  = useRef<HTMLDivElement>(null);

  const close = useCallback(() => setOpen(false), [setOpen]);

  // ── command definitions ───────────────────────────────────────────────────

  const commands = useMemo<Cmd[]>(
    () => [
      // Navigate — one entry per tab, Ctrl+N hint derived from TAB_ORDER
      ...NAV.map((n) => ({
        id: `nav-${n.key}`,
        label: `Go to ${n.label}`,
        icon: n.icon,
        group: 'Navigate',
        hint: `Ctrl+${TAB_ORDER.indexOf(n.key) + 1}`,
        action: () => {
          setActiveTab(n.key);
          setOpen(false);
        },
      })),

      // Paste & download — clipboard → url:detect → download:start
      {
        id: 'paste-download',
        label: 'Paste link & download',
        icon: Clipboard,
        group: 'Actions',
        hint: 'Ctrl+V',
        action: async () => {
          setOpen(false);
          const text = await navigator.clipboard.readText().catch(() => '');
          const url = text.trim();
          if (!url) return;
          const detection = await window.electronAPI.url.detect(url);
          if (detection.platform === 'unknown') return;
          const cfg = useAppStore.getState().config;
          // Audio only for music platforms — a video link stays a video download.
          const audioOnly = detection.platform === 'spotify' || detection.platform === 'soundcloud';
          void window.electronAPI.download.start(
            buildDownloadRequest(cfg, {
              url: detection.url,
              source: detection.platform,
              format: audioOnly ? (cfg?.defaultFormat ?? 'mp3') : 'mp4',
              isAudioOnly: audioOnly,
              isPlaylist: detection.isCollection,
            }),
          );
        },
      },

      // Toggle theme (light ↔ dark)
      {
        id: 'toggle-theme',
        label: config?.theme === 'light' ? 'Switch to dark mode' : 'Switch to light mode',
        icon: config?.theme === 'light' ? Moon : Sun,
        group: 'Actions',
        action: () => {
          updateConfig({ theme: config?.theme === 'light' ? 'dark' : 'light' });
          setOpen(false);
        },
      },

      {
        id: 'check-updates',
        label: 'Check for updates',
        icon: RefreshCw,
        group: 'Actions',
        action: () => {
          setOpen(false);
          void window.electronAPI.update.check();
        },
      },

      {
        id: 'clear-completed',
        label: 'Clear completed downloads',
        icon: Trash2,
        group: 'Actions',
        action: () => {
          clearCompletedAndPersist();
          setOpen(false);
        },
      },

      {
        id: 'cancel-all',
        label: 'Cancel all downloads',
        icon: XCircle,
        group: 'Actions',
        action: () => {
          setOpen(false);
          void window.electronAPI.download.cancelAll();
        },
      },
    ],
    [config, setActiveTab, setOpen, clearCompletedAndPersist, updateConfig],
  );

  // ── filtering + grouping ──────────────────────────────────────────────────

  const visibleCmds = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter(
      (c) => c.label.toLowerCase().includes(q) || c.group.toLowerCase().includes(q),
    );
  }, [commands, query]);

  const groupedCmds = useMemo(() => {
    const map = new Map<string, Cmd[]>();
    for (const cmd of visibleCmds) {
      const arr = map.get(cmd.group) ?? [];
      arr.push(cmd);
      map.set(cmd.group, arr);
    }
    return map;
  }, [visibleCmds]);

  // cmd.id → flat index in visibleCmds (for activeIndex comparison)
  const cmdFlatIdx = useMemo(() => {
    const m = new Map<string, number>();
    visibleCmds.forEach((c, i) => m.set(c.id, i));
    return m;
  }, [visibleCmds]);

  // ── side effects ──────────────────────────────────────────────────────────

  // Reset active when filtered list changes
  useEffect(() => { setActiveIndex(0); }, [visibleCmds]);

  // Focus input + reset state on open
  useEffect(() => {
    if (!isOpen) return;
    setQuery('');
    setActiveIndex(0);
    const t = setTimeout(() => inputRef.current?.focus(), 10);
    return () => clearTimeout(t);
  }, [isOpen]);

  // Scroll active item into view
  useEffect(() => {
    if (!isOpen) return;
    listRef.current
      ?.querySelector<HTMLElement>(`[data-cmd-idx="${activeIndex}"]`)
      ?.scrollIntoView({ block: 'nearest' });
  }, [activeIndex, isOpen]);

  // Ctrl+K global toggle
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        togglePalette();
      }
    };
    document.addEventListener('keydown', h);
    return () => document.removeEventListener('keydown', h);
  }, [togglePalette]);

  // In-palette keyboard navigation (esc / arrows / enter)
  useEffect(() => {
    if (!isOpen) return;
    const h = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { close(); return; }
      if (e.key === 'Tab') { e.preventDefault(); return; } // single focusable field — trap focus in the panel
      if (e.ctrlKey && e.key.toLowerCase() === 'v') {
        e.preventDefault();
        void commands.find((c) => c.id === 'paste-download')?.action();
        return;
      }
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setActiveIndex((i) => Math.min(i + 1, Math.max(0, visibleCmds.length - 1)));
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setActiveIndex((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === 'Enter') {
        e.preventDefault();
        void visibleCmds[activeIndex]?.action();
      }
    };
    document.addEventListener('keydown', h);
    return () => document.removeEventListener('keydown', h);
  }, [isOpen, visibleCmds, activeIndex, close, commands]);

  // ── render ────────────────────────────────────────────────────────────────

  return (
    <AnimatePresence>
      {isOpen && (
        <>
          {/* Backdrop */}
          <motion.div
            key="pal-scrim"
            className="fixed inset-0 z-(--z-palette) bg-bg-primary/60 backdrop-blur-sm"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.16 }}
            onClick={close}
          />

          {/* Panel */}
          <motion.div
            key="pal-panel"
            role="dialog"
            aria-modal="true"
            aria-label="Command palette"
            className="no-drag fixed left-1/2 top-24 z-(--z-palette) -translate-x-1/2"
            style={{ width: 'min(560px, 92vw)' }}
            initial={{ opacity: 0, y: -10, scale: 0.97 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -10, scale: 0.97 }}
            transition={{ duration: 0.18, ease: EASE }}
          >
            <div className="overflow-hidden rounded-xl border border-border bg-bg-glass backdrop-blur-xl shadow-lg">

              {/* ── search row ── */}
              <div className="flex items-center gap-3 border-b border-border px-[18px] py-4">
                <span
                  className="font-mono text-[15px] text-accent leading-none select-none"
                  aria-hidden
                >
                  ›
                </span>
                <input
                  ref={inputRef}
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder="Type a command or paste a link…"
                  spellCheck={false}
                  aria-label="Command palette search"
                  role="combobox"
                  aria-expanded="true"
                  aria-controls="palette-listbox"
                  aria-activedescendant={visibleCmds.length ? `cmd-opt-${activeIndex}` : undefined}
                  className="flex-1 min-w-0 bg-transparent text-sm text-text-primary placeholder:text-text-muted outline-none font-mono"
                />
                {!query && (
                  <Search size={14} className="text-text-muted shrink-0" aria-hidden />
                )}
                <Kbd>ESC</Kbd>
              </div>

              {/* ── command list ── */}
              <div ref={listRef} id="palette-listbox" role="listbox" className="max-h-[320px] overflow-y-auto py-1.5">
                {visibleCmds.length === 0 ? (
                  <p className="px-[18px] py-7 text-center text-sm text-text-muted">
                    No commands match
                  </p>
                ) : (
                  Array.from(groupedCmds.entries()).map(([group, items]) => (
                    <div key={group} role="group" aria-label={group}>
                      {/* section label mirrors concept-c // SECTION style */}
                      <p
                        aria-hidden
                        className="px-[18px] pb-1.5 pt-3 font-mono text-[9.5px] uppercase tracking-[0.14em] text-text-muted select-none"
                      >
                        // {group}
                      </p>

                      {items.map((cmd) => {
                        const flatIdx = cmdFlatIdx.get(cmd.id) ?? 0;
                        const active  = flatIdx === activeIndex;
                        const Icon    = cmd.icon;
                        return (
                          <button
                            key={cmd.id}
                            type="button"
                            role="option"
                            id={`cmd-opt-${flatIdx}`}
                            aria-selected={active}
                            data-cmd-idx={flatIdx}
                            onClick={() => void cmd.action()}
                            onMouseEnter={() => setActiveIndex(flatIdx)}
                            className={[
                              'relative flex w-full items-center gap-3 px-[18px] py-[9px] text-[12.5px] transition-colors duration-75 text-left',
                              active
                                ? 'bg-accent-soft text-text-primary before:absolute before:left-0 before:inset-y-1.5 before:w-0.5 before:rounded-r before:bg-accent'
                                : 'text-text-secondary hover:bg-bg-hover hover:text-text-primary',
                            ].join(' ')}
                          >
                            <Icon
                              size={15}
                              className={[
                                'shrink-0 transition-colors duration-75',
                                active ? 'text-accent' : 'text-text-muted',
                              ].join(' ')}
                              aria-hidden
                            />
                            <span className="flex-1">{cmd.label}</span>
                            {cmd.hint && (
                              <span className="ml-auto">
                                <Kbd>{cmd.hint}</Kbd>
                              </span>
                            )}
                          </button>
                        );
                      })}
                    </div>
                  ))
                )}
              </div>

              {/* ── footer ── */}
              <div className="flex items-center gap-4 border-t border-border bg-bg-tertiary px-[18px] py-2.5">
                <span className="flex items-center gap-1.5 font-mono text-[10px] text-text-muted">
                  <Kbd>↑↓</Kbd> navigate
                </span>
                <span className="flex items-center gap-1.5 font-mono text-[10px] text-text-muted">
                  <Kbd>↵</Kbd> select
                </span>
                <span className="flex items-center gap-1.5 font-mono text-[10px] text-text-muted">
                  <Kbd>esc</Kbd> close
                </span>
              </div>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
