// Home / Dashboard tab — asymmetric hero + platform cards + recent downloads strip.
// Matches concept-a-aurora-graphite visual language; uses only tokens defined in index.css.
import { useState, useCallback, useEffect } from 'react';
import { motion } from 'framer-motion';
import { Link2, ArrowUpRight, ArrowRight } from 'lucide-react';
import { useAppStore } from '@/store';
import { looksLikeUrl } from '@/components/tabs/PlatformTab';
import { PLATFORMS } from '@/constants';
import type { TabKey } from '@/constants';
import { Button } from '@/components/ui';
import { formatBytes, formatDuration } from '@/lib/format';
import { useTierLocks } from '@/lib/tier';
import type { DownloadStatus, DownloadTask, SourcePlatform } from '@shared/types';

// ponytail: color-mix wraps CSS var refs so alpha suffixes (`var(...)40`) don't land in CSS
const alpha = (color: string, pct: number) =>
  `color-mix(in oklch, ${color} ${pct}%, transparent)`;

// Instrument-readout padding: telemetry counts show two digits (ACTIVE 02).
const pad2 = (n: number) => String(n).padStart(2, '0');

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

const PLATFORM_DESC: Record<string, string> = {
  youtube: 'Video & audio up to 4K · MP3 320 · FLAC',
  spotify: 'Tracks, albums & playlists with metadata',
  soundcloud: 'Full quality streams & artist sets',
  reels: 'TikTok, Reels & Shorts — no watermark',
};

// Terminal statuses: not counted as "active"
const TERMINAL = new Set<DownloadStatus>(['done', 'failed', 'cancelled']);

// ── RecentTile ────────────────────────────────────────────────────────────────

interface RecentTileProps {
  task: DownloadTask;
  onOpen: () => void;
}

