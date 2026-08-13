import type { DownloadStatus } from '../shared/types.js';

// Download job state machine (HOW-THE-APP-WORKS.md §7).
// Every legal transition is written down here and validated before a job's
// status changes — an impossible transition (e.g. done → downloading) is
// rejected instead of silently corrupting the job.

/**
 * Legal next-states for each {@link DownloadStatus}. The forward path is
 * queued → fetching_info → downloading → converting → embedding → done; the
 * active states branch into paused/retrying/failed/cancelled.
 * `done` and `cancelled` are terminal (no outgoing transitions).
 */
export const transitions: Record<DownloadStatus, readonly DownloadStatus[]> = {
  queued: ['fetching_info', 'paused', 'failed', 'cancelled'],
  fetching_info: ['downloading', 'paused', 'retrying', 'failed', 'cancelled'],
  downloading: ['converting', 'paused', 'retrying', 'failed', 'cancelled'],
  converting: ['embedding', 'paused', 'retrying', 'failed', 'cancelled'],
  embedding: ['done', 'paused', 'retrying', 'failed', 'cancelled'],
  paused: ['downloading', 'queued', 'cancelled'],
  retrying: ['queued', 'downloading', 'cancelled'],
  failed: ['retrying', 'queued', 'cancelled'],
  done: [],
  cancelled: [],
};

export function canTransition(from: DownloadStatus, to: DownloadStatus): boolean {
  return transitions[from].includes(to);
}

export function assertTransition(from: DownloadStatus, to: DownloadStatus): void {
  if (!canTransition(from, to)) {
    throw new Error(`Illegal download status transition: ${from} -> ${to}`);
  }
}
