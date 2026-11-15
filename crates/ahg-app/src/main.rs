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
mod format;
mod gate;
mod layouts;
mod picker;
mod sample;

use gate::{Direction, State};
use rune::{App, AppSpec, Cx, El, Titlebar, Transition, el};

/// The window's title band, in points. Rune draws the three caption buttons over its trailing end.
pub const TITLEBAR: f32 = 44.0;

/// The preset on show, keyed by its place in the list so that a switch
/// dissolves the old one into the new one instead of cutting.
fn view(s: &State, cx: &mut Cx<State>) -> El {
    const KEYS: [&str; 6] = ["preset-0", "preset-1", "preset-2", "preset-3", "preset-4", "preset-5"];
    let shown = el().key(KEYS[gate::preset(s)]).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).transition(Transition::blur_replace()).child(preset_view(s, cx));
    let window = el().w_full().h_full().child(shown);
    if s.picker { window.child(picker::view(s, cx)) } else { window }
}

fn preset_view(s: &State, cx: &mut Cx<State>) -> El {
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

fn main() {
    let spec = AppSpec::<State> {
        min_size: Some((960.0, 640.0)),
        scenes: gate::SCENES,
        fonts: FONTS,
        // The interface family and the one for figures, per direction (owner, 2026-11-15).
        families: gate::direction_from_args().families(),
        setup: Some(gate::setup),
        on_key: Some(gate::on_key),
        ..AppSpec::new("AHG Universal Converter", (1240.0, 820.0), gate::init, view)
    };
    App::new(spec).titlebar(Titlebar::new(TITLEBAR)).run();
}
