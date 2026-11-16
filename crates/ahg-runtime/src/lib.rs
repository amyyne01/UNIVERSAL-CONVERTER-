//! The part of AHG Universal Converter that touches the outside world: it runs
//! yt-dlp and ffmpeg, and carries out what the engine decides. The engine
//! (`ahg-engine`) holds the logic and has no threads or processes of its own.

pub mod app_update;
pub mod core;
pub mod desktop;
pub mod download;
pub mod engine;
pub mod engine_update;
pub mod http;
#[cfg(feature = "license")]
pub mod machine;
pub mod proc;
pub mod sealed;
#[cfg(feature = "standin")]
pub mod standin;
pub mod unpack;
pub mod ytdlp;

use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Lock a mutex, surviving a thread that panicked while holding it.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
