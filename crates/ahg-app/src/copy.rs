//! Every user-visible string of the Home and Downloads screens and the shell
//! around them. The strings are the Electron app's, word for word (curly
//! apostrophes, middle dots, ellipses and dashes included). Do not reword them.
//!
//! `docs/design-gate/CONTENT.md` says where each string is used, when it shows
//! and what it does. Names here follow the order of that file.

// ── Shell

pub const BRAND: &str = "AHG";
pub const BRAND_REST: &str = " Universal Converter";
pub const SEARCH_CAPSULE: &str = "Search";
pub const SEARCH_CAPSULE_ARIA: &str = "Search or run a command";
pub const KEY_CTRL: &str = "Ctrl";
pub const KEY_K: &str = "K";
pub const WINDOW_MINIMIZE: &str = "Minimize";
pub const WINDOW_MAXIMIZE: &str = "Maximize";
pub const WINDOW_RESTORE: &str = "Restore";
pub const WINDOW_CLOSE: &str = "Close";
pub const SKIP_TO_CONTENT: &str = "Skip to content";

pub const RAIL_ARIA: &str = "Tabs";
pub const NAV_HOME: &str = "Home";
pub const NAV_DOWNLOADS: &str = "Downloads";
pub const NAV_YOUTUBE: &str = "YouTube";
pub const NAV_SPOTIFY: &str = "Spotify";
pub const NAV_SOUNDCLOUD: &str = "SoundCloud";
pub const NAV_REELS: &str = "Reels & Shorts";
pub const NAV_REELS_RAIL_CAPTION: &str = "Reels";
pub const NAV_SETTINGS: &str = "Settings";

/// Rail item tooltip, e.g. `Home (Ctrl+1)`. `position` is 1-based.
pub fn nav_tooltip(label: &str, position: usize) -> String {
    format!("{label} (Ctrl+{position})")
}

// Source names printed on a Downloads row's detail line.
pub const SOURCE_YOUTUBE: &str = "YouTube";
pub const SOURCE_SPOTIFY: &str = "Spotify";
pub const SOURCE_SOUNDCLOUD: &str = "SoundCloud";
pub const SOURCE_INSTAGRAM: &str = "Reels";
pub const SOURCE_TIKTOK: &str = "TikTok";
pub const SOURCE_FACEBOOK: &str = "Facebook";
pub const SOURCE_TWITTER: &str = "X / Twitter";
pub const SOURCE_VIMEO: &str = "Vimeo";
pub const SOURCE_TWITCH: &str = "Twitch";
pub const SOURCE_BANDCAMP: &str = "Bandcamp";
pub const SOURCE_REDDIT: &str = "Reddit";
pub const SOURCE_DAILYMOTION: &str = "Dailymotion";
pub const SOURCE_MIXCLOUD: &str = "Mixcloud";
pub const SOURCE_RUMBLE: &str = "Rumble";
pub const SOURCE_DIRECT: &str = "Direct";
pub const SOURCE_GENERIC: &str = "Link";
pub const SOURCE_UNKNOWN: &str = "Unknown";

// Keyboard shortcuts sheet (Ctrl+/).
pub const SHORTCUTS_ARIA: &str = "Keyboard shortcuts";
pub const SHORTCUTS_TITLE: &str = "Keyboard shortcuts";
pub const SHORTCUT_PALETTE: &str = "Open the command palette";
// OD-13: placeholder until the owner words it (Electron ShortcutsSheet.tsx:11: "Switch tab (the order of the left rail)")
pub const SHORTCUT_TABS: &str = "Go to Home, Downloads, Settings";
pub const SHORTCUT_PASTE: &str = "Paste a link from Home and look it up";
pub const SHORTCUT_ENTER: &str = "Look up the link or search in the field";
pub const SHORTCUT_MOVE: &str = "Move between results";
pub const SHORTCUT_SELECT: &str = "Select a result; Enter downloads it";
pub const SHORTCUT_SELECT_ALL: &str = "Select every result";
pub const SHORTCUT_CLOSE: &str = "Close a sheet or the palette";
pub const SHORTCUT_SHOW: &str = "Show this list";
pub const SHORTCUT_HOTKEY: &str = "Bring this window to the front from anywhere";
pub const KEYCAP_ENTER: &str = "Enter";
pub const KEYCAP_ARROWS: [&str; 2] = ["↑", "↓"];
pub const KEYCAP_SPACE: &str = "Space";
pub const KEYCAP_A: &str = "A";
pub const KEYCAP_ESC: &str = "Esc";
pub const KEYCAP_SLASH: &str = "/";

/// Keycap for the tab shortcut row, e.g. `1…7`.
pub fn keycap_tab_range(tab_count: usize) -> String {
    format!("1…{tab_count}")
}

