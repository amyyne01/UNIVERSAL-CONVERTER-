import { describe, it, expect, vi } from 'vitest';
import { downloadFeedback } from './feedback';
import type { DownloadTask, DownloadStatus } from '../shared/types';

const task = (status: DownloadStatus, percent = 0, title = 'Song'): DownloadTask =>
  ({ taskId: title, title, progress: { status, percent } } as DownloadTask);

const make = (enabled = true) => {
  const deps = { enabled: vi.fn(() => enabled), notify: vi.fn(), setProgress: vi.fn() };
  return { deps, fb: downloadFeedback(deps) };
};

describe('downloadFeedback (A8)', () => {
  it('fires one toast per finished download when the setting is on', () => {
    const { deps, fb } = make();
    fb.completed(task('done', 100, 'My Song'));
    expect(deps.notify).toHaveBeenCalledTimes(1);
    expect(deps.notify).toHaveBeenCalledWith('My Song');
  });

  it('stays silent when the setting is off', () => {
    const { deps, fb } = make(false);
    fb.completed(task('done', 100));
    expect(deps.notify).not.toHaveBeenCalled();
  });

  it('stays silent for a cancelled or failed download', () => {
    const { deps, fb } = make();
    fb.completed(task('cancelled'));
    fb.completed(task('failed'));
    expect(deps.notify).not.toHaveBeenCalled();
  });

  it('shows the average percent of active downloads and clears the bar at idle', () => {
    const { deps, fb } = make();
    fb.progress([task('downloading', 20, 'a'), task('converting', 60, 'b'), task('done', 100, 'c'), task('queued', 0, 'd')]);
    expect(deps.setProgress).toHaveBeenLastCalledWith(0.4);
    fb.progress([task('done', 100, 'a')]);
    expect(deps.setProgress).toHaveBeenLastCalledWith(-1);
  });

  it('only touches the taskbar when the value changes', () => {
    const { deps, fb } = make();
    fb.progress([]);
    fb.progress([]);
    expect(deps.setProgress).toHaveBeenCalledTimes(1);
  });
});
