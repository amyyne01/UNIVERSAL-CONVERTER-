// The launch update prompt (§12).
//
// The self-updater has always worked; nothing ever TOLD anyone about it. The only
// surface was a row three sections down the Settings tab, so an install could sit
// versions behind indefinitely while the user had no reason to go looking. This is
// that check, brought to the front once per version.
//
// Deliberately not a gate: it appears only when a newer release actually exists,
// Esc and "Later" both dismiss it, and the dismissal is remembered PER VERSION —
// so declining 1.2.2 stays declined across launches, while 1.2.3 asks once more.
import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { motion, AnimatePresence, useReducedMotion } from 'framer-motion';
import { Close, Download, Power, Premium, Alert } from '@/components/ui/icons';
import { Button } from '@/components/ui/Button';
import { ProgressBar } from '@/components/ui/ProgressBar';
import type { UpdateState } from '@shared/types';

// One key, holding the last version the user waved off. localStorage is the right
// home: it is a UI preference, it must survive a restart, and it is worthless to
// anything in main — config.ts stays the store for things the ENGINE reads.
const DISMISSED_KEY = 'ahg-update-dismissed';

const readDismissed = (): string => {
  try { return localStorage.getItem(DISMISSED_KEY) ?? ''; } catch { return ''; }
};
const writeDismissed = (version: string): void => {
  try { localStorage.setItem(DISMISSED_KEY, version); } catch { /* private mode — nag again, don't crash */ }
};

