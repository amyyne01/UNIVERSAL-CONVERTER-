import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CommandPalette } from '@/components/CommandPalette';
import { useAppStore } from '@/store';
import type { AppConfig, DownloadProgress, DownloadTask } from '@shared/types';

const config: AppConfig = {
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

describe('CommandPalette', () => {
  beforeEach(() => {
    useAppStore.setState({ commandPaletteOpen: false, config, downloads: {}, selectedDownloadIds: new Set() });
    vi.clearAllMocks();
    // jsdom doesn't implement scrollIntoView; the palette calls it when the
    // active row changes.
    Element.prototype.scrollIntoView = vi.fn();
  });

  it('Ctrl+K toggles the palette open and closed', async () => {
    render(<CommandPalette />);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    await userEvent.keyboard('{Control>}k{/Control}');
    expect(await screen.findByRole('dialog')).toBeInTheDocument();

    await userEvent.keyboard('{Control>}k{/Control}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('traps Tab focus inside the panel while open', async () => {
    useAppStore.setState({ commandPaletteOpen: true });
    render(<CommandPalette />);
    await screen.findByRole('dialog');

    const preventDefault = vi.fn();
    document.dispatchEvent(
      Object.assign(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }), {
        preventDefault,
      }),
    );
    expect(preventDefault).toHaveBeenCalled();
  });

  it('"Clear completed downloads" persists via the shared store action', async () => {
    const progress: DownloadProgress = {
      status: 'done', percent: 100, speed: 0, eta: 0, downloaded: 0, total: 0,
      filename: '', error: '', playlistIndex: 0, playlistTotal: 0,
    };
    const doneTask: DownloadTask = {
      taskId: 'd1', url: 'https://youtube.com/watch?v=d1', outputDir: 'C:\\Downloads',
      format: 'mp3', quality: '320', videoQuality: 'best', isAudioOnly: true, isPlaylist: false,
      playlistName: '', embedThumbnail: true, embedMetadata: true, skipExisting: true,
      title: 'Done Song', thumbnailUrl: '', duration: 100, uploader: 'Artist', source: 'youtube',
      progress,
    };
    useAppStore.getState().setDownloads([doneTask]);
    useAppStore.setState({ commandPaletteOpen: true });
    render(<CommandPalette />);
    await screen.findByRole('dialog');

    const opt = screen.getByRole('option', { name: /clear completed downloads/i });
    fireEvent.click(opt);

    expect(window.electronAPI.download.remove).toHaveBeenCalledWith(['d1']);
    expect(useAppStore.getState().downloads).toEqual({});
    expect(useAppStore.getState().commandPaletteOpen).toBe(false);
  });
});
