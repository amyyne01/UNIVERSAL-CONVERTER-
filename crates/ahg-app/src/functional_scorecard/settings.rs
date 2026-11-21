//! Settings: each control drawn, each value written and kept across a restart of the core, the
//! theme, the palette, a preset switch, and Basic's locks.
use serde_json::{Value, json};

use super::{Ctx, Res, Scenario, expect};
use crate::actions;
use crate::gate::{Filter, Nav, PRESETS, Tone};
use crate::keys;
use crate::settings_spec::{self, Kind};

pub const SETTINGS: &[Scenario] = &[
    ("a toggle changes its setting", toggle),
    ("every setting has its control", every_control),
    ("every setting changes and survives a restart", every_value),
    ("the theme changes and survives a restart", theme),
    ("the palette changes and survives a restart", palette),
    ("a change made just before quitting is kept", write_then_quit),
    ("Basic's locked choice says why and writes nothing", basic_lock),
    ("a preset switch keeps the place and filter, and survives a restart", preset_switch),
];

/// One of Electron's toggles and the Rust-only one ("Needs review", kept in `settings-rust.json`).
fn toggle(c: &mut Ctx) -> Res {
    c.go(Nav::Settings)?;
    let mut stuck = Vec::new();
    for key in ["showNotifications", "holdUnconfirmed"] {
        let before = value(&c.rt.state, key);
        c.click(&keys::setting(key))?;
        c.frames(30);
        if value(&c.rt.state, key) == before {
            stuck.push(format!("`set-{key}` left it {before}"));
        }
    }
    expect(stuck.is_empty(), || stuck.join("; "))
}

/// The keyed settings rows, with their kinds.
fn rows(c: &Ctx) -> Vec<(&'static str, Kind, bool)> {
    settings_spec::sections(&c.rt.state)
        .into_iter()
        .flat_map(|s| s.rows)
        .filter_map(|r| {
            let locked = r.lock.is_some();
            Some((r.key?, r.kind, locked))
        })
        .collect()
}

fn every_control(c: &mut Ctx) -> Res {
    c.go(Nav::Settings)?;
    let mut missing = Vec::new();
    for (key, kind, _) in rows(c) {
        // The theme and palette rows are drawn as their choices; the layout row as its button.
        let control = match kind {
            Kind::Theme => keys::theme(0),
            Kind::Palette => keys::palette(1),
            Kind::Layout => keys::LAYOUT_CHOOSE.into(),
            Kind::Days => keys::day(0),
            Kind::Folder => keys::CHANGE_FOLDER.into(),
            _ => keys::setting(key),
        };
        if !c.has(&control) {
            missing.push(control);
        }
    }
    expect(missing.is_empty(), || format!("{} not drawn: {}", missing.len(), missing.join(", ")))
}

/// A setting's value; `holdUnconfirmed` is Rust-only and kept in `State.own`.
fn value(s: &crate::gate::State, key: &str) -> Value {
    if key == "holdUnconfirmed" {
        return json!(s.own.hold_unconfirmed);
    }
    s.settings.get(key).cloned().unwrap_or(Value::Null)
}

/// A different valid value for a row, or `None` when the kind has its own scenario.
fn other(c: &Ctx, key: &str, kind: &Kind) -> Option<Value> {
    let now = value(&c.rt.state, key);
    match kind {
        Kind::Toggle => Some(json!(!now.as_bool().unwrap_or(false))),
        Kind::Select(opts) => {
            let same = |v: &str| now.as_str() == Some(v) || now.as_u64().is_some_and(|n| v.parse() == Ok(n));
            let pick = opts.iter().find(|o| o.lock.is_none() && !same(&o.value))?;
            let number = ahg_engine::config::default_config().get(key).is_some_and(Value::is_number);
            Some(if number { pick.value.parse::<u64>().map(Value::from).unwrap_or(json!(pick.value)) } else { json!(pick.value) })
        }
        Kind::Time => Some(json!(if now == json!("07:30") { "08:45" } else { "07:30" })),
        Kind::Days => Some(json!([1, 3, 5])),
        Kind::Folder => {
            let other = c.dir.join("Other");
            let _ = std::fs::create_dir_all(&other);
            Some(json!(other.to_string_lossy()))
        }
        _ => None,
    }
}

fn every_value(c: &mut Ctx) -> Res {
    let mut wanted = Vec::new();
    let mut refused = Vec::new();
    for (key, kind, locked) in rows(c) {
        let Some(value) = other(c, key, &kind).filter(|_| !locked) else { continue };
        let v = value.clone();
        let ok = std::cell::Cell::new(false);
        c.rt.update(|s, _| ok.set(actions::set_setting(s, key, v)));
        if ok.get() { wanted.push((key, value)) } else { refused.push(key) }
    }
    c.frames(30);
    let invalid: Vec<String> = c.rt.state.invalid.iter().cloned().collect();
    c.restart()?;
    let lost: Vec<String> = wanted
        .iter()
        .filter(|(key, v)| value(&c.rt.state, key) != *v)
        .map(|(key, v)| format!("{key} ({v} -> {})", value(&c.rt.state, key)))
        .collect();
    expect(wanted.len() >= 10, || format!("only {} settings could be changed", wanted.len()))?;
    expect(refused.is_empty() && invalid.is_empty(), || format!("refused: {refused:?}, marked invalid by the core: {invalid:?}"))?;
    expect(lost.is_empty(), || format!("not kept across the restart: {}", lost.join(", ")))
}

