//! Every action a control or a key can trigger, as plain functions over
//! `State`, the same for all six presets: a preset's button calls one of
//! these and draws the result. In sample mode they change `State` only;
//! nothing reaches outside. A `// live:` line marks where live mode will also
//! tell the core (step 37 on). Each action ends with `State::sync`.
use ahg_engine::downloader::errors::Fix;
use ahg_engine::queue::{TaskInit, create_task};
use ahg_engine::text::{ascii_ci_regex, is_js_whitespace, js_len, js_trim};
use ahg_engine::tier::{BASIC_LIMITS, Plan, is_premium_format, is_premium_video_quality};
use ahg_engine::types::{AUDIO_ONLY_SOURCES, ContentType, DownloadStatus as St, DownloadTask, SourcePlatform, Track, VideoQuality};
use ahg_engine::url_detector::detect_url;
use rune::{Mods, TextEdit, Tx};
use serde_json::Value;

use crate::copy::{self, found as fc};
use crate::gate::{
    Act, Band, Card, Choice, Confirm, Filter, Found, Nav, Notice, Overlay, PRESETS, PaletteState, SCOPES, Screen, State, Tone, Wait,
    WaitKey,
};
use crate::live::Change;
use crate::motion::{self, LOADING_DELAY};
use crate::ring::Pass;
use crate::settings_spec::{self, Kind, Opt};
use crate::timeline::{self, Due};
use crate::{format, live};
use ahg_runtime::core::{CookiesImport, Core, Shown};

/// `premiumCopy.cookies()`, which `copy` does not carry yet.
// TODO(copy): move into copy/settings.rs.
const LOCK_COOKIES: &str = "Signing in with your browser is a Premium feature.";

// ── Places and overlays

/// Goes to one of the three places. The old `screen` and `reach` follow,
/// for the presets that still read them.
pub fn go(s: &mut State, nav: Nav) {
    s.nav = nav;
    s.reach = nav == Nav::Settings;
    s.screen = if nav == Nav::Downloads { Screen::Downloads } else { Screen::Home };
    s.sync();
}

pub fn open(s: &mut State, overlay: Overlay) {
    s.picker = overlay == Overlay::Picker;
    s.overlay = overlay;
    s.command = PaletteState::default();
    s.sync();
}

pub fn close(s: &mut State) {
    s.picker = false;
    s.overlay = Overlay::None;
    s.command = PaletteState::default();
    s.sync();
}

/// The commands that match what is typed: the label or the group name, ignoring case.
pub fn palette_filter(s: &State) -> Vec<Command> {
    let q = s.command.text.trim().to_lowercase();
    commands(s)
        .into_iter()
        .filter(|c| q.is_empty() || c.label(s).to_lowercase().contains(&q) || c.group().to_lowercase().contains(&q))
        .collect()
}

/// Typing in the search line; the highlight goes back to the first row.
pub fn palette_type(s: &mut State, typed: &str) {
    s.command.text.push_str(typed);
    s.command.highlight = 0;
}

pub fn palette_backspace(s: &mut State) {
    s.command.text.pop();
    s.command.highlight = 0;
}

/// Up (-1) and Down (1) over the matching rows, wrapping.
pub fn palette_move(s: &mut State, delta: isize) {
    let n = palette_filter(s).len() as isize;
    if n > 0 {
        s.command.highlight = (s.command.highlight as isize + delta).rem_euclid(n) as usize;
    }
}

/// Runs the highlighted row, if any.
pub fn palette_run(s: &mut State, tx: &mut Tx) {
    if let Some(&c) = palette_filter(s).get(s.command.highlight) {
        run(s, tx, c);
    }
}

/// A part starts waiting now. Its deadlines (bones, the two wait lines) go on the timeline.
pub fn begin_wait(s: &mut State, key: WaitKey) {
    s.waits.insert(key, Wait { started: s.now, answered: None, shown_at: None });
    for (at, due) in timeline::waiting(s.now) {
        s.timeline.at(at, due);
    }
}

/// The part's answer came now. Once nothing else waits the wait deadlines go; bones that are up
/// get their release at `shown_at + LOADING_MIN`.
// ponytail: answered waits stay in the table (a few entries); prune when the live code owns the clock.
pub fn answer_wait(s: &mut State, key: WaitKey) {
    let now = s.now;
    let Some(w) = s.waits.get_mut(&key).filter(|w| w.answered.is_none()) else { return };
    w.answered = Some(now);
    let shown_at = w.shown_at;
    if s.waits.values().all(|w| w.answered.is_some()) {
        for due in timeline::ANSWERED {
            s.timeline.cancel(due);
        }
    }
    for (at, due) in timeline::answered(now, shown_at) {
        s.timeline.at(at, due);
    }
}

/// The clock moved to `now`: fires what is due. Returns whether the picture may differ.
pub fn tick(s: &mut State, now: f64) -> bool {
    !fire(s, now).is_empty()
}

/// [`tick`], naming what each deadline that changed `State` changed, for the cause table.
pub fn fire(s: &mut State, now: f64) -> Vec<Change> {
    s.now = now;
    let mut out = Vec::new();
    for due in s.timeline.advance(now) {
        match due {
            Due::LoadingShow => {
                for w in s.waits.values_mut().filter(|w| w.shown_at.is_none() && now >= w.started + LOADING_DELAY) {
                    if w.answered.is_none_or(|a| a > w.started + LOADING_DELAY) {
                        w.shown_at = Some(now);
                    }
                }
                out.push(Change::Bones);
            }
            Due::LoadingFree => out.push(Change::Freed),
            Due::WaitLine(_) => out.push(Change::WaitLine),
            // Nothing here: the drain's `Rings::sync` that follows finds the stall.
            Due::Stall => {}
            // The finish chain's last step (MOTION-SYSTEM 5.6).
            Due::RingLanded(id) => {
                let held = s.rings.pass(&id) == Some(Pass::Done);
                s.rings.land(&id);
                if held {
                    out.push(Change::Landed);
                }
            }
        }
    }
    out
}

/// Switches preset in the running app: no restart, the fonts follow, the colours spring
/// (`MOTION-SYSTEM.md` 4.3). Every element of the new preset is new, so its scroll is put back
/// by intent (4.6): the anchor lands on the frame the new preset first draws, else its top.
/// Focus on a picker tile stays by itself (the picker is outside the preset).
pub fn set_preset(s: &mut State, index: usize, tx: &mut Tx) {
    let (_, direction, layout) = PRESETS[index % PRESETS.len()];
    let same = (s.direction, s.layout) == (direction, layout);
    s.direction = direction;
    s.layout = layout;
    // The fonts are set even for the preset already on (a launch on a saved preset).
    let (sans, mono) = direction.families();
    tx.set_font_families(sans, mono);
    if !same {
        // No sidebar pops open in the new preset.
        s.nav_open = false;
        motion::recolour(tx);
        if let Some(anchor) = scroll_anchor(s) {
            tx.scroll_key_to(anchor, 0.0);
        }
    }
    s.sync();
}

/// What a switch keeps in view: the Layout control in Settings; elsewhere the open sleeve,
/// the staged task or the first selected row.
fn scroll_anchor(s: &State) -> Option<String> {
    if s.nav == Nav::Settings {
        return Some(crate::keys::LAYOUT_CHOOSE.into());
    }
    let selected = || s.tasks.iter().map(|t| &t.task_id).find(|id| s.selected.contains(*id));
    s.open_sleeve.as_ref().or(s.staged.as_ref()).or_else(selected).map(|id| crate::keys::row(id))
}

pub fn toggle_pin(s: &mut State) {
    s.nav_pinned = !s.nav_pinned;
    s.sync();
}

/// A row of the command palette (`CONTENT.md` 1.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Go(Nav),
    /// "Go to YouTube" and the other three: the field's scope (D42).
    Scope(SourcePlatform),
    PasteAndDownload,
    ToggleTheme,
    CheckUpdates,
    ClearCompleted,
    PauseAll,
    ResumeAll,
    ChangeFolder,
    Shortcuts,
    CancelAll,
}

impl Command {
    pub fn label(self, s: &State) -> String {
        let place = |nav| match nav {
            Nav::Home => copy::NAV_HOME,
            Nav::Downloads => copy::NAV_DOWNLOADS,
            Nav::Settings => copy::NAV_SETTINGS,
        };
        let scope = |p| match p {
            SourcePlatform::Youtube => copy::NAV_YOUTUBE,
            SourcePlatform::Spotify => copy::NAV_SPOTIFY,
            SourcePlatform::Soundcloud => copy::NAV_SOUNDCLOUD,
            _ => copy::NAV_REELS,
        };
        match self {
            Command::Go(nav) => copy::palette_go_to(place(nav)),
            Command::Scope(p) => copy::palette_go_to(scope(p)),
            Command::PasteAndDownload => copy::PALETTE_PASTE_AND_DOWNLOAD.into(),
            Command::ToggleTheme if theme(s) == "light" => copy::PALETTE_SWITCH_TO_DARK.into(),
            Command::ToggleTheme => copy::PALETTE_SWITCH_TO_LIGHT.into(),
            Command::CheckUpdates => copy::PALETTE_CHECK_UPDATES.into(),
            Command::ClearCompleted => copy::PALETTE_CLEAR_COMPLETED.into(),
            Command::PauseAll => copy::PALETTE_PAUSE_ALL.into(),
            Command::ResumeAll => copy::PALETTE_RESUME_ALL.into(),
            Command::ChangeFolder => copy::PALETTE_CHANGE_FOLDER.into(),
            Command::Shortcuts => copy::PALETTE_SHORTCUTS.into(),
            Command::CancelAll => copy::PALETTE_CANCEL_ALL.into(),
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            Command::Go(_) | Command::Scope(_) => copy::PALETTE_GROUP_NAVIGATE,
            _ => copy::PALETTE_GROUP_ACTIONS,
        }
    }

    /// The keycap hint: `Ctrl+1` ... `Ctrl+7`, `Ctrl+V`, `Ctrl+/`.
    pub fn hint(self) -> Option<String> {
        let key = match self {
            Command::Go(nav) => ((nav as u8) + b'1') as char,
            Command::Scope(p) => (SCOPES.iter().position(|x| *x == p)? as u8 + b'4') as char,
            Command::PasteAndDownload => 'V',
            Command::Shortcuts => '/',
            _ => return None,
        };
        Some(format!("{}+{key}", copy::KEY_CTRL))
    }
}

/// The palette's rows in order: the seven places, then the actions.
pub fn commands(_s: &State) -> Vec<Command> {
    use Command::*;
    let mut all = vec![Go(Nav::Home), Go(Nav::Downloads), Go(Nav::Settings)];
    all.extend(SCOPES.map(Scope));
    all.extend([PasteAndDownload, ToggleTheme, CheckUpdates, ClearCompleted, PauseAll, ResumeAll, ChangeFolder, Shortcuts, CancelAll]);
    all
}

/// Runs a palette row; every row closes the palette first.
pub fn run(s: &mut State, tx: &mut Tx, c: Command) {
    close(s);
    match c {
        Command::Go(nav) => go(s, nav),
        Command::Scope(p) => {
            set_scope(s, Some(p));
            go(s, Nav::Home);
            tx.focus(crate::keys::FIELD, false);
        }
        Command::PasteAndDownload => tx.read_clipboard(|s: &mut State, content, _| paste_and_download(s, content.text())),
        Command::ToggleTheme => set_theme(s, if theme(s) == "light" { "dark" } else { "light" }, tx),
        Command::CheckUpdates => check_update(s),
        Command::ClearCompleted => clear_completed(s),
        Command::PauseAll => set_all(s, St::Paused),
        Command::ResumeAll => set_all(s, St::Queued),
        Command::ChangeFolder => choose_folder(s, tx),
        Command::Shortcuts => show_shortcuts(s),
        Command::CancelAll => {
            // The palette's Cancel all offers no Undo (Electron's `download.cancelAll`).
            for t in s.tasks.iter_mut().filter(|t| format::is_active(t)) {
                t.progress.status = St::Cancelled;
            }
            live::tell(s, Core::cancel_all);
        }
    }
    s.sync();
}

/// "Paste link & download": the clipboard's link is queued with the saved
/// defaults, without the found step. Only Spotify and SoundCloud are audio
/// here (Electron's palette), not every audio-only source.
pub fn paste_and_download(s: &mut State, text: &str) {
    let url = js_trim(text);
    if url.is_empty() {
        return notify(s, Tone::Error, copy::PALETTE_CLIPBOARD_EMPTY.into(), None);
    }
    let det = detect_url(url);
    if det.platform == SourcePlatform::Unknown {
        return notify(s, Tone::Error, copy::error_text(copy::ERR_UNSUPPORTED_CAUSE, copy::ERR_UNSUPPORTED_REMEDY, false), None);
    }
    let audio = matches!(det.platform, SourcePlatform::Spotify | SourcePlatform::Soundcloud);
    let init = TaskInit { url: Some(det.url), source: Some(det.platform), is_playlist: Some(det.is_collection), ..defaults(s, audio) };
    queue(s, init);
    s.sync();
}

fn theme(s: &State) -> &str {
    s.settings.get("theme").and_then(Value::as_str).unwrap_or("system")
}

// ── The field

