// Live aggregate download speed: one sample per second from the store's per-task
// progress (no IPC). Sampling stops while the window is hidden or reduced motion
// is on; the chart then holds still.
import { useEffect, useState } from 'react';
import { useReducedMotion } from 'framer-motion';
import { useAppStore } from '@/store';
import { formatBytes } from '@/lib/format';
import { SPARK_SAMPLES, niceCeil, sparkPath, stepCeiling } from '@/lib/sparkline';

const H = 40;

const totalSpeed = () =>
  Object.values(useAppStore.getState().downloads).reduce(
    (sum, t) => sum + (t.progress.status === 'downloading' ? t.progress.speed || 0 : 0), 0);

export function SpeedSparkline() {
  const reduce = useReducedMotion();
  const [{ samples, ceil, tick }, setState] = useState({ samples: [] as number[], ceil: 1, tick: 0 });

  useEffect(() => {
    const sample = () => setState((s) => {
      const next = [...s.samples, totalSpeed()].slice(-SPARK_SAMPLES);
      return { samples: next, ceil: stepCeiling(s.ceil, niceCeil(Math.max(...next))), tick: s.tick + 1 };
    });
    sample();
    if (reduce) return;
    let id: ReturnType<typeof setInterval> | undefined;
    const sync = () => {
      clearInterval(id);
      id = document.visibilityState === 'hidden' ? undefined : setInterval(sample, 1000);
    };
    sync();
    document.addEventListener('visibilitychange', sync);
    return () => { clearInterval(id); document.removeEventListener('visibilitychange', sync); };
  }, [reduce]);

  const now = samples[samples.length - 1] ?? 0;
  const d = sparkPath(samples, ceil, H);
  return (
    <div className="flex items-end gap-4">
      <div className="shrink-0 w-28">
        <div data-testid="spark-now" className="font-mono tabular-nums text-[22px] font-semibold text-text-primary">
          {formatBytes(now)}/s
        </div>
        <div data-testid="spark-ceil" className="font-mono tabular-nums text-[10.5px] text-text-muted">
          scale {formatBytes(Math.round(ceil))}/s
        </div>
      </div>
      <svg
        role="img"
        aria-label={`Download speed, ${formatBytes(now)} per second`}
        viewBox={`0 0 ${SPARK_SAMPLES} ${H}`}
        preserveAspectRatio="none"
        className="flex-1 min-w-0 h-12 overflow-hidden"
      >
        <g key={tick} className={reduce ? undefined : 'spark-scroll'}>
          <path d={d} fill="none" stroke="var(--color-accent)" strokeWidth="1.5" vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
        </g>
      </svg>
    </div>
  );
}