fn theme(c: &mut Ctx) -> Res {
    c.go(Nav::Settings)?;
    // The choice that is not the system's: Light (0) on a dark system, Dark (1) on a light one.
    let (i, want) = if c.dark { (0, "light") } else { (1, "dark") };
    c.click(&keys::theme(i))?;
    c.frames(30);
    expect(c.rt.state.settings.get("theme") == Some(&json!(want)), || {
        format!("`set-theme-{i}` set {:?}", c.rt.state.settings.get("theme"))
    })?;
    c.restart()?;
    let kept = c.rt.state.settings.get("theme") == Some(&json!(want));
    c.go(Nav::Settings)?;
    c.click(&keys::theme(2))?;
    expect(kept, || "the theme was not kept across the restart".into())
}

fn palette(c: &mut Ctx) -> Res {
    c.go(Nav::Settings)?;
    c.click(&keys::palette(3))?;
    expect(c.rt.state.palette == 3, || format!("`set-palette-3` left palette {}", c.rt.state.palette))?;
    c.frames(30);
    c.restart()?;
    expect(c.rt.state.palette == 3, || format!("palette {} after the restart", c.rt.state.palette))?;
    c.go(Nav::Settings)?;
    c.click(&keys::palette(0))?;
    c.frames(30);
    c.restart()?;
    expect(c.rt.state.palette == 0, || format!("the look's own palette came back as {}", c.rt.state.palette))
}

fn write_then_quit(c: &mut Ctx) -> Res {
    c.go(Nav::Settings)?;
    let before = c.rt.state.settings.get("embedMetadata").and_then(Value::as_bool).unwrap_or(false);
    let k = keys::setting("embedMetadata");
    if !c.has(&k) {
        return Err(format!("no control `{k}` drawn"));
    }
    let (x, y) = c.rt.find(&k).unwrap();
    c.rt.pointer_move(x, y);
    c.rt.pointer_down(x, y);
    c.rt.pointer_up(x, y);
    // One frame for the press to run, then straight to quit.
    c.rt.frame(1.0 / 60.0);
    expect(c.rt.state.settings.get("embedMetadata") == Some(&json!(!before)), || "the press did not change it".into())?;
    c.restart()?;
    expect(c.rt.state.settings.get("embedMetadata") == Some(&json!(!before)), || "the change was lost at quit".into())
}

fn basic_lock(c: &mut Ctx) -> Res {
    let locked = settings_spec::sections(&c.rt.state).into_iter().flat_map(|s| s.rows).find_map(|r| match r.kind {
        Kind::Select(opts) => Some((r.key?, opts.into_iter().find(|o| o.lock.is_some())?)),
        _ => None,
    });
    let (key, opt) = locked.ok_or("no locked choice on Basic")?;
    let before = c.rt.state.settings.get(key).cloned();
    let value = opt.value.clone();
    c.rt.update(|s, tx| {
        actions::pick_setting(s, tx, key, &value);
    });
    c.frames(30);
    expect(c.rt.state.settings.get(key) == before.as_ref(), || format!("{key} was written"))?;
    expect(c.rt.state.notices.last().is_some_and(|n| n.tone == Tone::Premium), || "no premium notice".into())?;
    let why = opt.lock.unwrap();
    expect(c.drawn(&why), || format!("`{why}` is not drawn"))
}

fn preset_switch(c: &mut Ctx) -> Res {
    let (from, to) = (c.preset, (c.preset + 1) % PRESETS.len());
    // A finished row, so Downloads has its filters.
    let done = c.media("switch.mp3");
    c.script(&format!("out [FILEPATH]{}\nexit 0\n", done.display()));
    c.json(&super::flows::found_one());
    c.queue(super::flows::LINK);
    c.until("a finished row", |s| s.tasks.iter().any(|t| t.progress.status == ahg_engine::types::DownloadStatus::Done))?;
    c.go(Nav::Downloads)?;
    c.click(&keys::filter(Filter::Done))?;
    c.ctrl('.');
    expect(c.rt.state.picker, || "Ctrl+. did not open the picker".into())?;
    c.click(&keys::preset(to))?;
    let s = &c.rt.state;
    let now = crate::gate::preset(s);
    expect(now == to, || format!("`preset-card-{to}` left preset {now}"))?;
    expect(s.nav == Nav::Downloads && s.filter == Filter::Done, || format!("the switch moved to {:?} / {:?}", s.nav, s.filter))?;
    c.frames(30);
    c.restart()?;
    let kept = crate::gate::preset(&c.rt.state);
    // Back to this column's preset, so nothing after it runs on another.
    c.rt.update(move |s, tx| actions::set_preset(s, from, tx));
    c.frames(30);
    expect(kept == to, || format!("preset {kept} after the restart, not {to}"))
}
