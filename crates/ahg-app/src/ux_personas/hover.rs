//! Hover and focus, per preset: every control lights on hover within a frame, moves no
//! neighbour, leaves nothing lit after the pointer goes, and shows keyboard focus.
use rune::{ElementInfo, Key, KeyEvent, Mods};

use super::Ux;

/// The roles a user can press or pick.
const ROLES: &[&str] = &["button", "link", "tab", "radio", "checkbox", "switch", "menuitem", "option", "row", "combobox"];

/// The controls on show: key, centre and path, inside the window.
pub fn controls(ux: &Ux) -> Vec<(Option<String>, (f32, f32), String)> {
    ux.shown()
        .into_iter()
        .filter(|e| e.focusable && !e.inert && ROLES.contains(&e.role) && e.visible[2] >= 6.0 && e.visible[3] >= 6.0)
        .map(|e| (e.key.clone(), (e.visible[0] + e.visible[2] * 0.5, e.visible[1] + e.visible[3] * 0.5), e.path.clone()))
        .collect()
}

/// The pointer travels to `away` (two moves, as a real one sends; a single jump leaves a
/// control whose holder re-laid out under it lit until the next move), then the hover exits,
/// tweens that settle_all does not land, are waited out: 0.4 s is longer than any of them.
fn leave(ux: &mut Ux, away: (f32, f32)) {
    ux.rt.pointer_move(away.0 - 1.0, away.1 - 1.0);
    ux.rt.frame(1.0 / 60.0);
    ux.rt.pointer_move(away.0, away.1);
    ux.frames(24);
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3]
}

/// `a` is `b` or holds it.
fn holds(a: &str, b: &str) -> bool {
    b == a || b.strip_prefix(a).is_some_and(|rest| rest.starts_with('/'))
}

/// The control, what it holds, or a tight holder (at most four times its area) shows hover:
/// a big hover group (a sidebar) does not count for every control inside it.
pub(super) fn lit(els: &[ElementInfo], path: &str) -> bool {
    let area = |r: [f32; 4]| r[2] * r[3];
    let own = els.iter().find(|e| e.path == path).map_or(0.0, |e| area(e.rect));
    els.iter().any(|e| e.hover > 0.01 && (holds(path, &e.path) || (holds(&e.path, path) && area(e.rect) <= own * 4.0)))
}

/// What the control's own line paints: fills, ink, opacity and scale, by path.
pub(super) fn paint(els: &[ElementInfo], path: &str) -> Vec<String> {
    els.iter()
        .filter(|e| !e.exiting && (holds(&e.path, path) || holds(path, &e.path)))
        .map(|e| format!("{}{:?}{:?}{:.2}{:.3}", e.path, e.fill, e.text_color, e.opacity, e.scale))
        .collect()
}

/// Hovers every control of the screen in turn.
pub fn sweep(ux: &mut Ux, screen: &str) {
    let all = controls(ux);
    let (mut none, mut slow, mut shift) = (Vec::new(), Vec::new(), Vec::new());
    let name = |c: &(Option<String>, (f32, f32), String)| c.0.clone().unwrap_or_else(|| c.2.clone());
    // A spot inside the window over no control: where the pointer goes between controls.
    let away = [(1236.0, 816.0), (620.0, 816.0), (4.0, 816.0), (1236.0, 400.0), (620.0, 60.0)]
        .into_iter()
        .find(|&(x, y)| !ux.shown().iter().any(|e| e.focusable && inside(e.visible, x, y)))
        .unwrap_or((1236.0, 816.0));
    let mut stuck = Vec::new();
    for c in all.iter().take(40) {
        leave(ux, away);
        // Where the control is now: a hover sidebar listed opened is closed again, and what only
        // shows opened (its pin) is reached as a user does, through the strip at the window's edge.
        let at = ux.rt.find(&c.2).or_else(|| {
            ux.rt.pointer_move(8.0, c.1.1);
            ux.settle();
            ux.rt.find(&c.2)
        });
        let at = at.unwrap_or(c.1);
        let before = ux.rt.elements();
        ux.rt.pointer_move(at.0, at.1);
        ux.rt.frame(1.0 / 60.0);
        let first = ux.rt.elements();
        ux.settle();
        // What holds it may have opened under the pointer (a sidebar): follow the control.
        if let Some((x, y)) = ux.rt.find(&c.2) {
            ux.rt.pointer_move(x, y);
            ux.settle();
        }
        let after = ux.rt.elements();
        let track = lit(&after, &c.2);
        let painted = paint(&before, &c.2) != paint(&after, &c.2);
        if !track && !painted {
            none.push(name(c));
            continue;
        }
        if !lit(&first, &c.2) && paint(&before, &c.2) == paint(&first, &c.2) {
            slow.push(name(c));
        }
        // The sidebar that opens on hover is the design; elsewhere nothing may move.
        if !c.0.as_deref().is_some_and(|k| k.starts_with("nav-")) {
            for e in after.iter().filter(|e| e.key.is_some() && !e.exiting && !holds(&e.path, &c.2) && !holds(&c.2, &e.path)) {
                let moved =
                    before.iter().find(|b| b.path == e.path).is_some_and(|b| b.rect.iter().zip(e.rect).any(|(p, q)| (p - q).abs() > 0.5));
                if moved {
                    shift.push(format!("{} moves {}", name(c), e.key.clone().unwrap_or_default()));
                    break;
                }
            }
        }
        // The pointer leaves for the empty spot: the control's hover goes.
        leave(ux, away);
        let left = ux.rt.elements();
        if lit(&left, &c.2) && !left.iter().any(|e| e.focused && (holds(&e.path, &c.2) || holds(&c.2, &e.path))) {
            stuck.push(name(c));
        }
    }
    let tooltip = ux.rt.help_tooltip().is_some();
    let n = all.len().min(40);
    ux.part(format!("{screen}: every control has a hover state"), n - none.len(), n, format!("no hover on {none:?}"));
    ux.part(format!("{screen}: hover shows within one frame"), n - none.len() - slow.len(), n - none.len(), format!("late on {slow:?}"));
    ux.part(format!("{screen}: hover moves no neighbour"), n - shift.len(), n, format!("{shift:?}"));
    ux.check(
        format!("{screen}: nothing stays lit after the pointer leaves"),
        stuck.is_empty() && !tooltip,
        format!("lit {stuck:?}, tooltip {tooltip}"),
    );
}

/// Tab through the screen: focus moves through the controls and lands on what is on show.
/// (Whether a ring is drawn is not in the tree: `focus_visible` only says the focus came by keyboard.)
pub fn focus(ux: &mut Ux, screen: &str) {
    ux.rt.pointer_leave();
    let mut seen = std::collections::HashSet::new();
    let (mut stops, mut shown) = (0, 0);
    for _ in 0..30 {
        ux.rt.key(KeyEvent { key: Key::Tab, mods: Mods::default() });
        ux.settle();
        if let Some(e) = ux.rt.elements().into_iter().find(|e| e.focused) {
            stops += 1;
            seen.insert(e.path.clone());
            if !e.inert && e.visible[2] > 0.0 && e.visible[3] > 0.0 {
                shown += 1;
            }
        }
    }
    ux.check(format!("{screen}: Tab visits the controls"), seen.len() >= 5, format!("{} distinct stops", seen.len()));
    ux.part(format!("{screen}: Tab lands on controls on show"), shown, stops.max(1), "focus went to a hidden or inert element");
    ux.rt.update(|_, tx| tx.blur());
    ux.settle();
}
