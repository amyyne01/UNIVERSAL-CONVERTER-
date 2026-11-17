//! The surfaces that sit over any preset, drawn once and coloured by a [`Skin`]: the command
//! palette (Ctrl+K), the keyboard shortcuts sheet (Ctrl+/), the notice stack, the update
//! prompt, and the preset picker (`picker.rs`). Behaviour is `actions`'; this file draws.
use std::cell::RefCell;

use rune::{Color, Cx, El, KeyEvent, ScrollSpec, Span, TextStyle, Tx, col, el, icon, rich, row, spacer, text};

use crate::actions::{self, Command};
use crate::gate::{Layout, Notice, Overlay, State, Tone};
use crate::{copy, icons, keys, picker};

/// How long a notice stays: 6 s for info and premium, 9 s for an error (`CONTENT.md` 1.6).
// TODO(motion): these are motion.rs tokens; move them there and set `Notice::until` from them.
pub const NOTICE_LIFE: f64 = 6.0;
// TODO(motion): as above.
pub const NOTICE_LIFE_ERROR: f64 = 9.0;

/// The overlays' colours. The shared look builds it from the active palette; a preset may
/// later pass its own.
#[derive(Debug, Clone, Copy)]
pub struct Skin {
    pub bg: Color,
    pub raised: Color,
    pub text: Color,
    pub text2: Color,
    pub line: Color,
    pub accent: Color,
    /// Words on the accent.
    pub on_accent: Color,
    /// The error tone's icon.
    pub error: Color,
}

impl Skin {
    /// From `palette::roles`. Palette 0 (the look's own mood) is not in that table: Aurora stands in.
    pub fn shared(s: &State, dark: bool) -> Skin {
        let r = crate::palette::roles(s.palette.saturating_sub(1), dark);
        Skin {
            bg: r.bg_primary,
            raised: r.bg_tertiary,
            text: r.text_primary,
            text2: r.text_secondary,
            line: r.border,
            accent: r.accent,
            on_accent: r.on_accent,
            error: r.error,
        }
    }
}

/// Every overlay on show, in paint order: the open sheet, then the notices above it. Empty
/// when nothing is open. Called on every view, so it also clears the palette's typing once closed.
pub fn view(s: &State, cx: &mut Cx<State>) -> Vec<El> {
    let k = Skin::shared(s, s.dark(cx));
    if s.overlay != Overlay::Palette {
        reset_palette();
    }
    let mut out = Vec::new();
    if s.picker {
        out.push(picker::view(s, cx));
    } else {
        let sheet = match s.overlay {
            // One bar keeps its own command panel (owner, Plan B decision 2).
            Overlay::Palette if s.layout != Some(Layout::Bar) => Some(palette(s, &k)),
            Overlay::Shortcuts => Some(shortcuts(s, &k)),
            Overlay::Update => update(s, &k),
            _ => None,
        };
        out.extend(sheet.map(scrim));
    }
    if !s.notices.is_empty() {
        let stack = col().key(keys::NOTICE).pin(None, None, Some(24.0), Some(24.0)).gap(8.0).items_end();
        out.push(stack.children(s.notices.iter().map(|n| notice(n, &k))));
    }
    out
}

// ── Shared pieces

fn style(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight)
}

fn keycap(k: &Skin, label: &str) -> El {
    el().h(20.0)
        .min_w(20.0)
        .px(6.0)
        .center()
        .radius(4.0)
        .fill(k.raised)
        .border(1.0, k.line)
        .child(text(label).font(style(11.0, 500.0).mono().features("'tnum' 1")).color(k.text2))
}

fn keycaps<'a>(k: &Skin, caps: impl IntoIterator<Item = &'a str>) -> El {
    row().gap(4.0).center_y().children(caps.into_iter().map(|c| keycap(k, c)))
}

/// A quiet icon button: the glyph in `text2`, a raised fill on hover.
fn icon_button(k: &Skin, key: &'static str, glyph: &'static rune::Icon, label: &'static str) -> El {
    el().key(key)
        .size(28.0, 28.0)
        .center()
        .radius(6.0)
        .role("button")
        .label(label)
        .help(label)
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .hover(k.raised)
        .child(icon(glyph, 14.0).color(k.text2))
}

