// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The steps family** (#4480, spec #4469): a guide's list of steps, each with its state, declared
//! once here and once in its wrapper, `app/src/ui/steps/Steps.tsx`, whose stylesheet (`steps.css`)
//! draws the defaults [`super::Look`] holds a copy of.
//!
//! A step's state is its variant, and is told by its mark and its words as well as its colour
//! (WCAG 1.4.1).

use super::{Family, Look, Part, Prop, State, TRANSPARENT, Variant};

/// What a step's words take: their colour, size and weight.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

/// **The steps** (#4480): a guide's list of steps.
pub(super) const STEPS: Family = Family {
    name: "steps",
    description: "A guide's list of steps: how far along the person is, each step's state, and a \
                  way back to any step reached.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The list and what is said around it.",
            properties: &[Prop::Text],
        },
        Part {
            name: "count",
            since: super::NEXT,
            description: "How many steps are done.",
            properties: WORDS,
        },
        Part {
            name: "press",
            since: super::NEXT,
            description: "A step's whole line, pressed to go to it: its corners and its padding.",
            properties: &[Prop::CornerRadius, Prop::Padding],
        },
        Part {
            name: "mark",
            since: super::NEXT,
            description: "A step's mark: its state's icon, or its number.",
            properties: &[Prop::Text, Prop::BorderColour],
        },
        Part {
            name: "name",
            since: super::NEXT,
            description: "A step's name.",
            properties: WORDS,
        },
        Part {
            name: "line",
            since: super::NEXT,
            description: "A short line under its name: what it settled on, or what it needs.",
            properties: WORDS,
        },
    ],
    variants: &[
        Variant {
            name: "done",
            since: super::NEXT,
            description: "Finished.",
            consent: None,
        },
        Variant {
            name: "current",
            since: super::NEXT,
            description: "Where the person has reached.",
            consent: None,
        },
        Variant {
            name: "attention",
            since: super::NEXT,
            description: "Something here needs the person.",
            consent: None,
        },
        Variant {
            name: "skipped",
            since: super::NEXT,
            description: "The person skipped it.",
            consent: None,
        },
        Variant {
            name: "waiting",
            since: super::NEXT,
            description: "Waiting on someone else.",
            consent: None,
        },
        Variant {
            name: "later",
            since: super::NEXT,
            description: "Not reached yet, and can't be pressed.",
            consent: None,
        },
    ],
    // A view's sunken side, and a page's ground.
    grounds: &["surface.sunken", "surface.base"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.sm",
    line_height: 1.4,
    border_width: 0.0,
    defaults: steps_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[("count", "text.muted"), ("line", "text.muted")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **A step's line as `steps.css` draws it**: no ground of its own, its words in the primary tier.
pub(super) fn steps_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: TRANSPARENT,
    }
}
