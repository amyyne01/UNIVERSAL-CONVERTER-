//! The contract between the queue and the layer that runs downloads, beyond
//! what the recorded Electron scenarios exercise. In Electron a killed process
//! never reports back, and the downloader writes the Spotify match straight
//! onto the task object; here both are explicit.
use ahg_engine::queue::{Effect, Queue, TaskInit, create_task};
use ahg_engine::types::{DownloadProgress, DownloadStatus, SourcePlatform};

const T0: u64 = 1_763_200_000_000;

fn started(effects: &[Effect]) -> Vec<(String, u64)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Download { task, attempt } => Some((task.task_id.clone(), *attempt)),
            _ => None,
        })
        .collect()
}

fn one_running_task(queue: &mut Queue) -> (String, u64) {
    queue.set_max_slots(1.0, T0);
    let init = TaskInit { url: Some("ytsearch:Artist - Song".into()), source: Some(SourcePlatform::Spotify), ..Default::default() };
    let effects = queue.enqueue(create_task(&init, "task_a".into(), T0), T0);
    started(&effects).pop().expect("the task starts at once")
}

#[test]
fn a_report_from_a_superseded_attempt_is_refused_after_pause_and_resume() {
    let mut queue = Queue::new();
    let (id, first) = one_running_task(&mut queue);
    queue.pause_task(&id, T0);
    let (_, effects) = queue.resume_task(&id, T0 + 10);
    let (_, second) = started(&effects).pop().expect("the task restarts");
    assert_ne!(first, second);

    // The first process was killed. If it reports anyway, nothing may happen.
    let progress = DownloadProgress { status: DownloadStatus::Downloading, percent: 99.0, ..Default::default() };
    assert!(queue.on_progress(&id, first, progress, T0 + 20).is_empty());
    assert!(queue.on_done(&id, first, "C:\\x.mp3", &[], false, T0 + 20).is_empty());
    assert!(queue.on_error(&id, first, "ERROR: Video unavailable", T0 + 5000).is_empty());
    let task = &queue.all_tasks()[0];
    assert_eq!(task.progress.status, DownloadStatus::FetchingInfo);
    assert_eq!(task.progress.percent, 0.0);

    // The live attempt still works.
    assert!(!queue.on_done(&id, second, "", &[], false, T0 + 30).is_empty());
    assert_eq!(queue.all_tasks()[0].progress.status, DownloadStatus::Done);
}

#[test]
fn the_spotify_match_is_recorded_on_the_running_task() {
    let mut queue = Queue::new();
    let (id, attempt) = one_running_task(&mut queue);
    queue.on_match(&id, attempt, "https://www.youtube.com/watch?v=abcdefghijk", 0.82);
    let task = &queue.all_tasks()[0];
    assert_eq!(task.matched_url.as_deref(), Some("https://www.youtube.com/watch?v=abcdefghijk"));
    assert_eq!(task.match_confidence, Some(0.82));
}

#[test]
fn a_match_from_a_superseded_attempt_is_ignored() {
    let mut queue = Queue::new();
    let (id, first) = one_running_task(&mut queue);
    queue.pause_task(&id, T0);
    queue.resume_task(&id, T0 + 10);
    queue.on_match(&id, first, "https://stale.example/x", 0.5);
    assert_eq!(queue.all_tasks()[0].matched_url, None);
}
