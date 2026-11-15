//! Persisted, human-readable settings in the per-user app-data folder:
//! defaults laid first, saved values merged on top, unknown keys stripped,
//! debounced saves with one immediate flush at shutdown.
//!
//! Settings are kept as an ordered JSON object, not a typed struct. The file
//! is shared with the Electron app, which accepts any value whose JSON type
//! matches the default's (a `theme` of "neon" is kept and written back), and
//! the order of keys on disk is the order of the defaults.
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::atomic_write::write_file_atomic;
use crate::validate::win32_resolve;

const DEBOUNCE: Duration = Duration::from_millis(400);
const BACKUP_COUNT: usize = 3;
const SCHEMA_VERSION: u64 = 1;
const DANGEROUS_KEYS: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// The colour palette ids (`PALETTES` in shared/types.ts).
pub const PALETTE_IDS: [&str; 7] = ["aurora", "slate", "moss", "ember", "paper", "lagoon", "orchid"];

pub fn default_config() -> Map<String, Value> {
    let defaults = json!({
        "schemaVersion": SCHEMA_VERSION,
        "outputDir": "",
        "defaultFormat": "mp3",
        "defaultQuality": "320",
        "defaultVideoQuality": "best",
        "embedThumbnail": true,
        "embedMetadata": true,
        "skipExisting": true,
        "theme": "system",
        "palette": "aurora",
        "clipboardWatch": false,
        "showNotifications": true,
        "rateLimit": "",
        "proxy": "",
        "globalHotkey": "CommandOrControl+Shift+D",
        "discordRichPresence": false,
        "scheduleEnabled": false,
        "scheduleTime": "03:00",
        "scheduleDays": [],
        "scheduleShutdown": false,
        "autoUpdateEngine": true,
        // Clamped by tier before it reaches the queue. 2 is what basic allows,
        // so a settings file with no such key lands on the free cap.
        "maxConcurrentDownloads": 2,
        // Download extras, flat like every other key so each is checked on its own.
        "subtitleMode": "off",
        "subtitleLangs": "en",
        "subtitleAuto": false,
        "embedChapters": true,
        "splitChapters": false,
        "sponsorBlock": "off",
        "sponsorBlockCategories": ["sponsor"],
        "writeThumbnail": false,
        "writeInfoJson": false,
        "writeDescription": false,
        "videoContainer": "mp4",
        "videoCodec": "any",
        "cookieBrowser": "",
        "outputTemplate": "",
    });
    match defaults {
        Value::Object(map) => map,
        _ => unreachable!("the literal above is an object"),
    }
}

/// Brings a loaded settings object up to the current shape. No key has been
/// renamed yet, so the only step stamps `schemaVersion`.
/// A hand-edited fractional or negative version is stamped too; Electron
/// crashes at launch on those.
pub fn migrate_config(mut obj: Map<String, Value>) -> Map<String, Value> {
    let current = obj.get("schemaVersion").and_then(Value::as_f64);
    if current.is_none_or(|v| v < SCHEMA_VERSION as f64) {
        obj.insert("schemaVersion".into(), json!(SCHEMA_VERSION));
    }
    obj
}

fn is_dir(p: &str) -> bool {
    fs::metadata(p).is_ok_and(|m| m.is_dir())
}

/// Locations no download may be written into, read from the environment and
/// not from hard-coded drive letters.
const SYSTEM_DIR_VARS: [&str; 5] = ["SystemRoot", "windir", "ProgramFiles", "ProgramFiles(x86)", "ProgramData"];

/// A download folder: a drive-letter path (no network share, no relative
/// path), no `..` segment, outside every system location, and an existing
/// directory. The system drive's own root is refused; other drive roots are allowed.
fn is_output_dir(p: &str) -> bool {
    let mut chars = p.chars();
    let drive_path = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.next() == Some(':')
        && matches!(chars.next(), Some('\\' | '/'));
    if !drive_path || p.split(['\\', '/']).any(|seg| seg == "..") {
        return false;
    }
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let abs = win32_resolve(&[p]).to_lowercase();
    if let Some(drive) = env("SystemDrive")
        && abs == win32_resolve(&[&format!("{drive}\\")]).to_lowercase()
    {
        return false;
    }
    for var in SYSTEM_DIR_VARS {
        if let Some(dir) = env(var) {
            let root = win32_resolve(&[&dir]).to_lowercase();
            if abs == root || abs.starts_with(&format!("{root}\\")) {
                return false;
            }
        }
    }
    is_dir(p)
}

