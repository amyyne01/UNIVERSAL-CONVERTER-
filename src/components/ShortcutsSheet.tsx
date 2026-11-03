// Keyboard cheat sheet: one place that lists every shortcut, reachable from
// Ctrl+K and Settings. Rows are data so the list cannot drift from itself.
import { useEffect, useRef, type KeyboardEvent } from 'react';
import { Close } from '@/components/ui/icons';
import { IconButton } from '@/components/ui';
import { TAB_ORDER } from '@/constants';
import { useAppStore } from '@/store';

const ROWS: readonly { keys: string[]; what: string }[] = [
  { keys: ['Ctrl', 'K'], what: 'Open the command palette' },
  { keys: ['Ctrl', `1…${TAB_ORDER.length}`], what: 'Switch tab (the order of the left rail)' },
  { keys: ['Ctrl', 'V'], what: 'Paste a link from Home and look it up' },
  { keys: ['Enter'], what: 'Look up the link or search in the field' },
  { keys: ['↑', '↓'], what: 'Move between results' },
  { keys: ['Space'], what: 'Select a result; Enter downloads it' },
  { keys: ['Ctrl', 'A'], what: 'Select every result' },
  { keys: ['Esc'], what: 'Close a sheet or the palette' },
  { keys: ['Ctrl', '/'], what: 'Show this list' },
];

export function ShortcutsSheet() {
  const open = useAppStore((s) => s.shortcutsOpen);
  const setOpen = useAppStore((s) => s.setShortcutsOpen);
  const hotkey = useAppStore((s) => s.config?.globalHotkey);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const opener = document.activeElement as HTMLElement | null;
    ref.current?.focus();
    return () => opener?.focus?.();
  }, [open]);

  if (!open) return null;
  const close = () => setOpen(false);
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') { e.stopPropagation(); close(); }
    // The close button is the only control, so Tab stays inside the sheet and lands on it.
    if (e.key === 'Tab') { e.preventDefault(); ref.current?.querySelector('button')?.focus(); }
  };
  const rows = hotkey ? [...ROWS, { keys: hotkey.replace('CommandOrControl', 'Ctrl').split('+'), what: 'Bring this window to the front from anywhere' }] : ROWS;

  return (
    <div
      className="fixed inset-0 z-(--z-modal) flex items-center justify-center p-4 no-drag"
      style={{ background: 'var(--color-scrim)' }}
      onMouseDown={(e) => { if (e.target === e.currentTarget) close(); }}
    >
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-label="Keyboard shortcuts"
        tabIndex={-1}
        onKeyDown={onKeyDown}
        className="relative w-full max-w-[460px] max-h-[90vh] overflow-y-auto rounded-xl border border-border bg-bg-secondary p-6 outline-none"
        style={{ boxShadow: 'var(--shadow-lg)' }}
      >
        <IconButton icon={Close} label="Close" onClick={close} className="absolute right-4 top-4" />
        <h2 className="text-h2 text-text-primary">Keyboard shortcuts</h2>
        <ul className="mt-4 divide-y divide-border-soft">
          {rows.map((r) => (
            <li key={r.what} className="flex items-center justify-between gap-6 py-3">
              <span className="text-sm text-text-secondary">{r.what}</span>
              <span className="flex shrink-0 items-center gap-1">
                {r.keys.map((k) => <kbd key={k} className="keycap">{k}</kbd>)}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
