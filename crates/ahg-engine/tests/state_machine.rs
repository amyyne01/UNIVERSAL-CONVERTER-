//! Ported from electron/state-machine.test.ts, plus the literal table: the
//! original first test compares `canTransition` with the table it reads from,
//! so nothing pinned the edges themselves.
use ahg_engine::state_machine::{can_transition, transitions};
use ahg_engine::types::DownloadStatus::{self, *};

const ALL: [DownloadStatus; 10] = [Queued, FetchingInfo, Downloading, Converting, Embedding, Paused, Retrying, Failed, Done, Cancelled];

#[test]
fn the_table_is_the_one_in_electron_state_machine_ts() {
    let expected: [(DownloadStatus, &[DownloadStatus]); 10] = [
        (Queued, &[FetchingInfo, Paused, Failed, Cancelled]),
        (FetchingInfo, &[Downloading, Paused, Retrying, Failed, Cancelled]),
        (Downloading, &[Converting, Paused, Retrying, Failed, Cancelled]),
        (Converting, &[Downloading, Embedding, Paused, Retrying, Failed, Cancelled]),
        (Embedding, &[Downloading, Done, Paused, Retrying, Failed, Cancelled]),
        (Paused, &[Downloading, Queued, Cancelled]),
        (Retrying, &[Queued, Downloading, Cancelled]),
        (Failed, &[Retrying, Queued, Cancelled]),
        (Done, &[]),
        (Cancelled, &[]),
    ];
    for (from, to) in expected {
        assert_eq!(transitions(from), to, "{from:?}");
    }
}

#[test]
fn can_transition_matches_the_transition_table_for_every_pair() {
    for from in ALL {
        for to in ALL {
            assert_eq!(can_transition(from, to), transitions(from).contains(&to), "{from:?} -> {to:?}");
        }
    }
}

#[test]
fn allows_the_full_forward_path_queued_to_done() {
    let path = [Queued, FetchingInfo, Downloading, Converting, Embedding, Done];
    for w in path.windows(2) {
        assert!(can_transition(w[0], w[1]), "{:?} -> {:?}", w[0], w[1]);
    }
}

#[test]
fn lets_a_collection_return_to_downloading_after_an_item_finishes_post_processing() {
    assert!(can_transition(Converting, Downloading));
    assert!(can_transition(Embedding, Downloading));
}

#[test]
fn treats_done_and_cancelled_as_terminal() {
    assert!(transitions(Done).is_empty());
    assert!(transitions(Cancelled).is_empty());
}

#[test]
fn never_lists_a_self_transition() {
    for s in ALL {
        assert!(!transitions(s).contains(&s), "{s:?}");
    }
}

#[test]
fn status_serialises_to_the_strings_electron_writes_to_downloads_history() {
    let names = ["queued", "fetching_info", "downloading", "converting", "embedding", "paused", "retrying", "failed", "done", "cancelled"];
    for (s, name) in ALL.iter().zip(names) {
        assert_eq!(serde_json::to_string(s).unwrap(), format!("\"{name}\""));
        assert_eq!(serde_json::from_str::<DownloadStatus>(&format!("\"{name}\"")).unwrap(), *s);
    }
}