// Command palette (Ctrl+K).
pub const PALETTE_ARIA: &str = "Command palette";
pub const PALETTE_PLACEHOLDER: &str = "Command or link";
pub const PALETTE_SEARCH_ARIA: &str = "Command palette search";
pub const PALETTE_ESC_CAP: &str = "ESC";
pub const PALETTE_GROUP_NAVIGATE: &str = "Navigate";
pub const PALETTE_GROUP_ACTIONS: &str = "Actions";
pub const PALETTE_NO_MATCH: &str = "No commands match";
pub const PALETTE_FOOTER_NAVIGATE_CAP: &str = "↑↓";
pub const PALETTE_FOOTER_NAVIGATE: &str = " navigate";
pub const PALETTE_FOOTER_SELECT_CAP: &str = "↵";
pub const PALETTE_FOOTER_SELECT: &str = " select";
pub const PALETTE_FOOTER_CLOSE_CAP: &str = "esc";
pub const PALETTE_FOOTER_CLOSE: &str = " close";
pub const PALETTE_PASTE_AND_DOWNLOAD: &str = "Paste link & download";
pub const PALETTE_SWITCH_TO_DARK: &str = "Switch to dark mode";
pub const PALETTE_SWITCH_TO_LIGHT: &str = "Switch to light mode";
pub const PALETTE_CHECK_UPDATES: &str = "Check for updates";
pub const PALETTE_CLEAR_COMPLETED: &str = "Clear completed downloads";
pub const PALETTE_PAUSE_ALL: &str = "Pause all downloads";
pub const PALETTE_RESUME_ALL: &str = "Resume all downloads";
pub const PALETTE_CHANGE_FOLDER: &str = "Change download folder";
pub const PALETTE_SHORTCUTS: &str = "Keyboard shortcuts";
pub const PALETTE_CANCEL_ALL: &str = "Cancel all downloads";
pub const PALETTE_CLIPBOARD_EMPTY: &str = "Your clipboard is empty. Copy a link first.";

/// Palette navigation entry, e.g. `Go to Home`.
pub fn palette_go_to(label: &str) -> String {
    format!("Go to {label}")
}

// Transient popup.
pub const NOTICE_DISMISS: &str = "Dismiss";
pub const NOTICE_SEE_PLANS: &str = "See plans";

// ── Home

pub const HOME_HEADLINE: &str = "Paste a link. Get the file.";
pub const HOME_IN_FLIGHT_SUFFIX: &str = " in flight";

pub const FIELD_PLACEHOLDER: &str = "Paste a link or search";
pub const FIELD_ARIA: &str = "Paste a link";
pub const FIELD_BUTTON_DOWNLOAD: &str = "Download";
pub const FIELD_BUTTON_SEARCH: &str = "Search";
pub const FIELD_BUTTON_LOOK_UP: &str = "Look up";

/// Chip inside the field when several links are pasted.
pub fn link_count_chip(count: usize) -> String {
    format!("{count} links")
}

// Status line.
pub const STATUS_DROP: &str = "Drop to add every link";
pub const STATUS_READING: &str = "Reading the link…";
pub const STATUS_SEARCHING: &str = "Searching YouTube, SoundCloud and Spotify…";
pub const STATUS_NOT_A_LINK: &str = "Not a link. Press Enter to search YouTube, SoundCloud and Spotify for it";
pub const STATUS_SEE_PLANS: &str = "See plans";

pub fn status_adding(count: usize) -> String {
    format!("Adding {count} links…")
}

pub fn status_links_ready(count: usize) -> String {
    format!("{count} links ready. Press Enter to download them all")
}

// Rest-state caption.
pub const REST_CHANGE: &str = "Change";
pub const REST_CHANGE_ARIA: &str = "Change output folder and format";
pub const REST_KEY_V: &str = "V";
pub const REST_PLUS_SPACED: &str = " + ";
pub const REST_PASTES_A_LINK: &str = " pastes a link";
pub const REST_HOTKEY_LEAD: &str = ". ";
pub const REST_HOTKEY_TRAIL: &str = " grabs one from any app";

/// Left part of the rest caption, before the folder name. `format` is the saved
/// audio format; pass it as stored and it is upper-cased here (`mp3` prints `MP3`).
pub fn rest_saves_as(format: &str) -> String {
    format!("Audio saves as {}, video as MP4, in ", format.to_uppercase())
}

// Errors on Home.
pub const ERR_NOT_SUPPORTED: &str = "This site isn’t supported. Paste a link from YouTube, Spotify, SoundCloud, Vimeo and more.";
pub const ERR_NO_LINKS: &str = "No downloadable links found. Check what you pasted and try again.";

