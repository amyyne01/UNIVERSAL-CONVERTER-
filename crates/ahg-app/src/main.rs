//! AHG Universal Converter's interface.
//!
//! For now it is the design gate: Home and Downloads drawn three ways over
//! prepared downloads (see `gate.rs`). Headless checks, from the workspace root:
//!
//! ```text
//! target/debug/AHG-Converter.exe --direction shelf --scene home-history --theme dark --snap out.png
//! target/debug/AHG-Converter.exe --list-scenes
//! ```
// Every string the screens show, word for word from the Electron app. A direction uses what it draws.
#[allow(dead_code)]
mod copy;
mod directions;
// Shared by every preset; they move onto it as each is next touched.
#[allow(dead_code)]
mod actions;
#[cfg(test)]
mod behaviour_tests;
#[allow(dead_code)]
mod format;
mod gate;
#[allow(dead_code)]
mod icons;
#[allow(dead_code)]
mod keys;
mod layouts;
#[allow(dead_code)]
mod live;
#[allow(dead_code)]
mod motion;
#[cfg(test)]
mod motion_tests;
#[allow(dead_code)]
mod overlays;
#[allow(dead_code)]
mod palette;
mod picker;
#[allow(dead_code)]
mod ring;
mod sample;
#[allow(dead_code)]
mod settings_spec;
#[allow(dead_code)]
mod timeline;

use gate::{Direction, State};
use rune::{App, AppSpec, Cx, El, Titlebar, Transition, el};

/// The window's title band, in points. Rune draws the three caption buttons over its trailing end.
pub const TITLEBAR: f32 = 44.0;

/// The preset on show, keyed by its place in the list so that a switch
/// dissolves the old one into the new one instead of cutting
/// (`docs/design-gate/MOTION-SYSTEM.md` 4.2): a fade with `motion::DISSOLVE`'s blur, no scale.
/// Under reduced motion Rune makes it its plain short fade without the blur.
fn view(s: &State, cx: &mut Cx<State>) -> El {
    const KEYS: [&str; 6] = ["preset-0", "preset-1", "preset-2", "preset-3", "preset-4", "preset-5"];
    let blur = match motion::DISSOLVE {
        motion::Dissolve::FadeBlur(blur) => blur,
        motion::Dissolve::Fade => 0.0,
    };
    let shown = el()
        .key(KEYS[gate::preset(s)])
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        // Below the overlays even while leaving: Rune lists a leaving element after its
        // siblings, which would paint the old preset over the picker the user just pressed.
        .paint_order(-1)
        .transition(Transition::blur(blur).springs(motion::DISSOLVE_SPRING))
        .child(preset_view(s, cx));
    // The window is filled with the shown preset's own ground, so the dissolve never shows
    // Rune's black clear colour; it springs to the new ground with the switch.
    let window = el().w_full().h_full().fill(backdrop(s, cx)).child(shown);
    // Empty when nothing is open over the preset.
    window.children(overlays::view(s, cx))
}

/// The fill of the shown preset's root, with the same palette and the same light or dark
/// reading the preset makes.
// ponytail: repeats each preset's choice of ground; a preset-side `ground(s, cx)` replaces this
// when the presets are next touched.
fn backdrop(s: &State, cx: &Cx<State>) -> rune::Color {
    #[allow(unused_variables)]
    let (system, chosen) = (cx.system_dark(), s.dark(cx));
    match (s.layout, s.direction) {
        #[cfg(feature = "bar")]
        (Some(gate::Layout::Bar), _) => directions::signal::theme::pal(s.palette, chosen).bg,
        #[cfg(feature = "collection")]
        (Some(gate::Layout::Collection), _) => directions::shelf::theme::Pal::new(s.palette, system).bg,
        #[cfg(feature = "stage")]
        (Some(gate::Layout::Stage), _) => directions::orbit::look::Pal::new(s.palette, chosen).bg,
        #[cfg(feature = "shelf")]
        (None, Direction::Shelf) => directions::shelf::theme::Pal::new(s.palette, system).sunk,
        #[cfg(feature = "signal")]
        (None, Direction::Signal) => directions::signal::theme::pal(s.palette, system).bg,
        #[cfg(feature = "orbit")]
        (None, Direction::Orbit) => directions::orbit::look::Pal::new(s.palette, chosen).bg,
        #[allow(unreachable_patterns)]
        _ => rune::Color::BLACK,
    }
}

fn preset_view(s: &State, cx: &mut Cx<State>) -> El {
    if s.ring {
        return ring::gallery(s, cx); // throwaway, removed at step 80 (K54)
    }
    match s.layout {
        #[cfg(feature = "bar")]
        Some(gate::Layout::Bar) => return layouts::bar::view(s, cx),
        #[cfg(feature = "collection")]
        Some(gate::Layout::Collection) => return layouts::collection::view(s, cx),
        #[cfg(feature = "stage")]
        Some(gate::Layout::Stage) => return layouts::stage::view(s, cx),
        _ => {}
    }
    match s.direction {
        #[cfg(feature = "shelf")]
        Direction::Shelf => directions::shelf::view(s, cx),
        #[cfg(feature = "signal")]
        Direction::Signal => directions::signal::view(s, cx),
        #[cfg(feature = "orbit")]
        Direction::Orbit => directions::orbit::view(s, cx),
        // A build with only some directions (one designer's own build).
        #[allow(unreachable_patterns)]
        _ => {
            let _ = cx;
            rune::text("This direction is not in this build")
        }
    }
}

/// Every face the three directions use, all under the SIL Open Font License
/// (`assets/fonts/OFL-*.txt`). The direction that wins keeps its own; the
/// rest are removed with the other two directions.
const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/Geist-Variable.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Variable.ttf"),
    include_bytes!("../assets/fonts/InstrumentSans-Variable.ttf"),
    include_bytes!("../assets/fonts/InstrumentSerif-Regular.ttf"),
    include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"),
    include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf"),
    include_bytes!("../assets/fonts/SpaceGrotesk-Variable.ttf"),
    include_bytes!("../assets/fonts/SpaceMono-Regular.ttf"),
];

/// The app's spec, shared by `main` and the scripted runtime in tests.
pub(crate) fn spec() -> AppSpec<State> {
    AppSpec::<State> {
        min_size: Some((960.0, 640.0)),
        scenes: gate::SCENES,
        fonts: FONTS,
        // The interface family and the one for figures, per direction (owner, 2026-11-15).
        families: gate::direction_from_args().families(),
        setup: Some(setup),
        poll: Some(live::poll),
        on_key: Some(gate::on_key),
        ..AppSpec::new("AHG Universal Converter", (1240.0, 820.0), gate::init, view)
    }
}

/// The sample covers, then live mode when `--live` named a folder.
fn setup(s: &mut State, waker: rune::Waker) {
    gate::setup(s, waker.clone());
    if let Some(folder) = live::FOLDER.get() {
        live::start(s, folder, waker);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match live::folder_from(&args) {
        Some(Ok(folder)) => drop(live::FOLDER.set(folder)),
        Some(Err(why)) => {
            eprintln!("--live refused: {why}");
            std::process::exit(2);
        }
        None => {}
    }
    // Quitting stops the core: its lookups first, then the downloads, then the save.
    App::new(spec()).titlebar(Titlebar::new(TITLEBAR)).on_quit(|s, _| s.live = None).run();
}
