//! When things may show, and in which order chains run (MOTION-SYSTEM 5.3, 5.4, 5.6).
//! Pure: the clock is passed in, deadlines are values the caller polls (`next_deadline` / `advance`).

use crate::gate::{Part, State, is_loading};
use crate::motion::{LOADING_DELAY, LOADING_MIN, RING_HOLD, STALL_AFTER, WAIT_STILL_GOING, WAIT_STILL_WORKING};
use ahg_engine::types::DownloadStatus;

/// What a loading part may draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    Nothing,
    Bones,
    Content,
}

/// The loading rule. `started`: the wait began; `answered`: the answer came;
/// `shown_at`: the commit that put bones up (None when they never showed).
pub fn show(now: f64, started: f64, answered: Option<f64>, shown_at: Option<f64>) -> Show {
    let bones_due = started + LOADING_DELAY;
    if answered.is_none_or(|a| now < a) {
        return if shown_at.is_some() || now >= bones_due { Show::Bones } else { Show::Nothing };
    }
    // Answered past the delay with no recorded commit: the bones went up at the delay.
    let shown = shown_at.or(answered.filter(|&a| a > bones_due).map(|_| bones_due));
    match shown {
        Some(t) if now < t + LOADING_MIN => Show::Bones,
        _ => Show::Content,
    }
}

/// What a preset asks. Today `State` has no wait timestamps, so a loading part is drawn as if
/// its wait were already past `LOADING_DELAY` (the sample scenes); history and statistics never get bones.
pub fn shows(s: &State, part: Part) -> Show {
    match (is_loading(s, part), part) {
        (false, _) => Show::Content,
        (true, Part::History | Part::Stats) => Show::Nothing,
        (true, _) => Show::Bones,
    }
}

/// Something a deadline wakes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Due {
    /// A wait reached `LOADING_DELAY`: bones may show.
    LoadingShow,
    /// Bones have stayed `LOADING_MIN`: content may replace them.
    LoadingFree,
    /// A downloading task may have stalled: sync the rings again.
    Stall,
    /// A slow wait's line: 1 at `WAIT_STILL_WORKING`, 2 at `WAIT_STILL_GOING`.
    WaitLine(u8),
    /// A finished task's ring has held `RING_HOLD`: land it.
    RingLanded(String),
}

/// Pending deadlines, in seconds on the caller's clock.
#[derive(Debug, Default)]
pub struct Timeline {
    due: Vec<(f64, Due)>,
}

impl Timeline {
    pub fn at(&mut self, deadline: f64, due: Due) {
        self.due.push((deadline, due));
    }

    /// Drops every pending deadline for `due`.
    pub fn cancel(&mut self, due: &Due) {
        self.due.retain(|(_, d)| d != due);
    }

    pub fn next_deadline(&self) -> Option<f64> {
        self.due.iter().map(|(t, _)| *t).reduce(f64::min)
    }

    /// Everything due by `now`, earliest first (ties in the order they were added), removed.
    pub fn advance(&mut self, now: f64) -> Vec<Due> {
        let (mut ready, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.due).into_iter().partition(|(t, _)| *t <= now);
        self.due = rest;
        ready.sort_by(|a, b| a.0.total_cmp(&b.0)); // stable: ties keep insertion order
        ready.into_iter().map(|(_, d)| d).collect()
    }
}

/// What a wait that starts at `now` schedules: the bones, then the two wait lines.
pub fn waiting(now: f64) -> Vec<(f64, Due)> {
    vec![(now + LOADING_DELAY, Due::LoadingShow), (now + WAIT_STILL_WORKING, Due::WaitLine(1)), (now + WAIT_STILL_GOING, Due::WaitLine(2))]
}

/// The deadlines an answer cancels.
pub const ANSWERED: &[Due] = &[Due::LoadingShow, Due::WaitLine(1), Due::WaitLine(2)];

/// What an answer at `now` schedules: when bones show, their release at `shown_at + LOADING_MIN`.
pub fn answered(now: f64, shown_at: Option<f64>) -> Vec<(f64, Due)> {
    shown_at.map(|t| t + LOADING_MIN).filter(|&free| now < free).map(|free| (free, Due::LoadingFree)).into_iter().collect()
}

/// The finish and fail chains for one drained batch: whether its commit is animated, and what to
/// schedule. Finish: the commit puts up `Pass::Done`, then the ring lands after `RING_HOLD`.
/// Fail: `Pass::Failed` and the move to "needs attention" in that one commit, no hold.
pub fn chain(now: f64, finished: &[String], failed: &[String]) -> (bool, Vec<(f64, Due)>) {
    let lands = finished.iter().map(|id| (now + RING_HOLD, Due::RingLanded(id.clone()))).collect();
    (!finished.is_empty() || !failed.is_empty(), lands)
}

