// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A workspace's colour, as a hue shift** (charter-app#281): the arithmetic that turns the
//! tokens of [`super::vocabulary::TINTED`] to a workspace's hue, so [`super::check`] can hold a
//! theme's pairs at every hue a workspace can name.
//!
//! The shift is done in OKLCH, and then the lightness is searched for until the tinted colour
//! has **the relative luminance the original had**, so every contrast ratio a theme cleared its
//! tinted shades clear too, up to the rounding of one 8-bit channel. A grey is given
//! [`NEUTRAL_CHROMA`]; a colour keeps its own chroma, lowered (never the lightness) until the
//! screen can show it.
//!
//! The window draws the same tints live (`app/src/theme/tint.ts`), and the two are held to one
//! fixture of inputs and answers (`theme/tint-cases.json`), which both test suites read.

use super::{ResolvedTheme, vocabulary::TINTED};

/// The palette a workspace names its colour from, as OKLCH hues in degrees, in
/// [`crate::extension::project::theme::PALETTE`]'s order.
pub const PALETTE: [(&str, f64); 8] = [
    ("red", 25.0),
    ("orange", 55.0),
    ("yellow", 95.0),
    ("green", 145.0),
    ("teal", 190.0),
    ("blue", 255.0),
    ("purple", 305.0),
    ("pink", 350.0),
];

/// How much colour a neutral grey is given: a step a person feels on a strip rather than sees.
const NEUTRAL_CHROMA: f64 = 0.03;

/// Below this chroma a colour counts as grey, and has no hue of its own worth keeping.
const GREY: f64 = 0.02;

type Vec3 = [f64; 3];

/// The hue a workspace's colour tints with: a [`PALETTE`] name's, or the OKLCH hue of a
/// `#rrggbb`. `None` for anything else, and for a grey, which has no hue to give.
pub fn hue_of(colour: &str) -> Option<f64> {
    if let Some((_, hue)) = PALETTE.iter().find(|(name, _)| *name == colour) {
        return Some(*hue);
    }
    let digits = colour.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let [_, a, b] = oklab(parse(colour)?.0);
    if a.hypot(b) < GREY {
        return None;
    }
    Some(degrees(b.atan2(a)))
}

/// `hex` turned to `hue`, with the relative luminance it had. `hex` is a theme's value, so it is
/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`; an alpha it had, it keeps. A value that is not a
/// colour is answered as it is.
pub fn tint_hex(hex: &str, hue: f64) -> String {
    let Some((rgb, alpha)) = parse(hex) else {
        return hex.to_owned();
    };
    let target = luminance(rgb);
    let [_, a, b] = oklab(rgb);
    let had = a.hypot(b);
    let chroma = if had < GREY { NEUTRAL_CHROMA } else { had };
    let at = |lightness: f64| shown(lightness, chroma, hue);
    // Luminance rises with lightness at a fixed chroma and hue, so the lightness that gives the
    // luminance the colour had is found by halving the interval.
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    for _ in 0..32 {
        let middle = (low + high) / 2.0;
        if luminance(at(middle)) < target {
            low = middle;
        } else {
            high = middle;
        }
    }
    let rgb = at((low + high) / 2.0);
    format!(
        "#{}{}{}{alpha}",
        channel(rgb[0]),
        channel(rgb[1]),
        channel(rgb[2])
    )
}

/// `theme` with a workspace's colour turning [`TINTED`] to its hue; the theme as it is for a
/// colour with no hue.
pub fn tinted(theme: &ResolvedTheme, colour: &str) -> ResolvedTheme {
    let Some(hue) = hue_of(colour) else {
        return theme.clone();
    };
    let mut tint = theme.clone();
    for token in TINTED {
        if let Some(value) = tint.values.get_mut(token) {
            *value = tint_hex(value, hue);
        }
    }
    tint
}

/// A theme value in OKLab, its alpha left out; `None` for a value that is not a colour.
pub(super) fn oklab_of(hex: &str) -> Option<Vec3> {
    parse(hex).map(|(rgb, _)| oklab(rgb))
}

/// WCAG's relative luminance of a colour in linear sRGB.
fn luminance([r, g, b]: Vec3) -> f64 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// The colour at this OKLCH, in linear sRGB, with the chroma lowered until the screen can show
/// it when it cannot.
fn shown(lightness: f64, chroma: f64, hue: f64) -> Vec3 {
    let at = |c: f64| linear([lightness, c * cos(hue), c * sin(hue)]);
    let whole = at(chroma);
    if in_gamut(whole) {
        return whole;
    }
    let (mut low, mut high) = (0.0_f64, chroma);
    for _ in 0..24 {
        let middle = (low + high) / 2.0;
        if in_gamut(at(middle)) {
            low = middle;
        } else {
            high = middle;
        }
    }
    at(low).map(|v| v.clamp(0.0, 1.0))
}

fn in_gamut(rgb: Vec3) -> bool {
    rgb.iter().all(|v| *v >= -1e-6 && *v <= 1.0 + 1e-6)
}

fn cos(deg: f64) -> f64 {
    (deg * std::f64::consts::PI / 180.0).cos()
}

fn sin(deg: f64) -> f64 {
    (deg * std::f64::consts::PI / 180.0).sin()
}

fn degrees(rad: f64) -> f64 {
    (rad * 180.0 / std::f64::consts::PI + 360.0) % 360.0
}

/// A theme value as linear sRGB, and its alpha as the two hex digits it was written with.
fn parse(hex: &str) -> Option<(Vec3, String)> {
    let digits = hex.strip_prefix('#')?;
    if !super::vocabulary::is_colour(hex) {
        return None;
    }
    let long: String = if digits.len() <= 4 {
        digits.chars().flat_map(|d| [d, d]).collect()
    } else {
        digits.to_owned()
    };
    let byte = |at: usize| u8::from_str_radix(&long[at..at + 2], 16).ok();
    let rgb = [
        to_linear(f64::from(byte(0)?) / 255.0),
        to_linear(f64::from(byte(2)?) / 255.0),
        to_linear(f64::from(byte(4)?) / 255.0),
    ];
    Some((rgb, long.get(6..8).unwrap_or_default().to_owned()))
}

/// One linear channel as two hex digits.
fn channel(linear_value: f64) -> String {
    let v = linear_value.clamp(0.0, 1.0);
    let gamma = if v <= 0.003_130_8 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    // JavaScript's `Math.round`, which rounds a half up; the value is never negative here.
    let rounded = (gamma * 255.0 + 0.5).floor().clamp(0.0, 255.0);
    format!("{:02x}", rounded as u8)
}

fn to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear sRGB to OKLab (Björn Ottosson's matrices).
fn oklab([r, g, b]: Vec3) -> Vec3 {
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    ]
}

/// OKLab to linear sRGB.
fn linear([lightness, a, b]: Vec3) -> Vec3 {
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powf(3.0);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powf(3.0);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548 * b).powf(3.0);
    [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    ]
}
