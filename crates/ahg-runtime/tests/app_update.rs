//! The app's own updater with a scripted network, and the swap of the EXE in
//! a scratch folder. Nothing here touches the network or the real app.
mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ahg_engine::updater::Event;
use ahg_runtime::app_update::{AppUpdater, remove_spare, swap_in};
use common::{Net, sha};
use serde_json::json;

const FEED: &str = "https://api.github.com/repos/amyyne01/UNIVERSAL-CONVERTER-/releases/latest";
const EXE: &str = "https://github.com/amyyne01/UNIVERSAL-CONVERTER-/releases/download/v2.1.0/AHG-Converter.exe";
const SUMS: &str = "https://github.com/amyyne01/UNIVERSAL-CONVERTER-/releases/download/v2.1.0/SHA256SUMS.txt";

/// A folder whose name holds what breaks a pasted command line.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ahg-appup-{}-{name} 100% & (été) ^caret!", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 300 KB, so the download arrives in several chunks.
fn new_exe() -> Vec<u8> {
    (0..300_000u32).map(|n| (n % 251) as u8).collect()
}

fn feed(tag: &str, name: &str, exe: &str) -> Vec<u8> {
    json!({ "tag_name": tag, "assets": [{ "name": name, "browser_download_url": exe }, { "name": "SHA256SUMS.txt", "browser_download_url": SUMS }] }).to_string().into_bytes()
}

fn good_release() -> Net {
    Net::default().with(FEED, 200, &feed("v2.1.0", "AHG-Converter.exe", EXE)).with(EXE, 200, &new_exe()).with(
        SUMS,
        200,
        format!("{}  AHG-Converter.exe\n", sha(&new_exe())).as_bytes(),
    )
}

struct Rig {
    updater: AppUpdater,
    updates: PathBuf,
    events: Arc<Mutex<Vec<Event>>>,
}

fn rig(name: &str, net: &Net, version: &str) -> Rig {
    let updates = scratch(name).join("updates");
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = events.clone();
    let updater = AppUpdater::new(&updates, version, Box::new(net.clone()), Box::new(move |event| seen.lock().unwrap().push(event)));
    Rig { updater, updates, events }
}

impl Rig {
    fn channels(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().iter().map(|e| e.channel).collect()
    }

    fn error(&self) -> Option<String> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|e| e.channel == "update:error")
            .map(|e| e.payload["message"].as_str().unwrap().to_owned())
    }

    fn nothing_staged(&self) -> bool {
        !self.updates.exists() || fs::read_dir(&self.updates).unwrap().next().is_none()
    }
}

#[test]
fn two_downloads_at_once_both_finish() {
    let r = rig("overlap", &good_release(), "2.0.0");
    r.updater.check();
    let updater = &r.updater;
    std::thread::scope(|s| {
        let both = [s.spawn(|| updater.download()), s.spawn(|| updater.download())];
        for h in both {
            assert_eq!(h.join().unwrap().unwrap().phase, "ready");
        }
    });
}

#[test]
fn a_check_reports_a_newer_version_or_that_there_is_none() {
    let r = rig("check", &good_release(), "2.0.0");
    let state = r.updater.check();
    assert_eq!((state.phase, state.version.as_deref()), ("available", Some("2.1.0")));
    assert_eq!(r.channels(), ["update:available"]);

    let same = rig("same", &good_release(), "2.1.0");
    assert_eq!(same.updater.check().phase, "uptodate");
    assert_eq!(same.channels(), ["update:not-available"]);
    assert_eq!(same.updater.download().unwrap_err(), "Update refused: no available update to download");

    let down = rig("down", &Net::default().with(FEED, 500, b""), "2.0.0");
    assert_eq!(down.updater.check().phase, "error");
    assert_eq!(down.error().as_deref(), Some("Update failed: HTTP 500"));
}

