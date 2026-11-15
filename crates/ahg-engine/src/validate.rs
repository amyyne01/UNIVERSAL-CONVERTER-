//! Checks on input that really comes from outside the program: pasted and
//! clipboard links, `ahg://` links, file paths and imported cookie files.
//! Ported from electron/security.ts. Its sender attestation, rate limiting and
//! content policy guarded the web page inside Electron and have no counterpart.
use crate::text::js_trim;

/// `value.length` in JavaScript: UTF-16 code units.
fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

const MAX_URL: usize = 2048;

/// The guard a pasted URL passes before it reaches detection or anything that
/// spawns: bounded, on one line, and unable to be read as a command-line flag.
pub fn url_arg(value: &str) -> Result<&str, String> {
    if js_len(value) > MAX_URL {
        return Err("Invalid input: expected a string within length limits".into());
    }
    if value.starts_with('-') {
        return Err("Invalid input: URL must not start with \"-\"".into());
    }
    if value.contains(['\r', '\n']) {
        return Err("Invalid input: URL must be a single line".into());
    }
    Ok(value)
}

fn starts_with_ignore_ascii_case(s: &str, prefix: &str) -> bool {
    s.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

/// `url_arg` for everything that spawns yt-dlp: http(s) links only, plus the
/// Spotify bridge's `ytsearch:` query.
pub fn media_url_arg(value: &str) -> Result<&str, String> {
    let url = url_arg(value)?;
    if ["https://", "http://", "ytsearch:"].iter().any(|p| starts_with_ignore_ascii_case(url, p)) {
        Ok(url)
    } else {
        Err("Invalid input: only http(s) links can be downloaded".into())
    }
}

const MAX_PLAYLIST_ITEMS: usize = 256;

/// A yt-dlp `--playlist-items` spec such as "1,3-5": digits, commas and ranges
/// only, bounded. It becomes a spawn argument.
pub fn playlist_items_arg(value: &str) -> Result<&str, String> {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let item = |s: &str| match s.split_once('-') {
        Some((from, to)) => digits(from) && digits(to),
        None => digits(s),
    };
    if js_len(value) <= MAX_PLAYLIST_ITEMS && value.split(',').all(item) {
        Ok(value)
    } else {
        Err("Invalid input: playlistItems".into())
    }
}

/// Measured in UTF-16 units, like the original's `text.length`.
pub const MAX_COOKIES_BYTES: usize = 2 * 1024 * 1024;

/// True when `text` is a plausible Netscape cookies.txt: bounded, and every
/// line is blank, a `#` comment, or seven tab-separated fields. The content
/// stays untrusted (yt-dlp parses it); this only refuses obvious non-cookie
/// files before they are copied into the app's data folder.
pub fn is_netscape_cookies(text: &str) -> bool {
    if text.is_empty() || js_len(text) > MAX_COOKIES_BYTES || text.contains('\0') {
        return false;
    }
    let mut cookies = 0;
    for line in text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)) {
        if js_trim(line).is_empty() || line.starts_with('#') {
            continue;
        }
        if line.split('\t').count() != 7 {
            return false;
        }
        cookies += 1;
    }
    cookies > 0
}

fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}

/// Node's `path.win32.resolve`, done on text alone: it never touches the disk
/// and keeps trailing dots and spaces, which the Windows API would strip.
fn win32_resolve(paths: &[&str]) -> String {
    let cwd = || std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let mut device = String::new();
    let mut tail = String::new();
    let mut absolute = false;

    for i in (-1..paths.len() as isize).rev() {
        let path: String = if i >= 0 {
            let p = paths[i as usize];
            if p.is_empty() {
                continue;
            }
            p.to_owned()
        } else if device.is_empty() {
            cwd()
        } else {
            // The per-drive current directory, falling back to the drive root.
            let p = std::env::var(format!("={device}")).unwrap_or_else(|_| cwd());
            let c: Vec<char> = p.chars().take(3).collect();
            let same_drive = c.len() >= 2 && c[..2].iter().collect::<String>().eq_ignore_ascii_case(&device);
            if !same_drive && c.get(2) == Some(&'\\') { format!("{device}\\") } else { p }
        };

        let c: Vec<char> = path.chars().collect();
        let len = c.len();
        let mut root_end = 0;
        let mut this_device = String::new();
        let mut is_absolute = false;
        if len == 1 {
            if is_sep(c[0]) {
                root_end = 1;
                is_absolute = true;
            }
        } else if is_sep(c[0]) {
            is_absolute = true;
            if is_sep(c[1]) {
                // A UNC root is \\server\share; anything less is a plain root.
                let mut j = 2;
                let mut last = j;
                while j < len && !is_sep(c[j]) {
                    j += 1;
                }
                if j < len && j != last {
                    let server: String = c[last..j].iter().collect();
                    last = j;
                    while j < len && is_sep(c[j]) {
                        j += 1;
                    }
                    if j < len && j != last {
                        last = j;
                        while j < len && !is_sep(c[j]) {
                            j += 1;
                        }
                        if j == len || j != last {
                            this_device = format!("\\\\{server}\\{}", c[last..j].iter().collect::<String>());
                            root_end = j;
                        }
                    }
                }
            } else {
                root_end = 1;
            }
        } else if c[0].is_ascii_alphabetic() && c[1] == ':' {
            this_device = c[..2].iter().collect();
            root_end = 2;
            if len > 2 && is_sep(c[2]) {
                is_absolute = true;
                root_end = 3;
            }
        }

        if !this_device.is_empty() {
            if device.is_empty() {
                device = this_device;
            } else if !this_device.eq_ignore_ascii_case(&device) {
                continue; // a path on another drive contributes nothing
            }
        }
        if absolute {
            if !device.is_empty() {
                break;
            }
        } else {
            tail = format!("{}\\{tail}", c[root_end..].iter().collect::<String>());
            absolute = is_absolute;
            if is_absolute && !device.is_empty() {
                break;
            }
        }
    }

    // Collapse `.`, `..` and repeated separators.
    let mut parts: Vec<&str> = Vec::new();
    for seg in tail.split(is_sep) {
        match seg {
            "" | "." => {}
            ".." => match parts.last() {
                Some(&top) if top != ".." => {
                    parts.pop();
                }
                _ if !absolute => parts.push(".."),
                _ => {}
            },
            seg => parts.push(seg),
        }
    }
    let tail = parts.join("\\");
    if absolute {
        format!("{device}\\{tail}")
    } else if device.is_empty() && tail.is_empty() {
        ".".to_owned()
    } else {
        format!("{device}{tail}")
    }
}

/// Confines a path to an allowed root. Returns the absolute path when it sits
/// inside `root` (or is `root`), `None` when it escapes through `..`, another
/// drive, a network share or an absolute path elsewhere. The comparison is
/// case-sensitive, as in the original.
pub fn confine_to_root(candidate: &str, root: &str) -> Option<String> {
    let abs_root = win32_resolve(&[root]);
    let abs = win32_resolve(&[&abs_root, candidate]);
    // D29: a drive root already ends in a separator. The original appended a
    // second one, so nothing was ever inside `D:\`.
    let inside = if abs_root.ends_with('\\') { abs_root.clone() } else { format!("{abs_root}\\") };
    (abs == abs_root || abs.starts_with(&inside)).then_some(abs)
}
