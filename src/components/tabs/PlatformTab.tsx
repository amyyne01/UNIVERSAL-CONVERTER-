// One reusable tab for all four platforms (no per-platform duplication).
// URL → detect/parse → single download or playlist preview; plain text → search.
import { useState, useCallback, useEffect, useRef } from 'react';
import { motion, useReducedMotion } from 'framer-motion';
import {
  Search, Link, Download, Video, Audio, Clock, Alert, Retry, Track as TrackIcon,
  type AppIcon,
} from '@/components/ui/icons';
import { useAppStore } from '@/store';
import { PLATFORMS, AUDIO_QUALITIES, VIDEO_QUALITIES, type PlatformKey } from '@/constants';
import { Button, Input, Select, FormatSelector, SegmentedCapsule } from '@/components/ui';
import { buildDownloadRequest, spotifyDownloadUrl } from '@/lib/download';
import { useTierLocks, premiumCopy, basicCopy } from '@/lib/tier';
import { formatDuration } from '@/lib/format';
import type { AudioFormat, VideoQuality, SearchResult, Track, SourcePlatform } from '@shared/types';

interface PlatformTabProps {
  platform: PlatformKey;
}

/** A row in the results list — normalized from a SearchResult or a Spotify Track. */
interface ResultItem {
  id: string;
  title: string;
  subtitle: string;
  thumbnailUrl: string;
  duration: number;
  url: string;
  track?: Track;
  /** Detected source for direct-URL results (reels tab hosts several platforms). */
  source?: SourcePlatform;
  /** True when this is THE fetched item for a pasted URL (renders as a big card). */
  direct?: boolean;
  /** Human label from url:detect (e.g. "YouTube video") for the result-card eyebrow. */
  label?: string;
}

// The fetch is four distinct outcomes, not one string: an untouched tab, work in
// flight, a real answer, a legitimate "nothing matched", and a failure the user
// can retry. One state so a branch can never leave the field disabled with no
// explanation on screen.
type Phase =
  | { k: 'idle' }
  | { k: 'busy'; note: string }
  | { k: 'results' }
  | { k: 'empty'; msg: string }
  | { k: 'error'; msg: string };

/** Per-platform voice. One record, four tabs — the component itself never forks. */
const PLATFORM_COPY: Record<PlatformKey, { capability: string; hint: string; empty: string }> = {
  youtube: {
    capability: 'Video + Audio · MP4/MP3 · Playlists',
    hint: 'Paste a video or playlist link — or just type what you want and search YouTube.',
    empty: 'Check the spelling, or paste the link straight from YouTube.',
  },
  spotify: {
    capability: 'Audio · MP3/FLAC · Playlists + Albums',
    hint: 'Paste a track, album or playlist link — or search by song and artist. Tracks are matched to the best available audio source.',
    empty: 'Try the artist and title together, or paste the Spotify link.',
  },
  soundcloud: {
    capability: 'Audio · MP3/FLAC · Artist sets',
    hint: 'Paste a track or set link — or search SoundCloud by artist, title or set.',
    empty: 'Try a shorter query, or paste the SoundCloud link.',
  },
  reels: {
    capability: 'Video · MP4 · No watermark',
    hint: 'Paste an Instagram Reel, TikTok or Facebook link. The video arrives without the watermark.',
    empty: 'That link had nothing to download.',
  },
};

const EASE: [number, number, number, number] = [0.16, 1, 0.3, 1];

export const looksLikeUrl = (s: string): boolean => /^(https?:\/\/|spotify:|[\w-]+\.[a-z]{2,})/i.test(s.trim());

/** Thrown for local parse/detection failures — distinct from a network/fetch failure. */
class ValidationError extends Error {}

