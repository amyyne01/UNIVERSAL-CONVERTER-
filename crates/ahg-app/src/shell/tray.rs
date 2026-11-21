//! The tray icon and its menu (step 61): Electron's `tray.ts`, with the menu itself
//! (`tray_menu`: recent downloads, then the five actions) decided by the engine.
use ahg_engine::shell_model::{MenuEntry, TRAY_TOOLTIP, TrayAction, tray_menu};
use ahg_engine::types::DownloadStatus;
use rune::{AppHandle, Tx};

use crate::actions;
use crate::gate::State;

/// `icon-64.png` as straight RGBA, 64 x 64 (the tray draws it at 16-32 px).
const ICON: &[u8] = include_bytes!("../../assets/tray-64.rgba");
const SIZE: u32 = 64;
/// Electron's recent window (the queue's `RECENT_CAP`).
const RECENT: usize = 8;

#[derive(Default)]
pub struct Tray {
    tray: Option<rune::Tray>,
    shown: Vec<MenuEntry>,
}

impl super::Part for Tray {
    fn name(&self) -> &'static str {
        "tray"
    }

    fn start(&mut self, app: &AppHandle<State>) {
        let h = app.clone();
        self.shown = tray_menu(&[]);
        let spec = rune::TraySpec::new("AHG-Converter", TRAY_TOOLTIP, rune::TrayIcon::rgba(SIZE, SIZE, ICON.to_vec()))
            .tooltip(TRAY_TOOLTIP)
            .menu(items(&self.shown));
        // No tray on this shell is not an error: the app runs without one.
        self.tray = rune::Tray::new(spec, move |e| {
            if let rune::TrayEvent::Menu(id) = e
                && let Some(action) = action_of(&id)
            {
                run(&h, action);
            }
        })
        .ok();
    }

    /// Electron rebuilds the menu on every queue change; here it is set when it differs.
    fn poll(&mut self, s: &mut State, _: &mut Tx) -> bool {
        let menu = tray_menu(&recent(s));
        if let Some(tray) = &self.tray
            && menu != self.shown
        {
            tray.set_menu(items(&menu));
            self.shown = menu;
        }
        false
    }

    /// Dropping the `Tray` takes the icon off the bar.
    fn stop(&mut self) {
        self.tray = None;
    }
}

/// The newest rows first (`State.tasks` keeps the newest at the front), each by title or link.
fn recent(s: &State) -> Vec<(String, DownloadStatus)> {
    s.tasks.iter().take(RECENT).map(|t| (if t.title.is_empty() { t.url.clone() } else { t.title.clone() }, t.progress.status)).collect()
}

const ACTIONS: [(TrayAction, &str); 5] = [
    (TrayAction::PauseAll, "pause-all"),
    (TrayAction::ResumeAll, "resume-all"),
    (TrayAction::ToggleWindow, "toggle-window"),
    (TrayAction::PasteAndDownload, "paste-and-download"),
    (TrayAction::Quit, "quit"),
];

fn id_of(a: TrayAction) -> &'static str {
    ACTIONS.iter().find(|(x, _)| *x == a).map_or("", |(_, id)| id)
}

fn action_of(id: &str) -> Option<TrayAction> {
    ACTIONS.iter().find(|(_, i)| *i == id).map(|(a, _)| *a)
}

/// A menu item's id is its action's; a disabled line gets an id nothing maps back from.
fn items(menu: &[MenuEntry]) -> Vec<rune::TrayItem> {
    menu.iter()
        .enumerate()
        .map(|(n, e)| match e {
            MenuEntry::Separator => rune::TrayItem::Separator,
            MenuEntry::Item { label, enabled, action } => {
                let id = action.map_or_else(|| format!("line-{n}"), |a| id_of(a).to_string());
                rune::TrayItem::action(id, label.clone()).enabled(*enabled)
            }
        })
        .collect()
}

/// Runs on the tray's own thread: the window calls go straight to the handle, the rest
/// to the UI thread.
fn run(h: &AppHandle<State>, action: TrayAction) {
    match action {
        TrayAction::ToggleWindow if h.is_visible() => h.hide(),
        TrayAction::ToggleWindow => h.open(),
        TrayAction::Quit => h.quit(),
        TrayAction::PauseAll => h.update(|s, _| actions::set_all_paused(s, true)),
        TrayAction::ResumeAll => h.update(|s, _| actions::set_all_paused(s, false)),
        TrayAction::PasteAndDownload => h.update(|s, tx| actions::run(s, tx, actions::Command::PasteAndDownload)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_is_64_by_64_rgba() {
        assert_eq!(ICON.len(), (SIZE * SIZE * 4) as usize);
        assert!(ICON.chunks(4).any(|p| p[3] != 0), "the icon is blank");
    }

    #[test]
    fn the_menu_maps_to_five_working_actions_and_never_empty() {
        let menu = items(&tray_menu(&[]));
        assert!(!menu.is_empty(), "an empty tray menu");
        let ids: Vec<_> = menu
            .iter()
            .filter_map(|i| match i {
                rune::TrayItem::Action { id, enabled: true, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        let acts: Vec<_> = ids.iter().filter_map(|id| action_of(id)).collect();
        assert_eq!(
            acts,
            [TrayAction::PauseAll, TrayAction::ResumeAll, TrayAction::ToggleWindow, TrayAction::PasteAndDownload, TrayAction::Quit]
        );
        assert_eq!(ids.len(), 5);
    }

    #[test]
    fn recent_downloads_are_disabled_lines_before_the_controls() {
        let recent = vec![("Song".to_string(), DownloadStatus::Done), ("https://x.test/a".to_string(), DownloadStatus::Queued)];
        let menu = items(&tray_menu(&recent));
        assert!(matches!(&menu[0], rune::TrayItem::Action { label, enabled: false, .. } if label == "✓  Song"));
        assert!(matches!(&menu[1], rune::TrayItem::Action { label, enabled: false, .. } if label == "◷  https://x.test/a"));
        assert_eq!(menu.iter().filter(|i| matches!(i, rune::TrayItem::Action { enabled: true, .. })).count(), 5);
    }

    #[test]
    fn pause_all_and_resume_all_go_one_way_each() {
        let mut s = State::sample(0);
        actions::set_all_paused(&mut s, true);
        assert!(s.tasks.iter().any(|t| t.progress.status == DownloadStatus::Paused));
        actions::set_all_paused(&mut s, true);
        actions::set_all_paused(&mut s, false);
        assert!(s.tasks.iter().all(|t| t.progress.status != DownloadStatus::Paused));
    }

    #[test]
    fn the_state_supplies_at_most_eight_recent_rows() {
        let r = recent(&State::sample(0));
        assert!(!r.is_empty() && r.len() <= RECENT, "{}", r.len());
    }
}
