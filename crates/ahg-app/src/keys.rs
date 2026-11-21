//! The element key of every control the shared actions answer to, one
//! constant or function each, so all six presets name a control the same way
//! (scripts, `--tree`, `Tx::focus` and motion find it by this key). Where two
//! presets spelled one control differently, the name here is the one kept.
use crate::gate::{Confirm, Filter, Nav};

// ── Places and overlays
pub const NAV_HOME: &str = "nav-home";
pub const NAV_DOWNLOADS: &str = "nav-downloads";
pub const NAV_SETTINGS: &str = "nav-settings";
pub const NAV_PIN: &str = "nav-pin";
pub const PICKER: &str = "picker";
pub const PALETTE: &str = "palette";
pub const PALETTE_SEARCH: &str = "palette-search";
pub const SHORTCUTS: &str = "shortcuts";
pub const SHORTCUTS_CLOSE: &str = "shortcuts-close";
pub const UPDATE_PROMPT: &str = "update-prompt";
pub const UPDATE_LATER: &str = "update-later";
pub const NOTICE: &str = "notice";
pub const NOTICE_ACTION: &str = "notice-action";
pub const NOTICE_DISMISS: &str = "notice-dismiss";

/// The place's own button in the preset's navigation.
pub fn nav(place: Nav) -> &'static str {
    match place {
        Nav::Home => NAV_HOME,
        Nav::Downloads => NAV_DOWNLOADS,
        Nav::Settings => NAV_SETTINGS,
    }
}

/// A preset's card in the picker, in the order of `gate::PRESETS`.
pub fn preset(index: usize) -> String {
    format!("preset-card-{index}")
}

/// A command palette row, by its place in `actions::commands`.
pub fn command(index: usize) -> String {
    format!("command-{index}")
}

// ── The field and the found step
pub const FIELD: &str = "field";
/// The soft right edge a long link fades out under (Now playing's dock).
pub const FIELD_FADE: &str = "field-fade";
pub const LOOK_UP: &str = "look-up";
/// The found card's cover box, and the bone that stands in for it while the link is read.
pub const FOUND_COVER: &str = "found-cover";
pub const PASTE: &str = "paste";
pub const SCOPE_CHIP: &str = "scope-chip";
pub const FOUND: &str = "found";
pub const FOUND_CLOSE: &str = "found-close";
pub const FOUND_DOWNLOAD: &str = "found-download";
pub const FOUND_SELECT_ALL: &str = "found-select-all";
pub const FOUND_SEE_PLANS: &str = "found-see-plans";
pub const CHOICE_VIDEO: &str = "choice-video";
pub const CHOICE_AUDIO: &str = "choice-audio";
pub const CHOICE_AUDIO_QUALITY: &str = "choice-audio-quality";
pub const CHOICE_VIDEO_QUALITY: &str = "choice-video-quality";
pub const CHOICE_FOLDER: &str = "choice-folder";
pub const CHOICE_FORMAT: &str = "choice-format";
pub const SEE_PLANS: &str = "see-plans";
pub const FOUND_RETRY: &str = "found-retry";

/// An audio format chip in the found step (`mp3`, `flac` ...).
pub fn choice_format(format: &str) -> String {
    format!("choice-format-{format}")
}

/// A video quality chip in the found step (`720p` ...).
pub fn choice_video_quality(q: &str) -> String {
    format!("choice-video-quality-{q}")
}

/// An audio quality chip in the found step (`320`, `lossless` ...).
pub fn choice_audio_quality(q: &str) -> String {
    format!("choice-audio-quality-{q}")
}

/// A playlist row's tick.
pub fn track(index: usize) -> String {
    format!("track-{index}")
}

/// A search result's own tick box.
pub fn result_tick(id: &str) -> String {
    format!("result-tick-{id}")
}

/// A search result's row.
pub fn result(id: &str) -> String {
    format!("result-{id}")
}

/// A search band's "Retry".
pub fn band_retry(band: usize) -> String {
    format!("band-retry-{band}")
}

/// A search band's "Show N more".
pub fn band_more(band: usize) -> String {
    format!("more-{band}")
}

// ── Downloads
pub const DOWNLOADS_SEARCH: &str = "dl-search";
/// The name the plan and the walk test use.
pub const DL_SEARCH: &str = DOWNLOADS_SEARCH;
pub const SELECT_ALL: &str = "dl-select-all";
pub const SHOW_ALL: &str = "dl-show-all";
pub const RESULTS_CLEAR: &str = "results-clear";
pub const CLEAR_SELECTION: &str = "dl-clear-selection";
pub const CANCEL_SELECTED: &str = "dl-cancel-selected";
pub const PAUSE_ALL: &str = "pause-all";
pub const CANCEL_ALL: &str = "cancel-all";
pub const RETRY_ALL_FAILED: &str = "retry-all-failed";
pub const RESUME_INTERRUPTED: &str = "interrupted-resume";
pub const DISCARD_INTERRUPTED: &str = "interrupted-discard";
pub const CONFIRM_YES: &str = "confirm-yes";
pub const CONFIRM_NO: &str = "confirm-no";

