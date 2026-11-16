//! The decisions and data behind the thin Electron wrappers: the completion
//! toast and taskbar progress (electron/feedback.ts), the global-hotkey checks
//! (hotkey.ts) and the tray menu (tray.ts). Showing a toast, setting the taskbar
//! bar, registering a shortcut and building the native menu are the app layer's.
use crate::types::DownloadStatus;

// ── feedback.ts ──────────────────────────────────────────────────────────────

fn is_active(status: DownloadStatus) -> bool {
    use DownloadStatus::*;
    matches!(status, FetchingInfo | Downloading | Converting | Embedding)
}

/// The toast for a finished task: its title (or a stock line when it has none),
/// only for a download that is done and only while notifications are on.
pub fn completion_toast(status: DownloadStatus, title: &str, enabled: bool) -> Option<String> {
    if status != DownloadStatus::Done || !enabled {
        return None;
    }
    Some(if title.is_empty() { "Download complete" } else { title }.to_string())
}

/// The taskbar progress bar: average percent of the active downloads as 0..1,
/// or -1 to remove the bar when none is active.
#[derive(Debug, Default)]
pub struct TaskbarProgress {
    last: Option<f64>,
}

impl TaskbarProgress {
    /// `tasks` are (status, percent). Returns the value to set, or `None` when it
    /// is the same as the last one set (a NaN is never the same).
    pub fn update(&mut self, tasks: &[(DownloadStatus, f64)]) -> Option<f64> {
        let active: Vec<f64> = tasks.iter().filter(|(s, _)| is_active(*s)).map(|(_, p)| *p).collect();
        let p = if active.is_empty() { -1.0 } else { active.iter().fold(0.0, |s, p| s + p) / active.len() as f64 / 100.0 };
        if self.last == Some(p) {
            return None;
        }
        self.last = Some(p);
        Some(p)
    }
}

// ── hotkey.ts ────────────────────────────────────────────────────────────────

/// Registering or unregistering an empty accelerator does nothing (and
/// registering one reports failure). Nothing else about the string is checked:
/// Electron decides whether it is valid.
pub fn accelerator_given(accelerator: &str) -> bool {
    !accelerator.is_empty()
}

/// What `registerGlobalHotkey` returns for Electron's answer; a missing one
/// counts as failure.
pub fn register_outcome(electron: Option<bool>) -> bool {
    electron.unwrap_or(false)
}

// ── tray.ts ──────────────────────────────────────────────────────────────────

pub const TRAY_TOOLTIP: &str = "AHG Universal Converter";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    PauseAll,
    ResumeAll,
    ToggleWindow,
    PasteAndDownload,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    Separator,
    /// A disabled entry has no action.
    Item {
        label: String,
        enabled: bool,
        action: Option<TrayAction>,
    },
}

/// A small glyph per state so the queue reads at a glance.
pub fn status_glyph(status: DownloadStatus) -> &'static str {
    use DownloadStatus::*;
    match status {
        Queued => "◷",
        FetchingInfo => "◌",
        Downloading => "↓",
        Converting => "⚙",
        Embedding => "♪",
        Done => "✓",
        Paused => "⏸",
        Retrying => "↻",
        Failed => "✗",
        Cancelled => "⊘",
    }
}

/// The tray menu for the recent downloads (title, status): one disabled line
/// each (or "No recent downloads"), then the controls.
pub fn tray_menu(recent: &[(String, DownloadStatus)]) -> Vec<MenuEntry> {
    let item = |label: &str, action| MenuEntry::Item { label: label.to_string(), enabled: true, action: Some(action) };
    let mut menu: Vec<MenuEntry> = if recent.is_empty() {
        vec![MenuEntry::Item { label: "No recent downloads".into(), enabled: false, action: None }]
    } else {
        recent
            .iter()
            .map(|(title, status)| MenuEntry::Item { label: format!("{}  {title}", status_glyph(*status)), enabled: false, action: None })
            .collect()
    };
    menu.extend([
        MenuEntry::Separator,
        item("Pause all", TrayAction::PauseAll),
        item("Resume all", TrayAction::ResumeAll),
        item("Show / Hide", TrayAction::ToggleWindow),
        item("Paste & Download", TrayAction::PasteAndDownload),
        MenuEntry::Separator,
        item("Quit", TrayAction::Quit),
    ]);
    menu
}
