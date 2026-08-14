// The upgrade sheet — the app's one commercial surface. Nothing blocks launch:
// an install with no key is a working basic install, and this sheet explains what
// the one-time purchase adds and takes the key that unlocks it.
//
// The two cards are deliberately NOT twins: basic is a flat, quiet statement of
// what you already have; premium carries the border, the badge and the price.
// Every row is generated from PLAN_FEATURES, whose numbers come from BASIC_LIMITS
// — the same constant the main process clamps against.
import { useState, useRef, useEffect, useCallback, type KeyboardEvent } from 'react';
import { motion, AnimatePresence, useReducedMotion } from 'framer-motion';
import {
  Close, Check, LicenseKey, Paste, Alert, Support, Premium, ChevronDown,
} from '@/components/ui/icons';
import { Button } from '@/components/ui/Button';
import { PLAN_FEATURES } from '@/constants';
import { useAppStore } from '@/store';

const PRICE = '$3.99';

type KeyState = 'idle' | 'loading' | 'error' | 'success';

export interface UpgradeSheetProps {
  onClose: () => void;
}

export function UpgradeSheet({ onClose }: UpgradeSheetProps) {
  const plan = useAppStore((s) => s.plan);
  const setLicense = useAppStore((s) => s.setLicense);
  const isPremium = plan === 'premium';
  const reduced = useReducedMotion();

  const [keyOpen, setKeyOpen] = useState(false);
  const [key, setKey] = useState('');
  const [keyState, setKeyState] = useState<KeyState>('idle');
  const [errorMsg, setErrorMsg] = useState('');
  const [clipboardHint, setClipboardHint] = useState(false);

  const dialogRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  // Return focus where it came from — the lock badge or Settings row that opened us.
  const openerRef = useRef<HTMLElement | null>(
    typeof document !== 'undefined' ? (document.activeElement as HTMLElement | null) : null,
  );

  useEffect(() => {
    const opener = openerRef.current;
    dialogRef.current?.focus();
    return () => opener?.focus?.();
  }, []);

  useEffect(() => {
    if (keyOpen) inputRef.current?.focus();
  }, [keyOpen]);

  // A verified key closes the sheet on its own so the unlocked UI is what's left.
  useEffect(() => {
    if (keyState !== 'success') return;
    const t = setTimeout(onClose, reduced ? 600 : 1800);
    return () => clearTimeout(t);
  }, [keyState, onClose, reduced]);

  const handlePaste = useCallback(async () => {
    try {
      setKey((await navigator.clipboard.readText()).trim().toUpperCase());
      setClipboardHint(false);
    } catch {
      setClipboardHint(true);
    }
  }, []);

  const handleActivate = useCallback(async () => {
    const trimmed = key.trim();
    if (!trimmed) return;
    setKeyState('loading');
    setErrorMsg('');
    try {
      const result = await window.electronAPI.license.activate(trimmed);
      if (result.success) {
        setKeyState('success');
        setLicense({ activated: true, plan: result.plan ?? 'premium' });
      } else {
        setKeyState('error');
        setErrorMsg(result.message ?? 'That key was not accepted. Check it and try again.');
      }
    } catch {
      setKeyState('error');
      setErrorMsg('Network error — check your connection and try again.');
    }
  }, [key, setLicense]);

  const openSupport = useCallback(() => {
    void window.electronAPI.app.openSupport();
  }, []);

  // Esc closes; Tab cycles inside the sheet instead of reaching the app behind it.
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== 'Tab') return;
    const focusables = e.currentTarget.querySelectorAll<HTMLElement>(
      'button:not([disabled]), input:not([disabled]), [tabindex="0"]',
    );
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

  return (
    <div
      className="fixed inset-0 z-(--z-modal) flex items-center justify-center p-4 no-drag"
      style={{ background: 'var(--color-scrim)', backdropFilter: 'blur(10px) saturate(1.1)' }}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      {/* @container: the two cards decide on the sheet's own width, which is capped
          at 820 and so never matches the window. On a 640px-tall window 90vh is
          576px and the sheet scrolls internally — the short-height variants below
          buy back enough of that for the price and the CTA. */}
      <motion.div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label="Upgrade to Premium"
        tabIndex={-1}
        onKeyDown={onKeyDown}
        initial={reduced ? { opacity: 0 } : { opacity: 0, y: 16, scale: 0.98 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: 0.32, ease: [0.22, 1, 0.36, 1] }}
        className="@container relative w-full max-w-[820px] max-h-[90vh] overflow-y-auto overscroll-contain rounded-xl border border-border outline-none"
        style={{
          background: 'var(--color-bg-secondary)',
          boxShadow: 'var(--shadow-lg)',
          padding: 'clamp(20px, 3.2vw, 34px)',
        }}
      >
        <button
          type="button"
          onClick={onClose}
          aria-label="Close"
          className="absolute right-4 top-4 grid place-items-center rounded-md text-text-muted hover:text-text-primary hover:bg-bg-hover transition-colors"
          style={{ width: 32, height: 32 }}
        >
          <Close size={16} />
        </button>

        <header className="mb-7 [@media(max-height:720px)]:mb-4 pr-10">
          <h2 className="text-h2 text-text-primary text-balance">
            {isPremium ? 'You’re on Premium' : 'One purchase. Every limit gone.'}
          </h2>
          <p className="text-text-secondary text-sm mt-1.5 leading-relaxed max-w-[62ch]">
            {isPremium
              ? 'Every feature below is unlocked on this machine. Thank you for buying it.'
              : 'AHG Universal Converter is fully usable for free. Premium lifts the ceilings — once, not monthly.'}
          </p>
        </header>

        {/* Side by side while the sheet has ~672px of content — true at the 960px
            minimum window (759px of card room). Narrower than that they stack, and
            the plan the user is actually on comes first: their own card is the
            anchor, the other one is the comparison. */}
        <div className="grid gap-4 @2xl:grid-cols-[minmax(0,1fr)_minmax(0,1.15fr)] items-start">
          <PlanCard
            className={`${isPremium ? 'order-2' : 'order-1'} @2xl:order-none`}
            title="Basic"
            blurb="What you have right now."
            price="Free"
            priceNote="no account, no key"
            values={PLAN_FEATURES.map((f) => f.basic)}
            active={!isPremium}
            delay={reduced ? 0 : 0.04}
          />

          {/* Premium — the committed card: accent frame, badge, price, the CTA. */}
          <PlanCard
            className={`${isPremium ? 'order-1' : 'order-2'} @2xl:order-none`}
            title="Premium"
            blurb="Everything, on this machine, forever."
            price={PRICE}
            priceNote="one-time purchase"
            values={PLAN_FEATURES.map((f) => f.premium)}
            active={isPremium}
            featured
            delay={reduced ? 0 : 0.1}
          >
            {isPremium ? (
              <div
                className="flex items-center justify-center gap-2 rounded-md text-sm font-medium"
                style={{
                  height: 42,
                  color: 'var(--color-success)',
                  background: 'color-mix(in oklab, var(--color-success) 12%, transparent)',
                }}
              >
                <Check size={15} />
                Active on this machine
              </div>
            ) : keyState === 'success' ? (
              <motion.div
                initial={reduced ? {} : { opacity: 0, scale: 0.96 }}
                animate={{ opacity: 1, scale: 1 }}
                className="flex items-center justify-center gap-2 rounded-md text-sm font-medium"
                style={{
                  height: 42,
                  color: 'var(--color-success)',
                  background: 'color-mix(in oklab, var(--color-success) 12%, transparent)',
                }}
              >
                <Check size={15} />
                Premium unlocked
              </motion.div>
            ) : (
              <div className="flex flex-col gap-2.5">
                <Button variant="primary" size="md" icon={Premium} onClick={openSupport} className="w-full">
                  Get a key — {PRICE}
                </Button>

                <button
                  type="button"
                  onClick={() => setKeyOpen((v) => !v)}
                  aria-expanded={keyOpen}
                  className="inline-flex items-center justify-center gap-1.5 text-text-muted text-xs hover:text-text-secondary transition-colors"
                >
                  I already have a key
                  <ChevronDown
                    size={13}
                    className="transition-transform duration-200"
                    style={{ transform: keyOpen ? 'rotate(180deg)' : 'none' }}
                  />
                </button>

                <AnimatePresence initial={false}>
                  {keyOpen && (
                    <motion.div
                      key="keyform"
                      initial={reduced ? { opacity: 0 } : { opacity: 0, height: 0 }}
                      animate={reduced ? { opacity: 1 } : { opacity: 1, height: 'auto' }}
                      exit={reduced ? { opacity: 0 } : { opacity: 0, height: 0 }}
                      transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
                      className="overflow-hidden"
                    >
                      <div
                        className="field-shell flex items-center gap-2 rounded-lg border border-transparent bg-bg-tertiary transition-[border-color,box-shadow] mt-1"
                        style={{ height: 46, paddingLeft: 12, paddingRight: 6 }}
                      >
                        <LicenseKey size={14} className="text-text-muted shrink-0" />
                        <input
                          ref={inputRef}
                          type="text"
                          value={key}
                          onChange={(e) => {
                            setKey(e.target.value.toUpperCase());
                            if (keyState === 'error') setKeyState('idle');
                          }}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') void handleActivate();
                          }}
                          placeholder="Paste your key…"
                          aria-label="License key"
                          spellCheck={false}
                          autoComplete="off"
                          disabled={keyState === 'loading'}
                          className="flex-1 min-w-0 bg-transparent outline-none text-text-primary font-mono text-sm tracking-[0.12em] placeholder:text-text-muted placeholder:font-sans placeholder:tracking-normal"
                        />
                        <button
                          type="button"
                          onClick={() => void handlePaste()}
                          disabled={keyState === 'loading'}
                          aria-label="Paste from clipboard"
                          className="shrink-0 grid place-items-center rounded-md border border-border-soft bg-bg-surface text-text-muted hover:border-accent hover:text-text-primary transition-[border-color,color] disabled:opacity-40"
                          style={{ width: 32, height: 32 }}
                        >
                          <Paste size={13} />
                        </button>
                      </div>

                      {clipboardHint && (
                        <p role="alert" className="text-xs text-text-muted mt-2">
                          Clipboard access blocked — paste manually.
                        </p>
                      )}

                      {keyState === 'error' && errorMsg && (
                        <div
                          role="alert"
                          className="flex items-start gap-2 rounded-lg border text-xs leading-snug mt-2"
                          style={{
                            padding: '9px 11px',
                            color: 'var(--color-error)',
                            background: 'color-mix(in oklab, var(--color-error) 11%, transparent)',
                            borderColor: 'color-mix(in oklab, var(--color-error) 32%, transparent)',
                          }}
                        >
                          <Alert size={13} className="shrink-0 mt-0.5" />
                          <span>{errorMsg}</span>
                        </div>
                      )}

                      <Button
                        variant="ghost"
                        size="sm"
                        loading={keyState === 'loading'}
                        disabled={!key.trim() || keyState === 'loading'}
                        onClick={() => void handleActivate()}
                        className="w-full mt-2"
                      >
                        {keyState === 'loading' ? 'Verifying…' : 'Unlock Premium'}
                      </Button>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            )}
          </PlanCard>
        </div>

        <footer className="flex items-center gap-3 mt-6">
          <button
            type="button"
            onClick={openSupport}
            className="inline-flex items-center gap-1.5 text-text-muted text-xs hover:text-text-secondary transition-colors"
          >
            <Support size={13} />
            Questions? Ask on Discord
          </button>
          <div className="flex-1" />
          <p className="text-text-muted text-xs">One key, one machine — transferable from Settings.</p>
        </footer>
      </motion.div>
    </div>
  );
}