export function UpdateModal() {
  const reduced = useReducedMotion();
  const [state, setState] = useState<UpdateState>({ phase: 'idle' });
  const [dismissed, setDismissed] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    // Every phase change arrives as an event, but the startup check fires on a 6s
    // timer in main and this component mounts before that — and if the check has
    // ALREADY landed (a slow first paint), its event is gone. Read the state main
    // kept as well as subscribing, the same belt-and-braces the Settings tab uses.
    const merge = (patch: Partial<UpdateState>) => setState((s) => ({ ...s, ...patch }));
    const unsubs = [
      window.electronAPI.onUpdateAvailable(({ version }) => merge({ phase: 'available', version })),
      window.electronAPI.onUpdateNotAvailable(() => merge({ phase: 'uptodate' })),
      window.electronAPI.onUpdateProgress(({ percent }) => merge({ phase: 'downloading', percent })),
      window.electronAPI.onUpdateDownloaded(({ version }) => merge({ phase: 'ready', version })),
      window.electronAPI.onUpdateError(({ message }) => merge({ phase: 'error', message })),
    ];
    void window.electronAPI.update.state().then((s) => {
      if (s.phase !== 'idle') setState(s);
    }).catch(() => { /* main not ready — the events still cover it */ });
    return () => unsubs.forEach((u) => u());
  }, []);

  const version = state.version ?? '';
  // 'ready' is included on purpose: an update downloaded in a previous session is
  // sitting on disk doing nothing until someone restarts, and that is worth saying.
  const wanted = state.phase === 'available' || state.phase === 'downloading' || state.phase === 'ready';
  const open = wanted && !dismissed && !(version && readDismissed() === version);

  const close = useCallback(() => {
    setDismissed(true);
    // Only a version the user actually SAW gets recorded, and only from a state
    // where declining is meaningful — dismissing mid-download must not suppress
    // the restart prompt that follows it.
    if (version && state.phase === 'available') writeDismissed(version);
  }, [version, state.phase]);

  // Focus the dialog when it opens and hand focus back to whatever had it after.
  useEffect(() => {
    if (!open) return;
    openerRef.current = document.activeElement as HTMLElement | null;
    dialogRef.current?.focus();
    return () => openerRef.current?.focus?.();
  }, [open]);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      close();
      return;
    }
    if (e.key !== 'Tab') return;
    // Trap Tab inside the dialog — the app behind it is inert while this is up.
    const focusables = e.currentTarget.querySelectorAll<HTMLElement>('button:not([disabled])');
    if (!focusables.length) return;
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  const downloading = state.phase === 'downloading';
  const ready = state.phase === 'ready';

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          key="update-modal"
          className="fixed inset-0 z-(--z-modal) flex items-center justify-center p-4 no-drag"
          style={{ background: 'var(--color-scrim)', backdropFilter: 'blur(10px) saturate(1.1)' }}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.2 }}
          onMouseDown={(e) => {
            // Clicking away is a dismissal, but not while bytes are moving — that
            // reads as "cancel", which is not what it would do.
            if (e.target === e.currentTarget && !downloading) close();
          }}
        >
          <motion.div
            ref={dialogRef}
            role="dialog"
            aria-modal="true"
            aria-labelledby="update-modal-title"
            tabIndex={-1}
            onKeyDown={onKeyDown}
            initial={reduced ? { opacity: 0 } : { opacity: 0, y: 16, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={reduced ? { opacity: 0 } : { opacity: 0, y: 8, scale: 0.99 }}
            transition={{ duration: 0.32, ease: [0.22, 1, 0.36, 1] }}
            className="relative w-full max-w-[460px] rounded-xl border border-border"
            style={{
              background: 'var(--color-bg-secondary)',
              boxShadow: 'var(--shadow-lg)',
              padding: 'clamp(20px, 3vw, 30px)',
              // Inline, not `outline-none`. index.css carries a BARE `:focus-visible
              // { outline: 2px solid var(--color-accent) }`, and unlayered CSS beats
              // every Tailwind layer — so the utility loses and the dialog we focus
              // on open draws a full accent ring around itself. Same cascade trap as
              // ProgressBar's relative/absolute. The panel is the focus target only
              // so the keyboard trap has somewhere to start; it is not interactive,
              // and nothing inside it loses its own ring.
              outline: 'none',
            }}
          >
            <button
              type="button"
              onClick={close}
              aria-label="Close"
              disabled={downloading}
              className="absolute right-4 top-4 grid place-items-center rounded-md text-text-muted hover:text-text-primary hover:bg-bg-hover transition-colors disabled:opacity-40"
              style={{ width: 32, height: 32 }}
            >
              <Close size={16} />
            </button>

            <div
              className="grid place-items-center rounded-lg mb-4"
              style={{ width: 40, height: 40, background: 'var(--color-bg-hover)' }}
            >
              <Premium size={20} className="text-accent" />
            </div>

            <h2 id="update-modal-title" className="text-h3 text-text-primary text-balance pr-8">
              {ready ? 'Update ready to install' : downloading ? 'Downloading the update…' : 'A new version is available'}
            </h2>
            <p className="mt-2 text-body-sm text-text-secondary text-pretty">
              {ready
                ? <>Version <span className="font-mono text-text-primary">{version}</span> is downloaded. Restarting takes a few seconds and puts you on it.</>
                : downloading
                  ? <>Version <span className="font-mono text-text-primary">{version}</span> is on its way. You can keep using the app — this finishes in the background.</>
                  : <>Version <span className="font-mono text-text-primary">{version}</span> is out. Downloading it now means the next launch is already up to date.</>}
            </p>

            {downloading && (
              <div className="mt-5 flex items-center gap-3">
                <ProgressBar
                  percent={state.percent ?? 0}
                  status="downloading"
                  label="Update download"
                  className="flex-1"
                />
                <span className="font-mono text-[11px] text-text-secondary w-9 text-right">
                  {(state.percent ?? 0).toFixed(0)}%
                </span>
              </div>
            )}

            {state.message && (
              <p className="mt-4 flex items-start gap-2 text-body-sm text-text-muted">
                <Alert size={15} className="mt-0.5 shrink-0" />
                <span>{state.message}</span>
              </p>
            )}

            <div className="mt-6 flex items-center justify-end gap-2">
              <Button variant="ghost" size="sm" onClick={close} disabled={downloading}>
                {ready ? 'Not now' : 'Later'}
              </Button>
              {ready ? (
                <Button
                  variant="primary"
                  size="sm"
                  icon={Power}
                  onClick={() => void window.electronAPI.update.install()}
                >
                  Restart &amp; install
                </Button>
              ) : (
                <Button
                  variant="primary"
                  size="sm"
                  icon={Download}
                  loading={downloading}
                  disabled={downloading}
                  onClick={() => void window.electronAPI.update.download()}
                >
                  Download
                </Button>
              )}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