/// Enter in the field: the decision tree of `05-tabs.md` 1.4. Several links
/// queue as a batch; words search; one link of a source with a place of its
/// own is looked up; any other supported link is queued at once.
pub fn submit(s: &mut State, _tx: &mut Tx) {
    let input = js_trim(&s.field.text).to_owned();
    if input.is_empty() {
        return;
    }
    let parts: Vec<&str> = input.split(is_js_whitespace).filter(|p| !p.is_empty()).collect();
    if parts.len() > 1 && parts.iter().all(|p| looks_like_url(p)) {
        s.found = Found::Nothing;
        return batch(s, &parts);
    }
    if !looks_like_url(&input) {
        if js_len(&input) < 2 {
            return;
        }
        reset_found(s);
        let scope = s.scope;
        let band = |i| if scope.is_none_or(|p| band_of(p) == i) { Band::Loading } else { Band::Empty };
        let asked = [band(0), band(1), band(2)].map(|b| matches!(b, Band::Loading));
        s.found = Found::Search { query: input.clone(), bands: [band(0), band(1), band(2)] };
        live::search(s, &input, asked);
        return s.sync();
    }
    let det = detect_url(&input);
    use SourcePlatform::*;
    match det.platform {
        Youtube | Spotify | Soundcloud | Instagram | Tiktok | Facebook => {
            reset_found(s);
            s.found = Found::LookingUp;
            live::look_up(s, det);
        }
        Unknown => s.found = Found::LookupFailed(copy::ERR_NOT_SUPPORTED.into()),
        _ => {
            s.found = Found::Nothing;
            batch(s, &[input.as_str()]);
        }
    }
    s.sync();
}

/// The search band a scope asks: YouTube (and Reels, which searched
/// YouTube in Electron), SoundCloud, Spotify.
pub(crate) fn band_of(scope: SourcePlatform) -> usize {
    match scope {
        SourcePlatform::Soundcloud => 1,
        SourcePlatform::Spotify => 2,
        _ => 0,
    }
}

fn reset_found(s: &mut State) {
    s.picked.clear();
    s.expanded = [false; 3];
    s.choice = Choice::from_settings(&s.settings);
    s.card = Card::default();
}

/// `batchDownload`: each link with the saved defaults; a Spotify collection
/// is skipped. Over the Basic limit nothing is queued.
fn batch(s: &mut State, urls: &[&str]) {
    if s.plan == Plan::Basic && urls.len() > BASIC_LIMITS.max_batch_links as usize {
        return premium(s, copy::BASIC_LIMIT_BATCH);
    }
    let mut started = 0;
    for url in urls {
        let det = detect_url(url);
        // live: a Spotify link is read first (`spotify.fetchTrack`); a collection reads as nothing.
        if det.platform == SourcePlatform::Unknown || (det.platform == SourcePlatform::Spotify && det.content_type != ContentType::Track) {
            continue;
        }
        let audio = AUDIO_ONLY_SOURCES.contains(&det.platform);
        let init = TaskInit { url: Some(det.url), source: Some(det.platform), is_playlist: Some(det.is_collection), ..defaults(s, audio) };
        queue(s, init);
        started += 1;
    }
    if started > 0 {
        go(s, Nav::Downloads)
    } else {
        s.found = Found::LookupFailed(copy::ERR_NO_LINKS.into())
    }
    s.sync();
}

/// Ctrl+V on Home: reads the clipboard, then [`paste_text`].
pub fn paste(_s: &mut State, tx: &mut Tx) {
    tx.read_clipboard(|s: &mut State, content, tx| paste_text(s, tx, content.text()));
}

/// Pasted text: into the field, then looked up, searched or queued.
pub fn paste_text(s: &mut State, tx: &mut Tx, text: &str) {
    let text = js_trim(text);
    if text.is_empty() {
        return;
    }
    s.set_field(text);
    submit(s, tx);
}

pub fn set_scope(s: &mut State, scope: Option<SourcePlatform>) {
    s.scope = scope;
    s.sync();
}

/// Closes what the field found, and empties the field and the notice.
pub fn dismiss_found(s: &mut State) {
    s.found = Found::Nothing;
    live::cancel_lookups(s);
    s.set_field("");
    s.notices.clear();
    s.sync();
}

/// `looksLikeUrl` (PlatformTab.tsx): a scheme, `spotify:` or a dotted name.
pub fn looks_like_url(text: &str) -> bool {
    let link = ascii_ci_regex(r"^(?:[A-Za-z][A-Za-z0-9_+.-]*://|spotify:|[A-Za-z0-9_-]+\.[A-Za-z]{2,})");
    link.is_match(js_trim(text)).unwrap_or(false)
}

// ── The found step

/// Format and quality. Outside a playlist they are also saved as the new
/// defaults, at once, as Electron's output panel did.
pub fn set_choice(s: &mut State, c: Choice) {
    if s.plan == Plan::Basic {
        // A locked option shows what Premium adds and changes nothing.
        let quality = serde_json::to_value(c.video_quality).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
        let lock = if c.format != s.choice.format && is_premium_format(&c.format) {
            Some(fc::lock_format(&c.format.to_uppercase()))
        } else if c.audio_quality != s.choice.audio_quality && c.audio_quality == "lossless" {
            Some(fc::LOCK_LOSSLESS_QUALITY.into())
        } else if c.video_quality != s.choice.video_quality && is_premium_video_quality(c.video_quality) {
            Some(fc::lock_video_quality(&quality))
        } else {
            None
        };
        if let Some(why) = lock {
            return premium(s, &why);
        }
    }
    if !matches!(s.found, Found::Playlist { .. }) {
        set_setting(s, "defaultFormat", c.format.clone().into());
        set_setting(s, "defaultQuality", c.audio_quality.clone().into());
        set_setting(s, "defaultVideoQuality", serde_json::to_value(c.video_quality).unwrap_or_default());
    }
    s.choice = c;
    s.sync();
}

/// One change to the found step's choice.
pub enum ChoiceChange {
    Video(bool),
    Format(String),
    AudioQuality(String),
    VideoQuality(VideoQuality),
    Folder(Option<String>),
}

/// `set_choice` for one change: the rest of the choice stays as it is.
pub fn set_choice_with(s: &mut State, change: ChoiceChange) {
    let mut c = s.choice.clone();
    match change {
        ChoiceChange::Video(v) => c.video = v,
        ChoiceChange::Format(f) => c.format = f,
        ChoiceChange::AudioQuality(q) => c.audio_quality = q,
        ChoiceChange::VideoQuality(q) => c.video_quality = q,
        ChoiceChange::Folder(f) => c.folder = f,
    }
    set_choice(s, c);
}

/// A YouTube playlist on Basic cannot be picked from: it says so instead.
fn picking_locked(s: &mut State) -> bool {
    let locked = s.plan == Plan::Basic && matches!(&s.found, Found::Playlist { playlist, .. } if !is_spotify(&playlist.url));
    if locked {
        premium(s, fc::LOCK_PICK_PLAYLIST);
    }
    locked
}

fn is_spotify(url: &str) -> bool {
    detect_url(url).platform == SourcePlatform::Spotify
}

pub fn toggle_track(s: &mut State, i: usize) {
    if picking_locked(s) {
        return;
    }
    if let Found::Playlist { selected, .. } = &mut s.found
        && let Some(tick) = selected.get_mut(i)
    {
        *tick = !*tick;
    }
    s.sync();
}

/// "Select all", or "Deselect all" when every track is ticked.
pub fn toggle_all_tracks(s: &mut State) {
    if picking_locked(s) {
        return;
    }
    if let Found::Playlist { selected, .. } = &mut s.found {
        let all = selected.iter().all(|t| *t);
        selected.iter_mut().for_each(|t| *t = !all);
    }
    s.sync();
}

/// "Deselect all" of the playlist: every track unticked.
pub fn deselect_all_tracks(s: &mut State) {
    if picking_locked(s) {
        return;
    }
    if let Found::Playlist { selected, .. } = &mut s.found {
        selected.iter_mut().for_each(|t| *t = false);
    }
    s.sync();
}

/// The search results' Clear: no row ticked.
pub fn clear_results(s: &mut State) {
    s.picked.clear();
    s.sync();
}

/// The Plan row's "See plans".
pub fn see_plans(s: &mut State) {
    // TODO(upgrade sheet, step 68): open the sheet; until then the notice says what Premium is.
    let (title, lead) = if s.plan == Plan::Basic {
        (copy::settings::UPGRADE_TITLE_BASIC, copy::settings::UPGRADE_LEAD_BASIC)
    } else {
        (copy::settings::UPGRADE_TITLE_PREMIUM, copy::settings::UPGRADE_LEAD_PREMIUM)
    };
    notify(s, Tone::Premium, format!("{title}. {lead}"), None);
    s.sync();
}

pub fn toggle_result(s: &mut State, id: &str) {
    if !s.picked.remove(id) {
        s.picked.insert(id.into());
    }
    s.sync();
}

/// Ctrl+A over the search results: every row ticked.
pub fn pick_all_results(s: &mut State) {
    if let Found::Search { bands, .. } = &s.found {
        s.picked =
            bands.iter().flat_map(|b| if let Band::Results(rows) = b { rows.as_slice() } else { &[] }).map(|r| r.id.clone()).collect();
    }
    s.sync();
}

/// The found step's Download. Once something is added the step closes and the field empties,
/// so the new row shows where the user looks (owner, 2026-11-18; Electron keeps the card).
pub fn download_found(s: &mut State) {
    let mut found = std::mem::replace(&mut s.found, Found::Nothing);
    if download(s, &mut found) {
        let added = std::mem::take(&mut s.notices);
        dismiss_found(s);
        s.notices = added;
    } else {
        s.found = found;
    }
    s.sync();
}

/// Queues what is chosen in `found`; true when it said "Added to downloads".
fn download(s: &mut State, found: &mut Found) -> bool {
    let audio_or = |s: &State, source| AUDIO_ONLY_SOURCES.contains(&source) || !s.choice.video;
    match found {
        Found::Media(m) => {
            let source = detect_url(&m.url).platform;
            let init = TaskInit {
                url: Some(m.url.clone()),
                source: Some(source),
                title: Some(m.title.clone()),
                uploader: Some(m.uploader.clone()),
                thumbnail_url: Some(m.thumbnail_url.clone()),
                duration: Some(m.duration),
                ..chosen(s, audio_or(s, source))
            };
            queue(s, init);
            notify(s, Tone::Info, fc::TOAST_ADDED.into(), Some(Act::ViewDownloads));
            true
        }
        Found::Search { bands, .. } => {
            let rows: Vec<_> = bands
                .iter()
                .flat_map(|b| if let Band::Results(rows) = b { rows.as_slice() } else { &[] })
                .filter(|r| s.picked.contains(&r.id))
                .cloned()
                .collect();
            if s.plan == Plan::Basic && rows.len() > BASIC_LIMITS.max_batch_links as usize {
                // The ticks stay so the user can trim and send again.
                premium(s, fc::LIMIT_BATCH);
                return false;
            }
            for r in &rows {
                let init = TaskInit {
                    url: Some(r.url.clone()),
                    source: Some(r.source),
                    title: Some(r.title.clone()),
                    uploader: Some(r.uploader.clone()),
                    thumbnail_url: Some(r.thumbnail_url.clone()),
                    duration: Some(r.duration),
                    ..chosen(s, audio_or(s, r.source))
                };
                queue(s, init);
            }
            if !rows.is_empty() {
                s.picked.clear();
                notify(
                    s,
                    Tone::Info,
                    if rows.len() == 1 { fc::TOAST_ADDED.into() } else { fc::toast_added_many(rows.len()) },
                    Some(Act::ViewDownloads),
                );
            }
            !rows.is_empty()
        }
        Found::Playlist { playlist, selected, .. } => {
            let ticked: Vec<usize> = selected.iter().enumerate().filter(|(_, t)| **t).map(|(i, _)| i).collect();
            let cap = BASIC_LIMITS.max_collection_tracks as usize;
            if is_spotify(&playlist.url) {
                let take = if s.plan == Plan::Basic { cap } else { usize::MAX };
                for &i in ticked.iter().take(take) {
                    let t: &Track = &playlist.tracks[i];
                    let init = TaskInit {
                        url: Some(t.spotify_url.clone().unwrap_or_else(|| playlist.url.clone())),
                        source: Some(SourcePlatform::Spotify),
                        title: Some(t.name.clone()),
                        uploader: Some(t.artist.clone()),
                        playlist_name: Some(playlist.name.clone()),
                        thumbnail_url: Some(t.thumbnail_url.clone()),
                        duration: Some(t.duration_ms / 1000.0),
                        track: Some(t.clone()),
                        ..chosen(s, true)
                    };
                    queue(s, init);
                }
                if ticked.len() > take {
                    premium(s, fc::LIMIT_COLLECTION);
                }
            } else if !ticked.is_empty() {
                // One job for the whole playlist: the ticked entries, or the top 20 on Basic.
                let basic = s.plan == Plan::Basic;
                let items = ticked.iter().map(|i| (i + 1).to_string()).collect::<Vec<_>>().join(",");
                let init = TaskInit {
                    url: Some(playlist.url.clone()),
                    source: Some(SourcePlatform::Youtube),
                    is_playlist: Some(true),
                    playlist_name: Some(playlist.name.clone()),
                    title: Some(playlist.name.clone()),
                    playlist_items: (!basic).then_some(items),
                    playlist_limit: basic.then_some(cap as f64),
                    ..chosen(s, true)
                };
                queue(s, init);
            }
            selected.iter_mut().for_each(|t| *t = false);
            false
        }
        _ => false,
    }
}

/// A band that could not be reached, asked again.
pub fn retry_band(s: &mut State, i: usize) {
    if let Found::Search { query, bands } = &mut s.found
        && let Some(band @ Band::Failed) = bands.get_mut(i)
    {
        *band = Band::Loading;
        let query = query.clone();
        live::search_band(s, &query, i);
    }
    s.sync();
}

/// "Show N more" on a band.
pub fn show_more(s: &mut State, i: usize) {
    if let Some(open) = s.expanded.get_mut(i) {
        *open = true;
    }
    s.sync();
}

// ── One download

