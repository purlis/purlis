// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window's own families** (#2913, spec #2897): the region panel, a panel's section, an
//! empty state, a status item, the project rail (#2915) and the title bar's pinned workspaces
//! (#2916), each declared once here and once in its wrapper under
//! `app/src/ui/`, whose stylesheet draws the defaults [`super::Look`] holds a copy of.
//!
//! A panel, a section and an empty state are boxes as tall as what they hold (`height: None`):
//! no target of their own, so [`super::fits`] holds only their bounds, and each of their corners is
//! clamped to half their own box. A status item is a control, drawn 24px tall in either density
//! (R115), and holds the least target like a button; a rail button is 36px (R77).

use super::{ALL, Family, Length, Look, Part, Prop, State, TRANSPARENT, Variant};

/// The properties a box (a panel, a section's root, an empty state's root) may set: its fill,
/// edge, corners and shadow, and the room inside it.
const PANEL_ROOT: &[Prop] = &[
    Prop::Fill,
    Prop::BorderColour,
    Prop::BorderWidth,
    Prop::CornerRadius,
    Prop::Shadow,
];

/// What a box that only lays out what it holds may set: its room and its gaps.
const ROOM: &[Prop] = &[Prop::Padding, Prop::Gap];

/// A part's words: their colour, size and weight.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

/// **A region panel** (#2805, #2913): a side panel, the content, a half of the bottom panel. The
/// window's gutter and the panels' corners are the theme's `window.gutter` and
/// `panel.corner-radius`; a part sets one variant's on top.
pub(super) const PANEL: Family = Family {
    name: "panel",
    description: "A region of the window: a side panel, the content, a half of the bottom panel.",
    since: 4,
    parts: &[
        Part {
            name: "root",
            since: 4,
            description: "The panel's surface: its fill, edge, corners and shadow.",
            properties: PANEL_ROOT,
        },
        Part {
            name: "title",
            since: super::NEXT,
            description: "The title of the view it shows, on its head: the view's one heading.",
            properties: WORDS,
        },
    ],
    variants: &[
        Variant {
            name: "side",
            since: 4,
            description: "A side panel, left or right.",
            consent: None,
        },
        Variant {
            name: "content",
            since: 4,
            description: "The content: the chats and the views open in the middle.",
            consent: None,
        },
        Variant {
            name: "bottom",
            since: 4,
            description: "A half of the bottom panel.",
            consent: None,
        },
    ],
    // Drawn on the window's frame, which its gutter shows.
    grounds: &["surface.deep"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.4,
    border_width: 0.0,
    defaults: panel_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **The panel's colours as `app/src/ui/panel/panel.css` draws them**: the base surface, the
/// window's words, the subtle hairline.
fn panel_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: "surface.base",
        text: "text.primary",
        border: "border.subtle",
    }
}

/// **A section of a panel**: its heading line and what it holds under it. Its title is the
/// section's name in the label's capitals; the line between two sections is a separator, not a
/// part of it.
pub(super) const PANEL_SECTION: Family = Family {
    name: "panel-section",
    description: "A section of a panel: its heading line, its title and what it holds.",
    since: 4,
    parts: &[
        Part {
            name: "root",
            since: 4,
            description: "The section: the room around what it holds.",
            properties: ROOM,
        },
        Part {
            name: "head",
            since: 4,
            description: "Its heading line: the title and the section's actions.",
            properties: ROOM,
        },
        Part {
            name: "title",
            since: 4,
            description: "Its title.",
            properties: WORDS,
        },
        Part {
            name: "top",
            since: super::NEXT,
            description: "A view's top, held in place while its rows scroll: its heading line \
                          where it draws one, then its field, then its tabs and chips, on the \
                          panel's fill. A section with a top is a view.",
            properties: ROOM,
        },
    ],
    variants: &[Variant {
        name: "default",
        since: 4,
        description: "Every section.",
        consent: None,
    }],
    // The base ground, and the raised one a theme puts its panels on (Purlis, #2913).
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.xs",
    line_height: 1.4,
    border_width: 0.0,
    defaults: section_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **A section's colours as `panel.css` draws them**: no ground of its own, its title in the
/// secondary tier.
fn section_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.secondary",
        border: TRANSPARENT,
    }
}

