//! Monthly download counters.
//!
//! The download history is capped and drops its oldest finished rows, so a
//! total derived from it would start shrinking. Counting happens here instead,
//! when a download finishes, into a bucket per month. Two buckets are kept: the
//! current month and the last month that had activity, which is what the
//! dashboard compares against. Older months are dropped.
//!
//! It counts files, not tasks: one playlist task can write twenty files.
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::atomic_write::write_file_atomic;
use crate::clock::LocalTime;
use crate::text::js_trim;
use crate::types::{SourcePlatform, num};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthBucket {
    /// "YYYY-MM", local time.
    pub month: String,
    #[serde(default, serialize_with = "num::serialize")]
    pub files: f64,
    #[serde(default, serialize_with = "num::serialize")]
    pub bytes: f64,
    /// Files per platform, in the order each platform first appeared.
    #[serde(default)]
    pub by_platform: Map<String, Value>,
}

impl MonthBucket {
    fn empty(month: &str) -> Self {
        Self { month: month.to_owned(), files: 0.0, bytes: 0.0, by_platform: Map::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadStats {
    pub current: MonthBucket,
    /// The last month with activity, for the comparison; `None` on a first run.
    pub previous: Option<MonthBucket>,
}

/// What may be on disk: either bucket can be missing or have an empty month.
#[derive(Deserialize)]
struct StatsFile {
    current: Option<MonthBucket>,
    previous: Option<MonthBucket>,
}

/// "YYYY-MM" in local time: the user's month, not UTC's.
pub fn month_key(at: &LocalTime) -> String {
    format!("{}-{:02}", at.year, at.month)
}

/// Where the counters live, beside the rest of the app's data.
pub fn stats_file_path(user_data: impl AsRef<Path>) -> PathBuf {
    user_data.as_ref().join("download-stats.json")
}

#[derive(Default)]
pub struct StatsStore {
    file: Option<PathBuf>,
    data: Option<DownloadStats>,
}

impl StatsStore {
    /// Load the counters, or start them. A missing or unreadable file starts
    /// fresh: statistics are never worth failing a launch over.
    pub fn init(&mut self, file: &Path) {
        self.init_at(file, &month_key(&LocalTime::now()));
    }

    pub fn init_at(&mut self, file: &Path, month: &str) {
        self.file = Some(file.to_owned());
        let loaded = fs::read(file).ok().and_then(|b| serde_json::from_slice::<StatsFile>(&b).ok());
        self.data = Some(match loaded {
            Some(StatsFile { current: Some(current), previous }) if !current.month.is_empty() => {
                DownloadStats { current, previous: previous.filter(|p| !p.month.is_empty()) }
            }
            _ => DownloadStats { current: MonthBucket::empty(month), previous: None },
        });
        self.roll(month);
    }

    fn data(&mut self, month: &str) -> &mut DownloadStats {
        self.data.get_or_insert_with(|| DownloadStats { current: MonthBucket::empty(month), previous: None })
    }

    /// Move to `month` if the stored current month is a different one. After a
    /// gap of several months the stored month is still the honest "previous":
    /// it is the last month with activity.
    fn roll(&mut self, month: &str) {
        let data = self.data(month);
        if data.current.month == month {
            return;
        }
        let last = std::mem::replace(&mut data.current, MonthBucket::empty(month));
        data.previous = Some(last);
        self.save();
    }

    fn save(&self) {
        // Best effort: a lost counter must never break a download.
        if let (Some(file), Some(data)) = (&self.file, &self.data)
            && let Ok(text) = serde_json::to_string(data)
        {
            let _ = write_file_atomic(file, &text);
        }
    }

    /// Record a finished download. `filepaths` is every file the engine wrote:
    /// one for a single track, many for a collection. Sizes are read from disk;
    /// `fallback_bytes` covers the case where no path can be measured.
    pub fn record(&mut self, platform: SourcePlatform, filepaths: &[String], fallback_bytes: f64) {
        self.record_at(platform, filepaths, fallback_bytes, &month_key(&LocalTime::now()));
    }

    pub fn record_at(&mut self, platform: SourcePlatform, filepaths: &[String], fallback_bytes: f64, month: &str) {
        self.roll(month);
        let paths: Vec<&String> = filepaths.iter().filter(|p| !js_trim(p).is_empty()).collect();
        // A file moved or renamed since it finished only loses its size.
        let bytes: f64 = paths.iter().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len() as f64).sum();
        // A completion always counts as at least one file.
        let files = if paths.is_empty() { 1.0 } else { paths.len() as f64 };
        let name = serde_json::to_value(platform).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
        let cur = &mut self.data(month).current;
        cur.files += files;
        cur.bytes += if bytes != 0.0 { bytes } else { fallback_bytes };
        let so_far = cur.by_platform.get(&name).and_then(Value::as_f64).unwrap_or(0.0);
        cur.by_platform.insert(name, json!((so_far + files) as u64));
        self.save();
    }

    /// Snapshot for the dashboard. It rolls first, so a month boundary crossed
    /// while the app sat open shows the next time the dashboard asks.
    pub fn get(&mut self) -> DownloadStats {
        self.get_at(&month_key(&LocalTime::now()))
    }

    pub fn get_at(&mut self, month: &str) -> DownloadStats {
        self.roll(month);
        self.data(month).clone()
    }
}