fn task_mut<'a>(s: &'a mut State, id: &str) -> Option<&'a mut DownloadTask> {
    s.tasks.iter_mut().find(|t| t.task_id == id)
}

pub fn pause(s: &mut State, _tx: &mut Tx, id: &str) {
    if let Some(t) = task_mut(s, id).filter(|t| matches!(t.progress.status, St::Queued | St::FetchingInfo | St::Downloading)) {
        t.progress.status = St::Paused;
        let id = id.to_owned();
        live::tell(s, move |core| {
            core.pause(&id);
        });
    }
    s.sync();
}

pub fn resume(s: &mut State, _tx: &mut Tx, id: &str) {
    if let Some(t) = task_mut(s, id).filter(|t| t.progress.status == St::Paused) {
        t.progress.status = St::Queued;
        t.interrupted = None;
        let id = id.to_owned();
        live::tell(s, move |core| {
            core.resume(&id);
        });
    }
    s.sync();
}

/// Cancels one row; the notice offers Undo.
pub fn cancel(s: &mut State, _tx: &mut Tx, id: &str) {
    cancel_with_undo(s, &[id.to_owned()]);
}

fn cancel_with_undo(s: &mut State, ids: &[String]) {
    let mut done = Vec::new();
    for id in ids {
        if let Some(t) = task_mut(s, id).filter(|t| format::is_active(t)) {
            t.progress.status = St::Cancelled;
            done.push(id.clone());
        }
    }
    let cancelled = done.clone();
    live::tell(s, move |core| {
        cancelled.iter().for_each(|id| {
            core.cancel(id);
        })
    });
    if !done.is_empty() {
        notify(s, Tone::Info, copy::toast_canceled(done.len()), Some(Act::Undo(done)));
    }
    s.sync();
}

/// Queues a failed or canceled row again. In sample mode the row itself
/// starts over; `skip_existing` false is "Download again".
fn requeue(s: &mut State, id: &str, skip_existing: Option<bool>) {
    // Live, the core re-queues the stored request as a new task and the old row goes.
    let again = id.to_owned();
    live::tell(s, move |core| drop(core.retry(&again, skip_existing)));
    if let Some(t) = task_mut(s, id) {
        t.progress = Default::default();
        t.error_code = None;
        t.retry_count = None;
        t.interrupted = None;
        t.completed_at = None;
        if let Some(skip) = skip_existing {
            t.skip_existing = skip;
        }
    }
}

fn retryable(t: &DownloadTask) -> bool {
    matches!(t.progress.status, St::Failed | St::Cancelled) && !format::is_permanent(t.error_code)
}

pub fn retry(s: &mut State, _tx: &mut Tx, id: &str) {
    if s.tasks.iter().any(|t| t.task_id == id && retryable(t)) {
        requeue(s, id, None);
    }
    s.sync();
}

/// A row skipped because the file was already there, downloaded anyway.
pub fn download_again(s: &mut State, _tx: &mut Tx, id: &str) {
    if s.tasks.iter().any(|t| t.task_id == id && t.progress.status == St::Done && t.progress.already_on_disk == Some(true)) {
        requeue(s, id, Some(false));
    }
    s.sync();
}

/// "Do this next": to the front of the queue.
pub fn promote(s: &mut State, _tx: &mut Tx, id: &str) {
    move_queued(s, id, 0);
    // The list snaps back to the core's if it refuses.
    let id = id.to_owned();
    live::tell(s, move |core| {
        core.promote(&id);
    });
}

/// Removes a finished, failed or canceled row from the list and the history.
pub fn remove(s: &mut State, _tx: &mut Tx, id: &str) {
    s.tasks.retain(|t| t.task_id != id || format::is_active(t));
    if !s.tasks.iter().any(|t| t.task_id == id) {
        s.selected.remove(id);
        let id = id.to_owned();
        live::tell(s, move |core| core.remove(vec![id]));
    }
    s.sync();
}

/// The notice for a row with no file to open (`CONTENT.md` 3.8).
fn no_file(s: &mut State, id: &str) -> bool {
    let Some(t) = s.tasks.iter().find(|t| t.task_id == id) else { return true };
    if !t.progress.filename.is_empty() {
        return false;
    }
    let text = copy::file_never_finished(&copy::file_name_subject(&t.title));
    notify(s, Tone::Error, text, None);
    true
}

pub fn open_file(s: &mut State, _tx: &mut Tx, id: &str) {
    if !no_file(s, id) {
        reveal(s, id, true);
    }
    s.sync();
}

/// Shows the file in its folder; nothing at all when there is no file.
pub fn show_in_folder(s: &mut State, _tx: &mut Tx, id: &str) {
    if s.tasks.iter().any(|t| t.task_id == id && !t.progress.filename.is_empty()) {
        reveal(s, id, false);
    }
    s.sync();
}

/// Live: the core shows or opens the file, and its answer becomes Electron's notice (`revealFile`).
fn reveal(s: &State, id: &str, open: bool) {
    let Some(t) = s.tasks.iter().find(|t| t.task_id == id) else { return };
    let (path, subject) = (t.progress.filename.clone(), copy::file_name_subject(&t.title));
    live::command(s, move |core| {
        let text = match if open { core.open_file(&path) } else { core.show_in_folder(&path) } {
            Ok(Shown::Ok) => return None,
            Ok(Shown::NoPath) => copy::file_never_finished(&subject),
            Ok(Shown::Missing) => copy::file_moved(&subject),
            Err(_) => copy::file_cannot_open(&subject),
        };
        Some(Box::new(move |s: &mut State, _: &mut Tx| {
            notify(s, Tone::Error, text, None);
            true
        }))
    });
}

/// Copies a failed row's raw error.
pub fn copy_details(s: &mut State, tx: &mut Tx, id: &str) {
    if let Some(t) = s.tasks.iter().find(|t| t.task_id == id && t.progress.status == St::Failed && !t.progress.error.is_empty()) {
        tx.copy(t.progress.error.clone());
    }
    s.sync();
}

/// The failure's way out: Settings, or an engine update and then a retry.
pub fn fix(s: &mut State, _tx: &mut Tx, id: &str) {
    match s.tasks.iter().find(|t| t.task_id == id).and_then(format::fix) {
        Some(Fix::Settings) => go(s, Nav::Settings),
        Some(Fix::Engine) => {
            notify(s, Tone::Info, copy::TOAST_UPDATING_ENGINE.into(), None);
            update_engine(s);
            // live: when the update reports, its notice, then Core::retry(id).
        }
        None => {}
    }
    s.sync();
}

/// The notice's button: runs its action and dismisses it.
pub fn notice_action(s: &mut State, tx: &mut Tx) {
    let action = s.notices.last().and_then(|n| n.action.clone());
    dismiss_notice(s);
    match action {
        Some(Act::Undo(ids)) => {
            for id in ids {
                requeue(s, &id, None);
            }
        }
        Some(Act::ViewDownloads) => go(s, Nav::Downloads),
        Some(Act::OpenFile(id)) => open_file(s, tx, &id),
        // live: the plans sheet.
        Some(Act::SeePlans) | None => {}
    }
    s.sync();
}

pub fn dismiss_notice(s: &mut State) {
    s.notices.clear();
    s.sync();
}

/// Shows one notice, in place of any on show.
pub(crate) fn notify(s: &mut State, tone: Tone, text: String, action: Option<Act>) {
    s.notices = vec![Notice { tone, text, action, until: None }];
}

fn premium(s: &mut State, text: &str) {
    notify(s, Tone::Premium, text.into(), Some(Act::SeePlans));
    s.sync();
}

/// The saved defaults a new download starts with (`batchDownload`).
fn defaults(s: &State, audio: bool) -> TaskInit {
    let text = |key: &str| s.settings.get(key).and_then(Value::as_str).map(str::to_owned);
    let flag = |key: &str| s.settings.get(key).and_then(Value::as_bool);
    let c = Choice::from_settings(&s.settings);
    TaskInit {
        output_dir: text("outputDir"),
        format: Some(if audio { c.format } else { "mp4".into() }),
        quality: Some(c.audio_quality),
        video_quality: Some(c.video_quality),
        is_audio_only: Some(audio),
        embed_thumbnail: flag("embedThumbnail"),
        embed_metadata: flag("embedMetadata"),
        skip_existing: flag("skipExisting"),
        ..TaskInit::default()
    }
}

/// The defaults with the found step's choice on top.
fn chosen(s: &State, audio: bool) -> TaskInit {
    let c = &s.choice;
    TaskInit {
        output_dir: c.folder.clone().or_else(|| defaults(s, audio).output_dir),
        format: Some(if audio { c.format.clone() } else { "mp4".into() }),
        quality: Some(c.audio_quality.clone()),
        video_quality: Some(c.video_quality),
        ..defaults(s, audio)
    }
}

/// Adds a new download at the top of the list.
fn queue(s: &mut State, mut init: TaskInit) {
    // Live, the core queues it and its row arrives with the core's own id: no stand-in row first.
    if live::start_download(s, &init) {
        return;
    }
    let id = (0..).map(|n| format!("new-{n}")).find(|id| !s.tasks.iter().any(|t| &t.task_id == id)).unwrap_or_default();
    init.created_at = Some(crate::sample::NOW);
    s.tasks.insert(0, create_task(&init, id, crate::sample::NOW as u64));
}

// ── Many downloads

/// Pause all, or Resume all when everything is paused.
pub fn pause_or_resume_all(s: &mut State) {
    let to = if format::all_paused(&s.tasks) { St::Queued } else { St::Paused };
    set_all(s, to);
    s.sync();
}

/// `pauseAll` / `resumeAll`.
fn set_all(s: &mut State, to: St) {
    // ponytail: the core's pause_all / resume_all cannot refuse, so TOAST_LIST_UNREACHABLE has no live cause yet.
    let pause = to == St::Paused;
    live::tell(s, move |core| if pause { core.pause_all() } else { core.resume_all() });
    for t in &mut s.tasks {
        let from_ok = if to == St::Paused {
            matches!(t.progress.status, St::Queued | St::FetchingInfo | St::Downloading)
        } else {
            t.progress.status == St::Paused
        };
        if from_ok {
            t.progress.status = to;
            if to == St::Queued {
                t.interrupted = None;
            }
        }
    }
}

/// Downloads' Cancel all, with Undo.
pub fn cancel_all(s: &mut State) {
    let ids: Vec<String> = s.tasks.iter().filter(|t| format::is_active(t)).map(|t| t.task_id.clone()).collect();
    cancel_with_undo(s, &ids);
}

/// Failed rows only (Electron's `retryable`); a canceled row keeps its own Retry.
pub fn retry_all_failed(s: &mut State) {
    let ids: Vec<String> = s.tasks.iter().filter(|t| t.progress.status == St::Failed && retryable(t)).map(|t| t.task_id.clone()).collect();
    for id in ids {
        requeue(s, &id, None);
    }
    s.sync();
}

pub fn arm(s: &mut State, c: Confirm) {
    s.confirm = Some(c);
    s.sync();
}

/// Does what is armed.
pub fn confirm(s: &mut State) {
    match s.confirm.take() {
        Some(Confirm::Clear) => clear_completed(s),
        Some(Confirm::Remove) => {
            let ids: Vec<String> = s.selected.iter().cloned().collect();
            for id in ids {
                remove(s, &mut Tx::default(), &id);
            }
        }
        // live: the licence client (deactivate, release).
        Some(Confirm::Deactivate | Confirm::Move) | None => {}
    }
    s.sync();
}

fn clear_completed(s: &mut State) {
    // Core has no clear_completed: the finished rows are removed by id.
    let done: Vec<String> = s.tasks.iter().filter(|t| t.progress.status == St::Done).map(|t| t.task_id.clone()).collect();
    live::tell(s, move |core| core.remove(done));
    s.tasks.retain(|t| t.progress.status != St::Done);
    s.selected.retain(|id| s.tasks.iter().any(|t| &t.task_id == id));
}

pub fn disarm(s: &mut State) {
    s.confirm = None;
    s.sync();
}

fn interrupted(t: &DownloadTask) -> bool {
    t.interrupted == Some(true) && t.progress.status == St::Paused
}

pub fn resume_interrupted(s: &mut State) {
    let ids: Vec<String> = s.tasks.iter().filter(|t| interrupted(t)).map(|t| t.task_id.clone()).collect();
    for id in ids {
        resume(s, &mut Tx::default(), &id);
    }
    s.sync();
}

pub fn discard_interrupted(s: &mut State) {
    let gone: Vec<String> = s.tasks.iter().filter(|t| interrupted(t)).map(|t| t.task_id.clone()).collect();
    live::tell(s, move |core| core.remove(gone));
    s.tasks.retain(|t| !interrupted(t));
    s.selected.retain(|id| s.tasks.iter().any(|t| &t.task_id == id));
    s.sync();
}

pub fn set_filter(s: &mut State, f: Filter) {
    s.filter = f;
    s.sync();
}

pub fn toggle_select(s: &mut State, id: &str) {
    if !s.selected.remove(id) {
        s.selected.insert(id.into());
    }
    s.sync();
}

/// Downloads' selection let go.
pub fn clear_selection(s: &mut State) {
    s.selected.clear();
    s.sync();
}

/// The filtered-empty state's "Show all": the filter to All and the search emptied.
pub fn show_all(s: &mut State) {
    s.filter = Filter::All;
    s.query.replace_text("");
    s.sync();
}

/// A press on a row's tick: it toggles; with Shift, every row from the last pick to this one is ticked.
pub fn pick_row(s: &mut State, id: &str, mods: Mods) {
    let shown: Vec<String> = visible(s).map(|t| t.task_id.clone()).collect();
    let anchor = s.last_picked.as_deref().and_then(|a| shown.iter().position(|t| t == a));
    match (mods.shift, anchor, shown.iter().position(|t| t == id)) {
        (true, Some(from), Some(to)) => s.selected.extend(shown[from.min(to)..=from.max(to)].iter().cloned()),
        _ => {
            if !s.selected.remove(id) {
                s.selected.insert(id.into());
            }
        }
    }
    if !mods.shift {
        s.last_picked = Some(id.into());
    }
    s.sync();
}

