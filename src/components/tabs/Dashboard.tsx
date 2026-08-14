// Home / Dashboard tab.
//
// Two parts, in this order and no other: the paste field — the product — spans
// the full content column at the top and keeps its own state model, and a bento
// of four cells sits underneath it. There is deliberately ONE layout: no
// first-run / returning split, because a screen that rearranges itself as you
// use it is a screen you have to re-learn. Each cell carries its own empty
// state instead, so the same four boxes read correctly on a fresh install.
//
// The month figures come from `stats.get()` — real monthly counters kept in the
// main process. They are NOT derived from the `downloads` store: that store is
// the capped history (200, evicting), so a total built on it would start
// shrinking as the user kept downloading. The store still owns the two things
// it is authoritative for: what's in flight right now, and the recent strip.
//
// Uses only tokens defined in index.css.
import { useState, useCallback, useEffect, useRef, type ReactNode } from 'react';
import { motion, useReducedMotion } from 'framer-motion';
import { Link, GoTo, Forward, OpenFolder, Alert } from '@/components/ui/icons';
import { useAppStore } from '@/store';
import { looksLikeUrl } from '@/components/tabs/PlatformTab';
import { PLATFORMS } from '@/constants';
import type { PlatformKey, TabKey } from '@/constants';
import { Button } from '@/components/ui';
import { formatBytes, formatDuration } from '@/lib/format';
import { useTierLocks, premiumCopy } from '@/lib/tier';
import { revealFile } from '@/lib/reveal';
import type {
  DownloadStatus, DownloadTask, MonthBucket, SourcePlatform,
} from '@shared/types';

// ponytail: color-mix wraps CSS var refs so alpha suffixes (`var(...)40`) don't land in CSS
const alpha = (color: string, pct: number) =>
  `color-mix(in oklch, ${color} ${pct}%, transparent)`;

function platformToTab(platform: SourcePlatform): TabKey | null {
  switch (platform) {
    case 'youtube': return 'youtube';
    case 'spotify': return 'spotify';
    case 'soundcloud': return 'soundcloud';
    case 'instagram':
    case 'tiktok':
    case 'facebook': return 'reels';
    default: return null;
  }
}

// CSS variable strings for each platform / source
const PLATFORM_COLOR: Record<string, string> = {
  youtube: 'var(--color-youtube)',
  spotify: 'var(--color-spotify)',
  soundcloud: 'var(--color-soundcloud)',
  reels: 'var(--color-reels)',
};

const SOURCE_COLOR: Record<SourcePlatform, string> = {
  youtube: 'var(--color-youtube)',
  spotify: 'var(--color-spotify)',
  soundcloud: 'var(--color-soundcloud)',
  instagram: 'var(--color-reels)',
  tiktok: 'var(--color-reels)',
  facebook: 'var(--color-reels)',
  direct: 'var(--color-accent)',
  unknown: 'var(--color-accent)',
};

// The four sources do genuinely different jobs, so they don't get one repeated
// card template. YouTube leads: it's the only source that yields both video and
// audio, and it's where plain-text input is routed to be searched — so it gets
// the feature block and its capability list. The other three are compact rows,
// each carrying the one fact that distinguishes it (Spotify's search bridge is
// stated outright rather than hidden — it's why a Spotify grab is best-effort).
const SOURCE_COPY: Record<PlatformKey, { blurb: string; caps?: string[]; note?: string }> = {
  youtube: {
    blurb: 'The only source that gives you both the video and the audio — and where a plain search lands.',
    caps: ['Video to 4K · MP4', 'Audio to 320 kbps · FLAC', 'Playlists & channels'],
  },
  spotify: { blurb: 'Tracks, albums and playlists, tagged from Spotify’s own metadata.', note: 'matched via search' },
  soundcloud: { blurb: 'Audio only — full-quality streams, including long artist sets.' },
  reels: { blurb: 'TikTok, Reels and Shorts at original resolution, no watermark.' },
};

// Terminal statuses: not counted as "active"
const TERMINAL = new Set<DownloadStatus>(['done', 'failed', 'cancelled']);

/** 'YYYY-MM' → "August". Built from parts: `new Date('2026-08')` parses as UTC
 *  and can name the wrong month either side of the boundary. */
function monthName(month: string, style: 'long' | 'short' = 'long'): string {
  const [y, m] = month.split('-').map(Number);
  if (!Number.isFinite(y) || !Number.isFinite(m)) return month;
  return new Date(y, m - 1, 1).toLocaleString(undefined, { month: style });
}

// ── Cell shell ────────────────────────────────────────────────────────────────
// One shell, four genuinely different interiors — the shared part is the frame
// and the heading row, never the content shape. `min-w-0` is load-bearing: grid
// items default to min-width:auto, which lets a long title or a wide child push
// its track past the column and make the whole tab scroll sideways.

interface CellProps {
  title: string;
  /** Right-hand slot of the heading row — a month name, a "View all" control. */
  aside?: ReactNode;
  className?: string;
  /** Mount order: the four cells settle in sequence instead of all at once. */
  index: number;
  children: ReactNode;
}

