// One reusable tab for all four platforms (no per-platform duplication).
// URL → detect/parse → single download or playlist preview; plain text → search.
import { useState, useCallback, useEffect, useRef } from 'react';
import { motion } from 'framer-motion';
import { Search, Link2, Download as DownloadIcon, Video, AudioLines, Clock, Music2 } from 'lucide-react';
import { useAppStore } from '@/store';
import { PLATFORMS, AUDIO_QUALITIES, VIDEO_QUALITIES, type PlatformKey } from '@/constants';
import { Button, Input, Select, FormatSelector } from '@/components/ui';
import { buildDownloadRequest, spotifyDownloadUrl } from '@/lib/download';
import { useTierLocks } from '@/lib/tier';
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

// Micro-mono capability line — the instrument readout under the tab title.
const PLATFORM_CAPABILITY: Record<PlatformKey, string> = {
  youtube: 'Video + Audio · MP4/MP3 · Playlists',
  spotify: 'Audio · MP3/FLAC · Playlists + Albums',
  soundcloud: 'Audio · MP3/FLAC · Artist sets',
  reels: 'Video · MP4 · No watermark',
};

export const looksLikeUrl = (s: string): boolean => /^(https?:\/\/|spotify:|[\w-]+\.[a-z]{2,})/i.test(s.trim());

/** Thrown for local parse/detection failures — distinct from a network/fetch failure. */
class ValidationError extends Error {}

