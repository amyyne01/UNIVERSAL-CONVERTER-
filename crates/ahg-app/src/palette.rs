//! The one colour source: Electron's seven palettes (`src/index.css` for Aurora,
//! `src/palettes.css` for the rest), its semantic and platform colours, once, in OKLCH.
//! Numbers are copied from the CSS, never adjusted; the tests compare them to it.
//! A look's own palette 0 (a Rust-only mood) is not here: each look keeps its own.
use ahg_engine::config::PALETTE_IDS;
use ahg_engine::types::SourcePlatform;
use rune::{Color, oklcha};

/// A colour as the CSS writes it: lightness, chroma, hue, alpha.
type O = [f32; 4];

const fn o(l: f32, c: f32, h: f32) -> O {
    [l, c, h, 1.0]
}
const fn oa(l: f32, c: f32, h: f32, a: f32) -> O {
    [l, c, h, a]
}

/// Every role, named as the CSS custom property (`--color-bg-primary` is `bg_primary`).
macro_rules! roles {
    ($($f:ident),* $(,)?) => {
        pub struct Roles<T = Color> { $(pub $f: T),* }
        impl<T: Copy> Roles<T> {
            fn map<U>(&self, f: impl Fn(T) -> U) -> Roles<U> { Roles { $($f: f(self.$f)),* } }
            /// `(role name, value)` for every role.
            #[cfg(test)]
            fn list(&self) -> Vec<(&'static str, T)> { vec![$((stringify!($f), self.$f)),*] }
        }
    };
}
roles!(
    bg_secondary,
    bg_primary,
    bg_tertiary,
    bg_hover,
    bg_surface,
    bg_glass,
    scrim,
    text_primary,
    text_secondary,
    text_muted,
    accent,
    accent_hover,
    accent_deep,
    on_accent,
    border,
    border_soft,
    border_control,
    inner_light,
    media_scrim,
    on_media,
    success,
    warning,
    error,
);

pub const NAMES: [&str; 7] = ["Aurora", "Slate", "Moss", "Ember", "Paper", "Lagoon", "Orchid"];

const Z: O = [0.0; 4];

/// The roles no palette overrides, per theme (index.css: `--color-success` ... `--color-on-media`);
/// the palette fields are filled by `tone`.
const fn shared(dark: bool) -> Roles<O> {
    let (inner_light, success, warning, error) = if dark {
        (oa(0.98, 0.005, 220.0, 0.06), o(0.740, 0.160, 150.0), o(0.830, 0.145, 85.0), o(0.680, 0.195, 24.0))
    } else {
        (oa(1.0, 0.0, 0.0, 0.0), o(0.480, 0.140, 150.0), o(0.500, 0.125, 80.0), o(0.520, 0.190, 24.0))
    };
    Roles {
        bg_secondary: Z,
        bg_primary: Z,
        bg_tertiary: Z,
        bg_hover: Z,
        bg_surface: Z,
        bg_glass: Z,
        scrim: Z,
        text_primary: Z,
        text_secondary: Z,
        text_muted: Z,
        accent: Z,
        accent_hover: Z,
        accent_deep: Z,
        on_accent: Z,
        border: Z,
        border_soft: Z,
        border_control: Z,
        inner_light,
        // Ink and scrim on artwork: the same in both themes.
        media_scrim: oa(0.12, 0.012, 240.0, 0.88),
        on_media: o(0.955, 0.008, 220.0),
        success,
        warning,
        error,
    }
}

/// One palette in one theme: backgrounds (secondary, primary, tertiary, hover, surface, glass,
/// scrim), text (primary, secondary, muted), accent (base, hover, deep, on-accent), line
/// (border, soft, control).
const fn tone(dark: bool, bg: [O; 7], text: [O; 3], acc: [O; 4], line: [O; 3]) -> Roles<O> {
    Roles {
        bg_secondary: bg[0],
        bg_primary: bg[1],
        bg_tertiary: bg[2],
        bg_hover: bg[3],
        bg_surface: bg[4],
        bg_glass: bg[5],
        scrim: bg[6],
        text_primary: text[0],
        text_secondary: text[1],
        text_muted: text[2],
        accent: acc[0],
        accent_hover: acc[1],
        accent_deep: acc[2],
        on_accent: acc[3],
        border: line[0],
        border_soft: line[1],
        border_control: line[2],
        ..shared(dark)
    }
}

/// `[dark, light]` per palette, in `PALETTE_IDS` order.
const TABLE: [[Roles<O>; 2]; 7] = [
    // aurora
    [
        // index.css:11 (@theme, the dark default)
        tone(
            true,
            [
                o(0.122, 0.012, 240.0),
                o(0.150, 0.014, 240.0),
                o(0.190, 0.016, 240.0),
                o(0.235, 0.018, 240.0),
                o(0.205, 0.017, 240.0),
                oa(0.215, 0.018, 240.0, 0.58),
                oa(0.09, 0.010, 240.0, 0.80),
            ],
            [o(0.955, 0.008, 220.0), o(0.760, 0.015, 225.0), o(0.650, 0.018, 230.0)],
            [o(0.800, 0.135, 195.0), o(0.860, 0.125, 193.0), o(0.320, 0.060, 210.0), o(0.170, 0.020, 240.0)],
            [oa(0.55, 0.020, 230.0, 0.28), oa(0.55, 0.020, 230.0, 0.14), o(0.54, 0.025, 230.0)],
        ),
        // index.css:94
        tone(
            false,
            [
                o(0.948, 0.006, 240.0),
                o(0.975, 0.005, 240.0),
                o(0.930, 0.007, 240.0),
                o(0.915, 0.008, 240.0),
                o(0.997, 0.002, 240.0),
                oa(0.99, 0.002, 240.0, 0.65),
                oa(0.25, 0.015, 240.0, 0.45),
            ],
            [o(0.240, 0.020, 240.0), o(0.430, 0.020, 238.0), o(0.500, 0.018, 235.0)],
            [o(0.485, 0.105, 215.0), o(0.425, 0.110, 215.0), o(0.880, 0.045, 205.0), o(0.985, 0.003, 220.0)],
            [oa(0.35, 0.015, 240.0, 0.16), oa(0.35, 0.015, 240.0, 0.09), o(0.60, 0.020, 238.0)],
        ),
    ],
    // slate
    [
        // palettes.css:7
        tone(
            true,
            [
                o(0.140, 0.013, 268.0),
                o(0.170, 0.014, 268.0),
                o(0.210, 0.015, 268.0),
                o(0.255, 0.017, 268.0),
                o(0.225, 0.015, 268.0),
                oa(0.225, 0.015, 268.0, 0.58),
                oa(0.100, 0.013, 268.0, 0.8),
            ],
            [o(0.945, 0.018, 268.0), o(0.770, 0.018, 268.0), o(0.665, 0.018, 268.0)],
            [o(0.720, 0.140, 258.0), o(0.770, 0.140, 258.0), o(0.320, 0.060, 258.0), o(0.170, 0.020, 268.0)],
            [oa(0.550, 0.020, 268.0, 0.28), oa(0.550, 0.020, 268.0, 0.14), o(0.540, 0.025, 268.0)],
        ),
        // palettes.css:27
        tone(
            false,
            [
                o(0.948, 0.007, 268.0),
                o(0.975, 0.005, 268.0),
                o(0.930, 0.007, 268.0),
                o(0.915, 0.008, 268.0),
                o(0.997, 0.002, 268.0),
                oa(0.990, 0.002, 268.0, 0.65),
                oa(0.250, 0.015, 268.0, 0.45),
            ],
            [o(0.240, 0.018, 268.0), o(0.430, 0.018, 268.0), o(0.500, 0.018, 268.0)],
            [o(0.500, 0.170, 262.0), o(0.440, 0.170, 262.0), o(0.880, 0.045, 262.0), o(0.985, 0.003, 268.0)],
            [oa(0.350, 0.015, 268.0, 0.16), oa(0.350, 0.015, 268.0, 0.09), o(0.600, 0.020, 268.0)],
        ),
    ],
    // moss
    [
        // palettes.css:47
        tone(
            true,
            [
                o(0.150, 0.006, 60.0),
                o(0.185, 0.007, 60.0),
                o(0.225, 0.008, 60.0),
                o(0.270, 0.008, 60.0),
                o(0.240, 0.008, 60.0),
                oa(0.240, 0.008, 60.0, 0.58),
                oa(0.110, 0.006, 60.0, 0.8),
            ],
            [o(0.940, 0.018, 60.0), o(0.760, 0.018, 60.0), o(0.660, 0.018, 60.0)],
            [o(0.800, 0.150, 128.0), o(0.850, 0.150, 128.0), o(0.320, 0.060, 128.0), o(0.170, 0.020, 60.0)],
            [oa(0.550, 0.020, 60.0, 0.28), oa(0.550, 0.020, 60.0, 0.14), o(0.540, 0.025, 60.0)],
        ),
        // palettes.css:67
        tone(
            false,
            [
                o(0.948, 0.007, 60.0),
                o(0.975, 0.005, 60.0),
                o(0.930, 0.007, 60.0),
                o(0.915, 0.008, 60.0),
                o(0.997, 0.002, 60.0),
                oa(0.990, 0.002, 60.0, 0.65),
                oa(0.250, 0.015, 60.0, 0.45),
            ],
            [o(0.240, 0.018, 60.0), o(0.430, 0.018, 60.0), o(0.500, 0.018, 60.0)],
            [o(0.420, 0.140, 118.0), o(0.370, 0.135, 118.0), o(0.880, 0.045, 118.0), o(0.985, 0.003, 60.0)],
            [oa(0.350, 0.015, 60.0, 0.16), oa(0.350, 0.015, 60.0, 0.09), o(0.600, 0.020, 60.0)],
        ),
    ],
    // ember
    [
        // palettes.css:87
        tone(
            true,
            [
                o(0.125, 0.008, 260.0),
                o(0.155, 0.008, 260.0),
                o(0.190, 0.008, 260.0),
                o(0.235, 0.008, 260.0),
                o(0.205, 0.008, 260.0),
                oa(0.215, 0.008, 260.0, 0.58),
                oa(0.090, 0.010, 260.0, 0.8),
            ],
            [o(0.955, 0.012, 260.0), o(0.732, 0.016, 260.0), o(0.622, 0.016, 260.0)],
            [o(0.800, 0.120, 36.0), o(0.850, 0.085, 36.0), o(0.320, 0.050, 36.0), o(0.170, 0.020, 260.0)],
            [oa(0.550, 0.020, 260.0, 0.28), oa(0.550, 0.020, 260.0, 0.14), o(0.540, 0.025, 260.0)],
        ),
        // palettes.css:107
        tone(
            false,
            [
                o(0.948, 0.006, 95.0),
                o(0.975, 0.006, 95.0),
                o(0.930, 0.006, 95.0),
                o(0.915, 0.006, 95.0),
                o(0.997, 0.003, 95.0),
                oa(0.990, 0.003, 95.0, 0.65),
                oa(0.250, 0.015, 95.0, 0.45),
            ],
            [o(0.240, 0.009, 95.0), o(0.404, 0.012, 95.0), o(0.502, 0.012, 95.0)],
            [o(0.440, 0.140, 38.0), o(0.390, 0.130, 38.0), o(0.880, 0.051, 38.0), o(0.985, 0.003, 95.0)],
            [oa(0.350, 0.020, 95.0, 0.16), oa(0.350, 0.020, 95.0, 0.09), o(0.600, 0.020, 95.0)],
        ),
    ],
    // paper
    [
        // palettes.css:127
        tone(
            true,
            [
                o(0.125, 0.006, 70.0),
                o(0.155, 0.006, 70.0),
                o(0.190, 0.006, 70.0),
                o(0.235, 0.006, 70.0),
                o(0.205, 0.006, 70.0),
                oa(0.215, 0.006, 70.0, 0.58),
                oa(0.090, 0.010, 70.0, 0.8),
            ],
            [o(0.955, 0.009, 70.0), o(0.732, 0.012, 70.0), o(0.622, 0.012, 70.0)],
            [o(0.720, 0.100, 65.0), o(0.780, 0.090, 65.0), o(0.320, 0.058, 65.0), o(0.170, 0.020, 70.0)],
            [oa(0.550, 0.020, 70.0, 0.28), oa(0.550, 0.020, 70.0, 0.14), o(0.540, 0.025, 70.0)],
        ),
        // palettes.css:147
        tone(
            false,
            [
                o(0.948, 0.008, 75.0),
                o(0.975, 0.008, 75.0),
                o(0.930, 0.008, 75.0),
                o(0.915, 0.008, 75.0),
                o(0.997, 0.002, 75.0),
                oa(0.990, 0.002, 75.0, 0.65),
                oa(0.250, 0.015, 75.0, 0.45),
            ],
            [o(0.240, 0.012, 75.0), o(0.404, 0.016, 75.0), o(0.502, 0.016, 75.0)],
            [o(0.370, 0.080, 65.0), o(0.320, 0.075, 65.0), o(0.880, 0.041, 65.0), o(0.985, 0.003, 75.0)],
            [oa(0.350, 0.020, 75.0, 0.16), oa(0.350, 0.020, 75.0, 0.09), o(0.600, 0.020, 75.0)],
        ),
    ],
    // lagoon
    [
        // palettes.css:167
        tone(
            true,
            [
                o(0.125, 0.020, 195.0),
                o(0.155, 0.020, 195.0),
                o(0.190, 0.020, 195.0),
                o(0.235, 0.020, 195.0),
                o(0.205, 0.020, 195.0),
                oa(0.215, 0.020, 195.0, 0.58),
                oa(0.090, 0.010, 195.0, 0.8),
            ],
            [o(0.955, 0.030, 195.0), o(0.730, 0.040, 195.0), o(0.620, 0.040, 195.0)],
            [o(0.880, 0.100, 170.0), o(0.920, 0.090, 170.0), o(0.320, 0.059, 170.0), o(0.170, 0.020, 195.0)],
            [oa(0.550, 0.020, 195.0, 0.28), oa(0.550, 0.020, 195.0, 0.14), o(0.540, 0.025, 195.0)],
        ),
        // palettes.css:187
        tone(
            false,
            [
                o(0.948, 0.010, 190.0),
                o(0.975, 0.010, 190.0),
                o(0.930, 0.010, 190.0),
                o(0.915, 0.010, 190.0),
                o(0.997, 0.004, 190.0),
                oa(0.990, 0.004, 190.0, 0.65),
                oa(0.250, 0.015, 190.0, 0.45),
            ],
            [o(0.240, 0.015, 190.0), o(0.402, 0.020, 190.0), o(0.500, 0.020, 190.0)],
            [o(0.400, 0.090, 175.0), o(0.350, 0.085, 175.0), o(0.880, 0.027, 175.0), o(0.985, 0.003, 190.0)],
            [oa(0.350, 0.020, 190.0, 0.16), oa(0.350, 0.020, 190.0, 0.09), o(0.600, 0.020, 190.0)],
        ),
    ],
    // orchid
    [
        // palettes.css:207
        tone(
            true,
            [
                o(0.125, 0.020, 320.0),
                o(0.155, 0.020, 320.0),
                o(0.190, 0.020, 320.0),
                o(0.235, 0.020, 320.0),
                o(0.205, 0.020, 320.0),
                oa(0.215, 0.020, 320.0, 0.58),
                oa(0.090, 0.010, 320.0, 0.8),
            ],
            [o(0.955, 0.030, 320.0), o(0.734, 0.040, 320.0), o(0.626, 0.040, 320.0)],
            [o(0.800, 0.170, 335.0), o(0.850, 0.119, 335.0), o(0.320, 0.077, 335.0), o(0.170, 0.020, 320.0)],
            [oa(0.550, 0.020, 320.0, 0.28), oa(0.550, 0.020, 320.0, 0.14), o(0.540, 0.025, 320.0)],
        ),
        // palettes.css:227
        tone(
            false,
            [
                o(0.948, 0.008, 320.0),
                o(0.975, 0.008, 320.0),
                o(0.930, 0.008, 320.0),
                o(0.915, 0.008, 320.0),
                o(0.997, 0.002, 320.0),
                oa(0.990, 0.002, 320.0, 0.65),
                oa(0.250, 0.015, 320.0, 0.45),
            ],
            [o(0.240, 0.012, 320.0), o(0.406, 0.016, 320.0), o(0.504, 0.016, 320.0)],
            [o(0.520, 0.150, 335.0), o(0.470, 0.142, 335.0), o(0.880, 0.045, 335.0), o(0.985, 0.003, 320.0)],
            [oa(0.350, 0.020, 320.0, 0.16), oa(0.350, 0.020, 320.0, 0.09), o(0.600, 0.020, 320.0)],
        ),
    ],
];

/// The roles of palette `palette` (an index into `PALETTE_IDS`; past the end clamps to the last).
pub fn roles(palette: usize, dark: bool) -> Roles {
    TABLE[palette.min(PALETTE_IDS.len() - 1)][usize::from(!dark)].map(|v| oklcha(v[0], v[1], v[2], v[3]))
}

/// A platform or source colour (index.css: `--color-youtube` ... `--color-src-rumble`), the
/// same in every palette. A link with no colour of its own (direct, generic, unknown) takes
/// Aurora's accent; ask `roles()` for the active palette's.
pub fn platform(source: SourcePlatform, dark: bool) -> Color {
    use SourcePlatform::*;
    let (l, c, h) = match (source, dark) {
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
        (Direct | Generic | Unknown, _) => return roles(0, dark).accent,
    };
    oklcha(l, c, h, 1.0)
}

/// `color-mix(in oklch, c pct%, transparent)`, which Electron uses for every soft wash
/// (accent 14% / 10%, status colours 10%): the colour with its alpha scaled by `pct`.
pub fn mix(c: Color, pct: f32) -> Color {
    c.alpha(c.a * pct)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Verbatim lines of Electron's CSS (`index.css` 11-57 and 94-138, `palettes.css` 7-245): the
    /// expected values. A `/* file:N */` marker gives the real line where each part starts.
    const CSS: &str = r####"/* index.css:11 */
@theme {
  --color-bg-primary: oklch(0.150 0.014 240);
  --color-bg-secondary: oklch(0.122 0.012 240);
  --color-bg-tertiary: oklch(0.190 0.016 240);
  --color-bg-hover: oklch(0.235 0.018 240);
  --color-bg-surface: oklch(0.205 0.017 240);
  --color-bg-glass: oklch(0.215 0.018 240 / 0.58);
  --color-scrim: oklch(0.09 0.010 240 / 0.80);

  --color-text-primary: oklch(0.955 0.008 220);
  --color-text-secondary: oklch(0.760 0.015 225);
  --color-text-muted: oklch(0.650 0.018 230);

  --color-accent: oklch(0.800 0.135 195);
  --color-accent-hover: oklch(0.860 0.125 193);
  --color-accent-soft: color-mix(in oklch, var(--color-accent) 14%, transparent);
  --color-accent-deep: oklch(0.320 0.060 210);
  --color-on-accent: oklch(0.170 0.020 240);

  --color-youtube: oklch(0.690 0.220 27);
  --color-spotify: oklch(0.730 0.180 150);
  --color-soundcloud: oklch(0.725 0.190 55);
  --color-reels: oklch(0.700 0.235 352);
  /* Monogram tiles for the sources with no tab of their own: one hue each, kept clear of status and platform hues (check-contrast.mjs guards it). */
  --color-src-twitter: oklch(0.760 0.020 240);
  --color-src-vimeo: oklch(0.760 0.130 230);
  --color-src-twitch: oklch(0.760 0.130 275);
  --color-src-bandcamp: oklch(0.760 0.130 20);
  --color-src-reddit: oklch(0.760 0.130 115);
  --color-src-dailymotion: oklch(0.760 0.130 330);
  --color-src-mixcloud: oklch(0.760 0.130 70);
  --color-src-rumble: oklch(0.760 0.130 170);

  --color-success: oklch(0.740 0.160 150);
  --color-warning: oklch(0.830 0.145 85);
  --color-error: oklch(0.680 0.195 24);

  --color-border: oklch(0.55 0.020 230 / 0.28);
  --color-border-soft: oklch(0.55 0.020 230 / 0.14);
  --color-border-control: oklch(0.54 0.025 230);
  --color-inner-light: oklch(0.98 0.005 220 / 0.06);

  /* Ink and scrim for content sitting ON artwork (thumbnails, cover art). These
     do NOT flip with the theme: a cover is a photograph in both, so light-theme
     ink over it would be unreadable. Any overlay on media uses this pair. */
  --color-media-scrim: oklch(0.12 0.012 240 / 0.88);
  --color-on-media: oklch(0.955 0.008 220);
}
/* index.css:94 */
:root[data-theme="light"] {
  --color-bg-primary: oklch(0.975 0.005 240);
  --color-bg-secondary: oklch(0.948 0.006 240);
  --color-bg-tertiary: oklch(0.930 0.007 240);
  --color-bg-hover: oklch(0.915 0.008 240);
  --color-bg-surface: oklch(0.997 0.002 240);
  --color-bg-glass: oklch(0.99 0.002 240 / 0.65);
  --color-scrim: oklch(0.25 0.015 240 / 0.45);

  --color-text-primary: oklch(0.240 0.020 240);
  --color-text-secondary: oklch(0.430 0.020 238);
  --color-text-muted: oklch(0.500 0.018 235);

  --color-accent: oklch(0.485 0.105 215);
  --color-accent-hover: oklch(0.425 0.110 215);
  --color-accent-soft: color-mix(in oklch, var(--color-accent) 10%, transparent);
  --color-accent-deep: oklch(0.880 0.045 205);
  --color-on-accent: oklch(0.985 0.003 220);

  --color-youtube: oklch(0.525 0.210 27);
  --color-spotify: oklch(0.480 0.150 150);
  --color-soundcloud: oklch(0.510 0.140 55);
  --color-reels: oklch(0.525 0.220 352);
  --color-src-twitter: oklch(0.470 0.020 240);
  --color-src-vimeo: oklch(0.470 0.140 250);
  --color-src-twitch: oklch(0.470 0.120 275);
  --color-src-bandcamp: oklch(0.470 0.120 20);
  --color-src-reddit: oklch(0.470 0.120 115);
  --color-src-dailymotion: oklch(0.470 0.120 330);
  --color-src-mixcloud: oklch(0.400 0.090 70);
  --color-src-rumble: oklch(0.470 0.120 170);

  --color-success: oklch(0.480 0.140 150);
  --color-warning: oklch(0.500 0.125 80);
  --color-error: oklch(0.520 0.190 24);

  --color-border: oklch(0.35 0.015 240 / 0.16);
  --color-border-soft: oklch(0.35 0.015 240 / 0.09);
  --color-border-control: oklch(0.60 0.020 238);
  --color-inner-light: oklch(1 0 0 / 0);

  --shadow-sm: 0 1px 2px oklch(0.25 0.02 240 / 0.06);
  --shadow-md: 0 2px 10px oklch(0.25 0.02 240 / 0.08);
  --shadow-lg: 0 12px 40px oklch(0.25 0.02 240 / 0.13);
}
/* palettes.css:7 */
:root[data-palette="slate"][data-theme="dark"] {
  --color-bg-secondary: oklch(0.140 0.013 268);
  --color-bg-primary: oklch(0.170 0.014 268);
  --color-bg-tertiary: oklch(0.210 0.015 268);
  --color-bg-hover: oklch(0.255 0.017 268);
  --color-bg-surface: oklch(0.225 0.015 268);
  --color-bg-glass: oklch(0.225 0.015 268 / 0.58);
  --color-scrim: oklch(0.100 0.013 268 / 0.8);
  --color-text-primary: oklch(0.945 0.018 268);
  --color-text-secondary: oklch(0.770 0.018 268);
  --color-text-muted: oklch(0.665 0.018 268);
  --color-accent: oklch(0.720 0.140 258);
  --color-accent-hover: oklch(0.770 0.140 258);
  --color-accent-deep: oklch(0.320 0.060 258);
  --color-on-accent: oklch(0.170 0.020 268);
  --color-border: oklch(0.550 0.020 268 / 0.28);
  --color-border-soft: oklch(0.550 0.020 268 / 0.14);
  --color-border-control: oklch(0.540 0.025 268);
}

:root[data-palette="slate"][data-theme="light"] {
  --color-bg-secondary: oklch(0.948 0.007 268);
  --color-bg-primary: oklch(0.975 0.005 268);
  --color-bg-tertiary: oklch(0.930 0.007 268);
  --color-bg-hover: oklch(0.915 0.008 268);
  --color-bg-surface: oklch(0.997 0.002 268);
  --color-bg-glass: oklch(0.990 0.002 268 / 0.65);
  --color-scrim: oklch(0.250 0.015 268 / 0.45);
  --color-text-primary: oklch(0.240 0.018 268);
  --color-text-secondary: oklch(0.430 0.018 268);
  --color-text-muted: oklch(0.500 0.018 268);
  --color-accent: oklch(0.500 0.170 262);
  --color-accent-hover: oklch(0.440 0.170 262);
  --color-accent-deep: oklch(0.880 0.045 262);
  --color-on-accent: oklch(0.985 0.003 268);
  --color-border: oklch(0.350 0.015 268 / 0.16);
  --color-border-soft: oklch(0.350 0.015 268 / 0.09);
  --color-border-control: oklch(0.600 0.020 268);
}

:root[data-palette="moss"][data-theme="dark"] {
  --color-bg-secondary: oklch(0.150 0.006 60);
  --color-bg-primary: oklch(0.185 0.007 60);
  --color-bg-tertiary: oklch(0.225 0.008 60);
  --color-bg-hover: oklch(0.270 0.008 60);
  --color-bg-surface: oklch(0.240 0.008 60);
  --color-bg-glass: oklch(0.240 0.008 60 / 0.58);
  --color-scrim: oklch(0.110 0.006 60 / 0.8);
  --color-text-primary: oklch(0.940 0.018 60);
  --color-text-secondary: oklch(0.760 0.018 60);
  --color-text-muted: oklch(0.660 0.018 60);
  --color-accent: oklch(0.800 0.150 128);
  --color-accent-hover: oklch(0.850 0.150 128);
  --color-accent-deep: oklch(0.320 0.060 128);
  --color-on-accent: oklch(0.170 0.020 60);
  --color-border: oklch(0.550 0.020 60 / 0.28);
  --color-border-soft: oklch(0.550 0.020 60 / 0.14);
  --color-border-control: oklch(0.540 0.025 60);
}

:root[data-palette="moss"][data-theme="light"] {
  --color-bg-secondary: oklch(0.948 0.007 60);
  --color-bg-primary: oklch(0.975 0.005 60);
  --color-bg-tertiary: oklch(0.930 0.007 60);
  --color-bg-hover: oklch(0.915 0.008 60);
  --color-bg-surface: oklch(0.997 0.002 60);
  --color-bg-glass: oklch(0.990 0.002 60 / 0.65);
  --color-scrim: oklch(0.250 0.015 60 / 0.45);
  --color-text-primary: oklch(0.240 0.018 60);
  --color-text-secondary: oklch(0.430 0.018 60);
  --color-text-muted: oklch(0.500 0.018 60);
  --color-accent: oklch(0.420 0.140 118);
  --color-accent-hover: oklch(0.370 0.135 118);
  --color-accent-deep: oklch(0.880 0.045 118);
  --color-on-accent: oklch(0.985 0.003 60);
  --color-border: oklch(0.350 0.015 60 / 0.16);
  --color-border-soft: oklch(0.350 0.015 60 / 0.09);
  --color-border-control: oklch(0.600 0.020 60);
}

:root[data-palette="ember"][data-theme="dark"] {
  --color-bg-secondary: oklch(0.125 0.008 260);
  --color-bg-primary: oklch(0.155 0.008 260);
  --color-bg-tertiary: oklch(0.190 0.008 260);
  --color-bg-surface: oklch(0.205 0.008 260);
  --color-bg-hover: oklch(0.235 0.008 260);
  --color-bg-glass: oklch(0.215 0.008 260 / 0.58);
  --color-scrim: oklch(0.090 0.010 260 / 0.8);
  --color-text-primary: oklch(0.955 0.012 260);
  --color-text-secondary: oklch(0.732 0.016 260);
  --color-text-muted: oklch(0.622 0.016 260);
  --color-accent: oklch(0.800 0.120 36);
  --color-accent-hover: oklch(0.850 0.085 36);
  --color-accent-deep: oklch(0.320 0.050 36);
  --color-on-accent: oklch(0.170 0.020 260);
  --color-border: oklch(0.550 0.020 260 / 0.28);
  --color-border-soft: oklch(0.550 0.020 260 / 0.14);
  --color-border-control: oklch(0.540 0.025 260);
}

:root[data-palette="ember"][data-theme="light"] {
  --color-bg-secondary: oklch(0.948 0.006 95);
  --color-bg-primary: oklch(0.975 0.006 95);
  --color-bg-tertiary: oklch(0.930 0.006 95);
  --color-bg-surface: oklch(0.997 0.003 95);
  --color-bg-hover: oklch(0.915 0.006 95);
  --color-bg-glass: oklch(0.990 0.003 95 / 0.65);
  --color-scrim: oklch(0.250 0.015 95 / 0.45);
  --color-text-primary: oklch(0.240 0.009 95);
  --color-text-secondary: oklch(0.404 0.012 95);
  --color-text-muted: oklch(0.502 0.012 95);
  --color-accent: oklch(0.440 0.140 38);
  --color-accent-hover: oklch(0.390 0.130 38);
  --color-accent-deep: oklch(0.880 0.051 38);
  --color-on-accent: oklch(0.985 0.003 95);
  --color-border: oklch(0.350 0.020 95 / 0.16);
  --color-border-soft: oklch(0.350 0.020 95 / 0.09);
  --color-border-control: oklch(0.600 0.020 95);
}

:root[data-palette="paper"][data-theme="dark"] {
  --color-bg-secondary: oklch(0.125 0.006 70);
  --color-bg-primary: oklch(0.155 0.006 70);
  --color-bg-tertiary: oklch(0.190 0.006 70);
  --color-bg-surface: oklch(0.205 0.006 70);
  --color-bg-hover: oklch(0.235 0.006 70);
  --color-bg-glass: oklch(0.215 0.006 70 / 0.58);
  --color-scrim: oklch(0.090 0.010 70 / 0.8);
  --color-text-primary: oklch(0.955 0.009 70);
  --color-text-secondary: oklch(0.732 0.012 70);
  --color-text-muted: oklch(0.622 0.012 70);
  --color-accent: oklch(0.720 0.100 65);
  --color-accent-hover: oklch(0.780 0.090 65);
  --color-accent-deep: oklch(0.320 0.058 65);
  --color-on-accent: oklch(0.170 0.020 70);
  --color-border: oklch(0.550 0.020 70 / 0.28);
  --color-border-soft: oklch(0.550 0.020 70 / 0.14);
  --color-border-control: oklch(0.540 0.025 70);
}

:root[data-palette="paper"][data-theme="light"] {
  --color-bg-secondary: oklch(0.948 0.008 75);
  --color-bg-primary: oklch(0.975 0.008 75);
  --color-bg-tertiary: oklch(0.930 0.008 75);
  --color-bg-surface: oklch(0.997 0.002 75);
  --color-bg-hover: oklch(0.915 0.008 75);
  --color-bg-glass: oklch(0.990 0.002 75 / 0.65);
  --color-scrim: oklch(0.250 0.015 75 / 0.45);
  --color-text-primary: oklch(0.240 0.012 75);
  --color-text-secondary: oklch(0.404 0.016 75);
  --color-text-muted: oklch(0.502 0.016 75);
  --color-accent: oklch(0.370 0.080 65);
  --color-accent-hover: oklch(0.320 0.075 65);
  --color-accent-deep: oklch(0.880 0.041 65);
  --color-on-accent: oklch(0.985 0.003 75);
  --color-border: oklch(0.350 0.020 75 / 0.16);
  --color-border-soft: oklch(0.350 0.020 75 / 0.09);
  --color-border-control: oklch(0.600 0.020 75);
}

:root[data-palette="lagoon"][data-theme="dark"] {
  --color-bg-secondary: oklch(0.125 0.020 195);
  --color-bg-primary: oklch(0.155 0.020 195);
  --color-bg-tertiary: oklch(0.190 0.020 195);
  --color-bg-surface: oklch(0.205 0.020 195);
  --color-bg-hover: oklch(0.235 0.020 195);
  --color-bg-glass: oklch(0.215 0.020 195 / 0.58);
  --color-scrim: oklch(0.090 0.010 195 / 0.8);
  --color-text-primary: oklch(0.955 0.030 195);
  --color-text-secondary: oklch(0.730 0.040 195);
  --color-text-muted: oklch(0.620 0.040 195);
  --color-accent: oklch(0.880 0.100 170);
  --color-accent-hover: oklch(0.920 0.090 170);
  --color-accent-deep: oklch(0.320 0.059 170);
  --color-on-accent: oklch(0.170 0.020 195);
  --color-border: oklch(0.550 0.020 195 / 0.28);
  --color-border-soft: oklch(0.550 0.020 195 / 0.14);
  --color-border-control: oklch(0.540 0.025 195);
}

:root[data-palette="lagoon"][data-theme="light"] {
  --color-bg-secondary: oklch(0.948 0.010 190);
  --color-bg-primary: oklch(0.975 0.010 190);
  --color-bg-tertiary: oklch(0.930 0.010 190);
  --color-bg-surface: oklch(0.997 0.004 190);
  --color-bg-hover: oklch(0.915 0.010 190);
  --color-bg-glass: oklch(0.990 0.004 190 / 0.65);
  --color-scrim: oklch(0.250 0.015 190 / 0.45);
  --color-text-primary: oklch(0.240 0.015 190);
  --color-text-secondary: oklch(0.402 0.020 190);
  --color-text-muted: oklch(0.500 0.020 190);
  --color-accent: oklch(0.400 0.090 175);
  --color-accent-hover: oklch(0.350 0.085 175);
  --color-accent-deep: oklch(0.880 0.027 175);
  --color-on-accent: oklch(0.985 0.003 190);
  --color-border: oklch(0.350 0.020 190 / 0.16);
  --color-border-soft: oklch(0.350 0.020 190 / 0.09);
  --color-border-control: oklch(0.600 0.020 190);
}

:root[data-palette="orchid"][data-theme="dark"] {
  --color-bg-secondary: oklch(0.125 0.020 320);
  --color-bg-primary: oklch(0.155 0.020 320);
  --color-bg-tertiary: oklch(0.190 0.020 320);
  --color-bg-surface: oklch(0.205 0.020 320);
  --color-bg-hover: oklch(0.235 0.020 320);
  --color-bg-glass: oklch(0.215 0.020 320 / 0.58);
  --color-scrim: oklch(0.090 0.010 320 / 0.8);
  --color-text-primary: oklch(0.955 0.030 320);
  --color-text-secondary: oklch(0.734 0.040 320);
  --color-text-muted: oklch(0.626 0.040 320);
  --color-accent: oklch(0.800 0.170 335);
  --color-accent-hover: oklch(0.850 0.119 335);
  --color-accent-deep: oklch(0.320 0.077 335);
  --color-on-accent: oklch(0.170 0.020 320);
  --color-border: oklch(0.550 0.020 320 / 0.28);
  --color-border-soft: oklch(0.550 0.020 320 / 0.14);
  --color-border-control: oklch(0.540 0.025 320);
}

:root[data-palette="orchid"][data-theme="light"] {
  --color-bg-secondary: oklch(0.948 0.008 320);
  --color-bg-primary: oklch(0.975 0.008 320);
  --color-bg-tertiary: oklch(0.930 0.008 320);
  --color-bg-surface: oklch(0.997 0.002 320);
  --color-bg-hover: oklch(0.915 0.008 320);
  --color-bg-glass: oklch(0.990 0.002 320 / 0.65);
  --color-scrim: oklch(0.250 0.015 320 / 0.45);
  --color-text-primary: oklch(0.240 0.012 320);
  --color-text-secondary: oklch(0.406 0.016 320);
  --color-text-muted: oklch(0.504 0.016 320);
  --color-accent: oklch(0.520 0.150 335);
  --color-accent-hover: oklch(0.470 0.142 335);
  --color-accent-deep: oklch(0.880 0.045 335);
  --color-on-accent: oklch(0.985 0.003 320);
  --color-border: oklch(0.350 0.020 320 / 0.16);
  --color-border-soft: oklch(0.350 0.020 320 / 0.09);
  --color-border-control: oklch(0.600 0.020 320);
}
"####;

    type Vars = HashMap<String, [f32; 4]>;
    /// `(palette, dark) -> css name without "--color-" -> oklch`.
    type Blocks = HashMap<(String, bool), Vars>;

    /// Reads the colour blocks of `index.css` + `palettes.css`: `@theme` is Aurora dark,
    /// `:root[data-theme="light"]` Aurora light, a `data-palette` + `data-theme` pair is that
    /// palette. Every other block is skipped, and so is anything that is not a plain `oklch()`.
    fn parse(css: &str) -> Blocks {
        let mut out = Blocks::new();
        let mut cur: Option<(String, bool)> = None;
        for line in css.lines().map(str::trim) {
            if line.ends_with('{') {
                let attr = |k: &str| line.split(&format!("{k}=\"")).nth(1).and_then(|r| r.split('"').next()).map(String::from);
                cur = match (attr("data-palette"), attr("data-theme")) {
                    _ if line == "@theme {" => Some(("aurora".into(), true)),
                    (None, Some(t)) if line.starts_with(":root[") => Some(("aurora".into(), t == "dark")),
                    (Some(p), Some(t)) => Some((p, t == "dark")),
                    _ => None,
                };
            } else if line.starts_with('}') {
                cur = None;
            } else if let (Some(key), Some(rest)) = (&cur, line.strip_prefix("--color-")) {
                let (name, val) = rest.split_once(':').unwrap();
                let Some(inner) = val.trim().strip_prefix("oklch(").and_then(|v| v.split(')').next()) else { continue };
                let (lch, a) = inner.split_once('/').map_or((inner, "1"), |(x, a)| (x, a.trim()));
                let n: Vec<f32> = lch.split_whitespace().map(|x| x.parse().unwrap()).collect();
                out.entry(key.clone()).or_default().insert(name.into(), [n[0], n[1], n[2], a.parse().unwrap()]);
            }
        }
        out
    }

    fn color(v: [f32; 4]) -> Color {
        oklcha(v[0], v[1], v[2], v[3])
    }

    /// The CSS cascade: Aurora dark is the base, Aurora light overrides it, a palette overrides both.
    fn cascade(b: &Blocks, id: &str, dark: bool) -> Vars {
        let mut v = b[&("aurora".to_string(), true)].clone();
        if !dark {
            v.extend(b[&("aurora".to_string(), false)].clone());
        }
        if id != "aurora" {
            v.extend(b[&(id.to_string(), dark)].clone());
        }
        v
    }

    fn platforms() -> Vec<(&'static str, SourcePlatform)> {
        use SourcePlatform::*;
        vec![
            ("youtube", Youtube),
            ("spotify", Spotify),
            ("soundcloud", Soundcloud),
            ("reels", Instagram),
            ("src-twitter", Twitter),
            ("src-vimeo", Vimeo),
            ("src-twitch", Twitch),
            ("src-bandcamp", Bandcamp),
            ("src-reddit", Reddit),
            ("src-dailymotion", Dailymotion),
            ("src-mixcloud", Mixcloud),
            ("src-rumble", Rumble),
        ]
    }

    /// Every role of every palette in both themes and every platform colour, against `blocks`.
    fn check(blocks: &Blocks) -> usize {
        let mut cases = 0;
        for (i, id) in PALETTE_IDS.iter().enumerate() {
            for dark in [true, false] {
                let want = cascade(blocks, id, dark);
                for (name, got) in roles(i, dark).list() {
                    assert_eq!(got, color(want[&name.replace('_', "-")]), "{id} dark={dark} {name}");
                    cases += 1;
                }
                for (css, src) in platforms() {
                    assert_eq!(platform(src, dark), color(want[css]), "{id} dark={dark} {css}");
                    cases += 1;
                }
            }
        }
        cases
    }

    #[test]
    fn ids_and_names_line_up() {
        assert!(!PALETTE_IDS.is_empty());
        assert_eq!(NAMES.len(), PALETTE_IDS.len());
        assert_eq!(TABLE.len(), PALETTE_IDS.len());
        for (n, id) in NAMES.iter().zip(PALETTE_IDS) {
            assert_eq!(n.to_lowercase(), id);
        }
        for i in 0..PALETTE_IDS.len() {
            for dark in [true, false] {
                assert_eq!(roles(i, dark).list().len(), 23);
            }
        }
    }

    #[test]
    fn roles_match_the_css_copy() {
        let blocks = parse(CSS);
        assert_eq!(blocks.len(), 14, "7 palettes x 2 themes");
        // 23 roles + 12 platform colours, in 14 palette/theme pairs
        assert_eq!(check(&blocks), 14 * (23 + 12));
    }

    #[test]
    fn mix_scales_alpha() {
        assert_eq!(mix(oklcha(0.5, 0.1, 20.0, 0.5), 0.1).a, 0.05);
    }

    /// Local only: the same comparison against the live Electron checkout. `AHG_ELECTRON` is the
    /// checkout's folder; the default is where it sits on the owner's machine.
    #[test]
    #[ignore]
    fn roles_match_the_live_electron_css() {
        let root = std::env::var("AHG_ELECTRON").unwrap_or_else(|_| "C:/Users/midou/Desktop/works/converter-electron".into());
        let src = std::path::Path::new(&root).join("src");
        let live = format!(
            "{}\n{}",
            // lossy: index.css has one stray non-UTF-8 byte in a comment
            String::from_utf8_lossy(&std::fs::read(src.join("index.css")).unwrap()),
            String::from_utf8_lossy(&std::fs::read(src.join("palettes.css")).unwrap())
        );
        let blocks = parse(&live);
        assert_eq!(blocks.len(), 14);
        println!("{} comparisons against {root}", check(&blocks));
    }
}
