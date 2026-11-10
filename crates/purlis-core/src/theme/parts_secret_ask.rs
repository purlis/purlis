// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Secret ask's body** (#5080, spec #5235, ADR 0112): one notice for whatever a chat waits
//! on to use a secret (a key's value, a vault's access, a program's Allow), declared once here and
//! once in its wrapper, `app/src/ui/secret-ask/SecretAsk.tsx`, whose stylesheet
//! (`secret-ask.css`) draws the defaults [`super::Look`] holds a copy of.
//!
//! **Who says what is drawn apart.** The app's facts (the program's path, the hosts, the vault's
//! line, why there is no Allow) sit in the `facts` panel, which nothing of the chat's reaches. The
//! chat's words (its command, its purpose) sit outside it behind the `chat` part's quote rule,
//! each control or invisible character drawn as a `control` token. A value box takes the `value`
//! part's edge, since on an ask's card the window's control edge may be clear (#5070, #5227's
//! prototype review). Its answers are buttons of the button family; an Allow is the `grants`
//! variant. It draws no state.

use super::{Family, Look, Part, Prop, State, TRANSPARENT};

/// **The Secret ask's body** (#5080).
pub(super) const SECRET_ASK: Family = Family {
    name: "secret-ask",
    description: "The Secret ask's body: the app's facts in their own panel, the chat's words \
                  quoted apart, the value fields, the Allow and what it does, Remember, and the \
                  action bar.",
    since: 14,
    parts: &[
        Part {
            name: "root",
            since: 14,
            description: "The body: the room between what it holds.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "facts",
            since: 14,
            description: "The app's own facts, in a panel nothing of the chat's reaches: its \
                          ground, its edge and its room.",
            properties: &[
                Prop::Fill,
                Prop::BorderColour,
                Prop::BorderWidth,
                Prop::CornerRadius,
                Prop::Padding,
                Prop::Gap,
            ],
        },
        Part {
            name: "facts-head",
            since: 14,
            description: "The panel's heading, From purlis.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "who",
            since: 14,
            description: "What a line is, at its start: Program, Vault, or which chat says it.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "chat",
            since: 14,
            description: "The chat's own words, set apart by the rule at their start.",
            properties: &[Prop::BorderColour, Prop::Padding, Prop::Text],
        },
        Part {
            name: "command",
            since: 14,
            description: "The chat's command: plain, monospace and wrapped, on one flow.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "control",
            since: 14,
            description: "A control or invisible character in the chat's words, drawn as a \
                          token: ESC, LF, U+202E.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius],
        },
        Part {
            name: "value",
            since: 14,
            description: "Around a box a value or an entry is given in: the edge the box takes, \
                          visible on the card the ask is drawn on.",
            properties: &[Prop::BorderColour],
        },
        Part {
            name: "scope-said",
            since: 14,
            description: "What the Allow chosen does, under it.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "tool-line",
            since: 14,
            description: "The line Always allow writes, shown before it is pressed.",
            properties: &[
                Prop::Fill,
                Prop::Text,
                Prop::CornerRadius,
                Prop::Padding,
                Prop::FontSize,
            ],
        },
        Part {
            name: "foot",
            since: 14,
            description: "The action bar, held in view at the body's foot as it scrolls: its \
                          ground and its room. Its edge is the window's, never a theme's.",
            properties: &[Prop::Fill, Prop::Padding, Prop::Gap],
        },
    ],
    variants: &[],
    // The ask's card, raised on the Inbox's panel, and a pane's band.
    grounds: &["surface.raised", "surface.base"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.sm",
    line_height: 1.4,
    border_width: 0.0,
    defaults: secret_ask_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[
        ("facts-head", "text.secondary"),
        ("who", "text.muted"),
        ("chat", "text.secondary"),
        ("command", "text.primary"),
        ("control", "text.primary"),
        ("scope-said", "text.secondary"),
        ("tool-line", "text.primary"),
    ],
    corner_most: None,
    around: &[],
    fills: &[
        ("facts", "surface.sunken"),
        ("control", "wash.warning"),
        ("tool-line", "surface.sunken"),
        ("foot", "surface.raised"),
    ],
    under: &[],
    later: &[],
};

/// **The body's colours as `secret-ask.css` draws them**: no ground of its own (the card's
/// shows), the window's words, and the strong edge a value box takes.
pub(super) fn secret_ask_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: "border.strong",
    }
}