// Basic-plan limit.
pub const BASIC_LIMIT_BATCH: &str = "Basic limit reached: 5 links at a time. Premium removes the limit.";
pub const BASIC_LIMIT_COLLECTION: &str = "Basic limit reached: 20 tracks per playlist. Premium removes the limit.";

// "Works with" row (first launch).
pub const WORKS_WITH: &str = "Works with";
pub const WORKS_WITH_YOUTUBE: &str = "YouTube";
pub const WORKS_WITH_SPOTIFY: &str = "Spotify";
pub const WORKS_WITH_SOUNDCLOUD: &str = "SoundCloud";
pub const WORKS_WITH_REELS: &str = "Reels & Shorts";
pub const WORKS_WITH_TAIL: &str = "and most other video and music sites.";

// Bento cell titles and asides.
pub const CELL_SPEED: &str = "Speed";
pub const CELL_THIS_MONTH: &str = "This month";
pub const CELL_WHERE_FROM: &str = "Where it came from";
pub const CELL_RECENT: &str = "Recent downloads";
pub const CELL_RECENT_VIEW_ALL: &str = "View all";

// Speed cell.
pub fn speed_now(formatted_bytes: &str) -> String {
    format!("{formatted_bytes}/s")
}

pub fn speed_scale(formatted_bytes: &str) -> String {
    format!("scale {formatted_bytes}/s")
}

pub fn speed_aria(formatted_bytes: &str) -> String {
    format!("Download speed, {formatted_bytes} per second")
}

// This month cell.
pub const MONTH_FAILED: &str = "This month’s count couldn’t be read. Your downloads are unaffected.";
pub const MONTH_LOADING_SR: &str = "Counting this month’s files…";
pub const MONTH_NOTHING_YET: &str = "Nothing downloaded yet this month";
pub const MONTH_NOTHING_YET_NO_PREV: &str = "Paste a link above; the count starts with your first file.";
pub const MONTH_FIRST_ON_RECORD: &str = " on disk. First month on record, so there is nothing to compare against yet.";
pub const FILE_ONE: &str = "file";
pub const FILE_MANY: &str = "files";

/// `file` for 1, `files` otherwise.
pub fn file_word(count: u64) -> &'static str {
    if count == 1 { FILE_ONE } else { FILE_MANY }
}

/// Second line when this month is empty and last month was not.
/// `formatted_bytes` is the output of `formatBytes` for last month's bytes.
pub fn month_last_month_kept(prev_files: u64, formatted_bytes: &str) -> String {
    format!("Last month you kept {prev_files} {} ({formatted_bytes}).", file_word(prev_files))
}

/// Sentence after the bytes figure when the counts are equal.
pub fn month_same_count(prev_month_name: &str) -> String {
    format!(" on disk, the same count as {prev_month_name}.")
}

/// Sentence after the bytes figure; `diff` is this month minus last month, not zero.
pub fn month_diff(diff: i64, prev_month_name: &str) -> String {
    let word = if diff > 0 { "more" } else { "fewer" };
    format!(" on disk, {} {word} than {prev_month_name}.", diff.unsigned_abs())
}

pub fn month_bars_aria(cur_name: &str, cur_files: u64, prev_name: &str, prev_files: u64) -> String {
    format!("Files by month: {cur_name} {cur_files}, {prev_name} {prev_files}")
}

// Where it came from cell.
pub const SPLIT_FAILED: &str = "The source split isn’t available right now";
pub const SPLIT_LOADING_SR: &str = "Counting where this month’s files came from…";
pub const SPLIT_EMPTY: &str = "Once you have downloads, this shows which sources they came from.";
pub const SPLIT_OTHER: &str = "Other";
pub const SPLIT_YOUTUBE: &str = "YouTube";
pub const SPLIT_SPOTIFY: &str = "Spotify";
pub const SPLIT_SOUNDCLOUD: &str = "SoundCloud";
pub const SPLIT_REELS: &str = "Reels & Shorts";

pub fn split_aside(total: u64) -> String {
    format!("of {total} files")
}

/// Right side of a source row: `63% · 5`. Percent is `round(count / total * 100)`.
pub fn split_row_figures(pct: u64, count: u64) -> String {
    format!("{pct}% · {count}")
}

// Recent tiles.
pub const RECENT_UNTITLED: &str = "Untitled";
pub const RECENT_OPEN: &str = "Open";
pub const RECENT_DETAILS: &str = "Details";

pub fn recent_open_aria(title: &str) -> String {
    format!("Open the folder holding “{title}”")
}

pub fn recent_details_aria(title: &str) -> String {
    format!("Show “{title}” in Downloads")
}

/// Poster caption, e.g. `MP3 · 3.4 MB` or `MP3 · 3.4 MB · Artist`.
pub fn recent_poster_meta(format: &str, formatted_bytes: &str, uploader: &str) -> String {
    let base = format!("{} · {formatted_bytes}", format.to_uppercase());
    if uploader.is_empty() { base } else { format!("{base} · {uploader}") }
}

