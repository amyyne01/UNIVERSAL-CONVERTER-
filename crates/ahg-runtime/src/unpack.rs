//! The bundled tools (yt-dlp, ffmpeg) travel inside the app's single EXE and
//! are copied out once, into the app's data folder. Each bundle gets its own
//! folder, named after it, so a new app version brings its own copy.
//!
//! They never go into `<userData>/bin`: that folder belongs to the yt-dlp
//! self-updater, and copying an older bundled yt-dlp over a newer
//! self-updated one would downgrade it.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ahg_engine::updater::is_plain_file_name;

/// Written last, so its presence means the unpack finished.
const COMPLETE: &str = ".complete";

fn refuse(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, format!("unsafe {what}"))
}

/// True when `dir` holds a finished unpack of exactly these files.
fn is_complete(dir: &Path, files: &[(&str, &[u8])]) -> bool {
    dir.join(COMPLETE).exists()
        && files.iter().all(|(name, bytes)| fs::metadata(dir.join(name)).is_ok_and(|m| m.is_file() && m.len() == bytes.len() as u64))
}

/// Make sure `<user_data>/tools/<bundle>/` holds `files`, and return that
/// folder. Nothing is written when a finished copy is already there. A copy
/// that is damaged, incomplete or was interrupted is replaced. Older bundles
/// are removed.
pub fn unpack_tools(user_data: &Path, bundle: &str, files: &[(&str, &[u8])]) -> io::Result<PathBuf> {
    if !is_plain_file_name(bundle) {
        return Err(refuse("bundle name"));
    }
    if files.iter().any(|(name, _)| !is_plain_file_name(name)) {
        return Err(refuse("tool name"));
    }
    let root = user_data.join("tools");
    let dir = root.join(bundle);
    if !is_complete(&dir, files) {
        // Start clean, write everything, then the marker.
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        for (name, bytes) in files {
            fs::write(dir.join(name), bytes)?;
        }
        fs::write(dir.join(COMPLETE), bundle)?;
    }
    // Best effort: a bundle left by an earlier version is only wasted space.
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            if entry.path() != dir {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
    Ok(dir)
}

/// Where to look for the tools, first match wins: the folder a self-updated
/// yt-dlp lands in, then the unpacked bundle. A development build also looks
/// in a `bin` folder beside the program and in the working directory.
pub fn tool_dirs(user_data: &Path, unpacked: &Path) -> Vec<PathBuf> {
    tool_dirs_for(user_data, unpacked, cfg!(debug_assertions))
}

/// `tool_dirs`, with the kind of build stated. A shipped build (`development`
/// false) searches only the app's own data folder: a `bin` folder beside the
/// program or in the current folder could have been put there by someone
/// else, and whatever `yt-dlp.exe` it held would be run.
pub fn tool_dirs_for(user_data: &Path, unpacked: &Path, development: bool) -> Vec<PathBuf> {
    let mut dirs = vec![user_data.join("bin"), unpacked.to_owned()];
    if development {
        if let Some(beside) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|p| p.join("bin"))) {
            dirs.push(beside);
        }
        if let Ok(cwd) = std::env::current_dir() {
            dirs.push(cwd.join("bin"));
        }
    }
    dirs
}
