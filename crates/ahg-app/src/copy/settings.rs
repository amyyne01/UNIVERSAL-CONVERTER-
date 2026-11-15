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
/// Names "platform tab", which the new navigation no longer has: the owner rewords it.
pub const DEFAULTS_HINT: &str = "What a new download starts with. Each platform tab can still override them.";
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
pub const CLIPBOARD_WATCHER: (&str, &str) = ("Clipboard watcher", "Offer a one-click download whenever you copy a link, even outside the app");
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
pub const AUTO_UPDATE_ENGINE: (&str, &str) = ("Auto-update engine", "Keep yt-dlp current, and repair it when a site change breaks downloads");

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
pub const MOVE_ARMED: &str = "This PC deactivates now. The key becomes claimable on the new machine. The next transfer is allowed after 7 days.";
pub const MOVE_CONFIRM: &str = "Confirm move";

// ── Help
pub const HELP: &str = "Help";
pub const SHORTCUTS: (&str, &str) = ("Keyboard shortcuts", "Ctrl+K opens the command palette; the full list is one click away");
pub const SHOW_SHORTCUTS: &str = "Show shortcuts";
pub const REPORT: (&str, &str) = ("Report a problem", "Say what you pasted and the message you saw. If asked, attach ahg.log, found in the app's folder under %APPDATA%.");
pub const GET_SUPPORT: &str = "Get support";
