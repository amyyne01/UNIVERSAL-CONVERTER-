// Opt-in clipboard link watcher — JDownloader's LinkGrabber / IDM's catch-and-offer.
// Polls the clipboard; when the text CHANGES and detectUrl() classifies it as
// anything but 'unknown', the renderer is notified so it can offer a one-click
// download. Never downloads on its own — see main.ts's pasteAndDownload for the
// (deliberately different) auto-download hotkey behaviour.
import { clipboard } from 'electron';
import { detectUrl } from './url-detector.js';
import type { UrlDetection } from '../shared/types.js';

export interface ClipboardWatchDeps {
  isEnabled: () => boolean;
  onDetect: (detection: UrlDetection) => void;
  /** Injected for testing — defaults to Electron's clipboard.readText. */
  readClipboard?: () => string;
}

const POLL_MS = 1500;

export class ClipboardWatcher {
  private timer: ReturnType<typeof setInterval> | null = null;
  // Last clipboard text seen, changed-content is the only thing that can trigger
  // a notify — this alone is what stops the same link firing twice in a row.
  private lastText = '';

  constructor(private readonly deps: ClipboardWatchDeps) {}

  start(): void {
    if (this.timer) return;
    // Seed with whatever is already on the clipboard so enabling the watcher never
    // immediately re-offers a link the user copied before turning it on.
    try { this.lastText = this.read(); } catch { /* best-effort seed */ }
    this.timer = setInterval(() => this.tick(), POLL_MS);
    this.timer.unref?.();
  }

  stop(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  private read(): string {
    return (this.deps.readClipboard ?? clipboard.readText)().trim();
  }

  private tick(): void {
    if (!this.deps.isEnabled()) return;
    try {
      const text = this.read();
      if (!text || text === this.lastText) return;
      this.lastText = text;
      const detection = detectUrl(text);
      if (detection.platform === 'unknown') return;
      this.deps.onDetect(detection);
    } catch (err) {
      // Unattended timer — a bad read must not take down the process.
      console.error('[clipboard-watch] tick failed', err);
    }
  }
}
