//! The wall clock, in the user's local time zone. The scheduler and the monthly
//! counters both work in local time, and take a `LocalTime` so tests can set it.
use chrono::{Datelike, Local, SecondsFormat, Timelike, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    /// 1 to 12.
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    /// 0 is Sunday, as JavaScript's `getDay()`.
    pub weekday: u32,
}

impl LocalTime {
    pub fn now() -> Self {
        let now = Local::now();
        Self {
            year: now.year(),
            month: now.month(),
            day: now.day(),
            hour: now.hour(),
            minute: now.minute(),
            weekday: now.weekday().num_days_from_sunday(),
        }
    }
}

/// The current instant as `Date.prototype.toISOString` prints it.
pub fn iso_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
