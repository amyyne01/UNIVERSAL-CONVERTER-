//! The design gate: Home and Downloads built three ways (Shelf, Signal,
//! Orbit) over the same prepared downloads, so the owner can compare like for
//! like and pick one. Nothing here talks to the engine; the direction that
//! wins is then wired to it and the other two are deleted.
//!
//! Which direction and palette to show comes from the command line
//! (`--direction shelf|signal|orbit`, `--palette 0..=7`); which state from a
//! scene (`--scene home-history`); light or dark from Rune's own `--theme`.
use std::collections::{HashMap, HashSet, VecDeque};

use ahg_engine::stats::DownloadStats;
use ahg_engine::tier::Plan;
use ahg_engine::types::{DownloadStatus, DownloadTask, MediaMetadata, Playlist, SearchResult, SourcePlatform, VideoQuality};
use ahg_engine::updater::UpdateState;
use ahg_engine::ytdlp_updater::YtdlpStatus;
use rune::{Cx, Image, ImageCache, Key, KeyEvent, TextEdit, Tx, Waker};
use serde_json::{Map, Value};

use crate::{actions, keys, sample};

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

/// Forwards to [`actions::set_preset`] until the picker calls it there (step 56).
pub fn set_preset(s: &mut State, index: usize, tx: &mut Tx) {
    actions::set_preset(s, index, tx)
}

/// The search scopes Ctrl+4, 5, 6, 7 set (D42), in the order of Electron's
/// tab list: YouTube, Spotify, SoundCloud, Reels & Shorts. Reels stands as
/// Instagram, the first of the three sources that tab took.
pub const SCOPES: [SourcePlatform; 4] =
    [SourcePlatform::Youtube, SourcePlatform::Spotify, SourcePlatform::Soundcloud, SourcePlatform::Instagram];

/// The keys every preset shares, in the order of `CONTENT.md` 1.3: Ctrl+K
/// always; while the command palette is open it owns Esc and Ctrl+V and
/// nothing else gets through; then Ctrl+. (the preset picker), Ctrl+/,
/// Ctrl+1..3 (the three places), Ctrl+4..7 (the scope), Ctrl+V (paste and
/// look up, on Home), Ctrl+A (every search result), and Esc, which closes the
/// first of: an overlay, an armed confirm, what the field found.
pub fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let used = key(s, ev, tx);
    s.sync();
    used
}

fn key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let escape = ev.key == Key::Escape;
    if ev.is_command('k') {
        if s.overlay == Overlay::Palette {
            actions::close(s)
        } else {
            actions::open(s, Overlay::Palette)
        }
        return true;
    }
    if s.overlay == Overlay::Palette {
        if escape {
            actions::close(s);
        } else if ev.is_command('v') {
            actions::run(s, tx, actions::Command::PasteAndDownload);
        } else {
            return false;
        }
        return true;
    }
    let digit = ('1'..='7').position(|c| ev.is_command(c));
    if ev.is_command('.') {
        if s.picker { actions::close(s) } else { actions::open(s, Overlay::Picker) }
    } else if ev.is_command('/') {
        if s.overlay == Overlay::Shortcuts { actions::close(s) } else { actions::show_shortcuts(s) }
    } else if let Some(i @ 0..=2) = digit {
        actions::go(s, [Nav::Home, Nav::Downloads, Nav::Settings][i]);
    } else if let Some(i) = digit {
        actions::set_scope(s, Some(SCOPES[i - 3]));
        actions::go(s, Nav::Home);
        tx.focus(keys::FIELD, false);
    } else if ev.is_command('v') && s.nav == Nav::Home {
        actions::paste(s, tx);
    } else if ev.is_command('a') && matches!(s.found, Found::Search { .. }) {
        actions::pick_all_results(s);
    } else if escape && (s.picker || s.overlay != Overlay::None) {
        actions::close(s);
    } else if escape && s.confirm.is_some() {
        actions::disarm(s);
    } else if escape && !matches!(s.found, Found::Nothing) {
        // Esc closes what the field found, as the close button does.
        actions::dismiss_found(s);
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

/// One source's answer to a search (YouTube, SoundCloud or Spotify).
pub enum Band {
    /// Still asking: the preset draws its skeleton rows.
    Loading,
    Results(Vec<SearchResult>),
    /// Answered with nothing.
    Empty,
    /// The source could not be reached; the band offers Retry.
    Failed,
}

/// Where a Spotify track's match stands. Matching runs in the background
/// while the track list is already on screen (D34).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Matched {
    /// Still looking: skeleton or spinner on that row.
    Matching,
    Confirmed,
    /// A best guess, with its confidence below 1.
    BestGuess(f64),
    /// No song found: this track will not download.
    NoMatch,
}

