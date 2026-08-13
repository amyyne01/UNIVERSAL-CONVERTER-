import { Lock } from 'lucide-react';
import { AUDIO_FORMATS } from '@/constants';
import type { AudioFormat } from '@shared/types';
import { useTierLocks } from '@/lib/tier';
import { Chip } from './Chip';

export interface FormatSelectorProps {
  value: AudioFormat | string;
  onChange: (value: AudioFormat) => void;
  className?: string;
}

export function FormatSelector({ value, onChange, className = '' }: FormatSelectorProps) {
  // The lossless rule is the same everywhere this renders, so it lives here
  // rather than being re-plumbed through all three call sites.
  const { lockedFormats, nudge } = useTierLocks();

  return (
    <div className={`flex flex-wrap gap-2 ${className}`}>
      {AUDIO_FORMATS.map((f) => {
        const locked = lockedFormats.includes(f.value);
        return (
          <Chip
            key={f.value}
            selected={value === f.value}
            tag={f.lossless ? 'lossless' : undefined}
            icon={locked ? Lock : undefined}
            aria-label={locked ? `${f.label} — Premium` : undefined}
            onClick={() =>
              locked ? nudge(`${f.label} is lossless — a Premium format.`) : onChange(f.value)
            }
          >
            {f.label}
          </Chip>
        );
      })}
    </div>
  );
}
