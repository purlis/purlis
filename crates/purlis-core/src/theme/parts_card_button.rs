// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A card pressed whole** (#3885, #3655): the welcome's doors and a repo to get, declared once
//! here and once in its wrapper, `app/src/ui/card/CardButton.tsx`, whose stylesheet (`card.css`)
//! draws the defaults [`super::Look`] holds a copy of.
//!
//! It is a family of its own, not a part of the card's, so the order between them is explicit:
//! its parts are keyed on its own scope, `card-button`, and a theme's `card.*` keys never reach
//! them, inside a card or out of one (the clash #3834's review found: a `card.title` would have
//! won over the pressed card's own title). Its box is the root and says the button's state, as
//! React Aria says it of the button in it, so a state key reaches the box. It is a thing that is
//! pressed, drawn in the `control.*` shades every theme sets; a pick is one line on the surface
//! under it, in the accent's words.

use super::{Family, Look, Part, Prop, State, TRANSPARENT, Variant};

/// What a word's part may set: its colour, size and weight.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

/// **A card pressed whole** (#3885): a door on the welcome, a repo to get.
pub(super) const CARD_BUTTON: Family = Family {
    name: "card-button",
    description: "A card pressed whole: a door on the welcome, a repo to get from a list.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The card's box, the size of the button in it: its fill, words, edge, \
                          corners and the room inside it.",
            properties: &[
                Prop::Fill,
                Prop::Text,
                Prop::BorderColour,
                Prop::CornerRadius,
                Prop::Padding,
                Prop::Gap,
            ],
        },
        Part {
            name: "mark",
            since: super::NEXT,
            description: "A door's mark before its title, in the accent: the only colour on a \
                          page of doors.",
            properties: &[Prop::Text],
        },
        Part {
            name: "title",
            since: super::NEXT,
            description: "Its name, the first thing on it and the button's own words.",
            properties: WORDS,
        },
        Part {
            name: "line",
            since: super::NEXT,
            description: "What it does, under its title or after it: its description.",
            properties: &[Prop::Text, Prop::FontSize],
        },
    ],
    variants: &[
        Variant {
            name: "door",
            since: super::NEXT,
            description: "A door: its mark, title and line in a column, on the control's ground.",
            consent: None,
        },
        Variant {
            name: "pick",
            since: super::NEXT,
            description: "A pick from a list: one line on the surface under it, its words in the \
                          accent.",
            consent: None,
        },
    ],
    // The welcome's page and the Get page's list (the base surface, the raised one a theme floats
    // its panels on), and a dialog's body.
    grounds: &["surface.base", "surface.raised", "surface.overlay"],
    keeps_size: &[],
    sized_apart: &[],
    // As tall as what it holds: a door's column, a pick's line.
    height: None,
    font_size: "font-size.md",
    line_height: 1.4,
    border_width: 1.0,
    defaults: card_button_look,
    // What its box marks, as React Aria says it of its button: under the pointer, pressed, the
    // keyboard's focus and unavailable. It is never chosen.
    states: &[
        State::Hover,
        State::Pressed,
        State::Focused,
        State::Disabled,
    ],
    focused: "[data-focus-visible]",
    // A door's mark in the accent, and its line in the secondary tier.
    words: &[("mark", "accent.base"), ("line", "text.secondary")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **A pressed card's colours as `card.css` draws them.** A door stands on the control's ground
/// behind the subtle hairline, in the primary words; under the pointer and pressed, the control's
/// hover behind the strong line. A pick has no box of its own: the accent's words, on the hover
/// surface under the pointer and pressed. Unavailable, either is faint, on its own ground and
/// edge, and is never under the pointer or pressed.
pub(super) fn card_button_look(variant: &str, states: &[State]) -> Look {
    let disabled = states.contains(&State::Disabled);
    let lit = !disabled && (states.contains(&State::Hover) || states.contains(&State::Pressed));
    let (fill, text, border) = match (variant, lit) {
        ("pick", true) => ("surface.hover", "accent.base", TRANSPARENT),
        ("pick", false) => (TRANSPARENT, "accent.base", TRANSPARENT),
        (_, true) => ("control.hover", "text.primary", "border.strong"),
        (_, false) => ("control.base", "text.primary", "border.subtle"),
    };
    Look {
        fill,
        text: if disabled { "text.faint" } else { text },
        border,
    }
}
