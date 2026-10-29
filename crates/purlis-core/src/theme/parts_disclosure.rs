// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The disclosure** (#3675, spec #2897, added to the closed family list by the operator on
//! 2026-10-28): a button that shows and hides the region under it, declared once here and once
//! in its wrapper, `app/src/ui/disclosure/Disclosure.tsx`, whose stylesheet (`disclosure.css`)
//! draws the defaults [`super::Look`] holds a copy of.
//!
//! Its trigger is a button of the button family (the quiet ghost), which a theme reaches by the
//! button's own parts; the disclosure's are the row the trigger stands in, the twist that turns
//! in it, and the panel it shows. It draws no state of its own: opening it turns the twist, which
//! is a transform, and moves only what is under it.

use super::{Family, Look, Part, Prop, State, TRANSPARENT};

/// **The disclosure** (#3675): Add source's More, a file to read under its risk mark, a deleted
/// skill's last text.
pub(super) const DISCLOSURE: Family = Family {
    name: "disclosure",
    description: "A button that shows and hides the region under it: Add source's More, a file \
                  to read under its risk mark, a deleted skill's last text.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The disclosure: its trigger's row and, open, the panel under it.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "trigger",
            since: super::NEXT,
            description: "The row the trigger stands in, with any mark after it: the room around \
                          and between them. The trigger is a button of the button family.",
            properties: &[Prop::Padding, Prop::Gap],
        },
        Part {
            name: "twist",
            since: super::NEXT,
            description: "The chevron in the trigger, which turns as it opens and never swaps \
                          for another.",
            properties: &[Prop::Text],
        },
        Part {
            name: "panel",
            since: super::NEXT,
            description: "The region it shows, under its trigger: its ground, its edge, its \
                          corners and the room in it.",
            properties: &[
                Prop::Fill,
                Prop::BorderColour,
                Prop::CornerRadius,
                Prop::Padding,
                Prop::Gap,
            ],
        },
    ],
    variants: &[],
    // A page's panel (the base surface, the raised one a theme floats it on) and a dialog's body.
    grounds: &["surface.base", "surface.raised", "surface.overlay"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.md",
    line_height: 1.4,
    border_width: 0.0,
    defaults: disclosure_look,
    states: &[],
    focused: "[data-focus-visible]",
    // The twist in the quiet tier, as the tree's is.
    words: &[("twist", "text.quiet")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **The disclosure's colours as `disclosure.css` draws them**: no ground and no edge of its own
/// (what it stands on shows), the window's words.
pub(super) fn disclosure_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: TRANSPARENT,
    }
}
