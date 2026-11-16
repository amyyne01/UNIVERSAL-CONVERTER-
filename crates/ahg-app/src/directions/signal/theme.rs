//! Signal's colours. Palette 0 is the desk's own: graphite surfaces with one
//! cue-blue accent, the colour of a lit meter segment. Palettes 1 to 7 are the
//! app's existing ones (Aurora, Slate, Moss, Ember, Paper, Lagoon, Orchid), read
//! from `src/palettes.css`. Platform and status colours are Aurora's in every
//! palette, as in the Electron app.
use ahg_engine::types::SourcePlatform;
use rune::{Color, oklch, oklcha};

/// Every colour a Signal view paints with.
pub struct Pal {
    pub dark: bool,
    /// Title bar and rail.
    pub deep: Color,
    /// The page.
    pub bg: Color,
    /// Cards and the list.
    pub surface: Color,
    /// Wells, the active rail item, the selected filter.
    pub raised: Color,
    pub line: Color,
    pub line_strong: Color,
    pub text: Color,
    pub text2: Color,
    pub muted: Color,
    /// Lit meter segments, the tally, the one figure that matters.
    pub accent: Color,
    /// The accent as ink on a surface (a link, a small figure).
    pub accent_ink: Color,
    /// An unlit meter segment.
    pub unlit: Color,
    pub hover: Color,
    pub press: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    /// A faint tint behind failed rows and the interrupted banner.
    pub error_soft: Color,
    pub warning_soft: Color,
    /// The dark wash under captions laid on cover art (never flips).
    pub media_scrim: Color,
    pub on_media: Color,
}

/// One palette in one mode: a hue for the neutrals, then lightness and chroma per role.
struct Ramp {
    hue: f32,
    deep: [f32; 2],
    bg: [f32; 2],
    raised: [f32; 2],
    surface: [f32; 2],
    text: [f32; 2],
    text2: [f32; 2],
    muted: [f32; 2],
    accent: [f32; 3],
}

const fn r(
    hue: f32,
    deep: [f32; 2],
    bg: [f32; 2],
    raised: [f32; 2],
    surface: [f32; 2],
    text: [f32; 2],
    text2: [f32; 2],
    muted: [f32; 2],
    accent: [f32; 3],
) -> Ramp {
    Ramp { hue, deep, bg, raised, surface, text, text2, muted, accent }
}

/// `[dark, light]` per palette, index = `s.palette`.
const RAMPS: [[Ramp; 2]; 8] = [
    // 0 Signal: graphite, a cue-blue segment.
    [
        r(
            250.0,
            [0.165, 0.004],
            [0.192, 0.005],
            [0.262, 0.006],
            [0.218, 0.005],
            [0.950, 0.004],
            [0.760, 0.006],
            [0.640, 0.008],
            [0.760, 0.150, 250.0],
        ),
        r(
            250.0,
            [0.958, 0.003],
            [0.978, 0.002],
            [0.938, 0.004],
            [0.997, 0.001],
            [0.230, 0.006],
            [0.430, 0.008],
            [0.520, 0.008],
            [0.540, 0.170, 255.0],
        ),
    ],
    // 1 Aurora
    [
        r(
            240.0,
            [0.122, 0.012],
            [0.150, 0.014],
            [0.235, 0.018],
            [0.205, 0.017],
            [0.955, 0.008],
            [0.760, 0.015],
            [0.650, 0.018],
            [0.800, 0.135, 195.0],
        ),
        r(
            240.0,
            [0.948, 0.006],
            [0.975, 0.005],
            [0.915, 0.008],
            [0.997, 0.002],
            [0.240, 0.020],
            [0.430, 0.020],
            [0.500, 0.018],
            [0.485, 0.105, 215.0],
        ),
    ],
    // 2 Slate
    [
        r(
            268.0,
            [0.140, 0.013],
            [0.170, 0.014],
            [0.255, 0.017],
            [0.225, 0.015],
            [0.945, 0.018],
            [0.770, 0.018],
            [0.665, 0.018],
            [0.720, 0.140, 258.0],
        ),
        r(
            268.0,
            [0.948, 0.007],
            [0.975, 0.005],
            [0.915, 0.008],
            [0.997, 0.002],
            [0.240, 0.018],
            [0.430, 0.018],
            [0.500, 0.018],
            [0.500, 0.170, 262.0],
        ),
    ],
    // 3 Moss
    [
        r(
            60.0,
            [0.150, 0.006],
            [0.185, 0.007],
            [0.270, 0.008],
            [0.240, 0.008],
            [0.940, 0.018],
            [0.760, 0.018],
            [0.660, 0.018],
            [0.800, 0.150, 128.0],
        ),
        r(
            60.0,
            [0.948, 0.007],
            [0.975, 0.005],
            [0.915, 0.008],
            [0.997, 0.002],
            [0.240, 0.018],
            [0.430, 0.018],
            [0.500, 0.018],
            [0.420, 0.140, 118.0],
        ),
    ],
    // 4 Ember
    [
        r(
            260.0,
            [0.125, 0.008],
            [0.155, 0.008],
            [0.235, 0.008],
            [0.205, 0.008],
            [0.955, 0.012],
            [0.732, 0.016],
            [0.622, 0.016],
            [0.800, 0.120, 36.0],
        ),
        r(
            95.0,
            [0.948, 0.006],
            [0.975, 0.006],
            [0.915, 0.006],
            [0.997, 0.003],
            [0.240, 0.009],
            [0.404, 0.012],
            [0.502, 0.012],
            [0.440, 0.140, 38.0],
        ),
    ],
    // 5 Paper
    [
        r(
            70.0,
            [0.125, 0.006],
            [0.155, 0.006],
            [0.235, 0.006],
            [0.205, 0.006],
            [0.955, 0.009],
            [0.732, 0.012],
            [0.622, 0.012],
            [0.720, 0.100, 65.0],
        ),
        r(
            75.0,
            [0.948, 0.008],
            [0.975, 0.008],
            [0.915, 0.008],
            [0.997, 0.002],
            [0.240, 0.012],
            [0.404, 0.016],
            [0.502, 0.016],
            [0.370, 0.080, 65.0],
        ),
    ],
    // 6 Lagoon
    [
        r(
            195.0,
            [0.125, 0.020],
            [0.155, 0.020],
            [0.235, 0.020],
            [0.205, 0.020],
            [0.955, 0.030],
            [0.730, 0.040],
            [0.620, 0.040],
            [0.880, 0.100, 170.0],
        ),
        r(
            190.0,
            [0.948, 0.010],
            [0.975, 0.010],
            [0.915, 0.010],
            [0.997, 0.004],
            [0.240, 0.015],
            [0.402, 0.020],
            [0.500, 0.020],
            [0.400, 0.090, 175.0],
        ),
    ],
    // 7 Orchid
    [
        r(
            320.0,
            [0.125, 0.020],
            [0.155, 0.020],
            [0.235, 0.020],
            [0.205, 0.020],
            [0.955, 0.030],
            [0.734, 0.040],
            [0.626, 0.040],
            [0.800, 0.170, 335.0],
        ),
        r(
            320.0,
            [0.948, 0.008],
            [0.975, 0.008],
            [0.915, 0.008],
            [0.997, 0.002],
            [0.240, 0.012],
            [0.406, 0.016],
            [0.504, 0.016],
            [0.520, 0.150, 335.0],
        ),
    ],
];