#[test]
fn a_download_is_staged_only_when_it_matches_the_published_checksum() {
    let net = good_release();
    let r = rig("download", &net, "2.0.0");
    r.updater.check();
    let state = r.updater.download().unwrap();
    assert_eq!((state.phase, state.version.as_deref()), ("ready", Some("2.1.0")), "{:?}", r.error());
    assert_eq!(fs::read(r.updates.join("AHG-Converter.exe")).unwrap(), new_exe());
    assert!(!r.updates.join("download.part").exists());
    assert_eq!(net.asked(), [FEED, SUMS, EXE], "the checksum list is read before the EXE");
    // Progress climbs to 100 and the download is announced last.
    let events = r.events.lock().unwrap();
    let percents: Vec<u64> =
        events.iter().filter(|e| e.channel == "update:progress").map(|e| e.payload["percent"].as_u64().unwrap()).collect();
    assert!(percents.len() >= 3 && percents.windows(2).all(|w| w[0] < w[1]) && percents.last() == Some(&100), "{percents:?}");
    assert_eq!(events.last().unwrap().channel, "update:downloaded");
}

#[test]
fn a_tampered_or_unlisted_download_is_refused_and_leaves_nothing_behind() {
    let tampered = good_release().with(EXE, 200, b"MZ not what was published");
    let r = rig("tampered", &tampered, "2.0.0");
    r.updater.check();
    assert_eq!(r.updater.download().unwrap().phase, "error");
    assert_eq!(r.error().as_deref(), Some("Update refused: the download did not match its published checksum"));
    assert!(r.nothing_staged());
    assert_eq!(
        r.updater.install(Path::new("C:\\nowhere\\AHG-Converter.exe")).unwrap_err(),
        "Update refused: nothing downloaded to install"
    );

    let unlisted = good_release().with(SUMS, 200, format!("{}  Some-Other.exe\n", sha(&new_exe())).as_bytes());
    let r = rig("unlisted", &unlisted, "2.0.0");
    r.updater.check();
    r.updater.download().unwrap();
    assert_eq!(r.error().as_deref(), Some("Update refused: no checksum published for this release"));
    assert!(r.nothing_staged());
}

#[test]
fn nothing_comes_from_a_host_that_is_not_githubs_and_no_asset_name_can_leave_the_folder() {
    let evil = "https://evil.example/AHG-Converter.exe";
    let r = rig("evil", &Net::default().with(FEED, 200, &feed("v2.1.0", "AHG-Converter.exe", evil)), "2.0.0");
    assert_eq!(r.updater.check().phase, "error");
    assert_eq!(r.error().as_deref(), Some("Update refused: untrusted host \"https://evil.example/AHG-Converter.exe\""));

    let redirected = good_release().redirected(EXE, evil, &new_exe());
    let r = rig("redirected", &redirected, "2.0.0");
    r.updater.check();
    r.updater.download().unwrap();
    assert_eq!(r.error().as_deref(), Some("Update refused: redirected to untrusted host \"https://evil.example/AHG-Converter.exe\""));
    assert!(r.nothing_staged());

    // D32: an asset named as a path is refused when the release is read.
    let r = rig("path", &Net::default().with(FEED, 200, &feed("v2.1.0", "..\\..\\evil.exe", EXE)), "2.0.0");
    assert_eq!(r.updater.check().phase, "error");
}

// ── The swap (D33): no script, so no folder name can break it ───────────────

#[test]
fn the_new_exe_takes_the_place_of_the_old_one_in_any_folder() {
    let dir = scratch("swap");
    let (target, staged) = (dir.join("AHG-Converter.exe"), dir.join("updates").join("AHG-Converter.exe"));
    fs::create_dir_all(staged.parent().unwrap()).unwrap();
    fs::write(&target, b"old version").unwrap();
    fs::write(&staged, b"new version").unwrap();

    swap_in(&staged, &target).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"new version");
    assert_eq!(fs::read(dir.join("AHG-Converter.exe.old")).unwrap(), b"old version", "the old one waits beside it");
    assert!(!staged.exists());

    // The new version clears the spare when it starts; with none there, nothing happens.
    remove_spare(&target);
    remove_spare(&target);
    assert!(!dir.join("AHG-Converter.exe.old").exists());
    assert_eq!(fs::read(&target).unwrap(), b"new version");
}

