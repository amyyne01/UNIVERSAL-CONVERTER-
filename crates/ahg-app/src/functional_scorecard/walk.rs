//! Stability: a seeded random walk over the drawn controls and the keys, 1000 actions per theme
//! (2000 per preset), checking the state's invariants as it goes and that the core still answers.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rune::{Key, KeyEvent, Mods};

use super::flows::found_one;
use super::{Ctx, Res, Scenario, expect};
use crate::gate::{Overlay, State};
use crate::live;

pub const STABILITY: &[Scenario] = &[("1000 random actions: no panic, nothing stuck, the core answers", walk)];

const ACTIONS: usize = 1000;

/// A small LCG: the same seed walks the same way.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self, below: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % below.max(1)
    }
}

/// What must hold after any action.
fn invariants(s: &State) -> Res {
    expect((s.overlay == Overlay::Picker) == s.picker, || format!("overlay {:?} with the picker {}", s.overlay, s.picker))?;
    expect(s.palette <= ahg_engine::config::PALETTE_IDS.len(), || format!("palette {}", s.palette))?;
    expect(s.selected.iter().all(|id| s.tasks.iter().any(|t| &t.task_id == id)), || "a selected row that is not in the list".into())?;
    expect(s.notices.len() <= 1, || format!("{} notices at once", s.notices.len()))?;
    let mut ids: Vec<&str> = s.tasks.iter().map(|t| t.task_id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    expect(ids.len() == s.tasks.len(), || "a task listed twice".into())
}

/// Esc four times closes whatever is open; then the core must answer a call.
fn checkpoint(c: &mut Ctx, at: usize, last: &str) -> Res {
    for _ in 0..4 {
        c.rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
        c.frames(3);
    }
    let s = &c.rt.state;
    expect(s.overlay == Overlay::None && !s.picker, || format!("action {at} ({last}): {:?} stuck open after Esc", s.overlay))?;
    let answered = Arc::new(AtomicBool::new(false));
    let flag = answered.clone();
    live::command(&c.rt.state, move |core| {
        let _ = core.settings();
        flag.store(true, Ordering::SeqCst);
        None
    });
    c.until("the core's answer", |_| answered.load(Ordering::SeqCst)).map_err(|e| format!("action {at} ({last}): {e}"))
}

fn walk(c: &mut Ctx) -> Res {
    c.json(&found_one());
    c.script("exit 0\n");
    let mut rng = Seeded(0x5eed ^ (c.preset as u64) << 8 ^ c.dark as u64);
    let keys = [
        Key::Escape,
        Key::Tab,
        Key::Enter,
        Key::Command('k'),
        Key::Command('1'),
        Key::Command('2'),
        Key::Command('3'),
        Key::Command('4'),
        Key::Command('7'),
        Key::Command('.'),
        Key::Command('/'),
        Key::Command('a'),
        Key::Text("x".into()),
        Key::Backspace,
    ];
    let words = ["khruangbin", super::flows::LINK, "https://not-a-video-site.example/", "  "];
    let mut last = String::new();
    for at in 0..ACTIONS {
        match rng.next(10) {
            0..=5 => {
                let shown: Vec<(String, [f32; 4])> =
                    c.rt.elements()
                        .into_iter()
                        .filter(|e| e.focusable && !e.exiting && !e.inert && e.visible[2] > 1.0 && e.visible[3] > 1.0)
                        .filter_map(|e| Some((e.key?, e.visible)))
                        .collect();
                if let Some((key, r)) = shown.get(rng.next(shown.len())).cloned() {
                    let (x, y) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                    c.rt.pointer_move(x, y);
                    c.rt.pointer_down(x, y);
                    c.rt.pointer_up(x, y);
                    last = format!("press `{key}`");
                }
            }
            6 | 7 => {
                let key = keys[rng.next(keys.len())].clone();
                let mods = if matches!(key, Key::Command(_)) { Mods { cmd: true, ctrl: true, ..Mods::default() } } else { Mods::default() };
                last = format!("key {key:?}");
                c.rt.key(KeyEvent { key, mods });
            }
            8 => {
                let text = words[rng.next(words.len())];
                last = format!("paste `{text}`");
                c.rt.update(|s, tx| crate::actions::paste_text(s, tx, text));
            }
            _ => {
                let (x, y) = (rng.next(1240) as f32, rng.next(820) as f32);
                c.rt.wheel(x, y, if rng.next(2) == 0 { 120.0 } else { -120.0 });
                last = "wheel".into();
            }
        }
        c.frames(2);
        // A dialog the walk opened is answered with nothing chosen.
        if c.rt.file_dialog().is_some() {
            let _ = c.rt.resolve_file_dialog(None);
            c.frames(1);
        }
        invariants(&c.rt.state).map_err(|e| format!("action {at} ({last}): {e}"))?;
        if at % 100 == 99 {
            checkpoint(c, at, &last)?;
        }
    }
    checkpoint(c, ACTIONS, &last)
}
