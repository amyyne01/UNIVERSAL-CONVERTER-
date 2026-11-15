//! The part of AHG Universal Converter that touches the outside world: it runs
//! yt-dlp and ffmpeg, and carries out what the engine decides. The engine
//! (`ahg-engine`) holds the logic and has no threads or processes of its own.

pub mod core;
pub mod download;
pub mod engine;
pub mod http;
#[cfg(feature = "license")]
pub mod machine;
pub mod proc;
pub mod ytdlp;
