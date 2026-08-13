// Shared renderer formatters — single source of truth for duration/byte display
// (previously reimplemented per-component; one variant lacked hour support).

/** "3:07" under an hour, "1:02:03" at/above an hour. */
export function formatDuration(secs: number): string {
  if (!secs) return '';
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  const pad = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

/** Binary (1024-based) byte formatting: "512 B", "3.4 MB", "1.2 GB". */
export function formatBytes(n: number): string {
  if (n <= 0) return '—';
  const U = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'] as const;
  let i = 0;
  let v = n;
  while (v >= 1024 && i < U.length - 1) { v /= 1024; i++; }
  return `${i === 0 ? v : v.toFixed(1)} ${U[i]}`;
}
