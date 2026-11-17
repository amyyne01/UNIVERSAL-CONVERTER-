//! The Settings screen's wording, word for word from the Electron app
//! (`src/components/tabs/SettingsTab.tsx` at 1.6.1). Every preset's Settings
//! uses these; do not reword them and do not type a label that is here.
//! A row is (label, description).

pub const TITLE: &str = "Settings";
pub const SAVES: &str = "Changes save automatically";
pub const LOADING: &str = "Loading your settings…";
pub const CANCEL: &str = "Cancel";

// ── Destination (no heading in the old app)
pub const SAVING_TO: &str = "Saving to";
pub const CHANGE: &str = "Change";

// ── Download defaults
pub const DEFAULTS: &str = "Download defaults";
// OD-13: placeholder until the owner words it (Electron: SettingsTab.tsx:452 names the "platform tab", which the new navigation lacks)
pub const DEFAULTS_HINT: &str = "Each download can still override them.";
pub const AUDIO_FORMAT: &str = "Audio format";
pub const AUDIO_QUALITY: &str = "Audio quality";
pub const VIDEO_QUALITY: &str = "Video quality";
pub const SKIP_EXISTING: (&str, &str) = ("Skip existing", "Don't re-download files already present on disk");
pub const EMBED_THUMBNAIL: (&str, &str) = ("Embed thumbnail", "Write cover art into the audio file");
pub const EMBED_METADATA: (&str, &str) = ("Embed metadata", "Write title, artist, and album tags");
pub const SIMULTANEOUS: (&str, &str) = ("Simultaneous downloads", "How many downloads run at the same time. The rest wait their turn.");

// ── More download options
pub const MORE: &str = "More download options";
pub const MORE_HINT: &str = "Subtitles, chapters, sponsor segments, container and file naming";
pub const SUBTITLES: &str = "Subtitles";
pub const SUBTITLE_LANGUAGES: &str = "Subtitle languages";
pub const SUBTITLE_LANGUAGES_PLACEHOLDER: &str = "en,fr";
pub const SPONSOR_SEGMENTS: &str = "Sponsor segments";
pub const VIDEO_CONTAINER: &str = "Video container";
pub const VIDEO_CODEC: &str = "Video codec";
pub const SIGN_IN_WITH: &str = "Sign in with";
pub const IMPORT_COOKIES: (&str, &str) = ("Import cookies.txt", "For sites your browser is signed in to but cannot be read directly");
pub const CHOOSE_FILE: &str = "Choose file";
pub const COOKIES_IMPORTED: &str = "Cookies imported. Age-gated and private links can now be fetched.";
pub const AUTO_CAPTIONS: (&str, &str) = ("Auto-generated captions", "Use machine-written captions when a video has no real ones");
pub const EMBED_CHAPTERS: (&str, &str) = ("Embed chapters", "Keep the video's chapter marks in the file");
pub const SPLIT_BY_CHAPTER: (&str, &str) = ("Split by chapter", "Write one file per chapter instead of one long file");
pub const SAVE_COVER_ART: (&str, &str) = ("Save cover art", "Also write the thumbnail as a separate image");
pub const FILE_NAMING: (&str, &str) = ("File naming", "yt-dlp template, e.g. %(uploader)s/%(title)s. Leave empty for the title.");
pub const FILE_NAMING_PLACEHOLDER: &str = "%(title)s";

// ── Appearance & behavior
pub const APPEARANCE: &str = "Appearance & behavior";
pub const THEME: (&str, &str) = ("Theme", "Light, dark, or follow the system preference");
pub const THEMES: [&str; 3] = ["Light", "Dark", "System"];
pub const PALETTE: (&str, &str) = ("Palette", "Accent and surface colors");
pub const PALETTE_ARIA: &str = "Color palette";
pub const CLIPBOARD_WATCHER: (&str, &str) =
    ("Clipboard watcher", "Offer a one-click download whenever you copy a link, even outside the app");
pub const NOTIFICATIONS: (&str, &str) = ("Desktop notifications", "Notify when a download completes or fails");
pub const DISCORD: (&str, &str) = ("Discord Rich Presence", "Show what you're downloading in your Discord status");
/// New in the Rust app (the old one has a single layout): opens the preset picker.
pub const LAYOUT: (&str, &str) = ("Layout", "How the app is arranged and how it looks");
pub const LAYOUT_CHOOSE: &str = "Choose";

// ── Scheduled downloads
pub const SCHEDULE: &str = "Scheduled downloads";
pub const SCHEDULE_HINT: &str = "Start downloads automatically at a set time";
pub const SCHEDULE_HINT_LOCKED: &str = "Premium: runs unattended at a set time on chosen days";
pub const ENABLE_SCHEDULER: &str = "Enable scheduler";
pub const START_TIME: (&str, &str) = ("Start time", "Local time the schedule starts");
pub const ACTIVE_DAYS: (&str, &str) = ("Active days", "Days of the week the schedule is active");
pub const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
pub const SHUTDOWN: (&str, &str) = ("Shutdown after queue", "Power off the PC when scheduled downloads finish");
pub const SHUTDOWN_ON: &str = "This PC will power off when scheduled downloads finish";

