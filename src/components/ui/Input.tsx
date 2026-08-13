import { forwardRef, type InputHTMLAttributes } from 'react';
import type { LucideIcon } from 'lucide-react';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  icon?: LucideIcon;
  /** Wrapper className (the field shell). */
  wrapClassName?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { icon: Icon, wrapClassName = '', className = '', ...rest },
  ref,
) {
  return (
    <div
      className={`field-shell no-drag flex items-center gap-3 h-10 px-3 rounded-md bg-bg-tertiary border border-transparent transition-[border-color,box-shadow] ${wrapClassName}`}
    >
      {Icon && <Icon size={18} className="text-text-muted shrink-0" />}
      <input
        ref={ref}
        className={`flex-1 min-w-0 bg-transparent outline-none text-text-primary placeholder:text-text-muted text-sm ${className}`}
        {...rest}
      />
    </div>
  );
});
