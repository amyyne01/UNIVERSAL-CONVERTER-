//! Unpacking the bundled tools into the app's data folder: once per bundle,
//! never half-done, and never into the folder the yt-dlp self-updater owns.
mod common;

use std::fs;

use ahg_runtime::unpack::{tool_dirs, unpack_tools};
use common::scratch;

const FILES: [(&str, &[u8]); 2] = [("yt-dlp.exe", b"yt-dlp bytes"), ("ffmpeg.exe", b"ffmpeg bytes, longer")];

#[test]
fn the_first_run_writes_every_tool_and_later_runs_leave_them_alone() {
    let user_data = scratch("first");
    let dir = unpack_tools(&user_data, "2.0.0", &FILES).unwrap();
    assert_eq!(dir, user_data.join("tools").join("2.0.0"));
    assert_eq!(fs::read(dir.join("yt-dlp.exe")).unwrap(), b"yt-dlp bytes");
    assert_eq!(fs::read(dir.join("ffmpeg.exe")).unwrap(), b"ffmpeg bytes, longer");

    // A second run must not rewrite anything: mark a file and see the mark survive.
    fs::write(dir.join("marker.txt"), "still here").unwrap();
    let written = fs::metadata(dir.join("ffmpeg.exe")).unwrap().modified().unwrap();
    assert_eq!(unpack_tools(&user_data, "2.0.0", &FILES).unwrap(), dir);
    assert_eq!(fs::read_to_string(dir.join("marker.txt")).unwrap(), "still here");
    assert_eq!(fs::metadata(dir.join("ffmpeg.exe")).unwrap().modified().unwrap(), written);
}

#[test]
fn a_damaged_or_incomplete_copy_is_replaced() {
    let user_data = scratch("damaged");
    let dir = unpack_tools(&user_data, "2.0.0", &FILES).unwrap();

    fs::write(dir.join("ffmpeg.exe"), b"cut short").unwrap();
    unpack_tools(&user_data, "2.0.0", &FILES).unwrap();
    assert_eq!(fs::read(dir.join("ffmpeg.exe")).unwrap(), b"ffmpeg bytes, longer");

    fs::remove_file(dir.join("yt-dlp.exe")).unwrap();
    unpack_tools(&user_data, "2.0.0", &FILES).unwrap();
    assert_eq!(fs::read(dir.join("yt-dlp.exe")).unwrap(), b"yt-dlp bytes");
}

#[test]
fn an_interrupted_unpack_is_not_trusted() {
    let user_data = scratch("interrupted");
    // Both files are in place with the right sizes, but the unpack never finished.
    let dir = user_data.join("tools").join("2.0.0");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("yt-dlp.exe"), b"yt-dlp WRONG").unwrap();
    fs::write(dir.join("ffmpeg.exe"), b"ffmpeg WRONG, longer").unwrap();
    unpack_tools(&user_data, "2.0.0", &FILES).unwrap();
    assert_eq!(fs::read(dir.join("yt-dlp.exe")).unwrap(), b"yt-dlp bytes");
}

#[test]
fn a_new_bundle_gets_its_own_folder_and_the_old_one_is_removed() {
    let user_data = scratch("upgrade");
    let old = unpack_tools(&user_data, "2.0.0", &FILES).unwrap();
    let new = unpack_tools(&user_data, "2.0.1", &[("yt-dlp.exe", b"newer"), ("ffmpeg.exe", b"newer too")]).unwrap();
    assert_eq!(new, user_data.join("tools").join("2.0.1"));
    assert_eq!(fs::read(new.join("yt-dlp.exe")).unwrap(), b"newer");
    assert!(!old.exists(), "the previous bundle should be cleaned up");
}

#[test]
fn the_self_updated_yt_dlp_folder_is_never_touched_and_is_searched_first() {
    let user_data = scratch("selfupdate");
    // What the yt-dlp self-updater installed.
    fs::create_dir_all(user_data.join("bin")).unwrap();
    fs::write(user_data.join("bin").join("yt-dlp.exe"), b"self-updated, newer than the bundle").unwrap();

    let tools = unpack_tools(&user_data, "2.0.0", &FILES).unwrap();
    assert_eq!(fs::read(user_data.join("bin").join("yt-dlp.exe")).unwrap(), b"self-updated, newer than the bundle");

    let dirs = tool_dirs(&user_data, &tools);
    assert_eq!(dirs[0], user_data.join("bin"), "a self-updated yt-dlp wins over the bundled one");
    assert_eq!(dirs[1], tools);
}

#[test]
fn a_bundle_name_cannot_escape_the_tools_folder() {
    let user_data = scratch("escape");
    assert!(unpack_tools(&user_data, "..\\evil", &FILES).is_err());
    assert!(unpack_tools(&user_data, "2.0.0", &[("..\\evil.exe", b"x")]).is_err());
}

/// A shipped build must only ever run tools from the app's own data folder.
/// A `bin` folder beside the program or in the current folder could be planted
/// by someone else, so those are for development builds only.
#[test]
fn a_shipped_build_looks_only_in_the_apps_own_folders() {
    use ahg_runtime::unpack::tool_dirs_for;
    let user_data = scratch("search");
    let tools = user_data.join("tools").join("2.0.0");
    assert_eq!(tool_dirs_for(&user_data, &tools, false), vec![user_data.join("bin"), tools.clone()]);
    assert!(tool_dirs_for(&user_data, &tools, true).len() > 2, "a development build may also use a local bin folder");
}