// Home search results.
pub const SEARCH_BAND_YOUTUBE: &str = "YouTube";
pub const SEARCH_BAND_SOUNDCLOUD: &str = "SoundCloud";
pub const SEARCH_BAND_SPOTIFY: &str = "Spotify";
pub const SEARCH_SPOTIFY_NOTE: &str = "matched to best audio source";
pub const SEARCH_RETRY: &str = "Retry";
pub const SEARCH_TRY_AGAIN: &str = "Try again";
pub const SEARCH_ROW_DOWNLOAD: &str = "Download";
pub const SEARCH_ROW_ADDED: &str = "Added";
pub const SEARCH_VIEW_QUEUE: &str = "View queue";
pub const SEARCH_NOTHING_HINT: &str = "Check the spelling, or paste a link directly";
pub const SEARCH_ALL_FAILED: &str = "The search couldn’t reach YouTube, SoundCloud or Spotify";

pub fn search_show_more(hidden: usize) -> String {
    format!("Show {hidden} more")
}

pub fn search_no_matches(source: &str) -> String {
    format!("No matches on {source}.")
}

pub fn search_source_failed(source: &str) -> String {
    format!("{source} search failed")
}

pub fn search_nothing_matched(query: &str) -> String {
    format!("Nothing matched “{query}” on YouTube, SoundCloud or Spotify")
}

pub fn search_row_aria(title: &str, subtitle: &str) -> String {
    if subtitle.is_empty() { format!("Download {title}") } else { format!("Download {title}, {subtitle}") }
}

pub fn search_region_aria(query: &str) -> String {
    format!("Search results for “{query}”")
}

// ── Downloads

pub const DL_TITLE: &str = "Downloads";
pub const DL_ACTIVE_WORD: &str = " active";
pub const DL_PAUSED_SUFFIX: &str = " · paused";
pub const DL_TOTAL_WORD: &str = " total";
pub const DL_SEARCH_ARIA: &str = "Search downloads";
pub const DL_SEARCH_PLACEHOLDER: &str = "Search title, uploader or site";
pub const DL_RETRY_ALL_FAILED: &str = "Retry all failed";
pub const DL_CLEAR_COMPLETED: &str = "Clear completed";
pub const DL_CLEAR_CONFIRM_ARIA: &str = "Confirm clear completed";
pub const DL_PAUSE_ALL: &str = "Pause all";
pub const DL_RESUME_ALL: &str = "Resume all";
pub const DL_CANCEL_ALL: &str = "Cancel all";
pub const DL_CONFIRM_CANCEL: &str = "Cancel";
pub const DL_CONFIRM_YES: &str = "Confirm";

/// The header counts line, e.g. `2 active · 5 total` or `1 active · paused · 5 total`.
pub fn active_and_total(active: usize, total: usize, all_paused: bool) -> String {
    let paused = if all_paused { DL_PAUSED_SUFFIX } else { "" };
    format!("{active}{DL_ACTIVE_WORD}{paused} · {total}{DL_TOTAL_WORD}")
}

/// Summary line under the heading, e.g. `7 downloads: 1 downloading, 6 failed`.
/// `parts` are already `{n} {word}` strings in the fixed order, zero counts left out.
pub fn downloads_summary(total: usize, parts: &[String]) -> String {
    format!("{total} downloads: {}", parts.join(", "))
}

/// The word used for a status in the summary line (`cancelled` is spelled `canceled`).
pub const SUMMARY_WORDS: [&str; 7] = ["downloading", "retrying", "queued", "paused", "done", "failed", "canceled"];

pub fn clear_confirm(count: usize) -> String {
    format!("Clear {count}?")
}

// Control strip.
pub const DL_SELECT_ALL_ARIA: &str = "Toggle select all";
pub const DL_FILTER_ARIA: &str = "Filter downloads";
pub const DL_FILTER_ALL: &str = "All";
pub const DL_FILTER_ACTIVE: &str = "Active";
pub const DL_FILTER_DONE: &str = "Done";
pub const DL_FILTER_FAILED: &str = "Failed";
pub const DL_SELECTION_CANCEL: &str = "Cancel";
pub const DL_SELECTION_REMOVE: &str = "Remove";
pub const DL_SELECTION_CLEAR: &str = "Clear selection";
pub const DL_REMOVE_CONFIRM_ARIA: &str = "Confirm remove selected";

pub fn selected_count(count: usize) -> String {
    format!("{count} selected")
}

pub fn remove_confirm(count: usize) -> String {
    format!("Remove {count}?")
}

// Interrupted banner.
pub const DL_INTERRUPTED_RESUME: &str = "Resume";
pub const DL_INTERRUPTED_DISCARD: &str = "Discard";

