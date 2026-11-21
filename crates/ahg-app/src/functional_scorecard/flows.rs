//! The core flow, search and playlists, and the error states.
use ahg_engine::types::DownloadStatus as St;

use super::{Ctx, Res, Scenario, expect, found_kind};
use crate::copy::{self, found as fc};
use crate::format;
use crate::gate::{Band, Found, Nav, Tone};
use crate::keys::{self, RowAct};

pub const LINK: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
pub const TITLE: &str = "A Song";
pub const RECORD: &str = r#"{"id":"dQw4w9WgXcQ","title":"A Song","uploader":"Someone","duration":212,"webpage_url":"https://www.youtube.com/watch?v=dQw4w9WgXcQ"}"#;
/// A quarter, then half of 4 MiB.
pub const P25: &str = "err [PROGRESS]1048576|4194304|NA|524288|6";
pub const P50: &str = "err [PROGRESS]2097152|4194304|NA|524288|4";
/// A failure the engine does not retry by itself, with Settings as its fix.
pub const AGE: &str =
    "err ERROR: [youtube] dQw4w9WgXcQ: Sign in to confirm your age. This video may be inappropriate for some users.\nexit 1\n";

/// `out <record>` for the lookup.
pub fn found_one() -> String {
    format!("out {RECORD}\nexit 0\n")
}

// ── Core flow (25)

pub const CORE: &[Scenario] = &[
    ("a pasted link shows its found card", core_found),
    ("audio and a format are chosen", core_choose),
    ("Download adds a row that says Starting", core_download),
    ("the row shows progress", core_progress),
    ("the row ends Done with its file", core_done),
    ("the history keeps it after a restart", core_history),
];

fn core_found(c: &mut Ctx) -> Res {
    c.json(&found_one());
    let media = c.media("A Song.mp3");
    // A wide Starting window, then progress in two steps, then the file.
    c.script(&format!("sleep 2500\n{P25}\nsleep 800\n{P50}\nsleep 800\nout [FILEPATH]{}\nexit 0\n", media.display()));
    c.enter(LINK)?;
    c.until("the lookup's answer", |s| matches!(&s.found, Found::Media(m) if m.title == TITLE))
        .map_err(|e| format!("{e} (the field found {})", found_kind(&c.rt.state)))?;
    expect(c.has(keys::FOUND), || "the found card has no `found` element".into())?;
    expect(c.drawn(TITLE), || "the found card does not show the title".into())
}

fn core_choose(c: &mut Ctx) -> Res {
    c.click(keys::CHOICE_AUDIO)?;
    expect(!c.rt.state.choice.video, || "`choice-audio` left the choice on video".into())?;
    // mp3 is open on Basic (live mode's plan).
    c.click(&keys::choice_format("mp3"))?;
    expect(c.rt.state.choice.format == "mp3", || format!("the format is {} after `choice-format-mp3`", c.rt.state.choice.format))
}

fn core_download(c: &mut Ctx) -> Res {
    c.click(keys::FOUND_DOWNLOAD)?;
    c.until("the new row", |s| !s.tasks.is_empty())?;
    let toast = c.drawn(fc::TOAST_ADDED);
    let t = &c.rt.state.tasks[0];
    expect(t.title == TITLE && t.is_audio_only && t.format == "mp3", || {
        format!("the row is `{}`, audio {}, {}", t.title, t.is_audio_only, t.format)
    })?;
    c.go(Nav::Downloads)?;
    let starting = c.drawn(copy::STATE_STARTING);
    let status = c.rt.state.tasks[0].progress.status;
    expect(toast, || "no `Added to downloads` notice".into())?;
    expect(starting || !matches!(status, St::Queued | St::FetchingInfo), || format!("Downloads does not say Starting ({status:?})"))
}

fn core_progress(c: &mut Ctx) -> Res {
    c.until("the first percent", |s| s.tasks.first().is_some_and(|t| t.progress.percent > 0.0))?;
    let id = c.rt.state.tasks[0].task_id.clone();
    expect(c.has(&keys::row(&id)), || format!("no `{}` drawn while it downloads", keys::row(&id)))
}

