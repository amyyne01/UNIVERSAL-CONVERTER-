import { motion, type HTMLMotionProps } from 'framer-motion';

// Tone is a prop rather than a className override because the border colour is
// baked into the base class: overriding it from outside depends on Tailwind's
// source order, which is a coin flip. A surface that carries consequence has to
// be able to say so without re-implementing the card's shape.
const TONES = {
  default: 'border-border-soft hover:border-border',
  danger: 'border-error/30 hover:border-error/45',
} as const;

export interface CardProps extends HTMLMotionProps<'div'> {
  tone?: keyof typeof TONES;
}

export function Card({ tone = 'default', className = '', ...rest }: CardProps) {
  return (
    <motion.div
      className={`rounded-lg bg-bg-surface border shadow-sm ${TONES[tone]} ${className}`}
      {...rest}
    />
  );
}
