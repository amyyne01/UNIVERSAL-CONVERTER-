//! `ahg://` links: an address opened anywhere on the OS is handed to this app.
//! Ported from electron/protocol.ts. Registering as the OS handler, and the
//! single-instance forwarding of a link to the running window, are the app
//! layer's; what is here is what to register and which links to accept.
use url::{Host, Url};

use crate::types::{SourcePlatform, UrlDetection};
use crate::url_detector::detect_url;
use crate::validate::media_url_arg;

pub const PROTOCOL_SCHEME: &str = "ahg";
const PREFIX: &str = "ahg://";

/// What `registerProtocolHandler` asks the OS for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Registration {
    /// Nothing: running as the bare dev executable without an entry script.
    Skip,
    /// Register the scheme for this executable.
    Plain,
    /// Register the scheme for this executable and the entry script
    /// (`argv[1]` made absolute), so the OS does not relaunch bare Electron.
    ViaScript,
}

/// `default_app` is `process.defaultApp` (the executable is Electron itself).
pub fn registration(default_app: bool, argv_len: usize) -> Registration {
    match (default_app, argv_len >= 2) {
        (false, _) => Registration::Plain,
        (true, true) => Registration::ViaScript,
        (true, false) => Registration::Skip,
    }
}

/// The first `ahg://` entry of the launch arguments (case-sensitive).
pub fn extract_protocol_url<S: AsRef<str>>(argv: &[S]) -> Option<&str> {
    argv.iter().map(AsRef::as_ref).find(|a| a.starts_with(PREFIX))
}

/// The link inside an `ahg://` address, checked like a pasted URL and then
/// detected. `None` is "not a link this app can use".
pub fn resolve_protocol_link(raw: &str) -> Option<UrlDetection> {
    let target = media_url_arg(raw.strip_prefix(PREFIX)?).ok()?;
    if target.is_empty() {
        return None;
    }
    let detection = detect_url(target);
    // Whatever platform was detected: its patterns match anywhere in the text,
    // so a platform name in the path of an inside host would pass for a site.
    (detection.platform != SourcePlatform::Unknown && !points_inside(&detection.url)).then_some(detection)
}

/// D36 (Rust only): a link that arrives through `ahg://` may not name this PC
/// or the home network. Judged on the host text alone, no DNS: an IP literal
/// (any spelling the URL parser reads), `localhost`, `*.localhost`. A link the
/// user pastes is their own act and is not checked here.
/// The raw text goes on to the downloader, which may read an address
/// differently from the `url` crate, so anything that makes the host ambiguous
/// is refused first: a backslash or `%5c`, a space or control character, and
/// userinfo (`@` in the authority), none of which a media link needs.
/// ponytail: a public name that resolves to a private address is not caught
/// (needs DNS, which the engine does not do).
fn points_inside(target: &str) -> bool {
    let web = ["https://", "http://"].iter().find(|p| target.get(..p.len()).is_some_and(|h| h.eq_ignore_ascii_case(p)));
    let Some(web) = web else { return false }; // `ytsearch:` and the like have no host
    let authority = target[web.len()..].split(['/', '?', '#']).next().unwrap_or("");
    if target.bytes().any(|b| b <= b' ' || b == 0x7f || b == b'\\')
        || target.to_ascii_lowercase().contains("%5c")
        || authority.contains('@')
    {
        return true;
    }
    let Ok(url) = Url::parse(target) else { return true };
    match url.host() {
        Some(Host::Domain(d)) => {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            d == "localhost" || d.ends_with(".localhost")
        }
        // Every IP literal is refused; a real media site is addressed by name.
        Some(Host::Ipv4(_) | Host::Ipv6(_)) => true,
        None => false,
    }
}
