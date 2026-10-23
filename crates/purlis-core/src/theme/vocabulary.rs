// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a theme may say**: the token names, the motion names and their grammar, and the core
//! geometry's custom properties, which a theme reaches through its geometry tokens and fonts
//! (`theme::geometry`, `theme::fonts`, ADR 0097).
//!
//! These lists are the contract a theme author writes against, so they are written once, here.
//! The window still names the same tokens to write them to the document (`app/src/theme/theme.ts`
//! `TOKENS`, `motion.ts`, `geometry.ts`), and `app/src/theme/core.test.ts` reads each list out of
//! this file and fails when the two differ.

/// Every colour token, in the order a theme file is easiest to read in. The window's `TOKENS`
/// says what each one paints.
pub const TOKENS: &[&str] = &[
    "surface.base",
    "surface.sunken",
    "surface.deep",
    "surface.raised",
    "surface.overlay",
    "surface.hover",
    "control.base",
    "control.hover",
    "control.aimed",
    "control.count",
    "text.primary",
    "text.secondary",
    "text.muted",
    "text.quiet",
    "text.faint",
    "border.subtle",
    "border.strong",
    "accent.base",
    "accent.surface",
    "focus.ring",
    "tab.active",
    "list.selected",
    "list.selected-edge",
    "tree.guide",
    "layer.project",
    "layer.workspace",
    "layer.chat",
    "layer.selected",
    "layer.divider",
    "needs-you.base",
    "needs-you.text",
    "danger.base",
    "danger.surface",
    "danger.text",
    "danger.wash",
    "wash.warning",
    "diff.deleted",
    "diff.inserted",
    "diff.deleted-text",
    "diff.inserted-text",
    "icon.folder",
    "icon.motive",
    "icon.grey",
    "icon.red",
    "icon.orange",
    "icon.yellow",
    "icon.green",
    "icon.teal",
    "icon.blue",
    "icon.purple",
    "icon.pink",
    "state.running",
    "state.waiting",
    "state.waiting-glow",
    "state.failed",
    "state.success",
    "state.unreadable",
    "overlay.scrim",
    "overlay.shadow",
    "terminal.background",
    "terminal.foreground",
    "terminal.cursor",
    "terminal.cursor-accent",
    "terminal.selection",
    "terminal.find-match",
    "terminal.find-match-active",
    "terminal.selection-foreground",
    "terminal.selection-inactive",
    "terminal.scrollbar",
    "terminal.scrollbar-hover",
    "terminal.scrollbar-active",
    "terminal.ansi.black",
    "terminal.ansi.red",
    "terminal.ansi.green",
    "terminal.ansi.yellow",
    "terminal.ansi.blue",
    "terminal.ansi.magenta",
    "terminal.ansi.cyan",
    "terminal.ansi.white",
    "terminal.ansi.bright-black",
    "terminal.ansi.bright-red",
    "terminal.ansi.bright-green",
    "terminal.ansi.bright-yellow",
    "terminal.ansi.bright-blue",
    "terminal.ansi.bright-magenta",
    "terminal.ansi.bright-cyan",
    "terminal.ansi.bright-white",
    "syntax.keyword",
    "syntax.string",
    "syntax.number",
    "syntax.constant",
    "syntax.comment",
    "syntax.function",
    "syntax.type",
    "syntax.property",
    "syntax.attribute",
    "syntax.tag",
    "syntax.heading",
    "syntax.invalid",
    "syntax.variable",
    "syntax.operator",
    "syntax.punctuation",
    "markdown.link",
    "markdown.code",
    "markdown.code-block",
    "markdown.quote",
    "markdown.heading",
    "selection.background",
    "selection.text",
    "caret",
    "placeholder",
    "scrollbar.thumb",
    "scrollbar.track",
    "chart.1",
    "chart.2",
    "chart.3",
    "chart.4",
    "chart.5",
    "chart.6",
    "chart.7",
    "chart.8",
    "window.background",
];

/// How long each kind of change takes, in milliseconds (`motion.ts` says what each is for).
pub const DURATIONS: &[&str] = &[
    "duration.quick",
    "duration.enter",
    "duration.settle",
    "duration.spin",
    "duration.breathe",
];

/// How each kind of change moves, as the four numbers of a cubic Bézier.
pub const EASINGS: &[&str] = &[
    "easing.standard",
    "easing.enter",
    "easing.steady",
    "easing.breathe",
];

/// The longest a duration may be, in milliseconds. `0` is allowed, and is how a theme turns
/// motion off.
pub const LONGEST: u32 = 10_000;

/// **The tokens a workspace's colour turns when a theme says nothing about it** (charter-app#281):
/// the accent, the focus ring, the active-tab wash, the selected row, what is drawn in the accent
/// (a markdown link, a chart's first series, #2327), and the layer shades. Never the text and
/// never the terminal, so every contrast a theme clears still holds. Purlis Classic's recipe,
/// and the one a theme starts from ([`super::Tint`]).
pub const TINTED: &[&str] = &[
    "accent.base",
    "accent.surface",
    "focus.ring",
    "tab.active",
    "list.selected",
    "list.selected-edge",
    "markdown.link",
    "chart.1",
    "layer.workspace",
    "layer.chat",
    "layer.selected",
];

