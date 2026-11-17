//! Orbit's colours and type. One neutral ramp per palette and theme, one accent, and the
//! platform and status colours, which no palette overrides (Electron's `index.css` B2).

use ahg_engine::types::SourcePlatform;
use rune::{Color, El, TextStyle, oklch, text};

/// Palette 0 is Orbit's own: deep ink by night, a chart on paper by day. 1 to 7 are the
/// app's palettes in the order Settings lists them: Aurora, Slate, Moss, Ember, Paper,
/// Lagoon, Orchid. Each row: deepest, page, raised, hover, card, text, secondary, muted,
/// accent, ink on the accent (OKLCH).
type Ramp = [[f32; 3]; 10];
const RAMPS: [[Ramp; 2]; 8] = [
    // Orbit
    [
        [
            [0.135, 0.012, 262.0],
            [0.158, 0.013, 262.0],
            [0.215, 0.016, 262.0],
            [0.245, 0.017, 262.0],
            [0.184, 0.014, 262.0],
            [0.950, 0.006, 262.0],
            [0.765, 0.014, 262.0],
            [0.650, 0.016, 262.0],
            [0.780, 0.110, 238.0],
            [0.160, 0.020, 262.0],
        ],
        [
            [0.952, 0.006, 85.0],
            [0.974, 0.005, 85.0],
            [0.938, 0.007, 85.0],
            [0.922, 0.008, 85.0],
            [0.992, 0.003, 85.0],
            [0.250, 0.032, 262.0],
            [0.440, 0.028, 262.0],
            [0.530, 0.022, 262.0],
            [0.480, 0.140, 255.0],
            [0.985, 0.003, 85.0],
        ],
    ],
    // Aurora
    [
        [
            [0.122, 0.012, 240.0],
            [0.150, 0.014, 240.0],
            [0.190, 0.016, 240.0],
            [0.235, 0.018, 240.0],
            [0.205, 0.017, 240.0],
            [0.955, 0.008, 220.0],
            [0.760, 0.015, 225.0],
            [0.650, 0.018, 230.0],
            [0.800, 0.135, 195.0],
            [0.170, 0.020, 240.0],
        ],
        [
            [0.948, 0.006, 240.0],
            [0.975, 0.005, 240.0],
            [0.930, 0.007, 240.0],
            [0.915, 0.008, 240.0],
            [0.997, 0.002, 240.0],
            [0.240, 0.020, 240.0],
            [0.430, 0.020, 238.0],
            [0.500, 0.018, 235.0],
            [0.485, 0.105, 215.0],
            [0.985, 0.003, 220.0],
        ],
    ],
    // Slate
    [
        [
            [0.140, 0.013, 268.0],
            [0.170, 0.014, 268.0],
            [0.210, 0.015, 268.0],
            [0.255, 0.017, 268.0],
            [0.225, 0.015, 268.0],
            [0.945, 0.018, 268.0],
            [0.770, 0.018, 268.0],
            [0.665, 0.018, 268.0],
            [0.720, 0.140, 258.0],
            [0.170, 0.020, 268.0],
        ],
        [
            [0.948, 0.007, 268.0],
            [0.975, 0.005, 268.0],
            [0.930, 0.007, 268.0],
            [0.915, 0.008, 268.0],
            [0.997, 0.002, 268.0],
            [0.240, 0.018, 268.0],
            [0.430, 0.018, 268.0],
            [0.500, 0.018, 268.0],
            [0.500, 0.170, 262.0],
            [0.985, 0.003, 268.0],
        ],
    ],
    // Moss
    [
        [
            [0.150, 0.006, 60.0],
            [0.185, 0.007, 60.0],
            [0.225, 0.008, 60.0],
            [0.270, 0.008, 60.0],
            [0.240, 0.008, 60.0],
            [0.940, 0.018, 60.0],
            [0.760, 0.018, 60.0],
            [0.660, 0.018, 60.0],
            [0.800, 0.150, 128.0],
            [0.170, 0.020, 60.0],
        ],
        [
            [0.948, 0.007, 60.0],
            [0.975, 0.005, 60.0],
            [0.930, 0.007, 60.0],
            [0.915, 0.008, 60.0],
            [0.997, 0.002, 60.0],
            [0.240, 0.018, 60.0],
            [0.430, 0.018, 60.0],
            [0.500, 0.018, 60.0],
            [0.420, 0.140, 118.0],
            [0.985, 0.003, 60.0],
        ],
    ],
    // Ember
    [
        [
            [0.125, 0.008, 260.0],
            [0.155, 0.008, 260.0],
            [0.190, 0.008, 260.0],
            [0.235, 0.008, 260.0],
            [0.205, 0.008, 260.0],
            [0.955, 0.012, 260.0],
            [0.732, 0.016, 260.0],
            [0.622, 0.016, 260.0],
            [0.800, 0.120, 36.0],
            [0.170, 0.020, 260.0],
        ],
        [
            [0.948, 0.006, 95.0],
            [0.975, 0.006, 95.0],
            [0.930, 0.006, 95.0],
            [0.915, 0.006, 95.0],
            [0.997, 0.003, 95.0],
            [0.240, 0.009, 95.0],
            [0.404, 0.012, 95.0],
            [0.502, 0.012, 95.0],
            [0.440, 0.140, 38.0],
            [0.985, 0.003, 95.0],
        ],
    ],
    // Paper
    [
        [
            [0.125, 0.006, 70.0],
            [0.155, 0.006, 70.0],
            [0.190, 0.006, 70.0],
            [0.235, 0.006, 70.0],
            [0.205, 0.006, 70.0],
            [0.955, 0.009, 70.0],
            [0.732, 0.012, 70.0],
            [0.622, 0.012, 70.0],
            [0.720, 0.100, 65.0],
            [0.170, 0.020, 70.0],
        ],
        [
            [0.948, 0.008, 75.0],
            [0.975, 0.008, 75.0],
            [0.930, 0.008, 75.0],
            [0.915, 0.008, 75.0],
            [0.997, 0.002, 75.0],
            [0.240, 0.012, 75.0],
            [0.404, 0.016, 75.0],
            [0.502, 0.016, 75.0],
            [0.370, 0.080, 65.0],
            [0.985, 0.003, 75.0],
        ],
    ],
    // Lagoon
    [
        [
            [0.125, 0.020, 195.0],
            [0.155, 0.020, 195.0],
            [0.190, 0.020, 195.0],
            [0.235, 0.020, 195.0],
            [0.205, 0.020, 195.0],
            [0.955, 0.030, 195.0],
            [0.730, 0.040, 195.0],
            [0.620, 0.040, 195.0],
            [0.880, 0.100, 170.0],
            [0.170, 0.020, 195.0],
        ],
        [
            [0.948, 0.010, 190.0],
            [0.975, 0.010, 190.0],
            [0.930, 0.010, 190.0],
            [0.915, 0.010, 190.0],
            [0.997, 0.004, 190.0],
            [0.240, 0.015, 190.0],
            [0.402, 0.020, 190.0],
            [0.500, 0.020, 190.0],
            [0.400, 0.090, 175.0],
            [0.985, 0.003, 190.0],
        ],
    ],
    // Orchid
    [
        [
            [0.125, 0.020, 320.0],
            [0.155, 0.020, 320.0],
            [0.190, 0.020, 320.0],
            [0.235, 0.020, 320.0],
            [0.205, 0.020, 320.0],
            [0.955, 0.030, 320.0],
            [0.734, 0.040, 320.0],
            [0.626, 0.040, 320.0],
            [0.800, 0.170, 335.0],
            [0.170, 0.020, 320.0],
        ],
        [
            [0.948, 0.008, 320.0],
            [0.975, 0.008, 320.0],
            [0.930, 0.008, 320.0],
            [0.915, 0.008, 320.0],
            [0.997, 0.002, 320.0],
            [0.240, 0.012, 320.0],
            [0.406, 0.016, 320.0],
            [0.504, 0.016, 320.0],
            [0.520, 0.150, 335.0],
            [0.985, 0.003, 320.0],
        ],
    ],
];

