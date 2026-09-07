// yt-dlp wrapper: build args, resolve/validate binaries, spawn, parse progress.
// See HOW-THE-APP-WORKS.md §5 (building the job), §6 (parsing output),
// §8 (error classification) and §9 (binary resolution).
// NOTE: bare 'child_process'/'fs'/'electron' specifiers (not node:) so the
// vitest mocks in downloader.test.ts apply.
import { spawn, execSync } from 'child_process';
import { existsSync } from 'fs';
import path from 'node:path';
import { app } from 'electron';
import type {
  DownloadTask, DownloadProgress, Track, SearchResult, MediaMetadata, DownloadExtras,
  VideoContainer, VideoCodec,
} from '../shared/types.js';
import {
  LOSSLESS_FORMATS, defaultExtras, mergeExtras, SUBTITLE_MODES, SPONSORBLOCK_MODES,
  SPONSORBLOCK_CATEGORIES, VIDEO_CONTAINERS, VIDEO_CODECS, COOKIE_BROWSERS,
} from '../shared/types.js';
import { detectUrl } from './url-detector.js';

export type ErrorClass = 'retryable' | 'permanent' | 'auth' | 'geo';

const LOSSLESS = new Set<string>(LOSSLESS_FORMATS);

// ── The PO-token gate (§5) ────────────────────────────────────────────────────
// YouTube now puts most videos' adaptive (720p+) streams behind a "PO token" we do
// not mint. The effect is not an error at extraction time — it is worse than that:
// the one client that still LISTS those formats hands out URLs YouTube then answers
// with `HTTP Error 403: Forbidden` when the bytes are actually requested. Metadata
// resolves, the card renders with a title and a thumbnail, and only the download
// dies. Freshly posted videos from large channels are gated most aggressively,
// which is why "it worked yesterday on an old video" and "it fails on the trailer
// that went up this morning" are the same bug.
//
// Measured against the failing case (Marvel, X1aFkAkFASk, 2026-08-19):
//   android_vr (the default here) → lists 1080p/4K DASH → 403 on the media fetch
//   web / tv / ios / web_safari   → list no adaptive formats at all → "not available"
//   android / mweb / tv_simply    → serve, but usually only the 360p progressive
// So there is no client that is both gated-free and high quality. Downgrading every
// download to buy the 5% case would be the wrong trade — instead these clients are
// used ONLY on a retry, after the normal attempt has actually failed. A gated video
// then lands as a real file at whatever quality is obtainable instead of a red row.
//
// The gated client is EXCLUDED, not merely joined: listing it alongside the others
// (`default,android,…`) leaves its 1080p entries in the pool, the format selector
// still prefers them on quality, and the retry 403s exactly like the attempt it was
// meant to rescue. Verified both ways against X1aFkAkFASk.
const FALLBACK_PLAYER_CLIENTS = 'android,mweb,tv_simply';

// --socket-timeout bounds a stalled SOCKET, not a process that is busy (solving the
// player's JS challenge, walking a manifest) or simply wedged. Nothing else ever
// killed a metadata spawn, so its promise never settled: the renderer sat on
// "Reading the link…" with the field disabled and no way out but a restart. That is
// the shape of "sometimes it just doesn't fetch the link".
const METADATA_TIMEOUT_MS = 90_000;
const TIMEOUT_MESSAGE = 'Timed out reading that link — the site did not answer in time.';

// ── Spotify bridge scoring (§11) — all thresholds live here, one source of truth ──
const MAX_DURATION_DELTA_SEC = 12;   // beyond this a candidate is hard-rejected
const DURATION_PENALTY_WEIGHT = 0.4; // a full 12s-off (but not rejected) costs this
const UPLOADER_BOOST = 0.3;          // " - Topic" channel or the artist's own channel
const KEYWORD_PENALTY = 0.5;         // per bad keyword present in the title but not the Spotify title
const BAD_KEYWORDS = ['live', 'cover', 'remix', 'sped', '8d', 'nightcore'];

export interface ScoredCandidate { candidate: SearchResult; score: number; }

/** Tokenize to lowercase word tokens (unicode letters/digits), dropping punctuation. */
function tokenize(s: string): string[] {
  return (s || '').toLowerCase().replace(/[^\p{L}\p{N}]+/gu, ' ').split(/\s+/).filter(Boolean);
}

/** Pure, spawn-free (§11): rank ytsearch candidates for a Spotify track, best first.
 *  Candidates >MAX_DURATION_DELTA_SEC off (when both durations are known) are dropped;
 *  a missing candidate duration scores neutral (no penalty, not rejected). */
