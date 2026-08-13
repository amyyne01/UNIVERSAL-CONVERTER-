import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { PlaylistPreview } from '@/components/PlaylistPreview';
import { useAppStore } from '@/store';
import type { Playlist, Track } from '@shared/types';

const track = (id: string, name: string): Track => ({
  id,
  name,
  artist: 'Artist',
  artists: ['Artist'],
  album: 'Album',
  trackNumber: 1,
  durationMs: 180_000,
  thumbnailUrl: '',
  spotifyUrl: `https://open.spotify.com/track/${id}`,
});

const playlist: Playlist = {
  id: 'pl1',
  name: 'Test Playlist',
  description: '',
  owner: 'Owner',
  tracks: [track('t1', 'One'), track('t2', 'Two'), track('t3', 'Three')],
  thumbnailUrl: '',
  url: 'https://open.spotify.com/playlist/pl1',
  trackCount: 3,
};

const startMock = () => vi.mocked(window.electronAPI.download.start);

describe('PlaylistPreview', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({
      currentPlaylist: playlist,
      selectedTracks: new Set(['t1', 't2']),
      downloads: {},
      plan: 'premium',
    });
    startMock().mockImplementation(async (t) => ({ ...t, taskId: `task_${t.url}` }) as never);
  });

  const clickDownload = async (user: ReturnType<typeof userEvent.setup>) => {
    await user.click(screen.getByRole('button', { name: /download/i }));
  };

  it('queues one task per selected track', async () => {
    const user = userEvent.setup();
    render(<PlaylistPreview />);
    await clickDownload(user);
    await waitFor(() => expect(startMock()).toHaveBeenCalledTimes(2));
  });

  // A clean enqueue must drop the selection: the button re-enables the moment the
  // enqueue settles, and skipExisting only skips files already on disk — so a
  // second click on a live selection duplicates jobs that are still in flight.
  it('clears the selection after a clean enqueue, so a second click cannot duplicate it', async () => {
    const user = userEvent.setup();
    render(<PlaylistPreview />);
    await clickDownload(user);

    await waitFor(() => expect(useAppStore.getState().selectedTracks.size).toBe(0));
    await clickDownload(user);
    expect(startMock()).toHaveBeenCalledTimes(2);
  });

  it('keeps the selection when a track fails, so a retry is one click away', async () => {
    const user = userEvent.setup();
    startMock().mockRejectedValueOnce(new Error('boom'));
    render(<PlaylistPreview />);
    await clickDownload(user);

    expect(await screen.findByText(/failed to queue/i)).toBeInTheDocument();
    expect(useAppStore.getState().selectedTracks.size).toBe(2);
  });

  // The basic tier takes the first N of a collection — the same ceiling the main
  // process applies to engine-expanded playlists via --playlist-end.
  it('caps a basic-tier enqueue at the collection limit and says so', async () => {
    const user = userEvent.setup();
    const many = Array.from({ length: 25 }, (_, i) => track(`x${i}`, `Track ${i}`));
    useAppStore.setState({
      plan: 'basic',
      currentPlaylist: { ...playlist, tracks: many, trackCount: 25 },
      selectedTracks: new Set(many.map((t) => t.id)),
    });

    render(<PlaylistPreview />);
    await clickDownload(user);

    await waitFor(() => expect(startMock()).toHaveBeenCalledTimes(20));
    expect(useAppStore.getState().premiumNudge).toMatch(/20 tracks per collection/);
  });
});