/// Every colour a screen uses. Hairlines are pre-mixed opaque (translucent strokes bead at joints).
#[derive(Clone, Copy)]
pub struct Pal {
    pub dark: bool,
    pub deep: Color,
    pub bg: Color,
    pub raised: Color,
    pub hover: Color,
    pub card: Color,
    pub text: Color,
    pub text2: Color,
    pub muted: Color,
    pub accent: Color,
    pub on_accent: Color,
    /// Hairline on the page and on cards.
    pub line: Color,
    /// The orbit's track and control edges: a step stronger than a hairline.
    pub track: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub youtube: Color,
    pub spotify: Color,
    pub soundcloud: Color,
    pub reels: Color,
}

impl Pal {
    pub fn new(palette: usize, dark: bool) -> Pal {
        // Palette 0 is Orbit's own table; 1 to 7 are the app's, from the one colour source.
        let own = |i: usize| {
            let r = &RAMPS[0][usize::from(!dark)];
            oklch(r[i][0], r[i][1], r[i][2])
        };
        let shared = crate::palette::roles(palette.saturating_sub(1), dark);
        let c = |i: usize, shared: Color| if palette == 0 { own(i) } else { shared };
        let (bg, text) = (c(1, shared.bg_primary), c(5, shared.text_primary));
        // The semantic colours are the same in every palette.
        let tone = crate::palette::roles(0, dark);
        let platform = |src| crate::palette::platform(src, dark);
        Pal {
            dark,
            deep: c(0, shared.bg_secondary),
            bg,
            raised: c(2, shared.bg_tertiary),
            hover: c(3, shared.bg_hover),
            card: c(4, shared.bg_surface),
            text,
            text2: c(6, shared.text_secondary),
            muted: c(7, shared.text_muted),
            accent: c(8, shared.accent),
            on_accent: c(9, shared.on_accent),
            line: mix(bg, text, if dark { 0.15 } else { 0.12 }),
            track: mix(bg, text, if dark { 0.28 } else { 0.24 }),
            success: tone.success,
            warning: tone.warning,
            error: tone.error,
            youtube: platform(SourcePlatform::Youtube),
            spotify: platform(SourcePlatform::Spotify),
            soundcloud: platform(SourcePlatform::Soundcloud),
            reels: platform(SourcePlatform::Instagram),
        }
    }

