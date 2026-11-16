//! Small behaviours Electron has and the engine claims, pinned here: a lost
//! statistics write is silent, saving history before it was initialised writes
//! nothing, and an atomic write that fails leaves no `.tmp` behind.
use std::fs;
use std::path::PathBuf;

use ahg_engine::atomic_write::write_file_atomic;
use ahg_engine::queue::{Queue, TaskInit, create_task};
use ahg_engine::stats::StatsStore;
use ahg_engine::types::SourcePlatform;

const T0: u64 = 1_763_200_000_000;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("ahg-test-{}-gaps-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Electron stats.ts `save`: "best-effort: a lost counter must never break a download".
#[test]
fn statistics_survive_an_unwritable_folder() {
    let scratch = Scratch::new("stats");
    let file = scratch.0.join("no-such-folder").join("download-stats.json");
    let mut stats = StatsStore::default();
    stats.init_at(&file, "2026-11");
    stats.record_at(SourcePlatform::Youtube, &[], 100.0, "2026-11");
    stats.record_at(SourcePlatform::Youtube, &[], 50.0, "2026-11");
    assert!(!file.exists() && !scratch.0.join("no-such-folder").exists());
    let got = stats.get_at("2026-11");
    assert_eq!((got.current.files, got.current.bytes), (2.0, 150.0));
}

/// Electron queue.ts `flushHistory`: `if (!historyFile) return;`.
#[test]
fn flushing_history_before_it_is_initialised_writes_nothing() {
    let scratch = Scratch::new("flush");
    let mut queue = Queue::new();
    let init = TaskInit { url: Some("https://example.com/a".into()), ..Default::default() };
    queue.enqueue(create_task(&init, "task_a".into(), T0), T0);
    queue.flush_history();
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
}

/// Electron atomic-write.ts: on failure the tmp file is removed and the error thrown.
#[test]
fn a_failed_atomic_write_leaves_no_tmp_and_no_partial_file() {
    let scratch = Scratch::new("atomic");
    let target = scratch.0.join("target");
    fs::create_dir(&target).unwrap();
    assert!(write_file_atomic(&target, "data").is_err());
    assert!(target.is_dir(), "the original is left as it was");
    let names: Vec<_> = fs::read_dir(&scratch.0).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, vec![std::ffi::OsString::from("target")], "no .tmp left behind");
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0, "no partial file inside the directory");
}