// ── Software
pub const SOFTWARE: &str = "Software";
pub const SOFTWARE_HINT: &str = "Two moving parts: the app itself, and the yt-dlp engine that does the downloading.";
pub const APP_NAME: &str = "AHG Universal Converter";
pub const APP_REST: &str = "Check for a new version of AHG Universal Converter";
pub const APP_CHECKING: &str = "Checking for updates…";
pub const APP_DOWNLOADING: &str = "Downloading update…";
pub const APP_FAILED: &str = "Update check failed. Try again";
pub const CHECK_FOR_UPDATES: &str = "Check for updates";
pub const UPDATE_DOWNLOAD: &str = "Download";
pub const RESTART_AND_INSTALL: &str = "Restart & install";
pub const ENGINE_NAME: &str = "Download engine";
pub const ENGINE_CHECK: &str = "Check for engine update";
pub const ENGINE_UPDATE: &str = "Update engine";
pub const ENGINE_CHECKING: &str = "Checking for a newer yt-dlp…";
pub const ENGINE_DOWNLOADING: &str = "Downloading and verifying the engine…";
pub const ENGINE_FAILED: &str = "Engine check failed. Try again";
pub const AUTO_UPDATE_ENGINE: (&str, &str) =
    ("Auto-update engine", "Keep yt-dlp current, and repair it when a site change breaks downloads");

// ── Advanced
pub const ADVANCED: &str = "Advanced";
pub const ADVANCED_HINT: &str = "Network and shortcut settings most people never touch";
pub const RATE_LIMIT: (&str, &str) = ("Rate limit", "Bandwidth cap per download, such as 2M or 500K. Leave empty for no limit");
pub const RATE_LIMIT_PLACEHOLDER: &str = "e.g. 2M";
pub const PROXY: (&str, &str) = ("Proxy", "HTTP/SOCKS proxy for all outbound requests");
pub const PROXY_PLACEHOLDER: &str = "http://host:port";
pub const GLOBAL_HOTKEY: (&str, &str) = ("Global hotkey", "Shortcut that brings this window to the front from anywhere");
pub const GLOBAL_HOTKEY_PLACEHOLDER: &str = "e.g. Ctrl+Shift+D";

// ── Plan & license
pub const PLAN: &str = "Plan & license";
pub const YOUR_PLAN: &str = "Your plan";
pub const PLAN_PREMIUM: (&str, &str) = ("Premium", "Every feature is included on this machine");
pub const PLAN_BASIC: &str = "Basic";
pub const SEE_PLANS: &str = "See plans";
pub const DEACTIVATE: (&str, &str) = ("Deactivate license", "Release this machine so you can activate on another device");
pub const DEACTIVATE_ARMED: &str = "This machine loses Premium immediately. Reactivating needs the key again.";
pub const DEACTIVATE_CONFIRM: &str = "Confirm deactivate";
pub const MOVE: (&str, &str) = ("Move to another PC", "Deactivate here and release the key so it can be activated on a different computer");
pub const MOVE_ARMED: &str =
    "This PC deactivates now. The key becomes claimable on the new machine. The next transfer is allowed after 7 days.";
pub const MOVE_CONFIRM: &str = "Confirm move";

// ── Help
pub const HELP: &str = "Help";
pub const SHORTCUTS: (&str, &str) = ("Keyboard shortcuts", "Ctrl+K opens the command palette; the full list is one click away");
pub const SHOW_SHORTCUTS: &str = "Show shortcuts";
pub const REPORT: (&str, &str) = (
    "Report a problem",
    "Say what you pasted and the message you saw. If asked, attach ahg.log, found in the app's folder under %APPDATA%.",
);
pub const GET_SUPPORT: &str = "Get support";

// ── Needs review (the Core::set_hold_unconfirmed switch, under Download defaults)
// OD-13: placeholder until the owner supplies the wording
pub const NEEDS_REVIEW: (&str, &str) =
    ("Needs review", "Hold a Spotify song until you confirm the audio match, if the app is only guessing.");

