// Every tab must MOUNT. These crashes are uniquely expensive in this app: the
// renderer has no route-level recovery of its own, so before the ErrorBoundary
// existed a single bad field on one row unmounted the whole tree and the window
// went blank — rail, title bar and all.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { useAppStore } from '@/store';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { Dashboard } from '@/components/tabs/Dashboard';
import { DownloadsTab } from '@/components/tabs/DownloadsTab';
import { SettingsTab } from '@/components/tabs/SettingsTab';
import { PlatformTab } from '@/components/tabs/PlatformTab';
import { PlaylistPreview } from '@/components/PlaylistPreview';
import type { DownloadTask } from '@shared/types';

const task = {
  taskId: 't2', url: 'https://youtu.be/dQw4w9WgXcQ', source: 'youtube', outputDir: 'C:\\d',
  format: 'mp4', quality: '320', videoQuality: 'best', isAudioOnly: false, isPlaylist: false,
  playlistName: '', embedThumbnail: true, embedMetadata: true, skipExisting: true,
  title: 'A', thumbnailUrl: '', duration: 10, uploader: 'u', createdAt: 0,
  progress: {
    status: 'done', percent: 100, speed: 0, eta: 0, downloaded: 1, total: 1,
    filename: '', error: '', playlistIndex: 0, playlistTotal: 0,
  },
} as DownloadTask;

beforeEach(() => {
  useAppStore.setState({
    downloads: {}, config: null, currentPlaylist: null, selectedDownloadIds: new Set(),
  });
});

describe('every tab mounts', () => {
  it('Dashboard, with and without finished downloads', () => {
    expect(() => render(<Dashboard />)).not.toThrow();
    useAppStore.setState({ downloads: { t2: task } });
    expect(() => render(<Dashboard />)).not.toThrow();
  });

  it('DownloadsTab', () => {
    useAppStore.setState({ downloads: { t2: task } });
    expect(() => render(<DownloadsTab />)).not.toThrow();
  });

  it('PlatformTab, for all four platforms', () => {
    for (const p of ['youtube', 'spotify', 'soundcloud', 'reels'] as const) {
      expect(() => render(<PlatformTab platform={p} />)).not.toThrow();
    }
  });

  it('PlaylistPreview, for a collection with no tracks', () => {
    useAppStore.setState({
      currentPlaylist: {
        id: 'x', name: 'n', description: '', owner: '', tracks: [],
        thumbnailUrl: '', url: '', trackCount: 0,
      },
    });
    const { container } = render(<PlaylistPreview />);
    expect(container.innerHTML).not.toBe('');
  });

  // Regression: this drew an empty pane, which reads as a broken tab rather than
  // as a tab whose data never arrived.
  it('SettingsTab says something when config never loads', () => {
    const { container } = render(<SettingsTab />);
    expect(container.innerHTML).not.toBe('');
    expect(screen.getByRole('status')).toBeInTheDocument();
  });
});

describe('ErrorBoundary', () => {
  it('contains a throwing tab instead of blanking the window', () => {
    const Boom = (): never => { throw new Error('bad row'); };
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(<ErrorBoundary><Boom /></ErrorBoundary>);
    spy.mockRestore();
    expect(screen.getByRole('alert')).toHaveTextContent('This tab couldn’t be drawn');
  });
});