/// **An empty state**: what a surface says when there is nothing in it, and the way out.
pub(super) const EMPTY_STATE: Family = Family {
    name: "empty-state",
    description: "What a surface says when it holds nothing, and what to do about it.",
    since: 4,
    parts: &[
        Part {
            name: "root",
            since: 4,
            description: "The empty state: the room around it and between its lines.",
            properties: ROOM,
        },
        Part {
            name: "icon",
            since: 4,
            description: "The mark above it.",
            properties: &[Prop::Text],
        },
        Part {
            name: "headline",
            since: 4,
            description: "Its one line: what is true.",
            properties: WORDS,
        },
        Part {
            name: "body",
            since: 4,
            description: "The sentence under it: what goes here.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "action",
            since: 4,
            description: "Its way out: the buttons under it, and the room between them.",
            properties: &[Prop::Gap],
        },
    ],
    variants: &[
        Variant {
            name: "page",
            since: 4,
            description: "In the middle of the content.",
            consent: None,
        },
        Variant {
            name: "panel",
            since: 4,
            description: "At a side panel's width.",
            consent: None,
        },
    ],
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.md",
    line_height: 1.4,
    border_width: 0.0,
    defaults: empty_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **An empty state's colours as `empty-state.css` draws them**: no ground, its sentence and its
/// mark in the muted tier (its headline a tier up, which the floors measure as the lower).
fn empty_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.muted",
        border: TRANSPARENT,
    }
}

