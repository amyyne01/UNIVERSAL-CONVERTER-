import { motion, type HTMLMotionProps } from 'framer-motion';
import type { AppIcon } from '@/components/ui/icons';

export interface IconButtonProps extends HTMLMotionProps<'button'> {
  icon: AppIcon;
  label: string;
  size?: number;
}

export function IconButton({
  icon: Icon,
  label,
  size = 18,
  className = '',
  ...rest
}: IconButtonProps) {
  return (
    <motion.button
      whileTap={{ scale: 0.92 }}
      aria-label={label}
      title={label}
      className={`no-drag grid place-items-center w-9 h-9 rounded-md transition-colors text-text-muted hover:text-text-primary hover:bg-bg-hover ${className}`}
      {...rest}
    >
      <Icon size={size} />
    </motion.button>
  );
}