/// What the field found after a paste or a search: the step between the
/// empty field and a running download. It shows on Home, coming out of the field.
pub enum Found {
    Nothing,
    /// A link was pasted and its details are loading: the preset draws a skeleton.
    LookingUp,
    /// The lookup failed; the sentence to show.
    LookupFailed(String),
    /// Words were typed: one band per source, in the order YouTube, SoundCloud, Spotify.
    Search {
        query: String,
        bands: [Band; 3],
    },
    /// A playlist or album link: its tracks, to pick from. `matched` and
    /// `selected` run parallel to `playlist.tracks`.
    Playlist {
        playlist: Playlist,
        matched: Vec<Matched>,
        selected: Vec<bool>,
    },
    /// One video or track: choose audio or video, format and quality, then download.
    Media(MediaMetadata),
}

/// Everything a direction's view reads. The same for all three.
// Not all read yet: the presets move onto the shared state in step 56.
#[allow(dead_code)]
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
    /// True in the `ring` scene: the ring gallery instead of the preset (throwaway, removed at step 80 (K54)).
    pub ring: bool,
    /// Basic or Premium. Premium unless `--plan basic` is given: on Basic the
    /// presets draw the locks, limits and upgrade prompts (`ahg_engine::tier`).
    pub plan: Plan,
    /// A one-line notice on show, such as a Basic limit that was just hit.
    pub notice: Option<String>,
    /// What the field found; `Found::Nothing` in every scene but `search`, `playlist` and `media`.
    pub found: Found,
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
    /// What is typed into the paste box on Home: a copy of `field`, written
    /// wherever the field is, until the presets read `field` (step 56).
    pub input: String,
    /// The settings, the same map `Core::settings()` returns.
    pub settings: Map<String, Value>,
    /// Refused by `actions::set_setting`: the preset shows these rows invalid.
    pub invalid: HashSet<String>,
    /// The Rust-only choices kept in `settings-rust.json`.
    pub own: Own,
    /// Home's field.
    pub field: TextEdit,
    /// The search box on Downloads.
    pub query: TextEdit,
    /// The one source the field searches (a removable chip; D42); `None` searches all three.
    pub scope: Option<SourcePlatform>,
    pub filter: Filter,
    /// The downloads ticked on Downloads, by task id.
    pub selected: HashSet<String>,
    /// The search results ticked, by result id.
    pub picked: HashSet<String>,
    /// The queued row held for a keyboard reorder.
    pub grabbed: Option<String>,
    /// Format, quality and folder in the found step.
    pub choice: Choice,
    /// What `Found::Media` is drawn as. Kept beside `found` because the
    /// presets match `Found` exhaustively; it folds into `Found` (`Media`
    /// with its match line, and `Short`) when they move onto it.
    pub card: Card,
    /// "Show more" opened, per search band.
    pub expanded: [bool; 3],
    /// The task on stage (Now playing).
    pub staged: Option<String>,
    /// The sleeve opened (The collection), by task id.
    pub open_sleeve: Option<String>,
    /// The Settings section in view (One bar).
    pub section: usize,
    /// What sits over the preset. Derived from `picker` while that exists.
    pub overlay: Overlay,
    /// The two-step confirm that is armed.
    pub confirm: Option<Confirm>,
    /// The notice on show. Derived from `notice` while that exists.
    pub notices: Vec<Notice>,
    pub engine: EngineStatus,
    pub update: UpdateStatus,
    /// What a licence key binds to on this computer.
    pub machine_id: String,
    /// The last speed samples, in bytes per second, oldest first.
    pub speed: VecDeque<f32>,
    pub loading: Loading,
    /// State arriving after launch: it lands without an entrance.
    pub arriving: bool,
    /// The running core; `None` in sample mode.
    pub live: Option<crate::live::Handle>,
    cache: Option<ImageCache>,
}

impl State {
    /// The cover of a task, once it is decoded.
    pub fn cover(&self, task_id: &str) -> Option<&Image> {
        self.covers.get(task_id).filter(|image| image.is_ready())
    }

