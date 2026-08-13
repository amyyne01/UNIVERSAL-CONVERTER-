// The moment a basic install touches something premium, this is what answers.
// Deliberately NOT the full sheet: it names the exact limit that was just hit,
// stays out of the way, and offers one route on to the pricing cards.
import { useEffect } from 'react';
import { motion, AnimatePresence, useReducedMotion } from 'framer-motion';
import { Sparkles, X } from 'lucide-react';
import { useAppStore } from '@/store';

const DISMISS_MS = 6000;

export function PremiumNudge() {
  const reason = useAppStore((s) => s.premiumNudge);
  const dismiss = useAppStore((s) => s.dismissPremiumNudge);
  const setUpgradeOpen = useAppStore((s) => s.setUpgradeOpen);
  const reduced = useReducedMotion();

  // Re-arm on every new reason so a second lock doesn't inherit the first timer.
  useEffect(() => {
    if (!reason) return;
    const t = setTimeout(dismiss, DISMISS_MS);
    return () => clearTimeout(t);
  }, [reason, dismiss]);

  return (
    <AnimatePresence>
      {reason && (
        <motion.div
          key="premium-nudge"
          role="status"
          aria-live="polite"
          initial={reduced ? { opacity: 0 } : { opacity: 0, y: 14, scale: 0.97 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={reduced ? { opacity: 0 } : { opacity: 0, y: 8, scale: 0.98 }}
          transition={{ duration: 0.26, ease: [0.22, 1, 0.36, 1] }}
          className="fixed bottom-5 right-5 z-(--z-palette) no-drag flex items-start gap-3 rounded-lg border max-w-[380px]"
          style={{
            padding: '14px 14px 14px 16px',
            // Highlighted, not alarming: the accent carries it, and the app's one
            // glow token marks this as the single "you found the edge" moment.
            borderColor: 'color-mix(in oklab, var(--color-accent) 45%, transparent)',
            background: 'var(--color-bg-surface)',
            boxShadow: 'var(--shadow-lg), var(--shadow-glow)',
          }}
        >
          <span
            aria-hidden
            className="grid place-items-center rounded-md shrink-0"
            style={{ width: 30, height: 30, background: 'var(--color-accent-soft)', color: 'var(--color-accent)' }}
          >
            <Sparkles size={15} />
          </span>

          <div className="min-w-0 flex-1">
            <p className="text-text-primary text-sm font-medium leading-snug">{reason}</p>
            <button
              type="button"
              onClick={() => setUpgradeOpen(true)}
              className="mt-2 inline-flex items-center gap-1.5 rounded-md bg-accent text-on-accent text-xs font-medium hover:bg-accent-hover transition-colors"
              style={{ height: 30, paddingLeft: 11, paddingRight: 11 }}
            >
              See Premium — $3.99 once
            </button>
          </div>

          <button
            type="button"
            onClick={dismiss}
            aria-label="Dismiss"
            className="shrink-0 grid place-items-center rounded-md text-text-muted hover:text-text-primary hover:bg-bg-hover transition-colors"
            style={{ width: 26, height: 26 }}
          >
            <X size={14} />
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
