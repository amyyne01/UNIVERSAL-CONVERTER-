import { motion, type HTMLMotionProps } from 'framer-motion';

export interface CardProps extends HTMLMotionProps<'div'> {}

export function Card({ className = '', ...rest }: CardProps) {
  return (
    <motion.div
      className={`rounded-lg bg-bg-surface border border-border-soft shadow-sm hover:border-border ${className}`}
      {...rest}
    />
  );
}
