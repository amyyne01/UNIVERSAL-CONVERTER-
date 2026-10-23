import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DownloadsTab, QueueCard } from '@/components/tabs/DownloadsTab';
import { useAppStore } from '@/store';
import { formatDuration } from '@/lib/format';
import type { DownloadProgress, DownloadTask } from '@shared/types';

// formatDuration runs once per QueueCard render, so its call count is a render count.
vi.mock('@/lib/format', async (orig) => {
  const m = await orig<typeof import('@/lib/format')>();
  return { ...m, formatDuration: vi.fn(m.formatDuration) };
});

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
    await user.click(screen.getByRole('button', { name: /confirm/i }));

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

  it('"Do this next" promotes a queued row and is absent on the row already at the front', async () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'live', progress: progress({ status: 'downloading' }) }),
      task({ taskId: 'q1', title: 'First waiting', progress: progress({ status: 'queued' }) }),
      task({ taskId: 'q2', title: 'Second waiting', progress: progress({ status: 'queued' }) }),
    ]);
    render(<DownloadsTab />);
    const user = userEvent.setup();

    // One button, not two: the row already at the head of the tail can't move up.
    const promote = screen.getAllByRole('button', { name: 'Do this next' });
    expect(promote).toHaveLength(1);

    await user.click(promote[0]);
    expect(window.electronAPI.queue.promote).toHaveBeenCalledWith('q2');
  });

  it('the queued tail is drag-reorderable, and the sub-band labels it', () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'live', progress: progress({ status: 'downloading' }) }),
      task({ taskId: 'q1', title: 'One', progress: progress({ status: 'queued' }) }),
      task({ taskId: 'q2', title: 'Two', progress: progress({ status: 'queued' }) }),
    ]);
    render(<DownloadsTab />);

    expect(screen.getByText(/Up next · 2/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Reorder One' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Reorder Two' })).toBeInTheDocument();
  });

  it('a single queued row keeps its sub-band but gets no handle — never a dead grabber', () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'live', progress: progress({ status: 'downloading' }) }),
      task({ taskId: 'q1', title: 'One', progress: progress({ status: 'queued' }) }),
    ]);
    render(<DownloadsTab />);

    // The band still separates the live block from what's waiting (absence of a
    // second waiting row is not absence of a wait) — only the grabber is gone.
    expect(screen.getByText(/Up next · 1/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Reorder / })).not.toBeInTheDocument();
  });

  it('keyboard reorder: grab, move, drop — committing the new order and announcing it', async () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'live', progress: progress({ status: 'downloading' }) }),
      task({ taskId: 'q1', title: 'One', progress: progress({ status: 'queued' }) }),
      task({ taskId: 'q2', title: 'Two', progress: progress({ status: 'queued' }) }),
    ]);
    render(<DownloadsTab />);
    const user = userEvent.setup();

    screen.getByRole('button', { name: 'Reorder Two' }).focus();
    await user.keyboard('{ }');           // grab
    await user.keyboard('{ArrowUp}');     // one slot up
    expect(screen.getByText('Moved to position 1 of 2')).toBeInTheDocument();
    await user.keyboard('{ }');           // drop

    expect(window.electronAPI.queue.reorder).toHaveBeenCalledWith(['q2', 'q1']);
    expect(screen.getByText('Dropped')).toBeInTheDocument();
  });

  it('Escape restores the slot a row was grabbed from and commits nothing', async () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'q1', title: 'One', progress: progress({ status: 'queued' }) }),
      task({ taskId: 'q2', title: 'Two', progress: progress({ status: 'queued' }) }),
    ]);
    render(<DownloadsTab />);
    const user = userEvent.setup();

    screen.getByRole('button', { name: 'Reorder Two' }).focus();
    await user.keyboard('{ }{ArrowUp}{Escape}');

    expect(window.electronAPI.queue.reorder).not.toHaveBeenCalled();
    expect(screen.getByText('Reorder cancelled')).toBeInTheDocument();
  });

  it('pause all / resume all swap by aggregate state and fire without a confirm step', async () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'a', progress: progress({ status: 'downloading' }) }),
      task({ taskId: 'b', progress: progress({ status: 'queued' }) }),
    ]);
    const { rerender } = render(<DownloadsTab />);
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: 'Pause all' }));
    expect(window.electronAPI.download.pauseAll).toHaveBeenCalled();

    // Every active row paused → the same control becomes Resume all.
    useAppStore.getState().setDownloads([
      task({ taskId: 'a', progress: progress({ status: 'paused' }) }),
      task({ taskId: 'b', progress: progress({ status: 'queued' }) }),
    ]);
    rerender(<DownloadsTab />);
    await user.click(screen.getByRole('button', { name: 'Resume all' }));
    expect(window.electronAPI.download.resumeAll).toHaveBeenCalled();
  });
});

