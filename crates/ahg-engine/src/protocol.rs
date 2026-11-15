//! `ahg://` links: an address opened anywhere on the OS is handed to this app.
//! Ported from electron/protocol.ts. Registering as the OS handler, and the
//! single-instance forwarding of a link to the running window, are the app
//! layer's; what is here is what to register and which links to accept.
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
    (detection.platform != SourcePlatform::Unknown).then_some(detection)
}
