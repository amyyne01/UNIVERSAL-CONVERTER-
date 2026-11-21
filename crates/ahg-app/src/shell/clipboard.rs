//! The clipboard watcher (step 62): Electron's `clipboard-watch.ts`. It offers a link and never
//! downloads (K41): an offer goes into `State.clipboard_offer`, for the offer surface (step 68).
//! The decisions (changed text only, seeded at start, non-links ignored) are the engine's
//! `ClipboardWatcher`; this part is its timer and the clipboard read.
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use ahg_engine::clipboard_watch::{ClipboardWatcher, POLL_MS, Tick};
use rune::Tx;

use crate::gate::State;

#[derive(Default)]
pub struct Clipboard {
    watcher: Rc<RefCell<ClipboardWatcher>>,
    /// The next read is due then; None while the setting is off.
    due: Option<Instant>,
}

impl Clipboard {
    /// One look at the setting and the clock. Off: stopped, and the clipboard is never read.
    /// Turned on: the text now on the clipboard is taken as seen (so a link copied before is not
    /// offered), then every `POLL_MS` the text is read and judged. `tx.wake_after` keeps the
    /// cadence while the window is hidden.
    fn step(&mut self, now: Instant, enabled: bool, tx: &mut Tx) {
        let every = Duration::from_millis(POLL_MS);
        let w = self.watcher.clone();
        if !enabled {
            if self.due.take().is_some() {
                w.borrow_mut().stop();
            }
        } else if self.due.is_none() {
            self.due = Some(now + every);
            tx.read_clipboard(move |_: &mut State, c, _| {
                w.borrow_mut().start(|| Some(c.text().to_string()));
            });
            tx.wake_after(every);
        } else if self.due.is_some_and(|d| now >= d) {
            self.due = Some(now + every);
            tx.read_clipboard(move |s: &mut State, c, _| {
                if let Tick::Offer(detection) = w.borrow_mut().tick(true, || Some(c.text().to_string())) {
                    s.clipboard_offer = Some(detection);
                }
            });
            tx.wake_after(every);
        }
    }
}

impl super::Part for Clipboard {
    fn name(&self) -> &'static str {
        "clipboard"
    }

    fn poll(&mut self, s: &mut State, tx: &mut Tx) -> bool {
        let enabled = s.settings.get("clipboardWatch").and_then(|v| v.as_bool()).unwrap_or(false);
        let had = s.clipboard_offer.is_some();
        self.step(Instant::now(), enabled, tx);
        had != s.clipboard_offer.is_some()
    }

    fn stop(&mut self) {
        self.watcher.borrow_mut().stop();
        self.due = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";

    /// A runtime whose clipboard is the in-memory one; drives `step` as the app's poll does.
    struct Rig {
        rt: rune::Runtime<State>,
        board: rune::clipboard::MemoryClipboard,
        clip: Clipboard,
        now: Instant,
    }

    impl Rig {
        fn new() -> Self {
            let mut rt = rune::App::new(crate::spec()).headless_runtime((1240.0, 820.0), rune::Env { system_dark: false, scale: 1.0 });
            let board = rune::clipboard::memory();
            rt.use_clipboard(&board);
            crate::splash::skip(&mut rt.state);
            rt.frame(0.0);
            Rig { rt, board, clip: Clipboard::default(), now: Instant::now() }
        }

        /// One poll `ms` later, with the setting as given; the clipboard read answers inside `update`.
        fn poll(&mut self, ms: u64, enabled: bool) {
            self.now += Duration::from_millis(ms);
            let (clip, now) = (&mut self.clip, self.now);
            self.rt.update(|_, tx| clip.step(now, enabled, tx));
            self.rt.frame(0.0);
        }

        fn offer(&self) -> Option<String> {
            self.rt.state.clipboard_offer.as_ref().map(|d| d.url.clone())
        }
    }

    #[test]
    fn a_link_copied_before_turning_it_on_is_not_offered() {
        let mut r = Rig::new();
        r.board.set_text(LINK);
        r.poll(0, true); // seed
        r.poll(POLL_MS, true);
        assert_eq!(r.offer(), None);
    }

    #[test]
    fn a_new_link_is_offered_once_and_the_same_link_again_is_not() {
        let mut r = Rig::new();
        r.poll(0, true);
        r.board.set_text(LINK);
        r.poll(100, true);
        assert_eq!(r.offer(), None, "not due yet");
        r.poll(POLL_MS, true);
        assert_eq!(r.offer().as_deref(), Some(LINK));
        r.rt.state.clipboard_offer = None; // the offer surface dismissed it
        r.poll(POLL_MS, true);
        assert_eq!(r.offer(), None, "the same text offers nothing again");
        r.board.set_text("some words");
        r.poll(POLL_MS, true);
        r.board.set_text(LINK);
        r.poll(POLL_MS, true);
        assert_eq!(r.offer().as_deref(), Some(LINK), "changed back: offered again");
    }

    #[test]
    fn text_that_is_not_a_link_is_ignored_and_nothing_downloads() {
        let mut r = Rig::new();
        r.poll(0, true);
        r.board.set_text("buy milk");
        r.poll(POLL_MS, true);
        assert_eq!(r.offer(), None);
        let tasks = r.rt.state.tasks.len();
        r.board.set_text(LINK);
        r.poll(POLL_MS, true);
        assert!(r.offer().is_some());
        assert_eq!(r.rt.state.tasks.len(), tasks, "the watcher never queues a download (K41)");
    }

    #[test]
    fn disabled_means_no_read_and_turning_it_on_again_reseeds() {
        let mut r = Rig::new();
        r.board.set_text("seed");
        r.poll(0, false);
        r.board.set_text(LINK);
        r.poll(POLL_MS, false);
        assert_eq!(r.offer(), None);
        assert!(r.clip.due.is_none(), "stopped, no read scheduled");
        r.poll(POLL_MS, true); // on again: the link on the clipboard now is taken as seen
        r.poll(POLL_MS, true);
        assert_eq!(r.offer(), None);
    }
}