/// **A status item** (N28, R115): a cell of the status bar, or a pill at the title bar's right.
/// What can be pressed runs a command; what cannot is its words. 24px tall in either density.
pub(super) const STATUS_ITEM: Family = Family {
    name: "status-item",
    description: "A status bar item, or a status pill at the title bar's right.",
    since: 4,
    parts: &[
        Part {
            name: "root",
            since: 4,
            description: "The item itself: its fill, border, corners and size.",
            properties: ALL,
        },
        Part {
            name: "icon",
            since: 4,
            description: "Its icon.",
            properties: &[Prop::Text],
        },
        Part {
            name: "label",
            since: 4,
            description: "Its words.",
            properties: WORDS,
        },
        Part {
            name: "count",
            since: 4,
            description: "Its count.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[
        Variant {
            name: "bar",
            since: 4,
            description: "A cell of the status bar: no box until the pointer reaches it.",
            consent: None,
        },
        Variant {
            name: "pill",
            since: 4,
            description: "A status pill at the title bar's right: full corners, quiet words.",
            consent: None,
        },
        Variant {
            name: "waiting",
            since: 4,
            description: "The pill that says what waits on the person.",
            consent: None,
        },
        Variant {
            name: "accent",
            since: 4,
            description: "The pill that is news: an update on offer.",
            consent: None,
        },
    ],
    grounds: &["surface.deep"],
    keeps_size: &[],
    sized_apart: &[],
    height: Some(Length::Number(24.0)),
    font_size: "font-size.sm",
    line_height: 1.0,
    border_width: 0.0,
    defaults: status_look,
    // What its wrapper marks: under the pointer, pressed, the keyboard's focus, unavailable.
    states: &[
        State::Hover,
        State::Pressed,
        State::Focused,
        State::Disabled,
    ],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **A status item's colours as `app/src/ui/status/status.css` draws them**, at rest and under
/// the pointer.
fn status_look(variant: &str, states: &[State]) -> Look {
    let mut look = match variant {
        "pill" => Look {
            fill: TRANSPARENT,
            text: "text.muted",
            border: TRANSPARENT,
        },
        "accent" => Look {
            fill: "accent.surface",
            text: "accent.text",
            border: TRANSPARENT,
        },
        "waiting" => Look {
            fill: "wash.warning",
            text: "text.primary",
            border: TRANSPARENT,
        },
        _ => Look {
            fill: TRANSPARENT,
            text: "text.primary",
            border: TRANSPARENT,
        },
    };
    // Under the pointer, whatever else holds at once (pressed, the keyboard's focus): a status
    // item draws no look of its own for those.
    if states.contains(&State::Hover) {
        look.fill = if variant == "accent" {
            "list.selected"
        } else {
            "surface.hover"
        };
    }
    look
}

/// **The project rail** (#2915, R76–R89, N17): a button per project at the window's far left,
/// then open a project and the rail's own hide. A button is 36px with corners of 9 (R77); the
/// rail's ground is the window's frame, which the current project's ring keeps a gap of (R78).
///
/// The family's root is the button (`rail.button.*`, R139): the column around the buttons lays
/// them out and is no part a theme keys.
pub(super) const RAIL: Family = Family {
    name: "rail",
    description: "The project rail at the window's left: a button per project, open and hide.",
    since: 9,
    parts: &[
        Part {
            name: "button",
            since: 9,
            description: "A button of the rail: its fill, edge, corners and size.",
            properties: ALL,
        },
        Part {
            name: "monogram",
            since: 9,
            description: "A project's two letters, in the project's own colour.",
            properties: &[Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "badge",
            since: 9,
            description: "How many things wait on the person in the project, at its corner.",
            properties: &[Prop::Fill, Prop::Text],
        },
        Part {
            name: "unsaved",
            since: 9,
            description: "The dot of the project's unsaved work, at its other corner.",
            properties: &[Prop::Fill],
        },
    ],
    variants: &[
        Variant {
            name: "project",
            since: 9,
            description: "A project: its monogram on its own colour, ringed while it is current.",
            consent: None,
        },
        Variant {
            name: "action",
            since: 9,
            description: "Open a project, and hide or keep the rail: an icon, no box at rest.",
            consent: None,
        },
    ],
    // On the window's frame, in its column or peeking over the content.
    grounds: &["surface.deep"],
    keeps_size: &[],
    sized_apart: &[],
    height: Some(Length::Number(36.0)),
    font_size: "font-size.xs",
    line_height: 1.0,
    border_width: 0.0,
    defaults: rail_look,
    // What its wrapper marks: under the pointer, the current project, the keyboard's focus.
    states: &[State::Hover, State::Selected, State::Focused],
    focused: "[data-focus-visible]",
    words: &[("badge", "state.waiting")],
    corner_most: None,
    around: &[],
    // The badge's pill, the overlay's surface (`rail.css`): its digits are read on it (#3041).
    fills: &[("badge", "surface.overlay")],
    under: &[],
};

/// **A rail button's colours as `app/src/ui/rail/rail.css` draws them.** A project's are its own
/// colour (the accent turned to the project's hue, keeping its luminance) on that colour at 13%:
/// measured here as the accent on the accent's surface, which is what they are at the accent's
/// own hue. An action is quiet words with no box until the pointer reaches it.
fn rail_look(variant: &str, states: &[State]) -> Look {
    if variant == "project" {
        return Look {
            fill: "accent.surface",
            text: "accent.base",
            border: TRANSPARENT,
        };
    }
    Look {
        fill: if states.contains(&State::Hover) {
            "surface.overlay"
        } else {
            TRANSPARENT
        },
        text: "text.muted",
        border: TRANSPARENT,
    }
}

/// **The title bar's pinned workspaces** (#2916, R101–R105, R140, N17): a chip per pinned
/// workspace of the project in front, after its name, and the dashed chip that pins the current
/// workspace while it is not pinned. A chip is 28px with corners of 8 and quiet words (R101); the
/// current one takes the accent's wash and the primary words, the stronger wash under the
/// pointer, while any other under the pointer takes the card's fill (R101 as amended by #3345);
/// its waiting count is the amber one (R101). The plane root's chip is its `root` variant
/// (#3654): a house drawn as a pin is, outside the strip.
///
/// The family's root is the chip (`titlebar.workspace.*`, as the prototype's part keys name it):
/// the strip around the chips lays them out and scrolls, and is no part a theme keys. The crumb,
/// the ⌄, and a switcher's rows and pin toggles are drawn apart from every chip, each a control
/// in its own states (`parts::APART`, #3656), and a notice's dot beside one (`around`).
pub(super) const TITLEBAR: Family = Family {
    name: "titlebar",
    description: "The title bar's pinned workspaces: a chip per pin, and the chip that pins the current one.",
    since: 12,
    parts: &[
        Part {
            name: "workspace",
            since: 12,
            description: "A workspace's chip: its fill, edge, corners and size.",
            properties: ALL,
        },
        Part {
            name: "count",
            since: 12,
            description: "How many things wait on the person in the workspace, after its name.",
            properties: &[Prop::Fill, Prop::Text],
        },
        Part {
            name: "crumb",
            since: 12,
            description: "The project's crumb while the rail is hidden, which opens the switcher.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius],
        },
        Part {
            name: "all",
            since: 12,
            description: "The ⌄ at the strip's end, which opens every workspace.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius],
        },
        Part {
            name: "pin",
            since: 12,
            description: "A switcher row's pin toggle: its icon, faint at rest and the accent while pinned.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius],
        },
        Part {
            name: "row",
            since: 12,
            description: "A switcher's row: a workspace or a project to go to.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius],
        },
        Part {
            name: "dot",
            since: super::NEXT,
            description: "The dot a notice about the workspace leaves on its chip's corner while \
                          its card is hidden, where the most urgent is news: its disc and its \
                          fill, ringed in the bar's ground.",
            properties: &[Prop::Fill, Prop::Text],
        },
        Part {
            name: "dot-warning",
            since: super::NEXT,
            description: "The dot on a chip's corner where the most urgent of its hidden notices \
                          is a warning: its triangle and its fill, ringed in the bar's ground.",
            properties: &[Prop::Fill, Prop::Text],
        },
        Part {
            name: "dot-needs",
            since: super::NEXT,
            description: "The dot on a chip's corner while a chat asks the person to remove the \
                          workspace, the most urgent kind: its raised hand and its fill, ringed \
                          in the bar's ground.",
            properties: &[Prop::Fill, Prop::Text],
        },
    ],
    variants: &[
        Variant {
            name: "pinned",
            since: 12,
            description: "A pinned workspace: quiet words, the accent's wash while it is current.",
            consent: None,
        },
        Variant {
            name: "unpinned",
            since: 12,
            description: "The current workspace while it is not pinned: a dashed edge, and a press pins it.",
            consent: None,
        },
        Variant {
            name: "root",
            since: super::NEXT,
            description: "The plane root's house after the project's name, outside the strip: \
                          always there, never unpinned, the accent's wash while the root is in front.",
            consent: None,
        },
        Variant {
            name: "dormant",
            since: super::NEXT,
            description: "A pin whose workspace is gone, kept dormant, after the live pins: muted \
                          slanted words and a dotted edge; a press shows its notice.",
            consent: None,
        },
    ],
    // On the title bar, which is the window's frame.
    grounds: &["surface.deep"],
    keeps_size: &[],
    sized_apart: &[],
    height: Some(Length::Number(28.0)),
    font_size: "font-size.md",
    line_height: 1.0,
    border_width: 0.0,
    defaults: titlebar_look,
    // What its wrapper marks: under the pointer, the current workspace, the keyboard's focus.
    states: &[State::Hover, State::Selected, State::Focused],
    focused: "[data-focus-visible]",
    // Each part's words as `titlebar.css` draws them: the count in amber, the crumb secondary,
    // the ⌄ quiet and a row's pin faint, marks both, and a switcher's row secondary.
    words: &[
        ("count", "state.waiting"),
        ("crumb", "text.secondary"),
        ("all", "text.quiet"),
        ("pin", "text.faint"),
        ("row", "text.secondary"),
        // A notice's dot (#3648): its shape, a mark, in the window's ground on the accent.
        ("dot", "surface.base"),
        // A warning's dot (#3649): its triangle, a mark, in the window's ground on the amber.
        ("dot-warning", "surface.base"),
        // A needs-you dot (#3652): its hand, a mark, in the window's ground on the failed red.
        ("dot-needs", "surface.base"),
    ],
    corner_most: None,
    // A chip's notice dot is the chip's sibling in its slot (#3648), drawn around the chip.
    // Each kind's dot is a part of its own (#3649), so a theme colours one kind at a time.
    around: &["dot", "dot-warning", "dot-needs"],
    // The count's pill, the amber wash (`titlebar.css`): its digits are read on it (#3041).
    fills: &[
        ("count", "wash.warning"),
        ("dot", "accent.base"),
        ("dot-warning", "state.waiting"),
        ("dot-needs", "state.failed"),
    ],
    // A switcher's row and its pin paint no ground at rest: they stand on the popover's surface
    // the switcher is drawn on, which their words are read on (#3099).
    under: &[("row", "surface.overlay"), ("pin", "surface.overlay")],
};

/// **A chip's colours as `app/src/ui/titlebar/titlebar.css` draws them.** Quiet words with no box
/// at rest; the card's fill and the primary words under the pointer; the accent's wash and the
/// primary words while current, and the stronger wash while current under the pointer, so the
/// chosen chip never looks hovered (#3345). The unpinned chip's words are muted, its edge the
/// window's subtle line, dashed, and it is lit only under the pointer. A dormant chip (#3651) is
/// drawn as the unpinned one is, its line dotted and its words slanted.
fn titlebar_look(variant: &str, states: &[State]) -> Look {
    let hovered = states.contains(&State::Hover);
    // The unpinned chip is always current, and draws no fill for it: only the pointer lights it.
    // A dormant one is never current (#3651).
    let edged = variant == "unpinned" || variant == "dormant";
    let chosen = !edged && states.contains(&State::Selected);
    let lit = hovered || chosen;
    Look {
        fill: match (chosen, hovered) {
            (true, true) => "accent.wash-hover",
            (true, false) => "accent.wash",
            (false, true) => "surface.overlay",
            (false, false) => TRANSPARENT,
        },
        text: if lit {
            "text.primary"
        } else if edged {
            "text.muted"
        } else {
            "text.quiet"
        },
        border: if edged {
            "border.subtle"
        } else {
            TRANSPARENT
        },
    }
}