/// Text after the bold count, e.g. ` download interrupted when the app closed. Finish it now?`.
pub fn interrupted_sentence(count: usize) -> String {
    let (noun, pronoun) = if count == 1 { ("download", "it") } else { ("downloads", "them") };
    format!(" {noun} interrupted when the app closed. Finish {pronoun} now?")
}

// Group headers.
pub const GROUP_IN_PROGRESS: &str = "In progress";
pub const GROUP_NEEDS_ATTENTION: &str = "Needs attention";
pub const GROUP_COMPLETED: &str = "Completed";

pub fn up_next(count: usize) -> String {
    format!("Up next · {count}")
}

/// Accessible name of the group's transfer total; `speed` is empty when unknown.
pub fn transferring_aria(live: usize, speed: &str) -> String {
    if speed.is_empty() { format!("{live} transferring") } else { format!("{live} transferring, {speed} total") }
}

// Empty states.
pub const EMPTY_NONE_TITLE: &str = "No downloads yet";
pub const EMPTY_NONE_BODY: &str = "Paste a link on Home and its progress shows here";
pub const EMPTY_NONE_ACTION: &str = "Paste a link";
pub const EMPTY_FILTER_ACTIVE: &str = "Nothing is downloading right now";
pub const EMPTY_FILTER_DONE: &str = "No finished downloads yet";
pub const EMPTY_FILTER_FAILED: &str = "No failed or canceled downloads";

/// Body of a filter-matches-nothing row, e.g. `1 download in total.`
pub fn empty_filter_body(total: usize) -> String {
    format!("{total} download{} in total.", if total == 1 { "" } else { "s" })
}

pub fn show_all(total: usize) -> String {
    format!("Show all {total}")
}

// Row: per-status words.
pub const STATE_QUEUED: &str = "Queued";
pub const STATE_STARTING: &str = "Starting";
pub const STATE_CONVERTING: &str = "Converting";
pub const STATE_TAGGING: &str = "Tagging";
pub const STATE_PAUSED: &str = "Paused";
pub const STATE_RETRYING: &str = "Retrying";
pub const STATE_DONE: &str = "Done";
pub const STATE_SKIPPED: &str = "Skipped";
pub const STATE_FAILED: &str = "Failed";
pub const STATE_CANT_DOWNLOAD: &str = "Can’t download";
pub const STATE_CANCELED: &str = "Canceled";
/// Rate line when neither speed nor ETA is known (an en dash).
pub const RATE_UNKNOWN: &str = "–";

// Row: detail line pieces.
pub const ROW_ALREADY_DOWNLOADED: &str = " · Already downloaded, skipped";

/// ` · {file}` appended after the skipped text, or standing alone on done rows.
pub fn row_dot_part(part: &str) -> String {
    format!(" · {part}")
}

/// `Retrying in 5s (1 of 3)`; `k` is `min(retry_count + 1, 3)`.
pub fn row_retrying(seconds_left: u64, retry_count: usize) -> String {
    format!(" · Retrying in {seconds_left}s ({} of 3)", (retry_count + 1).min(3))
}

pub fn row_reorder_aria(label: &str) -> String {
    format!("Reorder {label}")
}

pub fn row_progress_aria(title: &str) -> String {
    let name = if title.is_empty() { "Download" } else { title };
    format!("{name} progress")
}

pub fn row_name_with_link(label: &str, url: &str) -> String {
    format!("{label}, {url}")
}

pub const ROW_SELECT_ARIA: &str = "Select";
pub const ROW_DESELECT_ARIA: &str = "Deselect";

// Row: action labels (each is both the tooltip and the accessible name).
pub const ACT_DO_THIS_NEXT: &str = "Do this next";
pub const ACT_PAUSE: &str = "Pause";
pub const ACT_RESUME: &str = "Resume";
pub const ACT_CANCEL: &str = "Cancel";
pub const ACT_COPY_DETAILS: &str = "Copy details";
pub const ACT_OPEN_SETTINGS: &str = "Open settings";
pub const ACT_UPDATE_ENGINE: &str = "Update engine";
pub const ACT_DOWNLOAD_AGAIN: &str = "Download again";
pub const ACT_RETRY: &str = "Retry";
pub const ACT_OPEN_FILE: &str = "Open file";
pub const ACT_SHOW_IN_FOLDER: &str = "Show in folder";
pub const ACT_REMOVE: &str = "Remove";

// Reorder announcements.
pub const ANNOUNCE_GRABBED: &str = "Grabbed. Use the arrow keys to move, Enter to drop, Escape to cancel.";
pub const ANNOUNCE_DROPPED: &str = "Dropped";
pub const ANNOUNCE_REORDER_CANCELED: &str = "Reorder canceled";

