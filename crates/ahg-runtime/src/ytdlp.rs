//! Finding yt-dlp and ffmpeg, and asking yt-dlp about a link without
//! downloading it: metadata for a link, search results, and the search behind
//! the Spotify bridge.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ahg_engine::downloader::errors::{ErrorClass, classify_error, error_code_of, explain_error};
use ahg_engine::downloader::metadata::{metadata_from_items, search_results};
use ahg_engine::log::{host_of, log_line};
use ahg_engine::text::js_trim;
use ahg_engine::types::{MediaMetadata, SearchResult, SourcePlatform};
use ahg_engine::url_detector::detect_url;
use serde_json::Value;

use crate::proc::{Stream, Utf8Chunks, hidden, kill_tree, read_output, wait_for};

pub const TIMEOUT_MESSAGE: &str = "Timed out reading that link — the site did not answer in time.";
/// How long a metadata or search call may take before it is stopped.
const METADATA_TIMEOUT: Duration = Duration::from_secs(90);
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_owned() }
}

/// `"<bin>" --version`, bounded: the output if it ran and succeeded.
fn version_of(bin: &Path) -> Option<String> {
    let mut cmd = hidden(bin);
    cmd.arg("--version").stderr(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    let Some(status) = wait_for(&mut child, VERSION_TIMEOUT) else {
        kill_tree(child.id());
        return None;
    };
    let mut out = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        use std::io::Read;
        let _ = pipe.read_to_string(&mut out);
    }
    status.success().then(|| js_trim(&out).to_owned())
}

/// An ordered, cached search for the bundled tools. yt-dlp must actually run
/// (`--version`), so a corrupt download falls through to the next folder;
/// ffmpeg only has to exist, because yt-dlp validates and drives it.
pub struct Tools {
    /// Folders to look in, first match wins. The folder a self-updated yt-dlp
    /// lands in comes first, then the bundled copy.
    dirs: Vec<PathBuf>,
    /// Whether a `yt-dlp` found on the system path may be used when the
    /// folders hold none. Never in a shipped build: the path is not the app's.
    allow_path: bool,
    ytdlp: Mutex<Option<PathBuf>>,
    ffmpeg: Mutex<Option<String>>,
}

