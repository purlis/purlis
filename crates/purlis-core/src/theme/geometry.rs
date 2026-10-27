// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Geometry a theme may set, and the bounds it is held to** (spec #2313, #2326, ADR 0097 §2).
//!
//! Every size the window draws from is a token here: the spacing scale, the type scale, the
//! three weights, the two line heights, the corners, the control and strip heights, the focus
//! ring and the two lifts. Each has a range core documents, and **a value outside its range, or
//! one that is not a number, refuses the theme** with a sentence that names the range. Core
//! gives every token its value, which is Purlis Classic's (`app/src/theme/geometry.css`, held
//! to [`STEPS`] by a test), so a theme names only what it changes.
//!
//! **Density** is the person's setting, comfortable or compact, and every theme has both: core
//! defines what compact means for Purlis Classic ([`Step::compact`]), and a theme may tune its
//! own compact values in a `compact` layer. A token core's compact leaves as it is follows the
//! theme's comfortable value; one core's compact moves is the smaller of core's compact value and
//! the theme's comfortable one, unless the theme's `compact` layer names it.
//!
//! A geometry value is a number. What reaches the window is written here from that number
//! ([`properties`]), never copied from a file (ADR 0041, property 4).

use std::collections::BTreeMap;

use serde_json::Value;

/// How a geometry token's number is written for the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Pixels at the window's default text size, written in rem so the length grows with the
    /// text-size setting: `6` is `calc(6rem / 14)`.
    Text,
    /// Pixels, hairline-scale: a corner, the focus ring.
    Px,
    /// A font weight, a whole number.
    Weight,
    /// A line height, a share of the text's own size.
    Ratio,
}

/// **One geometry token**: its name in a theme, the custom property it is written to, its unit,
/// its documented range, and Purlis Classic's comfortable and compact values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    /// What a theme names it by, in an appearance's `tokens` or the `compact` layer.
    pub token: &'static str,
    /// The custom property the window draws it from. The two numbers of a lift write one
    /// property between them.
    pub property: &'static str,
    pub unit: Unit,
    /// The smallest value a theme may give it.
    pub least: f64,
    /// The largest.
    pub most: f64,
    /// Purlis Classic's value, and core's.
    pub classic: f64,
    /// Purlis Classic's compact value.
    pub compact: f64,
}

const fn step(
    token: &'static str,
    property: &'static str,
    unit: Unit,
    (least, most): (f64, f64),
    classic: f64,
    compact: f64,
) -> Step {
    Step {
        token,
        property,
        unit,
        least,
        most,
        classic,
        compact,
    }
}

use Unit::{Px, Ratio, Text, Weight};

