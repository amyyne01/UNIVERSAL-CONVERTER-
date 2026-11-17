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
pub const LOOK_UP: &str = "look-up";
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

/// An audio format chip in the found step (`mp3`, `flac` ...).
pub fn choice_format(format: &str) -> String {
    format!("choice-format-{format}")
}

/// A playlist row's tick.
pub fn track(index: usize) -> String {
    format!("track-{index}")
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
pub const SELECT_ALL: &str = "dl-select-all";
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

/// A palette swatch: 0 is the look's own, 1 to 7 the app's palettes.
pub fn palette(index: usize) -> String {
    format!("set-palette-{index}")
}

/// A weekday chip of the schedule, 0 Sunday.
pub fn day(index: usize) -> String {
    format!("set-day-{index}")
}
