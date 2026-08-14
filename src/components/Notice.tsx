// The app's one transient popup, in two tones.
//
// 'premium' answers "you just found a ceiling" — it names what the tier costs you
// in that exact moment and offers the one route onward. 'error' answers "that
// didn't work" — it says what happened in plain language and, where there is one,
// what to do instead. Same mechanics, different urgency and colour, because
// shipping two near-identical toasts is how they drift apart.
import { useEffect } from 'react';
import { motion, AnimatePresence, useReducedMotion } from 'framer-motion';
import { Premium, Alert, Close } from '@/components/ui/icons';
import { useAppStore } from '@/store';

// An error has to outlast a glance at what broke; a nudge is an invitation and
// should not loiter.
const DISMISS_MS = { premium: 6000, error: 9000 } as const;

export function Notice() {
  const notice = useAppStore((s) => s.notice);
  const dismiss = useAppStore((s) => s.dismissNotice);
  const setUpgradeOpen = useAppStore((s) => s.setUpgradeOpen);
  const reduced = useReducedMotion();

  const tone = notice?.tone ?? 'premium';
  const isError = tone === 'error';

  // Re-arms on every new message so a second notice doesn't inherit the first timer.
  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(dismiss, DISMISS_MS[notice.tone]);
    return () => clearTimeout(t);
  }, [notice, dismiss]);

  return (
    <AnimatePresence>
      {notice && (
        <motion.div
          key="notice"
          role="status"
          aria-live={isError ? 'assertive' : 'polite'}
          initial={reduced ? { opacity: 0 } : { opacity: 0, y: 14, scale: 0.97 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={reduced ? { opacity: 0 } : { opacity: 0, y: 8, scale: 0.98 }}
          transition={{ duration: 0.26, ease: [0.22, 1, 0.36, 1] }}
          className="fixed bottom-5 right-5 z-(--z-palette) no-drag flex items-start gap-3 rounded-lg border max-w-[380px]"
          style={{
            padding: '14px 14px 14px 16px',
            borderColor: isError
              ? 'color-mix(in oklab, var(--color-error) 45%, transparent)'
              : 'color-mix(in oklab, var(--color-accent) 45%, transparent)',
            background: 'var(--color-bg-surface)',
            // The app's one glow marks the premium moment; an error gets weight
            // from its border and icon instead — a glowing failure reads as decor.
            boxShadow: isError ? 'var(--shadow-lg)' : 'var(--shadow-lg), var(--shadow-glow)',
          }}
        >
          <span
            aria-hidden
            className="grid place-items-center rounded-md shrink-0"
            style={{
              width: 30,
              height: 30,
              background: isError
                ? 'color-mix(in oklab, var(--color-error) 14%, transparent)'
                : 'var(--color-accent-soft)',
              color: isError ? 'var(--color-error)' : 'var(--color-accent)',
            }}
          >
            {isError ? <Alert size={16} /> : <Premium size={15} />}
          </span>

          <div className="min-w-0 flex-1">
            <p className="text-text-primary text-sm font-medium leading-snug">{notice.message}</p>
            {!isError && (
              <button
                type="button"
                onClick={() => setUpgradeOpen(true)}
                className="mt-2 inline-flex items-center gap-1.5 rounded-md bg-accent text-on-accent text-xs font-medium hover:bg-accent-hover transition-colors"
                style={{ height: 30, paddingLeft: 11, paddingRight: 11 }}
              >
                See plans
              </button>
            )}
          </div>

          <button
            type="button"
            onClick={dismiss}
            aria-label="Dismiss"
            className="shrink-0 grid place-items-center rounded-md text-text-muted hover:text-text-primary hover:bg-bg-hover transition-colors"
            style={{ width: 26, height: 26 }}
          >
            <Close size={14} />
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