function Cell({ title, aside, className = '', index, children }: CellProps) {
  const reduce = useReducedMotion();
  return (
    <motion.section
      initial={{ opacity: 0, y: reduce ? 0 : 8 }}
      animate={{ opacity: 1, y: 0 }}
      // Short and out-fast (expo). Reduced motion keeps the fade — a crossfade
      // still reads as "this arrived" — but drops the travel entirely.
      transition={{ duration: reduce ? 0.14 : 0.26, delay: reduce ? 0 : index * 0.045, ease: [0.16, 1, 0.3, 1] }}
      className={`min-w-0 rounded-lg border border-border-soft bg-bg-surface shadow-sm p-5 ${className}`}
    >
      <div className="flex items-center justify-between gap-3 mb-4">
        <h2 className="text-[13.5px] font-display font-semibold text-text-primary">{title}</h2>
        {aside}
      </div>
      {children}
    </motion.section>
  );
}

// ── This month ────────────────────────────────────────────────────────────────

interface StatsCellProps {
  stats: MonthStats;
  index: number;
}

/** What the dashboard knows about the counters: pending, failed, or the numbers. */
type MonthStats =
  | { state: 'loading' }
  | { state: 'failed' }
  | { state: 'ready'; current: MonthBucket; previous: MonthBucket | null };

function BarRow({ label, value, max, lead }: { label: string; value: number; max: number; lead?: boolean }) {
  const reduce = useReducedMotion();
  // A month with files must never render as an invisible sliver next to a much
  // bigger one — the floor keeps the comparison legible, not just accurate.
  const pct = value > 0 ? Math.max((value / max) * 100, 3) : 0;
  return (
    <div className="grid grid-cols-[2.6rem_minmax(0,1fr)_auto] items-center gap-3">
      <span className="font-mono text-[11px] text-text-secondary">{label}</span>
      <span className="block h-1.5 rounded-full bg-bg-hover overflow-hidden">
        <motion.span
          className="block h-full rounded-full origin-left"
          // Width is static; only scaleX animates — a growing bar must not be a
          // layout animation.
          style={{ width: `${pct}%`, background: lead ? 'var(--color-accent)' : 'var(--color-text-muted)' }}
          initial={{ scaleX: reduce ? 1 : 0 }}
          animate={{ scaleX: 1 }}
          transition={{ duration: reduce ? 0 : 0.42, delay: reduce ? 0 : 0.14, ease: [0.16, 1, 0.3, 1] }}
        />
      </span>
      <span className="font-mono text-[11px] tabular-nums text-text-secondary">{value}</span>
    </div>
  );
}

function MonthCell({ stats, index }: StatsCellProps) {
  const cur = stats.state === 'ready' ? stats.current : null;
  // A previous month with zero files is not a comparison — it's a blank.
  const prev = stats.state === 'ready' && stats.previous?.files ? stats.previous : null;
  const delta = cur && prev ? Math.round(((cur.files - prev.files) / prev.files) * 100) : null;
  const max = Math.max(cur?.files ?? 0, prev?.files ?? 0, 1);

  return (
    <Cell
      index={index}
      title="This month"
      className="@min-[1500px]:col-span-2"
      aside={cur ? <span className="font-mono text-[11px] text-text-secondary">{monthName(cur.month)}</span> : undefined}
    >
      {stats.state === 'failed' ? (
        <p className="text-[13px] text-text-secondary leading-relaxed">
          This month’s count couldn’t be read. Your downloads are unaffected.
        </p>
      ) : !cur ? (
        <p className="text-[13px] text-text-secondary">Counting…</p>
      ) : cur.files === 0 ? (
        <div className="flex flex-col gap-1.5">
          <p className="text-[14px] text-text-primary">Nothing downloaded yet this month.</p>
          <p className="text-[13px] text-text-secondary leading-relaxed max-w-[52ch]">
            {prev
              ? `Last month you kept ${prev.files} ${prev.files === 1 ? 'file' : 'files'} — ${formatBytes(prev.bytes)}.`
              : 'Paste a link above; the count starts with your first file.'}
          </p>
        </div>
      ) : (
        // Past 1500px this cell is two columns wide, so the figure and the
        // month-on-month comparison lie down side by side rather than leaving a
        // half-cell of empty surface under the number.
        <div className="flex flex-col gap-4 @min-[1500px]:flex-row @min-[1500px]:items-end @min-[1500px]:justify-between @min-[1500px]:gap-10">
          <div className="min-w-0">
            <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
              <span className="font-display text-[40px] leading-none tracking-[-0.03em] tabular-nums text-text-primary">
                {cur.files}
              </span>
              <span className="text-[15px] text-text-secondary">{cur.files === 1 ? 'file' : 'files'}</span>
              <span className="font-mono text-[13px] text-text-secondary">{formatBytes(cur.bytes)}</span>
            </div>
            <p className="mt-2 text-[12.5px] text-text-secondary">
              {prev && delta !== null ? (
                <>
                  {/* Neutral, not success/error: downloading less than last month
                      isn't a failure, so it doesn't get an alarm colour. */}
                  <span className="font-mono text-text-primary">
                    {delta > 0 ? '+' : delta < 0 ? '−' : '±'}{Math.abs(delta)}%
                  </span>
                  {` vs ${monthName(prev.month)}`}
                </>
              ) : (
                'First month on record — nothing to compare against yet.'
              )}
            </p>
          </div>
          {prev && (
            <div className="flex flex-col gap-2 @min-[1500px]:flex-1 @min-[1500px]:max-w-[46%] @min-[1500px]:min-w-[260px]">
              <BarRow label={monthName(cur.month, 'short')} value={cur.files} max={max} lead />
              <BarRow label={monthName(prev.month, 'short')} value={prev.files} max={max} />
            </div>
          )}
        </div>
      )}
    </Cell>
  );
}

