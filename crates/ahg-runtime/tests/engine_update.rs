//! The yt-dlp updater with a scripted network: what it fetches, what it
//! refuses, and what ends up on disk. Nothing here touches the network.
mod common;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use ahg_engine::updater::Event;
use ahg_runtime::engine_update::EngineUpdater;
use common::{Net, scratch, sha};
use serde_json::json;

const FEED: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
const ASSET: &str = "https://github.com/yt-dlp/yt-dlp/releases/download/2026.11.01/yt-dlp.exe";
const SUMS: &str = "https://github.com/yt-dlp/yt-dlp/releases/download/2026.11.01/SHA2-256SUMS";
const BINARY: &[u8] = b"MZ this stands in for the new yt-dlp.exe";

fn feed(tag: &str, asset: &str, sums: &str) -> Vec<u8> {
    json!({ "tag_name": tag, "assets": [{ "name": "yt-dlp.exe", "browser_download_url": asset }, { "name": "SHA2-256SUMS", "browser_download_url": sums }] }).to_string().into_bytes()
}

/// A release with a good binary and its published checksum.
fn good_release() -> Net {
    Net::default().with(FEED, 200, &feed("2026.11.01", ASSET, SUMS)).with(ASSET, 200, BINARY).with(
        SUMS,
        200,
        format!("{}  yt-dlp.exe\n{}  yt-dlp\n", sha(BINARY), sha(b"other")).as_bytes(),
    )
}

struct Rig {
    updater: EngineUpdater,
    bin: PathBuf,
    states: Arc<Mutex<Vec<String>>>,
    swaps: Arc<AtomicUsize>,
}

fn rig(name: &str, net: &Net, installed: &'static str) -> Rig {
    let bin = scratch(name).join("bin");
    let states = Arc::new(Mutex::new(Vec::new()));
    let swaps = Arc::new(AtomicUsize::new(0));
    let (seen, count) = (states.clone(), swaps.clone());
    let updater = EngineUpdater::new(
        bin.to_str().unwrap(),
        Box::new(net.clone()),
        Box::new(move || installed.to_owned()),
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }),
        Box::new(move |event: Event| {
            assert_eq!(event.channel, "ytdlp:status");
            seen.lock().unwrap().push(event.payload["state"].as_str().unwrap().to_owned());
        }),
    );
    Rig { updater, bin, states, swaps }
}

impl Rig {
    fn states(&self) -> Vec<String> {
        self.states.lock().unwrap().clone()
    }

    fn nothing_installed(&self) -> bool {
        !self.bin.join("yt-dlp.exe").exists() && !self.bin.join("yt-dlp.exe.tmp").exists() && self.swaps.load(Ordering::SeqCst) == 0
    }
}

#[test]
fn a_check_says_whether_a_newer_yt_dlp_exists_and_asks_only_once_a_day() {
    let net = good_release();
    let r = rig("check", &net, "2026.06.30");
    let status = r.updater.check(false);
    assert_eq!((status.state, status.current.as_str(), status.latest.as_str()), ("available", "2026.06.30", "2026.11.01"));
    assert_eq!(r.states(), ["checking", "available"]);
    // Within a day the answer is reused; a forced check asks again.
    r.updater.check(false);
    assert_eq!(net.asked().len(), 1);
    r.updater.check(true);
    assert_eq!(net.asked(), [FEED, FEED]);
    assert!(r.nothing_installed(), "a check installs nothing");

    let same = rig("uptodate", &net, "2026.11.01");
    assert_eq!(same.updater.check(false).state, "uptodate");
}

#[test]
fn an_update_installs_the_binary_whose_checksum_the_release_published() {
    let net = good_release();
    let r = rig("update", &net, "2026.06.30");
    let status = r.updater.update();
    assert_eq!((status.state, status.latest.as_str()), ("updated", "2026.11.01"), "{:?}", status.message);
    assert_eq!(fs::read(r.bin.join("yt-dlp.exe")).unwrap(), BINARY);
    assert!(!r.bin.join("yt-dlp.exe.tmp").exists());
    assert_eq!(r.swaps.load(Ordering::SeqCst), 1, "the app is told to look for yt-dlp again");
    assert_eq!(r.states(), ["downloading", "updated"]);
    assert_eq!(net.asked(), [FEED, ASSET, SUMS]);
}

#[test]
fn a_binary_that_does_not_match_the_published_checksum_is_never_installed() {
    let tampered = good_release().with(ASSET, 200, b"MZ something else entirely");
    let r = rig("mismatch", &tampered, "2026.06.30");
    let status = r.updater.update();
    assert_eq!((status.state, status.message.as_deref()), ("error", Some("Engine update refused: checksum mismatch")));
    assert!(r.nothing_installed());

    // A release that publishes no checksum for it is refused as well.
    let unlisted = good_release().with(SUMS, 200, format!("{}  yt-dlp_linux\n", sha(BINARY)).as_bytes());
    let r = rig("unlisted", &unlisted, "2026.06.30");
    assert_eq!(r.updater.update().message.as_deref(), Some("Engine update refused: no published checksum"));
    assert!(r.nothing_installed());
}

