import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Dashboard } from '@/components/tabs/Dashboard';
import { useAppStore } from '@/store';
import type { DownloadTask } from '@shared/types';

const task = (status: string, speed: number) => ({
  taskId: 't1',
  source: 'youtube',
  format: 'mp4',
  progress: { status, speed, percent: 10, eta: 0, downloaded: 0, total: 0, filename: '', error: '', playlistIndex: 0, playlistTotal: 0 },
} as unknown as DownloadTask);

afterEach(() => useAppStore.setState({ downloads: {} }));

describe('Dashboard empty Recent', () => {
  it('is one message and one action that focuses the link field, no ghost tiles', () => {
    render(<Dashboard />);
    expect(screen.getByText(/Finished downloads land here/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Paste a link' }));
    expect(screen.getByRole('textbox', { name: /link/i })).toHaveFocus();
    expect(document.querySelectorAll('[aria-hidden] .aspect-\\[16\\/9\\]')).toHaveLength(0);
  });
});

describe('Dashboard says where files go', () => {
  it('names the output folder and default format, with a Change link to Settings', async () => {
    useAppStore.setState({
      config: { outputDir: 'C:\\Music', defaultFormat: 'mp3', globalHotkey: 'Ctrl+Shift+D' } as never,
      activeTab: 'dashboard' as never,
    });
    render(<Dashboard />);
    const hint = document.getElementById('paste-status')!;
    expect(hint).toHaveTextContent('C:\\Music');
    expect(hint).toHaveTextContent(/MP3/);
    expect(hint).toHaveTextContent('Ctrl+Shift+D');
    fireEvent.click(screen.getByRole('button', { name: /change/i }));
    expect(useAppStore.getState().activeTab).toBe('settings');
    useAppStore.setState({ config: null as never });
  });
});

describe('Dashboard paste validation', () => {
  const det = (platform: string, url = 'https://example.com/x') =>
    ({ url, platform, contentType: 'video', isCollection: false, label: '' });
  const paste = (text: string) =>
    fireEvent.paste(screen.getByRole('textbox', { name: /link/i }), { clipboardData: { getData: () => text } });

  beforeEach(() => vi.clearAllMocks());

  it('says an unsupported site is not supported, and queues nothing', async () => {
    vi.mocked(window.electronAPI.url.detect).mockResolvedValueOnce(det('unknown') as never);
    render(<Dashboard />);
    paste('https://not-a-media-site.example/page');
    expect(await screen.findByText(/This site isn.t supported/)).toBeInTheDocument();
    expect(document.getElementById('paste-status')).toHaveAttribute('aria-live', 'polite');
    expect(window.electronAPI.download.start).not.toHaveBeenCalled();
  });

  it('treats a rejected detect as not supported, not a crash', async () => {
    vi.mocked(window.electronAPI.url.detect).mockRejectedValueOnce(new Error('scheme not allowed'));
    render(<Dashboard />);
    paste('file:///C:/secret.txt');
    expect(await screen.findByText(/This site isn.t supported/)).toBeInTheDocument();
    expect(window.electronAPI.download.start).not.toHaveBeenCalled();
  });

  it('queues a Bandcamp link as audio', async () => {
    // Not "Once": the generic path detects again inside batchDownload.
    vi.mocked(window.electronAPI.url.detect).mockResolvedValue(det('bandcamp', 'https://a.bandcamp.com/track/b') as never);
    vi.mocked(window.electronAPI.download.start).mockImplementation(async (t) => ({ ...task('queued', 0), ...t }) as never);
    render(<Dashboard />);
    paste('https://a.bandcamp.com/track/b');
    await waitFor(() => expect(window.electronAPI.download.start).toHaveBeenCalledWith(
      expect.objectContaining({ source: 'bandcamp', isAudioOnly: true }),
    ));
  });
});

describe('Dashboard live speed', () => {
  it('shows no speed tile or pulse dot when nothing is in flight', () => {
    render(<Dashboard />);
    expect(screen.queryByTestId('spark-now')).toBeNull();
    expect(document.querySelector('.pulse-dot')).toBeNull();
  });

  it('shows the speed tile and pulsing dot once a download is active, and drops them when it finishes', () => {
    act(() => { useAppStore.setState({ downloads: { t1: task('downloading', 2_000_000) } }); });
    render(<Dashboard />);
    expect(screen.getByTestId('spark-now')).toHaveTextContent('/s');
    expect(screen.getByRole('img', { name: /download speed/i })).toBeInTheDocument();
    expect(document.querySelector('.pulse-dot')).not.toBeNull();

    act(() => { useAppStore.setState({ downloads: { t1: task('done', 0) } }); });
    expect(screen.queryByTestId('spark-now')).toBeNull();
    expect(document.querySelector('.pulse-dot')).toBeNull();
  });
});