// ── Where it came from ────────────────────────────────────────────────────────
// Shares are by FILE COUNT, never by bytes: one 4K video outweighs hundreds of
// MP3s, so a byte split would say "you use YouTube" about a month of Spotify.

interface SplitRow { key: string; label: string; color: string; count: number; pct: number }

const BUCKET_LABEL: Record<string, string> =
  Object.fromEntries([...PLATFORMS.map((p) => [p.key, p.label]), ['other', 'Other']]);

function splitRows(byPlatform: Partial<Record<SourcePlatform, number>>): SplitRow[] {
  // The six raw sources collapse onto the four the user actually navigates —
  // the same mapping the tabs use, so the legend can't disagree with the rail.
  const totals = new Map<string, number>();
  for (const [source, n] of Object.entries(byPlatform) as [SourcePlatform, number][]) {
    if (!n) continue;
    const key = platformToTab(source) ?? 'other';
    totals.set(key, (totals.get(key) ?? 0) + n);
  }
  const total = [...totals.values()].reduce((s, n) => s + n, 0);
  if (!total) return [];
  return [...totals]
    .map(([key, count]) => ({
      key,
      label: BUCKET_LABEL[key] ?? 'Other',
      color: PLATFORM_COLOR[key] ?? 'var(--color-text-muted)',
      count,
      pct: (count / total) * 100,
    }))
    .sort((a, b) => b.count - a.count);
}

function SplitCell({ stats, index }: StatsCellProps) {
  const reduce = useReducedMotion();
  const rows = stats.state === 'ready' ? splitRows(stats.current.byPlatform) : [];

  return (
    <Cell
      index={index}
      title="Where it came from"
      // Says outright what the bar measures — a proportion bar that doesn't
      // name its unit is a bar you have to guess at.
      aside={rows.length > 0 ? <span className="text-[11.5px] text-text-secondary">by file count</span> : undefined}
    >
      {stats.state === 'failed' ? (
        <p className="text-[13px] text-text-secondary leading-relaxed">The platform split isn’t available right now.</p>
      ) : stats.state === 'loading' ? (
        <p className="text-[13px] text-text-secondary">Counting…</p>
      ) : rows.length === 0 ? (
        <div className="flex flex-col gap-3">
          {/* The empty rail keeps the cell's shape, so the bar doesn't appear
              from nowhere the moment the first file lands. */}
          <span aria-hidden className="block h-2.5 rounded-full bg-bg-hover" />
          <p className="text-[13px] text-text-secondary leading-relaxed max-w-[52ch]">
            Once files start landing, this splits them across YouTube, Spotify, SoundCloud and short-form.
          </p>
        </div>
      ) : (
        <>
          <div
            className="flex gap-[2px] h-2.5"
            role="img"
            aria-label={`By file count: ${rows.map((r) => `${r.label} ${Math.round(r.pct)} percent`).join(', ')}`}
          >
            {rows.map((r, i) => (
              <motion.span
                key={r.key}
                className="block h-full rounded-full origin-left min-w-[4px]"
                style={{ flexBasis: `${r.pct}%`, background: r.color }}
                initial={{ scaleX: reduce ? 1 : 0 }}
                animate={{ scaleX: 1 }}
                transition={{ duration: reduce ? 0 : 0.38, delay: reduce ? 0 : 0.1 + i * 0.05, ease: [0.16, 1, 0.3, 1] }}
              />
            ))}
          </div>
          <ul className="mt-4 flex flex-col gap-2">
            {rows.map((r) => (
              <li key={r.key} className="flex items-center gap-2.5 text-[12.5px]">
                <span aria-hidden className="w-2 h-2 rounded-[3px] shrink-0" style={{ background: r.color }} />
                <span className="min-w-0 flex-1 truncate text-text-secondary">{r.label}</span>
                <span className="font-mono text-[11.5px] tabular-nums text-text-primary">{r.count}</span>
                <span className="font-mono text-[11.5px] tabular-nums text-text-secondary w-9 text-right">
                  {Math.round(r.pct)}%
                </span>
              </li>
            ))}
          </ul>
        </>
      )}
    </Cell>
  );
}

