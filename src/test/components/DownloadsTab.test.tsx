import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DownloadsTab, QueueCard } from '@/components/tabs/DownloadsTab';
import { useAppStore } from '@/store';
import type { DownloadProgress, DownloadTask } from '@shared/types';

function progress(overrides: Partial<DownloadProgress> = {}): DownloadProgress {
  return {
    status: 'downloading',
    percent: 40,
    speed: 0,
    eta: 0,
    downloaded: 0,
    total: 0,
    filename: '',
    error: '',
    playlistIndex: 0,
    playlistTotal: 0,
    ...overrides,
  };
}

function task(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    taskId: 't1',
    url: 'https://youtube.com/watch?v=t1',
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
    title: 'Song One',
    thumbnailUrl: '',
    duration: 100,
    uploader: 'Artist',
    source: 'youtube',
    progress: progress(),
    ...overrides,
  };
}

describe('DownloadsTab', () => {
  beforeEach(() => {
    useAppStore.setState({ downloads: {}, selectedDownloadIds: new Set() });
    vi.clearAllMocks();
  });

  it('pause/resume/cancel buttons call the matching electronAPI method with the task id', async () => {
    useAppStore.getState().setDownloads([task({ taskId: 't1', progress: progress({ status: 'downloading' }) })]);
    render(<DownloadsTab />);
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: 'Pause' }));
    expect(window.electronAPI.download.pause).toHaveBeenCalledWith('t1');

    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(window.electronAPI.download.cancel).toHaveBeenCalledWith('t1');
  });

  it('resume calls electronAPI.download.resume for a paused task', async () => {
    useAppStore.getState().setDownloads([task({ taskId: 't2', progress: progress({ status: 'paused' }) })]);
    render(<DownloadsTab />);
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: 'Resume' }));
    expect(window.electronAPI.download.resume).toHaveBeenCalledWith('t2');
  });

  it('select-all is scoped to the active filter, not the whole history', async () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'a', progress: progress({ status: 'downloading' }) }),
      task({ taskId: 'b', progress: progress({ status: 'done', percent: 100 }) }),
    ]);
    render(<DownloadsTab />);
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: /^active/i }));
    await user.click(screen.getByRole('button', { name: 'Toggle select all' }));

    expect(useAppStore.getState().selectedDownloadIds).toEqual(new Set(['a']));
  });

  it('clear completed removes done+cancelled tasks via IPC and locally', async () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'a', progress: progress({ status: 'done', percent: 100 }) }),
      task({ taskId: 'b', progress: progress({ status: 'cancelled' }) }),
      task({ taskId: 'c', progress: progress({ status: 'downloading' }) }),
    ]);
    render(<DownloadsTab />);
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: /clear completed/i }));

    expect(window.electronAPI.download.remove).toHaveBeenCalledWith(expect.arrayContaining(['a', 'b']));
    const remaining = Object.keys(useAppStore.getState().downloads);
    expect(remaining).toEqual(['c']);
  });

  it('cancel-all requires two clicks to confirm once more than two are active', async () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'a', progress: progress({ status: 'downloading' }) }),
      task({ taskId: 'b', progress: progress({ status: 'downloading' }) }),
      task({ taskId: 'c', progress: progress({ status: 'downloading' }) }),
    ]);
    render(<DownloadsTab />);
    const user = userEvent.setup();

    const cancelAllBtn = screen.getByRole('button', { name: /cancel all/i });
    await user.click(cancelAllBtn);
    expect(window.electronAPI.download.cancelAll).not.toHaveBeenCalled();
    expect(screen.getByText(/really cancel 3/i)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /really cancel 3/i }));
    expect(window.electronAPI.download.cancelAll).toHaveBeenCalled();
  });
});

describe('QueueCard memo comparator (B18)', () => {
  it('ignores fresh inline callback identities and only compares task + selected', () => {
    // Pre-fix, memo() had no custom comparator so `.compare` is null (React falls
    // back to a shallow compare of ALL props, including the callbacks that are
    // recreated every parent render) — this assertion fails on the old code.
    expect(typeof QueueCard.compare).toBe('function');

    const t = task();
    const base = { task: t, selected: false, onToggle: () => {}, onCancel: () => {}, onPause: () => {}, onResume: () => {}, onRetry: () => {}, onOpen: () => {}, onRemove: () => {} };
    const freshCallbacks = { ...base, onToggle: () => {}, onCancel: () => {}, onPause: () => {}, onResume: () => {}, onRetry: () => {}, onOpen: () => {}, onRemove: () => {} };

    // Same task/selected, brand-new callback refs → should be treated as equal (no re-render).
    expect(QueueCard.compare!(base, freshCallbacks)).toBe(true);

    // Different task object → must re-render.
    expect(QueueCard.compare!(base, { ...base, task: task({ taskId: 'other' }) })).toBe(false);

    // Different selected → must re-render.
    expect(QueueCard.compare!(base, { ...base, selected: true })).toBe(false);
  });
});
