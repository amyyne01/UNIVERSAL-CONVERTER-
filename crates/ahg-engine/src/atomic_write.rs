use std::fs;
use std::io;
use std::path::Path;

/// Write via `<path>.tmp` then rename, so a crash or a full disk never leaves a
/// truncated file at `path`. On failure the tmp file is removed, the original
/// is left as it was, and the error is returned for the caller to handle.
pub fn write_file_atomic(path: &Path, data: &str) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let result = fs::write(&tmp, data).and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