pub fn announce_moved(position: usize, count: usize) -> String {
    format!("Moved to position {position} of {count}")
}

// Toasts and messages raised from Downloads.
pub const TOAST_CANCELED_ONE: &str = "Download canceled";
pub const TOAST_UNDO: &str = "Undo";
pub const TOAST_UPDATING_ENGINE: &str = "Updating the download engine…";
pub const TOAST_ENGINE_FAILED: &str = "Couldn’t update the engine. Try again from Settings.";
pub const TOAST_ENGINE_UPDATED: &str = "Engine updated. Retrying.";
pub const TOAST_ENGINE_CURRENT: &str = "The engine is up to date. Retrying.";
pub const TOAST_LIST_UNREACHABLE: &str = "Couldn’t reach the downloads list. Try again.";
pub const TOAST_SAVED_UNTITLED: &str = "Saved your download";
pub const TOAST_SAVED_OPEN_FILE: &str = "Open file";

pub fn toast_canceled(count: usize) -> String {
    if count == 1 { TOAST_CANCELED_ONE.to_string() } else { format!("{count} downloads canceled") }
}

pub fn toast_saved(title: &str) -> String {
    format!("Saved “{title}”")
}

/// The subject of a file-reveal message: `“{title}”`, or `That file` when no title.
pub fn file_name_subject(title: &str) -> String {
    let t = title.trim();
    if t.is_empty() { "That file".to_string() } else { format!("“{t}”") }
}

pub fn file_never_finished(subject: &str) -> String {
    format!("{subject} has no file on this PC. The download never finished.")
}

pub fn file_moved(subject: &str) -> String {
    format!("{subject} isn’t where it was saved. It was moved, renamed or deleted outside the app.")
}

pub fn file_cannot_open(subject: &str) -> String {
    format!("{subject} couldn’t be opened. It may no longer be in your downloads folder.")
}

// Failed rows: cause and remedy per error kind. `{thing}` is filled by `error_text`.
pub const ERR_PRIVATE_CAUSE: &str = "This {thing} is private.";
pub const ERR_PRIVATE_REMEDY: &str = "Ask the owner for access, or sign in with your browser in Settings, then retry.";
pub const ERR_REMOVED_CAUSE: &str = "We couldn’t find anything at this link.";
pub const ERR_REMOVED_REMEDY: &str = "Check the link, or paste a different one.";
pub const ERR_AGE_RESTRICTED_CAUSE: &str = "This {thing} is age-restricted.";
pub const ERR_AGE_RESTRICTED_REMEDY: &str = "Sign in with your browser in Settings, then retry.";
pub const ERR_LOGIN_REQUIRED_CAUSE: &str = "This site wants you to sign in first.";
pub const ERR_LOGIN_REQUIRED_REMEDY: &str = ERR_AGE_RESTRICTED_REMEDY; // alias: Electron words both the same
pub const ERR_GEO_BLOCKED_CAUSE: &str = "This {thing} isn’t available in your country.";
pub const ERR_GEO_BLOCKED_REMEDY: &str = "Paste a different link, or use a VPN you trust.";
pub const ERR_DRM_CAUSE: &str = "This {thing} is copy-protected.";
pub const ERR_DRM_REMEDY: &str = "It can’t be downloaded. Paste a different link.";
pub const ERR_RATE_LIMITED_CAUSE: &str = "The site is asking us to slow down.";
pub const ERR_RATE_LIMITED_REMEDY: &str = "Wait a few minutes, then retry.";
pub const ERR_NETWORK_CAUSE: &str = "The connection dropped.";
pub const ERR_NETWORK_REMEDY: &str = "Check your internet, then retry.";
pub const ERR_UNSUPPORTED_CAUSE: &str = "This link isn’t something we can download.";
pub const ERR_UNSUPPORTED_REMEDY: &str = "Check the link, or paste one from YouTube, Spotify, SoundCloud, Vimeo and more.";
pub const ERR_FFMPEG_MISSING_CAUSE: &str = "A part of the app needed for conversion is missing.";
pub const ERR_FFMPEG_MISSING_REMEDY: &str = "Download the app again, or restore the files it came with.";
pub const ERR_DISK_FULL_CAUSE: &str = "There is no space left on the drive.";
pub const ERR_DISK_FULL_REMEDY: &str = "Free up some space or pick another folder in Settings, then retry.";
pub const ERR_UNKNOWN_CAUSE: &str = "Something went wrong.";
pub const ERR_UNKNOWN_REMEDY: &str = "Try again in a moment.";