/// **Every token a theme may have a workspace's colour tint** (#2329): the window's surfaces,
/// controls, words, rules, the accent and its kin, the selected row, what is drawn in the accent
/// (`markdown.link`, `chart.1`), the tree's guides and the workspace's own strip shades. A tint
/// keeps each colour's luminance, so a pair these draw keeps its contrast at every hue.
///
/// The rest are never a tint's: the consent and state colours (`needs-you.*`, `danger.*`,
/// `state.*`) are purlis's to hold (ADR 0097 §6); the diff washes, the icon colours, the syntax
/// roles and the other markdown and chart colours carry their meaning in their hue; the terminal
/// is handed its colours once, as an object, and a workspace switch does not redraw it; the
/// selection, caret, placeholder and scrollbar may be left to the platform; the window's
/// background is drawn before any workspace is; the project strip (`layer.project`) is not a
/// workspace's; and the overlay's scrim and shadow (`overlay.*`) lie over every workspace at
/// once.
pub const TINTABLE: &[&str] = &[
    "surface.base",
    "surface.sunken",
    "surface.deep",
    "surface.raised",
    "surface.overlay",
    "surface.hover",
    "control.base",
    "control.hover",
    "control.aimed",
    "control.count",
    "text.primary",
    "text.secondary",
    "text.muted",
    "text.quiet",
    "text.faint",
    "border.subtle",
    "border.strong",
    "accent.base",
    "accent.surface",
    "focus.ring",
    "tab.active",
    "list.selected",
    "list.selected-edge",
    "markdown.link",
    "chart.1",
    "tree.guide",
    "layer.workspace",
    "layer.chat",
    "layer.selected",
    "layer.divider",
];

/// **The tokens a tint draws only on a workspace's own tab and its chat strip**, never on the
/// whole window: the strip shades, which every workspace's tab draws in its own colour whether
/// or not it is in front (ADR 0036).
pub const TAB_ONLY: &[&str] = &[
    "layer.workspace",
    "layer.chat",
    "layer.selected",
    "layer.divider",
];

/// Every core geometry custom property, each written by the engine from a theme's numbers
/// (`theme::geometry::properties`) or fonts (`theme::Fonts`), never from a file's text. The
/// window's `GEOMETRY_PROPERTIES`, whose groups say what each one sizes.
pub const GEOMETRY: &[&str] = &[
    "--space-0",
    "--space-1",
    "--space-2",
    "--space-3",
    "--space-4",
    "--space-5",
    "--space-6",
    "--space-7",
    "--space-8",
    "--font-size-2xs",
    "--font-size-xs",
    "--font-size-sm",
    "--font-size-md",
    "--font-size-base",
    "--font-size-lg",
    "--font-size-xl",
    "--font-weight-regular",
    "--font-weight-medium",
    "--font-weight-strong",
    "--line-height-tight",
    "--line-height-body",
    "--font-ui",
    "--font-mono",
    "--radius-sm",
    "--radius-md",
    "--radius-lg",
    "--radius-xl",
    "--radius-full",
    "--size-control",
    "--size-chrome",
    "--window-gutter",
    "--panel-corner-radius",
    "--panel-border-width",
    "--focus-ring-width",
    "--focus-ring-offset",
    "--focus-ring-inset",
    "--pane-border-width",
    "--elevation-raised",
    "--elevation-overlay",
];

/// The first word of a key that, on its own, says the key is about geometry.
const GEOMETRY_GROUPS: &[&str] = &[
    "geometry",
    "space",
    "spacing",
    "size",
    "sizes",
    "radius",
    "radii",
    "elevation",
    "density",
    "font",
    "type",
    "typography",
    "line",
    "leading",
    "weight",
];

/// **The views a theme's icon set may draw** (#2337): each view's tab on the rails, by the
/// window's own id (`OwnViewId` in `app/src/sideViews.ts`, in `VIEW_MARKS`' order).
pub const RAIL_VIEWS: &[&str] = &[
    "chats", "explorer", "search", "changes", "repos", "inbox", "todos", "memory", "personas",
    "sessions", "vaults",
];

/// **The quiet tiers a theme leaves out follow the muted it names** (#2902, ADR 0097's
/// amendment of 2026-10-21). `given` is what one place gives (an appearance, a layer of an
/// option, a format 1 file) and `below` the values it is laid over (its parent, the appearance
/// an option is drawn on, or Purlis Classic's). A tier `below` draws in the grey of the tier
/// above it is following that tier, as Purlis Classic's quiet and faint follow its muted:
///
/// - `text.quiet` not given takes the `text.muted` given, while `below`'s quiet is its muted;
/// - `text.faint` not given takes the `text.quiet` given (or just followed), while `below`'s
///   faint is its quiet.
///
/// So a theme written before the tiers draws its quiet words and its marks in its own muted,
/// as legibly as before, and a theme that changes only its muted over a parent with tiers of
/// its own (Purlis's) keeps the parent's quiet and faint: it holds only its differences.
pub fn follow_muted(
    given: &mut std::collections::BTreeMap<&'static str, String>,
    below: &std::collections::BTreeMap<&'static str, String>,
) {
    let under = |token: &str| below.get(token).map(String::as_str);
    let follows = |tier: &str, above: &str| under(tier) == under(above);
    if let Some(muted) = given.get("text.muted").cloned()
        && !given.contains_key("text.quiet")
        && follows("text.quiet", "text.muted")
    {
        given.insert("text.quiet", muted);
    }
    if let Some(quiet) = given.get("text.quiet").cloned()
        && !given.contains_key("text.faint")
        && follows("text.faint", "text.quiet")
    {
        given.insert("text.faint", quiet);
    }
}

