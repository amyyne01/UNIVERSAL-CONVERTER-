import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Dashboard } from '@/components/tabs/Dashboard';
import { useAppStore } from '@/store';
import type { SearchResult, Track } from '@shared/types';

const ytResult: SearchResult = {
  id: 'yt1', title: 'YT Song', uploader: 'YT Uploader', duration: 120, thumbnailUrl: '', url: 'https://youtu.be/yt1', source: 'youtube',
};
const scResult: SearchResult = {
  id: 'sc1', title: 'SC Track', uploader: 'SC Artist', duration: 90, thumbnailUrl: '', url: 'https://sc.com/sc1', source: 'soundcloud',
};
const spTrack: Track = {
  id: 'sp1', name: 'Sp Track', artist: 'Sp Artist', artists: ['Sp Artist'], album: '', trackNumber: 1,
  durationMs: 200000, thumbnailUrl: '', spotifyUrl: 'https://open.spotify.com/track/sp1',
};

describe('Dashboard unified search', () => {
  beforeEach(() => {
    useAppStore.setState({ downloads: {}, config: null, activeTab: 'home', pendingInput: null });
    vi.clearAllMocks();
    (window.electronAPI.youtube.search as any).mockResolvedValue([ytResult]);
    (window.electronAPI.soundcloud.search as any).mockResolvedValue([scResult]);
    (window.electronAPI.spotify.search as any).mockResolvedValue([spTrack]);
    (window.electronAPI.download.start as any).mockResolvedValue({
      taskId: 'q1', url: '', outputDir: '', format: 'mp3', quality: '320', videoQuality: 'best',
      isAudioOnly: true, isPlaylist: false, playlistName: '', embedThumbnail: true, embedMetadata: true,
      skipExisting: true, title: '', thumbnailUrl: '', duration: 0, uploader: '', createdAt: 0,
      progress: {
        status: 'queued', percent: 0, speed: 0, eta: 0, downloaded: 0, total: 0,
        filename: '', error: '', playlistIndex: 0, playlistTotal: 0,
      },
    });
  });

  it('plain text + Enter fires all three searches and renders one group per source', async () => {
    render(<Dashboard />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Paste a link'), 'lofi beats{Enter}');

    expect(window.electronAPI.youtube.search).toHaveBeenCalledWith('lofi beats');
    expect(window.electronAPI.soundcloud.search).toHaveBeenCalledWith('lofi beats');
    expect(window.electronAPI.spotify.search).toHaveBeenCalledWith('lofi beats');

    await waitFor(() => {
      expect(screen.getByText('YT Song')).toBeInTheDocument();
      expect(screen.getByText('SC Track')).toBeInTheDocument();
      expect(screen.getByText('Sp Track')).toBeInTheDocument();
    }, { timeout: 3000 });

    // Getting a row queues a download and swaps its pill to "Queued".
    await user.click(screen.getByRole('button', { name: /Download YT Song/i }));
    await waitFor(() => expect(window.electronAPI.download.start).toHaveBeenCalled(), { timeout: 3000 });
    await waitFor(() => expect(screen.getByText('Queued')).toBeInTheDocument(), { timeout: 3000 });
  });

  it('a real link clears any results on screen instead of searching', async () => {
    (window.electronAPI.url.detect as any).mockResolvedValue({
      url: 'https://youtube.com/watch?v=abc', platform: 'youtube', contentType: 'video', isCollection: false, label: 'YouTube video',
    });
    render(<Dashboard />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Paste a link'), 'lofi beats{Enter}');
    await waitFor(() => expect(screen.getByText('YT Song')).toBeInTheDocument(), { timeout: 3000 });

    await user.clear(screen.getByLabelText('Paste a link'));
    await user.type(screen.getByLabelText('Paste a link'), 'https://youtube.com/watch?v=abc{Enter}');
    await waitFor(() => expect(screen.queryByText('YT Song')).not.toBeInTheDocument(), { timeout: 3000 });
  });
});