// ── Source entries ────────────────────────────────────────────────────────────
// Inside a cell these are flat rows, not cards: a bordered card inside a
// bordered cell is a nested box, and the frame is already doing that job.
// Affordance comes from the hover wash and the arrow, not from a second border.

interface SourceProps {
  def: (typeof PLATFORMS)[number];
  onOpen: () => void;
}

function SourceFeature({ def, onOpen }: SourceProps) {
  const color = PLATFORM_COLOR[def.key] ?? 'var(--color-accent)';
  const copy = SOURCE_COPY[def.key];
  const Icon = def.icon;
  return (
    <button
      type="button"
      onClick={onOpen}
      className="no-drag group relative flex flex-col rounded-md p-3 -m-1 text-left hover:bg-bg-hover transition-colors duration-150 cursor-pointer"
    >
      <GoTo
        size={16}
        className="absolute top-3 right-3 text-text-muted transition-transform duration-200 group-hover:translate-x-0.5 group-hover:-translate-y-0.5"
      />
      <span className="flex items-center gap-2.5 mb-2">
        <Icon size={22} style={{ color }} />
        <span className="text-[17px] font-display font-semibold text-text-primary">{def.label}</span>
      </span>
      <span className="block text-[13px] text-text-secondary leading-relaxed max-w-[42ch]">{copy.blurb}</span>
      {/* Past ~1500px of content column the feature block is wide enough that a
          stacked list leaves a column of dead space; the capabilities lie down
          in a row instead, so the extra width buys shape rather than emptiness. */}
      <span className="mt-4 pt-3 border-t border-border-soft flex flex-col gap-1.5 @min-[1500px]:flex-row @min-[1500px]:flex-wrap @min-[1500px]:gap-x-7">
        {copy.caps?.map((c) => (
          <span key={c} className="flex items-center gap-2 text-[12px] text-text-secondary">
            <span aria-hidden className="w-1 h-1 rounded-full shrink-0" style={{ background: color }} />
            {c}
          </span>
        ))}
      </span>
    </button>
  );
}

// min-w-0 on the button: as a grid item its automatic minimum is min-content
// (icon + label + note + arrow ≈ 473px), which outvotes w-full and pushes the
// track past the content column at 960. The truncates inside do the rest.
function SourceRow({ def, onOpen }: SourceProps) {
  const color = PLATFORM_COLOR[def.key] ?? 'var(--color-accent)';
  const copy = SOURCE_COPY[def.key];
  const Icon = def.icon;
  return (
    <button
      type="button"
      onClick={onOpen}
      className="no-drag group flex items-center gap-3.5 w-full min-w-0 rounded-md px-3 py-3 text-left hover:bg-bg-hover transition-colors duration-150 cursor-pointer"
    >
      <span
        aria-hidden
        className="flex items-center justify-center w-9 h-9 rounded-md shrink-0"
        style={{ background: alpha(color, 12) }}
      >
        <Icon size={18} style={{ color }} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="flex items-baseline gap-2 min-w-0">
          <span className="text-[14px] font-display font-semibold text-text-primary">{def.label}</span>
          {copy.note && (
            <span className="font-mono text-[10.5px] text-text-secondary truncate">{copy.note}</span>
          )}
        </span>
        <span className="block text-[12px] text-text-secondary leading-snug truncate">{copy.blurb}</span>
      </span>
      <GoTo
        size={15}
        className="text-text-muted shrink-0 transition-transform duration-200 group-hover:translate-x-0.5 group-hover:-translate-y-0.5"
      />
    </button>
  );
}

// ── RecentTile ────────────────────────────────────────────────────────────────

interface RecentTileProps {
  task: DownloadTask;
  onQueue: () => void;
  className?: string;
}