export function PlatformTab({ platform }: PlatformTabProps) {
  const def = PLATFORMS.find((p) => p.key === platform)!;
  const Icon = def.icon;
  const accent = `var(--color-${def.accent})`;
  const copy = PLATFORM_COPY[platform];
  const reduce = useReducedMotion();

  const config = useAppStore((s) => s.config);
  const setCurrentPlaylist = useAppStore((s) => s.setCurrentPlaylist);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const addDownload = useAppStore((s) => s.addDownload);
  const pendingInput = useAppStore((s) => s.pendingInput);
  const setPendingInput = useAppStore((s) => s.setPendingInput);
  const locks = useTierLocks();

  const audioOnlyPlatform = platform === 'spotify' || platform === 'soundcloud';
  const canSearch = platform !== 'reels';

  const [query, setQuery] = useState('');
  const [phase, setPhase] = useState<Phase>({ k: 'idle' });
  const [results, setResults] = useState<ResultItem[]>([]);
  const [mediaType, setMediaType] = useState<'audio' | 'video'>(audioOnlyPlatform ? 'audio' : 'video');
  const [audioFormat, setAudioFormat] = useState<AudioFormat>(
    (config?.defaultFormat as AudioFormat) ?? 'mp3',
  );
  const [audioQuality, setAudioQuality] = useState<string>(config?.defaultQuality ?? '320');
  const [videoQuality, setVideoQuality] = useState<VideoQuality>(
    (config?.defaultVideoQuality as VideoQuality) ?? 'best',
  );

  const busy = phase.k === 'busy';
  const isAudio = audioOnlyPlatform || mediaType === 'audio';

  // Apply the user's saved format/quality defaults once config arrives (the
  // useState initializers run before the async config load resolves). Skipped
  // once the user has touched a selector, so an in-flight config load can't
  // clobber a choice they already made.
  const seededDefaults = useRef(false);
  const touchedDefaults = useRef(false);
  useEffect(() => {
    if (!config || seededDefaults.current || touchedDefaults.current) return;
    seededDefaults.current = true;
    setAudioFormat((config.defaultFormat as AudioFormat) ?? 'mp3');
    setAudioQuality(config.defaultQuality ?? '320');
    setVideoQuality((config.defaultVideoQuality as VideoQuality) ?? 'best');
  }, [config]);

  // What the file will actually be, on this tier — locks are applied here so the
  // readout can never promise a format the engine will clamp away.
  const outcome = isAudio
    ? `${locks.effectiveFormat(audioFormat).toUpperCase()} · ${
        AUDIO_QUALITIES.find((q) => q.value === locks.effectiveAudioQuality(audioQuality))?.label ?? ''
      }`
    : `MP4 · ${
        VIDEO_QUALITIES.find((q) => q.value === locks.effectiveVideoQuality(videoQuality))?.label ?? ''
      }`;

  const startDownload = useCallback(
    (item: ResultItem, source: SourcePlatform) => {
      void window.electronAPI.download
        .start(
          buildDownloadRequest(config, {
            url: item.track ? spotifyDownloadUrl(item.track) : item.url,
            source,
            title: item.title,
            uploader: item.subtitle,
            thumbnailUrl: item.thumbnailUrl,
            duration: item.duration,
            isAudioOnly: isAudio,
            isPlaylist: false,
            format: isAudio ? audioFormat : 'mp4',
            quality: audioQuality,
            videoQuality,
            track: item.track,
          }),
        )
        .then((task) => addDownload(task))
        .catch(() => setPhase({ k: 'error', msg: 'Could not start the download.' }));
    },
    [isAudio, audioFormat, audioQuality, videoQuality, config, addDownload],
  );

  const handleSubmit = useCallback(async (override?: string) => {
    const input = (override ?? query).trim();
    if (!input) return;
    const isLink = looksLikeUrl(input);
    setPhase({ k: 'busy', note: isLink ? 'Reading the link…' : `Searching ${def.label}…` });
    setResults([]);
    try {
      // ── Spotify: credential-free scraper ─────────────────────────────────
      if (platform === 'spotify') {
        if (isLink) {
          // parseUrl is local pattern-matching — failure means bad input, not network.
          const { type } = await window.electronAPI.spotify
            .parseUrl(input)
            .catch(() => { throw new ValidationError("That link couldn't be read — check it and try again."); });
          if (type === 'playlist' || type === 'album') {
            const pl =
              type === 'playlist'
                ? await window.electronAPI.spotify.fetchPlaylist(input)
                : await window.electronAPI.spotify.fetchAlbum(input);
            if (!pl) return setPhase({ k: 'error', msg: 'Could not read that Spotify collection.' });
            setCurrentPlaylist(pl);
            return;
          }
          const track = await window.electronAPI.spotify.fetchTrack(input);
          if (!track) return setPhase({ k: 'error', msg: 'Could not read that Spotify track.' });
          // Show the fetched track as a big result card — the user confirms format/quality.
          setResults([{ ...trackToItem(track), direct: true }]);
          return setPhase({ k: 'results' });
        }
        const tracks = await window.electronAPI.spotify.search(input);
        setResults(tracks.map(trackToItem));
        return setPhase(
          tracks.length ? { k: 'results' } : { k: 'empty', msg: `No Spotify results for “${input}”.` },
        );
      }

      // ── YouTube / SoundCloud / Reels ─────────────────────────────────────
      if (isLink) {
        // url.detect is local pattern-matching — failure means bad input, not network.
        const detection = await window.electronAPI.url
          .detect(input)
          .catch(() => { throw new ValidationError("That link couldn't be read — check it and try again."); });
        if (detection.platform === 'unknown') {
          return setPhase({ k: 'error', msg: `That link isn't one ${def.label} can handle.` });
        }
        if (detection.isCollection) {
          const meta = await window.electronAPI.url.fetchMetadata(input);
          setResults(meta.entries.map(searchToItem));
          return setPhase(
            meta.entries.length
              ? { k: 'results' }
              : { k: 'empty', msg: 'That collection is empty.' },
          );
        }
        // Show the fetched item as a result card — the user confirms format/quality.
        const meta = await window.electronAPI.url.fetchMetadata(input).catch(() => null);
        setResults([
          {
            id: detection.id ?? input,
            title: meta?.title ?? input,
            subtitle: meta?.uploader ?? '',
            thumbnailUrl: meta?.thumbnailUrl ?? '',
            duration: meta?.duration ?? 0,
            url: input,
            source: detection.platform,
            direct: true,
            label: detection.label,
          },
        ]);
        return setPhase({ k: 'results' });
      }

      // plain text search (youtube / soundcloud)
      const fn =
        platform === 'soundcloud'
          ? window.electronAPI.soundcloud.search
          : window.electronAPI.youtube.search;
      const found = await fn(input);
      setResults(found.map(searchToItem));
      setPhase(found.length ? { k: 'results' } : { k: 'empty', msg: `No results for “${input}”.` });
    } catch (err) {
      setPhase({
        k: 'error',
        msg: err instanceof ValidationError
          ? err.message
          : 'Something went wrong — check your connection and try again.',
      });
    } finally {
      // Belt and braces: no branch may leave the field disabled with nothing on
      // screen, so any still-busy phase falls back to idle.
      setPhase((p) => (p.k === 'busy' ? { k: 'idle' } : p));
    }
  }, [platform, query, def.label, setCurrentPlaylist, setActiveTab, startDownload]);

  const resultSource: SourcePlatform = platform === 'reels' ? 'unknown' : platform;

  const get = useCallback((item: ResultItem) => {
    startDownload(item, item.track ? 'spotify' : (item.source ?? resultSource));
    setActiveTab('queue');
  }, [startDownload, resultSource, setActiveTab]);

  // Link handed over from the Dashboard / Ctrl+V: seed the field and fetch it
  // immediately so the user lands on a result card, not an empty form.
  useEffect(() => {
    if (!pendingInput) return;
    setQuery(pendingInput);
    setPendingInput(null);
    void handleSubmit(pendingInput);
  }, [pendingInput, setPendingInput, handleSubmit]);

  const directItem = results.length === 1 && results[0].direct && platform !== 'reels' ? results[0] : null;

  return (
    // @container: everything below reflows on the width of THIS column, not the
    // window — the app shell owns the outer container and may cap it tighter.
    // max-w is a ceiling, not a column: past ~1600px a single search field and a
    // one-per-row result list stop being a layout and start being dead canvas.
    <div
      className="@container mx-auto w-full max-w-[1600px] px-6 lg:px-8 2xl:px-12 py-8 lg:py-10"
      style={{ '--color-accent': accent } as React.CSSProperties}
    >
      {/* Header — platform glyph in an accent-soft well + Display title + capability readout */}
      <div className="flex items-center gap-4 mb-8">
        <div className="w-10 h-10 rounded-lg bg-accent-soft grid place-items-center shrink-0">
          <Icon size={22} style={{ color: accent }} />
        </div>
        <div>
          <h1 className="font-display font-semibold text-[32px] leading-[1.1] tracking-[-0.025em] text-text-primary">{def.label}</h1>
          <p className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted mt-1">{copy.capability}</p>
          {/* What the free tier gives you, on the surface where you'd use it.
              Without this the tier only ever speaks when it says no, which is how
              a free version starts feeling like a demo instead of a product. */}
          {!locks.isPremium && (
            <p className="text-[11.5px] text-text-secondary mt-1.5">
              {audioOnlyPlatform ? basicCopy.audio : basicCopy.video}
            </p>
          )}
        </div>
      </div>

      {/* Input — flat well, same anatomy as the Dashboard field (h-14) */}
      <div className="flex items-center gap-3 mb-5">
        <Input
          icon={canSearch ? Search : Link}
          wrapClassName="flex-1 h-14! rounded-lg!"
          placeholder={canSearch ? `Paste a ${def.label} link or search…` : `Paste a ${def.label} link…`}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') void handleSubmit(); }}
          spellCheck={false}
          autoComplete="off"
          disabled={busy}
        />
        <Button size="md" loading={busy} icon={Download} onClick={() => void handleSubmit()}>
          Fetch
        </Button>
      </div>

      {/* Output — the controls and, on the right, the file they add up to.
          Eight format chips + a quality select + the readout only share one line
          past ~1150px of column; below that (the 960px minimum window) the chips
          drop to their own row instead of shoving the readout off the edge. */}
      <div className="mb-6 rounded-lg bg-bg-surface border border-border-soft shadow-sm p-4 flex flex-wrap items-center gap-x-3 gap-y-2.5">
        {!audioOnlyPlatform && (
          <SegmentedCapsule
            options={[
              { value: 'video', label: 'Video', icon: Video },
              { value: 'audio', label: 'Audio', icon: Audio },
            ] as const}
            value={mediaType}
            onChange={setMediaType}
            className="h-10 items-center px-1"
          />
        )}
        {isAudio ? (
          <>
            <FormatSelector
              className="order-last basis-full @6xl:order-none @6xl:basis-auto"
              value={audioFormat}
              onChange={(f) => { touchedDefaults.current = true; setAudioFormat(f); }}
            />
            <Select
              label="Audio quality"
              options={AUDIO_QUALITIES}
              value={locks.effectiveAudioQuality(audioQuality)}
              lockedValues={locks.lockedAudioQualities}
              onLocked={() => locks.nudge(premiumCopy.losslessQuality())}
              onChange={(e) => { touchedDefaults.current = true; setAudioQuality(e.target.value); }}
            />
          </>
        ) : (
          <Select
            label="Video quality"
            options={VIDEO_QUALITIES}
            value={locks.effectiveVideoQuality(videoQuality)}
            lockedValues={locks.lockedVideoQualities}
            onLocked={(v) => locks.nudge(premiumCopy.videoQuality(v))}
            onChange={(e) => { touchedDefaults.current = true; setVideoQuality(e.target.value as VideoQuality); }}
          />
        )}
        <p className="ml-auto text-[12.5px] text-text-muted">
          You get <span className="font-mono text-text-secondary">{outcome}</span>
        </p>
      </div>

      {/* One region, five states — keyed so the swap is a clean fade, not a diff. */}
      <motion.div
        key={phase.k}
        initial={reduce ? false : { opacity: 0, y: 4 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.16, ease: EASE }}
      >
        {phase.k === 'busy' && <FetchSkeleton note={phase.note} square={audioOnlyPlatform} reduce={!!reduce} />}

        {phase.k === 'idle' && (
          <EmptyPanel icon={Icon} accentIcon={accent} title={copy.hint}>
            <span className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted">
              Enter to fetch
            </span>
          </EmptyPanel>
        )}

        {phase.k === 'empty' && (
          <EmptyPanel icon={Search} title={phase.msg}>
            <span className="text-[12.5px] text-text-secondary">{copy.empty}</span>
          </EmptyPanel>
        )}

        {phase.k === 'error' && (
          <div
            role="alert"
            className="flex items-start gap-3 rounded-lg border p-4"
            style={{
              borderColor: 'color-mix(in oklch, var(--color-error) 38%, transparent)',
              background: 'color-mix(in oklch, var(--color-error) 9%, transparent)',
            }}
          >
            <Alert size={18} className="text-error shrink-0 mt-px" />
            <div className="min-w-0 flex-1">
              <p className="text-sm text-text-primary">{phase.msg}</p>
            </div>
            <Button variant="ghost" size="sm" icon={Retry} onClick={() => void handleSubmit()} className="shrink-0">
              Try again
            </Button>
          </div>
        )}

        {phase.k === 'results' && (directItem ? (
          <SingleResultCard
            item={directItem}
            accent={accent}
            eyebrow={directItem.label ?? `${def.label} · ${isAudio ? 'Audio' : 'Video'}`}
            outcome={outcome}
            square={audioOnlyPlatform}
            reduce={!!reduce}
            onGet={() => get(directItem)}
          />
        ) : (
          <ResultList
            items={results}
            square={audioOnlyPlatform}
            reduce={!!reduce}
            onGet={get}
          />
        ))}
      </motion.div>
    </div>
  );
}

