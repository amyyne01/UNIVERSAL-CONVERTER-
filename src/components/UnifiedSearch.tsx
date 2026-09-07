// Unified search — one Enter, three concurrent searches (YouTube, SoundCloud,
// Spotify), grouped on the Dashboard below the paste field. Reuses the three
// existing per-platform search IPC channels (no new IPC) and the row/empty/
// skeleton idiom PlatformTab already established.
//
// Staleness: the PARENT remounts this component (new `key`) on every Enter, so
// a superseded query's in-flight promises land in a dead instance and are
// simply never rendered — no generation prop needed here. A per-source retry
// still needs its own guard (see `reqId` below) since retrying doesn't remount.
import { useCallback, useEffect, useRef, useState } from 'react';
import { motion, useReducedMotion } from 'framer-motion';
import { Search, Alert, Retry, Check, GoTo, ChevronDown, Download } from '@/components/ui/icons';
import { Button, Spinner } from '@/components/ui';
import { Artwork, artClass, EmptyPanel } from '@/components/tabs/PlatformTab';
import { PLATFORMS } from '@/constants';
import { useAppStore } from '@/store';
import { buildDownloadRequest, spotifyDownloadUrl } from '@/lib/download';
import { searchToItem, trackToItem, type ResultItem } from '@/lib/results';
import { formatDuration } from '@/lib/format';

type Source = 'youtube' | 'soundcloud' | 'spotify';
const SOURCES: readonly Source[] = ['youtube', 'soundcloud', 'spotify']; // fixed nav order, never arrival order

type Phase =
  | { k: 'loading' }
  | { k: 'ok'; items: ResultItem[] }
  | { k: 'empty' }
  | { k: 'error' };

const EASE: [number, number, number, number] = [0.25, 0.46, 0.45, 0.94];
const ROW_CAP = 5;

const SOURCE_LABEL: Record<Source, string> = { youtube: 'YouTube', soundcloud: 'SoundCloud', spotify: 'Spotify' };
// Literal strings, not a template — Tailwind's scanner only picks up class
// names it can see verbatim in source.
const SOURCE_TEXT_CLASS: Record<Source, string> = { youtube: 'text-youtube', soundcloud: 'text-soundcloud', spotify: 'text-spotify' };

async function fetchSource(source: Source, query: string): Promise<ResultItem[]> {
  if (source === 'spotify') return (await window.electronAPI.spotify.search(query)).map(trackToItem);
  const fn = source === 'youtube' ? window.electronAPI.youtube.search : window.electronAPI.soundcloud.search;
  return (await fn(query)).map(searchToItem);
}

export interface UnifiedSearchProps {
  query: string;
  /** Lets the field show "Searching…" vs its idle hint. */
  onBusyChange: (busy: boolean) => void;
  /** Escape inside the results list returns focus to the field (results stay). */
  onEscapeToField: () => void;
}

