//! Shelf's colours. Palette 0 is Shelf's own mood: warm paper-and-ink
//! neutrals with a blue-black ink accent (the stamp on a liner note). Palettes
//! 1 to 7 are the app's existing ones (Aurora, Slate, Moss, Ember, Paper,
//! Lagoon, Orchid), token for token from the Electron stylesheet. Platform and
//! semantic colours never change with the palette, as in Electron.
use ahg_engine::types::SourcePlatform;
use rune::{Color, TextStyle, oklch, oklcha, tok::Family};

use crate::overlays::{Mark, Skin};
use crate::{gate, palette};

/// Every colour the direction draws with. Nothing in the views holds a literal.
#[derive(Clone, Copy)]
pub struct Pal {
    pub dark: bool,
    /// Title bar and rail: the deepest surface.
    pub sunk: Color,
    /// The page.
    pub bg: Color,
    /// The list surface and the cells on Home.
    pub card: Color,
    /// Inputs, an active rail item, keycaps, hover.
    pub raised: Color,
    pub text: Color,
    pub text2: Color,
    pub muted: Color,
    pub accent: Color,
    /// Hairlines.
    pub line: Color,
    pub line_soft: Color,
    /// Control boundaries (checkboxes, the field), at least 3:1.
    pub control: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    platforms: [Color; 4],
    others: [Color; 8],
}

impl Pal {
    /// Shelf's hover on what is already raised (the chosen chip, the place you are in): the
    /// card pressed a shade into the ink, where the others lift to the raised paper.
    pub fn pressed_in(&self) -> Color {
        self.raised.mix(self.text, 0.06)
    }

    pub fn new(palette: usize, dark: bool) -> Pal {
        if palette == 0 { own(dark) } else { existing(palette, dark) }
    }

    /// A platform's colour, for sources (rows, sleeves, the split on Home).
    pub fn source(&self, p: SourcePlatform) -> Color {
        use SourcePlatform::*;
        match p {
            Youtube => self.platforms[0],
            Spotify => self.platforms[1],
            Soundcloud => self.platforms[2],
            Instagram | Tiktok | Facebook => self.platforms[3],
            Twitter => self.others[0],
            Vimeo => self.others[1],
            Twitch => self.others[2],
            Bandcamp => self.others[3],
            Reddit => self.others[4],
            Dailymotion => self.others[5],
            Mixcloud => self.others[6],
            Rumble => self.others[7],
            Direct | Generic | Unknown => self.accent,
        }
    }

    pub fn youtube(&self) -> Color {
        self.platforms[0]
    }
    pub fn spotify(&self) -> Color {
        self.platforms[1]
    }
    pub fn soundcloud(&self) -> Color {
        self.platforms[2]
    }
    pub fn reels(&self) -> Color {
        self.platforms[3]
    }

    /// A soft wash of a colour over the page, for tinted rows and the banner.
    pub fn soft(&self, c: Color) -> Color {
        c.alpha(if self.dark { 0.10 } else { 0.08 })
    }
}

/// The overlays on the shelf: a liner-note card with soft 16 pt corners, titles in the serif,
/// the highlighted row a paper well with its words inked in the accent. Words on the accent in
/// the shelf's deepest paper, as on its own badges.
pub fn skin(palette: usize, dark: bool) -> Skin {
    let p = Pal::new(palette, dark);
    Skin {
        dark,
        bg: p.card,
        raised: p.raised,
        text: p.text,
        text2: p.text2,
        line: p.line,
        accent: p.accent,
        on_accent: p.sunk,
        error: p.error,
        radius: 16.0,
        mark: Mark::Ink,
        mono: false,
        serif: true,
    }
}

// ── The sleeve's own materials: physical things, the same in every palette ──

/// The paper of a spine label.
pub fn label_paper() -> Color {
    oklch(0.925, 0.022, 85.0)
}
/// The ink printed on it.
pub fn label_ink() -> Color {
    oklch(0.30, 0.020, 60.0)
}
/// A record.
pub fn vinyl() -> Color {
    oklch(0.175, 0.004, 70.0)
}
/// The record's edge, where it catches the light.
pub fn rim() -> Color {
    oklch(0.46, 0.008, 70.0)
}
pub fn groove() -> Color {
    oklch(0.34, 0.006, 70.0)
}

// ── Type ────────────────────────────────────────────────────────────────────

