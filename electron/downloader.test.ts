import { describe, it, expect, vi, beforeEach } from 'vitest';
import { EventEmitter } from 'events';

// Shared fake child process — spawn() always returns it (per testing-vitest §7a).
vi.mock('child_process', () => {
  const mockProc = new EventEmitter() as any;
  mockProc.stdout = new EventEmitter() as any;
  mockProc.stderr = new EventEmitter() as any;
  mockProc.kill = vi.fn();
  const spawn = vi.fn(() => mockProc);
  const execSync = vi.fn(() => Buffer.from('2024.01.01\n'));
  return { default: { spawn, execSync }, spawn, execSync };
});

vi.mock('electron', () => ({
  app: {
    getAppPath: vi.fn().mockReturnValue('C:\\app'),
    // #26: userData/bin is prepended to the binary search — give it a stable stub.
    getPath: vi.fn((name: string) => (name === 'userData' ? 'C:\\userdata' : '')),
  },
}));

vi.mock('fs', async () => {
  const actual = await vi.importActual('fs');
  return { ...actual, existsSync: vi.fn().mockReturnValue(true) };
});

import { Downloader, classifyError, scoreCandidates } from './downloader';
import type { DownloadTask, Track, SearchResult } from '../shared/types';
import { spawn, execSync } from 'child_process';
import { existsSync } from 'fs';

function makeTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    taskId: 'task_test',
    url: 'https://youtube.com/watch?v=dQw4w9WgXcQ',
    outputDir: 'C:\\Downloads',
    format: 'mp3',
    quality: '320',
    videoQuality: 'best',
    isAudioOnly: true,
    isPlaylist: false,
    playlistName: '',
    embedThumbnail: true,
    embedMetadata: true,
    skipExisting: true,
    title: 'Test Song',
    thumbnailUrl: '',
    duration: 212,
    uploader: 'Artist',
    source: 'youtube',
    progress: { status: 'queued', percent: 0, speed: 0, eta: 0, downloaded: 0, total: 0, filename: '', error: '', playlistIndex: 0, playlistTotal: 0 },
    ...overrides,
  };
}

