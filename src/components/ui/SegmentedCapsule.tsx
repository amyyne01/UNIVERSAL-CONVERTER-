import { useId } from 'react';
import { motion } from 'framer-motion';
import type { AppIcon } from '@/components/ui/icons';

export interface SegmentOption<T extends string> {
  value: T;
  label: string;
  icon?: AppIcon;
}

export interface SegmentedCapsuleProps<T extends string> {
  options: readonly SegmentOption<T>[];
  value: T;
  onChange: (value: T) => void;
  className?: string;
}

/** Generic sliding-pill segmented control (e.g. Light / Dark / System). */
export function SegmentedCapsule<T extends string>({
  options,
  value,
  onChange,
  className = '',
}: SegmentedCapsuleProps<T>) {
  const id = useId();
  return (
    <div
      role="radiogroup"
      className={`no-drag inline-flex p-0.5 gap-0.5 rounded-md bg-bg-tertiary ${className}`}
    >
      {options.map((o) => {
        const active = o.value === value;
        const Icon = o.icon;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(o.value)}
            className={`relative focus-visible:focus-ring inline-flex items-center gap-1.5 px-3.5 h-8 rounded-xs text-sm font-medium transition-colors ${
              active ? 'text-text-primary' : 'text-text-secondary hover:text-text-primary'
            }`}
          >
            {active && (
              <motion.span
                layoutId={`seg-${id}`}
                className="absolute inset-0 rounded-xs bg-bg-surface shadow-sm"
                transition={{ type: 'spring', stiffness: 480, damping: 34 }}
              />
            )}
            {Icon && <Icon size={15} className="relative z-10" />}
            <span className="relative z-10">{o.label}</span>
          </button>
        );
      })}
    </div>
  );
}
