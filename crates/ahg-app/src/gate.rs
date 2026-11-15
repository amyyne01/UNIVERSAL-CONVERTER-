//! The design gate: Home and Downloads built three ways (Shelf, Signal,
//! Orbit) over the same prepared downloads, so the owner can compare like for
//! like and pick one. Nothing here talks to the engine; the direction that
//! wins is then wired to it and the other two are deleted.
//!
//! Which direction and palette to show comes from the command line
//! (`--direction shelf|signal|orbit`, `--palette 0..=7`); which state from a
//! scene (`--scene home-history`); light or dark from Rune's own `--theme`.
use std::collections::HashMap;

use ahg_engine::stats::DownloadStats;
use ahg_engine::types::DownloadTask;
use rune::{Image, ImageCache, Waker};

use crate::sample;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// A record collection: cover art leads, warm paper-and-ink neutrals.
    Shelf,
    /// A transfer desk: meters and timecode, cool graphite.
    Signal,
    /// The planet logo's world: a download coming in to land, deep ink.
    Orbit,
}

impl Direction {
    /// The interface family and the one for figures. Shelf: Instrument Sans
    /// with IBM Plex Mono. Signal: Geist with Geist Mono. Orbit: Space
    /// Grotesk with Space Mono.
    pub fn families(self) -> (&'static str, &'static str) {
        match self {
            Direction::Shelf => ("Instrument Sans", "IBM Plex Mono"),
            Direction::Signal => ("Geist", "Geist Mono"),
            Direction::Orbit => ("Space Grotesk", "Space Mono"),
        }
    }
}

/// The display serif, for Shelf's one headline per screen and nothing else.
pub const SERIF: &str = "Instrument Serif";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Downloads,
}

/// Everything a direction's view reads. The same for all three.
pub struct State {
    pub direction: Direction,
    pub screen: Screen,
    /// 0 is the direction's own colour mood; 1 to 7 are the app's existing
    /// palettes, in the order Settings lists them, to show the direction
    /// survives a switch.
    pub palette: usize,
    /// The downloads on show, newest first. Empty on a first launch.
    pub tasks: Vec<DownloadTask>,
    /// This month and the last active one; `None` before anything was downloaded.
    pub stats: Option<DownloadStats>,
    /// Cover art by task id. A task without one gets the direction's own placeholder.
    pub covers: HashMap<String, Image>,
    /// The planet logo (256 px), for a direction to draw at any size.
    pub logo: Option<Image>,
    /// What is typed into the paste box on Home.
    pub input: String,
    cache: Option<ImageCache>,
}

impl State {
    /// The cover of a task, once it is decoded.
    pub fn cover(&self, task_id: &str) -> Option<&Image> {
        self.covers.get(task_id).filter(|image| image.is_ready())
    }
}

/// The value after `--name` on the command line.
fn arg(name: &str) -> Option<&'static str> {
    let args = rune::extra_args();
    args.iter().position(|a| a == name).and_then(|at| args.get(at + 1)).map(String::as_str)
}

/// `--direction shelf|signal|orbit`; Shelf when none is given.
pub fn direction_from_args() -> Direction {
    match arg("--direction") {
        Some("signal") => Direction::Signal,
        Some("orbit") => Direction::Orbit,
        _ => Direction::Shelf,
    }
}

pub fn init() -> State {
    let direction = direction_from_args();
    let palette = arg("--palette").and_then(|p| p.parse().ok()).filter(|p| *p <= 7).unwrap_or(0);
    // With no scene asked for, the fullest state: Home with history.
    State { direction, screen: Screen::Home, palette, tasks: sample::everything(), stats: Some(sample::stats()), covers: HashMap::new(), logo: None, input: String::new(), cache: None }
}

/// Loads the sample covers. It waits for each to decode, so a headless
/// picture taken on the first frame already shows them.
pub fn setup(s: &mut State, waker: Waker) {
    let cache = s.cache.insert(ImageCache::new(waker));
    let logo = cache.load(&include_bytes!("../../../assets/icon-256.png")[..]);
    let _ = logo.wait();
    s.logo = Some(logo);
    for (task_id, bytes) in sample::covers() {
        let image = cache.load(&bytes[..]);
        let _ = image.wait();
        s.covers.insert(task_id, image);
    }
}

/// The states the comparison page shows, for `--scene`.
pub const SCENES: &[(&str, fn(&mut State))] = &[
    ("home-first", |s| {
        s.screen = Screen::Home;
        s.tasks.clear();
        s.stats = None;
    }),
    ("home-history", |s| s.screen = Screen::Home),
    ("downloads-empty", |s| {
        s.screen = Screen::Downloads;
        s.tasks.clear();
    }),
    ("downloads-active", |s| {
        s.screen = Screen::Downloads;
        s.tasks = sample::active();
    }),
    ("downloads-finished", |s| {
        s.screen = Screen::Downloads;
        s.tasks = sample::finished();
    }),
    ("downloads-failed", |s| {
        s.screen = Screen::Downloads;
        s.tasks = sample::troubled();
    }),
    ("downloads-everything", |s| s.screen = Screen::Downloads),
];
