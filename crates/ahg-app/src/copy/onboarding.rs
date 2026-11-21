//! The first-launch setup's words. Electron has no setup screen: every string here is new
//! (invented, waiting for the owner's approval). A setting shown in it keeps its Settings
//! label word for word (`copy::settings`), so none of those is typed here again.

/// The step indicator: "Step 1 of 3".
pub fn step_of(at: usize, of: usize) -> String {
    format!("Step {at} of {of}") // invented (owner approval pending)
}

/// Each step's title and its line under it.
pub const STEP_LAYOUT: (&str, &str) = (super::PICKER_TITLE, "You can change it any time in Settings."); // invented (owner approval pending)
pub const STEP_LOOK: (&str, &str) = ("Pick a look", "Light or dark, and the colors the app wears."); // invented (owner approval pending)
pub const STEP_ESSENTIALS: (&str, &str) = ("Set your defaults", "Where files go and what a download is saved as."); // invented (owner approval pending)

pub const BACK: &str = "Back"; // invented (owner approval pending)
pub const NEXT: &str = "Next"; // invented (owner approval pending)
pub const FINISH: &str = "Start using the app"; // invented (owner approval pending)
pub const SKIP_ALL: &str = "Skip setup"; // invented (owner approval pending)

/// Esc on the first step asks before it skips.
pub const SKIP_CONFIRM: &str = "Skip setup and keep the current settings?"; // invented (owner approval pending)
pub const SKIP_YES: &str = "Skip"; // invented (owner approval pending)
pub const SKIP_NO: &str = "Keep going"; // invented (owner approval pending)

/// The keys line at the foot.
pub const KEYS_HINT: &str = "Enter continues, Esc goes back"; // invented (owner approval pending)

/// The Settings row that opens the setup again.
pub const RUN_AGAIN: (&str, &str) = ("Setup", "Go through the first-launch choices again"); // invented (owner approval pending)
pub const RUN_AGAIN_BUTTON: &str = "Run setup again"; // invented (owner approval pending)
