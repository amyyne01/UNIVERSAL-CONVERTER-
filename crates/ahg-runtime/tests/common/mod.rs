//! Shared by the test files. The scripted network lives in the crate's
//! `standin` module; the helpers below were copied into several test files.
#![allow(dead_code, unused_imports)]
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use ahg_engine::queue::{TaskInit, create_task};
use ahg_engine::types::DownloadTask;
use ahg_runtime::download::Report;
use sha2::{Digest, Sha256};

pub use ahg_runtime::standin::Net;

const WAIT: Duration = Duration::from_secs(20);

pub fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A folder for one test's script and output files.
pub fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ahg-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn task(id: &str) -> DownloadTask {
    let init = TaskInit { url: Some("https://youtu.be/dQw4w9WgXcQ".into()), output_dir: Some("C:\\Music".into()), ..Default::default() };
    create_task(&init, id.into(), 0)
}

/// Waits for an event `want` accepts, returning everything seen up to it.
pub fn wait_for<T>(rx: &Receiver<T>, mut want: impl FnMut(&T) -> bool) -> Vec<T> {
    let mut seen = Vec::new();
    let deadline = Instant::now() + WAIT;
    while let Ok(event) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        let hit = want(&event);
        seen.push(event);
        if hit {
            return seen;
        }
    }
    panic!("never saw the expected event; saw {}", seen.len());
}

/// Every report up to and including the one that ends the download.
pub fn until_ended(rx: &Receiver<Report>) -> Vec<Report> {
    let mut seen = Vec::new();
    while let Ok(report) = rx.recv_timeout(WAIT) {
        let ended = matches!(report, Report::Done { .. } | Report::Error { .. });
        seen.push(report);
        if ended {
            break;
        }
    }
    seen
}

/// How often a stand-in's script reached its `count` line.
pub fn runs(file: &Path) -> usize {
    fs::read_to_string(file).map_or(0, |text| text.lines().count())
}

/// Whether a process with this id is still running (Windows `tasklist`).
pub fn pid_is_running(pid: &str) -> bool {
    let out = std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).contains(pid)
}
