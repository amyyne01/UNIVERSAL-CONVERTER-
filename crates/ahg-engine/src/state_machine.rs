//! Download job state machine. Every legal transition is written down here and
//! checked before a job's status changes.
use crate::types::DownloadStatus::{self, *};

/// Legal next states. `Done` and `Cancelled` are terminal.
pub fn transitions(from: DownloadStatus) -> &'static [DownloadStatus] {
    match from {
        Queued => &[FetchingInfo, Paused, Failed, Cancelled],
        FetchingInfo => &[Downloading, Paused, Retrying, Failed, Cancelled],
        Downloading => &[Converting, Paused, Retrying, Failed, Cancelled],
        Converting => &[Downloading, Embedding, Paused, Retrying, Failed, Cancelled],
        Embedding => &[Downloading, Done, Paused, Retrying, Failed, Cancelled],
        Paused => &[Downloading, Queued, Cancelled],
        Retrying => &[Queued, Downloading, Cancelled],
        Failed => &[Retrying, Queued, Cancelled],
        Done | Cancelled => &[],
    }
}

pub fn can_transition(from: DownloadStatus, to: DownloadStatus) -> bool {
    transitions(from).contains(&to)
}
