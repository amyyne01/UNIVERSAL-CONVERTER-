//! Scheduled downloads. Polled once a minute: when scheduling is on, today is a
//! chosen day (no days means every day) and the clock is within the hour after
//! the target time, it fires, at most once per calendar day. The same check at
//! startup catches a run missed while the machine was asleep or the app closed.
//! Shutdown is reported only together with a genuine trigger.
use serde::Deserialize;

use crate::clock::LocalTime;
use crate::downloader::progress::js_number;
use crate::tier::BASIC_LIMITS;

/// How often the owner should call `poll`.
pub const POLL_MS: u64 = 60_000;
/// Trigger within the hour after the scheduled time.
const MISSED_WINDOW_MIN: f64 = 60.0;

/// The scheduling settings, as stored in the settings file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScheduleConfig {
    #[serde(rename = "scheduleEnabled")]
    pub enabled: bool,
    /// "HH:MM".
    #[serde(rename = "scheduleTime")]
    pub time: String,
    /// Weekdays, 0 for Sunday. Empty means every day.
    #[serde(rename = "scheduleDays")]
    pub days: Vec<f64>,
    #[serde(rename = "scheduleShutdown")]
    pub shutdown: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fire {
    None,
    /// Resume the queue now; `shutdown` asks for the PC to be shut down after.
    Trigger {
        shutdown: bool,
    },
}

#[derive(Debug, Default)]
pub struct Scheduler {
    /// "YYYY-MM-DD" of the last fire, which blocks a second fire the same day.
    last_fired_date: String,
}

impl Scheduler {
    /// A scheduler that already fired on `last_fired_date` ("YYYY-MM-DD", or
    /// empty for never). D11: the caller saves `last_fired_date()` after each
    /// trigger and restores it at startup, so a restart cannot fire twice in a day.
    pub fn restored(last_fired_date: &str) -> Self {
        Self { last_fired_date: last_fired_date.to_owned() }
    }

    /// The day of the last trigger, "YYYY-MM-DD", or empty if there was none.
    pub fn last_fired_date(&self) -> &str {
        &self.last_fired_date
    }

    /// One check, for the minute timer and for startup alike. A window, not an
    /// exact minute: a poll delayed past the target still catches the run, and
    /// the once-a-day gate stops it repeating.
    /// ponytail: the window does not wrap past midnight, so a target of 23:30
    /// is only caught until 23:59. The default (03:00) is well clear of it.
    pub fn poll(&mut self, cfg: &ScheduleConfig, is_premium: bool, now: &LocalTime) -> Fire {
        let Some(target) = target_minutes(cfg, is_premium, now) else { return Fire::None };
        let now_min = f64::from(now.hour * 60 + now.minute);
        if !(now_min >= target && now_min < target + MISSED_WINDOW_MIN) {
            return Fire::None;
        }
        let today = format!("{}-{:02}-{:02}", now.year, now.month, now.day);
        if self.last_fired_date == today {
            return Fire::None;
        }
        self.last_fired_date = today;
        Fire::Trigger { shutdown: cfg.shutdown }
    }
}

fn is_scheduled_day(cfg: &ScheduleConfig, is_premium: bool, now: &LocalTime) -> bool {
    // Scheduling is premium unless the free tier is granted it.
    cfg.enabled && (BASIC_LIMITS.scheduler || is_premium) && (cfg.days.is_empty() || cfg.days.contains(&f64::from(now.weekday)))
}

/// The schedule time as minutes since midnight, or `None` when scheduling is
/// off, today is not a chosen day, or the time cannot be read as two numbers.
fn target_minutes(cfg: &ScheduleConfig, is_premium: bool, now: &LocalTime) -> Option<f64> {
    if !is_scheduled_day(cfg, is_premium, now) {
        return None;
    }
    let mut parts = cfg.time.split(':').map(js_number);
    let (h, m) = (parts.next()?, parts.next()?);
    (h.is_finite() && m.is_finite()).then_some(h * 60.0 + m)
}
