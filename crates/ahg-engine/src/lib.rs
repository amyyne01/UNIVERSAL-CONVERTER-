//! The UI-free engine of AHG Universal Converter. Nothing here may depend on Rune.

pub mod api;
pub mod atomic_write;
pub mod clipboard_watch;
pub mod clock;
pub mod config;
pub mod downloader;
#[cfg(feature = "license")]
pub mod license;
pub mod log;
pub mod presence;
pub mod progress_follow;
pub mod protocol;
pub mod queue;
pub mod scheduler;
pub mod shell_model;
pub mod spotify;
pub mod state_machine;
pub mod stats;
pub mod text;
pub mod tier;
pub mod types;
pub mod updater;
pub mod url_detector;
pub mod validate;
pub mod ytdlp_updater;