/// Replaces the generic "removed" sentence when the source is a generic site.
pub const ERR_GENERIC_DEAD_END: &str = "We couldn’t find anything at this link. The site may not be supported, or the address is wrong. Supported sites include YouTube, Vimeo, X, TikTok, Twitch, SoundCloud, Bandcamp and Reddit.";
/// Replaces the remedy for a Spotify source whose kind is unknown, removed, unsupported or missing.
// OD-13: placeholder until the owner words it (Electron: "Search for it in the Spotify tab"; there is no Spotify tab now)
pub const ERR_SPOTIFY_REMEDY: &str = "We find Spotify songs by name. Search for it on Home, or paste a different link.";

/// One readable sentence pair: `{cause} {remedy}`, with `{thing}` set to `video` or `link`.
pub fn error_text(cause: &str, remedy: &str, is_video_source: bool) -> String {
    let thing = if is_video_source { "video" } else { "link" };
    format!("{} {remedy}", cause.replace("{thing}", thing))
}

// Number formats (CONTENT.md section 4): one copy, in `format.rs`.

pub fn format_bytes(n: f64) -> String {
    crate::format::bytes(n)
}

pub fn format_duration(secs: f64) -> String {
    crate::format::duration(secs)
}

pub fn format_speed(bps: f64) -> String {
    crate::format::speed(bps)
}

pub fn format_eta(secs: f64) -> String {
    crate::format::eta(secs)
}

/// Rate line under the percentage: speed and ETA joined with ` · `, `–` when neither.
pub fn rate_line(bps: f64, eta_secs: f64) -> String {
    crate::format::rate(bps, eta_secs)
}

// ── Wording the Electron app does not have, typed by the presets until now.
// invented (owner approved 2026-11-16, D43)
pub const KEEP_OPEN: &str = "Keep open"; // invented (owner approved 2026-11-16, D43)
pub const MATCH_MATCHING: &str = "Matching…"; // invented (owner approved 2026-11-16, D43)
pub const MATCH_CONFIRMED: &str = "Confirmed"; // invented (owner approved 2026-11-16, D43)
pub const MATCH_BEST_GUESS: &str = "Best guess"; // invented (owner approved 2026-11-16, D43)
pub const MATCH_NONE: &str = "No match found"; // invented (owner approved 2026-11-16, D43)
pub const COL_MATCH: &str = "Match"; // invented (owner approved 2026-11-16, D43)
pub const LABEL_TIME_LEFT: &str = "Time left"; // invented (owner approved 2026-11-16, D43)
pub const LABEL_SIZE: &str = "Size"; // invented (owner approved 2026-11-16, D43)
pub const PICKER_TITLE: &str = "Choose a layout"; // invented (owner approved 2026-11-16, D43)
pub const PICKER_ESC: &str = "Esc closes"; // invented (owner approved 2026-11-16, D43)
pub const PICKER_NEW_LAYOUT: &str = "New layout"; // invented (owner approved 2026-11-16, D43)
pub const PICKER_CLASSIC_LAYOUT: &str = "Classic layout"; // invented (owner approved 2026-11-16, D43)
pub const PRESET_SIGNAL: &str = "Signal"; // invented (owner approved 2026-11-16, D43)
pub const PRESET_SHELF: &str = "Shelf"; // invented (owner approved 2026-11-16, D43)
pub const PRESET_ORBIT: &str = "Orbit"; // invented (owner approved 2026-11-16, D43)
pub const NOT_IN_BUILD: &str = "This direction is not in this build"; // invented (owner approved 2026-11-16, D43)

// ── Settings: its own file, shared by all six presets.
pub mod settings;
// ── The after-paste step (what the field found), shared by all six presets.
pub mod found;

/// Every `pub const` of the three copy files with the string literals it holds, read from
/// the sources (so a new constant is checked without being listed). A name with several
/// literals (a tuple, an array) appears once per literal.
#[cfg(test)]
mod tests {
    const FILES: [(&str, &str); 3] =
        [("copy", include_str!("copy.rs")), ("found", include_str!("copy/found.rs")), ("settings", include_str!("copy/settings.rs"))];