/// The rows on show: the filter, then the search over title, uploader and link.
pub fn visible(s: &State) -> impl Iterator<Item = &DownloadTask> {
    let q = js_trim(&s.query.text).to_lowercase();
    s.tasks.iter().filter(move |t| {
        s.filter.passes(t) && (q.is_empty() || format!("{} {} {}", t.title, t.uploader, t.url).to_lowercase().contains(&q))
    })
}

/// Every row on show ticked, or the selection cleared when they all are.
pub fn toggle_select_all(s: &mut State) {
    let shown: Vec<String> = visible(s).map(|t| t.task_id.clone()).collect();
    if !shown.is_empty() && shown.iter().all(|id| s.selected.contains(id)) {
        s.selected.clear()
    } else {
        s.selected = shown.into_iter().collect()
    }
    s.sync();
}

/// Cancels the selected active rows, deselecting them; no Undo.
pub fn cancel_selected(s: &mut State) {
    let ids: Vec<String> =
        s.tasks.iter().filter(|t| s.selected.contains(&t.task_id) && format::is_active(t)).map(|t| t.task_id.clone()).collect();
    for id in ids {
        if let Some(t) = task_mut(s, &id) {
            t.progress.status = St::Cancelled;
        }
        s.selected.remove(&id);
        live::tell(s, move |core| {
            core.cancel(&id);
        });
    }
    s.sync();
}

/// Moves a queued row to place `to` of the queued block.
pub fn move_queued(s: &mut State, id: &str, to: usize) {
    // Live, the core is told the new order; the list snaps back if it refuses.
    let slots: Vec<usize> = s.tasks.iter().enumerate().filter(|(_, t)| t.progress.status == St::Queued).map(|(i, _)| i).collect();
    let mut order: Vec<DownloadTask> = slots.iter().map(|&i| s.tasks[i].clone()).collect();
    if let Some(from) = order.iter().position(|t| t.task_id == id) {
        let t = order.remove(from);
        order.insert(to.min(order.len()), t);
        // The queued block's first row runs next (as `promote` reads it), as `Core::reorder` takes ids.
        let ids: Vec<String> = order.iter().map(|t| t.task_id.clone()).collect();
        for (slot, t) in slots.into_iter().zip(order) {
            s.tasks[slot] = t;
        }
        live::tell(s, move |core| core.reorder(ids));
    }
    s.sync();
}

// ── Settings

/// Writes one setting. A refused value comes back `false` and its key is
/// marked in `invalid`; a good one clears the mark.
// TODO(engine-export): `ConfigManager::is_valid_value` is private, so this
// checks the key and the JSON type against the defaults only (plus the
// palette ids); live mode goes through the core, which checks in full.
pub fn set_setting(s: &mut State, key: &str, value: Value) -> bool {
    let defaults = ahg_engine::config::default_config();
    let same_type = |a: &Value, b: &Value| std::mem::discriminant(a) == std::mem::discriminant(b);
    let ok = match (defaults.get(key), &value) {
        (None, _) => false,
        (Some(Value::Array(_)), Value::Array(items)) => {
            items.iter().all(|v| if key == "sponsorBlockCategories" { v.is_string() } else { v.is_number() })
        }
        (_, Value::String(p)) if key == "palette" => ahg_engine::config::PALETTE_IDS.contains(&p.as_str()),
        (Some(d), v) => same_type(d, v),
    };
    if ok {
        s.settings.insert(key.into(), value.clone());
        s.invalid.remove(key);
        // Live, the core checks in full; a refusal marks the row and puts the old value back.
        let key = key.to_owned();
        live::command(s, move |core| {
            let refused = !core.update_settings(&serde_json::Map::from_iter([(key.clone(), value)])).is_empty();
            refused.then(|| -> live::Job {
                Box::new(move |s, _| {
                    s.invalid.insert(key);
                    true
                })
            })
        });
    } else {
        s.invalid.insert(key.into());
    }
    s.sync();
    ok
}

/// The editor of a Settings text field, made from the setting on first use.
pub fn edit_text<'a>(s: &'a mut State, key: &str) -> &'a mut TextEdit {
    let now = s.settings.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
    s.edits.entry(key.into()).or_insert_with(|| {
        let mut e = TextEdit::default();
        e.replace_text(&now);
        e
    })
}

/// Writes the edited text as the setting. A refusal leaves the text and the key in `invalid`;
/// a good value resets the edit from the setting.
pub fn commit_text(s: &mut State, key: &str) -> bool {
    let text = edit_text(s, key).text.clone();
    let ok = set_setting(s, key, Value::String(text));
    if ok {
        s.edits.remove(key);
    }
    ok
}

/// A select's choice as the setting. A locked one says why on Basic and writes nothing.
pub fn pick_setting(s: &mut State, _tx: &mut Tx, key: &str, value: &str) -> bool {
    if let Some(why) = select_options(s, key).into_iter().find(|o| o.value == value).and_then(|o| o.lock) {
        premium(s, &why);
        return false;
    }
    // A number setting (the simultaneous downloads) is picked by its digits.
    let number = ahg_engine::config::default_config().get(key).is_some_and(Value::is_number);
    let v = match value.parse::<u64>() {
        Ok(n) if number => n.into(),
        _ => value.into(),
    };
    set_setting(s, key, v)
}

/// A select row's choices with their locks, for a popup.
pub fn select_options(s: &State, key: &str) -> Vec<Opt> {
    settings_spec::sections(s)
        .into_iter()
        .flat_map(|sec| sec.rows)
        .find_map(|r| match r.kind {
            Kind::Select(o) if r.key == Some(key) => Some(o),
            _ => None,
        })
        .unwrap_or_default()
}

/// `light`, `dark` or `system`. Nothing is re-keyed: only the colours spring.
pub fn set_theme(s: &mut State, theme: &str, tx: &mut Tx) {
    motion::recolour(tx);
    set_setting(s, "theme", theme.into());
}

/// 0 is the look's own mood (Rust only); 1 to 7 are Electron's palettes,
/// written to `palette`. Palette 0 leaves that key as it was, so Electron
/// 1.x can still read it. Nothing is re-keyed: only the colours spring.
pub fn set_palette(s: &mut State, i: usize, tx: &mut Tx) {
    motion::recolour(tx);
    s.palette = i.min(ahg_engine::config::PALETTE_IDS.len());
    if let Some(id) = s.palette.checked_sub(1).map(|at| ahg_engine::config::PALETTE_IDS[at]) {
        set_setting(s, "palette", id.into());
    }
    s.sync();
}

/// A weekday chip of the schedule: added at the end, or taken out.
pub fn toggle_day(s: &mut State, i: u8) {
    let mut days: Vec<Value> = s.settings.get("scheduleDays").and_then(Value::as_array).cloned().unwrap_or_default();
    let before = days.len();
    days.retain(|d| d.as_u64() != Some(u64::from(i)));
    if days.len() == before {
        days.push(i.into());
    }
    set_setting(s, "scheduleDays", Value::Array(days));
}

/// Live: the folder dialog, then the folder as `outputDir`. Sample mode shows no dialog.
pub fn choose_folder(s: &mut State, tx: &mut Tx) {
    if s.live.is_some() {
        tx.open_folder(rune::FileDialogOptions::default(), |s: &mut State, chosen, _| {
            if let Ok(Some(folder)) = chosen {
                set_setting(s, "outputDir", folder.to_string_lossy().into_owned().into());
            }
        });
    }
    s.sync();
}

pub fn import_cookies(s: &mut State, tx: &mut Tx) {
    if s.plan == Plan::Basic {
        return premium(s, LOCK_COOKIES);
    }
    // Live: the file dialog, then the core's import (which checks the file in full).
    if s.live.is_some() {
        tx.open_file(rune::FileDialogOptions::default(), |s: &mut State, chosen, _| {
            let Ok(Some(file)) = chosen else { return };
            live::command(s, move |core| match core.import_cookies(Some(&file)) {
                CookiesImport::Ok => Some(Box::new(|s, _| {
                    notify(s, Tone::Info, copy::settings::COOKIES_IMPORTED.into(), None);
                    true
                })),
                CookiesImport::Premium => Some(Box::new(|s, _| {
                    premium(s, LOCK_COOKIES);
                    true
                })),
                // ponytail: copy has no words for a refused cookies file yet, so nothing shows.
                CookiesImport::Cancelled | CookiesImport::Invalid => None,
            });
        });
    }
    s.sync();
}

pub fn check_update(s: &mut State) {
    s.update.phase = "checking";
    // Live, the core's events set `update`.
    live::spawn(s, |core| drop(core.check_update()));
    s.sync();
}

pub fn download_update(s: &mut State) {
    if s.update.phase == "available" {
        s.update.phase = "downloading";
        live::spawn(s, |core| drop(core.download_update()));
    }
    s.sync();
}

pub fn install_update(s: &mut State, _tx: &mut Tx) {
    // live: Core swaps the EXE (D33) and the app restarts.
    s.sync();
}

pub fn check_engine(s: &mut State) {
    s.engine.state = "checking";
    // Live, yt-dlp is checked ignoring the 24 h cache; its status sets `engine`.
    live::spawn(s, |core| drop(core.check_engine(true)));
    s.sync();
}

pub fn update_engine(s: &mut State) {
    s.engine.state = "downloading";
    live::spawn(s, |core| drop(core.update_engine()));
    s.sync();
}

pub fn show_shortcuts(s: &mut State) {
    open(s, Overlay::Shortcuts);
}

pub fn get_support(s: &mut State, _tx: &mut Tx) {
    // live: opens the support page.
    s.sync();
}

#[cfg(test)]
mod tests {
    use ahg_engine::tier::Plan;
    use ahg_engine::types::{DownloadStatus as St, DownloadTask, SourcePlatform, VideoQuality};
    use rune::{Key, KeyEvent, Mods, Tx};
    use serde_json::{Value, json};

    use super::*;
    use crate::copy;
    use crate::copy::found as fc;
    use crate::gate::{self, Act, Band, Card, Choice, Confirm, Filter, Found, Matched, Nav, Overlay, Screen, State, Tone};
    use crate::sample;

