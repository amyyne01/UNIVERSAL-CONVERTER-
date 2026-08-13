import { motion, useReducedMotion } from 'framer-motion';
import type { DownloadStatus } from '@shared/types';

function colorFor(status: DownloadStatus): string {
  switch (status) {
    case 'done':
      return 'bg-success';
    case 'failed':
      return 'bg-error';
    case 'converting':
    case 'embedding':
    case 'retrying':
    case 'paused':
      return 'bg-warning';
    case 'queued':
    case 'cancelled':
      return 'bg-text-muted';
    default:
      return 'bg-accent';
  }
}

// yt-dlp reports no percentage while it probes, muxes or writes tags — those phases
// sweep instead of freezing the bar at whatever the download left behind.
const INDETERMINATE = new Set<DownloadStatus>(['fetching_info', 'converting', 'embedding', 'retrying']);

export interface ProgressBarProps {
  percent: number;
  status: DownloadStatus;
  className?: string;
  /** Accessible name for the progressbar (e.g. the task title). */
  label?: string;
}

export function ProgressBar({ percent, status, className = '', label }: ProgressBarProps) {
  const reduced = useReducedMotion();
  const indeterminate = INDETERMINATE.has(status);
  const width = Math.max(0, Math.min(100, percent));
  const live = status === 'downloading';

  return (
    <div
      className={`relative h-1.5 rounded-full bg-bg-secondary overflow-hidden ${className}`}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={indeterminate ? undefined : width}
      aria-valuetext={indeterminate ? status : `${width}%`}
      aria-label={label}
    >
      {indeterminate ? (
        <motion.div
          className={`absolute inset-y-0 w-2/5 rounded-full ${colorFor(status)}`}
          animate={reduced ? { x: '30%' } : { x: ['-100%', '250%'] }}
          transition={reduced ? { duration: 0 } : { repeat: Infinity, duration: 1.3, ease: 'easeInOut' }}
        />
      ) : (
        <motion.div
          className={`relative h-full rounded-full overflow-hidden ${colorFor(status)}`}
          initial={false}
          animate={{ width: `${width}%` }}
          // Progress arrives in bursty ~2/s ticks; a slightly longer eased tween
          // carries the fill between them so it reads as continuous motion.
          transition={{ ease: 'easeOut', duration: reduced ? 0 : 0.6 }}
        >
          {/* Live sheen — proof the transfer is moving even when a big fragment
              stalls the percentage for a second or two. */}
          {live && !reduced && (
            <motion.span
              aria-hidden
              className="absolute inset-y-0 w-1/3 bg-gradient-to-r from-transparent via-white/35 to-transparent"
              animate={{ x: ['-120%', '420%'] }}
              transition={{ repeat: Infinity, duration: 1.6, ease: 'linear' }}
            />
          )}
        </motion.div>
      )}
    </div>
  );
}
