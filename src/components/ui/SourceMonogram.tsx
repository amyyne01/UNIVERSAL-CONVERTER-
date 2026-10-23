import type { SourcePlatform } from '@shared/types';
import { SOURCE_META } from '@/constants';

export interface SourceMonogramProps {
  source: SourcePlatform;
  className?: string;
}

/** 1-2 mono letters on a tinted tile. A letter mark, not a brand mark: no logos. */
export function SourceMonogram({ source, className = '' }: SourceMonogramProps) {
  const { label, color, mono } = SOURCE_META[source];
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      style={{ color, background: `color-mix(in oklch, ${color} 16%, transparent)` }}
      className={`inline-flex h-5 min-w-5 px-1 items-center justify-center rounded-sm font-mono text-[11px] font-semibold leading-none tracking-tight ${className}`}
    >
      {mono}
    </span>
  );
}