export function PlatformTab({ platform }: PlatformTabProps) {
  const def = PLATFORMS.find((p) => p.key === platform)!;
  const Icon = def.icon;
  const accent = `var(--color-${def.accent})`;

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
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [results, setResults] = useState<ResultItem[]>([]);
  const [mediaType, setMediaType] = useState<'audio' | 'video'>(audioOnlyPlatform ? 'audio' : 'video');
  const [audioFormat, setAudioFormat] = useState<AudioFormat>(
    (config?.defaultFormat as AudioFormat) ?? 'mp3',
  );
  const [audioQuality, setAudioQuality] = useState<string>(config?.defaultQuality ?? '320');
  const [videoQuality, setVideoQuality] = useState<VideoQuality>(
    (config?.defaultVideoQuality as VideoQuality) ?? 'best',
  );

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
        .catch(() => setError('Could not start the download.'));
    },
    [isAudio, audioFormat, audioQuality, videoQuality, config, addDownload],
  );

  const handleSubmit = useCallback(async (override?: string) => {
    const input = (override ?? query).trim();
    if (!input) return;
    setBusy(true);
    setError(null);
    setResults([]);
    try {
      // ── Spotify: credential-free scraper ─────────────────────────────────
      if (platform === 'spotify') {
        if (looksLikeUrl(input)) {
          // parseUrl is local pattern-matching — failure means bad input, not network.
          const { type } = await window.electronAPI.spotify
            .parseUrl(input)
            .catch(() => { throw new ValidationError("That link couldn't be read — check it and try again."); });
          if (type === 'playlist' || type === 'album') {
            const pl =
              type === 'playlist'
                ? await window.electronAPI.spotify.fetchPlaylist(input)
                : await window.electronAPI.spotify.fetchAlbum(input);
            if (!pl) return setError('Could not read that Spotify collection.');
            setCurrentPlaylist(pl);
            return;
          }
          const track = await window.electronAPI.spotify.fetchTrack(input);
          if (!track) return setError('Could not read that Spotify track.');
          // Show the fetched track as a big result card — the user confirms format/quality.
          setResults([{ ...trackToItem(track), direct: true }]);
          return;
        }
        const tracks = await window.electronAPI.spotify.search(input);
        setResults(tracks.map(trackToItem));
        if (!tracks.length) setError('No Spotify results.');
        return;
      }

      // ── YouTube / SoundCloud / Reels ─────────────────────────────────────
      if (looksLikeUrl(input)) {
        // url.detect is local pattern-matching — failure means bad input, not network.
        const detection = await window.electronAPI.url
          .detect(input)
          .catch(() => { throw new ValidationError("That link couldn't be read — check it and try again."); });
        if (detection.platform === 'unknown') return setError('Unrecognized link.');
        if (detection.isCollection) {
          const meta = await window.electronAPI.url.fetchMetadata(input);
          setResults(meta.entries.map(searchToItem));
          if (!meta.entries.length) setError('That collection is empty.');
          return;
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
        return;
      }

      // plain text search (youtube / soundcloud)
      const fn =
        platform === 'soundcloud'
          ? window.electronAPI.soundcloud.search
          : window.electronAPI.youtube.search;
      const found = await fn(input);
      setResults(found.map(searchToItem));
      if (!found.length) setError('No results.');
    } catch (err) {
      setError(
        err instanceof ValidationError
          ? err.message
          : 'Something went wrong — check your connection and try again.',
      );
    } finally {
      setBusy(false);
    }
  }, [platform, query, setCurrentPlaylist, setActiveTab, startDownload]);

  const resultSource: SourcePlatform = platform === 'reels' ? 'unknown' : platform;

  // Link handed over from the Dashboard / Ctrl+V: seed the field and fetch it
  // immediately so the user lands on a result card, not an empty form.
  useEffect(() => {
    if (!pendingInput) return;
    setQuery(pendingInput);
    setPendingInput(null);
    void handleSubmit(pendingInput);
  }, [pendingInput, setPendingInput, handleSubmit]);

  return (
    <div className="max-w-[860px] mx-auto px-10 2xl:px-16 py-10" style={{ '--color-accent': accent } as React.CSSProperties}>
      {/* Header — platform glyph in an accent-soft well + Display title + capability readout */}
      <div className="flex items-center gap-4 mb-8">
        <div className="w-10 h-10 rounded-lg bg-accent-soft grid place-items-center shrink-0">
          <Icon size={22} strokeWidth={1.8} style={{ color: accent }} />
        </div>
        <div>
          <h1 className="font-display font-semibold text-[32px] leading-[1.1] tracking-[-0.025em] text-text-primary">{def.label}</h1>
          <p className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted mt-1">{PLATFORM_CAPABILITY[platform]}</p>
        </div>
      </div>

      {/* Input — flat well, same anatomy as the Dashboard field (h-14) */}
      <div className="flex items-center gap-3 mb-5">
        <Input
          icon={canSearch ? Search : Link2}
          wrapClassName="flex-1 h-14! rounded-lg!"
          placeholder={canSearch ? `Paste a ${def.label} link or search…` : `Paste a ${def.label} link…`}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') void handleSubmit(); }}
          spellCheck={false}
          autoComplete="off"
          disabled={busy}
        />
        <Button size="md" loading={busy} icon={DownloadIcon} onClick={() => void handleSubmit()}>
          Fetch
        </Button>
      </div>

      {/* Output — format/quality controls in one card under a mono section label */}
      <div className="mb-6 rounded-lg bg-bg-surface border border-border-soft shadow-sm p-5">
        <div className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted mb-3">Output</div>
        <div className="flex flex-wrap items-center gap-3">
          {!audioOnlyPlatform && (
            <div className="flex rounded-md border border-border overflow-hidden">
              {(['video', 'audio'] as const).map((m) => (
                <button
                  key={m}
                  onClick={() => setMediaType(m)}
                  className={`flex items-center gap-1.5 px-3.5 h-9 text-xs font-medium transition-colors ${
                    mediaType === m ? 'bg-bg-hover text-text-primary' : 'text-text-muted hover:text-text-secondary'
                  }`}
                >
                  {m === 'video' ? <Video size={14} /> : <AudioLines size={14} />}
                  {m === 'video' ? 'Video' : 'Audio'}
                </button>
              ))}
            </div>
          )}
          {isAudio ? (
            <>
              <FormatSelector
                value={audioFormat}
                onChange={(f) => { touchedDefaults.current = true; setAudioFormat(f); }}
              />
              <Select
                label="Audio quality"
                options={AUDIO_QUALITIES}
                value={locks.effectiveAudioQuality(audioQuality)}
                lockedValues={locks.lockedAudioQualities}
                onLocked={() => locks.nudge('Lossless audio is a Premium feature.')}
                onChange={(e) => { touchedDefaults.current = true; setAudioQuality(e.target.value); }}
              />
            </>
          ) : (
            <Select
              label="Video quality"
              options={VIDEO_QUALITIES}
              value={locks.effectiveVideoQuality(videoQuality)}
              lockedValues={locks.lockedVideoQualities}
              onLocked={(v) => locks.nudge(`${v} needs Premium — Basic downloads up to 1080p.`)}
              onChange={(e) => { touchedDefaults.current = true; setVideoQuality(e.target.value as VideoQuality); }}
            />
          )}
        </div>
      </div>

      {error && <p className="text-sm text-error mb-4">{error}</p>}

      {/* Single pasted-URL result — big confirmation card.
          Reels is excluded: it gets a full preview-before-download feature instead. */}
      {results.length === 1 && results[0].direct && platform !== 'reels' ? (
        <SingleResultCard
          item={results[0]}
          accent={accent}
          eyebrow={results[0].label ?? `${def.label} · ${isAudio ? 'Audio' : 'Video'}`}
          square={audioOnlyPlatform}
          onGet={() => {
            const item = results[0];
            startDownload(item, item.track ? 'spotify' : (item.source ?? resultSource));
            setActiveTab('queue');
          }}
        />
      ) : results.length > 0 && (
        <div className="flex flex-col gap-2">
          {results.map((item) => (
            <motion.div
              key={item.id}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              className="flex items-center gap-4 p-3 rounded-lg bg-bg-surface border border-border-soft"
            >
              <div className="w-12 h-12 rounded-md overflow-hidden shrink-0 bg-bg-tertiary">
                {item.thumbnailUrl && (
                  <img src={item.thumbnailUrl} alt="" className="w-full h-full object-cover" loading="lazy" />
                )}
              </div>
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium text-text-primary truncate">{item.title}</p>
                <p className="text-xs text-text-muted truncate">{item.subtitle}</p>
              </div>
              {item.duration > 0 && (
                <span className="font-mono text-xs text-text-muted tabular-nums">{formatDuration(item.duration)}</span>
              )}
              <Button
                variant="ghost"
                size="sm"
                icon={DownloadIcon}
                onClick={() => { startDownload(item, item.track ? 'spotify' : (item.source ?? resultSource)); setActiveTab('queue'); }}
              >
                Get
              </Button>
            </motion.div>
          ))}
        </div>
      )}
    </div>
  );
}

