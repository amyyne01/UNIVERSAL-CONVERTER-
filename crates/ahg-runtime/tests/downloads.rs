//! The process layer, run against a scripted stand-in for yt-dlp
//! (src/bin/fake_ytdlp.rs). Nothing here touches the network.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use ahg_engine::queue::{TaskInit, create_task};
use ahg_engine::types::{DownloadStatus, DownloadTask};
use ahg_runtime::download::{Downloader, Report};
use ahg_runtime::ytdlp::{Tools, YtDlp};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_ytdlp");
const WAIT: Duration = Duration::from_secs(20);

/// A folder for one test's script and output files.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ahg-rt-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn task(id: &str) -> DownloadTask {
    let init = TaskInit { url: Some("https://youtu.be/dQw4w9WgXcQ".into()), output_dir: Some("C:\\Music".into()), ..Default::default() };
    create_task(&init, id.into(), 0)
}

/// A downloader whose yt-dlp is the stand-in playing `script`.
fn downloader(dir: &Path, script: &str) -> Downloader {
    let file = dir.join("script.txt");
    fs::write(&file, script).unwrap();
    let mut d = Downloader::new(PathBuf::from(FAKE), None);
    d.env("FAKE_YTDLP_SCRIPT", file.to_str().unwrap());
    d.env("FAKE_YTDLP_ARGS", dir.join("args.txt").to_str().unwrap());
    d
}

/// Every report up to and including the one that ends the download.
fn until_end(rx: &Receiver<Report>) -> Vec<Report> {
    let mut seen = Vec::new();
    let deadline = Instant::now() + WAIT;
    while let Ok(report) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        let ended = matches!(report, Report::Done { .. } | Report::Error { .. });
        seen.push(report);
        if ended {
            return seen;
        }
    }
    panic!("the download never ended; saw {seen:?}");
}

fn pid_is_running(pid: &str) -> bool {
    let out = Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).contains(pid)
}

