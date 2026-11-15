//! The opt-in clipboard link watcher: when the clipboard text changes to
//! something the detector recognises, offer it. Never downloads on its own.
//! Ported from electron/clipboard-watch.ts. The timer and the clipboard read
//! belong to the app layer: it calls `tick` every `POLL_MS` while running.
use crate::text::js_trim;
use crate::types::{SourcePlatform, UrlDetection};
use crate::url_detector::detect_url;

/// How often the owner should call `tick`.
pub const POLL_MS: u64 = 1500;

#[derive(Debug, Default)]
pub struct ClipboardWatcher {
    running: bool,
    /// Last text seen. Only a change can offer, which is what stops the same
    /// link firing twice in a row. Kept across `stop`.
    last_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tick {
    /// The watcher is stopped; the timer should not be running.
    Stopped,
    /// Nothing to offer (disabled, empty, unchanged or not a link).
    Idle,
    Offer(UrlDetection),
    /// The clipboard could not be read. The app layer logs it and carries on.
    ReadFailed,
}

impl ClipboardWatcher {
    /// Starts the watcher. `read` returns the clipboard text, or `None` when it
    /// cannot be read. Returns true when the caller should start its timer
    /// (false when it was already running, and `read` is not called either).
    /// The text on the clipboard now is taken as seen, so enabling the watcher
    /// never offers a link copied before it was turned on.
    pub fn start(&mut self, read: impl FnOnce() -> Option<String>) -> bool {
        if self.running {
            return false;
        }
        if let Some(text) = read() {
            self.last_text = js_trim(&text).to_string();
        }
        self.running = true;
        true
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    /// One poll. `read` is only called when `enabled`.
    pub fn tick(&mut self, enabled: bool, read: impl FnOnce() -> Option<String>) -> Tick {
        if !self.running {
            return Tick::Stopped;
        }
        if !enabled {
            return Tick::Idle;
        }
        let Some(text) = read() else { return Tick::ReadFailed };
        let text = js_trim(&text);
        if text.is_empty() || text == self.last_text {
            return Tick::Idle;
        }
        self.last_text = text.to_string();
        let detection = detect_url(text);
        if detection.platform == SourcePlatform::Unknown { Tick::Idle } else { Tick::Offer(detection) }
    }
}