describe('queue header: batch summary, retry all, search', () => {
  beforeEach(() => {
    useAppStore.setState({ downloads: {}, selectedDownloadIds: new Set() });
    vi.clearAllMocks();
  });

  const failed = (taskId: string, errorCode: 'network' | 'removed', url = `https://youtube.com/watch?v=${taskId}`) =>
    task({ taskId, url, errorCode, progress: progress({ status: 'failed', error: 'x' }) });

  it('summarises the batch once there are two or more links', () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'live', progress: progress({ status: 'downloading' }) }),
      failed('a', 'network'), failed('b', 'removed'),
    ]);
    render(<DownloadsTab />);
    expect(screen.getByText('3 links: 1 downloading, 2 failed')).toBeInTheDocument();
  });

  it('shows no summary for a single link', () => {
    useAppStore.getState().setDownloads([failed('a', 'network')]);
    render(<DownloadsTab />);
    expect(screen.queryByText(/^1 link/)).toBeNull();
  });

  it('"Retry all failed" retries only the failures a retry can fix', async () => {
    useAppStore.getState().setDownloads([failed('a', 'network'), failed('b', 'removed'), failed('c', 'network')]);
    render(<DownloadsTab />);
    await userEvent.setup().click(screen.getByRole('button', { name: /Retry all failed/ }));
    const urls = vi.mocked(window.electronAPI.download.start).mock.calls.map(([r]) => r.url);
    expect(urls.sort()).toEqual(['https://youtube.com/watch?v=a', 'https://youtube.com/watch?v=c']);
  });

  it('search filters rows by title, uploader or host', async () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'a', title: 'Alpha song', uploader: 'Ann', progress: progress({ status: 'done', percent: 100 }) }),
      task({ taskId: 'b', title: 'Beta clip', uploader: 'Bob', url: 'https://vimeo.com/1', source: 'vimeo', progress: progress({ status: 'done', percent: 100 }) }),
    ]);
    render(<DownloadsTab />);
    const user = userEvent.setup();
    const box = screen.getByRole('searchbox', { name: /search/i });
    await user.type(box, 'vimeo');
    expect(screen.queryByText('Alpha song')).toBeNull();
    expect(screen.getByText('Beta clip')).toBeInTheDocument();
    await user.clear(box);
    await user.type(box, 'ann');
    expect(screen.getByText('Alpha song')).toBeInTheDocument();
    expect(screen.queryByText('Beta clip')).toBeNull();
  });
});

describe('empty queue', () => {
  it('shows one message and no select-all, filter chips or ghost rows', () => {
    useAppStore.setState({ downloads: {}, selectedDownloadIds: new Set() });
    render(<DownloadsTab />);
    expect(screen.getByText('Your queue is empty')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /select all/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /^All\b/ })).toBeNull();
    expect(screen.getAllByRole('listitem')).toHaveLength(1);
  });
});

