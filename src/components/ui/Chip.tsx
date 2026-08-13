import type { ReactNode } from 'react';
import { motion, type HTMLMotionProps } from 'framer-motion';
import type { LucideIcon } from 'lucide-react';

export interface ChipProps extends Omit<HTMLMotionProps<'button'>, 'children'> {
  selected?: boolean;
  /** Small mono suffix, e.g. a bitrate or "lossless". */
  tag?: string;
  /** Leading glyph — used for the lock on tier-gated options. */
  icon?: LucideIcon;
  children?: ReactNode;
}

export function Chip({ selected = false, tag, icon: Icon, className = '', children, ...rest }: ChipProps) {
  return (
    <motion.button
      type="button"
      whileTap={{ scale: 0.96 }}
      aria-pressed={selected}
      className={`no-drag inline-flex items-center gap-2 h-8 px-3 rounded-md border text-[12.5px] font-medium transition-colors disabled:opacity-50 ${
        selected
          ? 'bg-accent-soft border-accent/40 text-accent'
          : 'border-border text-text-secondary hover:text-text-primary hover:bg-bg-hover'
      } ${className}`}
      {...rest}
    >
      {Icon && <Icon size={12} className="shrink-0 text-text-muted" aria-hidden />}
      {children}
      {tag && (
        <span className={`font-mono text-[10px] ${selected ? 'text-accent' : 'text-text-muted'}`}>
          {tag}
        </span>
      )}
    </motion.button>
  );
}
