//! Keyboard and navigation, the shell's lifetime, and start-up.
use std::time::{Duration, Instant};

use ahg_engine::types::DownloadStatus as St;
use rune::{Key, Mods};

use super::flows::{P25, found_one};
use super::{Ctx, Res, Scenario, expect};
use crate::gate::{Nav, Overlay};
use crate::keys;

// ── Keyboard and navigation (5)

pub const KEYBOARD: &[Scenario] = &[
    ("Ctrl+1, 2 and 3 go to the three places", places),
    ("Ctrl+K opens the palette, typing filters, Enter runs, Ctrl+K closes", palette),
    ("Esc closes every overlay and what the field found", escape),
    ("Tab reaches every control on Home and Downloads", tab_order),
];

fn places(c: &mut Ctx) -> Res {
    for (digit, nav) in [('2', Nav::Downloads), ('3', Nav::Settings), ('1', Nav::Home)] {
        c.ctrl(digit);
        expect(c.rt.state.nav == nav, || format!("Ctrl+{digit} left the app on {:?}", c.rt.state.nav))?;
    }
    Ok(())
}

fn palette(c: &mut Ctx) -> Res {
    c.ctrl('k');
    expect(c.rt.state.overlay == Overlay::Palette, || "Ctrl+K opened nothing".into())?;
    c.type_text("settings");
    let focused = c.rt.elements().into_iter().find(|e| e.focused).map(|e| e.key.unwrap_or(e.path));
    expect(c.rt.state.command.text == "settings", || {
        format!("the palette's line holds `{}` (focus on {focused:?})", c.rt.state.command.text)
    })?;
    c.key(Key::Enter, Mods::default());
    expect(c.rt.state.nav == Nav::Settings && c.rt.state.overlay == Overlay::None, || {
        format!("Enter left {:?} with {:?} open", c.rt.state.nav, c.rt.state.overlay)
    })?;
    c.ctrl('k');
    c.ctrl('k');
    expect(c.rt.state.overlay == Overlay::None, || "a second Ctrl+K did not close the palette".into())
}

fn escape(c: &mut Ctx) -> Res {
    let mut stuck = Vec::new();
    for (open, what) in [('k', Overlay::Palette), ('/', Overlay::Shortcuts), ('.', Overlay::Picker)] {
        c.ctrl(open);
        let opened = c.rt.state.overlay == what;
        c.key(Key::Escape, Mods::default());
        if !opened || c.rt.state.overlay != Overlay::None || c.rt.state.picker {
            stuck.push(format!("{what:?} (opened {opened}, after Esc {:?})", c.rt.state.overlay));
        }
    }
    c.json(&found_one());
    c.enter(super::flows::LINK)?;
    c.until("the found card", |s| !matches!(s.found, crate::gate::Found::Nothing | crate::gate::Found::LookingUp))?;
    c.key(Key::Escape, Mods::default());
    if !matches!(c.rt.state.found, crate::gate::Found::Nothing) {
        stuck.push("the found card".into());
    }
    expect(stuck.is_empty(), || format!("Esc left open: {}", stuck.join(", ")))
}

/// Every focusable keyed control on screen, and the ones Tab never reached in `limit` presses.
fn unreached(c: &mut Ctx, limit: usize) -> (usize, Vec<String>) {
    let shown = |c: &Ctx| -> Vec<String> {
        c.rt.elements()
            .into_iter()
            .filter(|e| e.focusable && !e.exiting && !e.inert && e.opacity > 0.0 && e.visible[2] > 0.0 && e.visible[3] > 0.0)
            .filter_map(|e| e.key)
            .collect()
    };
    let want = shown(c);
    let mut seen = std::collections::HashSet::new();
    for _ in 0..limit {
        c.rt.key(rune::KeyEvent { key: Key::Tab, mods: Mods::default() });
        c.frames(2);
        if let Some(k) = c.rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key) {
            seen.insert(k);
        }
        if want.iter().all(|k| seen.contains(k)) {
            break;
        }
    }
    (want.len(), want.into_iter().filter(|k| !seen.contains(k)).collect())
}