// ── Option labels of every select in the Settings tab
// Electron `src/constants.ts`: AUDIO_FORMATS 25-32, AUDIO_QUALITIES 45-51 and VIDEO_QUALITIES 55-63 are
// `copy::found::{AUDIO_FORMATS, AUDIO_QUALITIES, VIDEO_QUALITIES}`; THEMES is above.
/// constants.ts:68-73 (SUBTITLE_MODE_OPTIONS)
pub const SUBTITLE_MODES: [&str; 4] = ["Off", "Embed in the file", "Separate .srt", "Both"];
/// constants.ts:75-79 (SPONSORBLOCK_OPTIONS)
pub const SPONSOR_MODES: [&str; 3] = ["Off", "Mark as chapters", "Cut them out"];
/// constants.ts:81-85 (VIDEO_CONTAINER_OPTIONS)
pub const CONTAINERS: [&str; 3] = ["MP4 (most compatible)", "MKV (keeps everything)", "WebM"];
/// constants.ts:87-92 (VIDEO_CODEC_OPTIONS)
pub const CODECS: [&str; 4] = ["Best available", "H.264 (plays anywhere)", "VP9 (smaller)", "AV1 (smallest)"];
/// constants.ts:101-110 (COOKIE_BROWSER_OPTIONS), the first is the "no cookies" choice
pub const COOKIE_BROWSERS: [&str; 8] = ["Don't use cookies", "Chrome", "Edge", "Firefox", "Brave", "Chromium", "Opera", "Vivaldi"];
/// constants.ts:98 (CONCURRENCY_OPTIONS, `1 at a time`, `2 at a time` ...)
pub fn concurrency_option(slots: usize) -> String {
    format!("{slots} at a time")
}
/// SettingsTab.tsx:74 (DAYS, the day chips; the full names are `DAYS` above, SettingsTab.tsx:75-77)
pub const DAYS_SHORT: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

// ── Settings: the rest of the screen's states
/// SettingsTab.tsx:336-337, under `LOADING` when the file could not be read
pub const LOADING_HINT: &str =
    "If this doesn’t clear, your settings file couldn’t be read. Restart the app. Your downloads and license are unaffected.";
/// SettingsTab.tsx:1029 (the title of the machine id line)
pub const MACHINE_ID_TITLE: &str = "What a key binds to on this computer";
/// SettingsTab.tsx:1030
pub const MACHINE_ID: &str = "Machine ID";
/// SettingsTab.tsx:1018-1019; `date` is the local date of the end of the cooldown
pub fn cooldown_note(date: &str) -> String {
    format!("Transfer unavailable until {date} — a license can be moved to another PC once every 7 days.")
}

// ── Software: status lines (SettingsTab.tsx:360-381)
pub const ENGINE_IDLE: &str = "yt-dlp powers every download"; // :366
pub const ENGINE_UP_TO_DATE: &str = "yt-dlp is up to date"; // :364
pub const APP_UP_TO_DATE: &str = "You're on the latest version"; // :380
/// :361
pub fn engine_available(latest: &str) -> String {
    format!("yt-dlp {latest} is available")
}
/// :363
pub fn engine_updated(version: &str) -> String {
    format!("Engine updated to {version}")
}
/// :377
pub fn app_available(version: &str) -> String {
    format!("v{version} is available")
}
/// :379
pub fn app_ready(version: &str) -> String {
    format!("v{version} ready, restart to install")
}

// ── Plan & license: Basic sentences and locks
/// SettingsTab.tsx:531-533 (the Simultaneous downloads description on Basic; `lib/tier.ts:82`)
pub const SIMULTANEOUS_BASIC: &str = "Basic: 2 downloads at once. The rest wait their turn.";
/// SettingsTab.tsx:967 with `lib/tier.ts:85` (the plan card on Basic)
pub const PLAN_BASIC_DESC: &str = "Included in Basic: 1080p video · 320 kbps audio · 5-link batches · 20 tracks per playlist. Premium lifts these limits with a one-time purchase.";
/// `lib/tier.ts:52-58`: what a locked control says when it is tapped on Basic
pub const LOCK_SCHEDULER: &str = "Scheduled downloads are a Premium feature.";
pub const LOCK_SUBTITLE_LANGUAGES: &str = "Multiple subtitle languages are a Premium feature.";
pub const LOCK_SUBTITLE_AUTO: &str = "Auto-generated captions are a Premium feature.";
pub const LOCK_SPLIT_CHAPTERS: &str = "Splitting by chapter is a Premium feature.";
pub const LOCK_CONCURRENCY: &str = "More than 2 downloads at once is a Premium feature.";
pub const LOCK_COOKIES: &str = "Signing in with your browser is a Premium feature.";

// ── The update prompt (`src/components/UpdateModal.tsx`)
pub const UPDATE_TITLE_READY: &str = "Update ready to install"; // :157
pub const UPDATE_TITLE_DOWNLOADING: &str = "Downloading the update…"; // :157
pub const UPDATE_TITLE_AVAILABLE: &str = "A new version is available"; // :157
pub const UPDATE_LATER: &str = "Not now"; // :190 (when the update is ready)
pub const UPDATE_LATER_AVAILABLE: &str = "Later"; // :190
/// :161; `version` is shown in the mono face
pub const UPDATE_BODY_READY: (&str, &str) = ("Version ", " is downloaded. Restarting takes a few seconds and puts you on it.");
/// :163
pub const UPDATE_BODY_DOWNLOADING: (&str, &str) =
    ("Version ", " is on its way. You can keep using the app — this finishes in the background.");
