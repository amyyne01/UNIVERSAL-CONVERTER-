//! The global hotkey (step 62): Electron's `hotkey.ts` and `pasteAndDownload` (`main.ts`).
//! The stored accelerator keeps Electron's format; [`translate`] adapts it to Rune's.
use ahg_engine::shell_model::accelerator_given;
use ahg_engine::text::js_trim;
use ahg_engine::types::SourcePlatform;
use ahg_engine::url_detector::detect_url;
use rune::{AppHandle, Shortcut, ShortcutError, ShortcutId, Tx};

use crate::actions;
use crate::gate::State;

/// Electron's accelerator ("CommandOrControl+Shift+D") as a Rune one; None when Rune cannot take it
/// (what Electron's `register` answers false to).
pub fn translate(accelerator: &str) -> Option<Shortcut> {
    if !accelerator_given(accelerator) {
        return None;
    }
    let parts: Vec<String> = accelerator
        .split('+')
        .map(|p| match p.trim().to_ascii_lowercase().as_str() {
            "commandorcontrol" | "cmdorctrl" => "mod".into(),
            other => other.to_string(),
        })
        .collect();
    parts.join("+").parse().ok()
}

/// The accelerator last asked for, and the registration it got (None: nothing held).
struct Held<I> {
    asked: String,
    id: Option<I>,
}

impl<I> Default for Held<I> {
    fn default() -> Self {
        Self { asked: String::new(), id: None }
    }
}

impl<I> Held<I> {
    /// Follows the setting: unregisters the old accelerator, registers the new one, and does
    /// nothing while it is unchanged (`syncServices`, `main.ts:211`). A failed registration is
    /// silent, as Electron's (its answer is never read); true when one is held afterwards.
    fn sync(&mut self, want: &str, register: impl FnOnce(Shortcut) -> Result<I, ShortcutError>, unregister: impl FnOnce(I)) -> bool {
        if want != self.asked {
            if let Some(id) = self.id.take() {
                unregister(id);
            }
            self.id = translate(want).and_then(|sc| register(sc).ok());
            self.asked = want.to_string();
        }
        self.id.is_some()
    }
}

#[derive(Default)]
pub struct Hotkey {
    app: Option<AppHandle<State>>,
    held: Held<ShortcutId>,
}

impl super::Part for Hotkey {
    fn name(&self) -> &'static str {
        "hotkey"
    }

    fn start(&mut self, app: &AppHandle<State>) {
        self.app = Some(app.clone());
    }

    fn poll(&mut self, s: &mut State, _: &mut Tx) -> bool {
        if let Some(app) = &self.app {
            let want = s.settings.get("globalHotkey").and_then(|v| v.as_str()).unwrap_or("");
            self.held.sync(want, |sc| app.register_shortcut(sc, fired), |id| drop(app.unregister_shortcut(id)));
        }
        false
    }

    fn stop(&mut self) {
        if let (Some(app), Some(id)) = (&self.app, self.held.id.take()) {
            drop(app.unregister_shortcut(id));
        }
    }
}

/// The shortcut fired, wherever the window is: read the clipboard, queue a link, bring the window forward.
fn fired(_: &mut State, tx: &mut Tx) {
    tx.read_clipboard(|s: &mut State, content, _| {
        if paste_link(s, content.text()) {
            super::show_window();
        }
    });
}

/// `pasteAndDownload`: an empty or unknown text does nothing at all (no notice, unlike the
/// palette's command), a Spotify collection needs the preview flow so it is not queued blind;
/// anything else queues with the saved defaults. True when a download was queued.
fn paste_link(s: &mut State, text: &str) -> bool {
    let det = detect_url(js_trim(text));
    if det.platform == SourcePlatform::Unknown || (det.platform == SourcePlatform::Spotify && det.is_collection) {
        return false;
    }
    actions::paste_and_download(s, text);
    true
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// Electron accelerator -> what Rune must parse it to; None where Electron's would fail.
    #[test]
    fn the_translator_table() {
        let cases: [(&str, Option<&str>); 9] = [
            ("CommandOrControl+Shift+D", Some("ctrl+shift+d")),
            ("CmdOrCtrl+Alt+V", Some("ctrl+alt+v")),
            ("Alt+F4", Some("alt+f4")),
            ("Ctrl+Plus", Some("ctrl+equal")),
            ("Control+Shift+Space", Some("ctrl+shift+space")),
            ("Super+Return", Some("super+enter")),
            ("", None),
            ("Ctrl+Shift", None),
            ("nonsense+++", None),
        ];
        for (accel, want) in cases {
            assert_eq!(translate(accel).map(|s| s.to_string()).as_deref(), want, "{accel:?}");
        }
    }

    #[test]
    fn the_default_setting_translates() {
        let d = ahg_engine::config::default_config();
        let accel = d["globalHotkey"].as_str().unwrap();
        assert!(translate(accel).is_some(), "{accel}");
    }

    #[test]
    fn a_change_unregisters_the_old_and_registers_the_new_once() {
        let mut h = Held::<u32>::default();
        let log = RefCell::new(Vec::<String>::new());
        let n = RefCell::new(0);
        let run = |h: &mut Held<u32>, want: &str| {
            h.sync(
                want,
                |sc| {
                    log.borrow_mut().push(format!("reg {sc}"));
                    *n.borrow_mut() += 1;
                    Ok(*n.borrow())
                },
                |id| log.borrow_mut().push(format!("unreg {id}")),
            )
        };
        assert!(run(&mut h, "CommandOrControl+Shift+D"));
        assert!(run(&mut h, "CommandOrControl+Shift+D"), "unchanged: no call");
        assert!(run(&mut h, "Alt+F4"));
        assert!(!run(&mut h, ""), "empty unregisters and registers nothing");
        assert!(!run(&mut h, "Ctrl+Shift"), "invalid: nothing held");
        assert_eq!(*log.borrow(), ["reg ctrl+shift+d", "unreg 1", "reg alt+f4", "unreg 2"]);
    }

    #[test]
    fn a_refused_registration_is_silent_and_holds_nothing() {
        let mut h = Held::<u32>::default();
        assert!(!h.sync("Alt+F4", |_| Err(ShortcutError::Conflict), |_| panic!("nothing to release")));
        assert!(!h.sync("Alt+F4", |_| panic!("not retried while unchanged"), |_| {}));
    }

    #[test]
    fn the_hotkey_queues_a_link_and_ignores_anything_else() {
        let mut s = State::sample(0);
        let before = s.tasks.len();
        assert!(!paste_link(&mut s, "  "));
        assert!(!paste_link(&mut s, "just some words"));
        assert!(s.notices.is_empty() && s.tasks.len() == before, "a miss is silent");
        assert!(paste_link(&mut s, " https://www.youtube.com/watch?v=dQw4w9WgXcQ \n"));
        assert_eq!(s.tasks.len(), before + 1);
    }
}