// ── Results ──────────────────────────────────────────────────────────────────

// One geometry, shared by the results and the skeleton that stands in for them,
// so the answer lands exactly where the wait was.
//
// A result row is artwork + title + meta + a Get pill — about 520px of content.
// Past ~1024px of column that row is mostly empty, so the list becomes 2 columns,
// and 3 past ~1280px. Below that (the 960px minimum window) it stays a single
// stack of full-width rows.
const RESULT_GRID = 'grid grid-cols-1 @5xl:grid-cols-2 @7xl:grid-cols-3 @5xl:gap-3';
// One hairline-divided surface while it's a single column; separate cards once
// it's a grid, because divide-y can't draw a two-dimensional rule.
const RESULT_SURFACE =
  'rounded-lg border border-border-soft bg-bg-surface shadow-sm overflow-hidden divide-y divide-border-soft ' +
  '@5xl:border-0 @5xl:bg-transparent @5xl:shadow-none @5xl:overflow-visible @5xl:divide-y-0';
const RESULT_CELL =
  '@5xl:rounded-lg @5xl:border @5xl:border-border-soft @5xl:bg-bg-surface @5xl:shadow-sm @5xl:overflow-hidden';

/** Artwork size classes: album art is square, video stills are 16:9. */
const artClass = (square: boolean) => (square ? 'w-14 h-14' : 'w-[92px] h-[52px]');

