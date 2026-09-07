// The clipboard watcher's one-click download offer (electron/clipboard-watch.ts).
// Same mechanics as Notice.tsx — a transient bottom-corner popup — but its own
// component: it carries a UrlDetection + a Download action, not a message + tone.
import { useEffect } from 'react';
import { motion, AnimatePresence, useReducedMotion } from 'framer-motion';
import { Download, Link, Close } from '@/components/ui/icons';
import { useAppStore } from '@/store';
import { buildDownloadRequest } from '@/lib/download';

const DISMISS_MS = 12_000; // an action needs longer on screen than a plain nudge

export function ClipboardOffer() {
  const offer = useAppStore((s) => s.clipboardOffer);
  const config = useAppStore((s) => s.config);
  const addDownload = useAppStore((s) => s.addDownload);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const dismiss = () => useAppStore.getState().setClipboardOffer(null);
  const reduced = useReducedMotion();

  useEffect(() => {
    if (!offer) return;
    const t = setTimeout(dismiss, DISMISS_MS);
    return () => clearTimeout(t);
  }, [offer]);

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
          transition={{ duration: 0.26, ease: [0.22, 1, 0.36, 1] }}
          className="fixed bottom-5 left-5 z-(--z-palette) no-drag flex items-start gap-3 rounded-lg border max-w-[380px]"
          style={{
            padding: '14px 14px 14px 16px',
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
            <Link size={15} />
          </span>

          <div className="min-w-0 flex-1">
            <p className="text-text-primary text-sm font-medium leading-snug">
              {offer.label} copied
            </p>
            <button
              type="button"
              onClick={download}
              className="mt-2 inline-flex items-center gap-1.5 rounded-md bg-accent text-on-accent text-xs font-medium hover:bg-accent-hover transition-colors"
              style={{ height: 30, paddingLeft: 11, paddingRight: 11 }}
            >
              <Download size={13} />
              Download
            </button>
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
