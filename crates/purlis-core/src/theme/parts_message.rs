// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The message family** (#5649, spec #5638, ADR 0117 decision 6): one message of the Inbox's
//! views, an ask, a line of Mine or Project, a Notice, drawn as a row of its view rather than a
//! boxed card, declared once here and once in its wrapper, `app/src/ui/message/Message.tsx`,
//! whose stylesheet (`message.css`) draws the defaults [`super::Look`] holds a copy of.
//!
//! A row's state is its variant, and is told by its meta's words as well as its mark's colour
//! (WCAG 1.4.1): the mark is decoration. It draws no ground of its own at rest, so its words are
//! read on the view's. **Under the pointer and holding the keyboard**, as React Aria marks them
//! on the row (`data-hovered`, `data-focus-within`), a theme may give it a fill of its own.

use super::{Family, Look, Part, Prop, State, TRANSPARENT, Variant};

/// What a row's words take: their colour, size and weight.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

/// **The message row** (#5649).
pub(super) const MESSAGE: Family = Family {
    name: "message",
    description: "A message of the Inbox's views as a row of its view: its state's mark, its \
                  title and time, what it is, what it says, its links and what answers it.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The row: its fill under the pointer or the keyboard, its words, its \
                          corners and the room around what it holds.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius, Prop::Padding],
        },
        Part {
            name: "mark",
            since: super::NEXT,
            description: "Its state's round icon: its colour, and its tint, by default its \
                          colour at a seventh, by the row's variant.",
            properties: &[Prop::Fill, Prop::Text],
        },
        Part {
            name: "main",
            since: super::NEXT,
            description: "Its column of lines, past the mark: the room between them.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "top",
            since: super::NEXT,
            description: "Its first line: its title, and its time at the right.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "title",
            since: super::NEXT,
            description: "The chat or the thing it is about.",
            properties: WORDS,
        },
        Part {
            name: "time",
            since: super::NEXT,
            description: "When it happened, or how long it has waited, which the × takes the \
                          place of under the pointer.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "meta",
            since: super::NEXT,
            description: "What it is and where, quiet under its title.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "state",
            since: super::NEXT,
            description: "What it waits on, in words, at the head of its meta: in the waiting \
                          colour on a row that waits.",
            properties: &[Prop::Text],
        },
        Part {
            name: "body",
            since: super::NEXT,
            description: "What it says.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "links",
            since: super::NEXT,
            description: "Its ways out as text links: the room between them.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "form",
            since: super::NEXT,
            description: "What answers it in the row: a reply box, an ask's answers.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "dismiss",
            since: super::NEXT,
            description: "Its ×, at the top right, over its time.",
            properties: &[],
        },
    ],
    variants: &[
        Variant {
            name: "waiting",
            since: super::NEXT,
            description: "It waits on the person: an ask, a Notice that needs a decision.",
            consent: None,
        },
        Variant {
            name: "done",
            since: super::NEXT,
            description: "Something finished.",
            consent: None,
        },
        Variant {
            name: "failed",
            since: super::NEXT,
            description: "Something failed, or needs a fix.",
            consent: None,
        },
        Variant {
            name: "report",
            since: super::NEXT,
            description: "Something came back: a report, a chat that returned.",
            consent: None,
        },
        Variant {
            name: "plain",
            since: super::NEXT,
            description: "A note, or a teammate's change.",
            consent: None,
        },
    ],
    // A side view's ground and the Panel's.
    grounds: &["surface.base", "surface.sunken"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.md",
    line_height: 1.4,
    border_width: 0.0,
    defaults: message_look,
    // Under the pointer, and holding the keyboard on itself or anything in it.
    states: &[State::Hover, State::Focused],
    focused: "[data-focus-within]",
    // Its time and its meta in the quiet tier, its mark's glyph in the secondary at rest.
    //
    // **Not held yet: `state` in the waiting colour** (#5638 review). `message.css` draws what an
    // ask waits on in `state.waiting`, as the Inbox's own rule did before it moved here, but the
    // check holds that colour at 3:1 on a view's ground, not the 4.5:1 words take
    // (`state_words_tests.rs`). Holding it there is a change to the floors every theme draft is
    // refused by, so it waits on its own ruling; until then `state` declares no words of its own.
    words: &[
        ("time", "text.quiet"),
        ("meta", "text.quiet"),
        ("mark", "text.secondary"),
    ],
    corner_most: None,
    around: &[],
    // Its mark's round, at rest: the hover surface, which each state's tint draws over.
    fills: &[("mark", "surface.hover")],
    under: &[],
};

/// **A row as `message.css` draws it**: no ground of its own at rest, the hover surface under the
/// pointer or the keyboard, its words in the primary tier, and no edge.
pub(super) fn message_look(_variant: &str, states: &[State]) -> Look {
    Look {
        fill: if states.is_empty() {
            TRANSPARENT
        } else {
            "surface.hover"
        },
        text: "text.primary",
        border: TRANSPARENT,
    }
}