/// A text button; `primary` is filled with the accent, the rest are hairline.
fn button(k: &Skin, key: &'static str, label: &str, primary: bool) -> El {
    let b = el().key(key).h(32.0).px(14.0).center().radius(8.0).role("button").label(label).focusable(true).cursor(rune::Cursor::Pointer);
    let words = text(label).font(style(13.0, 500.0));
    if primary {
        b.fill(k.accent).press_scale(0.98).child(words.color(k.on_accent))
    } else {
        b.border(1.0, k.line).hover(k.raised).press_scale(0.98).child(words.color(k.text))
    }
}

/// A sheet over the whole window, centred under a dim; a press on the dim closes it.
fn scrim(panel: El) -> El {
    // TODO(motion): the sheet's entry and exit (opacity and a small rise) come with the motion step.
    col()
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .items_center()
        .pt(crate::TITLEBAR + 72.0)
        .fill(Color::BLACK.alpha(0.45))
        .on_press(actions::close)
        .child(panel.on_press(|_: &mut State| {}).modal(true, actions::close))
}

fn panel(k: &Skin, key: &'static str, aria: &'static str, w: f32) -> El {
    col().key(key).w(w).role("dialog").label(aria).fill(k.bg).radius(12.0).clip_rounded(12.0).border(1.0, k.line).shadow(
        24.0,
        64.0,
        Color::BLACK.alpha(0.35),
    )
}

// ── The command palette

/// Fits the smallest window (640 pt) under the title bar; the list scrolls inside it.
const PALETTE_MAX_H: f32 = 500.0;

thread_local! {
    /// What is typed into the palette and the highlighted row.
    // TODO(gate): belongs in `State` (a `TextEdit` and an index, reset by `actions::open`/`close`);
    // gate.rs was not this step's to change. Cleared whenever the palette is not open.
    static TYPED: RefCell<(String, usize)> = const { RefCell::new((String::new(), 0)) };
}

fn reset_palette() {
    TYPED.with_borrow_mut(|t| *t = (String::new(), 0));
}

/// The commands that match what is typed: the label or the group name, ignoring case.
pub fn matching(s: &State, typed: &str) -> Vec<Command> {
    let q = typed.trim().to_lowercase();
    actions::commands(s)
        .into_iter()
        .filter(|c| q.is_empty() || c.label(s).to_lowercase().contains(&q) || c.group().to_lowercase().contains(&q))
        .collect()
}

/// The search line's keys: typing filters, Up and Down move, Enter runs. Esc, Ctrl+K and
/// Ctrl+V fall through to `gate::on_key`.
fn palette_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    use rune::Key;
    let (typed, at) = TYPED.with_borrow(|t| t.clone());
    let found = matching(s, &typed);
    let last = found.len().saturating_sub(1);
    let (typed, at) = match &ev.key {
        Key::Text(t) if !ev.mods.cmd && !ev.mods.ctrl => (typed + t, 0),
        Key::Backspace => (typed[..typed.char_indices().last().map_or(0, |(i, _)| i)].to_owned(), 0),
        Key::Down => (typed, if at >= last { 0 } else { at + 1 }),
        Key::Up => (typed, if at == 0 { last } else { at - 1 }),
        Key::Enter => {
            if let Some(&c) = found.get(at) {
                actions::run(s, tx, c);
            }
            return true;
        }
        _ => return false,
    };
    if let Some(i) = matching(s, &typed).get(at).and_then(|c| index(s, *c)) {
        tx.scroll_into_view(keys::command(i));
    }
    TYPED.with_borrow_mut(|t| *t = (typed, at));
    true
}

/// A command's place in the full list, for its key.
fn index(s: &State, c: Command) -> Option<usize> {
    actions::commands(s).iter().position(|x| *x == c)
}