    /// Light, Dark or System, from `settings["theme"]`.
    #[allow(dead_code)]
    pub fn dark(&self, cx: &Cx<State>) -> bool {
        match self.settings.get("theme").and_then(Value::as_str) {
            Some("light") => false,
            Some("dark") => true,
            _ => cx.system_dark(),
        }
    }

    /// Brings the new fields in line with the old ones the presets still
    /// write (`nav_pinned`, `picker`, `notice`, `palette`). The old field
    /// wins. Idempotent; called at the end of `on_key` and of every action.
    pub fn sync(&mut self) {
        self.own.preset = preset(self);
        self.own.nav_pinned = self.nav_pinned;
        self.own.own_palette = self.palette == 0;
        if self.picker {
            self.overlay = Overlay::Picker;
        } else if self.overlay == Overlay::Picker {
            self.overlay = Overlay::None;
        }
        match &self.notice {
            None => self.notices.clear(),
            Some(text) if self.notices.last().map(|n| &n.text) != Some(text) => self.notices = vec![Notice::info(text.clone())],
            Some(_) => {}
        }
    }

    /// The fullest state of a preset (Home with history), from samples and
    /// without reading the command line, so tests can build one.
    pub fn sample(preset: usize) -> State {
        let (_, direction, layout) = PRESETS[preset % PRESETS.len()];
        let settings = sample::settings();
        let mut s = State {
            direction,
            layout,
            reach: false,
            nav: Nav::Home,
            nav_open: false,
            ring: false,
            plan: Plan::Premium,
            notice: None,
            found: Found::Nothing,
            nav_pinned: false,
            picker: false,
            previews: Vec::new(),
            screen: Screen::Home,
            palette: 0,
            tasks: sample::everything(),
            stats: Some(sample::stats()),
            covers: HashMap::new(),
            logo: None,
            input: String::new(),
            choice: Choice::from_settings(&settings),
            settings,
            invalid: HashSet::new(),
            own: Own::default(),
            field: TextEdit::default(),
            query: TextEdit::default(),
            scope: None,
            filter: Filter::All,
            selected: HashSet::new(),
            picked: HashSet::new(),
            grabbed: None,
            card: Card::default(),
            expanded: [false; 3],
            staged: None,
            open_sleeve: None,
            section: 0,
            overlay: Overlay::None,
            confirm: None,
            notices: Vec::new(),
            engine: sample::engine(),
            update: sample::update(),
            machine_id: sample::MACHINE_ID.into(),
            speed: sample::speed(),
            loading: Loading::Ready,
            arriving: false,
            live: None,
            cache: None,
        };
        s.sync();
        s
    }

    /// Puts `text` in Home's field and its copy in `input`.
    pub fn set_field(&mut self, text: &str) {
        self.field.replace_text(text);
        self.input = text.into();
    }
}

/// The Rust-only choices, kept in `settings-rust.json`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Own {
    /// Which of [`PRESETS`] is on.
    pub preset: usize,
    pub nav_pinned: bool,
    /// The look's own colour mood (palette 0), which Electron's `palette` cannot name.
    pub own_palette: bool,
    /// "Needs review": a best-guess Spotify match waits for a yes (D34).
    pub hold_unconfirmed: bool,
}

/// The four filters on Downloads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Active,
    Done,
    /// Failed and canceled.
    Failed,
}

impl Filter {
    pub const ALL: [Filter; 4] = [Filter::All, Filter::Active, Filter::Done, Filter::Failed];

    pub fn passes(self, t: &DownloadTask) -> bool {
        match self {
            Filter::All => true,
            Filter::Active => crate::format::is_active(t),
            Filter::Done => t.progress.status == DownloadStatus::Done,
            Filter::Failed => matches!(t.progress.status, DownloadStatus::Failed | DownloadStatus::Cancelled),
        }
    }
}

/// Format, quality and folder in the found step, from the saved defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    /// Video rather than audio (YouTube and Reels; the other sources are audio only).
    pub video: bool,
    /// The audio format: `mp3`, `flac` ...
    pub format: String,
    /// `128` ... `320`, or `lossless`.
    pub audio_quality: String,
    pub video_quality: VideoQuality,
    /// A folder for this download only; `None` uses `outputDir`.
    pub folder: Option<String>,
}