#[test]
fn a_download_reports_progress_then_the_file_it_wrote() {
    let dir = scratch("happy");
    let script = "err [PROGRESS]1048576|4194304|NA|524288|6\nerr [ExtractAudio] Destination: a.mp3\nout [FILEPATH]C:\\Music\\a.mp3\nexit 0\n";
    let d = downloader(&dir, script);
    let (tx, rx) = channel();
    d.start(task("t1"), 7, tx);
    let reports = until_end(&rx);

    let progress: Vec<_> = reports
        .iter()
        .filter_map(|r| match r {
            Report::Progress { id, attempt, progress } => {
                assert_eq!((id.as_str(), *attempt), ("t1", 7));
                Some(progress.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(progress[0].percent, 25.0);
    assert_eq!(progress[0].speed, 524288.0);
    assert!(progress.iter().any(|p| p.status == DownloadStatus::Converting));
    assert_eq!(progress.last().unwrap().status, DownloadStatus::Done);
    match reports.last().unwrap() {
        Report::Done { id, attempt, path, filepaths, already_on_disk } => {
            assert_eq!((id.as_str(), *attempt, path.as_str()), ("t1", 7, "C:\\Music\\a.mp3"));
            assert_eq!(filepaths, &["C:\\Music\\a.mp3".to_owned()]);
            assert!(!already_on_disk);
        }
        other => panic!("expected Done, got {other:?}"),
    }
    // The link is the last argument, after the separator that stops it being read as an option.
    let args = fs::read_to_string(dir.join("args.txt")).unwrap();
    assert!(args.ends_with("--\nhttps://youtu.be/dQw4w9WgXcQ"), "{args}");
    assert_eq!(d.running(), 0);
}

#[test]
fn a_failed_download_reports_yt_dlps_own_error_lines() {
    let dir = scratch("fail");
    let d = downloader(&dir, "err WARNING: something minor\nerr ERROR: [youtube] abc: Video unavailable\nexit 1\n");
    let (tx, rx) = channel();
    d.start(task("t2"), 1, tx);
    match until_end(&rx).last().unwrap() {
        Report::Error { message, .. } => assert_eq!(message, "ERROR: [youtube] abc: Video unavailable"),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn an_exit_without_any_error_line_names_the_exit_code() {
    let dir = scratch("code");
    let d = downloader(&dir, "exit 3\n");
    let (tx, rx) = channel();
    d.start(task("t3"), 1, tx);
    match until_end(&rx).last().unwrap() {
        Report::Error { message, .. } => assert_eq!(message, "yt-dlp exited with code 3"),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn a_character_split_across_two_chunks_arrives_whole() {
    let dir = scratch("split");
    // "[FILEPATH]C:\Música\ñ.mp3\n" with the two-byte characters cut in half between writes.
    let line = "[FILEPATH]C:\\M\u{fa}sica\\\u{f1}.mp3\n".as_bytes().to_vec();
    let cut1 = line.iter().position(|b| *b == 0xC3).unwrap() + 1;
    let cut2 = line.iter().rposition(|b| *b == 0xC3).unwrap() + 1;
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let script = format!(
        "outhex {}\nsleep 150\nouthex {}\nsleep 150\nouthex {}\nexit 0\n",
        hex(&line[..cut1]),
        hex(&line[cut1..cut2]),
        hex(&line[cut2..])
    );
    let d = downloader(&dir, &script);
    let (tx, rx) = channel();
    d.start(task("t4"), 1, tx);
    match until_end(&rx).last().unwrap() {
        Report::Done { path, .. } => assert_eq!(path, "C:\\M\u{fa}sica\\\u{f1}.mp3"),
        other => panic!("expected Done, got {other:?}"),
    }
}

#[test]
fn cancelling_kills_the_whole_process_tree_and_reports_nothing_more() {
    let dir = scratch("cancel");
    let pid_file = dir.join("child.pid");
    let script = format!("child {}\nerr [PROGRESS]1|100|NA|1|9\nsleep 60000\nexit 0\n", pid_file.display());
    let d = downloader(&dir, &script);
    let (tx, rx) = channel();
    d.start(task("t5"), 1, tx);
    // Wait for the first progress line, so the process and its child are up.
    assert!(matches!(rx.recv_timeout(WAIT).unwrap(), Report::Progress { .. }));
    let child_pid = fs::read_to_string(&pid_file).unwrap();
    assert!(pid_is_running(&child_pid), "the child should be alive before the cancel");

    d.cancel("t5");
    let deadline = Instant::now() + WAIT;
    while d.running() > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(d.running(), 0, "the download thread should have ended");
    assert!(!pid_is_running(&child_pid), "the child process must be gone too");
    // A killed process reports nothing: no error row after the user cancelled.
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
}

#[test]
fn cancel_all_stops_every_download() {
    let dir = scratch("cancelall");
    let d = downloader(&dir, "err [PROGRESS]1|100|NA|1|9\nsleep 60000\n");
    let (tx, rx) = channel();
    d.start(task("a"), 1, tx.clone());
    d.start(task("b"), 2, tx);
    for _ in 0..2 {
        assert!(matches!(rx.recv_timeout(WAIT).unwrap(), Report::Progress { .. }));
    }
    assert_eq!(d.running(), 2);
    d.cancel_all();
    let deadline = Instant::now() + WAIT;
    while d.running() > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(d.running(), 0);
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
}

#[test]
fn a_binary_that_cannot_be_started_is_reported_as_an_error() {
    let d = Downloader::new(PathBuf::from("C:\\definitely\\not\\here\\yt-dlp.exe"), None);
    let (tx, rx) = channel();
    d.start(task("t6"), 1, tx);
    assert!(matches!(until_end(&rx).last().unwrap(), Report::Error { .. }));
    assert_eq!(d.running(), 0);
}

#[test]
fn a_missing_yt_dlp_binary_says_yt_dlp_not_found() {
    let dir = scratch("notfound");
    // A strict search over an empty folder finds no yt-dlp, as in a shipped build.
    let d = Downloader::with(Arc::new(YtDlp::from_tools(Arc::new(Tools::strict(vec![dir.clone()])))));
    let (tx, rx) = channel();
    d.start(task("t7"), 1, tx);
    match until_end(&rx).last().unwrap() {
        Report::Error { message, .. } => assert!(message.contains("yt-dlp not found"), "{message}"),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn a_stand_in_that_keeps_printing_after_the_kill_reports_nothing_more() {
    let dir = scratch("chatty");
    let script = format!("err [PROGRESS]1|100|NA|1|9\n{}", "err [PROGRESS]2|100|NA|1|9\nsleep 20\n".repeat(3000));
    let d = downloader(&dir, &script);
    let (tx, rx) = channel();
    d.start(task("t8"), 1, tx);
    assert!(matches!(rx.recv_timeout(WAIT).unwrap(), Report::Progress { .. }));

    d.cancel("t8");
    let deadline = Instant::now() + WAIT;
    while d.running() > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(d.running(), 0);
    // What was already sent before the download thread ended may still be queued; after that, silence.
    while rx.try_recv().is_ok() {}
    assert!(rx.recv_timeout(Duration::from_millis(500)).is_err(), "no report may follow the kill");
}
