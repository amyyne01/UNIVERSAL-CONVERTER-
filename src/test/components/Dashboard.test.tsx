import { act, render, screen } from '@testing-library/react';
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
