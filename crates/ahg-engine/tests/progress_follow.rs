//! The value a progress bar shows: it follows the download's real progress
//! smoothly instead of jumping with each report.
use ahg_engine::progress_follow::Follower;

/// Shown value after `frames` frames of `step` milliseconds, starting at `start`.
fn run(follower: &mut Follower, start: u64, frames: u64, step: u64) -> f64 {
    (1..=frames).map(|n| follower.value(start + n * step)).last().unwrap()
}

#[test]
fn the_bar_glides_toward_the_real_progress_and_never_passes_it() {
    let mut f = Follower::new();
    assert_eq!(f.value(0), 0.0);
    f.set_target(40.0, 0);
    let mut last = 0.0;
    for frame in 1..=60 {
        let shown = f.value(frame * 16);
        assert!(shown >= last && shown <= 40.0, "frame {frame}: {shown} after {last}");
        last = shown;
    }
    assert!(f.value(16) < 40.0 || last == 40.0);
    assert_eq!(last, 40.0, "about a second later the bar has arrived");
    assert!(f.is_settled());
}

#[test]
fn the_first_frames_move_but_do_not_jump() {
    let mut f = Follower::new();
    f.set_target(50.0, 0);
    let first = f.value(16);
    assert!(first > 0.0 && first < 10.0, "one frame in: {first}");
    assert!(!f.is_settled());
}

#[test]
fn the_motion_does_not_depend_on_the_frame_rate() {
    let (mut fast, mut slow) = (Follower::new(), Follower::new());
    fast.set_target(80.0, 0);
    slow.set_target(80.0, 0);
    let at_60 = run(&mut fast, 0, 12, 16); // 192 ms in 12 frames
    let at_15 = run(&mut slow, 0, 3, 64); // 192 ms in 3 frames
    assert!((at_60 - at_15).abs() < 0.01, "{at_60} against {at_15}");
}

#[test]
fn a_new_report_is_followed_from_where_the_bar_is() {
    let mut f = Follower::new();
    f.set_target(30.0, 0);
    let shown = run(&mut f, 0, 5, 16);
    f.set_target(60.0, 80);
    let next = f.value(96);
    assert!(next > shown && next < 60.0, "{next} after {shown}");
}

#[test]
fn a_download_that_starts_over_takes_the_bar_back_at_once() {
    let mut f = Follower::new();
    f.set_target(70.0, 0);
    run(&mut f, 0, 100, 16);
    // A retry or a resume reports from the start again.
    f.set_target(5.0, 2000);
    assert_eq!(f.value(2000), 5.0, "a bar must not slide backwards slowly");
}

#[test]
fn a_finished_download_fills_the_bar() {
    let mut f = Follower::new();
    f.set_target(12.0, 0);
    run(&mut f, 0, 10, 16);
    f.set_target(100.0, 160);
    assert_eq!(run(&mut f, 160, 90, 16), 100.0);
    assert!(f.is_settled());
}

#[test]
fn nonsense_from_outside_cannot_break_the_bar() {
    let mut f = Follower::new();
    for bad in [f64::NAN, f64::INFINITY, -5.0, 250.0] {
        f.set_target(bad, 0);
        let shown = run(&mut f, 0, 200, 16);
        assert!((0.0..=100.0).contains(&shown), "{bad} gave {shown}");
    }
    // A clock that goes backwards moves nothing.
    let mut f = Follower::new();
    f.set_target(50.0, 1000);
    let shown = f.value(1100);
    assert_eq!(f.value(900), shown);
}