fn palette(s: &State, k: &Skin) -> El {
    let (typed, at) = TYPED.with_borrow(|t| t.clone());
    let found = matching(s, &typed);
    let caret = el().size(1.5, 18.0).fill(k.accent);
    let words = if typed.is_empty() {
        row().center_y().child(caret).child(text(copy::PALETTE_PLACEHOLDER).font(style(14.0, 400.0).mono()).color(k.text2))
    } else {
        row().center_y().child(text(typed.clone()).font(style(14.0, 400.0).mono()).color(k.text)).child(caret)
    };
    let mut search = row()
        .key(keys::PALETTE_SEARCH)
        .h(52.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .role("searchbox")
        .label(copy::PALETTE_SEARCH_ARIA)
        .focusable(true)
        .autofocus()
        .no_focus_ring()
        .on_key(palette_key)
        .child(icon(&icons::CARET_RIGHT, 14.0).color(k.text2))
        .child(words)
        .child(spacer());
    if typed.is_empty() {
        search = search.child(icon(&icons::SEARCH, 14.0).color(k.text2));
    }
    search = search.child(keycap(k, copy::PALETTE_ESC_CAP));

    let mut list = col().px(8.0).py(8.0).gap(2.0);
    if found.is_empty() {
        list = list.child(el().h(72.0).center().child(text(copy::PALETTE_NO_MATCH).font(style(13.0, 400.0)).color(k.text2)));
    }
    let mut group = "";
    for (i, c) in found.iter().enumerate() {
        if c.group() != group {
            group = c.group();
            let label = text(format!("// {group}")).font(style(11.0, 500.0).mono()).color(k.text2);
            list = list.child(el().px(12.0).pt(if i == 0 { 4.0 } else { 12.0 }).pb(4.0).child(label));
        }
        list = list.child(command_row(s, k, *c, i == at));
    }
    let footer = row().h(36.0).px(16.0).gap(16.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], k.line).children(
        [
            (copy::PALETTE_FOOTER_NAVIGATE_CAP, copy::PALETTE_FOOTER_NAVIGATE),
            (copy::PALETTE_FOOTER_SELECT_CAP, copy::PALETTE_FOOTER_SELECT),
            (copy::PALETTE_FOOTER_CLOSE_CAP, copy::PALETTE_FOOTER_CLOSE),
        ]
        .map(|(cap, label)| row().gap(2.0).center_y().child(keycap(k, cap)).child(text(label).font(style(11.0, 400.0)).color(k.text2))),
    );
    panel(k, keys::PALETTE, copy::PALETTE_ARIA, 560.0)
        .max_h(PALETTE_MAX_H)
        .child(search)
        .child(el().h(1.0).w_full().fill(k.line))
        .child(list.shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }))
        .child(footer)
}

fn command_row(s: &State, k: &Skin, c: Command, on: bool) -> El {
    let key = keys::command(index(s, c).unwrap_or(0));
    let mut r = row()
        .key(key)
        .h(36.0)
        .px(12.0)
        .gap(12.0)
        .center_y()
        .radius(8.0)
        .role("option")
        .label(c.label(s))
        .checked(on)
        .cursor(rune::Cursor::Pointer)
        .hover(k.raised)
        .on_press_tx(move |s: &mut State, tx: &mut Tx| actions::run(s, tx, c));
    if on {
        r = r.fill(k.raised).child(el().size(2.0, 16.0).radius(1.0).fill(k.accent).absolute(0.0, 10.0));
    }
    r = r.child(text(c.label(s)).font(style(13.0, if on { 500.0 } else { 400.0 })).color(k.text)).child(spacer());
    match c.hint() {
        Some(hint) => r.child(keycaps(k, hint.split('+').collect::<Vec<_>>())),
        None => r,
    }
}

// ── The keyboard shortcuts sheet

