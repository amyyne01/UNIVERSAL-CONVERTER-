// The Reels & Shorts result. Short-form is the one place where the thumbnail IS
// the content: a vertical clip is recognised in a glance and unrecognisable from
// a title, so this shows the real poster at its real 9:16 shape rather than the
// letterboxed 16:9 row every other platform uses.
//
// It is a REMODEL of the result, not a new pipeline: the Download button hands
// back to the same path every other tab uses, honouring the format and quality
// controls above it. Nothing here starts a download itself.
import { motion, useReducedMotion } from 'framer-motion';
import type { MediaMetadata } from '@shared/types';
import { formatDuration } from '@/lib/format';
import { Button } from '@/components/ui/Button';
import { Download } from '@/components/ui/icons';

interface ShortFormPreviewProps {
  meta: MediaMetadata;
  /** Human label from url:detect, e.g. "TikTok video" — the tab hosts several. */
  eyebrow: string;
  accent: string;
  onGet: () => void;
}

/** 12.4M rather than 12,400,000: at a glance, and it never widens the row. */
function compact(n: number): string {
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1).replace(/\.0$/, '')}B`;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1).replace(/\.0$/, '')}K`;
  return String(n);
}

/** yt-dlp reports upload_date as YYYYMMDD; anything else is left alone. */
function formatUploadDate(raw?: string): string | null {
  if (!raw || !/^\d{8}$/.test(raw)) return null;
  const d = new Date(Number(raw.slice(0, 4)), Number(raw.slice(4, 6)) - 1, Number(raw.slice(6, 8)));
  if (Number.isNaN(d.getTime())) return null;
  return d.toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
}

export function ShortFormPreview({ meta, eyebrow, accent, onGet }: ShortFormPreviewProps) {
  const reduce = useReducedMotion();

  // Every one of these is best-effort per extractor, so each is dropped rather
  // than rendered as an empty or zero value.
  const facts = [
    meta.duration ? formatDuration(meta.duration) : null,
    meta.width && meta.height ? `${meta.width}x${meta.height}` : null,
    meta.viewCount ? `${compact(meta.viewCount)} views` : null,
    meta.likeCount ? `${compact(meta.likeCount)} likes` : null,
    formatUploadDate(meta.uploadDate),
  ].filter(Boolean) as string[];

  return (
    <motion.div
      initial={reduce ? { opacity: 0 } : { opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: reduce ? 0.14 : 0.28, ease: [0.16, 1, 0.3, 1] }}
      className="rounded-lg border border-border-soft bg-bg-surface shadow-sm p-4 @min-[640px]:p-5"
    >
      <div className="flex flex-col @min-[560px]:flex-row gap-4 @min-[560px]:gap-5">
        {/* The poster keeps the clip's own shape. Capped so a tall frame cannot
            push the actions off the bottom of a small window. */}
        <div
          className="relative shrink-0 self-start w-[150px] @min-[560px]:w-[168px] aspect-[9/16] rounded-md overflow-hidden bg-bg-tertiary"
          style={{ boxShadow: `0 0 0 1px ${accent}22` }}
        >
          <img
            src={meta.thumbnailUrl}
            alt=""
            className="w-full h-full object-cover"
            loading="lazy"
            draggable={false}
          />
          {meta.duration > 0 && (
            <span className="absolute bottom-1.5 right-1.5 px-1.5 py-0.5 rounded font-mono text-[11px] tabular-nums text-white bg-black/65">
              {formatDuration(meta.duration)}
            </span>
          )}
        </div>

        <div className="min-w-0 flex-1 flex flex-col">
          <span className="font-mono text-[11px] uppercase tracking-[0.14em]" style={{ color: accent }}>
            {eyebrow}
          </span>

          <h3 className="mt-2 text-[19px] @min-[640px]:text-[21px] font-semibold tracking-tight text-text-primary leading-snug line-clamp-2">
            {meta.title}
          </h3>

          {meta.uploader && (
            <p className="mt-1.5 text-[13px] text-text-secondary truncate">{meta.uploader}</p>
          )}

          {/* The caption is often the whole point of a short-form post, but it can
              also be forty hashtags — three lines, then it stops. */}
          {meta.description && meta.description !== meta.title && (
            <p className="mt-3 text-[13px] text-text-secondary leading-relaxed line-clamp-3">
              {meta.description}
            </p>
          )}

          {facts.length > 0 && (
            <ul className="mt-3 flex flex-wrap items-center gap-x-2.5 gap-y-1 font-mono text-[11.5px] tabular-nums text-text-secondary">
              {facts.map((f, i) => (
                <li key={f} className="flex items-center gap-2.5">
                  {i > 0 && <span aria-hidden className="text-text-muted">·</span>}
                  {f}
                </li>
              ))}
            </ul>
          )}

          <div className="mt-auto pt-4">
            <Button variant="primary" icon={Download} onClick={onGet}>
              Download
            </Button>
          </div>
        </div>
      </div>
    </motion.div>
  );
}