export function UnifiedSearch({ query, onBusyChange, onEscapeToField }: UnifiedSearchProps) {
  const reduce = useReducedMotion();
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const addDownload = useAppStore((s) => s.addDownload);

  const [phases, setPhases] = useState<Record<Source, Phase>>({
    youtube: { k: 'loading' }, soundcloud: { k: 'loading' }, spotify: { k: 'loading' },
  });
  const [expanded, setExpanded] = useState<Record<Source, boolean>>({ youtube: false, soundcloud: false, spotify: false });
  const [queuedIds, setQueuedIds] = useState<Set<string>>(new Set());
  const [queuedCount, setQueuedCount] = useState(0);
  const [queuedAnnouncement, setQueuedAnnouncement] = useState('');
  // Guards a per-source retry against a still-in-flight earlier request for
  // that SAME source landing after it (the whole-query staleness is handled
  // by the parent remounting this component instead).
  const reqId = useRef<Record<Source, number>>({ youtube: 0, soundcloud: 0, spotify: 0 });

  const run = useCallback((source: Source) => {
    const id = ++reqId.current[source];
    setPhases((p) => ({ ...p, [source]: { k: 'loading' } }));
    fetchSource(source, query)
      .then((items) => {
        if (reqId.current[source] !== id) return;
        setPhases((p) => ({ ...p, [source]: items.length ? { k: 'ok', items } : { k: 'empty' } }));
      })
      .catch(() => {
        if (reqId.current[source] !== id) return;
        setPhases((p) => ({ ...p, [source]: { k: 'error' } }));
      });
  }, [query]);

  // One line, recomputed from current state — so it updates the moment ANY
  // source's phase changes, exactly when the sighted grouped bands do too.
  const statusSummary = SOURCES.map((s) => {
    const ph = phases[s];
    const label = SOURCE_LABEL[s];
    if (ph.k === 'loading') return `${label}: searching.`;
    if (ph.k === 'error') return `${label}: search failed.`;
    if (ph.k === 'empty') return `${label}: no results.`;
    return `${label}: ${ph.items.length} result${ph.items.length === 1 ? '' : 's'}.`;
  }).join(' ');

  useEffect(() => {
    for (const s of SOURCES) run(s);
    // Only on mount — the parent remounts this component (new key) per query.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const anyLoading = SOURCES.some((s) => phases[s].k === 'loading');
  useEffect(() => { onBusyChange(anyLoading); }, [anyLoading, onBusyChange]);

  // Global roving-focus entry point: the very first result row across all
  // three groups (in fixed nav order) gets tabIndex 0, every other row -1.
  const firstRowId = SOURCES.reduce<string | null>((acc, s) => {
    if (acc) return acc;
    const ph = phases[s];
    return ph.k === 'ok' && ph.items.length > 0 ? ph.items[0].id : null;
  }, null);

  const allEmpty = SOURCES.every((s) => phases[s].k === 'empty');
  const allError = SOURCES.every((s) => phases[s].k === 'error');
  const allSettled = SOURCES.every((s) => phases[s].k !== 'loading');

  const handleDownload = useCallback((item: ResultItem, source: Source) => {
    const config = useAppStore.getState().config;
    const audioOnly = source !== 'youtube';
    void window.electronAPI.download
      .start(
        buildDownloadRequest(config, {
          url: item.track ? spotifyDownloadUrl(item.track) : item.url,
          source: item.track ? 'spotify' : source,
          title: item.title,
          uploader: item.subtitle,
          thumbnailUrl: item.thumbnailUrl,
          duration: item.duration,
          isAudioOnly: audioOnly,
          isPlaylist: false,
          format: audioOnly ? (config?.defaultFormat ?? 'mp3') : 'mp4',
          track: item.track,
        }),
      )
      .then((task) => {
        addDownload(task);
        setQueuedCount((c) => c + 1);
        setQueuedIds((prev) => new Set(prev).add(item.id));
        setQueuedAnnouncement(`Queued: ${item.title}`);
        setTimeout(() => {
          setQueuedIds((prev) => { const next = new Set(prev); next.delete(item.id); return next; });
        }, 1200);
      })
      .catch(() => {});
  }, [addDownload]);

  // Roving [data-row] focus across every group, as one list (single column).
  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') { e.preventDefault(); onEscapeToField(); return; }
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) return;
    e.preventDefault();
    const rows = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>('[data-row]'));
    if (rows.length === 0) return;
    const at = rows.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      e.key === 'Home' ? 0
      : e.key === 'End' ? rows.length - 1
      : Math.min(rows.length - 1, Math.max(0, at + (e.key === 'ArrowDown' ? 1 : -1)));
    rows[next]?.focus();
  };

  if (allSettled && allEmpty) {
    return (
      <EmptyPanel icon={Search} title={`Nothing matched “${query}” on YouTube, SoundCloud or Spotify.`}>
        <span className="text-[12.5px] text-text-secondary">Check the spelling — or paste a link directly.</span>
      </EmptyPanel>
    );
  }
  if (allSettled && allError) {
    return (
      <EmptyPanel icon={Alert} title="The search couldn’t reach YouTube, SoundCloud or Spotify.">
        <Button variant="ghost" size="sm" icon={Retry} onClick={() => SOURCES.forEach(run)}>
          Try again
        </Button>
      </EmptyPanel>
    );
  }

  return (
    <section
      role="region"
      aria-label={`Search results for “${query}”`}
      className="rounded-lg bg-bg-surface border border-border-soft shadow-sm overflow-hidden max-w-[880px]"
    >
      <div aria-live="polite" className="sr-only">{statusSummary}</div>
      <div aria-live="polite" className="sr-only">{queuedAnnouncement}</div>

      {queuedCount > 0 && (
        <div className="flex justify-end px-4 pt-3">
          <button
            type="button"
            onClick={() => setActiveTab('queue')}
            className="no-drag inline-flex items-center gap-1.5 text-[12.5px] text-accent hover:text-accent-hover focus-visible:text-accent-hover transition-colors"
          >
            View queue <GoTo size={13} />
          </button>
        </div>
      )}

      <div onKeyDown={onKeyDown}>
        {SOURCES.map((source) => (
          <SourceGroup
            key={source}
            source={source}
            phase={phases[source]}
            expanded={expanded[source]}
            onExpand={() => setExpanded((p) => ({ ...p, [source]: true }))}
            onRetry={() => run(source)}
            onDownload={(item) => handleDownload(item, source)}
            queuedIds={queuedIds}
            firstRowId={firstRowId}
            reduce={!!reduce}
          />
        ))}
      </div>
    </section>
  );
}