fn shortcuts(s: &State, k: &Skin) -> El {
    let range = copy::keycap_tab_range(3);
    let mut rows: Vec<(&str, Vec<&str>)> = vec![
        (copy::SHORTCUT_PALETTE, vec![copy::KEY_CTRL, copy::KEY_K]),
        (copy::SHORTCUT_TABS, vec![copy::KEY_CTRL, &range]),
        (copy::SHORTCUT_PASTE, vec![copy::KEY_CTRL, copy::REST_KEY_V]),
        (copy::SHORTCUT_ENTER, vec![copy::KEYCAP_ENTER]),
        (copy::SHORTCUT_MOVE, copy::KEYCAP_ARROWS.to_vec()),
        (copy::SHORTCUT_SELECT, vec![copy::KEYCAP_SPACE]),
        (copy::SHORTCUT_SELECT_ALL, vec![copy::KEY_CTRL, copy::KEYCAP_A]),
        (copy::SHORTCUT_CLOSE, vec![copy::KEYCAP_ESC]),
        (copy::SHORTCUT_SHOW, vec![copy::KEY_CTRL, copy::KEYCAP_SLASH]),
    ];
    // Only when a global hotkey is set; `CommandOrControl` shows as Ctrl.
    let hotkey = s.settings.get("globalHotkey").and_then(|v| v.as_str()).unwrap_or("");
    if !hotkey.is_empty() {
        rows.push((copy::SHORTCUT_HOTKEY, hotkey.split('+').map(|p| if p == "CommandOrControl" { copy::KEY_CTRL } else { p }).collect()));
    }
    let head = row()
        .h(56.0)
        .pl(20.0)
        .pr(14.0)
        .center_y()
        .child(text(copy::SHORTCUTS_TITLE).font(style(15.0, 600.0)).color(k.text))
        .child(spacer())
        .child(icon_button(k, keys::SHORTCUTS_CLOSE, &icons::CLOSE, copy::WINDOW_CLOSE).on_press(actions::close));
    let list = col().px(20.0).pb(12.0).children(rows.into_iter().map(|(label, caps)| {
        row()
            .h(40.0)
            .gap(16.0)
            .center_y()
            .border_sides([1.0, 0.0, 0.0, 0.0], k.line)
            .child(text(label).font(style(13.0, 400.0)).color(k.text).truncate().grow(1.0).min_w(0.0))
            .child(keycaps(k, caps))
    }));
    panel(k, keys::SHORTCUTS, copy::SHORTCUTS_ARIA, 520.0).child(head).child(list)
}

// ── The update prompt

fn update(s: &State, k: &Skin) -> Option<El> {
    use copy::settings as w;
    let u = &s.update;
    let (title, body, primary, later) = match u.phase {
        "available" => (
            w::UPDATE_TITLE_AVAILABLE,
            w::UPDATE_BODY_AVAILABLE,
            Some((keys::DOWNLOAD_UPDATE, w::UPDATE_DOWNLOAD)),
            w::UPDATE_LATER_AVAILABLE,
        ),
        "downloading" => (w::UPDATE_TITLE_DOWNLOADING, w::UPDATE_BODY_DOWNLOADING, None, w::UPDATE_LATER_AVAILABLE),
        "ready" => (w::UPDATE_TITLE_READY, w::UPDATE_BODY_READY, Some((keys::INSTALL_UPDATE, w::RESTART_AND_INSTALL)), w::UPDATE_LATER),
        // Nothing to say in the other phases: the prompt stays closed.
        _ => return None,
    };
    let version = u.version.as_deref().unwrap_or("");
    let words = format!("{}{version}{}", body.0, body.1);
    let at = body.0.len();
    let body = rich(words, vec![Span::new(at, at + version.len()).mono().color(k.text)]).font(style(13.0, 400.0).lh(20.0)).color(k.text2);
    let mut buttons = row().gap(8.0).justify_end().child(button(k, keys::UPDATE_LATER, later, false).on_press(actions::close));
    if let Some((key, label)) = primary {
        let b = button(k, key, label, true);
        buttons = buttons.child(if key == keys::INSTALL_UPDATE {
            b.on_press_tx(actions::install_update)
        } else {
            b.on_press(actions::download_update)
        });
    }
    let mut p = panel(k, keys::UPDATE_PROMPT, title, 440.0)
        .padding(24.0)
        .gap(12.0)
        .child(text(title).font(style(16.0, 600.0)).color(k.text))
        .child(body);
    if u.phase == "downloading" {
        let done = u.percent.as_ref().and_then(|p| p.as_f64()).unwrap_or(0.0).clamp(0.0, 100.0) as f32 / 100.0;
        p = p.child(el().h(4.0).w_full().radius(2.0).fill(k.raised).child(el().h(4.0).w_frac(done).radius(2.0).fill(k.accent)));
    }
    Some(p.child(buttons.mt(8.0)))
}

