//! The launch splash's words: Electron 1.6.1's (`index.html`, `public/splash-boot.js`) word
//! for word, and the few the Rust splash adds for its real loading stages (marked invented).

/// The wordmark, in two weights.
pub const WORD_BRAND: &str = "AHG";
pub const WORD_NAME: &str = "Universal Converter";

/// The looping status line. "They are jokes, so they never claim a boot step that did not
/// happen; the bar is where the honest progress lives."
pub const MESSAGES: [&str; 20] = [
    "Soundchecking the interface",
    "Quantizing the layout grid",
    "Warming up the valves",
    "Adding more cowbell",
    "Waiting for the drop",
    "Clearing the samples",
    "Rolling tape",
    "Auto-tuning the sidebar",
    "Skipping the drum solo",
    "Dropping the needle",
    "Polishing the gold master",
    "Chasing a sync licence",
    "Reading the liner notes",
    "Cueing the next track",
    "Feeding the algorithm",
    "Bouncing the final mix",
    "Sweeping the low end",
    "Rehearsing the transitions",
    "Sequencing the components",
    "Un-clipping the master bus",
];

/// The scramble's glyphs.
pub const GLYPHS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#*/\\<>[]{}=~";

// The loading stages, as the stall line and a failure notice name them. Invented (Rust only).
pub const STAGE_ENGINE: &str = "the download engine";
pub const STAGE_ART: &str = "the artwork";
pub const STAGE_SETTINGS: &str = "your settings";
pub const STAGE_DOWNLOADS: &str = "your downloads";
pub const STAGE_LAYOUT: &str = "the layout";
pub const STAGE_SHELL: &str = "the tray and shortcuts";

/// The status line once loading takes longer than `motion::SPLASH_STALL`. Invented.
pub fn still_loading(what: &str) -> String {
    format!("Still loading {what}")
}

/// The notice when a stage failed or never finished, shown over the app. Invented.
pub fn did_not_load(what: &str, why: &str) -> String {
    format!("Could not load {what}: {why}")
}

/// Why, when a stage was still going at `motion::SPLASH_GIVE_UP`. Invented.
pub const TOOK_TOO_LONG: &str = "it took too long";