/// A filter chip.
pub fn filter(filter: Filter) -> String {
    format!("filter-{}", Filter::ALL.iter().position(|f| *f == filter).unwrap_or(0))
}

/// The button that arms a two-step confirm.
pub fn arm(confirm: Confirm) -> &'static str {
    match confirm {
        Confirm::Clear => "clear-completed",
        Confirm::Remove => "dl-remove-selected",
        Confirm::Deactivate => "license-deactivate",
        Confirm::Move => "license-move",
    }
}

/// A download's row.
pub fn row(task_id: &str) -> String {
    format!("row-{task_id}")
}

/// A download's ring.
pub fn ring(task_id: &str) -> String {
    format!("ring-{task_id}")
}

/// The ring of a wait that is not a task (a look-up, a search).
pub const RING_WAITING: &str = "ring-waiting";
/// The ring of every unfinished download together.
pub const RING_OVERALL: &str = "ring-overall";

/// A download's tick.
pub fn row_select(task_id: &str) -> String {
    format!("row-select-{task_id}")
}

/// A download's grip, for the reorder.
pub fn row_grip(task_id: &str) -> String {
    format!("row-grip-{task_id}")
}

/// One of a row's action buttons.
pub fn row_action(act: RowAct, task_id: &str) -> String {
    let name = match act {
        RowAct::Promote => "promote",
        RowAct::Pause => "pause",
        RowAct::Resume => "resume",
        RowAct::Cancel => "cancel",
        RowAct::CopyDetails => "copy",
        RowAct::Fix => "fix",
        RowAct::DownloadAgain => "again",
        RowAct::Retry => "retry",
        RowAct::OpenFile => "open",
        RowAct::ShowInFolder => "folder",
        RowAct::Remove => "remove",
    };
    format!("row-{name}-{task_id}")
}

/// A row's action buttons, in the order of `CONTENT.md` 3.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAct {
    Promote,
    Pause,
    Resume,
    Cancel,
    CopyDetails,
    Fix,
    DownloadAgain,
    Retry,
    OpenFile,
    ShowInFolder,
    Remove,
}

// ── Settings
pub const LAYOUT_CHOOSE: &str = "layout-choose";
pub const CHANGE_FOLDER: &str = "change-folder";
pub const IMPORT_COOKIES: &str = "import-cookies";
pub const CHECK_UPDATE: &str = "check-update";
pub const DOWNLOAD_UPDATE: &str = "download-update";
pub const INSTALL_UPDATE: &str = "install-update";
pub const CHECK_ENGINE: &str = "check-engine";
pub const UPDATE_ENGINE: &str = "update-engine";
pub const SHOW_SHORTCUTS: &str = "show-shortcuts";
pub const GET_SUPPORT: &str = "get-support";

/// A Settings section, by its place in the list.
pub fn section(index: usize) -> String {
    format!("section-{index}")
}

/// A setting's control, by its config key.
pub fn setting(key: &str) -> String {
    format!("set-{key}")
}

/// A Settings text field's editor, by its config key.
/// The lock right after a setting's name when some of its choices need Premium: the one
/// place such a lock stands in every preset (jury ST-5).
pub fn setting_lock(key: &str) -> String {
    format!("set-lock-{key}")
}

pub fn setting_edit(key: &str) -> String {
    format!("set-edit-{key}")
}

/// A palette swatch: 0 is the look's own, 1 to 7 the app's palettes.
pub fn palette(index: usize) -> String {
    format!("set-palette-{index}")
}

/// A theme choice (Light, Dark, System), by its place in the list.
pub fn theme(index: usize) -> String {
    format!("set-theme-{index}")
}

/// A weekday chip of the schedule, 0 Sunday.
pub fn day(index: usize) -> String {
    format!("set-day-{index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `pub const NAME: &str = "value";` of this file, and one sample of each key function.
    #[test]
    fn every_key_is_unique() {
        let mut keys: Vec<String> = include_str!("keys.rs")
            .lines()
            .filter_map(|l| l.strip_prefix("pub const ")?.split_once(": &str = \"")?.1.strip_suffix("\";").map(String::from))
            .collect();
        assert!(keys.len() > 50, "the constants were not found");
        keys.extend([
            result("yt-1"),
            result_tick("yt-1"),
            choice_format("mp3"),
            choice_video_quality("720p"),
            choice_audio_quality("320"),
            setting_edit("outputTemplate"),
            theme(0),
            palette(0),
            setting("theme"),
            track(0),
            row("a"),
            row_select("a"),
            ring("a"),
        ]);
        let mut seen = std::collections::HashSet::new();
        for k in &keys {
            assert!(seen.insert(k), "{k} is used twice");
        }
        assert_eq!(DL_SEARCH, DOWNLOADS_SEARCH);
    }
}