/// :164
pub const UPDATE_BODY_AVAILABLE: (&str, &str) = ("Version ", " is out. Downloading it now means the next launch is already up to date.");

// ── The upgrade sheet and activation (`src/components/UpgradeSheet.tsx`, `src/constants.ts` PLAN_FEATURES)
pub const UPGRADE_ARIA: &str = "Upgrade to Premium"; // :135
pub const UPGRADE_TITLE_PREMIUM: &str = "You’re on Premium"; // :152
pub const UPGRADE_TITLE_BASIC: &str = "One purchase removes every limit"; // :152
pub const UPGRADE_LEAD_PREMIUM: &str = "Every feature below is included on this machine. Thank you for buying it."; // :156
pub const UPGRADE_LEAD_BASIC: &str = "AHG Universal Converter is fully usable for free. Premium lifts the limits with a one-time purchase."; // :157
pub const UPGRADE_PRICE: &str = "$3.99"; // :19
pub const UPGRADE_BASIC_BLURB: &str = "What you have right now."; // :169
pub const UPGRADE_BASIC_PRICE: &str = "Free"; // :170
pub const UPGRADE_BASIC_PRICE_NOTE: &str = "no account, no key"; // :171
pub const UPGRADE_PREMIUM_BLURB: &str = "Everything, on this machine, forever."; // :181
pub const UPGRADE_PREMIUM_PRICE_NOTE: &str = "one-time purchase"; // :183
pub const UPGRADE_RAIL_PREMIUM: &str = "Pay once"; // :391
pub const UPGRADE_RAIL_BASIC: &str = "Free, no account"; // :391
pub const UPGRADE_CURRENT: &str = "Current"; // :394
pub const UPGRADE_CURRENT_PLAN: &str = "Your current plan"; // :441
pub const UPGRADE_INCLUDED: &str = "Included in Premium"; // :441
pub const UPGRADE_ACTIVE_HERE: &str = "Active on this machine"; // :199
pub const UPGRADE_ACTIVE: &str = "Premium active"; // :214
pub const UPGRADE_GET_KEY_LEAD: &str = "Get a key for "; // :223, followed by `UPGRADE_PRICE`
pub const UPGRADE_HAVE_KEY: &str = "I already have a key"; // :232
pub const UPGRADE_KEY_NOTE: &str =
    "The key arrives by message after you buy. It is checked online once; after that it keeps working offline for up to 30 days."; // :250-251
pub const UPGRADE_KEY_PLACEHOLDER: &str = "Paste your key…"; // :269
pub const UPGRADE_KEY_ARIA: &str = "License key"; // :270
pub const UPGRADE_CLIPBOARD_BLOCKED: &str = "Clipboard access blocked. Paste the key manually."; // :290
pub const UPGRADE_VERIFYING: &str = "Verifying…"; // :318
pub const UPGRADE_TRY_AGAIN: &str = "Try again"; // :318
pub const UPGRADE_ACTIVATE: &str = "Activate Premium"; // :318
pub const UPGRADE_KEY_REFUSED: &str = "That key was not accepted."; // :84, when the reply has no message
pub const UPGRADE_KEY_REFUSED_TAIL: &str = " Check it for typos, then try again, or ask on Discord."; // :84, after the message
pub const UPGRADE_NETWORK_ERROR: &str = "Network error. Check your connection and try again."; // :88
pub const UPGRADE_ASK: &str = "Questions? Ask on Discord"; // :335
pub const UPGRADE_FOOTER: &str = "One key, one machine. Move it to another PC from Settings."; // :338
/// :217; `labels` are the lower-cased `UPGRADE_FEATURE_LABELS`
pub fn upgrade_now_without_limits(labels: &str) -> String {
    format!("Now without limits: {labels}.")
}
/// :365
pub fn upgrade_plan_aria(title: &str) -> String {
    format!("{title} plan")
}
/// constants.ts:37-73, PLAN_FEATURES (label, Basic value, Premium value)
pub const UPGRADE_FEATURES: [(&str, &str, &str); 5] = [
    ("Video quality", "Up to 1080p", "Up to 4K, or best available"),
    ("Batch paste", "5 links at a time", "As many links as you paste"),
    ("Playlists & albums", "First 20 tracks", "Every track"),
    ("Lossless audio", "Lossy formats only", "FLAC, WAV and ALAC"),
    ("Scheduled downloads", "Not included", "Run unattended, on your days"),
];