// ── Card ──────────────────────────────────────────────────────────────────────

interface PlanCardProps {
  title: string;
  blurb: string;
  price: string;
  priceNote: string;
  /** Per-feature value strings, index-aligned with PLAN_FEATURES. */
  values: string[];
  active: boolean;
  featured?: boolean;
  delay: number;
  /** Layout only — the stacking order at narrow sheet widths. */
  className?: string;
  children?: React.ReactNode;
}

function PlanCard({ title, blurb, price, priceNote, values, active, featured, delay, className = '', children }: PlanCardProps) {
  return (
    <motion.section
      aria-label={`${title} plan`}
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.34, delay, ease: [0.22, 1, 0.36, 1] }}
      className={`rounded-lg border overflow-hidden ${className}`}
      style={{
        borderColor: featured ? 'color-mix(in oklab, var(--color-accent) 42%, transparent)' : 'var(--color-border-soft)',
        background: featured ? 'var(--color-bg-surface)' : 'var(--color-bg-primary)',
        boxShadow: featured ? 'var(--shadow-md)' : 'none',
      }}
    >
      {/* Badge strip — the reference's "MOST POPULAR" rail, re-cut as a plain
          statement of the purchase model. Basic gets a matching rail so the two
          cards line up row-for-row without pretending to be the same offer. */}
      <div
        className="flex items-center justify-between gap-2"
        style={{
          padding: '10px 16px',
          borderBottom: '1px dashed var(--color-border-soft)',
          background: featured ? 'var(--color-accent-soft)' : 'transparent',
        }}
      >
        <span
          className="text-[11px] font-semibold uppercase"
          style={{ letterSpacing: '0.09em', color: featured ? 'var(--color-accent)' : 'var(--color-text-muted)' }}
        >
          {featured ? 'Pay once' : 'Free forever'}
        </span>
        {active && (
          <span
            className="inline-flex items-center gap-1 rounded-full text-[11px] font-medium"
            style={{
              padding: '2px 8px',
              color: 'var(--color-text-secondary)',
              background: 'var(--color-bg-hover)',
            }}
          >
            <Check size={11} />
            Current
          </span>
        )}
      </div>

      <div style={{ padding: 'clamp(16px, 2.2vw, 22px)' }}>
        <h3 className="text-lg font-semibold text-text-primary leading-none">{title}</h3>
        <p className="text-text-muted text-xs mt-1.5">{blurb}</p>

        {/* Short window: the price and the CTA under it are what has to stay in
            view, so the vertical rhythm gives way before they do. */}
        <div className="flex items-baseline gap-2.5 mt-5 mb-5 [@media(max-height:720px)]:mt-3 [@media(max-height:720px)]:mb-3">
          <span
            className={`font-semibold tabular-nums text-text-primary ${
              featured ? 'text-[44px] [@media(max-height:720px)]:text-[34px]' : 'text-[34px] [@media(max-height:720px)]:text-[28px]'
            }`}
            style={{ letterSpacing: '-0.03em', lineHeight: 1 }}
          >
            {price}
          </span>
          <span className="text-text-muted text-xs leading-tight max-w-[10ch]">{priceNote}</span>
        </div>

        <ul className="flex flex-col gap-2.5 mb-5 [@media(max-height:720px)]:gap-1.5 [@media(max-height:720px)]:mb-3">
          {PLAN_FEATURES.map((f, i) => {
            const Icon = f.icon;
            return (
              <li key={f.key} className="flex items-start gap-2.5">
                <Icon
                  size={15}
                  className="shrink-0 mt-0.5"
                  style={{ color: featured ? 'var(--color-accent)' : 'var(--color-text-muted)' }}
                  aria-hidden
                />
                <div className="min-w-0">
                  <p className="text-text-primary text-sm leading-snug">{values[i]}</p>
                  <p className="text-text-muted text-xs leading-snug">{f.label}</p>
                </div>
              </li>
            );
          })}
        </ul>

        {children ?? (
          <div
            className="flex items-center justify-center rounded-md text-sm text-text-muted"
            style={{ height: 42, border: '1px dashed var(--color-border-soft)' }}
          >
            {active ? 'Your current plan' : 'Included in Premium'}
          </div>
        )}
      </div>
    </motion.section>
  );
}