export function scoreCandidates(track: Track, candidates: SearchResult[]): ScoredCandidate[] {
  const trackSec = track.durationMs > 0 ? track.durationMs / 1000 : 0;
  const wanted = tokenize(`${track.artist} ${track.name}`);
  const spotifyText = `${track.artist} ${track.name}`.toLowerCase();
  const artistTokens = tokenize(track.artist);
  const out: ScoredCandidate[] = [];

  for (const c of candidates) {
    // Duration gate — only when BOTH durations are known (0 = missing = neutral).
    let durationPenalty = 0;
    if (c.duration > 0 && trackSec > 0) {
      const delta = Math.abs(c.duration - trackSec);
      if (delta > MAX_DURATION_DELTA_SEC) continue; // hard reject
      durationPenalty = (delta / MAX_DURATION_DELTA_SEC) * DURATION_PENALTY_WEIGHT;
    }

    const titleSet = new Set(tokenize(c.title));
    const overlap = wanted.length ? wanted.filter((t) => titleSet.has(t)).length / wanted.length : 0;

    const uploader = (c.uploader || '').toLowerCase();
    const uploaderBoost =
      uploader.endsWith('- topic') || (artistTokens.length > 0 && artistTokens.every((t) => uploader.includes(t)))
        ? UPLOADER_BOOST
        : 0;

    let keywordPenalty = 0;
    for (const kw of BAD_KEYWORDS) {
      if (titleSet.has(kw) && !spotifyText.includes(kw)) keywordPenalty += KEYWORD_PENALTY;
    }

    out.push({ candidate: c, score: overlap + uploaderBoost - durationPenalty - keywordPenalty });
  }

  return out.sort((a, b) => b.score - a.score);
}

const clamp01 = (n: number): number => Math.max(0, Math.min(1, n));

// Delimited progress template (§6): a marker + bytes|total|estimate|speed|eta.
const PROGRESS_MARKER = '[PROGRESS]';
const PROGRESS_TEMPLATE =
  `download:${PROGRESS_MARKER}%(progress.downloaded_bytes)s|%(progress.total_bytes)s` +
  `|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s`;

type Proc = ReturnType<typeof spawn>;
type ProgressCb = (progress: DownloadProgress) => void;
// filepaths: every after_move:filepath line seen (§future split-chapters use);
// filepath (2nd arg) stays the single primary path — same value as today.
type DoneCb = (task: DownloadTask, filepath: string, filepaths?: string[]) => void;
type ErrorCb = (task: DownloadTask, error: string) => void;

/** Classify a yt-dlp error so the queue knows whether to retry (§8). Conservative.
 *  Only 'retryable' triggers the backoff ladder (queue.ts) — 'auth'/'geo'/'permanent'
 *  are all reported immediately today, same as the old single 'permanent' bucket;
 *  the finer split exists for later features (#10/#46/#47) to consume. */
export function classifyError(text: string): ErrorClass {
  const t = (text || '').toLowerCase();
  const auth = [
    'sign in', 'login required', 'members-only', 'members only',
    'age-restricted', 'age restricted', 'private', 'confirm your age',
  ];
  for (const a of auth) if (t.includes(a)) return 'auth';
  const geo = ['geo', 'region', 'not available in your country'];
  for (const g of geo) if (t.includes(g)) return 'geo';
  const permanent = [
    'is unavailable', 'video unavailable', 'no longer available',
    'removed', 'deleted', 'does not exist', 'this video is not available',
    'copyright', 'drm', 'is not a valid url', 'unsupported url',
    // A premiere or a scheduled stream is not a failure and not a retry — the file
    // does not exist yet. Three backoffs would only make the user wait to be told
    // the same thing, so terminate immediately and say what is actually happening.
    'live event will begin', 'premieres in', 'this live stream recording is not available',
  ];
  for (const p of permanent) if (t.includes(p)) return 'permanent';
  const retryable = [
    'timed out', 'timeout', 'temporar',
    'http error 5',        // anchored 5xx server errors (not a bare '500')
    'too many requests',   // 429, matched by phrase not the bare number
    'connection reset', 'reset by peer', 'connection refused',
    'network', 'read error', 'unable to download',
    // The PO-token gate. These are retryable ONLY because the retry is different
    // from the attempt that failed — see needsClientFallback().
    ...GATED_SIGNS,
  ];
  for (const r of retryable) if (t.includes(r)) return 'retryable';
  return 'permanent';
}

/** yt-dlp text meaning "this player client's streams will not serve to us": a 403 on
 *  the media fetch, an empty adaptive-format list, or the tv client's reload demand.
 *  All three are the PO-token gate wearing different hats. Retrying the SAME command
 *  reproduces them exactly; the queue reads this to retry with FALLBACK_PLAYER_CLIENTS
 *  instead, which is the only thing that turns them into a file. */
const GATED_SIGNS = [
  '403: forbidden',
  'requested format is not available',
  'page needs to be reloaded',
];

export function needsClientFallback(text: string): boolean {
  const t = (text || '').toLowerCase();
  return GATED_SIGNS.some((s) => t.includes(s));
}

/** One sentence a user can act on, from yt-dlp's stderr. Unrecognised text is passed
 *  through rather than swallowed — a shape we have not seen yet must still be legible,
 *  and "something went wrong" for every failure is exactly what made a gated video,
 *  a dead link and a dropped connection indistinguishable. */