/// **Every geometry token**, in the order `geometry.css` declares them. The documented ranges
/// are `docs/design-system.md`'s table.
pub const STEPS: &[Step] = &[
    // The spacing scale, at the default text size.
    step("space.0", "--space-0", Text, (0.0, 0.0), 0.0, 0.0),
    step("space.1", "--space-1", Text, (0.0, 4.0), 2.0, 2.0),
    step("space.2", "--space-2", Text, (1.0, 8.0), 4.0, 3.0),
    step("space.3", "--space-3", Text, (2.0, 10.0), 6.0, 4.0),
    step("space.4", "--space-4", Text, (4.0, 12.0), 8.0, 6.0),
    step("space.5", "--space-5", Text, (6.0, 16.0), 12.0, 8.0),
    step("space.6", "--space-6", Text, (8.0, 24.0), 16.0, 12.0),
    step("space.7", "--space-7", Text, (12.0, 32.0), 24.0, 16.0),
    step("space.8", "--space-8", Text, (16.0, 48.0), 32.0, 24.0),
    // The type scale. Text size is the person's own setting, so compact leaves it.
    step(
        "font-size.2xs",
        "--font-size-2xs",
        Text,
        (8.0, 12.0),
        10.0,
        10.0,
    ),
    step(
        "font-size.xs",
        "--font-size-xs",
        Text,
        (9.0, 13.0),
        11.0,
        11.0,
    ),
    step(
        "font-size.sm",
        "--font-size-sm",
        Text,
        (10.0, 14.0),
        12.0,
        12.0,
    ),
    step(
        "font-size.md",
        "--font-size-md",
        Text,
        (11.0, 15.0),
        13.0,
        13.0,
    ),
    step(
        "font-size.base",
        "--font-size-base",
        Text,
        (12.0, 18.0),
        14.0,
        14.0,
    ),
    step(
        "font-size.lg",
        "--font-size-lg",
        Text,
        (13.0, 22.0),
        16.0,
        16.0,
    ),
    step(
        "font-size.xl",
        "--font-size-xl",
        Text,
        (16.0, 28.0),
        20.0,
        20.0,
    ),
    // The weights and line heights.
    step(
        "font-weight.regular",
        "--font-weight-regular",
        Weight,
        (300.0, 500.0),
        400.0,
        400.0,
    ),
    step(
        "font-weight.medium",
        "--font-weight-medium",
        Weight,
        (400.0, 600.0),
        500.0,
        500.0,
    ),
    step(
        "font-weight.strong",
        "--font-weight-strong",
        Weight,
        (500.0, 800.0),
        600.0,
        600.0,
    ),
    step(
        "line-height.tight",
        "--line-height-tight",
        Ratio,
        (1.0, 1.4),
        1.2,
        1.2,
    ),
    step(
        "line-height.body",
        "--line-height-body",
        Ratio,
        (1.2, 1.8),
        1.4,
        1.3,
    ),
    // Corners.
    step("radius.sm", "--radius-sm", Px, (0.0, 8.0), 3.0, 3.0),
    step("radius.md", "--radius-md", Px, (0.0, 10.0), 4.0, 4.0),
    // An md control's corner (#3113, #3177): a button's, a field's, a select's, a list's row
    // and a title-bar chip. Purlis Classic's is its `radius.md`; Purlis draws 8 against its 7.
    step(
        "radius.control",
        "--radius-control",
        Px,
        (0.0, 14.0),
        4.0,
        4.0,
    ),
    step("radius.lg", "--radius-lg", Px, (0.0, 14.0), 6.0, 6.0),
    step("radius.xl", "--radius-xl", Px, (0.0, 18.0), 8.0, 8.0),
    step(
        "radius.full",
        "--radius-full",
        Px,
        (0.0, 9999.0),
        9999.0,
        9999.0,
    ),
    // Heights: every control, and every strip and pane header.
    step(
        "size.control",
        "--size-control",
        Text,
        (20.0, 40.0),
        28.0,
        24.0,
    ),
    step(
        "size.chrome",
        "--size-chrome",
        Text,
        (16.0, 32.0),
        21.0,
        18.0,
    ),
    // The window's geometry (#2805, #2913): the gap between its regions, a region panel's
    // corners and the hairline between regions. Purlis Classic draws flush: no gap, square
    // corners, and the hairline it always drew.
    step(
        "window.gutter",
        "--window-gutter",
        Px,
        (0.0, 16.0),
        0.0,
        0.0,
    ),
    step(
        "panel.corner-radius",
        "--panel-corner-radius",
        Px,
        (0.0, 24.0),
        0.0,
        0.0,
    ),
    step(
        "panel.border-width",
        "--panel-border-width",
        Px,
        (0.0, 2.0),
        1.0,
        1.0,
    ),
    // The separator part's width (#2912, #2877): the line between two groups inside a surface,
    // 0 for none. Its colour is the `separator` family's part.
    step(
        "separator.width",
        "--separator-width",
        Px,
        (0.0, 2.0),
        1.0,
        1.0,
    ),
    // The focus ring: never thinner than a hairline, so it stays visible.
    step(
        "focus-ring.width",
        "--focus-ring-width",
        Px,
        (1.0, 4.0),
        2.0,
        2.0,
    ),
    step(
        "focus-ring.offset",
        "--focus-ring-offset",
        Px,
        (0.0, 4.0),
        2.0,
        2.0,
    ),
    step(
        "focus-ring.inset",
        "--focus-ring-inset",
        Px,
        (-4.0, 0.0),
        -2.0,
        -2.0,
    ),
    // The line around a pane (#2914, the operator's ruling of 2026-10-22): Classic's hairline,
    // which the focused pane draws in the accent; none in Purlis, whose panes are the panel.
    step(
        "pane.border-width",
        "--pane-border-width",
        Px,
        (0.0, 2.0),
        1.0,
        1.0,
    ),
    // The two lifts: how far a surface's shadow falls, and how soft it is.
    step(
        "elevation.raised-y",
        "--elevation-raised",
        Text,
        (0.0, 8.0),
        2.0,
        2.0,
    ),
    step(
        "elevation.raised-blur",
        "--elevation-raised",
        Text,
        (0.0, 24.0),
        8.0,
        8.0,
    ),
    step(
        "elevation.overlay-y",
        "--elevation-overlay",
        Text,
        (0.0, 16.0),
        8.0,
        8.0,
    ),
    step(
        "elevation.overlay-blur",
        "--elevation-overlay",
        Text,
        (0.0, 48.0),
        24.0,
        24.0,
    ),
];

/// **A theme's geometry, complete**: every token of [`STEPS`] and its number.
pub type Geometry = BTreeMap<&'static str, f64>;

