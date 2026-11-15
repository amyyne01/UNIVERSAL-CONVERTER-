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
mod gate;
mod sample;

use gate::{Direction, State};
use rune::{App, AppSpec, Cx, El, Titlebar};

/// The window's title band, in points. Rune draws the three caption buttons over its trailing end.
pub const TITLEBAR: f32 = 44.0;

fn view(s: &State, cx: &mut Cx<State>) -> El {
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
        ..AppSpec::new("AHG Universal Converter", (1240.0, 820.0), gate::init, view)
    };
    App::new(spec).titlebar(Titlebar::new(TITLEBAR)).run();
}
