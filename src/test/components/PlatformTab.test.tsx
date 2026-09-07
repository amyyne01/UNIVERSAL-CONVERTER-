import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { PlatformTab } from '@/components/tabs/PlatformTab';
import { useAppStore } from '@/store';
import type { AppConfig, Track } from '@shared/types';

const baseConfig: AppConfig = {
  outputDir: 'C:\\Downloads',
  defaultFormat: 'mp3',
  defaultQuality: '320',
  defaultVideoQuality: 'best',
  embedThumbnail: true,
  embedMetadata: true,
  skipExisting: true,
  theme: 'dark',
  rememberLastDir: true,
  autoPaste: true,
  clipboardWatch: false,
  showNotifications: true,
  rateLimit: '',
  proxy: '',
  ffmpegPath: '',
  globalHotkey: '',
  discordRichPresence: false,
  scheduleEnabled: false,
  scheduleTime: '00:00',
  scheduleDays: [],
  scheduleShutdown: false,
} as AppConfig;

describe('PlatformTab', () => {
  beforeEach(() => {
    useAppStore.setState({
      config: baseConfig,
      pendingInput: null,
      activeTab: 'youtube',
      downloads: {},
    });
    vi.clearAllMocks();
  });

  it('pendingInput from the dashboard seeds the field and auto-fetches', async () => {
    (window.electronAPI.url.detect as any).mockResolvedValue({
      url: 'https://youtube.com/watch?v=abc12345678',
      platform: 'youtube',
      contentType: 'video',
      isCollection: false,
      label: 'YouTube video',
      id: 'abc12345678',
    });
    (window.electronAPI.url.fetchMetadata as any).mockResolvedValue({
      title: 'Auto Fetched Title',
      uploader: 'Someone',
      thumbnailUrl: '',
      duration: 120,
    });

    render(<PlatformTab platform="youtube" />);
    act(() => { useAppStore.getState().setPendingInput('https://youtube.com/watch?v=abc12345678'); });

    await waitFor(() => expect(screen.getByText('Auto Fetched Title')).toBeInTheDocument());
    expect(window.electronAPI.url.detect).toHaveBeenCalledWith('https://youtube.com/watch?v=abc12345678');
    // pendingInput is consumed so it can't re-fire on re-render.
    expect(useAppStore.getState().pendingInput).toBeNull();
  });

  it('seeded config defaults do not clobber a quality the user already touched', async () => {
    render(<PlatformTab platform="youtube" />);

    const user = userEvent.setup();
    const select = screen.getByLabelText(/video quality/i) as HTMLSelectElement;
    await user.selectOptions(select, '720p');
    expect(select.value).toBe('720p');

    // Config arrives/changes later — the user's touched choice must survive.
    act(() => { useAppStore.setState({ config: { ...baseConfig, defaultVideoQuality: '2160p' } }); });
    await waitFor(() => expect(select.value).toBe('720p'));
  });

  it('a Spotify track with no spotifyUrl falls back to a ytsearch download', async () => {
    const track: Track = {
      id: 't1',
      name: 'Song Name',
      artist: 'Artist Name',
      artists: ['Artist Name'],
      album: 'Album',
      trackNumber: 1,
      durationMs: 200000,
      thumbnailUrl: '',
      spotifyUrl: undefined,
    };
    (window.electronAPI.spotify.parseUrl as any).mockResolvedValue({ type: 'track', id: 't1' });
    (window.electronAPI.spotify.fetchTrack as any).mockResolvedValue(track);

    render(<PlatformTab platform="spotify" />);
    const user = userEvent.setup();
    await user.type(screen.getByPlaceholderText(/paste a spotify link/i), 'https://open.spotify.com/track/t1');
    await user.click(screen.getByRole('button', { name: /fetch/i }));

    await waitFor(() => expect(screen.getByText('Song Name')).toBeInTheDocument());
    await user.click(screen.getByRole('button', { name: /download/i }));

    await waitFor(() => expect(window.electronAPI.download.start).toHaveBeenCalled());
    const req = (window.electronAPI.download.start as any).mock.calls[0][0];
    expect(req.url).toBe('ytsearch:Artist Name - Song Name');
  });

  // "Nothing matched" and "the fetch broke" are different outcomes: only the
  // second is an alert, and only the second offers a retry.
  it('separates an empty result set from a failed fetch', async () => {
    const user = userEvent.setup();
    (window.electronAPI.youtube.search as any).mockResolvedValue([]);

    render(<PlatformTab platform="youtube" />);
    await user.type(screen.getByPlaceholderText(/paste a youtube link/i), 'nothing');
    await user.click(screen.getByRole('button', { name: /fetch/i }));

    expect(await screen.findByText(/no results for/i)).toBeInTheDocument();
    expect(screen.queryByRole('alert')).toBeNull();

    (window.electronAPI.youtube.search as any).mockRejectedValue(new Error('offline'));
    await user.click(screen.getByRole('button', { name: /fetch/i }));

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent(/something went wrong/i);
    expect(screen.getByRole('button', { name: /try again/i })).toBeInTheDocument();
  });

  it('disables the URL input while a fetch is in flight, preventing a second submit (B19)', async () => {
    let resolveDetect: (v: any) => void;
    (window.electronAPI.url.detect as any).mockReturnValue(new Promise((r) => { resolveDetect = r; }));

    render(<PlatformTab platform="youtube" />);
    const user = userEvent.setup();
    const input = screen.getByPlaceholderText(/paste a youtube link/i);

    await user.type(input, 'https://youtube.com/watch?v=abc12345678');
    await user.click(screen.getByRole('button', { name: /fetch/i }));

    // Pre-fix, the input had no `disabled` prop, so this failed: a user could
    // press Enter again mid-fetch and fire a second detect/fetch round-trip.
    expect(input).toBeDisabled();

    resolveDetect!({ url: '', platform: 'youtube', contentType: 'video', isCollection: false, label: '', id: '' });
    await waitFor(() => expect(input).not.toBeDisabled());
  });

  // Batch import (b): a multi-line paste starts one download per link.
  it('a multi-line paste of links batches straight to the queue', async () => {
    (window.electronAPI.url.detect as any).mockImplementation((u: string) =>
      Promise.resolve({ url: u, platform: 'youtube', contentType: 'video', isCollection: false, label: 'YouTube video', id: 'x' }),
    );

    render(<PlatformTab platform="youtube" />);
    const input = screen.getByPlaceholderText(/paste a youtube link or search/i);
    const text = 'https://youtube.com/watch?v=aaaaaaaaaaa\nhttps://youtube.com/watch?v=bbbbbbbbbbb';
    fireEvent.paste(input, { clipboardData: { getData: () => text } });

    await waitFor(() => expect(window.electronAPI.download.start).toHaveBeenCalledTimes(2));
    expect(useAppStore.getState().activeTab).toBe('queue');
  });

  it('a batch pasted over the free tier limit nudges instead of downloading anything', async () => {
    useAppStore.setState({ plan: 'basic' });
    render(<PlatformTab platform="youtube" />);
    const input = screen.getByPlaceholderText(/paste a youtube link or search/i);
    const text = Array.from({ length: 6 }, (_, i) => `https://example.com/${i}`).join('\n');
    fireEvent.paste(input, { clipboardData: { getData: () => text } });

    await waitFor(() => expect(useAppStore.getState().notice?.message).toMatch(/free limit reached/i));
    expect(window.electronAPI.download.start).not.toHaveBeenCalled();
  });

  it('a single-line paste is not treated as a batch (normal paste behavior)', () => {
    render(<PlatformTab platform="youtube" />);
    const input = screen.getByPlaceholderText(/paste a youtube link or search/i);
    const event = fireEvent.paste(input, {
      clipboardData: { getData: () => 'https://youtube.com/watch?v=aaaaaaaaaaa' },
    });
    expect(event).toBe(true); // not preventDefault()-ed — default paste proceeds
    expect(window.electronAPI.url.detect).not.toHaveBeenCalled();
  });

  // The dropped file is read IN THE RENDERER (File.text()), not by asking main to
  // open a path. Main cannot verify that a drag happened, so a "read this .txt"
  // channel was an arbitrary-file-read primitive for a compromised renderer — the
  // one thing the trust boundary exists to deny. Nothing about the drop crosses
  // the bridge now except the links themselves, as ordinary download requests.
  it('reads a dropped .txt in the renderer and batches its links', async () => {
    (window.electronAPI.url.detect as any).mockResolvedValue({
      url: 'https://youtube.com/watch?v=aaaaaaaaaaa',
      platform: 'youtube',
      contentType: 'video',
      isCollection: false,
      label: 'YouTube video',
      id: 'x',
    });

    const { container } = render(<PlatformTab platform="youtube" />);
    const file = new File(['https://youtube.com/watch?v=aaaaaaaaaaa'], 'links.txt', { type: 'text/plain' });
    fireEvent.drop(container.firstElementChild!, { dataTransfer: { files: [file] } });

    await waitFor(() => expect(window.electronAPI.download.start).toHaveBeenCalled());
    // No path was ever handed across the bridge for this.
    expect((window.electronAPI as any).file).toBeUndefined();
  });

  it('rejects a dropped file that is not .txt/.csv without reading it', async () => {
    const { container } = render(<PlatformTab platform="youtube" />);
    const file = new File(['https://youtube.com/watch?v=aaaaaaaaaaa'], 'evil.exe', { type: 'application/octet-stream' });
    const spy = vi.spyOn(file, 'text');
    fireEvent.drop(container.firstElementChild!, { dataTransfer: { files: [file] } });

    expect(spy).not.toHaveBeenCalled();
    expect(window.electronAPI.download.start).not.toHaveBeenCalled();
  });
});