    /// The same sentence under two names on purpose: (the alias, what it points at).
    /// An alias holds no literal, so it can never be a second copy of the words.
    fn aliases() -> Vec<(&'static str, &'static str)> {
        use super::{found as f, settings as s};
        vec![
            (super::ERR_LOGIN_REQUIRED_REMEDY, super::ERR_AGE_RESTRICTED_REMEDY),
            (super::ERR_NO_LINKS, f::ERROR_BATCH_NONE),
            (super::BASIC_LIMIT_BATCH, f::LIMIT_BATCH),
            (super::BASIC_LIMIT_COLLECTION, f::LIMIT_COLLECTION),
            (super::ERR_PRIVATE_CAUSE, f::ERROR_PRIVATE.0),
            (super::ERR_PRIVATE_REMEDY, f::ERROR_PRIVATE.1),
            (super::ERR_REMOVED_CAUSE, f::ERROR_REMOVED.0),
            (super::ERR_REMOVED_REMEDY, f::ERROR_REMOVED.1),
            (super::ERR_AGE_RESTRICTED_CAUSE, f::ERROR_AGE_RESTRICTED.0),
            (super::ERR_AGE_RESTRICTED_REMEDY, f::ERROR_AGE_RESTRICTED.1),
            (super::ERR_LOGIN_REQUIRED_CAUSE, f::ERROR_LOGIN_REQUIRED.0),
            (super::ERR_LOGIN_REQUIRED_REMEDY, f::ERROR_LOGIN_REQUIRED.1),
            (super::ERR_GEO_BLOCKED_CAUSE, f::ERROR_GEO_BLOCKED.0),
            (super::ERR_GEO_BLOCKED_REMEDY, f::ERROR_GEO_BLOCKED.1),
            (super::ERR_DRM_CAUSE, f::ERROR_DRM.0),
            (super::ERR_DRM_REMEDY, f::ERROR_DRM.1),
            (super::ERR_RATE_LIMITED_CAUSE, f::ERROR_RATE_LIMITED.0),
            (super::ERR_RATE_LIMITED_REMEDY, f::ERROR_RATE_LIMITED.1),
            (super::ERR_NETWORK_CAUSE, f::ERROR_NETWORK.0),
            (super::ERR_NETWORK_REMEDY, f::ERROR_NETWORK.1),
            (super::ERR_UNSUPPORTED_CAUSE, f::ERROR_UNSUPPORTED.0),
            (super::ERR_UNSUPPORTED_REMEDY, f::ERROR_UNSUPPORTED.1),
            (super::ERR_FFMPEG_MISSING_CAUSE, f::ERROR_FFMPEG_MISSING.0),
            (super::ERR_FFMPEG_MISSING_REMEDY, f::ERROR_FFMPEG_MISSING.1),
            (super::ERR_DISK_FULL_CAUSE, f::ERROR_DISK_FULL.0),
            (super::ERR_DISK_FULL_REMEDY, f::ERROR_DISK_FULL.1),
            (super::ERR_UNKNOWN_CAUSE, f::ERROR_UNKNOWN.0),
            (super::ERR_UNKNOWN_REMEDY, f::ERROR_UNKNOWN.1),
            (super::ERR_GENERIC_DEAD_END, f::ERROR_GENERIC_DEAD_END),
            (super::ERR_SPOTIFY_REMEDY, f::ERROR_SPOTIFY_REMEDY),
            (s::CLIPBOARD_WATCHER.0, f::CLIPBOARD_SETTING.0),
            (s::CLIPBOARD_WATCHER.1, f::CLIPBOARD_SETTING.1),
        ]
    }

    /// (file, name, literal) for every `pub const` whose value is typed out.
    fn all() -> Vec<(&'static str, String, String)> {
        let mut out = Vec::new();
        for (file, text) in FILES {
            let mut name = String::new();
            let mut open = false;
            for line in text.lines() {
                let t = line.trim_start();
                if t.starts_with("pub const ") {
                    name = t["pub const ".len()..].split(':').next().unwrap().to_string();
                    open = true;
                }
                if open {
                    // No literal holds an escaped quote; the statement ends at the first `;`
                    // after the line's last literal.
                    let mut rest = line;
                    while let Some(a) = rest.find('"') {
                        let Some(b) = rest[a + 1..].find('"') else { break };
                        out.push((file, name.clone(), rest[a + 1..a + 1 + b].to_string()));
                        rest = &rest[a + b + 2..];
                    }
                    if rest.contains(';') {
                        open = false;
                    }
                }
            }
        }
        out
    }

    #[test]
    fn the_aliases_are_the_same_words() {
        let list = aliases();
        assert!(!list.is_empty());
        for (alias, real) in list {
            assert_eq!(alias, real);
        }
    }

    #[test]
    fn no_sentence_is_typed_twice() {
        let all = all();
        assert!(all.len() > 500, "the parser found only {} literals", all.len());
        // A sentence is four words or more; a one-word label (`Download`, `Cancel`) is a slot of its
        // own on each screen. Nothing is allowed twice: an intended copy is an alias, which holds no literal.
        let mut seen: std::collections::HashMap<&str, (&str, &str)> = std::collections::HashMap::new();
        let mut dupes = Vec::new();
        for (file, name, lit) in &all {
            if lit.split_whitespace().count() < 4 {
                continue;
            }
            if let Some((f2, n2)) = seen.insert(lit, (file, name)) {
                dupes.push(format!("{lit:?}: {f2}::{n2} and {file}::{name}"));
            }
        }
        assert!(dupes.is_empty(), "typed twice:\n{}", dupes.join("\n"));
    }
}