#[test]
fn a_swap_that_cannot_finish_puts_the_old_exe_back() {
    let dir = scratch("swapfail");
    let target = dir.join("AHG-Converter.exe");
    fs::write(&target, b"old version").unwrap();
    // The download is gone: nothing is touched.
    assert!(swap_in(&dir.join("updates").join("missing.exe"), &target).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"old version");
    assert!(!dir.join("AHG-Converter.exe.old").exists());
    // The "download" is a folder, which cannot be moved over a file or copied as one.
    let folder = dir.join("updates").join("AHG-Converter.exe");
    fs::create_dir_all(folder.join("inside")).unwrap();
    assert!(swap_in(&folder, &target).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"old version", "the app is still there");
}

#[test]
fn a_program_that_is_running_can_be_replaced() {
    let dir = scratch("swaprunning");
    let (target, staged) = (dir.join("AHG-Converter.exe"), dir.join("updates").join("AHG-Converter.exe"));
    fs::create_dir_all(staged.parent().unwrap()).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_fake_ytdlp"), &target).unwrap();
    fs::write(&staged, b"new version").unwrap();
    fs::write(dir.join("script.txt"), "sleep 2500\nexit 0\n").unwrap();
    let mut running = std::process::Command::new(&target).env("FAKE_YTDLP_SCRIPT", dir.join("script.txt")).spawn().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    swap_in(&staged, &target).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"new version");
    assert!(running.try_wait().unwrap().is_none(), "the old version is still running, from its spare name");
    // While it runs the spare cannot be removed; once it has exited it can.
    remove_spare(&target);
    assert!(dir.join("AHG-Converter.exe.old").exists());
    running.wait().unwrap();
    remove_spare(&target);
    assert!(!dir.join("AHG-Converter.exe.old").exists());
}

#[test]
fn installing_a_staged_download_swaps_it_in() {
    let net = good_release();
    let r = rig("install", &net, "2.0.0");
    let target = r.updates.parent().unwrap().join("AHG-Converter.exe");
    fs::write(&target, b"old version").unwrap();
    r.updater.check();
    r.updater.download().unwrap();
    r.updater.install(&target).unwrap();
    assert_eq!(fs::read(&target).unwrap(), new_exe());
    // What an earlier run staged is dropped at the next start.
    fs::create_dir_all(&r.updates).unwrap();
    fs::write(r.updates.join("leftover.exe"), b"x").unwrap();
    r.updater.cleanup();
    assert!(!r.updates.exists());
}

/// The real release page. As an old version the app is offered the latest
/// release; as 2.0.0 it is told it is current. Then the latest EXE is really
/// downloaded (about 120 MB) and must match its published checksum.
/// Network; run with --ignored.
#[test]
#[ignore = "downloads the real release EXE, about 120 MB"]
fn the_real_release_is_found_downloaded_and_verified() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let real = |name: &str, version: &str| {
        let seen = events.clone();
        let updates = scratch(name).join("updates");
        (
            AppUpdater::new(
                &updates,
                version,
                Box::new(ahg_runtime::http::Releases),
                Box::new(move |e: Event| seen.lock().unwrap().push(e)),
            ),
            updates,
        )
    };
    let (current, _) = real("realcurrent", "2.0.0");
    let state = current.check();
    println!("as 2.0.0: {} {:?}", state.phase, state.message);
    assert_eq!(state.phase, "uptodate", "{:?}", state.message);

    let (old, updates) = real("realold", "1.0.0");
    let state = old.check();
    println!("as 1.0.0: {} {:?}", state.phase, state.version);
    assert_eq!(state.phase, "available");
    let began = std::time::Instant::now();
    let state = old.download().unwrap();
    let file = updates.join("AHG-Converter.exe");
    println!(
        "download: {} in {:.0} s, {} bytes {:?}",
        state.phase,
        began.elapsed().as_secs_f64(),
        fs::metadata(&file).map(|m| m.len()).unwrap_or(0),
        state.message
    );
    assert_eq!(state.phase, "ready", "{:?}", state.message);
    assert!(fs::metadata(&file).unwrap().len() > 100_000_000);
    let _ = fs::remove_dir_all(updates.parent().unwrap());
}
