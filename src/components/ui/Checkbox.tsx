// Visual-only checkbox for rows that are themselves the role=checkbox / button.
export function Checkbox({ checked, accent }: { checked: boolean; accent: 'accent' | 'spotify' }) {
  const bg =
    accent === 'spotify'
      ? 'var(--color-spotify)'
      : 'var(--color-accent)';
  return (
    <span
      className="w-5 h-5 rounded-sm border flex items-center justify-center transition-colors flex-none"
      style={
        checked
          ? { background: bg, borderColor: bg }
          : { borderColor: 'var(--color-border)', background: 'var(--color-bg-secondary)' }
      }
    >
      {checked && (
        <svg viewBox="0 0 12 12" width="11" height="11" fill="none" aria-hidden>
          <path
            d="M2 6.5L4.5 9 10 3.5"
            stroke="var(--color-bg-primary)"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      )}
    </span>
  );
}