pub fn sans(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight)
}
/// Figures: sizes, speeds, times, percentages, counts.
pub fn mono(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).mono().features("'tnum' 1")
}
/// The one headline per screen.
pub fn serif(size: f32) -> TextStyle {
    TextStyle::new(size, 400.0).family(Family::Named(gate::SERIF))
}

// ── Palette 0: paper and ink ────────────────────────────────────────────────

fn own(dark: bool) -> Pal {
    let shared = shared(dark);
    if dark {
        Pal {
            dark,
            sunk: oklch(0.160, 0.006, 70.0),
            bg: oklch(0.188, 0.007, 70.0),
            card: oklch(0.212, 0.008, 70.0),
            raised: oklch(0.250, 0.009, 70.0),
            text: oklch(0.940, 0.013, 82.0),
            text2: oklch(0.770, 0.014, 76.0),
            muted: oklch(0.650, 0.014, 74.0),
            accent: oklch(0.760, 0.095, 252.0),
            line: oklcha(0.940, 0.013, 82.0, 0.10),
            line_soft: oklcha(0.940, 0.013, 82.0, 0.06),
            control: oklch(0.520, 0.014, 74.0),
            ..shared
        }
    } else {
        Pal {
            dark,
            sunk: oklch(0.940, 0.011, 82.0),
            bg: oklch(0.968, 0.009, 84.0),
            card: oklch(0.988, 0.006, 84.0),
            raised: oklch(0.925, 0.012, 82.0),
            text: oklch(0.245, 0.014, 66.0),
            text2: oklch(0.420, 0.016, 66.0),
            muted: oklch(0.515, 0.016, 66.0),
            accent: oklch(0.455, 0.115, 256.0),
            line: oklcha(0.245, 0.014, 66.0, 0.11),
            line_soft: oklcha(0.245, 0.014, 66.0, 0.06),
            control: oklch(0.600, 0.016, 70.0),
            ..shared
        }
    }
}

/// Semantic and platform colours: Aurora's, which no palette overrides.
fn shared(dark: bool) -> Pal {
    use SourcePlatform::*;
    let z = Color::TRANSPARENT;
    let roles = palette::roles(0, dark);
    let pf = |source| palette::platform(source, dark);
    Pal {
        dark,
        sunk: z,
        bg: z,
        card: z,
        raised: z,
        text: z,
        text2: z,
        muted: z,
        accent: z,
        line: z,
        line_soft: z,
        control: z,
        success: roles.success,
        warning: roles.warning,
        error: roles.error,
        platforms: [pf(Youtube), pf(Spotify), pf(Soundcloud), pf(Instagram)],
        others: [pf(Twitter), pf(Vimeo), pf(Twitch), pf(Bandcamp), pf(Reddit), pf(Dailymotion), pf(Mixcloud), pf(Rumble)],
    }
}

// ── Palettes 1 to 7: the app's own ─────────────────────────────────────────

// unused until cleanup (step 19): the Electron blocks now come from `palette::roles`.
#[allow(dead_code)]
type L = (f32, f32, f32);

/// One block of the Electron palette file: bg-secondary, bg-primary,
/// bg-tertiary, bg-hover, bg-surface; text primary, secondary, muted; accent,
/// accent-hover, on-accent (kept for the full app); border (with its alpha), border-soft alpha,
/// border-control.
#[allow(dead_code)] // unused until cleanup (step 19)
struct Block {
    bg: [L; 5],
    text: [L; 3],
    accent: [L; 3],
    border: (L, f32, f32),
    control: L,
}

#[allow(dead_code)] // unused until cleanup (step 19)
const fn b(bg: [L; 5], text: [L; 3], accent: [L; 3], border: (L, f32, f32), control: L) -> Block {
    Block { bg, text, accent, border, control }
}