export function explainError(text: string): string {
  const raw = (text || '').trim();
  if (!raw) return 'That link could not be read.';
  if (raw.startsWith(TIMEOUT_MESSAGE)) return TIMEOUT_MESSAGE;
  const t = raw.toLowerCase();
  if (needsClientFallback(t)) {
    return 'YouTube is gating this video’s streams — the download will retry at the quality it will actually serve.';
  }
  switch (classifyError(t)) {
    case 'auth':
      return 'That post needs a sign-in — it may be private, age-restricted or members-only.';
    case 'geo':
      return 'That post is not available in your region.';
    case 'retryable':
      return 'The site did not answer — check your connection and try again.';
    default:
      if (t.includes('live event will begin') || t.includes('premieres in')) {
        return 'That video has not premiered yet — try again once it is live.';
      }
      return 'That post could not be read — it may be private, removed, or need a sign-in.';
  }
}

export class Downloader {
  private ytDlpPath: string | undefined;
  private ffmpegPath: string | undefined;
  private readonly active = new Map<string, Proc>();
  // Killed procs, keyed by identity (not taskId): a pause→resume reuses the
  // taskId, so a string key could suppress the *resumed* proc's real close (§).
  private readonly killedProcs = new WeakSet<Proc>();
  // Spotify bridge pre-passes still in flight, keyed by taskId, valued by a
  // per-attempt token. Present = a download is about to be spawned for that task
  // and may still be called off; see download().
  private readonly pendingBridge = new Map<string, object>();

  // ── Argument building (§5) ──────────────────────────────────────────────

  /** Build the full yt-dlp argument list. The target URL is the final element. */
  buildYtDlpArgs(task: DownloadTask, ffmpegPath: string | null): string[] {
    const args: string[] = [
      '--no-warnings',
      '--newline',
      '--socket-timeout', '30', // bound network stalls so a job can't hang forever
      '--continue', // resume a paused/interrupted .part instead of restarting (explicit, not relying on the default)
      '--progress-template', PROGRESS_TEMPLATE,
      '--retries', '10',
      '--concurrent-fragments', '4',
      '--print', 'after_move:filepath',
    ];

    if (ffmpegPath) args.push('--ffmpeg-location', ffmpegPath);

    // Set by the queue on a retry, never by the renderer: the previous attempt
    // failed the PO-token gate, so ask the clients whose URLs actually serve.
    if (task.fallbackProfile) {
      args.push('--extractor-args', `youtube:player_client=${FALLBACK_PLAYER_CLIENTS}`);
    }

    // Extras (§5). Absent on rows persisted before extras existed, and possibly
    // partial in a hand-edited history file, so merge over the defaults rather
    // than trusting the object's shape.
    const x: DownloadExtras = mergeExtras(task.extras);
    // Every extras value below becomes a spawn argument, and config.json is
    // hand-editable, so each string enum is checked against its const list here —
    // the last gate before the value leaves the process. An unknown value falls
    // back to the default instead of being passed through.
    const pick = <T extends string>(v: string, allowed: readonly T[], fallback: T): T =>
      (allowed as readonly string[]).includes(v) ? (v as T) : fallback;

    const container = pick(x.videoContainer, VIDEO_CONTAINERS, 'mp4');

    if (x.rateLimit) args.push('--limit-rate', x.rateLimit);
    if (x.proxy) args.push('--proxy', x.proxy);
    const cookieBrowser = pick(x.cookieBrowser, COOKIE_BROWSERS, '');
    if (cookieBrowser) args.push('--cookies-from-browser', cookieBrowser);

    if (task.isAudioOnly) {
      const codec = task.format;
      args.push('-f', 'bestaudio/best', '--extract-audio', '--audio-format', codec);
      // Codec-aware quality: bitrate for lossy, omitted for lossless (§5).
      if (!LOSSLESS.has(codec) && task.quality && task.quality !== 'lossless') {
        args.push('--audio-quality', `${task.quality}K`);
      }
    } else {
      args.push(
        '-f', this.videoSelector(task.videoQuality, container, pick(x.videoCodec, VIDEO_CODECS, 'any')),
        '--merge-output-format', container,
      );
    }

    if (task.embedMetadata) args.push('--embed-metadata');
    if (task.embedThumbnail) args.push('--embed-thumbnail');
    if (task.skipExisting) args.push('--no-overwrites');

    // Subtitles. --embed-subs alone fetches them without keeping a file;
    // --write-subs alone keeps a .srt without muxing. 'both' does both.
    const subMode = pick(x.subtitleMode, SUBTITLE_MODES, 'off');
    if (subMode !== 'off') {
      args.push('--sub-langs', x.subtitleLangs || defaultExtras.subtitleLangs);
      if (x.subtitleAuto) args.push('--write-auto-subs');
      if (subMode === 'embed' || subMode === 'both') args.push('--embed-subs');
      if (subMode === 'file' || subMode === 'both') args.push('--write-subs', '--convert-subs', 'srt');
    }

    if (x.embedChapters) args.push('--embed-chapters');
    if (x.splitChapters) args.push('--split-chapters');

    const sbMode = pick(x.sponsorBlock, SPONSORBLOCK_MODES, 'off');
    if (sbMode !== 'off') {
      const cats = x.sponsorBlockCategories.filter((c) =>
        (SPONSORBLOCK_CATEGORIES as readonly string[]).includes(c));
      if (cats.length) args.push(sbMode === 'remove' ? '--sponsorblock-remove' : '--sponsorblock-mark', cats.join(','));
    }

    if (x.writeThumbnail) args.push('--write-thumbnail');
    if (x.writeInfoJson) args.push('--write-info-json');
    if (x.writeDescription) args.push('--write-description');

    // Output template + collection sub-folder + per-collection download archive.
    // A user template replaces the name part only — outputDir stays the anchor.
    const sub = task.isPlaylist ? this.sanitize(task.playlistName) : '';
    const template = this.safeTemplate(x.outputTemplate);
    const name = template
      ? `${template}.%(ext)s`
      : task.isPlaylist ? '%(title)s.%(ext)s' : `${this.sanitize(task.title) || '%(title)s'}.%(ext)s`;
    args.push('-o', path.join(task.outputDir, sub, name));
    if (task.isPlaylist) {
      args.push('--download-archive', path.join(task.outputDir, sub, '.download-archive.txt'));
      // Tier cap (set by the boundary, not read from the renderer) — yt-dlp stops
      // after N items instead of expanding the whole collection.
      if (task.playlistLimit && task.playlistLimit > 0) {
        args.push('--playlist-end', String(Math.floor(task.playlistLimit)));
      }
    } else {
      // `watch?v=…&list=…` — what YouTube hands you for a video opened from inside
      // a playlist — is classified as a single video, but yt-dlp defaults to
      // --yes-playlist and would expand the whole list: the user sees one item in
      // the UI and gets two hundred files, and a basic install sails past its
      // collection cap without touching anything. Say the quiet part explicitly.
      args.push('--no-playlist');
    }

    // `--` ends option parsing so a URL/value starting with "-" can't be read as a
    // yt-dlp flag (e.g. --exec=…). The Spotify bridge replaces this last element.
    args.push('--', task.url);
    return args;
  }

