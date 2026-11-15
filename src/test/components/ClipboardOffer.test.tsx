import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { ClipboardOffer } from '@/components/ClipboardOffer';
import { useAppStore } from '@/store';

const offer = (platform: string, url: string, isCollection = false) =>
  useAppStore.getState().setClipboardOffer({ url, platform, type: 'track', id: 'x', isCollection, label: 'Link' } as never);

describe('ClipboardOffer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({ pendingInput: null, activeTab: 'dashboard' as never });
  });

  // D1: a Spotify link taken from the offer was queued without its track, so the
  // Spotify bridge never ran.
  it('fetches the Spotify track first and sends it with the download', async () => {
    const track = { id: 't1', name: 'Dreams', artist: 'Fleetwood Mac' };
    vi.mocked(window.electronAPI.spotify.fetchTrack).mockResolvedValueOnce(track as never);
    render(<ClipboardOffer />);
    offer('spotify', 'https://open.spotify.com/track/t1');
    fireEvent.click(await screen.findByRole('button', { name: /Download/ }));
    await waitFor(() => expect(window.electronAPI.download.start).toHaveBeenCalledTimes(1));
    expect(vi.mocked(window.electronAPI.download.start).mock.calls[0][0]).toMatchObject({ source: 'spotify', track });
  });

  it('hands a Spotify playlist to the Spotify tab, link included, without looking it up as a track', async () => {
    render(<ClipboardOffer />);
    offer('spotify', 'https://open.spotify.com/playlist/p1', true);
    fireEvent.click(await screen.findByRole('button', { name: /Download/ }));
    await waitFor(() => expect(useAppStore.getState().activeTab).toBe('spotify'));
    expect(useAppStore.getState().pendingInput).toBe('https://open.spotify.com/playlist/p1');
    expect(window.electronAPI.spotify.fetchTrack).not.toHaveBeenCalled();
    expect(window.electronAPI.download.start).not.toHaveBeenCalled();
  });

  it('hands a Spotify track it could not read to the Spotify tab rather than dropping it', async () => {
    vi.mocked(window.electronAPI.spotify.fetchTrack).mockRejectedValueOnce(new Error('offline'));
    render(<ClipboardOffer />);
    offer('spotify', 'https://open.spotify.com/track/t1');
    fireEvent.click(await screen.findByRole('button', { name: /Download/ }));
    await waitFor(() => expect(useAppStore.getState().pendingInput).toBe('https://open.spotify.com/track/t1'));
    expect(useAppStore.getState().activeTab).toBe('spotify');
  });

  it('starts any other link at once', async () => {
    render(<ClipboardOffer />);
    offer('youtube', 'https://youtu.be/dQw4w9WgXcQ');
    fireEvent.click(await screen.findByRole('button', { name: /Download/ }));
    await waitFor(() => expect(window.electronAPI.download.start).toHaveBeenCalledTimes(1));
    expect(window.electronAPI.spotify.fetchTrack).not.toHaveBeenCalled();
  });
});