function SourceGroup({
  source, phase, expanded, onExpand, onRetry, onDownload, queuedIds, firstRowId, reduce,
}: {
  source: Source;
  phase: Phase;
  expanded: boolean;
  onExpand: () => void;
  onRetry: () => void;
  onDownload: (item: ResultItem) => void;
  queuedIds: Set<string>;
  firstRowId: string | null;
  reduce: boolean;
}) {
  const def = PLATFORMS.find((p) => p.key === source)!;
  const Icon = def.icon;
  const bandId = `search-band-${source}`;
  const count = phase.k === 'ok' ? phase.items.length : 0;
  const square = source !== 'youtube';

  return (
    <section aria-labelledby={bandId} aria-busy={phase.k === 'loading'} role={phase.k === 'error' ? 'status' : undefined}>
      <div className="h-8 flex items-center gap-2 px-4 bg-bg-secondary/60 border-b border-border-soft">
        <Icon size={14} className={`shrink-0 ${SOURCE_TEXT_CLASS[source]}`} />
        <h3 id={bandId} className="text-[11.5px] font-medium text-text-secondary">{def.label}</h3>
        <span className="font-mono text-[10.5px] tabular-nums text-text-muted">{count}</span>
        {phase.k === 'loading' && <Spinner size={12} className="text-text-muted" />}
        {source === 'spotify' && (
          <span className="ml-auto font-mono text-[10px] tracking-[0.06em] uppercase text-text-muted">
            matched to best audio source
          </span>
        )}
      </div>

      {phase.k === 'loading' && (
        <div className="divide-y divide-border-soft">
          {[0, 1].map((i) => (
            <div key={i} className="flex items-center gap-4 px-4 py-3">
              <motion.div
                className={`shrink-0 rounded-md bg-bg-tertiary ${artClass(square)}`}
                initial={{ opacity: reduce ? 0.6 : 0.45 }}
                animate={reduce ? undefined : { opacity: [0.45, 0.8, 0.45] }}
                transition={{ duration: 1.4, repeat: Infinity, delay: i * 0.12, ease: 'easeInOut' }}
              />
              <div className="min-w-0 flex-1 space-y-2">
                <div className="h-3.5 rounded bg-bg-tertiary" style={{ width: `${72 - i * 9}%` }} />
                <div className="h-2.5 rounded bg-bg-tertiary" style={{ width: `${38 - i * 4}%` }} />
              </div>
            </div>
          ))}
        </div>
      )}

      {phase.k === 'empty' && (
        <p className="h-12 flex items-center px-4 text-[12.5px] text-text-muted">No matches on {def.label}.</p>
      )}

      {phase.k === 'error' && (
        <div className="flex items-center gap-3 px-4 py-3">
          <Alert size={16} className="text-error shrink-0" />
          <p className="flex-1 text-[12.5px] text-text-primary">{def.label} search failed</p>
          <Button variant="ghost" size="sm" icon={Retry} onClick={onRetry}>Retry</Button>
        </div>
      )}

      {phase.k === 'ok' && (
        <ul className="divide-y divide-border-soft">
          {(expanded ? phase.items : phase.items.slice(0, ROW_CAP)).map((item, i) => (
            <motion.li
              key={item.id}
              initial={reduce ? false : { opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.18, delay: reduce ? 0 : Math.min(i, 6) * 0.025, ease: EASE }}
            >
              <ResultRow
                item={item}
                square={square}
                queued={queuedIds.has(item.id)}
                isFirst={item.id === firstRowId}
                onDownload={() => onDownload(item)}
              />
            </motion.li>
          ))}
          {!expanded && phase.items.length > ROW_CAP && (
            <li>
              <button
                type="button"
                data-row
                tabIndex={-1}
                onClick={onExpand}
                className="w-full flex items-center gap-2 px-4 h-10 text-[12.5px] text-text-secondary hover:bg-bg-hover focus-visible:bg-bg-hover transition-colors"
              >
                Show {phase.items.length - ROW_CAP} more <ChevronDown size={14} />
              </button>
            </li>
          )}
        </ul>
      )}
    </section>
  );
}

function ResultRow({
  item, square, queued, isFirst, onDownload,
}: { item: ResultItem; square: boolean; queued: boolean; isFirst: boolean; onDownload: () => void }) {
  return (
    <button
      type="button"
      data-row
      tabIndex={isFirst ? 0 : -1}
      aria-label={`Download ${item.title}${item.subtitle ? ` — ${item.subtitle}` : ''}`}
      onClick={onDownload}
      className="group w-full flex items-center gap-4 px-4 py-3 text-left transition-colors hover:bg-bg-hover focus-visible:bg-bg-hover"
    >
      <Artwork src={item.thumbnailUrl} square={square} className={artClass(square)} />
      <span className="min-w-0 flex-1">
        <span className="block text-[15px] font-medium text-text-primary truncate">{item.title}</span>
        <span className="mt-0.5 flex items-center gap-2 text-[12.5px] text-text-secondary">
          {item.subtitle && <span className="truncate">{item.subtitle}</span>}
          {item.subtitle && item.duration > 0 && <span aria-hidden>·</span>}
          {item.duration > 0 && <span className="font-mono tabular-nums shrink-0">{formatDuration(item.duration)}</span>}
        </span>
      </span>
      <span
        aria-hidden
        className={`shrink-0 inline-flex items-center gap-1.5 h-8 px-3 rounded-md border text-[12.5px] font-medium transition-colors ${
          queued
            ? 'border-success text-success'
            : 'border-border text-text-secondary group-hover:border-accent group-hover:text-accent group-focus-visible:border-accent group-focus-visible:text-accent'
        }`}
      >
        {queued ? <Check size={14} /> : <Download size={14} />}
        {queued ? 'Queued' : 'Get'}
      </span>
    </button>
  );
}