fn core_done(c: &mut Ctx) -> Res {
    c.until("the download to finish", |s| s.tasks.first().is_some_and(|t| t.progress.status == St::Done))?;
    let t = c.rt.state.tasks[0].clone();
    expect(t.progress.filename.ends_with("A Song.mp3"), || format!("the file is `{}`", t.progress.filename))?;
    c.frames(60);
    expect(c.drawn(TITLE), || "the finished row's title is not drawn".into())
}

fn core_history(c: &mut Ctx) -> Res {
    c.restart()?;
    c.until("the finished row after the restart", |s| s.tasks.iter().any(|t| t.title == TITLE && t.progress.status == St::Done))
}

// ── Search and playlists (15)

pub const SEARCH: &[Scenario] = &[
    ("words search and the results show", search_words),
    ("a ticked result downloads with the Added notice", search_download),
    ("a playlist link lists its tracks", playlist_lists),
    ("Basic says why a playlist cannot be picked", playlist_locked),
    ("the playlist downloads as one job", playlist_download),
];

/// Three results, as yt-dlp's flat search prints them.
fn results() -> String {
    (1..=3)
        .map(|i| format!(r#"out {{"id":"r{i}","title":"Result {i}","uploader":"Band {i}","duration":200,"url":"https://www.youtube.com/watch?v=r{i}xxxxxxxxx"}}"#))
        .chain(["exit 0".into()])
        .collect::<Vec<_>>()
        .join("\n")
}

fn search_words(c: &mut Ctx) -> Res {
    c.json(&results());
    c.script("sleep 60000\n");
    c.enter("khruangbin")?;
    c.until("the YouTube band's results", |s| matches!(&s.found, Found::Search { bands: [Band::Results(_), ..], .. }))
        .map_err(|e| format!("{e} (the field found {})", found_kind(&c.rt.state)))?;
    expect(c.has(&keys::result("r1")), || "no `result-r1` drawn".into())
}

fn search_download(c: &mut Ctx) -> Res {
    c.click(&keys::result_tick("r1"))?;
    expect(c.rt.state.picked.contains("r1"), || "`result-tick-r1` did not tick it".into())?;
    c.click(keys::FOUND_DOWNLOAD)?;
    // The stand-in answers both sources with the same rows, so the tick may take two.
    let notice = c.rt.state.notices.last().map(|n| n.text.clone()).unwrap_or_default();
    let added = notice == fc::TOAST_ADDED || (2..=6).any(|n| notice == fc::toast_added_many(n));
    let toast = added && c.drawn(&notice);
    c.until("the result's row", |s| s.tasks.iter().any(|t| t.title == "Result 1"))?;
    expect(toast, || format!("no `Added to downloads` notice drawn (notice `{notice}`)"))
}

const PLAYLIST: &str = "https://www.youtube.com/playlist?list=PLfunc0000000000000";

fn playlist_lists(c: &mut Ctx) -> Res {
    let entries = (1..=3).map(|i| format!(r#"out {{"id":"t{i}xxxxxxxxx","title":"Track {i}","uploader":"Band","duration":180,"url":"https://www.youtube.com/watch?v=t{i}xxxxxxxxx","playlist_title":"A List"}}"#));
    c.json(&entries.chain(["exit 0".into()]).collect::<Vec<_>>().join("\n"));
    c.enter(PLAYLIST)?;
    c.until("the playlist's tracks", |s| matches!(&s.found, Found::Playlist { playlist, .. } if playlist.tracks.len() == 3))
        .map_err(|e| format!("{e} (the field found {})", found_kind(&c.rt.state)))?;
    expect(c.drawn("Track 2"), || "the tracks are not drawn".into())?;
    expect(c.has(&keys::track(0)), || "no `track-0` drawn".into())
}

fn playlist_locked(c: &mut Ctx) -> Res {
    let before = selected(c);
    c.click(&keys::track(1))?;
    expect(selected(c) == before, || "a Basic playlist row was ticked".into())?;
    expect(c.rt.state.notices.last().is_some_and(|n| n.tone == Tone::Premium), || "no premium notice".into())?;
    expect(c.drawn(fc::LOCK_PICK_PLAYLIST), || "the lock's sentence is not drawn".into())
}

fn selected(c: &Ctx) -> Vec<bool> {
    if let Found::Playlist { selected, .. } = &c.rt.state.found { selected.clone() } else { Vec::new() }
}

fn playlist_download(c: &mut Ctx) -> Res {
    c.click(keys::FOUND_DOWNLOAD)?;
    c.until("the playlist's one job", |s| s.tasks.iter().any(|t| t.is_playlist && t.url == PLAYLIST))?;
    let jobs = c.rt.state.tasks.iter().filter(|t| t.url == PLAYLIST).count();
    expect(jobs == 1, || format!("{jobs} jobs for one playlist"))
}

// ── Error states (10)

pub const ERRORS: &[Scenario] = &[
    ("a link of no supported site says so", bad_link),
    ("a refused address never reaches yt-dlp", refused_host),
    ("a failed lookup explains why", lookup_failed),
    ("a failed download shows its reason and its fix", download_failed),
    ("offline: a source that cannot be reached offers Retry", offline),
];

fn bad_link(c: &mut Ctx) -> Res {
    c.enter("ftp://files.example.com/a.mp3")?;
    c.until("the refusal", |s| matches!(s.found, Found::LookupFailed(_)))
        .map_err(|e| format!("{e} (found {})", found_kind(&c.rt.state)))?;
    expect(c.drawn(copy::ERR_NOT_SUPPORTED), || "the refusal's sentence is not drawn".into())
}

fn refused_host(c: &mut Ctx) -> Res {
    let asked = c.dir.join("args.txt.json");
    let _ = std::fs::remove_file(&asked);
    let tasks = c.rt.state.tasks.len();
    c.enter("file:///C:/Windows/win.ini")?;
    c.frames(30);
    expect(!asked.exists(), || "yt-dlp was asked about a file:// link".into())?;
    expect(c.rt.state.tasks.len() == tasks, || "a file:// link was queued".into())?;
    expect(!matches!(c.rt.state.found, Found::Nothing | Found::LookingUp), || {
        format!("nothing says why (found {})", found_kind(&c.rt.state))
    })
}

fn lookup_failed(c: &mut Ctx) -> Res {
    c.json("err ERROR: [youtube] dQw4w9WgXcQ: Video unavailable. This video has been removed by the uploader\nexit 1\n");
    c.enter(LINK)?;
    c.until("the failed lookup", |s| matches!(s.found, Found::LookupFailed(_)))
        .map_err(|e| format!("{e} (found {})", found_kind(&c.rt.state)))?;
    let Found::LookupFailed(why) = &c.rt.state.found else { unreachable!() };
    let why = why.clone();
    expect(c.drawn(&why), || format!("`{why}` is not drawn"))
}

fn download_failed(c: &mut Ctx) -> Res {
    c.json(&found_one());
    c.script(AGE);
    c.enter(LINK)?;
    c.until("the lookup's answer", |s| matches!(s.found, Found::Media(_)))?;
    c.click(keys::FOUND_DOWNLOAD)?;
    c.until("the failure", |s| s.tasks.iter().any(|t| t.progress.status == St::Failed))?;
    c.go(Nav::Downloads)?;
    let t = c.rt.state.tasks.iter().find(|t| t.progress.status == St::Failed).cloned().unwrap();
    let reason = format::reason(&t).unwrap_or_default();
    expect(!reason.is_empty() && c.drawn(&reason), || format!("the reason `{reason}` is not drawn"))?;
    expect(format::fix(&t).is_some(), || "the age limit has no fix".into())?;
    c.row_act(RowAct::Fix, &t.task_id)?;
    expect(c.rt.state.nav == Nav::Settings, || "the fix did not open Settings".into())
}

fn offline(c: &mut Ctx) -> Res {
    c.json("err ERROR: Unable to download webpage: <urlopen error [Errno 11001] getaddrinfo failed>\nexit 1\n");
    c.enter("khruangbin")?;
    c.until("the band that could not be reached", |s| matches!(&s.found, Found::Search { bands: [Band::Failed, ..], .. }))
        .map_err(|e| format!("{e} (found {})", found_kind(&c.rt.state)))?;
    c.json(&results());
    c.click(&keys::band_retry(0))?;
    c.until("the retried band's results", |s| matches!(&s.found, Found::Search { bands: [Band::Results(_), ..], .. }))
}