  /** A preference with fallbacks (§5): the requested codec in the requested
   *  container at or below the requested height, degrading one constraint at a
   *  time so a video that cannot satisfy the preference still downloads.
   *  mp4 keeps the ext filters that let streams be stitched without a re-encode;
   *  mkv/webm accept anything, so constraining ext there only loses formats. */
  private videoSelector(
    quality: DownloadTask['videoQuality'],
    container: VideoContainer = 'mp4',
    codec: VideoCodec = 'any',
  ): string {
    const heights: Record<string, number> = { '360p': 360, '480p': 480, '720p': 720, '1080p': 1080, '2160p': 2160 };
    const h = heights[quality];
    const height = h ? `[height<=${h}]` : '';
    const vcodec = { any: '', h264: '[vcodec^=avc1]', vp9: '[vcodec^=vp9]', av1: '[vcodec^=av01]' }[codec];
    const ext = container === 'mp4' ? '[ext=mp4]' : '';
    const aext = container === 'mp4' ? '[ext=m4a]' : '';
    return [
      `bestvideo${height}${vcodec}${ext}+bestaudio${aext}`, // everything asked for
      `bestvideo${height}${vcodec}+bestaudio`,              // drop the container preference
      `bestvideo${height}${ext}+bestaudio${aext}`,          // drop the codec preference
      `bestvideo${height}+bestaudio`,                       // drop both
      h ? `best${height}` : 'best',                         // progressive at the height cap
      'best',
    ].join('/');
  }

  /** Accept a user output template only if it stays UNDER outputDir: no absolute
   *  path, no drive letter, no '..' segment. A template legitimately contains
   *  '/' (that is how it makes sub-folders), so slashes are allowed and only
   *  traversal is refused. An unsafe template falls back to the built-in naming
   *  rather than failing the download. */
  private safeTemplate(template: string): string {
    const t = (template || '').trim().replace(/\\/g, '/');
    if (!t) return '';
    if (t.startsWith('/') || /^[a-z]:/i.test(t)) return '';
    if (t.split('/').some((seg) => seg === '..')) return '';
    return t;
  }

