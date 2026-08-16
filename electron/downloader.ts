// yt-dlp wrapper: build args, resolve/validate binaries, spawn, parse progress.
// See HOW-THE-APP-WORKS.md §5 (building the job), §6 (parsing output),
// §8 (error classification) and §9 (binary resolution).
// NOTE: bare 'child_process'/'fs'/'electron' specifiers (not node:) so the
// vitest mocks in downloader.test.ts apply.
import { spawn, execSync } from 'child_process';
import { existsSync } from 'fs';
import path from 'node:path';
import { app } from 'electron';
import type { DownloadTask, DownloadProgress, Track, SearchResult, MediaMetadata } from '../shared/types.js';
import { LOSSLESS_FORMATS } from '../shared/types.js';
import { detectUrl } from './url-detector.js';

export type ErrorClass = 'retryable' | 'permanent' | 'auth' | 'geo';

const LOSSLESS = new Set<string>(LOSSLESS_FORMATS);

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
  ];
  for (const p of permanent) if (t.includes(p)) return 'permanent';
  const retryable = [
    'timed out', 'timeout', 'temporar',
    'http error 5',        // anchored 5xx server errors (not a bare '500')
    'too many requests',   // 429, matched by phrase not the bare number
    'connection reset', 'reset by peer', 'connection refused',
    'network', 'read error', 'unable to download',
  ];
  for (const r of retryable) if (t.includes(r)) return 'retryable';
  return 'permanent';
}

export class Downloader {
  private ytDlpPath: string | undefined;
  private ffmpegPath: string | undefined;
  private readonly active = new Map<string, Proc>();
  // Killed procs, keyed by identity (not taskId): a pause→resume reuses the
  // taskId, so a string key could suppress the *resumed* proc's real close (§).
  private readonly killedProcs = new WeakSet<Proc>();

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

    if (task.isAudioOnly) {
      const codec = task.format;
      args.push('-f', 'bestaudio/best', '--extract-audio', '--audio-format', codec);
      // Codec-aware quality: bitrate for lossy, omitted for lossless (§5).
      if (!LOSSLESS.has(codec) && task.quality && task.quality !== 'lossless') {
        args.push('--audio-quality', `${task.quality}K`);
      }
    } else {
      args.push('-f', this.videoSelector(task.videoQuality), '--merge-output-format', 'mp4');
    }

    if (task.embedMetadata) args.push('--embed-metadata');
    if (task.embedThumbnail) args.push('--embed-thumbnail');
    if (task.skipExisting) args.push('--no-overwrites');

    // Output template + collection sub-folder + per-collection download archive.
    const sub = task.isPlaylist ? this.sanitize(task.playlistName) : '';
    const name = task.isPlaylist ? '%(title)s.%(ext)s' : `${this.sanitize(task.title) || '%(title)s'}.%(ext)s`;
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

  /** Prefer mp4 video+audio capped at the requested height, falling back to best. */
  private videoSelector(quality: DownloadTask['videoQuality']): string {
    const heights: Record<string, number> = { '360p': 360, '480p': 480, '720p': 720, '1080p': 1080, '2160p': 2160 };
    const h = heights[quality];
    if (!h) return 'bestvideo[ext=mp4]+bestaudio[ext=m4a]/bestvideo+bestaudio/best';
    return (
      `bestvideo[height<=${h}][ext=mp4]+bestaudio[ext=m4a]/` +
      `bestvideo[height<=${h}]+bestaudio/best[height<=${h}]/best`
    );
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
    // ponytail: the ~1s pre-pass proc isn't tracked in `active`, so a cancel during
    //           that brief window is a no-op — acceptable; the download proc IS tracked.
    if (task.source === 'spotify' && track) {
      this.resolveSpotifyBridge(task, track, args)
        .catch(() => { /* keep the blind ytsearch1 fallback already written into args */ })
        .then(() => this.spawnDownload(bin, task, args, onProgress, onDone, onError));
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
    const proc = this.active.get(taskId);
    if (!proc) return;
    this.killedProcs.add(proc);
    this.killTree(proc);
    this.active.delete(taskId);
  }

  cancelAll(): void {
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
    const items = await this.runJson(['--dump-json', '--no-warnings', '--flat-playlist', ...single, '--', url]);
    if (items.length === 0) throw new Error('No metadata returned for URL');
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

  /** Run yt-dlp in inspect mode and parse newline-delimited JSON records. */
  private runJson(args: string[]): Promise<any[]> {
    return new Promise((resolve, reject) => {
      const bin = this.getYtDlpPath();
      if (!bin) { reject(new Error('yt-dlp not found')); return; }
      // --socket-timeout bounds a stalled connection so metadata/search can't hang forever.
      const proc = spawn(bin, ['--socket-timeout', '20', ...args], { windowsHide: true });
      let out = '';
      let err = '';
      proc.stdout?.on('data', (d: Buffer) => { out += d.toString(); });
      proc.stderr?.on('data', (d: Buffer) => { err += d.toString(); });
      proc.on('error', reject);
      proc.on('close', (code) => {
        if (code !== 0 && !out.trim()) { reject(new Error(err.trim() || `yt-dlp exited with code ${code}`)); return; }
        const items: any[] = [];
        for (const line of out.split('\n')) {
          const t = line.trim();
          if (!t) continue;
          try { items.push(JSON.parse(t)); } catch { /* ignore non-JSON noise */ }
        }
        resolve(items);
      });
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