/// A downloading task with no changed report since `seen_at` counts as stalled (D47).
pub fn stalled(seen_at: f64, now: f64) -> bool {
    now >= seen_at + STALL_AFTER
}

/// The earliest moment a downloading task that is not stalled yet becomes stalled.
pub fn next_stall(tasks_seen: &[(&str, DownloadStatus, f64)], now: f64) -> Option<f64> {
    tasks_seen
        .iter()
        .filter(|(_, status, seen_at)| *status == DownloadStatus::Downloading && !stalled(*seen_at, now))
        .map(|(_, _, seen_at)| seen_at + STALL_AFTER)
        .reduce(f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: f64 = 10.0;

    #[test]
    fn show_follows_every_row_of_the_table() {
        let d = LOADING_DELAY;
        let m = LOADING_MIN;
        // (now, started, answered, shown_at, expected, row)
        let cases: &[(f64, f64, Option<f64>, Option<f64>, Show, &str)] = &[
            // Answer before LOADING_DELAY: Nothing, then Content; bones never shown.
            (T0, T0, None, None, Show::Nothing, "just started"),
            (T0 + 0.1, T0, None, None, Show::Nothing, "waiting, before the delay"),
            (T0 + 0.1, T0, Some(T0 + 0.1), None, Show::Content, "answered at 0.1 s"),
            (T0 + 5.0, T0, Some(T0 + 0.1), None, Show::Content, "long after an early answer"),
            (T0 + d, T0, Some(T0 + d), None, Show::Content, "answered exactly at the delay"),
            // Still waiting at LOADING_DELAY: Bones from then.
            (T0 + d, T0, None, None, Show::Bones, "exactly at the delay"),
            (T0 + 3.0, T0, None, None, Show::Bones, "still waiting"),
            (T0 + 3.0, T0, None, Some(T0 + d), Show::Bones, "still waiting, bones up"),
            // Answer while bones show: Bones until shown_at + LOADING_MIN, then Content.
            (T0 + d + 0.1, T0, Some(T0 + d + 0.05), Some(T0 + d), Show::Bones, "answered, bones held"),
            (T0 + d + m - 0.01, T0, Some(T0 + d + 0.05), Some(T0 + d), Show::Bones, "just before the minimum"),
            (T0 + d + m, T0, Some(T0 + d + 0.05), Some(T0 + d), Show::Content, "exactly at the minimum"),
            (T0 + 9.0, T0, Some(T0 + 8.0), Some(T0 + d), Show::Content, "answered after the minimum"),
            // The bones' commit came late: the minimum counts from shown_at.
            (T0 + 0.8, T0, Some(T0 + 0.5), Some(T0 + 0.5), Show::Bones, "late commit, held"),
            (T0 + 1.0, T0, Some(T0 + 0.5), Some(T0 + 0.5), Show::Content, "late commit, released"),
            // Answered past the delay with no recorded commit: bones counted from the delay.
            (T0 + d + 0.1, T0, Some(T0 + d + 0.05), None, Show::Bones, "derived shown_at, held"),
            (T0 + d + m, T0, Some(T0 + d + 0.05), None, Show::Content, "derived shown_at, released"),
        ];
        assert!(!cases.is_empty());
        for &(now, started, answered, shown_at, want, row) in cases {
            assert_eq!(show(now, started, answered, shown_at), want, "{row}");
        }
    }

    #[test]
    fn history_and_statistics_never_get_bones() {
        let mut s = State::sample(0);
        s.loading = crate::gate::Loading::Booting;
        for part in [Part::History, Part::Stats] {
            assert_eq!(shows(&s, part), Show::Nothing, "{part:?} while booting");
        }
        s.loading = crate::gate::Loading::Ready;
        for part in [Part::History, Part::Stats, Part::Lookup] {
            assert_eq!(shows(&s, part), Show::Content, "{part:?} when ready");
        }
    }

    #[test]
    fn a_running_lookup_shows_bones() {
        let mut s = State::sample(0);
        s.found = crate::gate::Found::LookingUp;
        assert_eq!(shows(&s, Part::Lookup), Show::Bones);
    }

    #[test]
    fn advance_returns_in_time_order_ties_in_insertion_order_once() {
        let mut tl = Timeline::default();
        assert_eq!(tl.next_deadline(), None);
        tl.at(3.0, Due::Stall);
        tl.at(1.0, Due::WaitLine(1));
        tl.at(2.0, Due::RingLanded("a".into()));
        tl.at(2.0, Due::RingLanded("b".into()));
        assert_eq!(tl.next_deadline(), Some(1.0));
        assert_eq!(tl.advance(0.5), vec![]);
        assert_eq!(tl.advance(2.0), vec![Due::WaitLine(1), Due::RingLanded("a".into()), Due::RingLanded("b".into())]);
        assert_eq!(tl.advance(2.0), vec![], "a returned deadline is gone");
        assert_eq!(tl.next_deadline(), Some(3.0));
        // A deadline already past when advanced comes back once.
        assert_eq!(tl.advance(100.0), vec![Due::Stall]);
        assert_eq!(tl.advance(100.0), vec![]);
        assert_eq!(tl.next_deadline(), None);
    }

    #[test]
    fn cancel_drops_only_that_due() {
        let mut tl = Timeline::default();
        tl.at(1.0, Due::RingLanded("a".into()));
        tl.at(2.0, Due::RingLanded("b".into()));
        tl.at(3.0, Due::RingLanded("a".into()));
        tl.cancel(&Due::RingLanded("a".into()));
        assert_eq!(tl.advance(10.0), vec![Due::RingLanded("b".into())]);
    }

    fn schedule(tl: &mut Timeline, list: Vec<(f64, Due)>) {
        for (at, due) in list {
            tl.at(at, due);
        }
    }

    #[test]
    fn the_wait_lines_are_two_deadlines_cancelled_by_the_answer() {
        let mut tl = Timeline::default();
        schedule(&mut tl, waiting(T0));
        assert_eq!(tl.advance(T0 + LOADING_DELAY), vec![Due::LoadingShow]);
        assert_eq!(tl.advance(T0 + WAIT_STILL_WORKING), vec![Due::WaitLine(1)]);
        assert_eq!(tl.next_deadline(), Some(T0 + WAIT_STILL_GOING));
        // The answer comes at 6 s, bones up since the delay: line 2 never fires.
        for due in ANSWERED {
            tl.cancel(due);
        }
        schedule(&mut tl, answered(T0 + 6.0, Some(T0 + LOADING_DELAY)));
        assert_eq!(tl.next_deadline(), None, "bones held past their minimum: nothing left");

        // An early answer cancels everything, bones included.
        let mut tl = Timeline::default();
        schedule(&mut tl, waiting(T0));
        for due in ANSWERED {
            tl.cancel(due);
        }
        schedule(&mut tl, answered(T0 + 0.1, None));
        assert_eq!(tl.next_deadline(), None);
    }

    #[test]
    fn an_answer_while_bones_show_frees_them_after_the_minimum() {
        let shown = T0 + LOADING_DELAY;
        assert_eq!(answered(shown + 0.1, Some(shown)), vec![(shown + LOADING_MIN, Due::LoadingFree)]);
        assert_eq!(answered(shown + LOADING_MIN, Some(shown)), vec![], "exactly at the minimum: content now");
    }

    #[test]
    fn stall_deadline_arithmetic() {
        assert!(!stalled(T0, T0 + STALL_AFTER - 0.01));
        assert!(stalled(T0, T0 + STALL_AFTER), "exactly at STALL_AFTER");
        let tasks = [
            ("a", DownloadStatus::Downloading, T0 + 2.0),
            ("b", DownloadStatus::Downloading, T0 + 1.0),
            ("c", DownloadStatus::Converting, T0),
            ("d", DownloadStatus::Queued, T0),
        ];
        assert_eq!(next_stall(&tasks, T0 + 1.0), Some(T0 + 1.0 + STALL_AFTER));
        // b is stalled exactly at its deadline; the next is a's.
        assert_eq!(next_stall(&tasks, T0 + 1.0 + STALL_AFTER), Some(T0 + 2.0 + STALL_AFTER));
        assert_eq!(next_stall(&tasks, T0 + 2.0 + STALL_AFTER), None, "all stalled: no deadline");
        assert_eq!(next_stall(&[], T0), None);
    }

    #[test]
    fn the_finish_chain_holds_then_lands() {
        let (animated, list) = chain(T0, &["a".into()], &[]);
        assert!(animated, "the commit that puts up Pass::Done is animated");
        let mut tl = Timeline::default();
        schedule(&mut tl, list);
        assert_eq!(tl.advance(T0 + RING_HOLD - 0.01), vec![], "the ring holds");
        assert_eq!(tl.advance(T0 + RING_HOLD), vec![Due::RingLanded("a".into())]);
    }

    #[test]
    fn the_fail_chain_is_one_animated_commit_with_no_hold() {
        assert_eq!(chain(T0, &[], &["a".into()]), (true, vec![]));
        assert_eq!(chain(T0, &[], &[]), (false, vec![]), "nothing finished or failed: no cause");
    }
}
