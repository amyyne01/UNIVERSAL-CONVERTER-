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
use rune::{Image, ImageCache, KeyEvent, Tx, Waker};

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

/// A whole-app layout, new from scratch (owner, 2026-11-15): no sidebar and
/// no Home / Downloads pages. Each wears one of the three looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// One paste-and-command bar; downloads stream under it. Wears Signal.
    Bar,
    /// A wall of cover art; a download is a cover filling in. Wears Shelf.
    Collection,
    /// A player: the current download on stage, the queue beside it. Wears Orbit.
    Stage,
}

impl Layout {
    /// The look this layout wears.
    pub fn look(self) -> Direction {
        match self {
            Layout::Bar => Direction::Signal,
            Layout::Collection => Direction::Shelf,
            Layout::Stage => Direction::Orbit,
        }
    }
}

/// Where a new layout is: its three places (owner, 2026-11-15). Each layout
/// shows them with its own visible navigation; Ctrl+1, 2, 3 go to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    /// The layout's signature view.
    Home,
    /// The complete list, with filters, search and the bulk actions.
    Downloads,
    /// Everything else: schedule, licence, updates, appearance.
    Settings,
}

/// `--layout bar|collection|stage`; none means an old-layout direction is shown.
pub fn layout_from_args() -> Option<Layout> {
    match arg("--layout") {
        Some("bar") => Some(Layout::Bar),
        Some("collection") => Some(Layout::Collection),
        Some("stage") => Some(Layout::Stage),
        _ => None,
    }
}

/// The six presets the app ships (owner, 2026-11-15), in the order Settings
/// lists them: the three new layouts, then the three looks on the classic one.
pub const PRESETS: [(&str, Direction, Option<Layout>); 6] = [
    ("One bar", Direction::Signal, Some(Layout::Bar)),
    ("The collection", Direction::Shelf, Some(Layout::Collection)),
    ("Now playing", Direction::Orbit, Some(Layout::Stage)),
    ("Signal", Direction::Signal, None),
    ("Shelf", Direction::Shelf, None),
    ("Orbit", Direction::Orbit, None),
];

/// Which of [`PRESETS`] is on.
pub fn preset(s: &State) -> usize {
    PRESETS.iter().position(|p| p.1 == s.direction && p.2 == s.layout).unwrap_or(0)
}

/// Switches preset in the running app: no restart, the fonts follow.
pub fn set_preset(s: &mut State, index: usize, tx: &mut Tx) {
    let (_, direction, layout) = PRESETS[index % PRESETS.len()];
    s.direction = direction;
    s.layout = layout;
    let (sans, mono) = direction.families();
    tx.set_font_families(sans, mono);
}

/// The keys every preset shares: Ctrl+1, 2, 3 for Home, Downloads and
/// Settings, and Ctrl+. for the preset picker.
pub fn on_key(s: &mut State, ev: &KeyEvent, _tx: &mut Tx) -> bool {
    let go = |s: &mut State, nav: Nav| {
        s.nav = nav;
        s.reach = nav == Nav::Settings;
        s.screen = if nav == Nav::Downloads { Screen::Downloads } else { Screen::Home };
    };
    if ev.is_command('1') {
        go(s, Nav::Home);
    } else if ev.is_command('2') {
        go(s, Nav::Downloads);
    } else if ev.is_command('3') {
        go(s, Nav::Settings);
    } else if ev.is_command('.') {
        s.picker = !s.picker;
    } else if s.picker && ev.key == rune::Key::Escape {
        s.picker = false;
    } else {
        return false;
    }
    true
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
    /// The new layout on show, if one was asked for.
    pub layout: Option<Layout>,
    /// True in the `reach` scene: the layout shows its way to everything
    /// that is not a download (settings, schedule, licence, updates) open.
    pub reach: bool,
    /// The place a new layout is showing.
    pub nav: Nav,
    /// True in the `nav` scene: the layout's navigation in its opened state
    /// (a sidebar expanded as on hover, say), for the picture.
    pub nav_open: bool,
    /// The sidebar is pinned open (the presets that have one).
    pub nav_pinned: bool,
    /// The preset picker is open.
    pub picker: bool,
    /// The picker's six previews, in the order of [`PRESETS`].
    pub previews: Vec<Image>,
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
    if let Some(layout) = layout_from_args() {
        return layout.look();
    }
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
    State { direction, layout: layout_from_args(), reach: false, nav: Nav::Home, nav_open: false, nav_pinned: false, picker: false, previews: Vec::new(), screen: Screen::Home, palette, tasks: sample::everything(), stats: Some(sample::stats()), covers: HashMap::new(), logo: None, input: String::new(), cache: None }
}

/// Loads the sample covers. It waits for each to decode, so a headless
/// picture taken on the first frame already shows them.
pub fn setup(s: &mut State, waker: Waker) {
    let cache = s.cache.insert(ImageCache::new(waker));
    let logo = cache.load(&include_bytes!("../../../assets/icon-256.png")[..]);
    let _ = logo.wait();
    s.logo = Some(logo);
    s.previews = crate::picker::PREVIEWS.iter().map(|bytes| cache.load(*bytes)).collect();
    for preview in &s.previews {
        let _ = preview.wait();
    }
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
        s.nav = Nav::Downloads;
        s.tasks.clear();
    }),
    ("downloads-active", |s| {
        s.screen = Screen::Downloads;
        s.nav = Nav::Downloads;
        s.tasks = sample::active();
    }),
    ("downloads-finished", |s| {
        s.screen = Screen::Downloads;
        s.nav = Nav::Downloads;
        s.tasks = sample::finished();
    }),
    ("downloads-failed", |s| {
        s.screen = Screen::Downloads;
        s.nav = Nav::Downloads;
        s.tasks = sample::troubled();
    }),
    ("downloads-everything", |s| {
        s.screen = Screen::Downloads;
        s.nav = Nav::Downloads;
    }),
    // The new layouts have no pages, so their scenes are states of the app.
    ("first", |s| {
        s.tasks.clear();
        s.stats = None;
    }),
    ("idle", |s| s.tasks = sample::finished().into_iter().chain(sample::troubled()).collect()),
    ("active", |s| s.tasks = sample::active()),
    ("finished", |s| s.tasks = sample::finished()),
    ("failed", |s| s.tasks = sample::troubled()),
    ("everything", |_| {}),
    ("reach", |s| {
        s.reach = true;
        s.nav = Nav::Settings;
    }),
    ("downloads", |s| s.nav = Nav::Downloads),
    ("nav", |s| s.nav_open = true),
    ("picker", |s| s.picker = true),
    ("nav-pinned", |s| s.nav_pinned = true),
];
