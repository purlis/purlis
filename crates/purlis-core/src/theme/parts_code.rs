// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The code family** (#4481, spec #4469): text to read and copy, declared once here and once in
//! its wrapper, `app/src/ui/code/CodeBlock.tsx`, whose stylesheet (`code.css`) draws the defaults
//! [`super::Look`] holds a copy of.

use super::{Family, Look, Part, Prop, State};

/// **The code** (#4481): a command or YAML in a box, with its Copy button.
pub(super) const CODE: Family = Family {
    name: "code",
    description: "Text to read and copy: a command or YAML.",
    since: 13,
    parts: &[
        Part {
            name: "root",
            since: 13,
            description: "The box: its ground, its edge and its corners.",
            properties: &[Prop::Fill, Prop::BorderColour, Prop::CornerRadius],
        },
        Part {
            name: "label",
            since: 13,
            description: "What the text is.",
            properties: &[Prop::Text, Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "body",
            since: 13,
            description: "The text.",
            properties: &[Prop::Text, Prop::FontSize, Prop::Padding],
        },
    ],
    variants: &[],
    // A card, and a page's ground.
    grounds: &["surface.raised", "surface.base"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.sm",
    line_height: 1.5,
    border_width: 1.0,
    defaults: code_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[("label", "text.secondary")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
    later: &[],
};

/// **The box as `code.css` draws it**: on the sunken ground, edged subtly.
pub(super) fn code_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: "surface.sunken",
        text: "text.primary",
        border: "border.subtle",
    }
}