fn tab_order(c: &mut Ctx) -> Res {
    let mut gaps = Vec::new();
    for nav in [Nav::Home, Nav::Downloads] {
        c.go(nav)?;
        let (n, missed) = unreached(c, 400);
        if n == 0 {
            gaps.push(format!("{nav:?}: no focusable control"));
        } else if !missed.is_empty() {
            gaps.push(format!(
                "{nav:?}: {} of {n} never focused ({})",
                missed.len(),
                missed.iter().take(6).cloned().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    expect(gaps.is_empty(), || gaps.join("; "))
}

// ── Shell (5)

pub const SHELL: &[Scenario] = &[
    ("closing quits: the core stops and its yt-dlp with it", close_quits),
    ("closing twice stops once", close_twice),
    ("one instance: a second launch opens no window", one_instance),
];

/// Whether a process with this id runs.
fn alive(pid: u32) -> bool {
    let out = std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"]).output();
    out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")))
}

fn close_quits(c: &mut Ctx) -> Res {
    let pids = c.dir.join("pids.txt");
    c.json(&found_one());
    c.script(&format!("pid {}\n{P25}\nsleep 60000\n", pids.display()));
    c.queue("https://www.youtube.com/watch?v=closequit01");
    c.until("the download running", |s| s.tasks.iter().any(|t| t.progress.status == St::Downloading))?;
    let pid: u32 = std::fs::read_to_string(&pids)
        .map_err(|e| format!("no pid file: {e}"))?
        .lines()
        .last()
        .and_then(|l| l.trim().parse().ok())
        .ok_or("no pid")?;
    expect(alive(pid), || "the stand-in is not running".into())?;
    c.rt.update(crate::on_close);
    expect(c.rt.state.live.is_none(), || "the core is still held after close".into())?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    expect(!alive(pid), || format!("yt-dlp ({pid}) outlived the quit"))?;
    // The next scenarios need a core.
    c.restart()
}

fn close_twice(c: &mut Ctx) -> Res {
    c.rt.update(crate::on_close);
    c.rt.update(crate::on_quit);
    expect(c.rt.state.live.is_none(), || "the core is still held".into())?;
    c.restart()
}

fn one_instance(_: &mut Ctx) -> Res {
    use crate::shell::opens_window;
    expect(opens_window(false, || true), || "the first process opens no window".into())?;
    expect(!opens_window(false, || false), || "a second launch opens a window".into())?;
    expect(opens_window(true, || panic!("claimed")), || "a headless run opens nothing".into())
}

// ── Start-up (5)

pub const STARTUP: &[Scenario] = &[
    ("the first frame shows the splash", splash_first),
    ("main does no heavy work before the window runs", main_is_light),
    ("the splash goes only when every stage is done", splash_waits),
];

fn splash_first(_: &mut Ctx) -> Res {
    let mut rt = rune::App::new(crate::spec()).headless_runtime((1240.0, 820.0), rune::Env { system_dark: true, scale: 1.0 });
    rt.frame(0.0);
    expect(rt.find(keys::SPLASH).is_some(), || "no `splash` on the first frame (no splash is built yet: an owner decision, LOOP-9)".into())
}

/// `main` until `app.run()`: only the instance claim, the command line and the window's set-up;
/// no core, no file work, no waiting on a decode.
fn main_is_light(_: &mut Ctx) -> Res {
    let src = include_str!("../main.rs");
    let body = src.split("fn main()").nth(1).ok_or("no fn main")?;
    let before_run = body.split("app.run()").next().unwrap();
    let heavy: Vec<&str> = ["Core::", "live::start", "std::fs", "fs::", ".wait()", "thread::sleep", "ImageCache"]
        .into_iter()
        .filter(|w| before_run.contains(w))
        .collect();
    expect(heavy.is_empty(), || format!("main does {heavy:?} before run"))
}

/// A fresh start of the core: the splash is up on every frame while the first read is on its
/// way, and gone soon after it landed.
fn splash_waits(c: &mut Ctx) -> Res {
    use crate::gate::Loading;
    c.rt.state.live = None;
    c.attach();
    let mut frames = 0;
    loop {
        c.rt.frame(1.0 / 60.0);
        frames += 1;
        let booting = c.rt.state.loading == Loading::Booting;
        if (booting || frames == 1) && c.rt.find(keys::SPLASH).is_none() {
            return Err(format!("no `splash` on boot frame {frames} (no splash is built yet: an owner decision, LOOP-9)"));
        }
        if !booting || frames > 1200 {
            break;
        }
    }
    expect(c.rt.state.loading == Loading::Ready, || "the core never answered".into())?;
    c.frames(120);
    expect(c.rt.find(keys::SPLASH).is_none(), || "the splash stayed after the first read".into())
}
