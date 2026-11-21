//! The shell: what the app does outside its window (tray, global hotkey,
//! clipboard watcher, toasts, taskbar bar, Discord presence, scheduled PC
//! shutdown, the app updater), and its lifetime.
//!
//! Each subsystem is one [`Part`] in its own file; `main.rs` calls [`install`],
//! [`poll`] and [`quit`] once, so a later step fills only its own file (K16).
//! The shell lives on the UI thread, where Rune runs `poll` and the hooks; a
//! headless run (`--snap`, `--tree`, tests) has none.
use std::cell::RefCell;

use rune::{AppHandle, Tx};

use crate::gate::State;

mod clipboard;
mod hotkey;
mod power;
mod presence;
mod taskbar;
mod toast;
mod tray;
mod update;
// Windows API calls of the app side (step 65), empty until then.
#[cfg(windows)]
mod win;

/// One subsystem of the shell. Every hook does nothing until its step fills it.
pub trait Part {
    /// What the tests know it by.
    #[cfg_attr(not(test), allow(dead_code))]
    fn name(&self) -> &'static str;
    /// Once, before the window opens.
    fn start(&mut self, _app: &AppHandle<State>) {}
    /// Each time `poll` runs, also while the window is hidden; true when it changed what the view reads.
    fn poll(&mut self, _s: &mut State, _tx: &mut Tx) -> bool {
        false
    }
    /// Once, at quit, after the core has stopped.
    fn stop(&mut self) {}
}

pub struct Shell {
    parts: Vec<Box<dyn Part>>,
}

impl Shell {
    /// Every part, in start order. They stop in reverse, which puts the four Electron
    /// has first in its order (`main.ts:370-374`): hotkey, presence, clipboard watcher, tray.
    fn new() -> Self {
        Self::with(vec![
            Box::new(update::Update),
            Box::new(power::Power),
            Box::new(taskbar::Taskbar),
            Box::new(toast::Toast),
            Box::new(tray::Tray),
            Box::new(clipboard::Clipboard),
            Box::new(presence::Presence),
            Box::new(hotkey::Hotkey),
        ])
    }

    fn with(parts: Vec<Box<dyn Part>>) -> Self {
        Self { parts }
    }

    fn start(&mut self, app: &AppHandle<State>) {
        for part in &mut self.parts {
            part.start(app);
        }
    }

    fn poll(&mut self, s: &mut State, tx: &mut Tx) -> bool {
        self.parts.iter_mut().fold(false, |changed, part| part.poll(s, tx) | changed)
    }

    fn stop(mut self) {
        for part in self.parts.iter_mut().rev() {
            part.stop();
        }
    }
}

thread_local! {
    static SHELL: RefCell<Option<Shell>> = const { RefCell::new(None) };
    static APP: RefCell<Option<AppHandle<State>>> = const { RefCell::new(None) };
}

/// Whether this launch opens a window. One process only: `claim` (Rune's
/// `single_instance`, a per-user named pipe) answers false when another process
/// runs, after it handed it this launch, which brings that window forward; this
/// one's arguments are ignored. A headless run (`--snap`, `--tree`) runs beside it
/// and never claims, so it never hands its arguments over.
pub fn opens_window(headless: bool, claim: impl FnOnce() -> bool) -> bool {
    headless || claim()
}

/// Starts every part, unless the run is headless.
pub fn install(app: &AppHandle<State>) {
    if rune::is_headless() {
        return;
    }
    let mut shell = Shell::new();
    shell.start(app);
    SHELL.set(Some(shell));
    APP.set(Some(app.clone()));
}

/// The parts' share of `AppSpec::poll`.
pub fn poll(s: &mut State, tx: &mut Tx) -> bool {
    SHELL.with_borrow_mut(|shell| shell.as_mut().is_some_and(|shell| shell.poll(s, tx)))
}

/// Brings the window forward, shown and focused.
pub fn show_window() {
    APP.with_borrow(|app| {
        if let Some(app) = app {
            app.open();
        }
    });
}

