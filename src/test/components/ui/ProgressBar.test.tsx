import { render, screen } from '@testing-library/react';
import { ProgressBar } from '@/components/ui/ProgressBar';
import type { DownloadStatus } from '@shared/types';

/** Find a div by class membership (avoids CSS-selector slash escaping issues). */
function hasDivWithClass(container: HTMLElement, cls: string): boolean {
  return Array.from(container.querySelectorAll('div')).some((el) =>
    el.classList.contains(cls),
  );
}

describe('ProgressBar', () => {
  it('renders without crashing for every DownloadStatus', () => {
    const statuses: DownloadStatus[] = [
      'queued', 'fetching_info', 'downloading', 'converting',
      'embedding', 'done', 'paused', 'retrying', 'failed', 'cancelled',
    ];
    for (const status of statuses) {
      const { unmount } = render(<ProgressBar percent={50} status={status} />);
      unmount();
    }
  });

  it('renders the indeterminate bar (w-2/5) for fetching_info', () => {
    const { container } = render(<ProgressBar percent={0} status="fetching_info" />);
    expect(hasDivWithClass(container, 'w-2/5')).toBe(true);
  });

  it('does not render the indeterminate bar for other statuses', () => {
    const { container } = render(<ProgressBar percent={50} status="downloading" />);
    expect(hasDivWithClass(container, 'w-2/5')).toBe(false);
  });

  it('applies bg-success for done', () => {
    const { container } = render(<ProgressBar percent={100} status="done" />);
    expect(hasDivWithClass(container, 'bg-success')).toBe(true);
  });

  it('applies bg-error for failed', () => {
    const { container } = render(<ProgressBar percent={0} status="failed" />);
    expect(hasDivWithClass(container, 'bg-error')).toBe(true);
  });

  it('applies bg-accent for downloading', () => {
    const { container } = render(<ProgressBar percent={50} status="downloading" />);
    expect(hasDivWithClass(container, 'bg-accent')).toBe(true);
  });

  it('applies bg-text-muted for queued and cancelled', () => {
    for (const status of ['queued', 'cancelled'] as const) {
      const { container, unmount } = render(<ProgressBar percent={0} status={status} />);
      expect(hasDivWithClass(container, 'bg-text-muted')).toBe(true);
      unmount();
    }
  });

  it('applies bg-warning for converting, embedding, paused, retrying', () => {
    for (const status of ['converting', 'embedding', 'paused', 'retrying'] as const) {
      const { container, unmount } = render(<ProgressBar percent={50} status={status} />);
      expect(hasDivWithClass(container, 'bg-warning')).toBe(true);
      unmount();
    }
  });

  it('does not throw for out-of-range percent values', () => {
    expect(() => render(<ProgressBar percent={-10} status="downloading" />)).not.toThrow();
    expect(() => render(<ProgressBar percent={150} status="downloading" />)).not.toThrow();
  });

  it('accepts an optional className prop', () => {
    const { container } = render(
      <ProgressBar percent={50} status="downloading" className="my-custom" />,
    );
    expect(container.firstChild).toHaveClass('my-custom');
  });

  it('exposes progressbar ARIA with valuenow/valuetext for a determinate status', () => {
    render(<ProgressBar percent={42} status="downloading" />);
    const bar = screen.getByRole('progressbar');
    expect(bar).toHaveAttribute('aria-valuemin', '0');
    expect(bar).toHaveAttribute('aria-valuemax', '100');
    expect(bar).toHaveAttribute('aria-valuenow', '42');
    expect(bar).toHaveAttribute('aria-valuetext', '42%');
  });

  it('omits aria-valuenow and sets aria-valuetext to the status for the indeterminate state', () => {
    render(<ProgressBar percent={0} status="fetching_info" />);
    const bar = screen.getByRole('progressbar');
    expect(bar).not.toHaveAttribute('aria-valuenow');
    expect(bar).toHaveAttribute('aria-valuetext', 'fetching_info');
  });

  it('sets aria-label from the optional label prop', () => {
    render(<ProgressBar percent={50} status="downloading" label="video.mp4" />);
    expect(screen.getByRole('progressbar', { name: 'video.mp4' })).toBeInTheDocument();
  });
});