    fn state() -> State {
        State::sample(0)
    }
    fn task<'a>(s: &'a State, id: &str) -> &'a DownloadTask {
        s.tasks.iter().find(|t| t.task_id == id).unwrap_or_else(|| panic!("no task {id}"))
    }
    fn status(s: &State, id: &str) -> St {
        task(s, id).progress.status
    }
    fn has(s: &State, id: &str) -> bool {
        s.tasks.iter().any(|t| t.task_id == id)
    }
    fn ctrl(c: char) -> KeyEvent {
        KeyEvent { key: Key::Command(c), mods: Mods { cmd: true, ctrl: true, ..Mods::default() } }
    }
    fn esc() -> KeyEvent {
        KeyEvent { key: Key::Escape, mods: Mods::default() }
    }
    fn press(s: &mut State, ev: KeyEvent) -> bool {
        gate::on_key(s, &ev, &mut Tx::default())
    }
    fn notice(s: &State) -> &gate::Notice {
        s.notices.last().expect("a notice")
    }
    fn searching(s: &mut State) {
        s.set_field(sample::QUERY);
        s.found = Found::Search { query: sample::QUERY.into(), bands: sample::search().map(Band::Results) };
    }
    fn playlist(s: &mut State, youtube: bool) {
        let p = if youtube { sample::youtube_playlist() } else { sample::playlist() };
        let n = p.tracks.len();
        s.found = Found::Playlist { playlist: p, matched: vec![Matched::Confirmed; n], selected: vec![youtube; n] };
    }
    fn selected(s: &State) -> Vec<bool> {
        match &s.found {
            Found::Playlist { selected, .. } => selected.clone(),
            _ => panic!("not a playlist"),
        }
    }
    /// Sets two more rows to queued, so the queued block is [soundcloud-set, dreams, humble].
    fn three_queued(s: &mut State) {
        for id in ["dreams", "humble"] {
            let t = s.tasks.iter_mut().find(|t| t.task_id == id).unwrap();
            t.progress = Default::default();
        }
    }
    fn queued_order(s: &State) -> Vec<String> {
        s.tasks.iter().filter(|t| t.progress.status == St::Queued).map(|t| t.task_id.clone()).collect()
    }

    // ── State::sample and sync

    #[test]
    fn sample_has_every_preset_and_the_settings_defaults() {
        for i in 0..gate::PRESETS.len() {
            let s = State::sample(i);
            assert_eq!(gate::preset(&s), i);
            assert_eq!(s.own.preset, i);
        }
        let s = state();
        assert_eq!(s.settings.get("outputDir"), Some(&json!(sample::OUTPUT_DIR)));
        assert_eq!(s.settings.get("theme"), Some(&json!("system")));
        assert_eq!(s.choice, Choice::from_settings(&s.settings));
        assert_eq!(s.choice.video_quality, VideoQuality::Best);
    }

    #[test]
    fn sync_lets_the_old_fields_win() {
        let mut s = state();
        s.picker = true;
        s.nav_pinned = true;
        s.sync();
        assert_eq!(s.overlay, Overlay::Picker);
        assert!(s.own.nav_pinned);
        s.picker = false;
        s.sync();
        assert_eq!(s.overlay, Overlay::None);
        // A notice keeps its tone and action through sync.
        premium(&mut s, fc::LIMIT_BATCH);
        s.sync();
        assert_eq!((notice(&s).tone, notice(&s).action.clone()), (Tone::Premium, Some(Act::SeePlans)));
    }

    // ── on_key, in the order of CONTENT.md 1.3

    #[test]
    fn ctrl_k_toggles_the_palette_and_the_palette_owns_the_keys() {
        let mut s = state();
        assert!(press(&mut s, ctrl('k')));
        assert_eq!(s.overlay, Overlay::Palette);
        assert!(!press(&mut s, ctrl('2')), "Ctrl+2 does not get through the palette");
        assert!(!press(&mut s, ctrl('/')));
        assert_eq!((s.nav, s.overlay), (Nav::Home, Overlay::Palette));
        assert!(press(&mut s, ctrl('k')));
        assert_eq!(s.overlay, Overlay::None);
        press(&mut s, ctrl('k'));
        assert!(press(&mut s, esc()));
        assert_eq!(s.overlay, Overlay::None);
    }

    #[test]
    fn ctrl_v_in_the_palette_runs_paste_and_download() {
        let mut s = state();
        open(&mut s, Overlay::Palette);
        assert!(press(&mut s, ctrl('v')));
        assert_eq!(s.overlay, Overlay::None, "running a command closes the palette");
    }

    #[test]
    fn ctrl_slash_toggles_the_shortcuts_sheet() {
        let mut s = state();
        assert!(press(&mut s, ctrl('/')));
        assert_eq!(s.overlay, Overlay::Shortcuts);
        assert!(press(&mut s, ctrl('/')));
        assert_eq!(s.overlay, Overlay::None);
    }

    #[test]
    fn ctrl_dot_toggles_the_picker() {
        let mut s = state();
        assert!(press(&mut s, ctrl('.')));
        assert!(s.picker);
        assert_eq!(s.overlay, Overlay::Picker);
        assert!(press(&mut s, ctrl('.')));
        assert!(!s.picker);
    }

    #[test]
    fn ctrl_1_to_3_go_to_the_three_places() {
        let cases = [('2', Nav::Downloads, Screen::Downloads), ('3', Nav::Settings, Screen::Home), ('1', Nav::Home, Screen::Home)];
        assert!(!cases.is_empty());
        let mut s = state();
        for (c, nav, screen) in cases {
            assert!(press(&mut s, ctrl(c)));
            assert_eq!((s.nav, s.screen, s.reach), (nav, screen, nav == Nav::Settings), "Ctrl+{c}");
        }
    }

    #[test]
    fn ctrl_4_to_7_set_the_scope_and_go_home() {
        let cases = [
            ('4', SourcePlatform::Youtube),
            ('5', SourcePlatform::Spotify),
            ('6', SourcePlatform::Soundcloud),
            ('7', SourcePlatform::Instagram),
        ];
        assert!(!cases.is_empty());
        for (c, scope) in cases {
            let mut s = state();
            go(&mut s, Nav::Downloads);
            assert!(press(&mut s, ctrl(c)));
            assert_eq!((s.scope, s.nav), (Some(scope), Nav::Home), "Ctrl+{c}");
        }
        assert!(!press(&mut state(), ctrl('8')));
    }

    #[test]
    fn ctrl_v_pastes_on_home_only() {
        let mut s = state();
        assert!(press(&mut s, ctrl('v')));
        go(&mut s, Nav::Downloads);
        assert!(!press(&mut s, ctrl('v')));
    }

    #[test]
    fn ctrl_a_picks_every_search_result() {
        let mut s = state();
        assert!(!press(&mut s, ctrl('a')), "nothing to pick");
        searching(&mut s);
        assert!(press(&mut s, ctrl('a')));
        assert_eq!(s.picked.len(), sample::search().iter().map(Vec::len).sum::<usize>());
    }

    #[test]
    fn escape_closes_an_overlay_then_disarms_then_clears_what_was_found() {
        let mut s = state();
        searching(&mut s);
        notify(&mut s, Tone::Info, "note".into(), None);
        arm(&mut s, Confirm::Clear);
        open(&mut s, Overlay::Picker);
        assert!(press(&mut s, esc()));
        assert!(!s.picker);
        assert_eq!(s.confirm, Some(Confirm::Clear));
        assert!(press(&mut s, esc()));
        assert_eq!(s.confirm, None);
        assert!(matches!(s.found, Found::Search { .. }));
        assert!(press(&mut s, esc()));
        assert!(matches!(s.found, Found::Nothing));
        assert_eq!((s.input.as_str(), s.field.text.as_str(), s.notices.is_empty()), ("", "", true));
        assert!(!press(&mut s, esc()), "nothing left to close");
    }

    // ── Places and overlays

    #[test]
    fn go_sets_the_place_and_the_old_screen() {
        let mut s = state();
        go(&mut s, Nav::Downloads);
        assert_eq!((s.nav, s.screen, s.reach), (Nav::Downloads, Screen::Downloads, false));
        go(&mut s, Nav::Settings);
        assert_eq!((s.nav, s.screen, s.reach), (Nav::Settings, Screen::Home, true));
    }

    #[test]
    fn open_and_close_an_overlay() {
        let mut s = state();
        open(&mut s, Overlay::Picker);
        assert!(s.picker);
        open(&mut s, Overlay::Palette);
        assert!(!s.picker);
        assert_eq!(s.overlay, Overlay::Palette);
        close(&mut s);
        assert_eq!(s.overlay, Overlay::None);
    }

    #[test]
    fn set_preset_switches_look_and_layout() {
        let mut s = state();
        set_preset(&mut s, 4, &mut Tx::default());
        assert_eq!((s.direction, s.layout, s.own.preset), (gate::Direction::Shelf, None, 4));
        set_preset(&mut s, 6, &mut Tx::default());
        assert_eq!(s.own.preset, 0, "wraps");
    }

    #[test]
    fn toggle_pin_flips_the_sidebar_pin() {
        let mut s = state();
        toggle_pin(&mut s);
        assert!(s.nav_pinned && s.own.nav_pinned);
        toggle_pin(&mut s);
        assert!(!s.nav_pinned && !s.own.nav_pinned);
    }

    #[test]
    fn commands_are_the_palette_rows_in_order() {
        let mut s = state();
        let all = commands(&s);
        assert_eq!(all.len(), 7 + 9);
        assert_eq!(all[0], Command::Go(Nav::Home));
        assert_eq!(all[3], Command::Scope(SourcePlatform::Youtube));
        assert_eq!(all[7], Command::PasteAndDownload);
        assert_eq!(all[8].label(&s), copy::PALETTE_SWITCH_TO_LIGHT, "theme system");
        set_theme(&mut s, "light", &mut Tx::default());
        assert_eq!(Command::ToggleTheme.label(&s), copy::PALETTE_SWITCH_TO_DARK);
        assert_eq!(all[0].label(&s), copy::palette_go_to(copy::NAV_HOME));
        assert_eq!(all[6].label(&s), copy::palette_go_to(copy::NAV_REELS));
        assert_eq!(all[6].hint().as_deref(), Some("Ctrl+7"));
        assert_eq!(Command::Shortcuts.hint().as_deref(), Some("Ctrl+/"));
        assert_eq!(all[0].group(), copy::PALETTE_GROUP_NAVIGATE);
        assert_eq!(all[15], Command::CancelAll);
    }

    #[test]
    fn run_closes_the_palette_and_does_the_command() {
        let mut s = state();
        open(&mut s, Overlay::Palette);
        run(&mut s, &mut Tx::default(), Command::Go(Nav::Downloads));
        assert_eq!((s.overlay, s.nav), (Overlay::None, Nav::Downloads));
        run(&mut s, &mut Tx::default(), Command::Scope(SourcePlatform::Spotify));
        assert_eq!((s.scope, s.nav), (Some(SourcePlatform::Spotify), Nav::Home));
        run(&mut s, &mut Tx::default(), Command::ToggleTheme);
        assert_eq!(s.settings["theme"], json!("light"));
        run(&mut s, &mut Tx::default(), Command::ClearCompleted);
        assert!(!has(&s, "dreams"));
        run(&mut s, &mut Tx::default(), Command::PauseAll);
        assert_eq!(status(&s, "big-buck-bunny"), St::Paused);
        run(&mut s, &mut Tx::default(), Command::ResumeAll);
        assert_eq!(status(&s, "big-buck-bunny"), St::Queued);
        run(&mut s, &mut Tx::default(), Command::CancelAll);
        assert_eq!(status(&s, "big-buck-bunny"), St::Cancelled);
        assert!(s.notices.is_empty(), "the palette's Cancel all offers no Undo");
        run(&mut s, &mut Tx::default(), Command::Shortcuts);
        assert_eq!(s.overlay, Overlay::Shortcuts);
        run(&mut s, &mut Tx::default(), Command::CheckUpdates);
        assert_eq!(s.update.phase, "checking");
    }

    #[test]
    fn paste_and_download_queues_the_clipboard_link() {
        let mut s = state();
        let before = s.tasks.len();
        paste_and_download(&mut s, "  ");
        assert_eq!((notice(&s).tone, notice(&s).text.as_str()), (Tone::Error, copy::PALETTE_CLIPBOARD_EMPTY));
        paste_and_download(&mut s, "https://soundcloud.com/artist/track");
        assert_eq!(s.tasks.len(), before + 1);
        let t = &s.tasks[0];
        assert_eq!((t.source, t.is_audio_only, t.format.as_str()), (SourcePlatform::Soundcloud, true, "mp3"));
        paste_and_download(&mut s, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!((s.tasks[0].is_audio_only, s.tasks[0].format.as_str()), (false, "mp4"));
        paste_and_download(&mut s, "spotify:nothing");
        assert_eq!(notice(&s).tone, Tone::Error);
        assert_eq!(s.tasks.len(), before + 2);
    }

    // ── The field: the decision tree of 05-tabs.md 1.4

    #[derive(Debug, PartialEq)]
    enum Outcome {
        Nothing,
        Search,
        LookingUp,
        Queued(usize),
        Failed(&'static str),
        Limit,
    }

    fn outcome(input: &str, plan: Plan) -> Outcome {
        let mut s = state();
        s.plan = plan;
        let before = s.tasks.len();
        s.set_field(input);
        submit(&mut s, &mut Tx::default());
        if s.tasks.len() > before {
            assert_eq!(s.nav, Nav::Downloads, "a batch goes to Downloads");
            return Outcome::Queued(s.tasks.len() - before);
        }
        if s.notices.last().is_some_and(|n| n.tone == Tone::Premium) {
            return Outcome::Limit;
        }
        match &s.found {
            Found::Nothing => Outcome::Nothing,
            Found::Search { query, bands } => {
                assert_eq!(query, input.trim());
                assert!(bands.iter().all(|b| matches!(b, Band::Loading)));
                Outcome::Search
            }
            Found::LookingUp => Outcome::LookingUp,
            Found::LookupFailed(why) if why == copy::ERR_NOT_SUPPORTED => Outcome::Failed("unsupported"),
            Found::LookupFailed(why) if why == copy::ERR_NO_LINKS => Outcome::Failed("no links"),
            _ => panic!("unexpected {input}"),
        }
    }

    #[test]
    fn the_field_decides_between_several_links_words_and_one_link() {
        use Outcome::*;
        let six = (1..=6).map(|i| format!("https://vimeo.com/{i}")).collect::<Vec<_>>().join(" ");
        let cases: Vec<(String, Plan, Outcome)> = vec![
            ("".into(), Plan::Premium, Nothing),
            ("   ".into(), Plan::Premium, Nothing),
            // Words: a search from two characters; one is ignored.
            ("k".into(), Plan::Premium, Nothing),
            ("kh".into(), Plan::Premium, Search),
            ("  khruangbin  ".into(), Plan::Premium, Search),
            ("texas sun".into(), Plan::Premium, Search),
            // Several links, every part a link: a batch with the saved defaults.
            ("https://vimeo.com/1 https://www.youtube.com/watch?v=dQw4w9WgXcQ".into(), Plan::Premium, Queued(2)),
            ("https://vimeo.com/1\nhttps://vimeo.com/2\thttps://vimeo.com/3".into(), Plan::Premium, Queued(3)),
            // A Spotify collection is not batched; the rest is.
            ("https://open.spotify.com/album/abc https://vimeo.com/1".into(), Plan::Premium, Queued(1)),
            ("https://open.spotify.com/album/abc https://open.spotify.com/playlist/def".into(), Plan::Premium, Failed("no links")),
            (six.clone(), Plan::Premium, Queued(6)),
            (six, Plan::Basic, Limit),
            // One link of a source with a place of its own: it is looked up.
            ("https://www.youtube.com/watch?v=dQw4w9WgXcQ".into(), Plan::Premium, LookingUp),
            ("youtu.be/dQw4w9WgXcQ".into(), Plan::Premium, LookingUp),
            ("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC".into(), Plan::Premium, LookingUp),
            ("https://soundcloud.com/artist/track".into(), Plan::Premium, LookingUp),
            (sample::SHORT_URL.into(), Plan::Premium, LookingUp),
            ("https://www.tiktok.com/@someone/video/7123456789012345678".into(), Plan::Premium, LookingUp),
            // Any other supported link is queued at once.
            ("https://vimeo.com/123456".into(), Plan::Premium, Queued(1)),
            ("https://example.com/clip.mp4".into(), Plan::Premium, Queued(1)),
            // A link nothing knows.
            ("spotify:nothing".into(), Plan::Premium, Failed("unsupported")),
        ];
        assert!(!cases.is_empty());
        for (input, plan, want) in cases {
            assert_eq!(outcome(&input, plan), want, "{input:?} on {plan:?}");
        }
    }

    #[test]
    fn looks_like_url_is_electrons_pattern() {
        let cases = [
            ("https://x.com", true),
            ("HTTP://X", true),
            ("spotify:track:1", true),
            ("Spotify:x", true),
            ("youtube.com", true),
            ("my-site.io/path", true),
            ("  youtube.com  ", true),
            ("khruangbin", false),
            ("a.b", false),
            ("texas sun", false),
            ("://x", false),
            ("é.com", false),
        ];
        assert!(!cases.is_empty());
        for (text, want) in cases {
            assert_eq!(looks_like_url(text), want, "{text:?}");
        }
    }

    #[test]
    fn submit_resets_the_found_step() {
        let mut s = state();
        s.picked.insert("yt-1".into());
        s.expanded = [true; 3];
        s.choice.video = false;
        s.card = Card::Short { label: "x".into() };
        s.set_field("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        submit(&mut s, &mut Tx::default());
        assert!(s.picked.is_empty());
        assert_eq!(s.expanded, [false; 3]);
        assert_eq!(s.choice, Choice::from_settings(&s.settings));
        assert_eq!(s.card, Card::default());
    }

    #[test]
    fn a_scoped_search_asks_one_source() {
        let cases =
            [(SourcePlatform::Youtube, 0), (SourcePlatform::Soundcloud, 1), (SourcePlatform::Spotify, 2), (SourcePlatform::Instagram, 0)];
        assert!(!cases.is_empty());
        for (scope, band) in cases {
            let mut s = state();
            set_scope(&mut s, Some(scope));
            s.set_field("khruangbin");
            submit(&mut s, &mut Tx::default());
            let Found::Search { bands, .. } = &s.found else { panic!("no search") };
            for (i, b) in bands.iter().enumerate() {
                assert_eq!(matches!(b, Band::Loading), i == band, "{scope:?} band {i}");
            }
        }
    }

    #[test]
    fn paste_text_fills_the_field_and_submits() {
        let mut s = state();
        paste_text(&mut s, &mut Tx::default(), "   ");
        assert!(matches!(s.found, Found::Nothing));
        assert_eq!(s.field.text, "");
        paste_text(&mut s, &mut Tx::default(), " khruangbin \n");
        assert_eq!((s.field.text.as_str(), s.input.as_str()), ("khruangbin", "khruangbin"));
        assert!(matches!(s.found, Found::Search { .. }));
    }

    #[test]
    fn paste_asks_for_the_clipboard_and_changes_nothing_yet() {
        let mut s = state();
        paste(&mut s, &mut Tx::default());
        assert!(matches!(s.found, Found::Nothing));
    }

    #[test]
    fn set_scope_sets_and_clears_the_chip() {
        let mut s = state();
        set_scope(&mut s, Some(SourcePlatform::Spotify));
        assert_eq!(s.scope, Some(SourcePlatform::Spotify));
        set_scope(&mut s, None);
        assert_eq!(s.scope, None);
    }

    #[test]
    fn dismiss_found_clears_found_field_and_notice() {
        let mut s = state();
        searching(&mut s);
        notify(&mut s, Tone::Info, "x".into(), None);
        dismiss_found(&mut s);
        assert!(matches!(s.found, Found::Nothing));
        assert_eq!((s.input.as_str(), s.field.text.as_str(), s.notices.is_empty()), ("", "", true));
    }

    // ── The found step

    #[test]
    fn set_choice_saves_the_defaults_except_on_a_playlist() {
        let mut s = state();
        s.found = Found::Media(sample::media());
        let c = Choice {
            video: false,
            format: "flac".into(),
            audio_quality: "lossless".into(),
            video_quality: VideoQuality::P720,
            folder: None,
        };
        set_choice(&mut s, c.clone());
        assert_eq!(s.choice, c);
        assert_eq!(
            (&s.settings["defaultFormat"], &s.settings["defaultQuality"], &s.settings["defaultVideoQuality"]),
            (&json!("flac"), &json!("lossless"), &json!("720p"))
        );
        let mut s = state();
        playlist(&mut s, false);
        set_choice(&mut s, c);
        assert_eq!(s.settings["defaultFormat"], json!("mp3"));
    }

    #[test]
    fn set_choice_on_basic_refuses_a_locked_option() {
        let base = Choice::from_settings(&sample::settings());
        let cases: Vec<(Choice, String)> = vec![
            (Choice { format: "flac".into(), ..base.clone() }, fc::lock_format("FLAC")),
            (Choice { audio_quality: "lossless".into(), ..base.clone() }, fc::LOCK_LOSSLESS_QUALITY.into()),
            (Choice { video_quality: VideoQuality::P2160, ..base.clone() }, fc::lock_video_quality("2160p")),
            (Choice { video_quality: VideoQuality::Best, ..base.clone() }, fc::lock_video_quality("best")),
        ];
        assert!(!cases.is_empty());
        for (c, why) in cases {
            let mut s = state();
            s.plan = Plan::Basic;
            s.choice.video_quality = VideoQuality::P720;
            s.found = Found::Media(sample::media());
            let before = (s.choice.clone(), s.settings.clone());
            set_choice(&mut s, c);
            assert_eq!((s.choice.clone(), s.settings.clone()), before, "{why}");
            assert_eq!((notice(&s).tone, notice(&s).text.as_str()), (Tone::Premium, why.as_str()));
        }
    }

    #[test]
    fn toggle_track_ticks_one_row_but_not_on_a_basic_youtube_playlist() {
        let mut s = state();
        playlist(&mut s, false);
        toggle_track(&mut s, 2);
        assert!(selected(&s)[2]);
        toggle_track(&mut s, 2);
        assert!(!selected(&s)[2]);
        s.plan = Plan::Basic;
        playlist(&mut s, true);
        toggle_track(&mut s, 2);
        assert!(selected(&s)[2], "locked: stays ticked");
        assert_eq!((notice(&s).tone, notice(&s).text.as_str()), (Tone::Premium, fc::LOCK_PICK_PLAYLIST));
    }

    #[test]
    fn toggle_all_tracks_selects_all_then_none() {
        let mut s = state();
        playlist(&mut s, false);
        toggle_all_tracks(&mut s);
        assert!(selected(&s).iter().all(|t| *t));
        toggle_all_tracks(&mut s);
        assert!(selected(&s).iter().all(|t| !*t));
    }

    #[test]
    fn toggle_result_ticks_and_unticks() {
        let mut s = state();
        searching(&mut s);
        toggle_result(&mut s, "yt-2");
        assert!(s.picked.contains("yt-2"));
        toggle_result(&mut s, "yt-2");
        assert!(s.picked.is_empty());
    }

    #[test]
    fn download_found_queues_one_media_with_the_choice() {
        let mut s = state();
        let before = s.tasks.len();
        s.found = Found::Media(sample::media());
        s.choice.video_quality = VideoQuality::P1080;
        download_found(&mut s);
        let t = &s.tasks[0];
        assert_eq!(s.tasks.len(), before + 1);
        assert_eq!((t.is_audio_only, t.format.as_str(), t.video_quality), (false, "mp4", VideoQuality::P1080));
        assert_eq!(t.title, sample::media().title);
        assert_eq!(notice(&s).text, fc::TOAST_ADDED);
        assert_eq!(notice(&s).action, Some(Act::ViewDownloads));
        assert!(matches!(s.found, Found::Nothing), "the card closes");
        s.found = Found::Media(sample::media());
        s.choice.video = false;
        download_found(&mut s);
        assert_eq!((s.tasks[0].is_audio_only, s.tasks[0].format.as_str()), (true, "mp3"));
    }

    #[test]
    fn download_found_queues_the_picked_results_up_to_the_batch_limit() {
        let mut s = state();
        let before = s.tasks.len();
        searching(&mut s);
        for id in ["yt-1", "sc-1"] {
            toggle_result(&mut s, id);
        }
        download_found(&mut s);
        assert_eq!(s.tasks.len(), before + 2);
        assert!(s.picked.is_empty());
        assert_eq!(notice(&s).text, fc::toast_added_many(2));
        assert!(matches!(s.found, Found::Nothing), "the results close");
        searching(&mut s);
        s.plan = Plan::Basic;
        for id in ["yt-1", "yt-2", "yt-3", "yt-4", "yt-5", "yt-6"] {
            toggle_result(&mut s, id);
        }
        download_found(&mut s);
        assert_eq!(s.tasks.len(), before + 2, "over the limit: nothing queued");
        assert_eq!(s.picked.len(), 6, "the ticks stay");
        assert_eq!(notice(&s).tone, Tone::Premium);
        assert!(matches!(s.found, Found::Search { .. }), "a refusal keeps the results");
    }

    #[test]
    fn download_found_queues_a_spotify_playlist_track_by_track() {
        let mut s = state();
        let before = s.tasks.len();
        playlist(&mut s, false);
        for i in [0, 3, 5] {
            toggle_track(&mut s, i);
        }
        download_found(&mut s);
        assert_eq!(s.tasks.len(), before + 3);
        // In list order, so the first track is the oldest of the three new rows.
        assert_eq!(s.tasks[2].track.as_ref().map(|t| t.name.as_str()), Some("Speedo"));
        assert!(selected(&s).iter().all(|t| !*t), "the ticks clear");
        // Basic takes the first 20 and says so.
        let mut s = state();
        s.plan = Plan::Basic;
        let mut p = sample::playlist();
        p.tracks = (0..25).map(|i| ahg_engine::types::Track { id: format!("t{i}"), ..sample::playlist().tracks[0].clone() }).collect();
        s.found = Found::Playlist { playlist: p, matched: vec![Matched::Confirmed; 25], selected: vec![true; 25] };
        let before = s.tasks.len();
        download_found(&mut s);
        assert_eq!(s.tasks.len(), before + 20);
        assert_eq!(notice(&s).text, fc::LIMIT_COLLECTION);
    }

    #[test]
    fn download_found_queues_a_youtube_playlist_as_one_job() {
        let mut s = state();
        let before = s.tasks.len();
        playlist(&mut s, true);
        toggle_track(&mut s, 1);
        download_found(&mut s);
        assert_eq!(s.tasks.len(), before + 1);
        let t = &s.tasks[0];
        assert!(t.is_playlist && t.is_audio_only);
        assert_eq!(t.playlist_items.as_deref(), Some("1,3,4,5,6,7,8,9,10"));
        let mut s = state();
        s.plan = Plan::Basic;
        playlist(&mut s, true);
        download_found(&mut s);
        assert_eq!((s.tasks[0].playlist_items.as_deref(), s.tasks[0].playlist_limit), (None, Some(20.0)));
    }

    #[test]
    fn retry_band_asks_a_failed_band_again() {
        let mut s = state();
        s.found = Found::Search { query: "q".into(), bands: [Band::Failed, Band::Empty, Band::Failed] };
        retry_band(&mut s, 2);
        let Found::Search { bands, .. } = &s.found else { panic!() };
        assert!(matches!(bands, [Band::Failed, Band::Empty, Band::Loading]));
    }

    #[test]
    fn show_more_opens_one_band() {
        let mut s = state();
        searching(&mut s);
        show_more(&mut s, 1);
        assert_eq!(s.expanded, [false, true, false]);
    }

    #[test]
    fn pick_all_results_ticks_every_row() {
        let mut s = state();
        searching(&mut s);
        pick_all_results(&mut s);
        assert!(s.picked.contains("yt-7") && s.picked.contains("sp-4"));
    }

    // ── One download

    #[test]
    fn pause_holds_a_pausable_row_only() {
        let mut s = state();
        pause(&mut s, &mut Tx::default(), "big-buck-bunny");
        pause(&mut s, &mut Tx::default(), "soundcloud-set");
        pause(&mut s, &mut Tx::default(), "tiny-desk");
        assert_eq!(
            (status(&s, "big-buck-bunny"), status(&s, "soundcloud-set"), status(&s, "tiny-desk")),
            (St::Paused, St::Paused, St::Converting)
        );
    }

    #[test]
    fn resume_queues_a_paused_row_again() {
        let mut s = state();
        resume(&mut s, &mut Tx::default(), "lecture");
        assert_eq!(status(&s, "lecture"), St::Queued);
        assert_eq!(task(&s, "lecture").interrupted, None);
        resume(&mut s, &mut Tx::default(), "dreams");
        assert_eq!(status(&s, "dreams"), St::Done);
    }

    #[test]
    fn cancel_offers_undo_and_undo_queues_it_again() {
        let mut s = state();
        cancel(&mut s, &mut Tx::default(), "get-lucky");
        assert_eq!(status(&s, "get-lucky"), St::Cancelled);
        assert_eq!((notice(&s).text.as_str(), notice(&s).tone), (copy::TOAST_CANCELED_ONE, Tone::Info));
        notice_action(&mut s, &mut Tx::default());
        assert_eq!(status(&s, "get-lucky"), St::Queued);
        assert!(s.notices.is_empty(), "running the action dismisses");
        cancel(&mut s, &mut Tx::default(), "dreams");
        assert_eq!(status(&s, "dreams"), St::Done, "a finished row cannot be canceled");
    }

    #[test]
    fn retry_queues_a_failed_row_again_unless_permanent() {
        let mut s = state();
        retry(&mut s, &mut Tx::default(), "private-reel");
        retry(&mut s, &mut Tx::default(), "cancelled");
        retry(&mut s, &mut Tx::default(), "removed");
        assert_eq!(status(&s, "private-reel"), St::Queued);
        assert_eq!(task(&s, "private-reel").error_code, None);
        assert_eq!(task(&s, "private-reel").progress.error, "");
        assert_eq!(status(&s, "cancelled"), St::Queued);
        assert_eq!(status(&s, "removed"), St::Failed);
    }

    #[test]
    fn download_again_queues_a_skipped_row_without_skipping() {
        let mut s = state();
        download_again(&mut s, &mut Tx::default(), "dreams");
        assert_eq!(status(&s, "dreams"), St::Done, "not skipped: no Download again");
        s.tasks.iter_mut().find(|t| t.task_id == "dreams").unwrap().progress.already_on_disk = Some(true);
        download_again(&mut s, &mut Tx::default(), "dreams");
        assert_eq!(status(&s, "dreams"), St::Queued);
        assert!(!task(&s, "dreams").skip_existing);
    }

    #[test]
    fn promote_moves_a_queued_row_to_the_front_of_the_queue() {
        let mut s = state();
        three_queued(&mut s);
        let order = queued_order(&s);
        promote(&mut s, &mut Tx::default(), &order[2]);
        assert_eq!(queued_order(&s), vec![order[2].clone(), order[0].clone(), order[1].clone()]);
    }

    #[test]
    fn remove_drops_a_finished_row_only() {
        let mut s = state();
        s.selected.insert("dreams".into());
        remove(&mut s, &mut Tx::default(), "dreams");
        remove(&mut s, &mut Tx::default(), "get-lucky");
        assert!(!has(&s, "dreams") && has(&s, "get-lucky"));
        assert!(!s.selected.contains("dreams"));
    }

    #[test]
    fn open_file_without_a_file_says_so() {
        let mut s = state();
        open_file(&mut s, &mut Tx::default(), "dreams");
        assert!(s.notices.is_empty(), "sample mode opens nothing");
        s.tasks.iter_mut().find(|t| t.task_id == "dreams").unwrap().progress.filename.clear();
        open_file(&mut s, &mut Tx::default(), "dreams");
        assert_eq!(notice(&s).text, copy::file_never_finished(&copy::file_name_subject(&task(&s, "dreams").title)));
        assert_eq!(notice(&s).tone, Tone::Error);
    }

    #[test]
    fn show_in_folder_is_silent_without_a_file() {
        let mut s = state();
        s.tasks.iter_mut().find(|t| t.task_id == "dreams").unwrap().progress.filename.clear();
        show_in_folder(&mut s, &mut Tx::default(), "dreams");
        show_in_folder(&mut s, &mut Tx::default(), "humble");
        assert!(s.notices.is_empty());
    }

    #[test]
    fn copy_details_copies_a_failed_rows_error() {
        let mut s = state();
        copy_details(&mut s, &mut Tx::default(), "removed");
        assert!(s.notices.is_empty(), "the clipboard write is Rune's; State is unchanged");
        assert_eq!(status(&s, "removed"), St::Failed);
    }

    #[test]
    fn fix_opens_settings_or_updates_the_engine() {
        let mut s = state();
        fix(&mut s, &mut Tx::default(), "private-reel");
        assert_eq!(s.nav, Nav::Settings);
        fix(&mut s, &mut Tx::default(), "no-match");
        assert_eq!(notice(&s).text, copy::TOAST_UPDATING_ENGINE);
        assert_eq!(s.engine.state, "downloading");
    }

    #[test]
    fn notice_action_view_downloads_goes_there() {
        let mut s = state();
        s.found = Found::Media(sample::media());
        download_found(&mut s);
        notice_action(&mut s, &mut Tx::default());
        assert_eq!(s.nav, Nav::Downloads);
    }

    #[test]
    fn dismiss_notice_clears_it() {
        let mut s = state();
        cancel(&mut s, &mut Tx::default(), "get-lucky");
        dismiss_notice(&mut s);
        assert!(s.notices.is_empty());
    }

    // ── Many downloads

    #[test]
    fn pause_or_resume_all_pauses_then_resumes() {
        let mut s = state();
        pause_or_resume_all(&mut s);
        assert_eq!((status(&s, "big-buck-bunny"), status(&s, "soundcloud-set")), (St::Paused, St::Paused));
        // Converting and retrying rows are not pausable, so the list is not "all paused" yet.
        s.tasks.retain(|t| !matches!(t.progress.status, St::Converting | St::Retrying));
        pause_or_resume_all(&mut s);
        assert_eq!((status(&s, "big-buck-bunny"), status(&s, "lecture")), (St::Queued, St::Queued));
    }

    #[test]
    fn cancel_all_cancels_every_active_row_with_undo() {
        let mut s = state();
        let active = crate::format::active_count(&s.tasks);
        cancel_all(&mut s);
        assert_eq!(crate::format::active_count(&s.tasks), 0);
        assert_eq!(notice(&s).text, copy::toast_canceled(active));
        let Some(Act::Undo(ids)) = &notice(&s).action else { panic!("no undo") };
        assert_eq!(ids.len(), active);
    }

    #[test]
    fn retry_all_failed_skips_permanent_failures() {
        let mut s = state();
        retry_all_failed(&mut s);
        assert_eq!((status(&s, "private-reel"), status(&s, "no-match")), (St::Queued, St::Queued));
        assert_eq!((status(&s, "removed"), status(&s, "cancelled")), (St::Failed, St::Cancelled));
    }

    #[test]
    fn arm_confirm_disarm() {
        let cases = [Confirm::Clear, Confirm::Remove, Confirm::Deactivate, Confirm::Move];
        assert!(!cases.is_empty());
        for c in cases {
            let mut s = state();
            arm(&mut s, c);
            assert_eq!(s.confirm, Some(c));
            disarm(&mut s);
            assert_eq!(s.confirm, None);
        }
    }

    #[test]
    fn confirm_clear_removes_the_finished_rows() {
        let mut s = state();
        confirm(&mut s);
        assert_eq!(s.tasks.len(), sample::everything().len(), "nothing armed");
        arm(&mut s, Confirm::Clear);
        confirm(&mut s);
        assert!(!s.tasks.iter().any(|t| t.progress.status == St::Done));
        assert_eq!(s.confirm, None);
    }

    #[test]
    fn confirm_remove_removes_the_selected_finished_rows() {
        let mut s = state();
        for id in ["dreams", "removed", "get-lucky"] {
            toggle_select(&mut s, id);
        }
        arm(&mut s, Confirm::Remove);
        confirm(&mut s);
        assert!(!has(&s, "dreams") && !has(&s, "removed") && has(&s, "get-lucky"));
        assert_eq!(s.selected.iter().collect::<Vec<_>>(), vec!["get-lucky"]);
    }

    #[test]
    fn resume_interrupted_resumes_the_rows_left_by_a_restart() {
        let mut s = state();
        resume_interrupted(&mut s);
        assert_eq!(status(&s, "lecture"), St::Queued);
    }

    #[test]
    fn discard_interrupted_removes_them() {
        let mut s = state();
        toggle_select(&mut s, "lecture");
        discard_interrupted(&mut s);
        assert!(!has(&s, "lecture"));
        assert!(s.selected.is_empty());
    }

    #[test]
    fn set_filter_sets_it() {
        let mut s = state();
        set_filter(&mut s, Filter::Failed);
        assert_eq!(s.filter, Filter::Failed);
    }

    #[test]
    fn toggle_select_ticks_and_unticks() {
        let mut s = state();
        toggle_select(&mut s, "dreams");
        assert!(s.selected.contains("dreams"));
        toggle_select(&mut s, "dreams");
        assert!(s.selected.is_empty());
    }

    #[test]
    fn toggle_select_all_takes_the_filtered_rows() {
        let mut s = state();
        set_filter(&mut s, Filter::Done);
        s.query.replace_text("  ARTEMIS ");
        toggle_select_all(&mut s);
        assert_eq!(s.selected.iter().collect::<Vec<_>>(), vec!["artemis"]);
        toggle_select_all(&mut s);
        assert!(s.selected.is_empty());
    }

    #[test]
    fn cancel_selected_cancels_the_active_ones_without_undo() {
        let mut s = state();
        for id in ["get-lucky", "dreams"] {
            toggle_select(&mut s, id);
        }
        cancel_selected(&mut s);
        assert_eq!((status(&s, "get-lucky"), status(&s, "dreams")), (St::Cancelled, St::Done));
        assert_eq!(s.selected.iter().collect::<Vec<_>>(), vec!["dreams"]);
        assert!(s.notices.is_empty());
    }

    #[test]
    fn move_queued_reorders_the_queue() {
        let mut s = state();
        three_queued(&mut s);
        let order = queued_order(&s);
        move_queued(&mut s, &order[0], 2);
        assert_eq!(queued_order(&s), vec![order[1].clone(), order[2].clone(), order[0].clone()]);
        move_queued(&mut s, "dreams-not-there", 0);
        move_queued(&mut s, &order[0], 99);
        assert_eq!(queued_order(&s).last(), Some(&order[0]));
    }

    // ── Settings

    #[test]
    fn set_setting_keeps_a_valid_value_and_refuses_the_rest() {
        let cases: Vec<(&str, Value, bool)> = vec![
            ("skipExisting", json!(false), true),
            ("maxConcurrentDownloads", json!(3), true),
            ("scheduleDays", json!([1, 3]), true),
            ("sponsorBlockCategories", json!(["sponsor", "intro"]), true),
            ("rateLimit", json!(""), true),
            ("skipExisting", json!("no"), false),
            ("scheduleDays", json!(["mon"]), false),
            ("sponsorBlockCategories", json!([1]), false),
            ("palette", json!("neon"), false),
            ("palette", json!(""), false),
            ("palette", json!("moss"), true),
            ("noSuchKey", json!(1), false),
            ("__proto__", json!({}), false),
        ];
        assert!(!cases.is_empty());
        for (key, value, ok) in cases {
            let mut s = state();
            let before = s.settings.get(key).cloned();
            assert_eq!(set_setting(&mut s, key, value.clone()), ok, "{key} = {value}");
            assert_eq!(s.invalid.contains(key), !ok);
            assert_eq!(s.settings.get(key).cloned(), if ok { Some(value) } else { before });
        }
        let mut s = state();
        set_setting(&mut s, "proxy", json!(1));
        set_setting(&mut s, "proxy", json!(""));
        assert!(!s.invalid.contains("proxy"), "a good value clears the mark");
    }

    #[test]
    fn set_theme_writes_the_theme() {
        let mut s = state();
        set_theme(&mut s, "dark", &mut Tx::default());
        assert_eq!(s.settings["theme"], json!("dark"));
    }

    #[test]
    fn set_palette_writes_a_palette_electron_can_read() {
        let mut s = state();
        set_palette(&mut s, 3, &mut Tx::default());
        assert_eq!((s.palette, s.own.own_palette), (3, false));
        assert_eq!(s.settings["palette"], json!(ahg_engine::config::PALETTE_IDS[2]));
        set_palette(&mut s, 0, &mut Tx::default());
        assert_eq!((s.palette, s.own.own_palette), (0, true));
        assert_eq!(s.settings["palette"], json!(ahg_engine::config::PALETTE_IDS[2]), "palette 0 leaves Electron's key alone");
    }

    #[test]
    fn toggle_day_adds_and_removes_a_weekday() {
        let mut s = state();
        toggle_day(&mut s, 3);
        toggle_day(&mut s, 1);
        assert_eq!(s.settings["scheduleDays"], json!([3, 1]));
        toggle_day(&mut s, 3);
        assert_eq!(s.settings["scheduleDays"], json!([1]));
    }

    #[test]
    fn choose_folder_changes_nothing_in_sample_mode() {
        let mut s = state();
        choose_folder(&mut s, &mut Tx::default());
        assert_eq!(s.settings, sample::settings());
    }

    #[test]
    fn import_cookies_is_premium() {
        let mut s = state();
        import_cookies(&mut s, &mut Tx::default());
        assert!(s.notices.is_empty());
        s.plan = Plan::Basic;
        import_cookies(&mut s, &mut Tx::default());
        assert_eq!(notice(&s).tone, Tone::Premium);
    }

    #[test]
    fn check_update_starts_a_check() {
        let mut s = state();
        check_update(&mut s);
        assert_eq!(s.update.phase, "checking");
    }

    #[test]
    fn download_update_downloads_an_available_one() {
        let mut s = state();
        download_update(&mut s);
        assert_eq!(s.update.phase, "idle", "nothing available");
        s.update.phase = "available";
        download_update(&mut s);
        assert_eq!(s.update.phase, "downloading");
    }

    #[test]
    fn install_update_changes_nothing_in_sample_mode() {
        let mut s = state();
        s.update.phase = "ready";
        install_update(&mut s, &mut Tx::default());
        assert_eq!(s.update.phase, "ready");
    }

    #[test]
    fn check_engine_starts_a_check() {
        let mut s = state();
        check_engine(&mut s);
        assert_eq!(s.engine.state, "checking");
    }

    #[test]
    fn update_engine_downloads_it() {
        let mut s = state();
        update_engine(&mut s);
        assert_eq!(s.engine.state, "downloading");
    }

    #[test]
    fn show_shortcuts_opens_the_sheet() {
        let mut s = state();
        show_shortcuts(&mut s);
        assert_eq!(s.overlay, Overlay::Shortcuts);
    }

    #[test]
    fn get_support_changes_nothing_in_sample_mode() {
        let mut s = state();
        get_support(&mut s, &mut Tx::default());
        assert!(s.notices.is_empty());
    }

    // ── The scenes added with this step

    #[test]
    fn the_new_scenes_set_what_they_name() {
        let scene = |name: &str| {
            let mut s = state();
            (gate::SCENES.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("no scene {name}")).1)(&mut s);
            s
        };
        assert_eq!(scene("selected").selected.len(), 3);
        assert_eq!(scene("confirm").confirm, Some(Confirm::Clear));
        let s = scene("filtered-empty");
        assert!(s.tasks.iter().all(|t| !s.filter.passes(t)));
        assert_eq!(scene("short").card, Card::Short { label: "Instagram video".into() });
        assert_eq!((scene("settings-basic").plan, scene("settings-basic").nav), (Plan::Basic, Nav::Settings));
        assert_eq!(scene("palette").overlay, Overlay::Palette);
        assert_eq!(scene("shortcuts").overlay, Overlay::Shortcuts);
        assert_eq!(scene("notice").notices.len(), 1);
        let limit = scene("search-limit").notices.last().map(|n| (n.tone, n.action.clone()));
        assert_eq!(limit, Some((Tone::Premium, Some(Act::SeePlans))), "the limit scene is the real refusal");
        assert_eq!(scene("picker").overlay, Overlay::Picker);
    }

    // ── The palette's typed text and highlight live in State

    fn typed(s: &mut State, t: &str) {
        for c in t.chars() {
            palette_type(s, &c.to_string());
        }
    }
    fn labels(s: &State) -> Vec<String> {
        palette_filter(s).iter().map(|c| c.label(s)).collect()
    }

    #[test]
    fn palette_filter_matches_label_or_group_ignoring_case() {
        let all = commands(&state()).len();
        // (typed, first label expected, count check, row)
        let cases: &[(&str, Option<String>, fn(usize, usize) -> bool, &str)] = &[
            ("", None, |n, all| n == all, "empty lists everything"),
            ("  ", None, |n, all| n == all, "blanks list everything"),
            ("downl", Some(copy::palette_go_to(copy::NAV_DOWNLOADS)), |n, all| n > 0 && n < all, "label"),
            ("DOWNL", Some(copy::palette_go_to(copy::NAV_DOWNLOADS)), |n, all| n > 0 && n < all, "case"),
            ("navigate", None, |n, all| n > 0 && n < all, "group name"),
            ("zzzz", None, |n, _| n == 0, "nothing"),
        ];
        assert!(!cases.is_empty());
        for (q, first, count, row) in cases {
            let mut s = state();
            typed(&mut s, q);
            let got = labels(&s);
            assert!(count(got.len(), all), "{row}: {got:?}");
            if let Some(f) = first {
                assert_eq!(&got[0], f, "{row}");
            }
        }
    }

    #[test]
    fn palette_typing_resets_the_highlight_and_backspace_edits() {
        let mut s = state();
        palette_move(&mut s, 2);
        assert_eq!(s.command.highlight, 2);
        palette_type(&mut s, "d");
        assert_eq!((s.command.text.as_str(), s.command.highlight), ("d", 0));
        palette_move(&mut s, 1);
        palette_backspace(&mut s);
        assert_eq!((s.command.text.as_str(), s.command.highlight), ("", 0));
        palette_backspace(&mut s); // nothing left: no panic
        assert_eq!(s.command.text, "");
    }

    #[test]
    fn palette_move_wraps_both_ways_and_ignores_an_empty_list() {
        let mut s = state();
        let last = commands(&s).len() - 1;
        // (delta, expected highlight)
        let cases: &[(isize, usize)] = &[(1, 1), (-1, 0), (-1, last), (1, 0)];
        assert!(!cases.is_empty());
        for &(delta, want) in cases {
            palette_move(&mut s, delta);
            assert_eq!(s.command.highlight, want, "delta {delta}");
        }
        typed(&mut s, "zzzz");
        palette_move(&mut s, 1);
        palette_move(&mut s, -1);
        assert_eq!(s.command.highlight, 0, "no rows: stays put");
    }

    #[test]
    fn palette_run_runs_the_highlighted_row_and_closes_empty() {
        let mut s = state();
        open(&mut s, Overlay::Palette);
        typed(&mut s, "downl");
        palette_run(&mut s, &mut Tx::default());
        assert_eq!((s.overlay, s.nav), (Overlay::None, Nav::Downloads));
        assert_eq!(s.command, PaletteState::default(), "closing cleared it");
        open(&mut s, Overlay::Palette);
        typed(&mut s, "zzzz");
        palette_run(&mut s, &mut Tx::default());
        assert_eq!(s.overlay, Overlay::Palette, "no row: nothing runs");
    }

    #[test]
    fn open_and_close_reset_the_palette_input() {
        for reset in [|s: &mut State| open(s, Overlay::Palette), close] {
            let mut s = state();
            s.command = PaletteState { text: "x".into(), highlight: 3 };
            reset(&mut s);
            assert_eq!(s.command, PaletteState::default());
        }
    }

    // ── Wait timestamps and the timeline in State

    use crate::gate::{PaletteState, Part, Wait, WaitKey};
    use crate::motion::{LOADING_DELAY, LOADING_MIN};
    use crate::timeline::Show;

    fn waiting_lookup() -> State {
        let mut s = state();
        s.found = Found::LookingUp;
        s
    }

    #[test]
    fn begin_wait_records_the_start_and_schedules_the_bones() {
        let mut s = waiting_lookup();
        s.now = 10.0;
        begin_wait(&mut s, WaitKey::Lookup);
        assert_eq!(s.waits[&WaitKey::Lookup], Wait { started: 10.0, answered: None, shown_at: None });
        assert_eq!(s.timeline.next_deadline(), Some(10.0 + LOADING_DELAY));
        assert_eq!(crate::timeline::shows(&s, Part::Lookup), Show::Nothing);
    }

    #[test]
    fn an_answer_before_the_delay_never_shows_bones() {
        let mut s = waiting_lookup();
        s.now = 10.0;
        begin_wait(&mut s, WaitKey::Lookup);
        assert!(!tick(&mut s, 10.0 + LOADING_DELAY / 2.0));
        answer_wait(&mut s, WaitKey::Lookup);
        s.found = Found::Nothing;
        for dt in [LOADING_DELAY / 2.0, LOADING_DELAY, LOADING_DELAY + 1.0, 30.0] {
            tick(&mut s, 10.0 + dt);
            assert_eq!(crate::timeline::shows(&s, Part::Lookup), Show::Content, "dt {dt}");
        }
        assert_eq!(s.waits[&WaitKey::Lookup].shown_at, None);
    }

    #[test]
    fn bones_stay_until_shown_at_plus_the_minimum() {
        let mut s = waiting_lookup();
        s.now = 10.0;
        begin_wait(&mut s, WaitKey::Lookup);
        assert!(tick(&mut s, 10.0 + LOADING_DELAY), "LoadingShow changes State");
        assert_eq!(s.waits[&WaitKey::Lookup].shown_at, Some(10.0 + LOADING_DELAY));
        assert_eq!(crate::timeline::shows(&s, Part::Lookup), Show::Bones);
        s.now = 10.0 + LOADING_DELAY + 0.05;
        answer_wait(&mut s, WaitKey::Lookup);
        s.found = Found::Nothing; // the answer is in State, yet bones are held
        let free = 10.0 + LOADING_DELAY + LOADING_MIN;
        assert_eq!(s.timeline.next_deadline(), Some(free));
        assert!(!tick(&mut s, free - 0.01));
        assert_eq!(crate::timeline::shows(&s, Part::Lookup), Show::Bones);
        assert!(tick(&mut s, free), "LoadingFree changes State");
        assert_eq!(crate::timeline::shows(&s, Part::Lookup), Show::Content);
    }

    #[test]
    fn history_and_statistics_waits_never_show_bones() {
        for (key, part) in [(WaitKey::History, Part::History), (WaitKey::Stats, Part::Stats)] {
            let mut s = state();
            s.loading = crate::gate::Loading::Booting;
            begin_wait(&mut s, key);
            tick(&mut s, LOADING_DELAY + 1.0);
            assert_eq!(crate::timeline::shows(&s, part), Show::Nothing, "{part:?}");
        }
    }

    #[test]
    fn each_part_keeps_its_own_wait() {
        let mut s = state();
        let keys = [WaitKey::Band(0), WaitKey::Band(1), WaitKey::Match(2), WaitKey::Cover("a".into())];
        assert_eq!(Part::Cover("a").key(), keys[3]);
        for k in &keys {
            begin_wait(&mut s, k.clone());
        }
        answer_wait(&mut s, keys[1].clone());
        assert_eq!(s.waits.len(), 4);
        assert!(s.waits[&keys[1]].answered.is_some());
        assert_eq!(keys.iter().filter(|k| s.waits[*k].answered.is_none()).count(), 3);
        answer_wait(&mut s, WaitKey::Lookup); // no such wait: nothing happens
        assert_eq!(s.waits.len(), 4);
    }

    // ── Shared actions of step 54X

    fn ids(s: &State) -> Vec<String> {
        s.tasks.iter().map(|t| t.task_id.clone()).collect()
    }

    #[test]
    fn clear_selection_and_results_empty_their_sets() {
        let mut s = state();
        s.selected.insert("a".into());
        s.picked.insert("b".into());
        clear_selection(&mut s);
        assert!(s.selected.is_empty() && !s.picked.is_empty());
        clear_results(&mut s);
        assert!(s.picked.is_empty());
    }

    #[test]
    fn show_all_resets_filter_and_search() {
        let mut s = state();
        s.filter = Filter::Failed;
        s.query.replace_text("zzz");
        show_all(&mut s);
        assert_eq!(s.filter, Filter::All);
        assert_eq!(s.query.text, "");
    }

    #[test]
    fn pick_row_toggles_and_shift_takes_the_range() {
        let mut s = state();
        let all = ids(&s);
        assert!(all.len() >= 4);
        let shift = Mods { shift: true, ..Mods::default() };
        let ctrl = Mods { ctrl: true, cmd: true, ..Mods::default() };
        pick_row(&mut s, &all[1], Mods::default());
        assert_eq!(s.selected.len(), 1);
        pick_row(&mut s, &all[1], ctrl);
        assert!(s.selected.is_empty(), "ctrl toggles");
        pick_row(&mut s, &all[1], Mods::default());
        pick_row(&mut s, &all[3], shift);
        let want: std::collections::HashSet<String> = all[1..=3].iter().cloned().collect();
        assert_eq!(s.selected, want, "shift takes the range");
        assert_eq!(s.last_picked.as_deref(), Some(all[1].as_str()), "shift keeps the anchor");
        // Shift with no anchor is a plain toggle.
        let mut s = state();
        pick_row(&mut s, &all[2], shift);
        assert_eq!(s.selected.len(), 1);
    }

    #[test]
    fn deselect_all_tracks_unticks_everything_and_respects_the_basic_lock() {
        let mut s = state();
        playlist(&mut s, false);
        deselect_all_tracks(&mut s);
        assert!(selected(&s).iter().all(|t| !t));
        // A YouTube playlist on Basic cannot be changed.
        let mut s = state();
        s.plan = Plan::Basic;
        playlist(&mut s, true);
        deselect_all_tracks(&mut s);
        assert!(selected(&s).iter().all(|t| *t));
        assert_eq!(notice(&s).text, fc::LOCK_PICK_PLAYLIST);
    }

    #[test]
    fn see_plans_sets_the_plan_notice() {
        for (plan, title) in [(Plan::Basic, copy::settings::UPGRADE_TITLE_BASIC), (Plan::Premium, copy::settings::UPGRADE_TITLE_PREMIUM)] {
            let mut s = state();
            s.plan = plan;
            see_plans(&mut s);
            assert!(notice(&s).text.starts_with(title));
        }
    }

    #[test]
    fn set_choice_with_changes_one_field() {
        let mut s = state();
        let before = s.choice.clone();
        let cases = [
            ChoiceChange::Video(!before.video),
            ChoiceChange::Format("flac".into()),
            ChoiceChange::AudioQuality("192".into()),
            ChoiceChange::VideoQuality(VideoQuality::P720),
            ChoiceChange::Folder(Some("D:/x".into())),
        ];
        for change in cases {
            let mut s = state();
            set_choice_with(&mut s, change);
            assert_ne!(s.choice, before);
        }
        s.plan = Plan::Basic;
        set_choice_with(&mut s, ChoiceChange::Format("flac".into()));
        assert_eq!(s.choice, before, "a Basic lock changes nothing");
        assert_eq!(notice(&s).text, fc::lock_format("FLAC"));
    }

    #[test]
    fn commit_text_keeps_a_refused_value_and_resets_after_an_accepted_one() {
        let mut s = state();
        edit_text(&mut s, "outputTemplate").replace_text("%(title)s");
        assert!(commit_text(&mut s, "outputTemplate"));
        assert_eq!(s.settings["outputTemplate"], json!("%(title)s"));
        assert!(!s.edits.contains_key("outputTemplate"), "the edit is rebuilt from the setting");
        assert_eq!(edit_text(&mut s, "outputTemplate").text, "%(title)s", "created from the setting on first use");
        // Not a setting: refused, the text stays for the red edge.
        edit_text(&mut s, "nope").replace_text("x");
        assert!(!commit_text(&mut s, "nope"));
        assert!(s.invalid.contains("nope"));
        assert_eq!(s.edits["nope"].text, "x");
    }

    #[test]
    fn pick_setting_writes_and_honours_basic_locks() {
        let mut s = state();
        let mut tx = Tx::default();
        assert!(pick_setting(&mut s, &mut tx, "videoContainer", "mkv"));
        assert_eq!(s.settings["videoContainer"], json!("mkv"));
        assert!(pick_setting(&mut s, &mut tx, "maxConcurrentDownloads", "2"));
        assert_eq!(s.settings["maxConcurrentDownloads"], json!(2));
        let mut s = state();
        s.plan = Plan::Basic;
        let before = s.settings["defaultFormat"].clone();
        assert!(!pick_setting(&mut s, &mut tx, "defaultFormat", "flac"));
        assert_eq!(s.settings["defaultFormat"], before);
        assert_eq!(notice(&s).text, fc::lock_format("FLAC"));
        let opts = select_options(&s, "defaultFormat");
        assert!(opts.iter().any(|o| o.value == "flac" && o.lock.is_some()));
        assert!(opts.iter().any(|o| o.value == "mp3" && o.lock.is_none()));
        assert!(select_options(&s, "theme").is_empty(), "only selects have options");
    }
}
