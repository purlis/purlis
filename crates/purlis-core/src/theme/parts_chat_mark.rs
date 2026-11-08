// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The chat mark family** (#5340, spec #5339): a chat's mark, declared once here and once in its
//! wrapper, `app/src/ui/chat-mark/ChatMark.tsx`, whose stylesheet (`chat-mark.css`) draws each
//! state's colour from its token ([`mark_look`] says why that is not a copy here).
//!
//! The one mark on a chat's tab, its row in the list, its pane's head and a folded pane's rail.
//! Its parts are its root, who the chat runs as, the badge on the corner and the state's word,
//! which is heard and not seen. A variant is the chat's one state, which its glyph tells as well
//! as its colour (WCAG 1.4.1).

use super::{Family, Look, Part, Prop, State, TRANSPARENT, Variant};

/// **The chat mark** (#5340): who a chat runs as, with its one state on the corner.
pub(super) const CHAT_MARK: Family = Family {
    name: "chat-mark",
    description: "A chat's mark: who it runs as, with its one state on the corner, the same on \
                  its tab, its row, its pane's head and a folded pane's rail.",
    since: 14,
    parts: &[
        Part {
            name: "root",
            since: 14,
            description: "The mark: what holds who it runs as and its badge.",
            properties: &[],
        },
        Part {
            name: "who",
            since: 14,
            description: "Who the chat runs as: a persona's mark, a shell's or a task's.",
            properties: &[],
        },
        Part {
            name: "badge",
            since: 14,
            description: "The state's glyph on the mark's corner, in the state's colour.",
            properties: &[Prop::Text],
        },
        Part {
            name: "word",
            since: 14,
            description: "The state's word, heard and not seen.",
            properties: &[],
        },
    ],
    variants: &[
        Variant {
            name: "needs",
            since: 14,
            description: "It needs the person: the hand.",
            consent: None,
        },
        Variant {
            name: "failed",
            since: 14,
            description: "A task failed, until the chat has been seen: the cross.",
            consent: None,
        },
        Variant {
            name: "working",
            since: 14,
            description: "A turn runs: the ring.",
            consent: None,
        },
        Variant {
            name: "waiting",
            since: 14,
            description: "It waits on its tasks or its own work: the hourglass.",
            consent: None,
        },
        Variant {
            name: "idle",
            since: 14,
            description: "At rest, ready for the person: the pause.",
            consent: None,
        },
        Variant {
            name: "ended",
            since: 14,
            description: "Its program has ended: the dash.",
            consent: None,
        },
    ],
    // A row at rest, hovered and chosen; a tab at rest and chosen; a pane's head; a folded rail.
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
    font_size: "font-size.sm",
    line_height: 1.0,
    border_width: 0.0,
    defaults: mark_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **The mark's words at rest**: it draws none of its own but the state's, which are heard and
/// not seen. A badge's default colour is its state's token, drawn by `chat-mark.css` and held at
/// 3:1 as a mark by the token floors, so a theme recolours every mark by that token. A theme that
/// sets one state's badge here sets its text, which the engine holds as words.
pub(super) fn mark_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: TRANSPARENT,
    }
}