describe('Downloader', () => {
  let downloader: Downloader;

  beforeEach(() => {
    vi.clearAllMocks();
    // clearAllMocks keeps implementations; restore defaults so a prior test's
    // "not found" overrides don't leak into later tests (order-independent).
    vi.mocked(existsSync).mockReturnValue(true);
    vi.mocked(execSync).mockReturnValue(Buffer.from('2024.01.01\n'));
    downloader = new Downloader();
  });

  it('builds yt-dlp args for audio download', () => {
    const task = makeTask({ isAudioOnly: true, format: 'mp3', quality: '320' });
    const args = downloader.buildYtDlpArgs(task, 'C:\\ffmpeg.exe');
    expect(args).toContain('--ffmpeg-location');
    expect(args).toContain('C:\\ffmpeg.exe');
    expect(args).toContain('-f');
    expect(args).toContain('bestaudio/best');
    expect(args).toContain('--extract-audio');
    expect(args).toContain('--audio-format');
    expect(args).toContain('mp3');
    expect(args).toContain('--audio-quality');
    expect(args).toContain('320K');
    expect(args).toContain('--embed-metadata');
    expect(args).toContain('--embed-thumbnail');
  });

  it('builds yt-dlp args for video download', () => {
    const task = makeTask({ isAudioOnly: false, videoQuality: '1080p' });
    const args = downloader.buildYtDlpArgs(task, null);
    const formatIdx = args.indexOf('-f');
    expect(args[formatIdx + 1]).toContain('height<=1080');
    expect(args).toContain('--merge-output-format');
    expect(args).toContain('mp4');
  });

  it('adds --no-overwrites when skipExisting is true', () => {
    const args = downloader.buildYtDlpArgs(makeTask({ skipExisting: true }), null);
    expect(args).toContain('--no-overwrites');
  });

  // The tier cap reaches the engine as --playlist-end, so a capped collection
  // never expands past the limit rather than being trimmed after the fact.
  it('passes playlistLimit to yt-dlp as --playlist-end on a collection', () => {
    const args = downloader.buildYtDlpArgs(makeTask({ isPlaylist: true, playlistLimit: 20 }), null);
    const idx = args.indexOf('--playlist-end');
    expect(idx).toBeGreaterThan(-1);
    expect(args[idx + 1]).toBe('20');
  });

  it('omits --playlist-end when there is no cap, and for single items', () => {
    expect(downloader.buildYtDlpArgs(makeTask({ isPlaylist: true }), null)).not.toContain('--playlist-end');
    expect(
      downloader.buildYtDlpArgs(makeTask({ isPlaylist: false, playlistLimit: 20 }), null),
    ).not.toContain('--playlist-end');
  });

  // T3: `watch?v=…&list=…` is detected as a single video, but yt-dlp defaults to
  // --yes-playlist and would expand the whole list — the UI shows one item, the
  // user gets the entire playlist, and a basic install never meets its cap.
  it('pins a single item to --no-playlist so a &list= URL cannot expand', () => {
    const args = downloader.buildYtDlpArgs(
      makeTask({ isPlaylist: false, url: 'https://www.youtube.com/watch?v=abc&list=PL123' }),
      null,
    );
    expect(args).toContain('--no-playlist');
    expect(args).not.toContain('--playlist-end');
  });

  it('does not pass --no-playlist for a real collection', () => {
    const args = downloader.buildYtDlpArgs(makeTask({ isPlaylist: true, playlistLimit: 20 }), null);
    expect(args).not.toContain('--no-playlist');
    expect(args).toContain('--playlist-end');
  });

  it('uses quality map for video format selection', () => {
    const task = makeTask({ isAudioOnly: false, videoQuality: '2160p' });
    const args = downloader.buildYtDlpArgs(task, null);
    const idx = args.indexOf('-f');
    expect(args[idx + 1]).toContain('height<=2160');
  });

  it('calls spawn with correct binary and args on download', () => {
    const task = makeTask();
    const onProgress = vi.fn();
    downloader.download(task, undefined, onProgress);
    expect(spawn).toHaveBeenCalledWith(
      expect.stringContaining('yt-dlp'),
      expect.arrayContaining(['-f', 'bestaudio/best', '--extract-audio', '--audio-format', 'mp3']),
      expect.objectContaining({ windowsHide: true })
    );
  });

  it('reports progress via callback during download', async () => {
    const task = makeTask();
    const onProgress = vi.fn();
    const onDone = vi.fn();

    downloader.download(task, undefined, onProgress, onDone);

    const proc = (spawn as any).mock.results[0].value;
    proc.stderr.emit('data', Buffer.from('[download]  45.2% of ~5.23MiB at 1.2MiB/s ETA 00:12\n'));

    expect(onProgress).toHaveBeenCalled();
    const progressCall = onProgress.mock.calls[onProgress.mock.calls.length - 1][0];
    expect(progressCall.percent).toBe(45.2);
    expect(progressCall.status).toBe('downloading');
  });

  it('calls onDone on successful completion', () => {
    const task = makeTask();
    const onDone = vi.fn();
    const onProgress = vi.fn();

    downloader.download(task, undefined, onProgress, onDone);

    const proc = (spawn as any).mock.results[0].value;
    proc.stdout.emit('data', Buffer.from('C:\\Downloads\\Test Song.mp3\n'));
    proc.emit('close', 0);

    expect(onDone).toHaveBeenCalledWith(task, expect.any(String), expect.any(Array));
    expect(onProgress).toHaveBeenCalledWith(
      expect.objectContaining({ status: 'done', percent: 100 })
    );
  });

  it('calls onError when yt-dlp not found', () => {
    vi.mocked(existsSync).mockReturnValue(false);
    vi.mocked(execSync).mockImplementation(() => { throw new Error('not found'); });

    downloader = new Downloader();
    const task = makeTask();
    const onError = vi.fn();

    downloader.download(task, undefined, undefined, undefined, onError);
    expect(onError).toHaveBeenCalledWith(
      task,
      expect.stringContaining('yt-dlp not found')
    );
  });

  it('cancels a running download', () => {
    const task = makeTask();
    downloader.download(task);
    downloader.cancel(task.taskId);
    const proc = (spawn as any).mock.results[0].value;
    expect(proc.kill).toHaveBeenCalled();
  });

  // A1-1: pause→resume reuses the taskId, so output still draining from the
  // KILLED process would be attributed to its successor — rewinding percent, or
  // (via a phase marker) parking the job in 'converting', a state real progress
  // can no longer legally leave.
  it('ignores stdio that drains from an already-killed process', () => {
    const task = makeTask();
    const onProgress = vi.fn();
    const onDone = vi.fn();
    downloader.download(task, undefined, onProgress, onDone);
    const proc = (spawn as any).mock.results[0].value;

    downloader.cancel(task.taskId);
    onProgress.mockClear();

    proc.stderr.emit('data', Buffer.from('[download]  42.0% of 10.00MiB at 1.00MiB/s ETA 00:05\n'));
    proc.stdout.emit('data', Buffer.from('C:\\Downloads\\stale.mp3\n'));
    proc.emit('close', 0);

    expect(onProgress).not.toHaveBeenCalled();
    expect(onDone).not.toHaveBeenCalled();
  });

  it('cancels all downloads', () => {
    const task1 = makeTask({ taskId: 't1' });
    const task2 = makeTask({ taskId: 't2' });
    downloader.download(task1);
    downloader.download(task2);
    downloader.cancelAll();
    expect(spawn).toHaveBeenCalledTimes(2);
  });

  it('scores a ytsearch5 pre-pass and downloads the best candidate by its direct URL (§11)', async () => {
    const task = makeTask({ source: 'spotify' });
    const track: Track = { name: 'Bohemian Rhapsody', artist: 'Queen', artists: ['Queen'], id: '1', album: 'A Night at the Opera', albumYear: '1975', trackNumber: 1, durationMs: 354000, thumbnailUrl: '' };
    downloader.download(task, track);

    // First spawn = the ytsearch5 pre-pass (runJson). Feed candidates, then close.
    const preProc = (spawn as any).mock.results[0].value;
    const live = { id: 'liv', title: 'Bohemian Rhapsody (Live Aid 1985)', uploader: 'Queen Official', duration: 360, webpage_url: 'https://youtu.be/liv' };
    const official = { id: 'off', title: 'Bohemian Rhapsody', uploader: 'Queen - Topic', duration: 354, webpage_url: 'https://youtu.be/off' };
    preProc.stdout.emit('data', Buffer.from(JSON.stringify(live) + '\n' + JSON.stringify(official) + '\n'));
    preProc.emit('close', 0);

    await vi.waitFor(() => {
      expect(spawn).toHaveBeenCalledWith(
        expect.any(String),
        expect.arrayContaining(['https://youtu.be/off']),
        expect.any(Object)
      );
    });
    expect(task.matchedUrl).toBe('https://youtu.be/off');
    expect(task.matchConfidence).toBeGreaterThan(0);
  });

  // Short-form preview metadata (REDESIGN-PLAN §4.2): the extras ride along when the
  // extractor reports them, and stay ABSENT (not 0/'') when it doesn't — the preview
  // card must be able to tell "no likes reported" from "zero likes".
  it('maps the short-form preview extras and omits the ones yt-dlp did not report', async () => {
    const p = downloader.fetchMetadata('https://www.instagram.com/reel/Dba6wNXFJ_A/');
    const proc = (spawn as any).mock.results[0].value;
    proc.stdout.emit('data', Buffer.from(JSON.stringify({
      title: 'A reel', uploader: 'someone', duration: 21,
      description: 'caption text', width: 1080, height: 1920,
      view_count: 48213, upload_date: '20260731', extractor_key: 'Instagram',
      webpage_url: 'https://www.instagram.com/reel/Dba6wNXFJ_A/',
    }) + '\n'));
    proc.emit('close', 0);

    const meta = await p;
    expect(meta).toMatchObject({
      description: 'caption text', width: 1080, height: 1920,
      viewCount: 48213, uploadDate: '20260731', extractor: 'Instagram',
    });
    expect('likeCount' in meta).toBe(false); // not reported → not invented
  });

  // `watch?v=…&list=RD…` (a YouTube Mix, or any video opened from inside a list) is a
  // SINGLE video by detection, but yt-dlp defaults to --yes-playlist and would enumerate
  // the whole list before answering — ~400 records and 20s+ for one video, with the
  // list's title landing on the card instead of the video's.
  it('reads a single video without expanding the list it was opened from', async () => {
    const p = downloader.fetchMetadata('https://www.youtube.com/watch?v=HnvTFVudNbU&list=RDHnvTFVudNbU&start_radio=1');
    expect((spawn as any).mock.calls[0][1]).toContain('--no-playlist');
    const proc = (spawn as any).mock.results[0].value;
    proc.stdout.emit('data', Buffer.from(JSON.stringify({ title: 'La la la', duration: 304 }) + '\n'));
    proc.emit('close', 0);
    await expect(p).resolves.toMatchObject({ title: 'La la la', isCollection: false });
  });

  it('still expands a real playlist link', async () => {
    const p = downloader.fetchMetadata('https://www.youtube.com/playlist?list=PLabc123');
    expect((spawn as any).mock.calls[0][1]).not.toContain('--no-playlist');
    const proc = (spawn as any).mock.results[0].value;
    proc.stdout.emit('data', Buffer.from(
      JSON.stringify({ playlist_title: 'A list', title: 'One' }) + '\n' + JSON.stringify({ title: 'Two' }) + '\n',
    ));
    proc.emit('close', 0);
    await expect(p).resolves.toMatchObject({ title: 'A list', isCollection: true });
  });

  // Multi-file stdout capture (§future split-chapters use): every after_move:filepath
  // line is collected, with the same buffered-partial-line stitching as stderr; the
  // primary reported path (2nd arg) stays last-line-wins, matching today's behavior.
  it('collects every printed filepath line while keeping the primary path unchanged', () => {
    const task = makeTask();
    const onDone = vi.fn();
    downloader.download(task, undefined, undefined, onDone);

    const proc = (spawn as any).mock.results[0].value;
    // Split across chunks, mid-line, to exercise the partial-line buffer too.
    proc.stdout.emit('data', Buffer.from('C:\\Downloads\\Song - Part1.mp3\nC:\\Down'));
    proc.stdout.emit('data', Buffer.from('loads\\Song - Part2.mp3\n'));
    proc.emit('close', 0);

    expect(onDone).toHaveBeenCalledWith(
      task,
      'C:\\Downloads\\Song - Part2.mp3',
      ['C:\\Downloads\\Song - Part1.mp3', 'C:\\Downloads\\Song - Part2.mp3']
    );
  });

  it('calls onError with the parsed stderr phrase on non-zero exit', () => {
    const task = makeTask();
    const onError = vi.fn();
    downloader.download(task, undefined, undefined, undefined, onError);

    const proc = (spawn as any).mock.results[0].value;
    proc.stderr.emit('data', Buffer.from('ERROR: [youtube] xxx: Private video. Sign in\n'));
    proc.emit('close', 1);

    expect(onError).toHaveBeenCalledWith(task, expect.stringContaining('Private video'));
    // stderr fed to classifyError must route this to the 'auth' bucket (private/sign-in).
    expect(classifyError(onError.mock.calls[0][1])).toBe('auth');
  });

  it('classifies errors conservatively (anchored HTTP codes, phrase tokens)', () => {
    expect(classifyError('ERROR: HTTP Error 503: Service Unavailable')).toBe('retryable');
    expect(classifyError('ERROR: HTTP Error 429: Too Many Requests')).toBe('retryable');
    expect(classifyError('ERROR: Private video')).toBe('auth');
    // A bare '500' inside an id/byte count must NOT be read as a 5xx error.
    expect(classifyError('ERROR: file 500abc removed')).toBe('permanent');
  });

  // Widened ErrorClass (#10/#46/#47 seam): 'auth' and 'geo' split out of the old
  // single 'permanent' bucket; queue.ts still treats them like 'permanent' (no retry).
  it('classifies login-required text as auth', () => {
    expect(classifyError('ERROR: Sign in to confirm your age')).toBe('auth');
    expect(classifyError('ERROR: Login required')).toBe('auth');
    expect(classifyError('ERROR: This video is members-only')).toBe('auth');
  });

  it('classifies region-block text as geo', () => {
    expect(classifyError('ERROR: This video is not available in your country')).toBe('geo');
    expect(classifyError('ERROR: This content is geo-restricted')).toBe('geo');
  });

  it('still classifies DRM as permanent', () => {
    expect(classifyError('ERROR: This video is DRM protected')).toBe('permanent');
  });

  it('parses speed strings correctly', () => {
    const dl = new (Downloader as any)();
    expect(dl.parseSpeed('1.2MiB/s')).toBe(1.2 * 1024 * 1024);
    expect(dl.parseSpeed('500KiB/s')).toBe(500 * 1024);
    expect(dl.parseSpeed('2.5MiB/s')).toBe(2.5 * 1024 * 1024);
    expect(dl.parseSpeed('0')).toBe(0);
  });

  // B8: a playlist/track title of a Windows reserved device name yields an
  // uncreatable path (e.g. "CON.mp3"). sanitize() must suffix it so the file
  // can actually be created.
  it('suffixes Windows reserved device names in sanitize()', () => {
    const dl = downloader as any;
    expect(dl.sanitize('CON')).toBe('CON_');
    expect(dl.sanitize('con')).toBe('con_'); // case-insensitive
    expect(dl.sanitize('COM1')).toBe('COM1_');
    expect(dl.sanitize('LPT9')).toBe('LPT9_');
    // Non-reserved names pass through untouched.
    expect(dl.sanitize('My Song')).toBe('My Song');
  });

  // B20: execSync in killTree()/validateYtDlp() had no timeout, so a hung
  // taskkill/--version call would block the single-threaded main process
  // forever. Both call sites must pass a timeout.
  it('passes a timeout to execSync for validateYtDlp and killTree', () => {
    const task = makeTask();
    downloader.download(task); // triggers validateYtDlp() via getYtDlpPath()
    downloader.cancel(task.taskId); // triggers killTree()

    for (const call of (execSync as any).mock.calls) {
      expect(call[1]).toMatchObject({ timeout: 5000 });
    }
  });

  // B21: the fallback human-readable progress regex used
  // `parseFloat(m[1]) || lastPercent`, so a real "0%" line (falsy) was
  // discarded and replaced with the stale carried percent instead of 0.
  it('does not discard a real 0% in the fallback progress branch', () => {
    const task = makeTask();
    const onProgress = vi.fn();
    downloader.download(task, undefined, onProgress);

    const proc = (spawn as any).mock.results[0].value;
    // First establish a non-zero carried percent.
    proc.stderr.emit('data', Buffer.from('[download]  45.2% of ~5.23MiB at 1.2MiB/s ETA 00:12\n'));
    // Then a genuine 0% line should be reported as 0, not stuck at 45.2.
    proc.stderr.emit('data', Buffer.from('[download]  0% of ~5.23MiB at 1.2MiB/s ETA 00:12\n'));

    const last = onProgress.mock.calls[onProgress.mock.calls.length - 1][0];
    expect(last.percent).toBe(0);
  });
});