/// Shape rules for string settings that reach a spawn or the filesystem.
/// `None` means the key has no rule.
fn string_rule(key: &str, v: &str) -> Option<bool> {
    let digits_then = |v: &str, suffix: &[char]| {
        let digits = v.trim_end_matches(suffix);
        v.len() - digits.len() <= 1 && !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
    };
    Some(match key {
        "outputDir" => is_output_dir(v),
        "proxy" => ["http://", "https://", "socks5://", "socks5h://"].iter().any(|p| v.starts_with(p)),
        "rateLimit" => digits_then(v, &['K', 'M', 'G']),
        "palette" => PALETTE_IDS.contains(&v),
        _ => return None,
    })
}

/// JavaScript's `typeof`, for the JSON values that are not arrays.
fn type_of(v: &Value) -> &'static str {
    match v {
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Null | Value::Array(_) | Value::Object(_) => "object",
    }
}

pub type ListenerId = u64;

pub struct ConfigManager {
    defaults: Map<String, Value>,
    config: Map<String, Value>,
    file_path: PathBuf,
    downloads_dir: String,
    save_due: Option<Instant>,
    listeners: Vec<(ListenerId, Box<dyn FnMut() + Send>)>,
    next_listener: ListenerId,
    restored_from_backup: bool,
}

impl ConfigManager {
    /// `user_data` is the app's data folder; `downloads_dir` is the OS
    /// Downloads folder, which an empty output folder is anchored to.
    pub fn new(user_data: impl AsRef<Path>, downloads_dir: impl Into<String>) -> Self {
        let mut this = Self {
            defaults: default_config(),
            config: Map::new(),
            file_path: user_data.as_ref().join("config.json"),
            downloads_dir: downloads_dir.into(),
            save_due: None,
            listeners: Vec::new(),
            next_listener: 0,
            restored_from_backup: false,
        };
        this.config = this.load();
        // An empty output folder would make yt-dlp write into the working
        // directory; anchor it so a first run or a hand-cleared value is sane.
        if this.config["outputDir"].as_str().is_none_or(str::is_empty) {
            this.config.insert("outputDir".into(), json!(this.downloads_dir));
        }
        this
    }

    /// True if the last load fell back to the backup ring because the settings
    /// file was missing or corrupt and an earlier snapshot still parsed.
    pub fn was_restored_from_backup(&self) -> bool {
        self.restored_from_backup
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.config.get(key)
    }

    pub fn get_all(&self) -> &Map<String, Value> {
        &self.config
    }

    /// Called after every `set` and `update`, so services can follow a change
    /// made in Settings instead of reading the settings only at startup.
    pub fn on_change(&mut self, cb: impl FnMut() + Send + 'static) -> ListenerId {
        self.next_listener += 1;
        self.listeners.push((self.next_listener, Box::new(cb)));
        self.next_listener
    }

    pub fn off_change(&mut self, id: ListenerId) {
        self.listeners.retain(|(l, _)| *l != id);
    }

    fn notify_change(&mut self) {
        for (_, cb) in &mut self.listeners {
            cb();
        }
    }

    /// False when the key or value is refused, in which case nothing changes.
    pub fn set(&mut self, key: &str, value: Value) -> bool {
        self.set_at(key, value, Instant::now())
    }

    pub fn set_at(&mut self, key: &str, value: Value, now: Instant) -> bool {
        if !self.is_safe_key(key) || !self.is_valid_value(key, &value) {
            return false;
        }
        let value = self.normalize(key, value);
        self.config.insert(key.to_owned(), value);
        self.save_due = Some(now + DEBOUNCE);
        self.notify_change();
        true
    }

    /// Applies every acceptable key of `partial` and skips the rest.
    pub fn update(&mut self, partial: &Map<String, Value>) {
        self.update_at(partial, Instant::now());
    }

    pub fn update_at(&mut self, partial: &Map<String, Value>, now: Instant) {
        for (key, value) in partial {
            if self.is_safe_key(key) && self.is_valid_value(key, value) {
                let value = self.normalize(key, value.clone());
                self.config.insert(key.clone(), value);
            }
        }
        self.save_due = Some(now + DEBOUNCE);
        self.notify_change();
    }

