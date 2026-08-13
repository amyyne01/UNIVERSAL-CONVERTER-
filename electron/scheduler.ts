// Unattended/scheduled downloads — HOW-THE-APP-WORKS §13.
// Polls once a minute: when scheduling is on, today is a chosen day (no days =>
// every day) and the clock matches the target HH:MM, it fires the queued
// downloads — at most once per calendar day. A startup missed-run check covers
// the machine being asleep/closed at the exact minute. Optional shutdown is
// armed ONLY after a genuine trigger.
import type { AppConfig } from '../shared/types.js';
import { BASIC_LIMITS } from '../shared/types.js';

type SchedulerConfig = Pick<
  AppConfig,
  'scheduleEnabled' | 'scheduleTime' | 'scheduleDays' | 'scheduleShutdown'
>;

export interface SchedulerDeps {
  getConfig: () => SchedulerConfig;
  onTrigger: () => void;
  onShutdown: () => void;
  /** Scheduling is a premium feature — read per tick so a mid-session tier
   *  change takes effect without a restart. */
  isPremium: () => boolean;
}

const POLL_MS = 60_000;
const MISSED_WINDOW_MIN = 60; // §13: trigger within the hour AFTER the scheduled time.

export class Scheduler {
  private timer: ReturnType<typeof setInterval> | null = null;
  /** 'YYYY-MM-DD' of the last fire — blocks a second fire on the same day. */
  private lastFiredDate = '';

  constructor(private readonly deps: SchedulerDeps) {}

  start(): void {
    if (this.timer) return;
    this.timer = setInterval(() => this.tick(), POLL_MS);
    this.timer.unref?.(); // don't keep the loop alive on our own (Electron's stays up).
  }

  stop(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  /** Startup catch-up: if now is within the hour after scheduleTime on a valid
   *  day and it hasn't fired today, trigger immediately (§13). */
  checkMissedRun(): void {
    try {
      const cfg = this.deps.getConfig();
      const now = new Date();
      const target = this.targetMinutes(cfg, now);
      if (target === null) return;
      const nowMin = now.getHours() * 60 + now.getMinutes();
      // ponytail: simple same-day window; a target near midnight won't wrap to the
      // next day. Default schedule is overnight (03:00), well clear of midnight.
      if (nowMin >= target && nowMin < target + MISSED_WINDOW_MIN) this.fire(cfg, now);
    } catch (err) {
      // A malformed config value must not crash the main process — skip this run.
      console.error('[scheduler] checkMissedRun failed', err);
    }
  }

  // ── poll ───────────────────────────────────────────────────────────────────
  // Window match, not exact-minute: if the machine slept through the target minute
  // or a GC/drift delayed a poll, firing anytime in the hour after target (gated to
  // once/day by fire()) still catches the run instead of silently missing the day.
  private tick(): void {
    try {
      const cfg = this.deps.getConfig();
      const now = new Date();
      const target = this.targetMinutes(cfg, now);
      if (target === null) return;
      const nowMin = now.getHours() * 60 + now.getMinutes();
      if (nowMin >= target && nowMin < target + MISSED_WINDOW_MIN) this.fire(cfg, now);
    } catch (err) {
      // The poll runs unattended on a timer — swallow a bad-config throw so the
      // interval keeps ticking instead of taking down the process.
      console.error('[scheduler] tick failed', err);
    }
  }

  /** The once-per-day gate + the trigger itself. Shutdown is reachable only past
   *  the gate, so it can never fire without a genuine scheduled trigger. */
  private fire(cfg: SchedulerConfig, now: Date): void {
    const today = this.dateKey(now);
    if (this.lastFiredDate === today) return;
    this.lastFiredDate = today;
    this.deps.onTrigger();
    if (cfg.scheduleShutdown) this.deps.onShutdown();
  }

  private isScheduledDay(cfg: SchedulerConfig, now: Date): boolean {
    // BASIC_LIMITS is the one definition of what free allows, so read the flag
    // rather than hardcoding the tier gate — widening the free tier there must
    // actually widen it here.
    if (!cfg.scheduleEnabled) return false;
    if (!BASIC_LIMITS.scheduler && !this.deps.isPremium()) return false;
    return cfg.scheduleDays.length === 0 || cfg.scheduleDays.includes(now.getDay());
  }

  /** scheduleTime as minutes-since-midnight, or null if scheduling is off / today
   *  isn't a chosen day / the time is unparseable. */
  private targetMinutes(cfg: SchedulerConfig, now: Date): number | null {
    if (!this.isScheduledDay(cfg, now)) return null;
    const [h, m] = cfg.scheduleTime.split(':').map(Number);
    if (!Number.isFinite(h) || !Number.isFinite(m)) return null;
    return h * 60 + m;
  }

  private dateKey(d: Date): string {
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  }
}

function pad(n: number): string {
  return String(n).padStart(2, '0');
}
