//! Signal's colours. Palette 0 is the desk's own: graphite surfaces with one
//! cue-blue accent, the colour of a lit meter segment. Palettes 1 to 7 are the
//! app's existing ones (Aurora, Slate, Moss, Ember, Paper, Lagoon, Orchid), taken
//! from `crate::palette` (Electron's tables). Platform and status colours are
//! `palette`'s, the same in every palette.
use ahg_engine::types::SourcePlatform;
use rune::{Color, oklch};

use crate::overlays::{Mark, Skin};
use crate::palette;

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

/// Palette 0 only, `[dark, light]`: Signal's own graphite and cue-blue. Palettes 1 to 7 come from
/// `palette::roles` (Electron's tables).
const OWN: [Ramp; 2] = [
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
];

pub fn pal(palette: usize, dark: bool) -> Pal {
    // Palette 0 is Signal's own ramp; 1 to 7 are Electron's table. The status and media colours are shared.
    let r = palette::roles(palette.saturating_sub(1), dark);
    let (deep, bg, surface, raised, text, text2, muted, accent, accent_ink) = if palette == 0 {
        let ramp = &OWN[usize::from(!dark)];
        let c = |lc: [f32; 2]| oklch(lc[0], lc[1], ramp.hue);
        let accent = oklch(ramp.accent[0], ramp.accent[1], ramp.accent[2]);
        // Signal's own light accent is a lit segment first; as ink it goes a step darker.
        let accent_ink = if dark { accent } else { oklch(0.480, 0.160, 255.0) };
        (c(ramp.deep), c(ramp.bg), c(ramp.surface), c(ramp.raised), c(ramp.text), c(ramp.text2), c(ramp.muted), accent, accent_ink)
    } else {
        (r.bg_secondary, r.bg_primary, r.bg_surface, r.bg_hover, r.text_primary, r.text_secondary, r.text_muted, r.accent, r.accent)
    };
    let ink = |a: f32| text.alpha(a);
    Pal {
        dark,
        deep,
        bg,
        surface,
        raised,
        line: ink(if dark { 0.08 } else { 0.09 }),
        line_strong: ink(if dark { 0.14 } else { 0.16 }),
        text,
        text2,
        muted,
        accent,
        accent_ink,
        unlit: ink(if dark { 0.09 } else { 0.08 }),
        hover: ink(0.05),
        press: ink(0.09),
        success: r.success,
        warning: r.warning,
        error: r.error,
        error_soft: palette::mix(r.error, 0.10),
        warning_soft: palette::mix(r.warning, 0.10),
        media_scrim: r.media_scrim,
        on_media: r.on_media,
    }
}

/// The overlays on the desk: a card-grey panel, hard 8 pt corners, the command line in mono,
/// the highlighted row a well with one lit segment. Words on the accent in the desk's deepest
/// grey, as on its own buttons.
pub fn skin(palette: usize, dark: bool) -> Skin {
    let p = pal(palette, dark);
    Skin {
        dark,
        bg: p.surface,
        raised: p.raised,
        text: p.text,
        text2: p.text2,
        line: p.line_strong,
        accent: p.accent_ink,
        on_accent: p.deep,
        error: p.error,
        radius: 8.0,
        mark: Mark::Segment(p.accent),
        mono: true,
        serif: false,
    }
}

/// A source's colour (`palette::platform`; direct, generic and unknown links take the accent as ink).
pub fn source_color(p: &Pal, source: SourcePlatform) -> Color {
    use SourcePlatform::*;
    match source {
        Direct | Generic | Unknown => p.accent_ink,
        _ => palette::platform(source, p.dark),
    }
}
