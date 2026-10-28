// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The meter family** (#3674, spec #2897 as the operator amended it on 2026-10-28): a value
//! drawn as a bar, declared once here and once in its wrapper, `app/src/ui/meter/Meter.tsx`,
//! whose stylesheet (`meter.css`) draws the defaults [`super::Look`], [`meter_fill`] and the
//! track's fill hold a copy of.
//!
//! A meter is a grade's score beside the ones it compares with (with it, without it, its
//! previous version), and a progress bar how far a running grade is. Its parts are its root
//! (the line that holds the rest, and its variant), its label, its track, the fill on the track
//! and its value in figures.
//!
//! **Its fill is a mark on its track** ([`super::Tracked`]): held at 3:1 there on every ground it
//! stands on (WCAG 1.4.11), in each variant, so a theme that colours a fill or a track so the
//! bar is lost draws that variant's default back. Its label and its value are words, held at
//! 4.5:1 as every part's words are. A variant is told apart by its label and the baseline's
//! hatch as well as its colour (WCAG 1.4.1), which no theme takes away: the hatch is the
//! stylesheet's, not a part's property.

use super::{Family, Look, Part, Prop, State, TRANSPARENT, Tracked, Variant};

/// What a meter's words take: their colour, size and weight.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

/// **The meter** (#3674): a labelled value on a known range, and a progress bar.
pub(super) const METER: Family = Family {
    name: "meter",
    description: "A value drawn as a bar: a grade's score beside the ones it compares with, how \
                  far a running grade is.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The meter's line: its words, and the room between its label, its \
                          track and its value.",
            properties: &[Prop::Text, Prop::Gap],
        },
        Part {
            name: "label",
            since: super::NEXT,
            description: "Its name, at its start.",
            properties: WORDS,
        },
        Part {
            name: "track",
            since: super::NEXT,
            description: "The track the fill is drawn on: its fill and its corners.",
            properties: &[Prop::Fill, Prop::CornerRadius],
        },
        Part {
            name: "fill",
            since: super::NEXT,
            description: "The bar, as long as the value: held at 3:1 on its track.",
            properties: &[Prop::Fill],
        },
        Part {
            name: "value",
            since: super::NEXT,
            description: "The value in figures, at its end.",
            properties: WORDS,
        },
    ],
    variants: &[
        Variant {
            name: "subject",
            since: super::NEXT,
            description: "What is measured: the skill with it, how far a grade is.",
            consent: None,
        },
        Variant {
            name: "baseline",
            since: super::NEXT,
            description: "What it is compared with: the chats without it, hatched.",
            consent: None,
        },
        Variant {
            name: "previous",
            since: super::NEXT,
            description: "Its previous version.",
            consent: None,
        },
    ],
    // A page's ground, and a card's or a panel's raised one.
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.sm",
    line_height: 1.4,
    border_width: 0.0,
    defaults: meter_look,
    states: &[],
    focused: "[data-focus-visible]",
    // Its label is in the secondary tier.
    words: &[("label", "text.secondary")],
    corner_most: None,
    around: &[],
    // Its track's own fill, the subtle hairline's colour.
    fills: &[("track", "border.subtle")],
    under: &[],
};

/// **How a meter's fill stands on its track** ([`super::Tracked`]): the track is its third part,
/// the fill its fourth.
pub(super) const TRACKED: Tracked = Tracked {
    track: 2,
    fill: 3,
    fill_of: meter_fill,
};

/// **A meter's root as `meter.css` draws it**: no ground of its own, its words in the primary
/// tier.
pub(super) fn meter_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: TRANSPARENT,
    }
}

/// **A meter's fill as `meter.css` draws it, by variant**: the accent for the subject, the
/// waiting colour for the baseline, the muted tier for the previous version.
pub(super) fn meter_fill(variant: &str) -> &'static str {
    match variant {
        "baseline" => "state.waiting",
        "previous" => "text.muted",
        _ => "accent.base",
    }
}