// ── Notices

fn notice(n: &Notice, k: &Skin) -> El {
    // TODO(motion): entry, exit and the stack's reflow; `n.until` from NOTICE_LIFE / NOTICE_LIFE_ERROR.
    let glyph = match n.tone {
        Tone::Info => el().size(8.0, 8.0).radius(4.0).fill(k.accent),
        Tone::Error => icon(&icons::ALERT, 16.0).color(k.error),
        Tone::Premium => icon(&icons::LOCK_SIGNAL, 16.0).color(k.accent),
    };
    let tile = el().size(30.0, 30.0).radius(8.0).center().fill(k.raised).child(glyph);
    let mut r = row()
        .max_w(380.0)
        .pl(10.0)
        .pr(8.0)
        .py(8.0)
        .gap(12.0)
        .center_y()
        .role("status")
        .fill(k.bg)
        .radius(12.0)
        .border(1.0, k.line)
        .shadow(8.0, 24.0, Color::BLACK.alpha(0.25))
        .child(tile)
        .child(text(n.text.clone()).font(style(13.0, 500.0).lh(18.0)).color(k.text).wrap().shrink().min_w(0.0));
    if let Some(act) = &n.action {
        let label = act.label();
        let b = el()
            .key(keys::NOTICE_ACTION)
            .h(28.0)
            .px(10.0)
            .center()
            .radius(6.0)
            .role("button")
            .label(label)
            .focusable(true)
            .cursor(rune::Cursor::Pointer)
            .hover(k.raised)
            .on_press_tx(actions::notice_action)
            .child(text(label).font(style(13.0, 500.0)).color(k.accent));
        r = r.child(b);
    }
    r.child(icon_button(k, keys::NOTICE_DISMISS, &icons::CLOSE, copy::NOTICE_DISMISS).on_press(actions::dismiss_notice))
}

#[cfg(test)]
mod tests {
    use rune::{App, Env, Key, KeyEvent, Mods, Runtime};

    use super::*;
    use crate::gate::Nav;

    fn runtime(preset: usize) -> Runtime<State> {
        let mut rt = App::new(crate::spec()).headless_runtime((1240.0, 820.0), Env { system_dark: true, scale: 1.0 });
        rt.frame(0.0);
        rt.update(move |s, tx| actions::set_preset(s, preset, tx));
        settle(&mut rt);
        rt
    }
    fn settle(rt: &mut Runtime<State>) {
        for _ in 0..30 {
            rt.frame(1.0 / 60.0);
        }
    }
    fn press(rt: &mut Runtime<State>, key: Key, mods: Mods) {
        rt.key(KeyEvent { key, mods });
        settle(rt);
    }
    fn ctrl_k(rt: &mut Runtime<State>) {
        press(rt, Key::Command('k'), Mods { cmd: true, ctrl: true, ..Mods::default() });
    }
    /// The palette rows on screen, by label, in order.
    fn rows(rt: &Runtime<State>) -> Vec<String> {
        rt.elements()
            .into_iter()
            .filter(|e| !e.exiting && e.role == "option" && e.key.as_deref().is_some_and(|k| k.starts_with("command-")))
            .filter_map(|e| e.label)
            .collect()
    }
    fn shown(rt: &Runtime<State>, key: &str) -> bool {
        // A preset may use the same key (Collection's search capsule is `palette`): only the overlay's top-level surfaces count.
        rt.elements().iter().any(|e| !e.exiting && e.key.as_deref() == Some(key) && !e.path.contains("preset-"))
    }

    #[test]
    fn ctrl_k_opens_typing_filters_enter_runs_escape_closes() {
        for preset in [1, 2, 3, 4, 5] {
            let mut rt = runtime(preset);
            ctrl_k(&mut rt);
            assert_eq!(rt.state.overlay, Overlay::Palette, "preset {preset}");
            assert!(shown(&rt, keys::PALETTE), "preset {preset}: the palette is drawn");
            assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some(keys::PALETTE_SEARCH) && e.focused), "the search line has focus");
            let all = rows(&rt);
            assert_eq!(all.len(), actions::commands(&rt.state).len(), "every command is listed");

