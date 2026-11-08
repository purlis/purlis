// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The grip family** (#5290, spec #5288): what a person picks a pane or a chat up by, declared
//! once here and once in its wrapper, `app/src/ui/grip/Grip.tsx`, whose stylesheet (`grip.css`)
//! draws the defaults [`grip_fill`] holds a copy of.
//!
//! It stands in a chat mark's `who` (`ui/chat-mark`), or a view's mark in a pane's head: at rest
//! it is that mark, and under the pointer or the keyboard it is six dots, three rows of two, in
//! the mark's place. **It has no button ground**: its root takes no fill, border or shadow in any
//! state, so it reads as a handle and not an action, and a theme cannot give it one (the root
//! has no properties). Its dots are its one themable part.
//!
//! **Drawn in marks, not words** ([`super::MARKED`]): its dots are held at 3:1 on every ground it
//! stands on (WCAG 1.4.11). **Its states are its own** ([`OWN_STATES`]): a grip in a chat's row
//! is no control of its own (the row is), so the keyboard on the row shows its dots as the
//! keyboard on a head's grip does.

use super::{Family, Look, Part, Prop, State, TRANSPARENT};

/// **The grip** (#5290): a persona's mark that turns into dots to be picked up by.
pub(super) const GRIP: Family = Family {
    name: "grip",
    description: "What a pane or a chat is picked up by: its mark at rest, six dots under the \
                  pointer or the keyboard, with no button ground.",
    since: 14,
    parts: &[
        Part {
            name: "root",
            since: 14,
            description: "The grip: as big as the mark it stands for, with no ground of its own.",
            properties: &[],
        },
        Part {
            name: "mark",
            since: 14,
            description: "The mark it stands for, drawn at rest.",
            properties: &[],
        },
        Part {
            name: "dots",
            since: 14,
            description: "The dots, in three rows of two, in the mark's place.",
            properties: &[],
        },
        Part {
            name: "dot",
            since: 14,
            description: "Each of its six dots, drawn under the pointer and with the keyboard.",
            properties: &[Prop::Fill],
        },
    ],
    variants: &[],
    // A row at rest, hovered and chosen, and a pane's head (the chat mark's grounds).
    grounds: &[
        "surface.deep",
        "surface.base",
        "surface.raised",
        "surface.hover",
        "list.selected",
        "terminal.background",
    ],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 0.0,
    defaults: grip_look,
    // Under the pointer, and with the keyboard on it or on the row it stands in (`OWN_STATES`).
    states: &[State::Hover, State::Focused],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **What the grip is keyed on in each state** (#5290): the pointer on it, and the keyboard on it
/// or on what holds it (a chat's row, which is the control the grip stands in).
pub(super) const OWN_STATES: &[(State, &str)] = &[
    (State::Hover, ":hover"),
    (State::Focused, ":is(:focus-visible, :focus-visible *)"),
];

/// The one part with a fill: each dot, after the root, the mark and the dots.
const DOT: usize = 3;

/// **The grip's colours**: no fill and no edge in any state, and no words.
pub(super) fn grip_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: TRANSPARENT,
    }
}

/// **Each part's fill as `grip.css` draws it while `states` hold** (#5290): only a dot ([`DOT`]) has
/// one, none at rest, where the mark is drawn, and the muted tier under the pointer and with the
/// keyboard, a mark held at 3:1 on every ground.
pub(super) fn grip_fill(part: usize, states: &[State]) -> &'static str {
    if part != DOT || states.is_empty() {
        TRANSPARENT
    } else {
        "text.muted"
    }
}