impl Tools {
    /// In a development build a `yt-dlp` on the system path is the last resort.
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs, allow_path: cfg!(debug_assertions), ytdlp: Mutex::new(None), ffmpeg: Mutex::new(None) }
    }

    /// Only the given folders, as a shipped build searches.
    pub fn strict(dirs: Vec<PathBuf>) -> Self {
        Self { dirs, allow_path: false, ytdlp: Mutex::new(None), ffmpeg: Mutex::new(None) }
    }

    pub fn ytdlp(&self) -> Option<PathBuf> {
        let mut cached = self.ytdlp.lock().unwrap_or_else(|e| e.into_inner());
        if cached.is_none() {
            let found = self.dirs.iter().map(|d| d.join(exe("yt-dlp"))).find(|c| c.exists() && version_of(c).is_some());
            // In development the bare command on the system path is the last
            // resort, and is validated too.
            let on_path = || version_of(Path::new("yt-dlp")).map(|_| PathBuf::from("yt-dlp"));
            *cached = found.or_else(|| if self.allow_path { on_path() } else { None });
        }
        cached.clone()
    }

    pub fn ffmpeg(&self) -> Option<String> {
        let mut cached = self.ffmpeg.lock().unwrap_or_else(|e| e.into_inner());
        if cached.is_none() {
            *cached = self.dirs.iter().map(|d| d.join(exe("ffmpeg"))).find(|c| c.exists()).map(|c| c.to_string_lossy().into_owned());
        }
        cached.clone()
    }

    /// The installed yt-dlp release, such as "2026.06.30", or "" when there is none.
    pub fn ytdlp_version(&self) -> String {
        self.ytdlp().and_then(|bin| version_of(&bin)).unwrap_or_default()
    }

    /// Forget the cached yt-dlp, so the next call searches again. Called after
    /// the engine updater swaps in a new one.
    pub fn reset_cache(&self) {
        *self.ytdlp.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

enum Bins {
    Fixed(PathBuf, Option<String>),
    Found(Arc<Tools>),
}

/// yt-dlp, as the app runs it.
pub struct YtDlp {
    bins: Bins,
    env: Vec<(String, String)>,
    metadata_timeout: Duration,
    /// The proxy from Settings, for lookups and searches. The download gets
    /// its proxy through the task's extras.
    proxy: Mutex<Option<String>>,
}

impl YtDlp {
    /// A yt-dlp at a known path, with ffmpeg if there is one.
    pub fn new(ytdlp: PathBuf, ffmpeg: Option<String>) -> Self {
        Self { bins: Bins::Fixed(ytdlp, ffmpeg), env: Vec::new(), metadata_timeout: METADATA_TIMEOUT, proxy: Mutex::new(None) }
    }

    /// A yt-dlp looked up through `tools` on every use, so an engine update is picked up.
    pub fn from_tools(tools: Arc<Tools>) -> Self {
        Self { bins: Bins::Found(tools), env: Vec::new(), metadata_timeout: METADATA_TIMEOUT, proxy: Mutex::new(None) }
    }

    /// An extra environment variable for every process started.
    pub fn env(&mut self, key: &str, value: &str) {
        self.env.push((key.to_owned(), value.to_owned()));
    }

    /// D10: route lookups and searches through this proxy. `None` or empty is direct.
    pub fn set_proxy(&self, proxy: Option<&str>) {
        *self.proxy.lock().unwrap_or_else(|e| e.into_inner()) = proxy.filter(|p| !p.is_empty()).map(str::to_owned);
    }

    pub fn set_metadata_timeout(&mut self, limit: Duration) {
        self.metadata_timeout = limit;
    }

    pub fn bin(&self) -> Option<PathBuf> {
        match &self.bins {
            Bins::Fixed(bin, _) => Some(bin.clone()),
            Bins::Found(tools) => tools.ytdlp(),
        }
    }

    pub fn ffmpeg(&self) -> Option<String> {
        match &self.bins {
            Bins::Fixed(_, ffmpeg) => ffmpeg.clone(),
            Bins::Found(tools) => tools.ffmpeg(),
        }
    }

    /// A hidden command for `bin` with this yt-dlp's environment.
    pub fn command(&self, bin: &Path) -> Command {
        let mut cmd = hidden(bin);
        cmd.envs(self.env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        cmd
    }

    /// Run yt-dlp in inspect mode and read one JSON record per line. A
    /// transient failure is tried once more; a permanent one answers the same
    /// twice, and a timeout has already cost the full wait, so neither is.
    pub fn run_json(&self, args: &[&str]) -> Result<Vec<Value>, String> {
        match self.run_json_once(args) {
            Err(msg) if msg != TIMEOUT_MESSAGE && classify_error(&msg) == ErrorClass::Retryable => self.run_json_once(args),
            other => other,
        }
    }

    fn run_json_once(&self, args: &[&str]) -> Result<Vec<Value>, String> {
        let bin = self.bin().ok_or("yt-dlp not found")?;
        // The socket timeout bounds a stalled connection, and the retries let
        // yt-dlp ride out a hiccup itself.
        let mut cmd = self.command(&bin);
        cmd.args(["--socket-timeout", "20", "--retries", "3"]);
        if let Some(proxy) = self.proxy.lock().unwrap_or_else(|e| e.into_inner()).as_deref() {
            cmd.args(["--proxy", proxy]);
        }
        cmd.args(args);
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let pid = child.id();
        let chunks = read_output(&mut child);

        // The watchdog is the only thing that can end a process that never answers.
        let deadline = std::time::Instant::now() + self.metadata_timeout;
        let (mut out, mut err) = (String::new(), String::new());
        let (mut out_chunks, mut err_chunks) = (Utf8Chunks::default(), Utf8Chunks::default());
        loop {
            match chunks.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())) {
                Ok((Stream::Stdout, bytes)) => out.push_str(&out_chunks.push(&bytes)),
                Ok((Stream::Stderr, bytes)) => err.push_str(&err_chunks.push(&bytes)),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    kill_tree(pid);
                    let _ = child.wait();
                    return Err(TIMEOUT_MESSAGE.to_owned());
                }
            }
        }
        out.push_str(&out_chunks.finish());
        err.push_str(&err_chunks.finish());
        let code = child.wait().ok().and_then(|s| s.code());

        if code != Some(0) && js_trim(&out).is_empty() {
            let text = js_trim(&err);
            return Err(if text.is_empty() {
                format!("yt-dlp exited with code {}", code.map_or("null".to_owned(), |c| c.to_string()))
            } else {
                text.to_owned()
            });
        }
        // Lines that are not JSON are other chatter and are skipped.
        Ok(out.split('\n').map(js_trim).filter(|l| !l.is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect())
    }

    /// What a link holds: one item, or a collection with its entries.
    pub fn fetch_metadata(&self, url: &str) -> Result<MediaMetadata, String> {
        // A video opened from inside a list would otherwise make yt-dlp
        // enumerate the entire list before answering.
        let mut args = vec!["--dump-json", "--no-warnings", "--flat-playlist"];
        if !detect_url(url).is_collection {
            args.push("--no-playlist");
        }
        args.extend(["--", url]);
        let items = self.run_json(&args).map_err(|msg| {
            let code = serde_json::to_value(error_code_of(&msg)).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
            log_line("error", &format!("fetch failed host={} code={code}", host_of(url)));
            // The interface waits on this call, so it gets an explanation it can show.
            explain_error(&msg)
        })?;
        metadata_from_items(&items, url)
    }

    pub fn search_youtube(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        self.search(&format!("ytsearch12:{query}"), SourcePlatform::Youtube)
    }

    pub fn search_soundcloud(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        self.search(&format!("scsearch12:{query}"), SourcePlatform::Soundcloud)
    }

    fn search(&self, query: &str, source: SourcePlatform) -> Result<Vec<SearchResult>, String> {
        let items = self.run_json(&["--dump-json", "--no-warnings", "--flat-playlist", query])?;
        Ok(search_results(&items, source))
    }
}
