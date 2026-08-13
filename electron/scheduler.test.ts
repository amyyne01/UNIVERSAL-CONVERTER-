import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { Scheduler, type SchedulerDeps } from './scheduler';

const POLL = 60_000;
// 2026-06-27 is a Saturday → getDay() === 6.
const SATURDAY = 6;
const MONDAY = 1;

function makeScheduler(
  cfg: Partial<ReturnType<SchedulerDeps['getConfig']>> = {},
  isPremium = true,
) {
  const onTrigger = vi.fn();
  const onShutdown = vi.fn();
  const sched = new Scheduler({
    getConfig: () => ({
      scheduleEnabled: true,
      scheduleTime: '03:00',
      scheduleDays: [],
      scheduleShutdown: false,
      ...cfg,
    }),
    onTrigger,
    onShutdown,
    isPremium: () => isPremium,
  });
  return { sched, onTrigger, onShutdown };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.clearAllMocks();
});

describe('Scheduler', () => {
  it('fires at the matched minute and arms shutdown only when enabled', () => {
    vi.setSystemTime(new Date('2026-06-27T02:59:30')); // 30s before target
    const { sched, onTrigger, onShutdown } = makeScheduler({ scheduleShutdown: true });
    sched.start();
    vi.advanceTimersByTime(POLL); // → 03:00:30, matches "03:00"
    expect(onTrigger).toHaveBeenCalledTimes(1);
    expect(onShutdown).toHaveBeenCalledTimes(1);
    sched.stop();
  });

  it('never fires on the basic tier, even with scheduling enabled', () => {
    vi.setSystemTime(new Date('2026-06-27T02:59:30'));
    const { sched, onTrigger, onShutdown } = makeScheduler({ scheduleShutdown: true }, false);
    sched.start();
    vi.advanceTimersByTime(POLL); // would match "03:00" on premium
    sched.checkMissedRun();
    expect(onTrigger).not.toHaveBeenCalled();
    expect(onShutdown).not.toHaveBeenCalled();
    sched.stop();
  });

  it('does not fire twice the same day', () => {
    vi.setSystemTime(new Date('2026-06-27T03:30:00')); // within the hour after 03:00
    const { sched, onTrigger } = makeScheduler();
    sched.checkMissedRun();
    sched.checkMissedRun();
    expect(onTrigger).toHaveBeenCalledTimes(1);
  });

  it('filters by weekday', () => {
    vi.setSystemTime(new Date('2026-06-27T02:59:30')); // Saturday
    const off = makeScheduler({ scheduleDays: [MONDAY] }); // Mon-only → no fire on Sat
    off.sched.start();
    vi.advanceTimersByTime(POLL);
    expect(off.onTrigger).not.toHaveBeenCalled();
    off.sched.stop();

    vi.setSystemTime(new Date('2026-06-27T02:59:30'));
    const on = makeScheduler({ scheduleDays: [SATURDAY] }); // Sat included → fires
    on.sched.start();
    vi.advanceTimersByTime(POLL);
    expect(on.onTrigger).toHaveBeenCalledTimes(1);
    on.sched.stop();
  });

  it('does not fire when scheduling is disabled', () => {
    vi.setSystemTime(new Date('2026-06-27T03:30:00'));
    const { sched, onTrigger } = makeScheduler({ scheduleEnabled: false });
    sched.checkMissedRun();
    sched.start();
    vi.advanceTimersByTime(POLL);
    expect(onTrigger).not.toHaveBeenCalled();
    sched.stop();
  });

  it('swallows a malformed config value instead of crashing the process', () => {
    vi.setSystemTime(new Date('2026-06-27T03:30:00'));
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    // scheduleTime as a number → targetMinutes' .split(':') would throw.
    const { sched, onTrigger } = makeScheduler({ scheduleTime: 300 as unknown as string });
    expect(() => sched.checkMissedRun()).not.toThrow();
    sched.start();
    expect(() => vi.advanceTimersByTime(POLL)).not.toThrow();
    expect(onTrigger).not.toHaveBeenCalled();
    expect(errSpy).toHaveBeenCalled();
    sched.stop();
    errSpy.mockRestore();
  });

  describe('checkMissedRun window', () => {
    it.each([
      ['02:30:00', false], // before target
      ['03:00:00', true], // exactly at target
      ['03:59:00', true], // last minute of the hour-after window
      ['04:30:00', false], // past the window
    ])('at %s → fired=%s', (time, fired) => {
      vi.setSystemTime(new Date(`2026-06-27T${time}`));
      const { sched, onTrigger } = makeScheduler();
      sched.checkMissedRun();
      expect(onTrigger).toHaveBeenCalledTimes(fired ? 1 : 0);
    });
  });
});
