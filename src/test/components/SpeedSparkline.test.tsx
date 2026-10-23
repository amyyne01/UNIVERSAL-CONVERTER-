import { act, render, screen } from '@testing-library/react';
import { SpeedSparkline } from '@/components/SpeedSparkline';
import { useAppStore } from '@/store';
import type { DownloadTask } from '@shared/types';

let reduced = false;
vi.mock('framer-motion', () => ({ useReducedMotion: () => reduced }));

const task = (speed: number) => ({
  taskId: 't1',
  progress: { status: 'downloading', speed },
} as unknown as DownloadTask);

const setSpeed = (speed: number) =>
  act(() => { useAppStore.setState({ downloads: { t1: task(speed) } }); });

beforeEach(() => { vi.useFakeTimers(); reduced = false; });
afterEach(() => { vi.useRealTimers(); useAppStore.setState({ downloads: {} }); });

describe('SpeedSparkline', () => {
  it('samples the store speed each second and rescales the nice ceiling', () => {
    setSpeed(1_000_000);
    render(<SpeedSparkline />);
    expect(screen.getByTestId('spark-now')).toHaveTextContent('/s');
    const first = screen.getByTestId('spark-ceil').textContent;
    setSpeed(40_000_000);
    act(() => { vi.advanceTimersByTime(3000); });
    expect(screen.getByTestId('spark-ceil').textContent).not.toBe(first);
    expect(document.querySelector('path')?.getAttribute('d')).toMatch(/^M\d+ [\d.]+L/);
  });

  it('stops sampling while the document is hidden', () => {
    setSpeed(1_000_000);
    render(<SpeedSparkline />);
    const d0 = document.querySelector('path')!.getAttribute('d');
    Object.defineProperty(document, 'visibilityState', { value: 'hidden', configurable: true });
    act(() => { document.dispatchEvent(new Event('visibilitychange')); });
    act(() => { vi.advanceTimersByTime(5000); });
    expect(document.querySelector('path')!.getAttribute('d')).toBe(d0);
    Object.defineProperty(document, 'visibilityState', { value: 'visible', configurable: true });
  });

  it('is static under reduced motion: one sample, no scroll class', () => {
    reduced = true;
    setSpeed(1_000_000);
    render(<SpeedSparkline />);
    const d0 = document.querySelector('path')!.getAttribute('d');
    act(() => { vi.advanceTimersByTime(5000); });
    expect(document.querySelector('path')!.getAttribute('d')).toBe(d0);
    expect(document.querySelector('.spark-scroll')).toBeNull();
  });
});