function Artwork({ src, square, className = '' }: { src: string; square: boolean; className?: string }) {
  return (
    <div className={`relative shrink-0 rounded-md overflow-hidden bg-bg-tertiary border border-border-soft ${className}`}>
      {src ? (
        <img src={src} alt="" className="w-full h-full object-cover" loading="lazy" />
      ) : (
        <div className="w-full h-full grid place-items-center">
          <TrackIcon size={square ? 22 : 20} className="text-text-muted" />
        </div>
      )}
    </div>
  );
}

/** One surface, hairline-divided rows — a result set reads as one object, not
 *  a stack of floating cards. Arrow keys walk it; Enter downloads the focused row. */
function ResultList({
  items,
  square,
  reduce,
  onGet,
}: {
  items: ResultItem[];
  square: boolean;
  reduce: boolean;
  onGet: (item: ResultItem) => void;
}) {
  // Roving focus without per-row state: the rows are the only [data-row] nodes
  // in this list, so one DOM read per keypress beats re-rendering 40 rows.
  // The list is 1, 2 or 3 columns depending on the column width, so up/down must
  // step a whole row — the live column count comes from the resolved grid
  // template rather than from a duplicated copy of the breakpoints.
  const onKeyDown = (e: React.KeyboardEvent<HTMLUListElement>) => {
    if (!['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return;
    e.preventDefault();
    const rows = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>('[data-row]'));
    const at = rows.indexOf(document.activeElement as HTMLButtonElement);
    const cols = getComputedStyle(e.currentTarget).gridTemplateColumns.split(' ').length;
    const step =
      e.key === 'ArrowDown' ? cols
      : e.key === 'ArrowUp' ? -cols
      : e.key === 'ArrowRight' ? 1
      : e.key === 'ArrowLeft' ? -1
      : 0;
    const next =
      e.key === 'Home' ? 0
      : e.key === 'End' ? rows.length - 1
      : Math.min(rows.length - 1, Math.max(0, at + step));
    rows[next]?.focus();
  };

  return (
    <section aria-label="Results">
      <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 px-1 mb-2">
        <span className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted tabular-nums">
          {items.length} result{items.length === 1 ? '' : 's'}
        </span>
        <span className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted">
          Arrows move · Enter downloads
        </span>
      </div>

      <ul onKeyDown={onKeyDown} className={`${RESULT_GRID} ${RESULT_SURFACE}`}>
        {items.map((item, i) => (
          <motion.li
            key={item.id}
            initial={reduce ? false : { opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            // Stagger caps at the 6th row: a 40-result set must not take a second
            // to finish arriving.
            transition={{ duration: 0.18, delay: Math.min(i, 6) * 0.025, ease: EASE }}
            className={RESULT_CELL}
          >
            <button
              type="button"
              data-row
              tabIndex={i === 0 ? 0 : -1}
              aria-label={`Download ${item.title}${item.subtitle ? ` — ${item.subtitle}` : ''}`}
              onClick={() => onGet(item)}
              className="group w-full h-full flex items-center gap-4 px-3.5 py-3 text-left transition-colors hover:bg-bg-hover focus-visible:bg-bg-hover"
            >
              <Artwork src={item.thumbnailUrl} square={square} className={artClass(square)} />

              <span className="min-w-0 flex-1">
                <span className="block text-[15px] font-medium text-text-primary truncate">{item.title}</span>
                {/* text-secondary, not muted: at 12.5px the muted ramp lands under
                    4.5:1 on the light theme's near-white surface. */}
                <span className="mt-0.5 flex items-center gap-2 text-[12.5px] text-text-secondary">
                  {item.subtitle && <span className="truncate">{item.subtitle}</span>}
                  {item.subtitle && item.duration > 0 && <span aria-hidden>·</span>}
                  {item.duration > 0 && (
                    <span className="font-mono tabular-nums shrink-0">{formatDuration(item.duration)}</span>
                  )}
                </span>
              </span>

              {/* The row IS the button; this pill is the affordance, not a second target. */}
              <span
                aria-hidden
                className="shrink-0 inline-flex items-center gap-1.5 h-8 px-3 rounded-md border border-border text-[12.5px] font-medium text-text-secondary transition-colors group-hover:border-accent group-hover:text-accent group-focus-visible:border-accent group-focus-visible:text-accent"
              >
                <Download size={14} />
                Get
              </span>
            </button>
          </motion.li>
        ))}
      </ul>
    </section>
  );
}

/** Big confirmation card for a single pasted URL — artist, name, length, one Download. */
function SingleResultCard({
  item,
  accent,
  eyebrow,
  outcome,
  square,
  reduce,
  onGet,
}: {
  item: ResultItem;
  accent: string;
  eyebrow: string;
  outcome: string;
  square: boolean;
  reduce: boolean;
  onGet: () => void;
}) {
  return (
    <motion.div
      initial={reduce ? false : { opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.18, ease: EASE }}
      // Capped at ~880 (about one result-grid cell plus its neighbour): one
      // confirmation card is a single object, not a layout — stretched to 1500px
      // it is a thumbnail, a title, and a button at the far edge of the screen.
      className="flex flex-wrap items-center gap-5 p-5 max-w-[880px] rounded-xl bg-bg-surface border border-border shadow-md"
    >
      {/* Art — square for audio platforms (album art), 16:9 for video */}
      <Artwork src={item.thumbnailUrl} square={square} className={square ? 'w-28 h-28' : 'w-[198px] h-28'} />

      {/* Name / artist / length — basis 240px is the point where the title stops
          being readable, so the Download button wraps under instead of squeezing it. */}
      <div className="min-w-0 flex-1 basis-[240px]">
        <div
          className="font-mono text-[10.5px] tracking-[0.08em] uppercase mb-1.5"
          style={{ color: accent }}
        >
          {eyebrow}
        </div>
        <h3 className="font-display font-semibold text-[20px] leading-[1.25] tracking-[-0.015em] text-text-primary line-clamp-2 mb-0.5">
          {item.title}
        </h3>
        {item.subtitle && (
          <p className="text-sm text-text-secondary truncate">{item.subtitle}</p>
        )}
        <div className="flex items-center gap-3 mt-2.5 font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted tabular-nums">
          {item.duration > 0 && (
            <span className="flex items-center gap-1.5">
              <Clock size={12} />
              {formatDuration(item.duration)}
            </span>
          )}
          <span>{outcome}</span>
        </div>
      </div>

      <Button size="md" icon={Download} onClick={onGet} className="shrink-0">
        Download
      </Button>
    </motion.div>
  );
}

// ── Waiting / nothing states ─────────────────────────────────────────────────

/** Row-shaped placeholders: the answer lands where the wait was, so nothing jumps. */
function FetchSkeleton({ note, square, reduce }: { note: string; square: boolean; reduce: boolean }) {
  return (
    <section aria-busy="true" aria-live="polite">
      <div className="px-1 mb-2 font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted">
        {note}
      </div>
      {/* Six, not four: six divides evenly by the 1/2/3 column counts the results
          land in, so the placeholder block is the same shape as the answer. */}
      <div className={`${RESULT_GRID} ${RESULT_SURFACE}`}>
        {[0, 1, 2, 3, 4, 5].map((i) => (
          <motion.div
            key={i}
            aria-hidden
            className={`flex items-center gap-4 px-3.5 py-3 ${RESULT_CELL}`}
            // Opacity only — a 4-row breathe reads as "working" without costing a
            // layout pass, and reduced motion gets the same shapes, held still.
            initial={{ opacity: reduce ? 0.6 : 0.45 }}
            animate={reduce ? undefined : { opacity: [0.45, 0.8, 0.45] }}
            transition={{ duration: 1.4, repeat: Infinity, delay: i * 0.12, ease: 'easeInOut' }}
          >
            <div className={`shrink-0 rounded-md bg-bg-tertiary ${artClass(square)}`} />
            <div className="min-w-0 flex-1 space-y-2">
              <div className="h-3.5 rounded bg-bg-tertiary" style={{ width: `${72 - i * 9}%` }} />
              <div className="h-2.5 rounded bg-bg-tertiary" style={{ width: `${38 - i * 4}%` }} />
            </div>
            <div className="shrink-0 h-8 w-[74px] rounded-md bg-bg-tertiary" />
          </motion.div>
        ))}
      </div>
    </section>
  );
}

/** Shared shell for "nothing here yet" and "nothing matched" — same shape, different cause. */
function EmptyPanel({
  icon: PanelIcon,
  accentIcon,
  title,
  children,
}: {
  icon: AppIcon;
  accentIcon?: string;
  title: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="rounded-lg border border-dashed border-border px-6 py-10 text-center">
      <PanelIcon size={26} className="mx-auto opacity-60" style={accentIcon ? { color: accentIcon } : undefined} />
      <p className="mx-auto mt-3.5 max-w-[52ch] text-sm leading-relaxed text-text-secondary text-pretty">{title}</p>
      {children && <div className="mt-2.5">{children}</div>}
    </div>
  );
}

function searchToItem(r: SearchResult): ResultItem {
  return { id: r.id, title: r.title, subtitle: r.uploader, thumbnailUrl: r.thumbnailUrl, duration: r.duration, url: r.url };
}

function trackToItem(t: Track): ResultItem {
  return {
    id: t.id,
    title: t.name,
    subtitle: t.artist,
    thumbnailUrl: t.thumbnailUrl,
    duration: Math.round(t.durationMs / 1000),
    url: t.spotifyUrl ?? '',
    track: t,
  };
}