impl Choice {
    /// The saved defaults; Electron's own fallbacks when one is missing.
    pub fn from_settings(settings: &Map<String, Value>) -> Choice {
        let text = |key: &str, or: &str| settings.get(key).and_then(Value::as_str).unwrap_or(or).to_owned();
        Choice {
            video: true,
            format: text("defaultFormat", "mp3"),
            audio_quality: text("defaultQuality", "320"),
            video_quality: serde_json::from_value(Value::String(text("defaultVideoQuality", "best"))).unwrap_or(VideoQuality::Best),
            folder: None,
        }
    }
}

/// How `Found::Media` is drawn.
#[derive(Debug, Clone, PartialEq)]
pub enum Card {
    /// The single card. `matched` is a Spotify track's match line, the
    /// `{artist} - {name}` of `copy::found::matched_name`.
    Single { matched: Option<String> },
    /// The short-form poster of a Reels, TikTok or Facebook link; `label` is
    /// the detector's (`Instagram video`).
    Short { label: String },
}

impl Default for Card {
    fn default() -> Self {
        Card::Single { matched: None }
    }
}

/// What sits over the preset.
// Not all read yet: the presets move onto the shared state in step 56.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Overlay {
    #[default]
    None,
    Picker,
    /// The command palette (Ctrl+K).
    Palette,
    /// The keyboard shortcuts sheet (Ctrl+/).
    Shortcuts,
    /// The update prompt.
    Update,
}

/// A two-step action waiting for its confirm.
// Not all read yet: the presets move onto the shared state in step 56.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    /// "Clear N?": the finished rows.
    Clear,
    /// "Remove N?": the selected finished, failed and canceled rows.
    Remove,
    /// Deactivate the licence on this machine.
    Deactivate,
    /// Move the licence to another PC.
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Info,
    Error,
    /// A Basic limit; the button is "See plans".
    Premium,
}

/// A notice's one button.
// Not all read yet: the presets move onto the shared state in step 56.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// Queue these canceled downloads again.
    Undo(Vec<String>),
    ViewDownloads,
    OpenFile(String),
    SeePlans,
}

impl Act {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            Act::Undo(_) => crate::copy::TOAST_UNDO,
            Act::ViewDownloads => crate::copy::found::TOAST_VIEW_DOWNLOADS,
            Act::OpenFile(_) => crate::copy::TOAST_SAVED_OPEN_FILE,
            Act::SeePlans => crate::copy::NOTICE_SEE_PLANS,
        }
    }
}

/// The transient popup (`CONTENT.md` 1.6).
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub tone: Tone,
    pub text: String,
    pub action: Option<Act>,
    /// When it goes by itself, in `Tx::clock` seconds; set by the motion step.
    pub until: Option<f64>,
}

impl Notice {
    pub fn info(text: String) -> Notice {
        Notice { tone: Tone::Info, text, action: None, until: None }
    }
}

/// The download engine's state, as Settings shows it.
pub type EngineStatus = YtdlpStatus;
/// The app updater's state, as Settings shows it.
pub type UpdateStatus = UpdateState;

/// Whether the first data is still on its way.
// Not all read yet: the presets move onto the shared state in step 56.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loading {
    Booting,
    Ready,
}

/// A part of the screen that can be loading.
// Not all read yet: the presets move onto the shared state in step 56.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part<'a> {
    /// The downloads list.
    History,
    /// This month's statistics.
    Stats,
    /// A pasted link's details.
    Lookup,
    /// A search band, 0 YouTube, 1 SoundCloud, 2 Spotify.
    Band(usize),
    /// A playlist track's match.
    Match(usize),
    /// A task's cover.
    Cover(&'a str),
}

