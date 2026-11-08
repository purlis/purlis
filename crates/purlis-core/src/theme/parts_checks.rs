// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The checks family** (#4481, spec #4469): a list of things checked, each with its verdict,
//! declared once here and once in its wrapper, `app/src/ui/checks/Checks.tsx`, whose stylesheet
//! (`checks.css`) draws the defaults [`super::Look`] holds a copy of.
//!
//! A check's verdict is its variant, and is told by its mark and its words as well as its colour
//! (WCAG 1.4.1).

use super::{Family, Look, Part, Prop, State, TRANSPARENT, Variant};

/// What a line's words take: their colour, size and weight.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

const fn verdict(name: &'static str, description: &'static str) -> Variant {
    Variant {
        name,
        since: 13,
        description,
        consent: None,
    }
}

/// **The checks** (#4481): what was checked, and how each came out.
pub(super) const CHECKS: Family = Family {
    name: "checks",
    description: "A list of things checked, each with its verdict.",
    since: 13,
    parts: &[
        Part {
            name: "root",
            since: 13,
            description: "The list.",
            properties: &[Prop::Text],
        },
        Part {
            name: "mark",
            since: 13,
            description: "A line's mark: its verdict's icon.",
            properties: &[Prop::Text],
        },
        Part {
            name: "title",
            since: 13,
            description: "What was checked.",
            properties: WORDS,
        },
        Part {
            name: "more",
            since: 13,
            description: "What is said under it.",
            properties: WORDS,
        },
        Part {
            name: "trail",
            since: 13,
            description: "Something at the line's end: how long it took.",
            properties: WORDS,
        },
    ],
    variants: &[
        verdict("ok", "Passed."),
        verdict("missing", "Failed."),
        verdict("running", "Being checked now."),
        verdict("todo", "To be checked when the check runs."),
        verdict("warn", "Passed, with something to look at."),
        verdict("skip", "Not checked, and why is said."),
    ],
    // A card, and a page's ground.
    grounds: &["surface.raised", "surface.base"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.5,
    border_width: 0.0,
    defaults: checks_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[("more", "text.secondary"), ("trail", "text.muted")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
    later: &[],
};

/// **A line as `checks.css` draws it**: no ground of its own, its words in the primary tier.
pub(super) fn checks_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: TRANSPARENT,
    }
}