            for c in "downl".chars() {
                press(&mut rt, Key::Text(c.to_string()), Mods::default());
            }
            let some = rows(&rt);
            assert!(!some.is_empty() && some.len() < all.len(), "typing filters: {some:?}");
            assert_eq!(some[0], copy::palette_go_to(copy::NAV_DOWNLOADS));

            press(&mut rt, Key::Enter, Mods::default());
            assert_eq!((rt.state.overlay, rt.state.nav), (Overlay::None, Nav::Downloads), "Enter ran Go to Downloads");
            assert!(!shown(&rt, keys::PALETTE));

            // Reopened, it starts empty again; Escape closes it.
            ctrl_k(&mut rt);
            assert_eq!(rows(&rt).len(), all.len(), "the typed text was cleared");
            press(&mut rt, Key::Escape, Mods::default());
            assert_eq!(rt.state.overlay, Overlay::None);
            assert!(!shown(&rt, keys::PALETTE));
        }
    }

    #[test]
    fn arrows_move_the_highlight_and_nothing_matches() {
        let mut rt = runtime(3);
        ctrl_k(&mut rt);
        press(&mut rt, Key::Down, Mods::default());
        press(&mut rt, Key::Down, Mods::default());
        press(&mut rt, Key::Enter, Mods::default());
        assert_eq!(rt.state.nav, Nav::Settings, "the third row is Go to Settings");
        ctrl_k(&mut rt);
        press(&mut rt, Key::Up, Mods::default());
        press(&mut rt, Key::Enter, Mods::default());
        assert!(!rt.state.tasks.iter().any(crate::format::is_active), "Up from the first wraps to the last, Cancel all");
        ctrl_k(&mut rt);
        for c in "zzzz".chars() {
            press(&mut rt, Key::Text(c.to_string()), Mods::default());
        }
        assert!(rows(&rt).is_empty());
        assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some(copy::PALETTE_NO_MATCH)));
        press(&mut rt, Key::Backspace, Mods::default());
        assert!(rows(&rt).is_empty(), "zzz still matches nothing");
    }

    #[test]
    fn one_bar_keeps_its_own_panel() {
        let mut rt = runtime(0);
        ctrl_k(&mut rt);
        assert_eq!(rt.state.overlay, Overlay::Palette);
        assert!(!shown(&rt, keys::PALETTE), "One bar does not draw the shared palette");
    }

    #[test]
    fn the_update_prompt_in_its_three_phases() {
        let mut rt = runtime(3);
        for (phase, button, then) in [
            ("available", keys::DOWNLOAD_UPDATE, "downloading"),
            ("downloading", keys::UPDATE_LATER, "downloading"),
            ("ready", keys::INSTALL_UPDATE, "ready"),
        ] {
            rt.update(move |s, _| {
                s.update.phase = phase;
                s.update.version = Some("2.0.1".into());
                actions::open(s, Overlay::Update);
            });
            settle(&mut rt);
            assert!(shown(&rt, keys::UPDATE_PROMPT), "{phase}");
            assert!(rt.elements().iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains("2.0.1"))), "{phase}: the version is shown");
            let (x, y) = rt.find(button).unwrap_or_else(|| panic!("{phase}: no {button}"));
            rt.pointer_move(x, y);
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            settle(&mut rt);
            assert_eq!(rt.state.update.phase, then, "{phase}");
        }
        rt.update(|s, _| {
            s.update.phase = "idle";
            actions::open(s, Overlay::Update);
        });
        settle(&mut rt);
        assert!(!shown(&rt, keys::UPDATE_PROMPT), "idle has nothing to prompt");
    }

    #[test]
    fn a_notice_runs_its_action_and_dismisses() {
        let mut rt = runtime(4);
        rt.update(|s, tx| actions::cancel(s, tx, "big-buck-bunny"));
        settle(&mut rt);
        assert!(shown(&rt, keys::NOTICE_ACTION), "the Undo button");
        let (x, y) = rt.find(keys::NOTICE_ACTION).expect("Undo");
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        settle(&mut rt);
        assert!(rt.state.notices.is_empty());
        assert!(!shown(&rt, keys::NOTICE));
    }
}