function RecentTile({ task, onOpen }: RecentTileProps) {
  const color = SOURCE_COLOR[task.source] ?? 'var(--color-accent)';
  return (
    <motion.button
      whileHover={{ y: -3 }}
      onClick={onOpen}
      className="no-drag flex-none w-40 snap-start text-left"
    >
      {/* Thumbnail */}
      <div className="relative h-[92px] rounded-md border border-border-soft overflow-hidden shadow-sm">
        {task.thumbnailUrl ? (
          <img
            src={task.thumbnailUrl}
            alt={task.title}
            className="w-full h-full object-cover"
            loading="lazy"
          />
        ) : (
          <div
            className="w-full h-full"
            style={{
              background: `linear-gradient(135deg, ${alpha(color, 55)}, var(--color-bg-tertiary))`,
            }}
          />
        )}
        {/* Platform dot */}
        <span
          className="absolute top-2 left-2 w-2 h-2 rounded-full"
          style={{ background: color, boxShadow: `0 0 7px ${color}` }}
        />
        {/* Format tag */}
        <span className="absolute bottom-2 left-2 font-mono text-[9.5px] tracking-wide text-text-primary bg-bg-primary/[0.78] backdrop-blur-sm border border-border-soft rounded px-1.5 py-0.5">
          {task.format.toUpperCase() || '—'}
        </span>
        {/* Duration */}
        {task.duration > 0 && (
          <span className="absolute bottom-2 right-2 font-mono text-[9.5px] text-text-primary bg-bg-primary/[0.78] backdrop-blur-sm rounded px-1.5 py-0.5">
            {formatDuration(task.duration)}
          </span>
        )}
      </div>

      {/* Meta */}
      <div className="mt-2.5 px-0.5">
        <p className="text-[12.5px] font-medium text-text-primary leading-snug truncate">
          {task.title || 'Untitled'}
        </p>
        <div className="flex justify-between mt-1 font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted">
          <span style={{ color }}>{task.format.toUpperCase()}</span>
          <span>{formatBytes(task.progress.downloaded)}</span>
        </div>
      </div>
    </motion.button>
  );
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

export function Dashboard() {
  const downloads = useAppStore((s) => s.downloads);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const setPendingInput = useAppStore((s) => s.setPendingInput);
  const locks = useTierLocks();

  const [url, setUrl] = useState('');
  const [detecting, setDetecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const allTasks = Object.values(downloads);
  const doneTasks = allTasks.filter((t) => t.progress.status === 'done');
  const activeCount = allTasks.filter((t) => !TERMINAL.has(t.progress.status)).length;
  // most recent first, capped at 8
  const recentTiles = [...doneTasks].reverse().slice(0, 8);
  const totalBytes = doneTasks.reduce((s, t) => s + t.progress.downloaded, 0);

  // Batch: several links at once (drop / multi-line paste) download straight
  // to the queue with saved defaults — batching is the express lane.
  const batchDownload = useCallback(async (urls: string[]) => {
    const cfg = useAppStore.getState().config;
    // Basic queues the first N and says so — the rest are dropped, never silently.
    const accepted = locks.batchLimit === null ? urls : urls.slice(0, locks.batchLimit);
    if (accepted.length < urls.length) {
      locks.nudge(
        `Basic queues ${locks.batchLimit} links at a time — ${urls.length - accepted.length} were left out.`,
      );
    }
    let started = 0;
    for (const u of accepted) {
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
    const trimmed = (override ?? url).trim();
    if (!trimmed) return;

    // Several links pasted at once → batch them.
    const tokens = trimmed.split(/\s+/).filter(Boolean);
    if (tokens.length > 1 && tokens.every(looksLikeUrl)) {
      setDetecting(true);
      setError(null);
      await batchDownload(tokens);
      setDetecting(false);
      return;
    }

    // Plain text isn't an error — it's a search. Hand it to the YouTube tab.
    if (!looksLikeUrl(trimmed)) {
      setPendingInput(trimmed);
      setActiveTab('youtube');
      return;
    }

    setDetecting(true);
    setError(null);
    try {
      const detection = await window.electronAPI.url.detect(trimmed);
      const tab = platformToTab(detection.platform);
      if (tab) {
        setPendingInput(trimmed); // hand the link to the tab so it auto-fetches on arrival
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

  return (
    <div
      className="max-w-[1240px] 2xl:max-w-[1480px] mx-auto px-10 2xl:px-16 py-10"
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => {
        e.preventDefault();
        const urls = e.dataTransfer.getData('text').split(/\s+/).filter(looksLikeUrl);
        if (urls.length) void batchDownload(urls);
      }}
    >

      {/* ── Hero — single column, instrument readout ─────────────────────────── */}
      <div className="mb-12">

        {/* Mono telemetry strip — the rack readout */}
        <div className="flex items-center gap-2.5 font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted mb-8">
          <span>Active <b className="ml-0.5 font-medium text-text-primary">{pad2(activeCount)}</b></span>
          <span className="opacity-40">·</span>
          <span>Done <b className="ml-0.5 font-medium text-text-primary">{pad2(doneTasks.length)}</b></span>
          <span className="opacity-40">·</span>
          <span>Data <b className="ml-0.5 font-medium text-text-primary">{formatBytes(totalBytes)}</b></span>
        </div>

        <h1 className="font-display font-semibold text-[44px] min-[1280px]:text-[56px] leading-[1.0] tracking-[-0.035em] text-text-primary mb-4">
          Paste a link.<br />
          Get the <em className="not-italic text-accent">file.</em>
        </h1>

        <p className="text-text-secondary text-sm leading-relaxed max-w-[52ch] mb-7">
          One field for every platform — YouTube, Spotify, SoundCloud and short-form video.
          We detect the source and pick the right options.
        </p>

        {/* Paste field — flat well, no glass, no glow (glow lives on the carrier line) */}
        <div className="field-shell flex items-center gap-3 h-14 max-w-[640px] pl-4 pr-2 rounded-lg bg-bg-tertiary border border-border transition-[border-color,box-shadow] duration-150">
          <Link2 size={20} className="text-text-muted shrink-0" />
          <input
            className="no-drag flex-1 min-w-0 bg-transparent outline-none text-text-primary placeholder:text-text-muted text-[15px]"
            placeholder="Paste a YouTube, Spotify, SoundCloud or Reels link…"
            aria-label="Paste a link"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onKeyDown={(e) => { if (e.key === 'Enter') void handleFetch(); }}
            onPaste={(e) => {
              const text = e.clipboardData.getData('text').trim();
              if (text) { setUrl(text); void handleFetch(text); }
            }}
            spellCheck={false}
            autoComplete="off"
            autoFocus
          />
          <Button
            size="md"
            onClick={() => void handleFetch()}
            loading={detecting}
            icon={ArrowRight}
            className="shrink-0"
          >
            Fetch
          </Button>
        </div>

        {error && <p className="mt-2 text-xs text-error">{error}</p>}

        <div className="flex items-center gap-3 mt-3 text-xs text-text-muted">
          <span>
            <kbd className="font-mono text-[10.5px] bg-bg-surface border border-border rounded px-1.5 py-0.5 text-text-secondary">
              Ctrl
            </kbd>
            {' + '}
            <kbd className="font-mono text-[10.5px] bg-bg-surface border border-border rounded px-1.5 py-0.5 text-text-secondary">
              V
            </kbd>
            {' to paste & fetch instantly'}
          </span>
          <span className="opacity-40">·</span>
          <span>Drop multiple links to batch them</span>
        </div>
      </div>

      {/* ── Platform cards ────────────────────────────────────────────────────── */}
      <h3 className="text-base font-display font-semibold text-text-primary mb-4">
        Choose a source
      </h3>
      <div className="grid grid-cols-2 min-[1280px]:grid-cols-4 gap-4 mb-10">
        {PLATFORMS.map((p) => {
          const color = PLATFORM_COLOR[p.key] ?? 'var(--color-accent)';
          const Icon = p.icon;
          return (
            <motion.button
              key={p.key}
              whileHover={{ y: -3 }}
              whileTap={{ scale: 0.98 }}
              onClick={() => setActiveTab(p.key)}
              className="no-drag group relative overflow-hidden rounded-lg p-5 text-left bg-bg-surface border border-border-soft shadow-sm hover:border-border transition-colors cursor-pointer"
            >
              {/* 2px platform-colour identity bar — hairline, not colour-wash */}
              <span
                aria-hidden
                className="absolute inset-x-0 top-0 h-0.5"
                style={{ background: color }}
              />

              {/* Arrow */}
              <ArrowUpRight
                size={16}
                className="absolute top-4 right-4 text-text-muted transition-transform duration-200 group-hover:translate-x-0.5 group-hover:-translate-y-0.5"
              />

              {/* Bare platform-colored icon inline with the heading — no chip */}
              <div className="flex items-center gap-2.5 mb-2">
                <Icon size={19} strokeWidth={1.9} style={{ color }} />
                <h4 className="text-[15px] font-display font-semibold text-text-primary">
                  {p.label}
                </h4>
              </div>
              <p className="text-xs text-text-secondary leading-relaxed">
                {PLATFORM_DESC[p.key]}
              </p>
            </motion.button>
          );
        })}
      </div>

      {/* ── Recently downloaded ───────────────────────────────────────────────── */}
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-base font-display font-semibold text-text-primary">
          Recently downloaded
        </h3>
        {doneTasks.length > 0 && (
          <button
            className="no-drag text-[12.5px] text-text-muted hover:text-text-primary inline-flex items-center gap-1 transition-colors"
            onClick={() => setActiveTab('queue')}
          >
            View all <ArrowRight size={13} />
          </button>
        )}
      </div>

      {recentTiles.length === 0 ? (
        <div className="flex items-center justify-center h-28 rounded-xl border border-border-soft bg-bg-secondary text-text-muted text-sm">
          Downloads you complete will appear here
        </div>
      ) : (
        <div className="flex gap-4 overflow-x-auto pb-3 snap-x snap-mandatory">
          {recentTiles.map((task) => (
            <RecentTile
              key={task.taskId}
              task={task}
              onOpen={() => setActiveTab('queue')}
            />
          ))}
        </div>
      )}
    </div>
  );
}