/// The geometry token `token` names, if it is one.
pub fn step_of(token: &str) -> Option<&'static Step> {
    STEPS.iter().find(|one| one.token == token)
}

/// Purlis Classic's comfortable geometry: core's.
pub fn classic() -> Geometry {
    STEPS.iter().map(|one| (one.token, one.classic)).collect()
}

/// Purlis Classic's compact geometry.
pub fn classic_compact() -> Geometry {
    STEPS.iter().map(|one| (one.token, one.compact)).collect()
}

/// **The compact geometry of a theme whose comfortable geometry is `comfortable`**: where core's
/// compact moves a token, the smaller of core's compact value and the theme's comfortable one,
/// so a theme's compact is never looser than its comfortable; where core's compact does not,
/// the comfortable value; and then the theme's own `compact` layer over both.
pub fn compact_of(comfortable: &Geometry, layer: &Geometry) -> Geometry {
    STEPS
        .iter()
        .map(|one| {
            let own = comfortable.get(one.token).copied().unwrap_or(one.classic);
            let value = match layer.get(one.token) {
                Some(tuned) => *tuned,
                None if one.compact != one.classic => one.compact.min(own),
                None => own,
            };
            (one.token, value)
        })
        .collect()
}

/// A geometry value as a theme gives it, held to `step`'s type and range, or the sentence that
/// says why not. `at` says where it was given (`in appearance dark`).
pub fn number(step: &Step, value: &Value, at: &str) -> Result<f64, String> {
    let range = range(step);
    let Some(number) = value.as_f64().filter(|n| n.is_finite()) else {
        return Err(format!(
            "{} {at} is {}, which is not a number; it is {range}",
            step.token,
            super::stringify(value)
        ));
    };
    let whole = !matches!(step.unit, Unit::Ratio);
    if whole && number.fract() != 0.0 {
        return Err(format!(
            "{} {at} is {}, which is not a whole number; it is {range}",
            step.token,
            super::stringify(value)
        ));
    }
    if number < step.least || number > step.most {
        return Err(format!(
            "{} {at} is {}, outside its bound: it is {range}",
            step.token,
            super::stringify(value)
        ));
    }
    Ok(number)
}

/// A step's documented range, as a sentence says it.
fn range(step: &Step) -> String {
    let unit = match step.unit {
        Unit::Text => "px at the default text size",
        Unit::Px => "px",
        Unit::Weight => "as a font weight",
        Unit::Ratio => "times the text's size",
    };
    let kind = if matches!(step.unit, Unit::Ratio) {
        "a number"
    } else {
        "a whole number"
    };
    if step.least == step.most {
        format!("{} in every theme", show(step.least))
    } else {
        format!(
            "{kind} from {} to {} {unit}",
            show(step.least),
            show(step.most)
        )
    }
}

/// A number as a sentence or a declaration writes it: no trailing `.0`.
pub(super) fn show(number: f64) -> String {
    if number.fract() == 0.0 {
        format!("{}", number as i64)
    } else {
        format!("{number}")
    }
}

/// A length at the default text size, in rem: a whole share of 14 written plainly (`1rem`,
/// `1.5rem`, `2rem`), every other one as `calc(Nrem / 14)`, as `geometry.css` writes them.
pub(super) fn text_length(px: f64) -> String {
    if px == 0.0 {
        return "0".to_owned();
    }
    if (px % 7.0) == 0.0 {
        return format!("{}rem", show(px / 14.0));
    }
    format!("calc({}rem / 14)", show(px))
}

/// **`geometry` as the custom properties the window writes**, each value written here from its
/// number: what the stylesheet's `geometry.css` holds for Purlis Classic, byte for byte.
pub fn properties(geometry: &Geometry) -> BTreeMap<&'static str, String> {
    let value = |token: &str| {
        geometry
            .get(token)
            .copied()
            .or_else(|| step_of(token).map(|one| one.classic))
            .unwrap_or_default()
    };
    let mut out = BTreeMap::new();
    for one in STEPS {
        if one.property.starts_with("--elevation-") {
            continue;
        }
        let number = value(one.token);
        let written = match one.unit {
            Unit::Text => text_length(number),
            Unit::Px => format!("{}px", show(number)),
            Unit::Weight | Unit::Ratio => show(number),
        };
        out.insert(one.property, written);
    }
    for (property, lift) in [
        ("--elevation-raised", "elevation.raised"),
        ("--elevation-overlay", "elevation.overlay"),
    ] {
        out.insert(
            property,
            format!(
                "0 {} {} var(--overlay-shadow)",
                text_length(value(&format!("{lift}-y"))),
                text_length(value(&format!("{lift}-blur")))
            ),
        );
    }
    out
}