pub fn pal(palette: usize, dark: bool) -> Pal {
    let ramp = &RAMPS[palette.min(RAMPS.len() - 1)][usize::from(!dark)];
    let c = |lc: [f32; 2]| oklch(lc[0], lc[1], ramp.hue);
    let ink = |a: f32| oklcha(ramp.text[0], ramp.text[1], ramp.hue, a);
    let accent = oklch(ramp.accent[0], ramp.accent[1], ramp.accent[2]);
    // Signal's own light accent is a lit segment first; as ink it goes a step darker.
    let accent_ink = if palette == 0 && !dark { oklch(0.480, 0.160, 255.0) } else { accent };
    let (success, warning, error) = if dark {
        (oklch(0.740, 0.160, 150.0), oklch(0.830, 0.145, 85.0), oklch(0.680, 0.195, 24.0))
    } else {
        (oklch(0.480, 0.140, 150.0), oklch(0.500, 0.125, 80.0), oklch(0.520, 0.190, 24.0))
    };
    Pal {
        dark,
        deep: c(ramp.deep),
        bg: c(ramp.bg),
        surface: c(ramp.surface),
        raised: c(ramp.raised),
        line: ink(if dark { 0.08 } else { 0.09 }),
        line_strong: ink(if dark { 0.14 } else { 0.16 }),
        text: c(ramp.text),
        text2: c(ramp.text2),
        muted: c(ramp.muted),
        accent,
        accent_ink,
        unlit: ink(if dark { 0.09 } else { 0.08 }),
        hover: ink(0.05),
        press: ink(0.09),
        success,
        warning,
        error,
        error_soft: error.alpha(if dark { 0.07 } else { 0.05 }),
        warning_soft: warning.alpha(if dark { 0.08 } else { 0.08 }),
        media_scrim: oklcha(0.12, 0.012, 240.0, 0.82),
        on_media: oklch(0.955, 0.008, 220.0),
    }
}

/// A source's colour (Aurora's tokens; direct, generic and unknown links take the accent).
pub fn source_color(p: &Pal, source: SourcePlatform) -> Color {
    use SourcePlatform::*;
    let (l, c, h) = match (source, p.dark) {
        (Youtube, true) => (0.690, 0.220, 27.0),
        (Youtube, false) => (0.525, 0.210, 27.0),
        (Spotify, true) => (0.730, 0.180, 150.0),
        (Spotify, false) => (0.480, 0.150, 150.0),
        (Soundcloud, true) => (0.725, 0.190, 55.0),
        (Soundcloud, false) => (0.510, 0.140, 55.0),
        (Instagram | Tiktok | Facebook, true) => (0.700, 0.235, 352.0),
        (Instagram | Tiktok | Facebook, false) => (0.525, 0.220, 352.0),
        (Twitter, true) => (0.760, 0.020, 240.0),
        (Twitter, false) => (0.470, 0.020, 240.0),
        (Vimeo, true) => (0.760, 0.130, 230.0),
        (Vimeo, false) => (0.470, 0.140, 250.0),
        (Twitch, true) => (0.760, 0.130, 275.0),
        (Twitch, false) => (0.470, 0.120, 275.0),
        (Bandcamp, true) => (0.760, 0.130, 20.0),
        (Bandcamp, false) => (0.470, 0.120, 20.0),
        (Reddit, true) => (0.760, 0.130, 115.0),
        (Reddit, false) => (0.470, 0.120, 115.0),
        (Dailymotion, true) => (0.760, 0.130, 330.0),
        (Dailymotion, false) => (0.470, 0.120, 330.0),
        (Mixcloud, true) => (0.760, 0.130, 70.0),
        (Mixcloud, false) => (0.400, 0.090, 70.0),
        (Rumble, true) => (0.760, 0.130, 170.0),
        (Rumble, false) => (0.470, 0.120, 170.0),
        (Direct | Generic | Unknown, _) => return p.accent_ink,
    };
    oklch(l, c, h)
}