  private sanitize(s: string): string {
    // Strip path/control chars, then collapse a dot-only segment ('.', '..')
    // to '' so a playlistName of '..' can't traverse above outputDir (§ trust boundary).
    return (s || '')
      .replace(/[<>:"/\\|?*\x00-\x1f]/g, '')
      .trim()
      .replace(/^\.+$/, '')
      .replace(/^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/i, '$1_');
  }

  // ── Download (§5, §6) ───────────────────────────────────────────────────

  download(task: DownloadTask, track?: Track, onProgress?: ProgressCb, onDone?: DoneCb, onError?: ErrorCb): void {
    const bin = this.getYtDlpPath();
    if (!bin) {
      onError?.(task, 'yt-dlp not found. Please ensure yt-dlp is installed or bundled in bin/.');
      return;
    }

    const args = this.buildYtDlpArgs(task, this.getFfmpegPath());
    // Spotify bridge (§4, §11): resolve audio via search instead of the (undownloadable) URL.
    // Scored pre-pass is async; non-Spotify downloads stay fully synchronous.
    //
    // The ~1s pre-pass proc is not in `active`, so cancel() cannot kill it — but it
    // CAN stop the download that was about to be spawned, and under concurrent slots
    // it has to. Cancelling inside the window used to let the deferred spawn land
    // anyway; a pause→resume in those same seconds then started a SECOND proc under
    // the same taskId, which overwrote the first in `active` and left it running,
    // unkillable, with both procs' callbacks still passing the reference guard —
    // two yt-dlp processes contending for one .part file, either able to free the
    // live job's slot.
    //
    // The token is per-ATTEMPT, not per-task: a resume replaces it, so the
    // superseded attempt fails the identity check instead of racing. Same
    // reference-equality admission the queue uses for its own callbacks.
    if (task.source === 'spotify' && track) {
      const attempt = {};
      this.pendingBridge.set(task.taskId, attempt);
      this.resolveSpotifyBridge(task, track, args)
        .catch(() => { /* keep the blind ytsearch1 fallback already written into args */ })
        .then(() => {
          if (this.pendingBridge.get(task.taskId) !== attempt) return; // cancelled or superseded
          this.pendingBridge.delete(task.taskId);
          this.spawnDownload(bin, task, args, onProgress, onDone, onError);
        });
      return;
    }

    this.spawnDownload(bin, task, args, onProgress, onDone, onError);
  }

  /** Score a ytsearch5 pre-pass and rewrite args' final URL to the best direct hit (§11).
   *  On any failure, leaves the blind ytsearch1 query in place as a fallback. */
  private async resolveSpotifyBridge(task: DownloadTask, track: Track, args: string[]): Promise<void> {
    // No shell here (argv spawn), so literal quotes reach yt-dlp verbatim — strip them.
    const artist = (track.artist || track.artists?.[0] || '').replace(/"/g, '').trim();
    const name = (track.name || '').replace(/"/g, '').trim();
    const query = `${artist} - ${name}`;
    args[args.length - 1] = `ytsearch1:${query}`; // fallback if scoring yields nothing

    const items = await this.runJson(['--dump-json', '--no-warnings', '--flat-playlist', `ytsearch5:${query}`]);
    const best = scoreCandidates(track, items.map((j) => toSearchResult(j, 'youtube')))[0];
    if (best?.candidate.url) {
      args[args.length - 1] = best.candidate.url;
      task.matchedUrl = best.candidate.url;
      task.matchConfidence = clamp01(best.score);
    }
  }

  /** Spawn the download proc and wire up progress/stdout/close/error (§5, §6). */
  private spawnDownload(bin: string, task: DownloadTask, args: string[], onProgress?: ProgressCb, onDone?: DoneCb, onError?: ErrorCb): void {
    const proc = spawn(bin, args, { windowsHide: true });
    this.active.set(task.taskId, proc);

    const progress: DownloadProgress = {
      status: 'downloading', percent: 0, speed: 0, eta: 0,
      downloaded: 0, total: 0, filename: '', error: '',
      playlistIndex: 0, playlistTotal: 0,
    };
    let lastPercent = 0;
    let finalPath = '';
    const filepaths: string[] = [];
    let errorOut = '';
    let stderrBuf = '';
    let stdoutBuf = '';

    const flush = (line: string) => {
      lastPercent = this.parseLine(line, progress, lastPercent, onProgress);
      // Only yt-dlp's own ERROR: lines — substring 'error' also hits titles/notices.
      if (/^\s*ERROR:/.test(line)) errorOut += line + '\n';
    };

    const flushStdout = (line: string) => {
      const t = line.trim();
      if (t) { filepaths.push(t); finalPath = t; } // --print after_move:filepath (§5)
    };

    // The same identity guard the close/error handlers use, and for the same
    // reason: a pause→quick-resume reuses the taskId, so output still draining
    // from the KILLED process would otherwise be attributed to its successor —
    // rewinding percent, and (if the dying proc emitted a phase marker) moving
    // the task to 'converting', from which real 'downloading' updates are no
    // longer a legal transition and the job appears wedged.
    proc.stderr?.on('data', (d: Buffer) => {
      if (this.killedProcs.has(proc)) return;
      stderrBuf += d.toString();
      const lines = stderrBuf.split('\n');
      stderrBuf = lines.pop() ?? ''; // keep the partial trailing line (§6)
      for (const line of lines) flush(line);
    });

    proc.stdout?.on('data', (d: Buffer) => {
      if (this.killedProcs.has(proc)) return;
      stdoutBuf += d.toString();
      const lines = stdoutBuf.split('\n');
      stdoutBuf = lines.pop() ?? ''; // keep the partial trailing line (§6)
      for (const line of lines) flushStdout(line);
    });

    proc.on('error', (err: Error) => {
      // Identity guard: a pause→quick-resume reuses the taskId, so only clear the
      // map if THIS proc is still the registered one — never delete the successor.
      if (this.active.get(task.taskId) === proc) this.active.delete(task.taskId);
      if (this.killedProcs.has(proc)) return; // only THIS proc was killed
      onError?.(task, err.message);
    });

    proc.on('close', (code: number | null) => {
      if (stderrBuf) flush(stderrBuf); // flush leftover on close (§6)
      if (stdoutBuf) flushStdout(stdoutBuf); // flush leftover on close (§6)
      if (this.active.get(task.taskId) === proc) this.active.delete(task.taskId);
      if (this.killedProcs.has(proc)) return; // only THIS proc was killed
      if (code === 0) {
        progress.status = 'done';
        progress.percent = 100;
        onProgress?.(progress);
        onDone?.(task, finalPath, filepaths);
      } else {
        progress.status = 'failed';
        progress.error = errorOut.trim() || `yt-dlp exited with code ${code}`;
        onProgress?.(progress);
        onError?.(task, progress.error);
      }
    });
  }

  cancel(taskId: string): void {
    // Call off a spawn still waiting behind its bridge pre-pass. Done before the
    // proc lookup because in that window there IS no proc yet — returning early
    // on `!proc` is what let the cancelled download start anyway.
    this.pendingBridge.delete(taskId);
    const proc = this.active.get(taskId);
    if (!proc) return;
    this.killedProcs.add(proc);
    this.killTree(proc);
    this.active.delete(taskId);
  }

  cancelAll(): void {
    this.pendingBridge.clear();
    for (const proc of this.active.values()) {
      this.killedProcs.add(proc);
      this.killTree(proc);
    }
    this.active.clear();
  }

  /** Kill the whole process tree. On Windows proc.kill() spares yt-dlp's ffmpeg
   *  child, orphaning it — so terminate by pid with taskkill /T (tree). */
  private killTree(proc: Proc): void {
    if (process.platform === 'win32' && proc.pid) {
      try { execSync(`taskkill /pid ${proc.pid} /T /F`, { stdio: 'ignore', timeout: 5000 }); return; }
      catch { /* fall through to a plain kill */ }
    }
    proc.kill();
  }

  // ── Progress parsing (§6) ───────────────────────────────────────────────

  /** Parse one stderr line; updates progress + emits. Returns the carried percent. */
  private parseLine(line: string, progress: DownloadProgress, lastPercent: number, onProgress?: ProgressCb): number {
    // Phase markers.
    if (line.includes('[ExtractAudio]') || line.includes('[Merger]')) {
      progress.status = 'converting';
      onProgress?.(progress);
      return lastPercent;
    }
    if (line.includes('[Metadata]') || line.includes('[EmbedThumbnail]') || line.includes('[ThumbnailsConvertor]')) {
      progress.status = 'embedding';
      onProgress?.(progress);
      return lastPercent;
    }

    // Delimited template line.
    const marker = line.indexOf(PROGRESS_MARKER);
    if (marker !== -1) {
      const [dl, total, est, speed, eta] = line.slice(marker + PROGRESS_MARKER.length).split('|');
      const downloaded = toNum(dl);
      const totalBytes = toNum(total) || toNum(est);
      if (totalBytes > 0) lastPercent = Math.min(100, (downloaded / totalBytes) * 100);
      progress.status = 'downloading';
      progress.downloaded = downloaded;
      progress.total = totalBytes;
      progress.speed = toNum(speed);
      progress.eta = toNum(eta);
      progress.percent = lastPercent;
      onProgress?.(progress);
      return lastPercent;
    }

    // Fallback: human-readable yt-dlp progress line.
    const m = line.match(/(\d+(?:\.\d+)?)%\s+of\s+~?\s*([\d.]+\s*\w+)\s+at\s+(\S+)\s+ETA\s+(\S+)/);
    if (m) {
      const parsed = parseFloat(m[1]);
      lastPercent = Math.min(100, Number.isNaN(parsed) ? lastPercent : parsed);
      progress.status = 'downloading';
      progress.percent = lastPercent;
      progress.total = this.parseSize(m[2]);
      progress.downloaded = Math.round((progress.total * lastPercent) / 100);
      progress.speed = this.parseSpeed(m[3]);
      progress.eta = this.parseEta(m[4]);
      onProgress?.(progress);
    }
    return lastPercent;
  }

  /** "1.2MiB/s" → bytes/sec. Multiplies stepwise to match the test's exact float. */
  private parseSpeed(str: string): number {
    return this.parseSize(str.replace(/\/s$/i, ''));
  }

  /** "1.2MiB" → bytes. Binary units use 1024, decimal use 1000. Unparseable → 0. */
  private parseSize(str: string): number {
    const m = str.match(/([\d.]+)\s*([KMGT]i?B|B)/i);
    if (!m) return 0;
    let value = parseFloat(m[1]);
    if (!Number.isFinite(value)) return 0;
    const unit = m[2].toUpperCase();
    const base = unit.includes('I') ? 1024 : 1000;
    const exp: Record<string, number> = { B: 0, K: 1, M: 2, G: 3, T: 4 };
    for (let i = 0; i < (exp[unit[0]] ?? 0); i++) value *= base;
    return value;
  }

  /** "00:12" / "1:02:03" → seconds. Unparseable → 0. */
  private parseEta(str: string): number {
    if (!str || /unknown|na/i.test(str)) return 0;
    const parts = str.split(':').map((p) => parseInt(p, 10));
    if (parts.some((p) => Number.isNaN(p))) return 0;
    return parts.reduce((acc, p) => acc * 60 + p, 0);
  }

  // ── Binary resolution (§9) ──────────────────────────────────────────────

  /** Ordered, cached search for yt-dlp; validated by running --version. */
  private getYtDlpPath(): string | null {
    if (this.ytDlpPath) return this.ytDlpPath;
    const exe = process.platform === 'win32' ? 'yt-dlp.exe' : 'yt-dlp';
    for (const candidate of this.binCandidates(exe)) {
      if (existsSync(candidate) && this.validateYtDlp(candidate)) {
        this.ytDlpPath = candidate;
        return candidate;
      }
    }
    // PATH / bare command as last resort, still validated (§9).
    // ponytail: spec "trusts" the bare name; we validate so a missing binary
    // surfaces as a clear error instead of a later spawn ENOENT.
    if (this.validateYtDlp('yt-dlp')) {
      this.ytDlpPath = 'yt-dlp';
      return 'yt-dlp';
    }
    return null;
  }

  /** ffmpeg only needs to exist — yt-dlp validates and drives it (§9). */
  private getFfmpegPath(): string | null {
    if (this.ffmpegPath) return this.ffmpegPath;
    const exe = process.platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg';
    for (const candidate of this.binCandidates(exe)) {
      if (existsSync(candidate)) {
        this.ffmpegPath = candidate;
        return candidate;
      }
    }
    return null;
  }

  private binCandidates(exe: string): string[] {
    const list: string[] = [];
    // #26: a self-updated engine lands in the writable userData/bin and wins over the
    // frozen bundled copy. Still --version-validated, so a corrupt download falls back.
    try { list.push(path.join(app.getPath('userData'), 'bin', exe)); } catch { /* no app (tests) */ }
    const resources = (process as { resourcesPath?: string }).resourcesPath;
    if (resources) list.push(path.join(resources, 'bin', exe));
    let appPath = '';
    try { appPath = app.getAppPath(); } catch { appPath = ''; }
    if (appPath) list.push(path.join(appPath, 'bin', exe));
    list.push(path.join(process.cwd(), 'bin', exe));
    return list;
  }

  private validateYtDlp(bin: string): boolean {
    try {
      execSync(`"${bin}" --version`, { stdio: 'ignore', timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  // ── Metadata & search (§3.1) ────────────────────────────────────────────

  async fetchMetadata(url: string): Promise<MediaMetadata> {
    // Same `watch?v=…&list=…` trap buildYtDlpArgs() guards against, and the same
    // answer — yt-dlp defaults to --yes-playlist, so a video opened from inside a
    // list (or a YouTube Mix, `list=RD…`) makes this enumerate the entire list
    // before it can answer. That is the difference between "Reading the link…"
    // resolving at once and sitting there for a minute on a single video.
    // detectUrl is the one place that decides collection-vs-single; ask it.
    const detected = detectUrl(url);
    const single = detected.isCollection ? [] : ['--no-playlist'];
    // ponytail: flat dump (§3.1) — fast, best-effort; first record is the header.
    // The raw stderr is translated HERE, at the one boundary the renderer awaits:
    // every failure used to reach the UI as the same "something went wrong", which
    // sent people to re-check a link that was never the problem.
    const items = await this.runJson(['--dump-json', '--no-warnings', '--flat-playlist', ...single, '--', url])
      .catch((err: unknown) => { throw new Error(explainError(err instanceof Error ? err.message : String(err))); });
    if (items.length === 0) throw new Error('That link returned nothing to download.');
    const head = items[0] ?? {};
    // Count is not the test — detectUrl is. A playlist holding exactly ONE track
    // dumps one record, and calling that a single video handed the renderer an
    // empty `entries` list, which it reported as "That collection is empty".
    const isCollection = detected.isCollection || items.length > 1;
    const source = platformOf(url);
    return {
      title: String(head.playlist_title ?? head.title ?? ''),
      uploader: String(head.uploader ?? head.channel ?? ''),
      duration: toNum(head.duration),
      thumbnailUrl: thumb(head),
      url: String(head.webpage_url ?? url),
      isCollection,
      entries: isCollection ? items.map((j) => toSearchResult(j, source)) : [],
      // Short-form preview extras (REDESIGN-PLAN §4.2) — best-effort per extractor;
      // compact() keeps a field absent rather than reporting a fake 0/'' downstream.
      ...compact({
        description: String(head.description ?? ''),
        width: toNum(head.width),
        height: toNum(head.height),
        viewCount: toNum(head.view_count),
        likeCount: toNum(head.like_count),
        uploadDate: String(head.upload_date ?? ''),
        extractor: String(head.extractor_key ?? head.extractor ?? ''),
      }),
    };
  }

  async searchYouTube(query: string): Promise<SearchResult[]> {
    const items = await this.runJson(['--dump-json', '--no-warnings', '--flat-playlist', `ytsearch12:${query}`]);
    return items.map((j) => toSearchResult(j, 'youtube'));
  }

  async searchSoundCloud(query: string): Promise<SearchResult[]> {
    const items = await this.runJson(['--dump-json', '--no-warnings', '--flat-playlist', `scsearch12:${query}`]);
    return items.map((j) => toSearchResult(j, 'soundcloud'));
  }

  /** Warm binary resolution off the first-search critical path (§9). */
  warmUp(): void {
    this.getYtDlpPath();
    this.getFfmpegPath();
  }

  /** #26: installed yt-dlp release tag ('2026.06.30'), or '' when unresolvable. */
  ytDlpVersion(): string {
    const bin = this.getYtDlpPath();
    if (!bin) return '';
    try {
      return execSync(`"${bin}" --version`, { timeout: 5000 }).toString().trim();
    } catch {
      return '';
    }
  }

  /** #26: drop the cached binary path so the next call re-runs the ordered search
   *  (called after the engine updater swaps in a new yt-dlp). */
  resetBinaryCache(): void {
    this.ytDlpPath = undefined;
  }

  /** Run yt-dlp in inspect mode, once, and parse newline-delimited JSON records.
   *  Every exit path settles the promise exactly once and clears the watchdog. */
  private async runJson(args: string[]): Promise<any[]> {
    try {
      return await this.runJsonOnce(args);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      // One second attempt, and only for a genuinely transient failure. A removed or
      // private video answers identically twice, so retrying it only doubles the wait
      // before the user hears the truth; a timeout has already cost them 90 seconds.
      if (msg === TIMEOUT_MESSAGE || classifyError(msg) !== 'retryable') throw err;
      return this.runJsonOnce(args);
    }
  }

  private runJsonOnce(args: string[]): Promise<any[]> {
    return new Promise((resolve, reject) => {
      const bin = this.getYtDlpPath();
      if (!bin) { reject(new Error('yt-dlp not found')); return; }
      // --socket-timeout bounds a stalled connection; --retries lets yt-dlp itself ride
      // out a hiccup mid-extraction rather than handing us a hard failure the user reads
      // as a bad link. The download path always had this; the metadata path never did.
      const proc = spawn(bin, ['--socket-timeout', '20', '--retries', '3', ...args], { windowsHide: true });
      let out = '';
      let err = '';
      let settled = false;
      // The watchdog is the only thing that can end a WEDGED proc (see METADATA_TIMEOUT_MS).
      // `settled` guards it against the ordinary close/error paths and vice versa.
      const timer = setTimeout(() => {
        if (settled) return;
        settled = true;
        this.killTree(proc);
        reject(new Error(TIMEOUT_MESSAGE));
      }, METADATA_TIMEOUT_MS);
      timer.unref?.();
      const settle = (fn: () => void): void => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        fn();
      };

      proc.stdout?.on('data', (d: Buffer) => { out += d.toString(); });
      proc.stderr?.on('data', (d: Buffer) => { err += d.toString(); });
      proc.on('error', (e: Error) => settle(() => reject(e)));
      proc.on('close', (code) => settle(() => {
        if (code !== 0 && !out.trim()) { reject(new Error(err.trim() || `yt-dlp exited with code ${code}`)); return; }
        const items: any[] = [];
        for (const line of out.split('\n')) {
          const t = line.trim();
          if (!t) continue;
          try { items.push(JSON.parse(t)); } catch { /* ignore non-JSON noise */ }
        }
        resolve(items);
      }));
    });
  }
}

function toNum(s: unknown): number {
  const n = Number(s);
  return Number.isFinite(n) ? n : 0;
}

/** Drop empty values ('' / 0) so an unreported yt-dlp field stays ABSENT rather than
 *  arriving as a fake zero the UI would render as "0 views" (§4.2 best-effort). */
function compact<T extends Record<string, unknown>>(obj: T): Partial<T> {
  return Object.fromEntries(Object.entries(obj).filter(([, v]) => v !== '' && v !== 0)) as Partial<T>;
}

function thumb(j: any): string {
  if (j?.thumbnail) return String(j.thumbnail);
  const list = j?.thumbnails;
  if (Array.isArray(list) && list.length) return String(list[list.length - 1]?.url ?? '');
  return '';
}

function toSearchResult(j: any, source: SearchResult['source']): SearchResult {
  return {
    id: String(j?.id ?? ''),
    title: String(j?.title ?? ''),
    uploader: String(j?.uploader ?? j?.channel ?? j?.uploader_id ?? ''),
    duration: toNum(j?.duration),
    thumbnailUrl: thumb(j),
    url: String(j?.webpage_url ?? j?.url ?? ''),
    source,
  };
}

function platformOf(url: string): SearchResult['source'] {
  const u = url.toLowerCase();
  if (u.includes('soundcloud')) return 'soundcloud';
  if (u.includes('spotify')) return 'spotify';
  if (u.includes('youtu')) return 'youtube';
  return 'unknown';
}