    /// When the pending debounced save falls due, if one is pending.
    pub fn save_deadline(&self) -> Option<Instant> {
        self.save_due
    }

    /// Writes the pending save once its debounce has elapsed. The owner calls
    /// this from its timer; the manager starts no thread of its own.
    pub fn poll(&mut self) {
        self.poll_at(Instant::now());
    }

    pub fn poll_at(&mut self, now: Instant) {
        if self.save_due.is_some_and(|due| now >= due) {
            self.save_now();
        }
    }

    /// Immediate write, for shutdown.
    pub fn save_now(&mut self) {
        self.save_due = None;
        // Failures (full disk, locked file) are swallowed: this runs from a
        // timer and on quit, and the settings in memory stay correct.
        self.rotate_backups();
        if let Ok(text) = serde_json::to_string_pretty(&self.config) {
            let _ = write_file_atomic(&self.file_path, &text);
        }
    }

    fn load(&mut self) -> Map<String, Value> {
        let mut merged = self.defaults.clone();
        let saved = match fs::read(&self.file_path).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
            Some(saved) => saved,
            // Missing or corrupt: take the newest backup that still parses
            // instead of falling straight back to defaults.
            None => match self.restore_from_backup_ring() {
                Some(restored) => {
                    self.restored_from_backup = true;
                    restored
                }
                None => return merged,
            },
        };
        let Value::Object(saved) = saved else { return merged };
        for (key, value) in migrate_config(saved) {
            if self.is_safe_key(&key) && self.is_valid_value(&key, &value) {
                merged.insert(key, value);
            }
        }
        merged
    }

    fn backup_path(&self, n: usize) -> PathBuf {
        self.file_path.with_file_name(format!("config.backup.{n}.json"))
    }

    /// The first of backup 1..N, newest first, that parses to an object or array.
    fn restore_from_backup_ring(&self) -> Option<Value> {
        (1..=BACKUP_COUNT).find_map(|n| {
            let parsed: Value = serde_json::from_slice(&fs::read(self.backup_path(n)).ok()?).ok()?;
            matches!(parsed, Value::Object(_) | Value::Array(_)).then_some(parsed)
        })
    }

    /// Shift backup 1 to 2 to 3 (the oldest is dropped), then the settings file
    /// about to be overwritten becomes the new backup 1.
    fn rotate_backups(&self) {
        for n in (2..=BACKUP_COUNT).rev() {
            let _ = fs::copy(self.backup_path(n - 1), self.backup_path(n));
        }
        let _ = fs::copy(&self.file_path, self.backup_path(1));
    }

    /// Only known, non-dangerous keys.
    fn is_safe_key(&self, key: &str) -> bool {
        !DANGEROUS_KEYS.contains(&key) && self.defaults.contains_key(key)
    }

    /// Accept a value only when its JSON type matches the default's. The file
    /// is hand-editable, so a mistyped `"scheduleDays": "mon"` or a numeric
    /// hotkey is refused here instead of failing later in the scheduler.
    fn is_valid_value(&self, key: &str, value: &Value) -> bool {
        let Some(default) = self.defaults.get(key) else { return false };
        match (default, value) {
            (Value::Array(_), Value::Array(items)) => {
                // The element type cannot be read off an empty default, so the
                // one string-valued array is named.
                let elem = if key == "sponsorBlockCategories" { "string" } else { "number" };
                items.iter().all(|v| type_of(v) == elem)
            }
            (Value::Array(_), _) | (_, Value::Array(_)) => false,
            _ if type_of(value) != type_of(default) => false,
            // Empty always means "unset", except for the palette.
            (_, Value::String(s)) if s.is_empty() => key != "palette",
            (_, Value::String(s)) => string_rule(key, s).unwrap_or(true),
            _ => true,
        }
    }

    /// An emptied output folder is re-anchored on every write, not only at
    /// construction, so downloads never land in the working directory.
    fn normalize(&self, key: &str, value: Value) -> Value {
        if key == "outputDir" && value.as_str().is_some_and(str::is_empty) {
            json!(self.downloads_dir)
        } else {
            value
        }
    }
}