/// Dark and light, in Settings' order: Aurora, Slate, Moss, Ember, Paper, Lagoon, Orchid.
#[allow(dead_code)] // unused until cleanup (step 19)
const BLOCKS: [[Block; 2]; 7] = [
    [
        b(
            [(0.122, 0.012, 240.0), (0.150, 0.014, 240.0), (0.190, 0.016, 240.0), (0.235, 0.018, 240.0), (0.205, 0.017, 240.0)],
            [(0.955, 0.008, 220.0), (0.760, 0.015, 225.0), (0.650, 0.018, 230.0)],
            [(0.800, 0.135, 195.0), (0.860, 0.125, 193.0), (0.170, 0.020, 240.0)],
            ((0.55, 0.020, 230.0), 0.28, 0.14),
            (0.54, 0.025, 230.0),
        ),
        b(
            [(0.948, 0.006, 240.0), (0.975, 0.005, 240.0), (0.930, 0.007, 240.0), (0.915, 0.008, 240.0), (0.997, 0.002, 240.0)],
            [(0.240, 0.020, 240.0), (0.430, 0.020, 238.0), (0.500, 0.018, 235.0)],
            [(0.485, 0.105, 215.0), (0.425, 0.110, 215.0), (0.985, 0.003, 220.0)],
            ((0.35, 0.015, 240.0), 0.16, 0.09),
            (0.60, 0.020, 238.0),
        ),
    ],
    [
        b(
            [(0.140, 0.013, 268.0), (0.170, 0.014, 268.0), (0.210, 0.015, 268.0), (0.255, 0.017, 268.0), (0.225, 0.015, 268.0)],
            [(0.945, 0.018, 268.0), (0.770, 0.018, 268.0), (0.665, 0.018, 268.0)],
            [(0.720, 0.140, 258.0), (0.770, 0.140, 258.0), (0.170, 0.020, 268.0)],
            ((0.55, 0.020, 268.0), 0.28, 0.14),
            (0.54, 0.025, 268.0),
        ),
        b(
            [(0.948, 0.007, 268.0), (0.975, 0.005, 268.0), (0.930, 0.007, 268.0), (0.915, 0.008, 268.0), (0.997, 0.002, 268.0)],
            [(0.240, 0.018, 268.0), (0.430, 0.018, 268.0), (0.500, 0.018, 268.0)],
            [(0.500, 0.170, 262.0), (0.440, 0.170, 262.0), (0.985, 0.003, 268.0)],
            ((0.35, 0.015, 268.0), 0.16, 0.09),
            (0.60, 0.020, 268.0),
        ),
    ],
    [
        b(
            [(0.150, 0.006, 60.0), (0.185, 0.007, 60.0), (0.225, 0.008, 60.0), (0.270, 0.008, 60.0), (0.240, 0.008, 60.0)],
            [(0.940, 0.018, 60.0), (0.760, 0.018, 60.0), (0.660, 0.018, 60.0)],
            [(0.800, 0.150, 128.0), (0.850, 0.150, 128.0), (0.170, 0.020, 60.0)],
            ((0.55, 0.020, 60.0), 0.28, 0.14),
            (0.54, 0.025, 60.0),
        ),
        b(
            [(0.948, 0.007, 60.0), (0.975, 0.005, 60.0), (0.930, 0.007, 60.0), (0.915, 0.008, 60.0), (0.997, 0.002, 60.0)],
            [(0.240, 0.018, 60.0), (0.430, 0.018, 60.0), (0.500, 0.018, 60.0)],
            [(0.420, 0.140, 118.0), (0.370, 0.135, 118.0), (0.985, 0.003, 60.0)],
            ((0.35, 0.015, 60.0), 0.16, 0.09),
            (0.60, 0.020, 60.0),
        ),
    ],
    [
        b(
            [(0.125, 0.008, 260.0), (0.155, 0.008, 260.0), (0.190, 0.008, 260.0), (0.235, 0.008, 260.0), (0.205, 0.008, 260.0)],
            [(0.955, 0.012, 260.0), (0.732, 0.016, 260.0), (0.622, 0.016, 260.0)],
            [(0.800, 0.120, 36.0), (0.850, 0.085, 36.0), (0.170, 0.020, 260.0)],
            ((0.55, 0.020, 260.0), 0.28, 0.14),
            (0.54, 0.025, 260.0),
        ),
        b(
            [(0.948, 0.006, 95.0), (0.975, 0.006, 95.0), (0.930, 0.006, 95.0), (0.915, 0.006, 95.0), (0.997, 0.003, 95.0)],
            [(0.240, 0.009, 95.0), (0.404, 0.012, 95.0), (0.502, 0.012, 95.0)],
            [(0.440, 0.140, 38.0), (0.390, 0.130, 38.0), (0.985, 0.003, 95.0)],
            ((0.35, 0.020, 95.0), 0.16, 0.09),
            (0.60, 0.020, 95.0),
        ),
    ],
    [
        b(
            [(0.125, 0.006, 70.0), (0.155, 0.006, 70.0), (0.190, 0.006, 70.0), (0.235, 0.006, 70.0), (0.205, 0.006, 70.0)],
            [(0.955, 0.009, 70.0), (0.732, 0.012, 70.0), (0.622, 0.012, 70.0)],
            [(0.720, 0.100, 65.0), (0.780, 0.090, 65.0), (0.170, 0.020, 70.0)],
            ((0.55, 0.020, 70.0), 0.28, 0.14),
            (0.54, 0.025, 70.0),
        ),
        b(
            [(0.948, 0.008, 75.0), (0.975, 0.008, 75.0), (0.930, 0.008, 75.0), (0.915, 0.008, 75.0), (0.997, 0.002, 75.0)],
            [(0.240, 0.012, 75.0), (0.404, 0.016, 75.0), (0.502, 0.016, 75.0)],
            [(0.370, 0.080, 65.0), (0.320, 0.075, 65.0), (0.985, 0.003, 75.0)],
            ((0.35, 0.020, 75.0), 0.16, 0.09),
            (0.60, 0.020, 75.0),
        ),
    ],
    [
        b(
            [(0.125, 0.020, 195.0), (0.155, 0.020, 195.0), (0.190, 0.020, 195.0), (0.235, 0.020, 195.0), (0.205, 0.020, 195.0)],
            [(0.955, 0.030, 195.0), (0.730, 0.040, 195.0), (0.620, 0.040, 195.0)],
            [(0.880, 0.100, 170.0), (0.920, 0.090, 170.0), (0.170, 0.020, 195.0)],
            ((0.55, 0.020, 195.0), 0.28, 0.14),
            (0.54, 0.025, 195.0),
        ),
        b(
            [(0.948, 0.010, 190.0), (0.975, 0.010, 190.0), (0.930, 0.010, 190.0), (0.915, 0.010, 190.0), (0.997, 0.004, 190.0)],
            [(0.240, 0.015, 190.0), (0.402, 0.020, 190.0), (0.500, 0.020, 190.0)],
            [(0.400, 0.090, 175.0), (0.350, 0.085, 175.0), (0.985, 0.003, 190.0)],
            ((0.35, 0.020, 190.0), 0.16, 0.09),
            (0.60, 0.020, 190.0),
        ),
    ],
    [
        b(
            [(0.125, 0.020, 320.0), (0.155, 0.020, 320.0), (0.190, 0.020, 320.0), (0.235, 0.020, 320.0), (0.205, 0.020, 320.0)],
            [(0.955, 0.030, 320.0), (0.734, 0.040, 320.0), (0.626, 0.040, 320.0)],
            [(0.800, 0.170, 335.0), (0.850, 0.119, 335.0), (0.170, 0.020, 320.0)],
            ((0.55, 0.020, 320.0), 0.28, 0.14),
            (0.54, 0.025, 320.0),
        ),
        b(
            [(0.948, 0.008, 320.0), (0.975, 0.008, 320.0), (0.930, 0.008, 320.0), (0.915, 0.008, 320.0), (0.997, 0.002, 320.0)],
            [(0.240, 0.012, 320.0), (0.406, 0.016, 320.0), (0.504, 0.016, 320.0)],
            [(0.520, 0.150, 335.0), (0.470, 0.142, 335.0), (0.985, 0.003, 320.0)],
            ((0.35, 0.020, 320.0), 0.16, 0.09),
            (0.60, 0.020, 320.0),
        ),
    ],
];

#[allow(dead_code)] // unused until cleanup (step 19)
fn c(l: L) -> Color {
    oklch(l.0, l.1, l.2)
}

fn existing(index: usize, dark: bool) -> Pal {
    let r = palette::roles(index - 1, dark);
    Pal {
        dark,
        sunk: r.bg_secondary,
        bg: r.bg_primary,
        // Dark cards sit a step above the page (tertiary); light ones are the white surface.
        card: if dark { r.bg_tertiary } else { r.bg_surface },
        raised: if dark { r.bg_hover } else { r.bg_tertiary },
        text: r.text_primary,
        text2: r.text_secondary,
        muted: r.text_muted,
        accent: r.accent,
        line: r.border,
        line_soft: r.border_soft,
        control: r.border_control,
        ..shared(dark)
    }
}