/// The quit list (`main.ts:366`): the core first (`Core::shutdown` saves the settings,
/// stops every download and its yt-dlp and ffmpeg (D2), flushes the history and stops the
/// scheduler), then the parts. Runs from both `on_close` and `on_quit`, because closing the
/// window runs only `on_close` (Rune skips `on_quit` for a window already closed); the
/// second call does nothing.
pub fn quit(s: &mut State) {
    quit_with(SHELL.take(), || s.live = None);
}

fn quit_with(shell: Option<Shell>, stop_core: impl FnOnce()) {
    stop_core();
    if let Some(shell) = shell {
        shell.stop();
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;

    type Log = Rc<RefCell<Vec<String>>>;

    /// A part that writes what was asked of it into the log.
    struct Standin(&'static str, Log);

    impl Part for Standin {
        fn name(&self) -> &'static str {
            self.0
        }
        fn poll(&mut self, _: &mut State, _: &mut Tx) -> bool {
            self.1.borrow_mut().push(format!("poll {}", self.0));
            self.0 == "b"
        }
        fn stop(&mut self) {
            self.1.borrow_mut().push(format!("stop {}", self.0));
        }
    }

    fn shell(log: &Log) -> Shell {
        Shell::with(["a", "b", "c"].into_iter().map(|n| Box::new(Standin(n, log.clone())) as Box<dyn Part>).collect())
    }

    #[test]
    fn every_subsystem_has_a_part_and_none_twice() {
        let names: Vec<_> = Shell::new().parts.iter().map(|p| p.name()).collect();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "{names:?}");
        for want in ["tray", "hotkey", "clipboard", "toast", "taskbar", "presence", "power", "update"] {
            assert!(names.contains(&want), "no {want} part in {names:?}");
        }
        assert!(!names.contains(&"links"), "ahg:// links were removed (owner, 2026-11-21)");
    }

    #[test]
    fn the_parts_stop_in_electrons_order() {
        let stops: Vec<_> = Shell::new().parts.iter().rev().map(|p| p.name()).take(4).collect();
        assert_eq!(stops, ["hotkey", "presence", "clipboard", "tray"]);
    }

    #[test]
    fn quit_stops_the_core_first_then_the_parts_in_reverse() {
        let log: Log = Rc::default();
        let core = log.clone();
        quit_with(Some(shell(&log)), move || core.borrow_mut().push("core".into()));
        assert_eq!(*log.borrow(), ["core", "stop c", "stop b", "stop a"]);
    }

    /// Close and quit both run `quit`; the second call stops nothing again.
    fn runtime() -> rune::Runtime<State> {
        let mut rt = rune::App::new(crate::spec()).headless_runtime((1240.0, 820.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.frame(0.0);
        rt
    }

    #[test]
    fn closing_quits_once_through_the_real_hooks() {
        let mut rt = runtime();
        let log: Log = Rc::default();
        SHELL.set(Some(shell(&log)));
        rt.update(crate::on_close);
        rt.update(crate::on_quit);
        assert_eq!(*log.borrow(), ["stop c", "stop b", "stop a"]);
        assert!(rt.state.live.is_none());
        assert!(SHELL.with_borrow(Option::is_none), "the shell is gone after the quit");
    }

    #[test]
    fn a_second_launch_opens_no_window_and_a_headless_run_never_claims() {
        assert!(opens_window(false, || true), "the first process opens its window");
        assert!(!opens_window(false, || false), "a second launch exits");
        assert!(opens_window(true, || panic!("a headless run claimed the instance")));
    }

    #[test]
    fn poll_asks_every_part_and_reports_any_change() {
        let log: Log = Rc::default();
        let mut rt = runtime();
        let mut shell = shell(&log);
        let mut changed = false;
        rt.update(|s, tx| changed = shell.poll(s, tx));
        assert!(changed);
        assert_eq!(*log.borrow(), ["poll a", "poll b", "poll c"]);
    }
}
