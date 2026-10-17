// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a theme may say**: the token names, the motion names and their grammar, and the names
//! a theme may not set because they are core geometry (ADR 0094).
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

/// **The tokens a workspace's colour turns** on its own tab and its chat strip (charter-app#281):
/// the accent, the focus ring, the active-tab wash, the selected row, and the layer shades. Never
/// the text and never the terminal, so every contrast a theme clears still holds. The window's
/// `TINTED_TABS`; the window's own accent tint is the first six.
pub const TINTED: &[&str] = &[
    "accent.base",
    "accent.surface",
    "focus.ring",
    "tab.active",
    "list.selected",
    "list.selected-edge",
    "layer.workspace",
    "layer.chat",
    "layer.selected",
];

/// Every core geometry custom property, which no theme may set (ADR 0094). The window's
/// `GEOMETRY_PROPERTIES`, whose groups say what each one sizes.
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
    "--font-mono",
    "--radius-sm",
    "--radius-md",
    "--radius-lg",
    "--radius-xl",
    "--radius-full",
    "--size-control",
    "--size-chrome",
    "--focus-ring-width",
    "--focus-ring-offset",
    "--focus-ring-inset",
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
/// `spacing`). Used to tell a theme author why the key was ignored, rather than only that it was.
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