/** Big confirmation card for a single pasted URL — artist, name, length, one Download. */
function SingleResultCard({
  item,
  accent,
  eyebrow,
  square,
  onGet,
}: {
  item: ResultItem;
  accent: string;
  eyebrow: string;
  square: boolean;
  onGet: () => void;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
      className="flex items-center gap-5 p-5 rounded-xl bg-bg-glass backdrop-blur-xl border border-border shadow-lg"
    >
      {/* Art — 96px: square for audio platforms (album art), 16:9 for video */}
      <div
        className={`shrink-0 rounded-lg overflow-hidden border border-border-soft bg-bg-tertiary ${
          square ? 'w-24 h-24' : 'w-[170px] h-24'
        }`}
      >
        {item.thumbnailUrl ? (
          <img src={item.thumbnailUrl} alt="" className="w-full h-full object-cover" />
        ) : (
          <div className="w-full h-full grid place-items-center">
            <Music2 size={30} className="text-text-muted" />
          </div>
        )}
      </div>

      {/* Name / artist / length */}
      <div className="min-w-0 flex-1">
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
        {item.duration > 0 && (
          <div className="flex items-center gap-1.5 mt-2 font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted tabular-nums">
            <Clock size={12} />
            {formatDuration(item.duration)}
          </div>
        )}
      </div>

      <Button size="md" icon={DownloadIcon} onClick={onGet} className="shrink-0">
        Download
      </Button>
    </motion.div>
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
