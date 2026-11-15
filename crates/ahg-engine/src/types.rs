//! Domain types shared by the whole engine (the Rust home of shared/types.ts).
use serde::{Deserialize, Serialize};

/// A download job's state. The serialised names are the ones Electron writes
/// to `downloads-history.json`, so both apps can read the same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Queued,
    FetchingInfo,
    Downloading,
    Converting,
    Embedding,
    Paused,
    Retrying,
    Failed,
    Done,
    Cancelled,
}