describe('done rows say where the file went', () => {
  beforeEach(() => {
    useAppStore.setState({ downloads: {}, selectedDownloadIds: new Set() });
  });

  it('shows the saved folder and a visible "Show in folder"', () => {
    useAppStore.getState().setDownloads([
      task({ progress: progress({ status: 'done', percent: 100, filename: 'C:\\Music\\Mix\\Song One.mp3' }) }),
    ]);
    render(<DownloadsTab />);
    expect(screen.getByText(/C:\\Music\\Mix/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Show in folder' }).className).not.toMatch(/(^|\s)opacity-0/);
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

describe('failed and retrying rows', () => {
  beforeEach(() => {
    useAppStore.setState({ downloads: {}, selectedDownloadIds: new Set() });
    vi.clearAllMocks();
  });

  it('shows the readable reason and copies the raw message on "Copy details"', async () => {
    const raw = 'ERROR: [youtube] abc: Private video. Sign in if you have been granted access';
    useAppStore.getState().setDownloads([
      task({ errorCode: 'private', progress: progress({ status: 'failed', error: raw }) }),
    ]);
    render(<DownloadsTab />);
    expect(screen.getByText(/This video is private\./)).toBeInTheDocument();
    expect(screen.queryByText(/ERROR: \[youtube\]/)).not.toBeInTheDocument();

    const user = userEvent.setup(); // installs its own clipboard stub — spy after it
    const writeText = vi.spyOn(navigator.clipboard, 'writeText');
    await user.click(screen.getByRole('button', { name: 'Copy details' }));
    expect(writeText).toHaveBeenCalledWith(raw);
  });

  it('tells apart a batch of failed, untitled rows by host and id', () => {
    const urls = [
      'https://www.youtube.com/watch?v=aaaaaaaaaaa',
      'https://www.youtube.com/watch?v=bbbbbbbbbbb',
      'https://vimeo.com/999999',
      'https://x.com/a/status/1',
      'https://a.bandcamp.com/track/b',
      'https://www.dailymotion.com/video/x8',
      'https://rumble.com/v4-clip.html',
    ];
    useAppStore.getState().setDownloads(urls.map((url, i) =>
      task({ taskId: `f${i}`, url, title: '', errorCode: 'network', progress: progress({ status: 'failed', error: 'x' }) })));
    render(<DownloadsTab />);
    const rows = screen.getAllByRole('listitem');
    const names = rows.map((r) => r.getAttribute('aria-label'));
    expect(new Set(names).size).toBe(7);
    urls.forEach((url) => expect(names.some((n) => n?.includes(url))).toBe(true));
    expect(screen.getByText('youtube.com · aaaaaaaaaaa')).toHaveAttribute('title', urls[0]);
    expect(screen.queryByText('Untitled')).toBeNull();
  });

  it('offers no Retry on a row that no retry can fix', () => {
    useAppStore.getState().setDownloads([
      task({ errorCode: 'removed', progress: progress({ status: 'failed', error: 'HTTP Error 404' }) }),
    ]);
    render(<DownloadsTab />);
    expect(screen.getByText(/couldn.t find anything at this link/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull();
  });

  it('counts a retrying row down: "Retrying in Ns (n of 3)"', () => {
    useAppStore.getState().setDownloads([
      task({ retryCount: 1, progress: progress({ status: 'retrying' }) }),
    ]);
    render(<DownloadsTab />);
    expect(screen.getByText(/Retrying in 15s \(2 of 3\)/)).toBeInTheDocument();
  });

  it('a progress tick re-renders only the row it belongs to', () => {
    useAppStore.getState().setDownloads([
      task({ taskId: 'a', duration: 11 }),
      task({ taskId: 'b', duration: 22 }),
    ]);
    render(<DownloadsTab />);
    const calls = () => vi.mocked(formatDuration).mock.calls.map(([d]) => d);
    vi.mocked(formatDuration).mockClear();
    act(() => {
      useAppStore.getState().updateDownloadProgress('a', progress({ percent: 60 }));
    });
    expect(calls().filter((d) => d === 22)).toHaveLength(0);
    expect(calls().filter((d) => d === 11).length).toBeGreaterThan(0);
  });
});
