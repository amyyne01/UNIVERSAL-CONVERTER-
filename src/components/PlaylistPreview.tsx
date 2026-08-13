import { useState, useCallback } from 'react';
import { FolderOpen, Download, Music2 } from 'lucide-react';
import { useAppStore } from '@/store';
import { Button, Select } from '@/components/ui';
import { AUDIO_FORMATS, AUDIO_QUALITIES } from '@/constants';
import { buildDownloadRequest, spotifyDownloadUrl } from '@/lib/download';
import { useTierLocks } from '@/lib/tier';
import type { AudioFormat } from '@shared/types';

/** ms → m:ss with tabular digits */
function fmtDur(ms: number): string {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
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

  const [format, setFormat]       = useState<AudioFormat>((config?.defaultFormat as AudioFormat) ?? 'mp3');
  const [quality, setQuality]     = useState(config?.defaultQuality ?? '320');
  const [enqueueing, setEnqueueing] = useState(false);
  const [enqueueError, setEnqueueError] = useState('');
  const updateConfig = useAppStore((s) => s.updateConfig);

  const pickDir = useCallback(async () => {
    const dir = await window.electronAPI.dialog.selectDir();
    if (dir) updateConfig({ outputDir: dir });
  }, [updateConfig]);

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
        locks.nudge(
          `Basic downloads ${locks.collectionLimit} tracks per collection — ${chosen.length - tracks.length} were skipped.`,
        );
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
      }
    } finally {
      setEnqueueing(false);
    }
  }, [playlist, selectedTracks, format, quality, config, addDownload, clearTracks, locks]);

  if (!playlist) return null;

  const allSelected =
    playlist.tracks.length > 0 && selectedTracks.size === playlist.tracks.length;

  // Total runtime for the mono meta readout (e.g. "1h 42m").
  const totalMin = Math.round(playlist.tracks.reduce((s, t) => s + t.durationMs, 0) / 60000);
  const runtime = totalMin >= 60 ? `${Math.floor(totalMin / 60)}h ${totalMin % 60}m` : `${totalMin}m`;

  return (
    <div className="relative max-w-[1080px] mx-auto px-10 2xl:px-16 pt-10">
      {/* ── Sticky header Card — cover · name · mono meta · select-all ─────── */}
      <div className="sticky top-0 z-20 mb-4">
        <div className="flex items-center gap-4 rounded-lg bg-bg-glass backdrop-blur-xl border border-border shadow-lg p-4">
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
                <Music2 size={26} className="text-text-muted" />
              </div>
            )}
          </div>

          {/* Name + mono meta */}
          <div className="min-w-0 flex-1">
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

          {/* Select-all chip */}
          <button
            type="button"
            onClick={() => (allSelected ? clearTracks() : selectAllTracks())}
            className="no-drag flex items-center gap-2 h-8 px-3 rounded-md border border-border text-[12.5px] text-text-secondary hover:bg-bg-hover transition-colors select-none shrink-0"
          >
            <Checkbox checked={allSelected} accent="spotify" />
            {allSelected ? 'Deselect all' : 'Select all'}
          </button>
        </div>
      </div>

      {/* ── Track list ─────────────────────────────────────────────────── */}
      <div
        className="rounded-lg border border-border-soft bg-bg-surface shadow-sm overflow-hidden"
        style={{ marginBottom: 96 }}
      >
        {/* Column header */}
        <div
          className="grid gap-4 px-[18px] py-2.5 bg-bg-secondary border-b border-border-soft"
          style={{ gridTemplateColumns: '26px 30px 1fr auto' }}
        >
          <span />
          <span className="font-mono text-[10.5px] tracking-[0.08em] text-text-muted uppercase text-center">
            #
          </span>
          <span className="font-mono text-[10.5px] tracking-[0.08em] text-text-muted uppercase">
            Title
          </span>
          <span className="font-mono text-[10.5px] tracking-[0.08em] text-text-muted uppercase text-right">
            Time
          </span>
        </div>

        {playlist.tracks.map((track, i) => {
          const sel = selectedTracks.has(track.id);
          return (
            <div
              key={track.id}
              role="checkbox"
              aria-checked={sel}
              aria-label={`${track.name} — ${track.artist}, ${fmtDur(track.durationMs)}`}
              tabIndex={0}
              onClick={() => toggleTrack(track.id)}
              onKeyDown={(e) => {
                if (e.key === ' ' || e.key === 'Enter') {
                  e.preventDefault();
                  toggleTrack(track.id);
                }
              }}
              className={`no-drag relative grid gap-4 h-12 items-center px-[18px] border-b border-border-soft last:border-b-0 cursor-pointer transition-colors ${
                sel ? 'bg-accent-soft' : 'hover:bg-bg-hover'
              }`}
              style={{ gridTemplateColumns: '26px 30px 1fr auto' }}
            >
              {/* Selected left bar */}
              {sel && <span aria-hidden className="absolute left-0 inset-y-0 w-0.5 bg-accent" />}

              {/* Checkbox */}
              <div className="flex items-center">
                <Checkbox checked={sel} accent="accent" />
              </div>

              {/* Track # */}
              <div className="flex items-center justify-center">
                <span className="font-mono text-[11px] tabular-nums text-text-muted">
                  {i + 1}
                </span>
              </div>

              {/* Title + artist */}
              <div className="min-w-0">
                <div className="text-sm font-medium text-text-primary truncate">
                  {track.name}
                </div>
                <div className="text-[12.5px] text-text-muted truncate">{track.artist}</div>
              </div>

              {/* Duration */}
              <div className="flex items-center">
                <span className="font-mono text-[11px] tabular-nums text-text-secondary">
                  {fmtDur(track.durationMs)}
                </span>
              </div>
            </div>
          );
        })}
      </div>

      {/* ── Sticky action bar ──────────────────────────────────────────── */}
      <div
        className="sticky bottom-0 z-10 flex items-center gap-3.5 border-t border-border -mx-10 2xl:-mx-16 px-10 2xl:px-16 py-4"
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
            onLocked={(f) => locks.nudge(`${f.toUpperCase()} is lossless — a Premium format.`)}
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
            onLocked={() => locks.nudge('Lossless audio is a Premium feature.')}
            onChange={(e) => setQuality(e.target.value)}
          />
        </div>

        {/* Destination folder */}
        <div className="flex flex-col gap-1">
          <span className="font-mono text-[10.5px] tracking-[0.08em] uppercase text-text-muted">
            Folder
          </span>
          <button
            type="button"
            onClick={pickDir}
            className="no-drag flex items-center gap-2 h-10 px-3 rounded-md bg-bg-tertiary border border-transparent text-text-secondary text-sm hover:border-accent hover:text-text-primary transition-colors"
            style={{ maxWidth: 220 }}
          >
            <FolderOpen size={15} className="text-text-muted flex-none" />
            <span className="font-mono text-xs truncate">
              {config?.outputDir || 'Choose folder…'}
            </span>
          </button>
        </div>

        <div className="flex-1" />

        {enqueueError && (
          <span role="alert" className="text-sm text-error whitespace-nowrap">
            {enqueueError}
          </span>
        )}

        {/* Selection count */}
        <span className="text-sm text-text-secondary whitespace-nowrap">
          <span className="font-mono tabular-nums font-bold text-text-primary">
            {selectedTracks.size}
          </span>{' '}
          selected
        </span>

        {/* Download */}
        <Button
          icon={Download}
          loading={enqueueing}
          disabled={selectedTracks.size === 0}
          onClick={enqueue}
        >
          Download
        </Button>
      </div>
    </div>
  );
}

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
