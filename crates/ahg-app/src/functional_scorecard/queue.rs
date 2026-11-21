//! Queue controls, one download allowed at a time.
use ahg_engine::types::DownloadStatus as St;
use serde_json::json;

use super::flows::{AGE, P25, found_one};
use super::{Ctx, Res, Scenario, expect};
use crate::actions;
use crate::gate::{Confirm, Filter, Nav};
use crate::keys::{self, RowAct};

pub const QUEUE: &[Scenario] = &[
    ("a running row pauses", pause),
    ("a paused row resumes", resume),
    ("Do this next moves a queued row first", reorder),
    ("a running row cancels, offering Undo", cancel),
    ("a canceled row retries", retry),
    ("each filter shows only its rows", filters),
    ("Pause all and Resume all", pause_all),
    ("Retry all failed", retry_all_failed),
    ("Clear finished, with its confirm", clear_finished),
];

fn link(n: usize) -> String {
    format!("https://www.youtube.com/watch?v=queue{n:06}")
}

fn pause(c: &mut Ctx) -> Res {
    c.json(&found_one());
    c.script(&format!("{P25}\nsleep 60000\n"));
    c.rt.update(|s, _| {
        actions::set_setting(s, "maxConcurrentDownloads", json!(1));
    });
    for n in 0..3 {
        c.queue(&link(n));
    }
    c.until("three rows, one running", |s| s.tasks.len() == 3 && s.tasks.iter().any(|t| t.progress.status == St::Downloading))?;
    c.go(Nav::Downloads)?;
    let id = c.task(|t| t.progress.status == St::Downloading).unwrap();
    c.row_act(RowAct::Pause, &id)?;
    c.until("the row paused", |s| s.tasks.iter().any(|t| t.task_id == id && t.progress.status == St::Paused))
}

fn resume(c: &mut Ctx) -> Res {
    let id = c.task(|t| t.progress.status == St::Paused).ok_or("no paused row")?;
    c.row_act(RowAct::Resume, &id)?;
    c.until("the row resumed", |s| s.tasks.iter().any(|t| t.task_id == id && t.progress.status != St::Paused))
}

fn running(t: &ahg_engine::types::DownloadTask) -> bool {
    matches!(t.progress.status, St::FetchingInfo | St::Downloading)
}

/// Four fresh rows, one at a time: W runs, X, Y and Z wait in that order. "Do this next" on Z
/// must run Z next and keep X before Y.
fn reorder(c: &mut Ctx) -> Res {
    c.rt.update(|s, _| actions::cancel_all(s));
    c.until("nothing running", |s| !s.tasks.iter().any(running))?;
    let links: Vec<String> = (40..44).map(link).collect();
    for l in &links {
        c.queue(l);
        let l = l.clone();
        c.until("the new row", move |s| s.tasks.iter().any(|t| t.url == l))?;
    }
    let id = |c: &Ctx, l: &str| c.task(|t| t.url == l).unwrap();
    let [w, x, _, z] = [0, 1, 2, 3].map(|i| id(c, &links[i]));
    c.until("W running, three waiting", |s| s.tasks.iter().any(|t| t.task_id == w && running(t)))?;
    c.row_act(RowAct::Promote, &z)?;
    c.frames(30);
    let next = |c: &mut Ctx, stop: &str, want: &str, what: &str| -> Res {
        let stop = stop.to_owned();
        c.rt.update(move |s, tx| actions::cancel(s, tx, &stop));
        let w = want.to_owned();
        c.until(what, move |s| s.tasks.iter().any(|t| t.task_id == w && running(t))).map_err(|e| {
            let on: Vec<String> = c.rt.state.tasks.iter().filter(|t| running(t)).map(|t| t.url.clone()).collect();
            format!("{e} (running: {on:?})")
        })
    };
    next(c, &w, &z, "Z run next")?;
    next(c, &z, &x, "X run after Z, before Y")
}