/// What is loading. When the preset may show it is `timeline::shows` (step 42).
// Not all read yet: the presets move onto the shared state in step 56.
#[allow(dead_code)]
pub fn is_loading(s: &State, part: Part) -> bool {
    match part {
        Part::History | Part::Stats => s.loading == Loading::Booting,
        Part::Lookup => matches!(s.found, Found::LookingUp),
        Part::Band(i) => matches!(&s.found, Found::Search { bands, .. } if matches!(bands.get(i), Some(Band::Loading))),
        Part::Match(i) => matches!(&s.found, Found::Playlist { matched, .. } if matched.get(i) == Some(&Matched::Matching)),
        Part::Cover(id) => s.covers.get(id).is_some_and(|image| !image.is_ready()),
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

/// [`State::sample`] of the preset the command line asks for, with its
/// `--plan` and `--palette`. With no scene asked for, Home with history.
pub fn init() -> State {
    let (direction, layout) = (direction_from_args(), layout_from_args());
    let mut s = State::sample(PRESETS.iter().position(|p| p.1 == direction && p.2 == layout).unwrap_or(0));
    s.palette = arg("--palette").and_then(|p| p.parse().ok()).filter(|p| *p <= 7).unwrap_or(0);
    if arg("--plan") == Some("basic") {
        s.plan = Plan::Basic;
    }
    s.sync();
    s
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
    ("picker", |s| actions::open(s, Overlay::Picker)),
    ("search", |s| {
        s.set_field(sample::QUERY);
        s.found = Found::Search { query: sample::QUERY.into(), bands: sample::search().map(Band::Results) };
    }),
    ("search-loading", |s| {
        s.set_field(sample::QUERY);
        s.found = Found::Search { query: sample::QUERY.into(), bands: [Band::Loading, Band::Loading, Band::Loading] };
    }),
    // One source answered, one found nothing, one could not be reached.
    ("search-mixed", |s| {
        let [youtube, _, _] = sample::search();
        s.set_field(sample::QUERY);
        s.found = Found::Search { query: sample::QUERY.into(), bands: [Band::Results(youtube), Band::Empty, Band::Failed] };
    }),
    ("looking-up", |s| {
        s.set_field("https://www.youtube.com/watch?v=example");
        s.found = Found::LookingUp;
    }),
    ("lookup-failed", |s| {
        s.set_field("https://www.youtube.com/watch?v=example");
        s.found = Found::LookupFailed(ahg_engine::downloader::errors::explain_error(
            "ERROR: [youtube] example: Video unavailable. This video has been removed by the uploader",
        ));
    }),
    // Matches at every stage: confirmed, a best guess, still matching, none found.
    ("playlist", |s| {
        let playlist = sample::playlist();
        let matched = (0..playlist.tracks.len())
            .map(|i| match i {
                3 => Matched::BestGuess(0.9),
                6 => Matched::NoMatch,
                7.. => Matched::Matching,
                _ => Matched::Confirmed,
            })
            .collect();
        let selected = (0..playlist.tracks.len()).map(|i| i != 4 && i != 6).collect();
        s.set_field("https://open.spotify.com/album/example");
        s.found = Found::Playlist { playlist, matched, selected };
    }),
    // A YouTube playlist on the Basic plan: rows cannot be picked, 20 at most are taken.
    ("playlist-basic", |s| {
        let playlist = sample::youtube_playlist();
        let rows = playlist.tracks.len();
        s.plan = Plan::Basic;
        s.set_field(&playlist.url);
        s.found = Found::Playlist { playlist, matched: vec![Matched::Confirmed; rows], selected: vec![true; rows] };
    }),
    // More than five results ticked on Basic: nothing is queued and the notice shows.
    ("search-limit", |s| {
        s.plan = Plan::Basic;
        s.set_field(sample::QUERY);
        s.found = Found::Search { query: sample::QUERY.into(), bands: sample::search().map(Band::Results) };
        s.notice = Some(crate::copy::found::LIMIT_BATCH.into());
        s.sync();
    }),
    ("media", |s| {
        s.set_field("https://www.youtube.com/watch?v=example");
        s.found = Found::Media(sample::media());
    }),
    // The scenes below came with the shared state (step 36); the old layouts draw them as they can.
    ("selected", |s| {
        actions::go(s, Nav::Downloads);
        for id in ["get-lucky", "dreams", "removed"] {
            actions::toggle_select(s, id);
        }
    }),
    ("confirm", |s| {
        actions::go(s, Nav::Downloads);
        actions::arm(s, Confirm::Clear);
    }),
    ("filtered-empty", |s| {
        actions::go(s, Nav::Downloads);
        s.tasks = sample::finished();
        actions::set_filter(s, Filter::Failed);
    }),
    ("short", |s| {
        s.set_field(sample::SHORT_URL);
        s.found = Found::Media(sample::short());
        s.card = Card::Short { label: ahg_engine::url_detector::detect_url(sample::SHORT_URL).label };
    }),
    ("settings-basic", |s| {
        s.plan = Plan::Basic;
        actions::go(s, Nav::Settings);
    }),
    ("palette", |s| actions::open(s, Overlay::Palette)),
    ("shortcuts", |s| actions::show_shortcuts(s)),
    ("notice", |s| actions::cancel(s, &mut Tx::default(), "big-buck-bunny")),
    // throwaway, removed at step 80 (K54): the ring in three materials (`ring::gallery`).
    ("ring", |s| s.ring = true),
];