    /// The colour of a "more" source (X, Vimeo, ...), as `src-<name>` in Electron.
    pub fn more(&self, hue: f32, chroma: f32) -> Color {
        if self.dark { oklch(0.760, chroma, hue) } else { oklch(0.470, light_chroma(hue, chroma), hue) }
    }
}

// Type: Space Grotesk for the interface (400 and 500), Space Mono for every figure (its one
// weight, tabular by design). One scale: 12, 14, 16, 20, 32, 40; line heights on the 4 pt grid.
pub const DISPLAY: TextStyle = TextStyle::new(40.0, 500.0).lh(48.0).tracking(-0.8);
pub const PAGE_TITLE: TextStyle = TextStyle::new(32.0, 500.0).lh(40.0).tracking(-0.5);
pub const CELL_TITLE: TextStyle = TextStyle::new(16.0, 500.0).lh(24.0);
pub const BODY: TextStyle = TextStyle::new(14.0, 400.0).lh(20.0);
pub const BODY_M: TextStyle = TextStyle::new(14.0, 500.0).lh(20.0);
pub const SMALL: TextStyle = TextStyle::new(12.0, 400.0).lh(16.0);
pub const SMALL_M: TextStyle = TextStyle::new(12.0, 500.0).lh(16.0);
pub const FIG: TextStyle = TextStyle::new(12.0, 400.0).lh(16.0).mono();
pub const FIG_S: TextStyle = FIG;
pub const FIG_M: TextStyle = TextStyle::new(16.0, 400.0).lh(20.0).mono();
pub const FIG_L: TextStyle = TextStyle::new(20.0, 400.0).lh(28.0).mono();
pub const FIG_XL: TextStyle = TextStyle::new(40.0, 400.0).lh(48.0).mono().tracking(-1.0);

pub fn txt(s: impl Into<String>, style: TextStyle, color: Color) -> El {
    text(s).font(style).color(color)
}

/// `a` towards `b` by `t`, in OKLab, so a few percent of a bright colour stays a few percent
/// on a dark surface (Rune's `Color::mix` works in linear light).
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let (x, y) = (a.to_oklab(), b.to_oklab());
    Color::from_oklab([0, 1, 2, 3].map(|i| x[i] + (y[i] - x[i]) * t))
}

/// Light-theme chroma of a "more" source: Vimeo 0.140, X stays grey, the rest 0.120.
fn light_chroma(hue: f32, dark_chroma: f32) -> f32 {
    if dark_chroma < 0.05 {
        dark_chroma
    } else if hue == 250.0 {
        0.140
    } else {
        0.120
    }
}
