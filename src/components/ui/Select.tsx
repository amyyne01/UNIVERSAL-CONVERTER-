import { type SelectHTMLAttributes } from 'react';
import { ChevronDown } from '@/components/ui/icons';

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectProps extends Omit<SelectHTMLAttributes<HTMLSelectElement>, 'children'> {
  options: readonly SelectOption[];
  /** Accessible name for the control (sets aria-label). Optional so existing call sites keep compiling. */
  label?: string;
  /** Values this tier may not pick: labelled "· Premium" and routed to onLocked. */
  lockedValues?: readonly string[];
  onLocked?: (value: string) => void;
}

export function Select({ options, className = '', label, lockedValues, onLocked, onChange, ...rest }: SelectProps) {
  // A locked option stays selectable so picking it can explain itself, but the
  // selection is reverted and the change never reaches the caller.
  const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    if (lockedValues?.includes(e.target.value)) {
      const picked = e.target.value;
      e.target.value = String(rest.value ?? '');
      onLocked?.(picked);
      return;
    }
    onChange?.(e);
  };

  return (
    <div className="relative no-drag inline-flex">
      <select
        aria-label={label}
        onChange={handleChange}
        className={`appearance-none focus-visible:focus-ring h-10 pl-3 pr-9 rounded-md bg-bg-tertiary border border-transparent text-text-primary text-sm hover:border-border cursor-pointer ${className}`}
        {...rest}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {lockedValues?.includes(o.value) ? `${o.label} · Premium` : o.label}
          </option>
        ))}
      </select>
      <ChevronDown
        size={14}
        className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-text-muted"
      />
    </div>
  );
}
