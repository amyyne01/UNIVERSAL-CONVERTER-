import { useState, useCallback, useMemo, useRef, memo } from 'react';
import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import { OpenFolder, Download, Track, Lock, Premium } from '@/components/ui/icons';
import { useAppStore } from '@/store';
import { Button, Select } from '@/components/ui';
import { AUDIO_FORMATS, AUDIO_QUALITIES } from '@/constants';
import { buildDownloadRequest, spotifyDownloadUrl } from '@/lib/download';
import { useTierLocks, premiumCopy, basicCopy } from '@/lib/tier';
import type { AudioFormat, Track as TrackType } from '@shared/types';

// One template for the header and every row. Past ~896px of column the artist
// stops being a second line under the title and becomes its own column — that is
// the one thing a track table can honestly do with extra width. It costs no extra
// DOM per row: the title wrapper becomes `display:contents` and hands its two
// children straight to the grid, so a 200-track list renders the same nodes.
const TRACK_COLUMNS =
  'grid grid-cols-[26px_30px_minmax(0,1fr)_auto] @4xl:grid-cols-[26px_34px_minmax(0,1.7fr)_minmax(0,1fr)_auto]';

/** ms → m:ss with tabular digits */
function fmtDur(ms: number): string {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

/** ms → "1h 42m" / "42m" for the runtime readouts */
function fmtSpan(ms: number): string {
  const min = Math.round(ms / 60000);
  return min >= 60 ? `${Math.floor(min / 60)}h ${min % 60}m` : `${min}m`;
}

export function PlaylistPreview() {
  const playlist      = useAppStore((s) => s.currentPlaylist);
  const selectedTracks = useAppStore((s) => s.selectedTracks);
  const toggleTrack   = useAppStore((s) => s.toggleTrack);
  const selectAllTracks = useAppStore((s) => s.selectAllTracks);
  const clearTracks   = useAppStore((s) => s.clearTracks);
  const addDownload   = useAppStore((s) => s.addDownload);
  const config        = useAppStore((s) => s.config);
  const locks         = useTierLocks();
  const reduce        = useReducedMotion();

  const [format, setFormat]       = useState<AudioFormat>((config?.defaultFormat as AudioFormat) ?? 'mp3');
  const [quality, setQuality]     = useState(config?.defaultQuality ?? '320');
  const [enqueueing, setEnqueueing] = useState(false);
  const [enqueueError, setEnqueueError] = useState('');
  const updateConfig = useAppStore((s) => s.updateConfig);

  // Live refs so the row callback can stay identity-stable — the memoized rows
  // only re-render when their own selected/capped flag flips, which is what keeps
  // a 200-track list responsive while you click through it.
  const tracksRef = useRef<TrackType[]>([]);
  tracksRef.current = playlist?.tracks ?? [];
  const selectedRef = useRef(selectedTracks);
  selectedRef.current = selectedTracks;
  const anchorRef = useRef<number | null>(null);

  const pickDir = useCallback(async () => {
    const dir = await window.electronAPI.dialog.selectDir();
    if (dir) updateConfig({ outputDir: dir });
  }, [updateConfig]);

  const selectRow = useCallback((index: number, shift: boolean) => {
    const tracks = tracksRef.current;
    const anchor = anchorRef.current;
    if (shift && anchor !== null && anchor !== index && tracks[anchor]) {
      const [from, to] = anchor < index ? [anchor, index] : [index, anchor];
      // Shift-click adds the span (it never deselects), so a mis-aimed range is
      // one click to undo rather than a wiped selection.
      // ponytail: one toggle per track — React batches the set() calls into a
      // single render, and copying a 200-id Set is microseconds.
      for (let i = from; i <= to; i++) {
        if (!selectedRef.current.has(tracks[i].id)) toggleTrack(tracks[i].id);
      }
    } else {
      toggleTrack(tracks[index].id);
    }
    anchorRef.current = index;
  }, [toggleTrack]);

  // One pass over the list per selection change: how many are picked, how long
  // they run, and which ones fall past the tier's collection ceiling.
  const { selectedCount, selectedMs, cappedIds } = useMemo(() => {
    const limit = locks.collectionLimit;
    const capped = new Set<string>();
    let count = 0;
    let ms = 0;
    for (const t of playlist?.tracks ?? []) {
      if (!selectedTracks.has(t.id)) continue;
      count += 1;
      ms += t.durationMs;
      if (limit !== null && count > limit) capped.add(t.id);
    }
    return { selectedCount: count, selectedMs: ms, cappedIds: capped };
  }, [playlist, selectedTracks, locks.collectionLimit]);

  const queuedCount = locks.collectionLimit === null
    ? selectedCount
    : Math.min(selectedCount, locks.collectionLimit);
  const skipped = selectedCount - queuedCount;

  const enqueue = useCallback(async () => {
    if (!playlist || selectedTracks.size === 0) return;
    setEnqueueing(true);
    setEnqueueError('');
    try {
      const chosen = playlist.tracks.filter((t) => selectedTracks.has(t.id));
      // Basic takes the first N of a collection — same ceiling the main process
      // applies to engine-expanded playlists (--playlist-end).
      const tracks = locks.collectionLimit === null ? chosen : chosen.slice(0, locks.collectionLimit);
      if (tracks.length < chosen.length) {
        locks.nudge(premiumCopy.collection());
      }
      const results = await Promise.allSettled(
        tracks.map(async (t) => {
          const task = await window.electronAPI.download.start(
            buildDownloadRequest(config, {
              url: spotifyDownloadUrl(t),
              source: 'spotify',
              format,
              quality,
              isAudioOnly: true,
              isPlaylist: false,
              playlistName: playlist.name,
              title: t.name,
              thumbnailUrl: t.thumbnailUrl,
              duration: Math.floor(t.durationMs / 1000),
              uploader: t.artist,
              track: t,
            }),
          );
          addDownload(task);
        }),
      );
      const failed = results.filter((r) => r.status === 'rejected').length;
      if (failed > 0) {
        setEnqueueError(`${failed} of ${tracks.length} track${tracks.length === 1 ? '' : 's'} failed to queue.`);
      } else {
        // Queued cleanly → drop the selection. The button re-enables as soon as
        // the enqueue settles, and skipExisting only skips files already ON DISK,
        // so a second click on a live selection duplicates in-flight jobs.
        // A partial failure keeps the selection, so a retry is one click away.
        clearTracks();
        anchorRef.current = null;
      }
    } finally {
      setEnqueueing(false);
    }
  }, [playlist, selectedTracks, format, quality, config, addDownload, clearTracks, locks]);

  if (!playlist) return null;

  const allSelected =
    playlist.tracks.length > 0 && selectedTracks.size === playlist.tracks.length;

  // Total runtime for the mono meta readout (e.g. "1h 42m").
  const runtime = fmtSpan(playlist.tracks.reduce((s, t) => s + t.durationMs, 0));

  const downloadLabel =
    selectedCount === 0 ? 'Download'
    : skipped > 0 ? `Download ${queuedCount} of ${selectedCount}`
    : `Download ${queuedCount} track${queuedCount === 1 ? '' : 's'}`;

  return (
    // @container: the header, the table and the action bar all reflow on this
    // column's width. Capped at 1400 deliberately — a track table gains a real
    // artist column at that width and nothing after it, so the extra canvas on a
    // 2560 monitor is better left as margin than as a 2000px-wide title cell.
    <div className="@container relative mx-auto w-full max-w-[1400px] px-6 lg:px-8 2xl:px-12 pt-8 lg:pt-10">
      {/* ── Sticky header — cover · name · mono meta · live selection · select-all ── */}
      <div className="sticky top-0 z-20 mb-4">
        {/* Tighter at short viewports: a 640px-tall window has to spend its height
            on tracks, not on two glass bars. */}
        <div className="rounded-lg bg-bg-glass backdrop-blur-xl border border-border shadow-lg p-4 [@media(max-height:720px)]:p-3">
          <div className="flex flex-wrap items-center gap-x-4 gap-y-3">
            {/* Cover 64px */}
            <div className="w-16 h-16 rounded-md border border-border-soft overflow-hidden shrink-0 bg-bg-tertiary">
              {playlist.thumbnailUrl ? (
                <img
                  src={playlist.thumbnailUrl}
                  alt={playlist.name}
                  className="w-full h-full object-cover"
                />
              ) : (
                <div className="w-full h-full flex items-center justify-center">
                  <Track size={26} className="text-text-muted" />
                </div>
              )}
            </div>

            {/* Name + mono meta — below ~220px the name is unreadable, so the
                readout and select-all wrap under instead of crushing it. */}
            <div className="min-w-0 flex-1 basis-[220px]">
              <div className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-spotify mb-1">
                Spotify Playlist
              </div>
              <h2 className="font-display font-semibold text-[20px] leading-tight tracking-[-0.015em] text-text-primary truncate">
                {playlist.name}
              </h2>
              <div className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted mt-1 tabular-nums">
                {playlist.trackCount} tracks · {runtime} · by {playlist.owner}
              </div>
            </div>

            {/* Live selection readout — the answer to "how much have I picked?"
                stays on screen while you scroll the list, not just at the bottom. */}
            <div className="text-right shrink-0 ml-auto" aria-live="polite">
              <div className="font-mono text-[22px] leading-none tabular-nums text-text-primary">
                {selectedCount}
                <span className="text-text-muted text-[15px]">/{playlist.tracks.length}</span>
              </div>
              <div className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted mt-1.5 tabular-nums">
                {selectedCount === 0 ? 'none selected' : `selected · ${fmtSpan(selectedMs)}`}
              </div>
              {/* What free gives you here — shown before you hit the ceiling, so
                  the tier is not only ever heard saying no. Suppressed once the
                  cap warning is up: two messages about one limit is nagging. */}
              {!locks.isPremium && skipped === 0 && (
                <div className="text-[11px] text-text-secondary mt-1.5">{basicCopy.collection}</div>
              )}
            </div>

            {/* Select-all chip */}
            <button
              type="button"
              onClick={() => { anchorRef.current = null; allSelected ? clearTracks() : selectAllTracks(); }}
              className="no-drag flex items-center gap-2 h-8 px-3 rounded-md border border-border text-[12.5px] text-text-secondary hover:bg-bg-hover transition-colors select-none shrink-0"
            >
              <Checkbox checked={allSelected} accent="spotify" />
              {allSelected ? 'Deselect all' : 'Select all'}
            </button>
          </div>

          {/* The tier ceiling, while you choose — not after you press Download. */}
          <AnimatePresence initial={false}>
            {skipped > 0 && (
              <motion.div
                key="cap"
                initial={reduce ? false : { opacity: 0, y: -4 }}
                animate={{ opacity: 1, y: 0 }}
                exit={reduce ? { opacity: 0 } : { opacity: 0, y: -4 }}
                transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
                className="mt-3 flex items-center gap-2.5 rounded-md px-3 py-2"
                style={{
                  background: 'color-mix(in oklch, var(--color-warning) 12%, transparent)',
                  border: '1px solid color-mix(in oklch, var(--color-warning) 32%, transparent)',
                }}
              >
                <Lock size={14} className="text-warning shrink-0" />
                {/* Same words as the toast — one ceiling should not have two
                    descriptions. The count of what will be skipped is the extra
                    this strip earns by being visible while you are still picking. */}
                <p className="text-[12.5px] text-text-primary flex-1 min-w-0">
                  {premiumCopy.collection()}{' '}
                  <span className="text-text-secondary">
                    The last <span className="font-mono tabular-nums">{skipped}</span> you picked won’t be queued.
                  </span>
                </p>
                <Button variant="ghost" size="sm" icon={Premium} onClick={locks.openUpgrade} className="shrink-0">
                  See plans
                </Button>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>

      {/* ── Track list ─────────────────────────────────────────────────── */}
      <div
        className="rounded-lg border border-border-soft bg-bg-surface shadow-sm overflow-hidden select-none"
        style={{ marginBottom: 96 }}
      >
        {/* Column header — same template as the rows (see TRACK_COLUMNS). */}
        <div className={`${TRACK_COLUMNS} gap-4 px-[18px] py-2.5 bg-bg-secondary border-b border-border-soft`}>
          <span />
          <span className="font-mono text-[10.5px] tracking-[0.08em] text-text-muted uppercase text-center">
            #
          </span>
          <span className="flex items-baseline gap-2.5 min-w-0">
            <span className="font-mono text-[10.5px] tracking-[0.08em] text-text-muted uppercase">
              Title
            </span>
            <span className="text-[11px] text-text-muted truncate">shift-click to pick a range</span>
          </span>
          <span className="hidden @4xl:block font-mono text-[10.5px] tracking-[0.08em] text-text-muted uppercase">
            Artist
          </span>
          <span className="font-mono text-[10.5px] tracking-[0.08em] text-text-muted uppercase text-right">
            Time
          </span>
        </div>

        {playlist.tracks.map((track, i) => (
          <TrackRow
            key={track.id}
            track={track}
            index={i}
            selected={selectedTracks.has(track.id)}
            capped={cappedIds.has(track.id)}
            onSelect={selectRow}
          />
        ))}
      </div>

      {/* ── Sticky action bar ──────────────────────────────────────────── */}
      {/* Bleeds to the column edges, so its negative margins must track the root's
          padding exactly. Wrap order is DOM order: format, quality and folder wrap
          first; the readout + Download stay one item, so the primary action can
          never be orphaned from the sentence describing it. */}
      <div
        className="sticky bottom-0 z-10 flex flex-wrap items-end gap-x-3.5 gap-y-3 border-t border-border -mx-6 lg:-mx-8 2xl:-mx-12 px-6 lg:px-8 2xl:px-12 py-4 [@media(max-height:720px)]:py-2.5"
        style={{
          background: 'var(--color-bg-glass)',
          backdropFilter: 'blur(24px) saturate(1.3)',
          WebkitBackdropFilter: 'blur(24px) saturate(1.3)',
          boxShadow: '0 -10px 40px oklch(0 0 0 / 0.3)',
        }}
      >
        {/* Format */}
        <div className="flex flex-col gap-1">
          <span className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted">
            Format
          </span>
          <Select
            label="Playlist format"
            options={AUDIO_FORMATS}
            value={locks.effectiveFormat(format)}
            lockedValues={locks.lockedFormats}
            onLocked={(f) => locks.nudge(premiumCopy.losslessFormat(f.toUpperCase()))}
            onChange={(e) => setFormat(e.target.value as AudioFormat)}
          />
        </div>

        {/* Quality */}
        <div className="flex flex-col gap-1">
          <span className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted">
            Quality
          </span>
          <Select
            label="Playlist quality"
            options={AUDIO_QUALITIES}
            value={locks.effectiveAudioQuality(quality)}
            lockedValues={locks.lockedAudioQualities}
            onLocked={() => locks.nudge(premiumCopy.losslessQuality())}
            onChange={(e) => setQuality(e.target.value)}
          />
        </div>

        {/* Destination folder — the one control that improves with width: it grows
            into the slack so more of the path is legible, and shrinks to 180px
            before anything else on the bar is forced to wrap. */}
        <div className="flex flex-col gap-1 min-w-0 flex-1 basis-[180px] max-w-[260px] @4xl:max-w-[420px]">
          <span className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted">
            Folder
          </span>
          <button
            type="button"
            onClick={pickDir}
            className="no-drag flex w-full items-center gap-2 h-10 px-3 rounded-md bg-bg-tertiary border border-transparent text-text-secondary text-sm hover:border-accent hover:text-text-primary transition-colors"
            title={config?.outputDir || undefined}
          >
            <OpenFolder size={15} className="text-text-muted flex-none" />
            <span className="font-mono text-xs truncate">
              {config?.outputDir || 'Choose folder…'}
            </span>
          </button>
        </div>

        <div className="flex-1 min-w-[16px]" />

        {/* Wraps rather than forcing the Download button onto its own line. */}
        {enqueueError && (
          <span role="alert" className="self-center min-w-0 text-sm text-error">
            {enqueueError}
          </span>
        )}

        {/* What you'll get, spelled out next to the button that does it */}
        <div className="flex flex-col items-end gap-1 min-w-0">
          <span className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted tabular-nums whitespace-nowrap">
            {queuedCount === 0
              ? 'nothing selected'
              : `${queuedCount} × ${locks.effectiveFormat(format).toUpperCase()} · ${
                  AUDIO_QUALITIES.find((q) => q.value === locks.effectiveAudioQuality(quality))?.label ?? ''
                } · ${fmtSpan(selectedMs)}`}
          </span>
          <Button
            icon={Download}
            loading={enqueueing}
            disabled={selectedTracks.size === 0}
            onClick={enqueue}
          >
            {downloadLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}

// ── One track row ─────────────────────────────────────────────────────────────
// Memoized: selection changes the parent's Set identity, so without this every
// click re-renders all 200 rows.
const TrackRow = memo(function TrackRow({
  track,
  index,
  selected,
  capped,
  onSelect,
}: {
  track: TrackType;
  index: number;
  selected: boolean;
  capped: boolean;
  onSelect: (index: number, shift: boolean) => void;
}) {
  return (
    <div
      role="checkbox"
      aria-checked={selected}
      aria-label={`${track.name} — ${track.artist}, ${fmtDur(track.durationMs)}${
        capped ? ' — past the Basic collection limit, will be skipped' : ''
      }`}
      tabIndex={0}
      onClick={(e) => onSelect(index, e.shiftKey)}
      onKeyDown={(e) => {
        if (e.key === ' ' || e.key === 'Enter') {
          e.preventDefault();
          onSelect(index, e.shiftKey);
        }
      }}
      className={`no-drag ${TRACK_COLUMNS} gap-4 h-12 items-center px-[18px] border-b border-border-soft last:border-b-0 cursor-pointer transition-colors ${
        selected ? 'bg-accent-soft' : 'hover:bg-bg-hover'
      }`}
    >
      {/* Checkbox */}
      <div className="flex items-center">
        <Checkbox checked={selected} accent="accent" />
      </div>

      {/* Track # — swapped for a lock once a row falls past the tier ceiling */}
      <div className="flex items-center justify-center">
        {capped ? (
          <Lock size={13} className="text-warning" />
        ) : (
          <span className="font-mono text-[11px] tabular-nums text-text-muted">{index + 1}</span>
        )}
      </div>

      {/* Title + artist — stacked while the table is narrow, two columns once it
          isn't. `contents` dissolves this wrapper into the grid, so the dimming
          for capped rows moves onto the children (a box that isn't generated
          can't carry opacity). */}
      <div className="min-w-0 @4xl:contents">
        <div className={`text-sm font-medium text-text-primary truncate ${capped ? 'opacity-55' : ''}`}>
          {track.name}
        </div>
        {/* secondary, not muted: the muted ramp at 12.5px misses 4.5:1 on the
            light theme's near-white surface. */}
        <div className={`text-[12.5px] text-text-secondary truncate @4xl:text-sm ${capped ? 'opacity-55' : ''}`}>
          {track.artist}
        </div>
      </div>

      {/* Duration */}
      <div className={`flex items-center ${capped ? 'opacity-55' : ''}`}>
        <span className="font-mono text-[11px] tabular-nums text-text-secondary">
          {fmtDur(track.durationMs)}
        </span>
      </div>
    </div>
  );
});

// ── inline visual checkbox — shared by header + rows, no extra file ────────
function Checkbox({ checked, accent }: { checked: boolean; accent: 'accent' | 'spotify' }) {
  const bg =
    accent === 'spotify'
      ? 'var(--color-spotify)'
      : 'var(--color-accent)';
  return (
    <span
      className="w-[19px] h-[19px] rounded-[6px] border flex items-center justify-center transition-colors flex-none"
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