fn cancel(c: &mut Ctx) -> Res {
    let id = c.task(running).ok_or("no running row")?;
    c.row_act(RowAct::Cancel, &id)?;
    let undo = c.drawn(crate::copy::TOAST_UNDO);
    c.until("the row canceled", |s| s.tasks.iter().any(|t| t.task_id == id && t.progress.status == St::Cancelled))?;
    expect(undo, || "no Undo on the cancel's notice".into())
}

fn retry(c: &mut Ctx) -> Res {
    let id = c.task(|t| t.progress.status == St::Cancelled).ok_or("no canceled row")?;
    c.row_act(RowAct::Retry, &id)?;
    c.until("the row queued again", |s| s.tasks.iter().all(|t| t.task_id != id || t.progress.status != St::Cancelled))
}

fn filters(c: &mut Ctx) -> Res {
    // One finished row for Done, one canceled for Failed.
    let done = c.media("done.mp3");
    c.script(&format!("out [FILEPATH]{}\nexit 0\n", done.display()));
    c.rt.update(|s, _| {
        actions::set_setting(s, "maxConcurrentDownloads", json!(3));
    });
    c.queue(&link(10));
    c.until("a finished row", |s| s.tasks.iter().any(|t| t.progress.status == St::Done))?;
    let mut wrong = Vec::new();
    for f in Filter::ALL {
        c.click(&keys::filter(f))?;
        if c.rt.state.filter != f {
            wrong.push(format!("`{}` set {:?}", keys::filter(f), c.rt.state.filter));
            continue;
        }
        let tasks = c.rt.state.tasks.clone();
        for t in &tasks {
            if !f.passes(t) && c.rt.find(&keys::row(&t.task_id)).is_some() {
                wrong.push(format!("{f:?} shows a {:?} row", t.progress.status));
            }
        }
    }
    c.click(&keys::filter(Filter::All))?;
    expect(wrong.is_empty(), || wrong.join("; "))
}

fn active(c: &Ctx) -> Vec<St> {
    c.rt.state
        .tasks
        .iter()
        .map(|t| t.progress.status)
        .filter(|s| matches!(s, St::Queued | St::FetchingInfo | St::Downloading | St::Paused))
        .collect()
}

fn pause_all(c: &mut Ctx) -> Res {
    c.script(&format!("{P25}\nsleep 60000\n"));
    c.queue(&link(20));
    c.until("a running row", |s| s.tasks.iter().any(|t| t.progress.status == St::Downloading))?;
    c.click(keys::PAUSE_ALL)?;
    c.until("every active row paused", |s| {
        s.tasks.iter().all(|t| !matches!(t.progress.status, St::Queued | St::FetchingInfo | St::Downloading))
    })
    .map_err(|e| format!("{e}: {:?}", active(c)))?;
    c.click(keys::PAUSE_ALL)?;
    c.until("the rows resumed", |s| s.tasks.iter().all(|t| t.progress.status != St::Paused)).map_err(|e| format!("{e}: {:?}", active(c)))
}

fn retry_all_failed(c: &mut Ctx) -> Res {
    c.click(keys::CANCEL_ALL)?;
    c.until("nothing running", |s| s.tasks.iter().all(|t| !crate::format::is_active(t)))?;
    c.script(AGE);
    let url = link(30);
    c.queue(&url);
    let u = url.clone();
    c.until("the failed row", move |s| s.tasks.iter().any(|t| t.url == u && t.progress.status == St::Failed))?;
    let done = c.media("again.mp3");
    c.script(&format!("out [FILEPATH]{}\nexit 0\n", done.display()));
    c.click(keys::RETRY_ALL_FAILED)?;
    c.until("the failed row done after Retry all failed", move |s| s.tasks.iter().any(|t| t.url == url && t.progress.status == St::Done))
}

fn clear_finished(c: &mut Ctx) -> Res {
    expect(c.rt.state.tasks.iter().any(|t| t.progress.status == St::Done), || "no finished row".into())?;
    c.click(keys::arm(Confirm::Clear))?;
    expect(c.rt.state.confirm == Some(Confirm::Clear), || "`clear-completed` armed nothing".into())?;
    c.click(keys::CONFIRM_YES)?;
    c.until("the finished rows gone", |s| s.tasks.iter().all(|t| t.progress.status != St::Done))
}