/// Whether `token` is one of [`TOKENS`].
pub fn is_token(token: &str) -> bool {
    TOKENS.contains(&token)
}

/// Whether a value is a colour a theme may hold: `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`, and
/// nothing else. A value is written straight into a CSS declaration and into xterm's object, so
/// the grammar is closed rather than escaped: nothing that could end a declaration is a colour.
pub fn is_colour(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('#') else {
        return false;
    };
    matches!(digits.len(), 3 | 4 | 6 | 8) && digits.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Whether a key a theme file wrote names geometry: a property (`--space-4`), its dotted form
/// the way a token is spelled (`space.4`, `radius.md`), or a whole group (`geometry`,
/// `spacing`). Format 1 has no geometry, and a format 1 file that names some is told where it
/// goes, rather than only that it was not read.
pub fn names_geometry(key: &str) -> bool {
    let dashed = if key.starts_with("--") {
        key.to_owned()
    } else {
        format!("--{}", key.replace('.', "-"))
    };
    if GEOMETRY.contains(&dashed.as_str()) {
        return true;
    }
    let bare = key.strip_prefix("--").unwrap_or(key);
    let group = bare.split(['.', '-']).next().unwrap_or_default();
    GEOMETRY_GROUPS.contains(&group.to_lowercase().as_str())
}

/// **The tokens a theme gives as a flag**, `true` or `false`, not a colour (#2327): whether each
/// syntax role is drawn italic and bold, whether the terminal's cursor blinks, and whether the
/// terminal draws bold text in the bright ANSI colours.
pub const FLAGS: &[&str] = &[
    "syntax.keyword.italic",
    "syntax.keyword.bold",
    "syntax.string.italic",
    "syntax.string.bold",
    "syntax.number.italic",
    "syntax.number.bold",
    "syntax.constant.italic",
    "syntax.constant.bold",
    "syntax.comment.italic",
    "syntax.comment.bold",
    "syntax.function.italic",
    "syntax.function.bold",
    "syntax.type.italic",
    "syntax.type.bold",
    "syntax.property.italic",
    "syntax.property.bold",
    "syntax.attribute.italic",
    "syntax.attribute.bold",
    "syntax.tag.italic",
    "syntax.tag.bold",
    "syntax.heading.italic",
    "syntax.heading.bold",
    "syntax.invalid.italic",
    "syntax.invalid.bold",
    "syntax.variable.italic",
    "syntax.variable.bold",
    "syntax.operator.italic",
    "syntax.operator.bold",
    "syntax.punctuation.italic",
    "syntax.punctuation.bold",
    "terminal.cursor-blink",
    "terminal.bold-as-bright",
];

/// The token that says the terminal cursor's shape, one of [`CURSOR_STYLES`].
pub const CURSOR_STYLE: &str = "terminal.cursor-style";

/// The terminal cursor's shapes, as xterm names them.
pub const CURSOR_STYLES: &[&str] = &["block", "underline", "bar"];

/// The token that says the terminal's line height, a multiple of its text size within
/// [`LINE_HEIGHTS`].
pub const LINE_HEIGHT: &str = "terminal.line-height";

/// The least and the most [`LINE_HEIGHT`] may be: xterm draws nothing under 1.
pub const LINE_HEIGHTS: (f64, f64) = (1.0, 2.0);

/// **The colour tokens the platform draws unless a theme says** (#2327): the window's text
/// selection and the words in it, a field's caret and placeholder, the scrollbars' thumb, and the
/// terminal's colour for selected text (each cell's own). One of these may be [`PLATFORM`]
/// instead of a colour: purlis then writes nothing for it, and what draws it draws its default,
/// as Purlis Classic does. A colour, transparent included, is drawn as given. A scrollbar's track
/// is drawn only with a thumb, so it is not here.
pub const PLATFORM_DRAWN: &[&str] = &[
    "terminal.selection-foreground",
    "selection.background",
    "selection.text",
    "caret",
    "placeholder",
    "scrollbar.thumb",
];

/// The value that leaves a token of [`PLATFORM_DRAWN`] to the platform. A closed word, never CSS;
/// no palette may take it as a name.
pub const PLATFORM: &str = "platform";

/// Whether `token` holds [`PLATFORM`]: one of [`PLATFORM_DRAWN`] left to the platform.
pub fn is_platform(token: &str, value: &str) -> bool {
    value == PLATFORM && PLATFORM_DRAWN.contains(&token)
}
