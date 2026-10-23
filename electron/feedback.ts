// Completion toast + taskbar progress (A8). Pure over injected collaborators so the
// rules are testable without Electron; main.ts wires Notification and setProgressBar.
import type { DownloadTask, DownloadStatus } from '../shared/types.js';

const ACTIVE = new Set<DownloadStatus>(['fetching_info', 'downloading', 'converting', 'embedding']);

export function downloadFeedback(deps: {
  enabled: () => boolean;
  notify: (body: string) => void;
  setProgress: (fraction: number) => void;
}) {
  let last: number | undefined;
  return {
    /** One toast per finished download, only while "Notify when a download completes" is on. */
    completed(task: DownloadTask): void {
      if (task.progress.status === 'done' && deps.enabled()) deps.notify(task.title || 'Download complete');
    },
    /** Average percent of active downloads as 0..1; -1 removes the bar when nothing runs. */
    progress(tasks: DownloadTask[]): void {
      const active = tasks.filter((t) => ACTIVE.has(t.progress.status));
      const p = active.length ? active.reduce((s, t) => s + t.progress.percent, 0) / active.length / 100 : -1;
      if (p !== last) deps.setProgress((last = p));
    },
  };
}
