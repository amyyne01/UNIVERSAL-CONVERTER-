// The clipboard watcher's one-click download offer (electron/clipboard-watch.ts).
// Same mechanics as Notice.tsx — a transient bottom-corner popup — but its own
// component: it carries a UrlDetection + a Download action, not a message + tone.

import { motion, AnimatePresence, useReducedMotion } from 'framer-motion';
import { EASE_OUT_QUINT } from '@/components/ui/motion';
import { Download, Link, Close } from '@/components/ui/icons';
import { Button, IconButton } from '@/components/ui';
import { useAppStore } from '@/store';
import { useDismissTimer } from '@/lib/dismissTimer';
import { buildDownloadRequest } from '@/lib/download';

const DISMISS_MS = 12_000; // an action needs longer on screen than a plain nudge

export function ClipboardOffer() {
  const offer = useAppStore((s) => s.clipboardOffer);
  const config = useAppStore((s) => s.config);
  const addDownload = useAppStore((s) => s.addDownload);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const dismiss = () => useAppStore.getState().setClipboardOffer(null);
  const reduced = useReducedMotion();
  const hold = useDismissTimer(offer, DISMISS_MS, dismiss);

  const download = () => {
    if (!offer) return;
    void window.electronAPI.download
      .start(buildDownloadRequest(config, {
        url: offer.url,
        source: offer.platform,
        isPlaylist: offer.isCollection,
      }))
      .then((task) => {
        addDownload(task);
        setActiveTab('queue');
      })
      .catch(() => {});
    dismiss();
  };

  return (
    <AnimatePresence>
      {offer && (
        <motion.div
          key="clipboard-offer"
          role="status"
          aria-live="polite"
          initial={reduced ? { opacity: 0 } : { opacity: 0, y: 14, scale: 0.97 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={reduced ? { opacity: 0 } : { opacity: 0, y: 8, scale: 0.98 }}
          transition={{ duration: 0.26, ease: EASE_OUT_QUINT }}
          {...hold}
          className="fixed bottom-28 left-5 z-(--z-palette) no-drag flex items-start gap-3 rounded-lg border max-w-[380px]"
          style={{
            padding: '14px 14px 14px 16px',
            borderColor: 'var(--color-border)',
            background: 'var(--color-bg-surface)',
            boxShadow: 'var(--shadow-lg)',
          }}
        >
          <span
            aria-hidden
            className="grid place-items-center rounded-md shrink-0"
            style={{ width: 30, height: 30, background: 'var(--color-accent-soft)', color: 'var(--color-accent)' }}
          >
            <Link size={16} />
          </span>

          <div className="min-w-0 flex-1">
            <p className="text-text-primary text-sm font-medium leading-snug">
              {offer.label} copied
            </p>
            <Button size="sm" icon={Download} className="mt-2" onClick={download}>
              Download
            </Button>
          </div>

          <IconButton icon={Close} label="Dismiss" onClick={dismiss} className="shrink-0" />
        </motion.div>
      )}
    </AnimatePresence>
  );
}
