import { motion } from 'framer-motion';

export interface ToggleProps {
  checked: boolean;
  onChange: (next: boolean) => void;
  label?: string;
  disabled?: boolean;
  className?: string;
}

export function Toggle({ checked, onChange, label, disabled, className = '' }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`no-drag relative w-9 h-5 rounded-full transition-colors disabled:opacity-50 ${
        checked ? 'bg-accent' : 'bg-bg-hover'
      } ${className}`}
    >
      <motion.span
        animate={{ x: checked ? 16 : 0 }}
        transition={{ type: 'spring', stiffness: 500, damping: 34 }}
        className="absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-text-primary shadow-sm"
      />
    </button>
  );
}
