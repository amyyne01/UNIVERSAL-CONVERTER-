import { act, render, screen, waitFor } from '@testing-library/react';
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
});