// ── Spotify bridge scorer (§11) — pure, spawn-free ─────────────────────────
describe('scoreCandidates', () => {
  const track: Track = {
    id: '1', name: 'Blinding Lights', artist: 'The Weeknd', artists: ['The Weeknd'],
    album: 'After Hours', trackNumber: 1, durationMs: 200000, thumbnailUrl: '',
  };
  const cand = (o: Partial<SearchResult>): SearchResult => ({
    id: 'x', title: '', uploader: '', duration: 0, thumbnailUrl: '', url: 'https://y/x', source: 'youtube', ...o,
  });

  it('prefers a correct-duration official over a live version', () => {
    const official = cand({ title: 'Blinding Lights', uploader: 'The Weeknd - Topic', duration: 200, url: 'off' });
    const live = cand({ title: 'Blinding Lights (Live)', uploader: 'Some Channel', duration: 200, url: 'liv' });
    const ranked = scoreCandidates(track, [live, official]);
    expect(ranked[0].candidate.url).toBe('off');
  });

  it('rejects a candidate more than 12s off', () => {
    const bad = cand({ title: 'Blinding Lights', duration: 230, url: 'bad' }); // 30s off
    const good = cand({ title: 'Blinding Lights', duration: 201, url: 'good' });
    const ranked = scoreCandidates(track, [bad, good]);
    expect(ranked.find((r) => r.candidate.url === 'bad')).toBeUndefined();
    expect(ranked[0].candidate.url).toBe('good');
  });

  it('penalizes an obvious remix below the clean cut', () => {
    const clean = cand({ title: 'Blinding Lights', uploader: 'The Weeknd - Topic', duration: 200, url: 'clean' });
    const remix = cand({ title: 'Blinding Lights (Chromatics Remix)', uploader: 'The Weeknd - Topic', duration: 200, url: 'remix' });
    const ranked = scoreCandidates(track, [remix, clean]);
    expect(ranked[0].candidate.url).toBe('clean');
    const cleanScore = ranked.find((r) => r.candidate.url === 'clean')!.score;
    const remixScore = ranked.find((r) => r.candidate.url === 'remix')!.score;
    expect(remixScore).toBeLessThan(cleanScore);
  });

  it('treats a missing candidate duration as neutral (not rejected)', () => {
    const noDur = cand({ title: 'Blinding Lights', uploader: 'The Weeknd - Topic', duration: 0, url: 'nodur' });
    const ranked = scoreCandidates(track, [noDur]);
    expect(ranked).toHaveLength(1);
    expect(ranked[0].candidate.url).toBe('nodur');
  });
});