#[test]
fn nothing_is_fetched_from_or_accepted_through_a_host_that_is_not_githubs() {
    // The feed points the binary somewhere else: refused before that host is ever asked.
    let evil = "https://evil.example/yt-dlp.exe";
    let net = Net::default().with(FEED, 200, &feed("2026.11.01", evil, SUMS)).with(evil, 200, BINARY).with(
        SUMS,
        200,
        format!("{}  yt-dlp.exe\n", sha(BINARY)).as_bytes(),
    );
    let r = rig("evilfeed", &net, "2026.06.30");
    let status = r.updater.update();
    assert_eq!(status.message.as_deref(), Some("Engine update refused: untrusted host \"https://evil.example/yt-dlp.exe\""));
    assert_eq!(net.asked(), [FEED]);
    assert!(r.nothing_installed());

    // A trusted address that answers from somewhere else is refused too, checksum or not.
    let net = good_release().redirected(ASSET, "https://evil.example/yt-dlp.exe", BINARY);
    let r = rig("redirect", &net, "2026.06.30");
    assert_eq!(
        r.updater.update().message.as_deref(),
        Some("Engine update refused: redirected to untrusted host \"https://evil.example/yt-dlp.exe\"")
    );
    assert!(r.nothing_installed());
}

#[test]
fn a_failed_request_is_reported_and_leaves_things_as_they_were() {
    let down = Net::default().with(FEED, 503, b"");
    let r = rig("http503", &down, "2026.06.30");
    assert_eq!(r.updater.check(false).message.as_deref(), Some("Engine update failed: HTTP 503"));
    assert_eq!(r.updater.update().message.as_deref(), Some("Engine update failed: HTTP 503"));

    let offline = Net::default();
    let r = rig("offline", &offline, "2026.06.30");
    assert_eq!(r.updater.check(false).message.as_deref(), Some("fetch failed"));
    assert!(r.nothing_installed());

    let garbage = Net::default().with(FEED, 200, b"<html>not json</html>");
    let r = rig("garbage", &garbage, "2026.06.30");
    assert_eq!(r.updater.check(false).state, "error");

    // The binary's download fails half way through the update: nothing is written.
    let half = good_release().with(ASSET, 500, b"");
    let r = rig("half", &half, "2026.06.30");
    assert_eq!(r.updater.update().state, "error");
    assert!(r.nothing_installed());
    // After a failure the updater is free again.
    assert_eq!(rig("again", &good_release(), "2026.06.30").updater.update().state, "updated");
}

#[test]
fn a_reply_far_larger_than_a_release_file_is_refused() {
    let huge = good_release().with(SUMS, 200, &vec![b'a'; 6 * 1024 * 1024]);
    let r = rig("huge", &huge, "2026.06.30");
    assert_eq!(r.updater.update().message.as_deref(), Some("Engine update refused: the reply is too large"));
    assert!(r.nothing_installed());
}

#[test]
fn an_outdated_yt_dlp_error_asks_for_an_update_once_and_only_when_allowed() {
    let r = rig("reactive", &good_release(), "2026.06.30");
    let stale = "ERROR: [youtube] abc: Unable to extract player response; please report this issue on https://github.com/yt-dlp/yt-dlp/issues , confirm you are on the latest version using yt-dlp -U";
    assert!(!r.updater.on_download_failure("ERROR: Video unavailable", true), "an ordinary failure is not a reason");
    assert!(!r.updater.on_download_failure(stale, false), "automatic engine updates are off");
    assert!(r.updater.on_download_failure(stale, true));
    assert!(!r.updater.on_download_failure(stale, true), "once per session");
}

/// The real thing: GitHub is asked for the latest yt-dlp, the binary is
/// downloaded (about 18 MB), verified against the published checksum,
/// installed into a scratch folder, and run. Network; run with --ignored.
#[test]
#[ignore = "downloads the real yt-dlp"]
fn the_real_latest_yt_dlp_is_fetched_verified_installed_and_runs() {
    let bin = scratch("real").join("bin");
    let updater = EngineUpdater::new(
        bin.to_str().unwrap(),
        Box::new(ahg_runtime::http::Releases),
        Box::new(String::new),
        Box::new(|| {}),
        Box::new(|_| {}),
    );
    let seen = updater.check(true);
    println!("check: {} latest {} {:?}", seen.state, seen.latest, seen.message);
    assert_eq!(seen.state, "available", "{:?}", seen.message);
    let began = std::time::Instant::now();
    let done = updater.update();
    println!("update: {} in {:.1} s {:?}", done.state, began.elapsed().as_secs_f64(), done.message);
    assert_eq!(done.state, "updated", "{:?}", done.message);
    let installed = bin.join("yt-dlp.exe");
    assert!(fs::metadata(&installed).unwrap().len() > 5_000_000);
    let version = std::process::Command::new(&installed).arg("--version").output().unwrap();
    let version = String::from_utf8_lossy(&version.stdout).trim().to_owned();
    println!("installed yt-dlp reports {version}");
    assert_eq!(version, done.latest, "the installed binary is the release it claims to be");
}