function RecentTile({ task, onQueue, className = '' }: RecentTileProps) {
  const reduce = useReducedMotion();
  const color = SOURCE_COLOR[task.source] ?? 'var(--color-accent)';
  const filePath = task.progress.filename;

  // The overlay sits on artwork, which is theme-independent — so its scrim and
  // ink are fixed values rather than theme tokens, which would invert to dark
  // text on a dark scrim in the light theme.
  const overlayInk = 'var(--color-on-media)';

  return (
    // The lift is the tile's only hover motion; the overlay it reveals is a
    // crossfade, which is legible with or without the travel.
    <motion.div whileHover={reduce ? undefined : { y: -3 }} className={`group min-w-0 ${className}`}>
      <div className="relative aspect-[16/10] rounded-md border border-border-soft overflow-hidden shadow-sm">
        {task.thumbnailUrl ? (
          <img
            src={task.thumbnailUrl}
            alt=""
            className="w-full h-full object-cover"
            loading="lazy"
          />
        ) : (
          <div
            className="w-full h-full"
            style={{ background: `linear-gradient(135deg, ${alpha(color, 55)}, var(--color-bg-tertiary))` }}
          />
        )}

        {/* Resting chips — they step aside for the actions. */}
        <span
          aria-hidden
          className="absolute top-2 left-2 w-2 h-2 rounded-full transition-opacity duration-150 group-hover:opacity-0 group-focus-within:opacity-0"
          style={{ background: color, boxShadow: `0 0 7px ${color}` }}
        />
        {task.duration > 0 && (
          <span className="absolute bottom-2 right-2 font-mono text-[9.5px] text-text-primary bg-bg-primary/[0.78] rounded px-1.5 py-0.5 transition-opacity duration-150 group-hover:opacity-0 group-focus-within:opacity-0">
            {formatDuration(task.duration)}
          </span>
        )}

        {/* Identify + act. Revealed by hover AND by keyboard focus landing on
            either button, so the actions are never hover-only. */}
        <div
          className="absolute inset-0 flex flex-col justify-end gap-2 p-2 opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100"
          style={{ background: 'var(--color-media-scrim)' }}
        >
          <p
            className="text-[11.5px] leading-snug line-clamp-2"
            style={{ color: overlayInk }}
          >
            {task.title || 'Untitled'}
            {task.uploader && (
              <span className="block opacity-70 truncate">{task.uploader}</span>
            )}
          </p>
          <div className="flex items-center gap-1.5">
            {/* Deliberately not disabled when the path is missing: a dead button
                explains nothing, and a file can vanish after the button renders.
                Clicking always answers — with the folder, or with the reason. */}
            <button
              type="button"
              onClick={() => void revealFile(filePath, task.title)}
              className="no-drag inline-flex items-center gap-1.5 h-7 px-2 rounded text-[11px] font-medium bg-accent text-on-accent hover:bg-accent-hover transition-colors duration-150"
            >
              <OpenFolder size={13} />
              Open
            </button>
            <button
              type="button"
              onClick={onQueue}
              aria-label={`Show “${task.title || 'this download'}” in Downloads`}
              className="no-drag inline-flex items-center justify-center h-7 px-2 rounded border text-[11px] transition-colors duration-150"
              style={{ color: overlayInk, borderColor: alpha(overlayInk, 30) }}
            >
              Details
            </button>
          </div>
        </div>
      </div>

      {/* Meta */}
      <div className="mt-2.5 px-0.5">
        <p className="text-[12.5px] font-medium text-text-primary leading-snug truncate">
          {task.title || 'Untitled'}
        </p>
        <div className="flex justify-between gap-2 mt-1 font-mono text-[10.5px] text-text-secondary">
          <span className="truncate" style={{ color }}>{task.format.toUpperCase()}</span>
          <span className="shrink-0">{formatBytes(task.progress.downloaded)}</span>
        </div>
      </div>
    </motion.div>
  );
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

export function Dashboard() {
  const downloads = useAppStore((s) => s.downloads);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const setPendingInput = useAppStore((s) => s.setPendingInput);
  const locks = useTierLocks();
  const reduce = useReducedMotion();

  const [url, setUrl] = useState('');
  const [detecting, setDetecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const [stats, setStats] = useState<MonthStats>({ state: 'loading' });
  // dragenter/dragleave fire for every child crossed, so depth-count instead of
  // toggling — otherwise the highlight flickers off over the field's own icon.
  const dragDepth = useRef(0);

  // Counters are a snapshot, not a stream: read once on mount. A number that
  // ticks while you look at it invites you to watch it, and this screen's job
  // is to get you to the field.
  useEffect(() => {
    let alive = true;
    void (async () => {
      try {
        const s = await window.electronAPI.stats.get();
        if (alive) setStats({ state: 'ready', current: s.current, previous: s.previous });
      } catch {
        if (alive) setStats({ state: 'failed' });
      }
    })();
    return () => { alive = false; };
  }, []);

  const allTasks = Object.values(downloads);
  const activeCount = allTasks.filter((t) => !TERMINAL.has(t.progress.status)).length;
  // Most recent first. Eight is the widest row the 3-column grid shows; the
  // narrower tiers hide the tail rather than wrap it into a ragged second row.
  const recentTiles = [...allTasks].filter((t) => t.progress.status === 'done').reverse().slice(0, 8);

  // What the field is holding, read locally — no detection call is made until
  // the user commits, so this stays a pure render of the typed text.
  const trimmed = url.trim();
  const tokens = trimmed ? trimmed.split(/\s+/).filter(Boolean) : [];
  const linkCount = tokens.length > 1 && tokens.every(looksLikeUrl) ? tokens.length : 0;
  const overBatchLimit = locks.batchLimit !== null && linkCount > locks.batchLimit;

  // Batch: several links at once (drop / multi-line paste) download straight
  // to the queue with saved defaults — batching is the express lane.
  const batchDownload = useCallback(async (urls: string[]) => {
    const cfg = useAppStore.getState().config;
    // Nothing is queued when the batch is over the free ceiling — not even the
    // first N. Dropping the tail silently decides for the user; refusing the whole
    // batch leaves their paste intact so they can trim it themselves or upgrade.
    // The field's Fetch button is already disabled in that state; this guard is
    // for the paths that bypass the field — a drag-and-drop, or Ctrl+V.
    if (locks.batchLimit !== null && urls.length > locks.batchLimit) {
      locks.nudge(premiumCopy.batch());
      return;
    }
    let started = 0;
    for (const u of urls) {
      try {
        const det = await window.electronAPI.url.detect(u);
        if (det.platform === 'unknown') continue;
        // Spotify needs its scraped track to drive the ytsearch bridge.
        let track;
        if (det.platform === 'spotify') {
          track = (await window.electronAPI.spotify.fetchTrack(u)) ?? undefined;
          if (!track) continue; // collections go through the preview flow, not batch
        }
        const audioOnly = det.platform === 'spotify' || det.platform === 'soundcloud';
        const task = await window.electronAPI.download.start({
          url: det.url,
          source: det.platform,
          isPlaylist: det.isCollection,
          isAudioOnly: audioOnly,
          format: audioOnly ? (cfg?.defaultFormat ?? 'mp3') : 'mp4',
          quality: cfg?.defaultQuality ?? '320',
          videoQuality: cfg?.defaultVideoQuality ?? 'best',
          embedThumbnail: cfg?.embedThumbnail ?? true,
          embedMetadata: cfg?.embedMetadata ?? true,
          skipExisting: cfg?.skipExisting ?? true,
          track,
        });
        useAppStore.getState().addDownload(task);
        started++;
      } catch { /* skip the bad link, keep batching */ }
    }
    if (started > 0) setActiveTab('queue');
    else setError('No downloadable links found.');
  }, [setActiveTab, locks]);

  const handleFetch = useCallback(async (override?: string) => {
    const input = (override ?? url).trim();
    if (!input) return;

    // Several links pasted at once → batch them.
    const parts = input.split(/\s+/).filter(Boolean);
    if (parts.length > 1 && parts.every(looksLikeUrl)) {
      setDetecting(true);
      setError(null);
      await batchDownload(parts);
      setDetecting(false);
      return;
    }

    // Plain text isn't an error — it's a search. Hand it to the YouTube tab.
    if (!looksLikeUrl(input)) {
      setPendingInput(input);
      setActiveTab('youtube');
      return;
    }

    setDetecting(true);
    setError(null);
    try {
      const detection = await window.electronAPI.url.detect(input);
      const tab = platformToTab(detection.platform);
      if (tab) {
        setPendingInput(input); // hand the link to the tab so it auto-fetches on arrival
        setActiveTab(tab);
      } else {
        setError('That link isn’t from a supported platform — YouTube, Spotify, SoundCloud, TikTok, Reels or Shorts.');
      }
    } catch {
      // url.detect is local pattern-matching — failure means bad input, not network.
      setError('That link couldn’t be read — check it and try again.');
    } finally {
      setDetecting(false);
    }
  }, [url, setActiveTab, setPendingInput, batchDownload]);

  // "Ctrl+V to paste & fetch instantly" — when focus isn't already in a text field,
  // read the clipboard and fetch it. Inside an input, the browser's native paste wins.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey || (e.key !== 'v' && e.key !== 'V')) return;
      const tag = (document.activeElement as HTMLElement | null)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA') return;
      void navigator.clipboard.readText().then((text) => {
        const t = text.trim();
        if (t) { setUrl(t); void handleFetch(t); }
      }).catch(() => { /* clipboard blocked — ignore */ });
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [handleFetch]);

  const endDrag = () => { dragDepth.current = 0; setDragging(false); };

  // One status line, four readings — the field never grows a second hint row.
  const status = (() => {
    if (error) {
      return (
        <span className="inline-flex items-start gap-1.5 text-error">
          <Alert size={14} className="shrink-0 mt-px" />
          {error}
        </span>
      );
    }
    if (dragging) return <span className="text-accent">Drop to queue every link you’re holding</span>;
    if (detecting) {
      return <span className="text-text-secondary">{linkCount > 1 ? `Queueing ${linkCount} links…` : 'Reading the link…'}</span>;
    }
    // Over the free ceiling, the field stops rather than quietly queueing a
    // subset: pasting eight links and getting five back — with the explanation
    // arriving after the fact — feels like the app decided for you. Blocking up
    // front leaves the paste intact and puts the choice in the user's hands.
    if (overBatchLimit) {
      return (
        <span className="inline-flex items-center gap-2 flex-wrap text-warning">
          {premiumCopy.batch()}
          <button
            type="button"
            onClick={locks.openUpgrade}
            className="no-drag underline underline-offset-2 text-accent hover:text-accent-hover transition-colors"
          >
            See plans
          </button>
        </span>
      );
    }
    if (linkCount > 1) {
      return <span className="text-text-secondary">{linkCount} links — Enter queues them all</span>;
    }
    if (trimmed && !looksLikeUrl(trimmed)) {
      return <span className="text-text-secondary">Enter searches YouTube for this</span>;
    }
    return (
      <span className="text-text-secondary">
        <kbd className="font-mono text-[10.5px] bg-bg-surface border border-border rounded px-1.5 py-0.5 text-text-secondary">Ctrl</kbd>
        {' + '}
        <kbd className="font-mono text-[10.5px] bg-bg-surface border border-border rounded px-1.5 py-0.5 text-text-secondary">V</kbd>
        {' pastes and fetches from anywhere — drop a handful of links to batch them'}
      </span>
    );
  })();

  return (
    // Content column, measured against `<main>` (the @container in App.tsx) so
    // the rail and the scrollbar are already subtracted.
    //   · The ceiling is 1720px, not 1240px: below that the screen is edge-to-edge,
    //     so a 1920 window (~1835px column) reads as filled rather than as a
    //     narrow strip marooned in canvas.
    //   · Padding steps with the column instead of holding at 40px, so the 960px
    //     minimum spends 32px a side on chrome rather than 40.
    //   · Short windows: 640px tall is the enforced minimum, and 80px of vertical
    //     padding there is 13% of the viewport — trimmed so the grid starts
    //     peeking above the fold instead of being invisible.
    <div
      className="w-full max-w-[1720px] mx-auto px-8 @min-[1100px]:px-10 @min-[1600px]:px-14 py-10 [@media(max-height:720px)]:py-7"
      onDragEnter={() => { dragDepth.current += 1; setDragging(true); }}
      onDragLeave={() => { dragDepth.current -= 1; if (dragDepth.current <= 0) endDrag(); }}
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => {
        e.preventDefault();
        endDrag();
        const urls = e.dataTransfer.getData('text').split(/\s+/).filter(looksLikeUrl);
        if (urls.length) void batchDownload(urls);
      }}
    >
      {/* ── Header ────────────────────────────────────────────────────────────
          One headline for every state. The only thing that appears and
          disappears is the live indicator, because "in flight: none" is not
          information — the totals it used to sit beside now live in the This
          month cell, where each number has exactly one home. */}
      <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 mb-6">
        <h1 className="font-display font-semibold text-[30px] @min-[1240px]:text-[34px] [@media(max-height:720px)]:text-[26px] leading-[1.05] tracking-[-0.03em] text-text-primary">
          Paste a link. Get the <em className="not-italic text-accent">file.</em>
        </h1>
        {activeCount > 0 && (
          <button
            type="button"
            onClick={() => setActiveTab('queue')}
            className="no-drag inline-flex items-center gap-2.5 h-9 pl-3 pr-3.5 rounded-md border border-border-soft bg-bg-secondary shadow-sm hover:bg-bg-hover transition-colors duration-150"
          >
            <motion.span
              aria-hidden
              className="w-1.5 h-1.5 rounded-full shrink-0"
              style={{ background: 'var(--color-accent)' }}
              // The one thing on this screen that says "work is happening right
              // now". A static dot still says "active" when motion is reduced.
              animate={reduce ? { opacity: 1 } : { opacity: [1, 0.35, 1] }}
              transition={reduce ? { duration: 0 } : { duration: 1.8, repeat: Infinity, ease: 'easeInOut' }}
            />
            <span className="text-[13px] text-text-primary">
              <span className="font-mono tabular-nums">{activeCount}</span> in flight
            </span>
            <Forward size={13} className="text-text-muted" />
          </button>
        )}
      </div>

      {/* ── The paste field — the primary object on the screen ─────────────────
          Full width: it is not one of the bento cells and never sits in the
          grid. Its whole state model reads on one strip along the bottom edge:
          nothing at rest, a sweeping phosphor trace while detecting, a steady
          rule on error. That keeps focus (the ring), state (the strip) and
          explanation (the status line) on three separate channels. */}
      <div
        className={`field-shell relative overflow-hidden flex items-center gap-3 h-[62px] pl-5 pr-2.5 rounded-lg border transition-[border-color,box-shadow,background-color] duration-150 ${
          dragging ? 'border-accent bg-accent-soft' : 'border-border bg-bg-tertiary'
        }`}
      >
        <Link
          size={20}
          className={`shrink-0 transition-colors duration-150 ${trimmed ? 'text-accent' : 'text-text-muted'}`}
        />
        <input
          className="no-drag flex-1 min-w-0 bg-transparent outline-none text-text-primary placeholder:text-text-muted text-[16px]"
          placeholder="Paste a YouTube, Spotify, SoundCloud or Reels link…"
          aria-label="Paste a link"
          aria-invalid={!!error}
          aria-describedby="paste-status"
          value={url}
          onChange={(e) => { setUrl(e.target.value); if (error) setError(null); }}
          onKeyDown={(e) => { if (e.key === 'Enter') void handleFetch(); }}
          onPaste={(e) => {
            const text = e.clipboardData.getData('text').trim();
            if (text) { setUrl(text); void handleFetch(text); }
          }}
          spellCheck={false}
          autoComplete="off"
          autoFocus
        />

        {/* A multi-link paste is an object, not a hint — it gets a count in the field. */}
        {linkCount > 1 && (
          <span
            className={`shrink-0 font-mono text-[10.5px] tracking-[0.06em] rounded px-2 py-1 ${
              overBatchLimit ? 'text-warning' : 'text-accent'
            }`}
            style={{ background: alpha(overBatchLimit ? 'var(--color-warning)' : 'var(--color-accent)', 14) }}
          >
            {linkCount} LINKS
          </span>
        )}

        {/* Dimmed, not hidden: the button staying in place (and the paste staying
            in the field) says "this is one step away", where a vanished control
            would say "you did something wrong". The status line names the limit
            and carries the way out. */}
        <Button
          size="md"
          onClick={() => void handleFetch()}
          loading={detecting}
          disabled={overBatchLimit}
          title={overBatchLimit ? premiumCopy.batch() : undefined}
          icon={Forward}
          className="shrink-0"
        >
          Fetch
        </Button>

        <span aria-hidden className="absolute inset-x-0 bottom-0 h-[2px] overflow-hidden">
          {error && <span className="block h-full w-full bg-error" />}
          {detecting && (reduce ? (
            <span className="block h-full w-full bg-accent opacity-60" />
          ) : (
            <motion.span
              className="block h-full w-1/4 bg-accent"
              animate={{ x: ['-100%', '400%'] }}
              transition={{ duration: 1.1, ease: 'linear', repeat: Infinity }}
            />
          ))}
        </span>
      </div>

      {/* The field spans the column, but its caption is prose: capped for
          reading so the last word isn't half a screen from the field. */}
      <p id="paste-status" role="status" aria-live="polite" className="mt-3 max-w-[80ch] text-xs leading-relaxed min-h-[1.25rem]">
        {status}
      </p>

      {/* ── The bento ─────────────────────────────────────────────────────────
          One grid for everyone; the cells carry their own empty states.
          Column counts are keyed to `<main>`, not the viewport — the fixed 68px
          rail makes viewport breakpoints fire a rail's width early.
            ·  <1000px (960px window ⇒ ~875px column): ONE column, everything
               stacked. Chosen over pairing tiles: at that width a pair puts two
               half-legible cells side by side instead of one readable one.
            · ≥1000px: 2 columns — the two counters pair, Recent and Sources
               take the full row underneath.
            · ≥1500px (1920 window ⇒ ~1835px column): 3 columns — This month
               widens to two tracks and lays its comparison out beside the
               figure, and Recent shows eight tiles instead of five. Width buys
               content, not gutters. */}
      <div className="mt-8 grid gap-4 @min-[1000px]:grid-cols-2 @min-[1500px]:grid-cols-3">
        <MonthCell stats={stats} index={0} />
        <SplitCell stats={stats} index={1} />

        <Cell
          index={2}
          title="Recent downloads"
          className="@min-[1000px]:col-span-2 @min-[1500px]:col-span-3"
          aside={recentTiles.length > 0 ? (
            <button
              type="button"
              className="no-drag text-[12.5px] text-text-secondary hover:text-text-primary inline-flex items-center gap-1 transition-colors duration-150"
              onClick={() => setActiveTab('queue')}
            >
              View all <Forward size={13} />
            </button>
          ) : undefined}
        >
          {recentTiles.length === 0 ? (
            <p className="text-[13px] text-text-secondary leading-relaxed max-w-[62ch]">
              Finished downloads land here, newest first — each one a click away from the file on disk.
            </p>
          ) : (
            // A fitted grid, never a horizontal scroller: the tile track is what
            // used to run past the right edge of the window. The tail is hidden
            // rather than wrapped, so every tier shows one full row.
            <div className="grid gap-3 grid-cols-4 @min-[1000px]:grid-cols-5 @min-[1500px]:grid-cols-8">
              {recentTiles.map((task, i) => (
                <RecentTile
                  key={task.taskId}
                  task={task}
                  onQueue={() => setActiveTab('queue')}
                  className={i >= 5 ? 'hidden @min-[1500px]:block' : i >= 4 ? 'hidden @min-[1000px]:block' : ''}
                />
              ))}
            </div>
          )}
        </Cell>

        <Cell
          index={3}
          title="Sources to start from"
          className="@min-[1000px]:col-span-2 @min-[1500px]:col-span-3"
        >
          {/* Not four identical cards: YouTube is the only source that yields
              both video and audio and the one that absorbs a plain search, so it
              gets the feature block; the other three are rows carrying the one
              fact that separates them. Below ~860px they stack; the pair holds
              at the minimum window because stacking costs ~190px of scroll on a
              640px-tall screen, and the rows already truncate. */}
          <div className="grid gap-3 @min-[860px]:grid-cols-[minmax(0,1.05fr)_minmax(0,1fr)] @min-[1500px]:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)]">
            <SourceFeature def={PLATFORMS[0]} onOpen={() => setActiveTab(PLATFORMS[0].key)} />
            <div className="grid gap-1 content-between @min-[860px]:border-l @min-[860px]:border-border-soft @min-[860px]:pl-4">
              {PLATFORMS.slice(1).map((p) => (
                <SourceRow key={p.key} def={p} onOpen={() => setActiveTab(p.key)} />
              ))}
            </div>
          </div>
        </Cell>
      </div>
    </div>
  );
}
