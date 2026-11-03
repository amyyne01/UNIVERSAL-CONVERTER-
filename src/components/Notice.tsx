// The app's one transient popup, in three tones (plus a calm "info" confirmation with an optional follow-up action).
//
// 'premium' answers "you just found a ceiling" — it names what the tier costs you
// in that exact moment and offers the one route onward. 'error' answers "that
// didn't work" — it says what happened in plain language and, where there is one,
// what to do instead. Same mechanics, different urgency and colour, because
// shipping two near-identical toasts is how they drift apart.

import { motion, AnimatePresence, useReducedMotion } from 'framer-motion';
import { EASE_OUT_QUINT } from '@/components/ui/motion';
import { Premium, Alert, Close, Info } from '@/components/ui/icons';
import { Button, IconButton } from '@/components/ui';
import { useAppStore } from '@/store';
import { useDismissTimer } from '@/lib/dismissTimer';

// An error has to outlast a glance at what broke; a nudge is an invitation and
// should not loiter.
const DISMISS_MS = { premium: 6000, error: 9000, info: 6000 } as const;

export function Notice() {
  const notice = useAppStore((s) => s.notice);
  const dismiss = useAppStore((s) => s.dismissNotice);
  const setUpgradeOpen = useAppStore((s) => s.setUpgradeOpen);
  const reduced = useReducedMotion();

  const tone = notice?.tone ?? 'premium';
  const isError = tone === 'error';
  const isInfo = tone === 'info';

  // Re-arms on every new message so a second notice doesn't inherit the first timer.
  const hold = useDismissTimer(notice, DISMISS_MS[tone], dismiss);

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
          transition={{ duration: 0.26, ease: EASE_OUT_QUINT }}
          {...hold}
          className="fixed bottom-28 right-5 z-(--z-palette) no-drag flex items-start gap-3 rounded-lg border max-w-[380px]"
          style={{
            padding: '14px 14px 14px 16px',
            borderColor: isError
              ? 'color-mix(in oklab, var(--color-error) 45%, transparent)'
              : 'var(--color-border)',
            background: 'var(--color-bg-surface)',
            boxShadow: 'var(--shadow-lg)',
          }}
        >
          <span
            aria-hidden
            className="grid place-items-center rounded-md shrink-0"
            style={{
              width: 30,
              height: 30,
              background: isError
                ? 'var(--color-error-soft)'
                : 'var(--color-accent-soft)',
              color: isError ? 'var(--color-error)' : 'var(--color-accent)',
            }}
          >
            {isError ? <Alert size={16} /> : isInfo ? <Info size={16} /> : <Premium size={16} />}
          </span>

          <div className="min-w-0 flex-1">
            <p className="text-text-primary text-sm font-medium leading-snug">{notice.message}</p>
            {(isInfo ? notice?.action : !isError) && (
              <Button
                size="sm"
                className="mt-2"
                onClick={() => {
                  if (isInfo) { notice?.action?.run(); dismiss(); } else setUpgradeOpen(true);
                }}
              >
                {isInfo ? notice?.action?.label : 'See plans'}
              </Button>
            )}
          </div>

          <IconButton icon={Close} label="Dismiss" onClick={dismiss} className="shrink-0" />
        </motion.div>
      )}
    </AnimatePresence>
  );
}
