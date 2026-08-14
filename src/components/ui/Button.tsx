import type { ReactNode } from 'react';
import { motion, type HTMLMotionProps } from 'framer-motion';
import type { AppIcon } from '@/components/ui/icons';
import { Spinner } from './Spinner';

const VARIANTS = {
  primary: 'bg-accent text-on-accent hover:bg-accent-hover',
  ghost: 'text-text-secondary border border-border hover:bg-bg-hover hover:text-text-primary',
  danger: 'bg-error text-on-accent hover:brightness-110',
} as const;

const SIZES = {
  sm: 'h-9 px-3.5 text-sm',
  md: 'h-10 px-4 text-sm',
} as const;

export interface ButtonProps extends Omit<HTMLMotionProps<'button'>, 'children'> {
  variant?: keyof typeof VARIANTS;
  size?: keyof typeof SIZES;
  loading?: boolean;
  icon?: AppIcon;
  children?: ReactNode;
}

export function Button({
  variant = 'primary',
  size = 'md',
  loading = false,
  icon: Icon,
  className = '',
  children,
  disabled,
  ...rest
}: ButtonProps) {
  return (
    <motion.button
      whileHover={{ y: -1 }}
      whileTap={{ scale: 0.98 }}
      disabled={disabled || loading}
      className={`no-drag focus-visible:focus-ring inline-flex items-center justify-center gap-2 font-medium rounded-md transition-[background,filter] duration-150 disabled:opacity-50 disabled:pointer-events-none ${VARIANTS[variant]} ${SIZES[size]} ${className}`}
      {...rest}
    >
      {loading ? <Spinner size={16} /> : Icon ? <Icon size={16} /> : null}
      {children}
    </motion.button>
  );
}
