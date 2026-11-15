//! The value a progress bar shows. A download reports its progress in steps;
//! a bar set straight from those reports jumps. A `Follower` glides toward the
//! latest report instead: the interface gives it each report and asks it for
//! the value to draw on every frame.
//!
//! Like the rest of the engine it keeps no timer: the clock is passed in.

/// How quickly the bar closes the gap, in milliseconds: after this long about
/// two thirds of the distance is covered, after five times this it has arrived.
const EASE_MS: f64 = 140.0;
/// Closer than this (in percent) counts as arrived.
const NEAR: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Follower {
    /// The download's real progress, 0 to 100.
    target: f64,
    /// What the bar shows, 0 to 100.
    shown: f64,
    /// When `shown` was last moved, in milliseconds.
    at: u64,
}

impl Default for Follower {
    fn default() -> Self {
        Self::new()
    }
}

impl Follower {
    /// An empty bar.
    pub fn new() -> Self {
        Self { target: 0.0, shown: 0.0, at: 0 }
    }

    /// The download reported `percent` (0 to 100) at `now` milliseconds.
    /// Progress that falls back (a retry, a resume) takes the bar back at once:
    /// a bar sliding backwards reads as something going wrong.
    pub fn set_target(&mut self, percent: f64, now: u64) {
        // Bring the bar up to this moment first, so the new report is followed from where it is.
        self.value(now);
        self.target = if percent.is_nan() { self.target } else { percent.clamp(0.0, 100.0) };
        if self.target < self.shown {
            self.shown = self.target;
        }
    }

    /// The value to draw at `now` milliseconds. It never passes the real
    /// progress, and the motion is the same at any frame rate.
    pub fn value(&mut self, now: u64) -> f64 {
        let elapsed = now.saturating_sub(self.at) as f64;
        self.at = self.at.max(now);
        let gap = self.target - self.shown;
        if gap > 0.0 && elapsed > 0.0 {
            // The remaining gap shrinks by the same share in equal times, however the time is cut into frames.
            let left = gap * (-elapsed / EASE_MS).exp();
            self.shown = if left < NEAR { self.target } else { self.target - left };
        }
        self.shown
    }

    /// True when the bar shows the real progress: the interface can stop
    /// redrawing until the next report.
    pub fn is_settled(&self) -> bool {
        self.shown == self.target
    }
}
