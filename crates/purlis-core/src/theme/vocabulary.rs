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
    "border.control",
    "accent.base",
    "accent.surface",
    "accent.wash",
    "accent.wash-hover",
    "accent.text",
    "accent.fill",
    "accent.fill-hover",
    "accent.fill-edge",
    "accent.fill-text",
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
    "needs-you.count",
    "needs-you.count-text",
    "danger.base",
    "danger.surface",
    "danger.text",
    "danger.wash",
    "attention.surface",
    "attention.text",
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
    "duration.slide",
    "duration.reveal",
    "duration.leave",
    "duration.open",
    "duration.swap",
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
/// the accent, its surface, its washes and its words (#3100, #4666), the primary's fill at rest
/// and under the pointer (#3177), the focus ring, the active-tab wash, the selected row, what is
/// drawn in the accent (a markdown link, a chart's first series, #2327), and the layer shades.
/// Never the other text (the primary's words among it) and never the terminal, so every contrast
/// a theme clears still holds. The accent's words and washes turn together, only where they stay
/// apart from the waiting and danger colours ([`super::tint::APART`]). Purlis Classic's recipe,
/// and the one a theme starts from ([`super::Tint`]).
pub const TINTED: &[&str] = &[
    "accent.base",
    "accent.surface",
    "accent.wash",
    "accent.wash-hover",
    "accent.text",
    "accent.fill",
    "accent.fill-hover",
    "accent.fill-edge",
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
/// `attention.*`, `state.*`) are purlis's to hold (ADR 0097 §6); the diff washes, the icon colours, the syntax
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
    "border.control",
    "accent.base",
    "accent.surface",
    "accent.wash",
    "accent.wash-hover",
    "accent.text",
    "accent.fill",
    "accent.fill-hover",
    "accent.fill-edge",
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
    "--radius-control",
    "--radius-lg",
    "--radius-xl",
    "--radius-full",
    "--size-control",
    "--size-chrome",
    "--window-gutter",
    "--panel-corner-radius",
    "--panel-border-width",
    "--separator-width",
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
    "chats", "explorer", "search", "skills", "changes", "repos", "asks", "updates", "notices",
    "todos", "memory", "personas", "sessions", "vaults",
];

/// **The glyphs a theme's icon set may draw besides its views** (#2922, ruling #2866): the
/// harness glyphs purlis draws in its own stroke style, which a profile names as its icon and a
/// built-in harness is drawn with. An icon pack redraws them as it redraws a view.
pub const GLYPHS: &[&str] = &["spark", "braces", "terminal"];

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

/// **The tokens that came in after a theme could already say what they draw**, each beside the
/// token that drew it before (#3177): a plain control's edge was the strong rule's, the accent's
/// wash and the primary's fill were its surface, the accent's words were the accent, the
/// primary's fill under the pointer was the active tab's, its edge was the accent, and its words
/// were the primary words. A chosen title-bar chip's wash under the pointer (#3345) was the
/// active tab's, as the primary's fill under the pointer was, which Purlis Classic draws alike. The
/// hot count (#3098) was the needs-you mark and its words.
pub const KIN: [(&str, &str); 10] = [
    ("border.control", "border.strong"),
    ("accent.wash", "accent.surface"),
    ("accent.wash-hover", "tab.active"),
    ("accent.text", "accent.base"),
    ("accent.fill", "accent.surface"),
    ("accent.fill-hover", "tab.active"),
    ("accent.fill-edge", "accent.base"),
    ("accent.fill-text", "text.primary"),
    ("needs-you.count", "needs-you.base"),
    ("needs-you.count-text", "needs-you.text"),
];

/// **A token of [`KIN`] a theme leaves out follows the one that drew it before**, while `below`
/// draws the two alike, as [`follow_muted`] follows a tier (#3177). So a theme written over
/// Purlis Classic that colours its accent's surface still fills its primary with it, as it did
/// before the primary had a fill of its own; one over Purlis, whose filled primary is its own,
/// keeps Purlis's.
///
/// **The accent's words follow the accent whatever `below` draws** (#3100): `accent.text` is what
/// a chosen item says, in the accent, and an accent given without it must reach those words, or
/// a teal option over Purlis, or a person's teal over Purlis, says them in Purlis's lavender.
///
/// **The grounds those words stand on follow the accent too** (#4666): an accent given without
/// `accent.surface`, `accent.wash` or `accent.wash-hover` turns `below`'s to its own hue, at no
/// more chroma than it has, keeping each ground's luminance and alpha ([`super::tint::toward`]).
/// So a teal over Purlis says its words on a teal wash, as light as Purlis's lavender one, and
/// every contrast on it stays where Purlis put it; a grey accent draws grey grounds. A ground
/// that turning would bring nearer the waiting or danger wash, as drawn, than the parent's is the
/// parent's ([`super::tint::NO_NEARER`]). **So does what is drawn in the accent** (#4918,
/// [`DRAWN_IN_THE_ACCENT`]): the primary's fill, the focus ring and the selected row's edge, each
/// kept as far from waiting and danger as [`super::tint::APART`] asks, or the parent's. **A link
/// and the first chart colour** (#5157) never go back to the parent's while a colour of the
/// accent's family can hold: where neither the turned colour nor the accent keeps them
/// [`super::floors::SIGNAL_APART`] from the state colours and at their contrast floors, they take
/// the nearest that does ([`super::tint::nudged`]).
///
/// What follows, each beside what it follows, so a colour left out takes them with it.
pub fn follow_kin(
    given: &mut std::collections::BTreeMap<&'static str, String>,
    below: &std::collections::BTreeMap<&'static str, String>,
) -> Vec<(&'static str, &'static str)> {
    let mut followed = Vec::new();
    for (token, drew) in KIN {
        let follows = below.get(token) == below.get(drew) || token == ACCENT_WORDS;
        if given.contains_key(token) || !follows {
            continue;
        }
        if let Some(value) = given.get(drew).cloned() {
            given.insert(token, value);
            followed.push((token, drew));
        }
    }
    let accent = given
        .get(ACCENT)
        .filter(|accent| below.get(ACCENT) != Some(*accent))
        .cloned();
    if let Some(accent) = accent {
        for ground in ACCENT_GROUNDS {
            if given.contains_key(ground) {
                continue;
            }
            let Some(under) = below.get(ground) else {
                continue;
            };
            let turned = super::tint::toward(under, &accent);
            // Never nearer the waiting or danger wash, as drawn, than the parent's ground.
            let drawn = |token: &'static str| {
                given
                    .get(token)
                    .or_else(|| below.get(token))
                    .cloned()
                    .unwrap_or_default()
            };
            if super::tint::no_nearer(&turned, under, drawn) {
                given.insert(ground, turned);
                followed.push((ground, ACCENT));
            }
        }
        // And the accent's marks, a link and the first chart colour (#5157).
        for token in DRAWN_IN_THE_ACCENT
            .into_iter()
            .chain(super::tint::SIGNAL_MARKS)
        {
            if given.contains_key(token) {
                continue;
            }
            let Some(under) = below.get(token) else {
                continue;
            };
            // A token `below` draws as the one that drew it before follows that one, as it always
            // did (Classic's primary fill is its surface), not the accent: as that one is now
            // drawn, turned to the accent or not (#5157). The fill under the pointer is drawn as
            // the active tab, which no accent turns, so it turns with the fill instead, keeping
            // the fill's hue, as the wash under the pointer keeps the accent's.
            let drawn_as_kin = KIN
                .iter()
                .find(|(kin, drew)| *kin == token && below.get(kin) == below.get(drew));
            let mut turning_to = accent.as_str();
            if let Some((_, drew)) = drawn_as_kin {
                if let Some(value) = given.get(drew).cloned() {
                    given.insert(token, value);
                    followed.push((token, *drew));
                    continue;
                }
                match given.get("accent.fill") {
                    Some(fill) if token == FILL_HOVER => turning_to = fill,
                    _ => continue,
                }
            }
            // One `below` draws as its accent is the accent given (#5157); one drawn in the
            // accent's family turns to its hue. A mark that would read as a state there takes the
            // accent's own colour, since a link reads as the accent (#3049), as a tinted one does
            // ([`super::tint::signal_chroma_of`]). Either way a colour that would lose a contrast
            // is passed over for the next: the turned one keeps the parent's luminance, and so
            // its contrasts. An edge `below` draws as its ring tries the ring as it is drawn
            // first, since a theme can tune its accent and its ring apart.
            let turned = super::tint::toward(under, turning_to);
            let mark = super::tint::SIGNAL_MARKS.contains(&token);
            let alike = below.get(RING) == below.get(EDGE);
            let the_ring = (token == EDGE && alike)
                .then(|| given.get(RING).cloned())
                .flatten();
            let tries: Vec<String> = the_ring
                .iter()
                .cloned()
                .chain(if below.get(ACCENT) == Some(under) {
                    vec![accent.clone(), turned]
                } else if mark {
                    vec![turned, accent.clone()]
                } else {
                    vec![turned]
                })
                .collect();
            let is = |token: &str| {
                given
                    .get(token)
                    .or_else(|| below.get(token))
                    .map_or("", String::as_str)
            };
            let was = |token: &str| below.get(token).map_or("", String::as_str);
            // A ring whose edge is still to come holds the edge's pairs too, so the two take one
            // colour: the edge has a pair the ring has not, the palette's aimed row.
            let edge_to_come = token == RING && alike && !given.contains_key(EDGE);
            let apart = HeldApart::of(token, under, |token| Some(is(token).to_owned()));
            let contrast = super::check::KeepsContrast::of(token, &was);
            let edge = super::check::KeepsContrast::of(EDGE, &was);
            let keeps = |one: &str| {
                apart.holds(one)
                    && contrast.keeps(one, &is)
                    && (!edge_to_come || edge.keeps(one, &is))
                    && (token != FIRST_SERIES || apart_from_the_series(one, &is, &was))
            };
            let holding = tries.iter().find(|one| keeps(one)).cloned().or_else(|| {
                // The person's ruling (#5157): a mark too near a state stays in the accent's
                // family, nudged just far enough, rather than going back to the parent's colour.
                mark.then(|| super::tint::nudged(&accent, &keeps)).flatten()
            });
            if let Some(holding) = holding {
                let from = if the_ring.as_ref() == Some(&holding) {
                    RING
                } else {
                    ACCENT
                };
                given.insert(token, holding);
                followed.push((token, from));
            }
        }
    }
    followed
}

/// **Whether `colour`, drawn as the first chart colour, stands apart from every other series**,
/// at least [`SERIES_APART`], or no nearer than the parent's was (#5157).
fn apart_from_the_series<'v>(
    colour: &str,
    is: &dyn Fn(&str) -> &'v str,
    was: &dyn Fn(&str) -> &'v str,
) -> bool {
    use super::tint::{distance, oklab_of};
    OTHER_SERIES.iter().all(|series| {
        let Some(other) = oklab_of(is(series)) else {
            return true;
        };
        let Some(now) = oklab_of(colour) else {
            return true;
        };
        let parents = oklab_of(was(FIRST_SERIES))
            .zip(oklab_of(was(series)))
            .map_or(SERIES_APART, |(one, two)| distance(one, two));
        distance(now, other) >= SERIES_APART.min(parents)
    })
}

/// **What every rule of [`super::tint::APART`] that `token` is the token of holds it to** (#4918):
/// at least the rule's distance from its other side as `value` gives it, or no nearer than
/// `written`, the value it is turned from, already was: a mark of the accent
/// ([`super::tint::SIGNAL_MARKS`]) so from each state colour too (#5157). A rule of the accent's
/// words is measured by its own answer, and a value that is not a colour has nothing to be near.
///
/// Each rule's other side and bound are found once, so a search that asks of thousands of colours
/// ([`super::tint::nudged`]) measures only the colour it asks of (#3597).
pub struct HeldApart {
    /// Each rule's other side, and the distance a colour must keep from it.
    rules: Vec<([f64; 3], f64)>,
}

impl HeldApart {
    pub fn of(token: &str, written: &str, value: impl Fn(&str) -> Option<String>) -> Self {
        let written = super::tint::oklab_of(written);
        let rules = super::tint::APART
            .iter()
            .filter(|rule| rule.token == token && !rule.words)
            .filter_map(|rule| {
                let from = value(rule.from)
                    .as_deref()
                    .and_then(super::tint::oklab_of)?;
                let was = super::tint::distance(written?, from);
                Some((from, rule.distance.min(was)))
            })
            .collect();
        Self { rules }
    }

    /// Whether `turned`, what the token would be drawn as, holds every rule.
    pub fn holds(&self, turned: &str) -> bool {
        let Some(turned) = super::tint::oklab_of(turned) else {
            return true;
        };
        self.rules
            .iter()
            .all(|(from, bound)| super::tint::distance(turned, *from) >= *bound)
    }
}

/// The accent.
const ACCENT: &str = "accent.base";

/// The focus ring, which a parent can draw as the selected row's edge.
const RING: &str = "focus.ring";

/// The selected row's edge, which stays its ring where the parent draws the two alike (#5157).
const EDGE: &str = "list.selected-edge";

/// The primary's fill under the pointer, which turns with the fill (#5157).
const FILL_HOVER: &str = "accent.fill-hover";

/// The first chart colour, the mark of the accent that stands among other series.
const FIRST_SERIES: &str = "chart.1";

/// The series the first chart colour is told from.
const OTHER_SERIES: [&str; 7] = [
    "chart.2", "chart.3", "chart.4", "chart.5", "chart.6", "chart.7", "chart.8",
];

/// **How far a first chart colour that follows an accent stays from each other series, at least**,
/// or no nearer than the parent's was (#5157): two lines of a chart are told apart at a glance.
pub const SERIES_APART: f64 = 0.1;

/// **What the window draws in the accent, past its words and their grounds** (#4918): the
/// primary's fill, at rest and under the pointer, the focus ring and the selected row's edge. An
/// accent given over a parent turns the parent's to its hue as it turns the grounds
/// ([`follow_kin`]): luminance and alpha kept, so the focus ring keeps its 3:1 and the primary's
/// words their 4.5:1, and chroma no more than the accent's. One the parent draws as its accent
/// is the accent given (#5157), so what the parent draws alike stays alike: a base16 import over
/// Purlis Classic gives its accent and its ring base0d, and its selected edge is base0d too, as
/// the accent's words already are (#3100). One that
/// turning would bring nearer
/// the waiting or danger colour than the rules of [`super::tint::APART`] allow, and nearer than
/// the parent's, stays the parent's. One the parent draws alike the token of [`KIN`] that drew
/// it before follows that token as it is drawn, turned or not (#5157), not the accent: Purlis
/// Classic's primary is filled with its surface, turned or not, and under the pointer it turns to
/// that fill's hue. An edge the parent draws as its ring stays the ring as it is drawn. The active
/// tab is not here: Purlis draws it as a grey ground, with nothing of the accent to turn.
pub const DRAWN_IN_THE_ACCENT: [&str; 4] = [
    "accent.fill",
    "accent.fill-hover",
    "focus.ring",
    "list.selected-edge",
];

/// The accent's words on a chosen item (#3100), and what they follow ([`follow_kin`]).
pub const ACCENT_WORDS: &str = "accent.text";

/// **The grounds the accent's words stand on** (#4666): a chosen item's surface and the washes
/// of a chosen chip, row or tab, which follow the accent ([`follow_kin`]).
pub const ACCENT_GROUNDS: [&str; 3] = ["accent.surface", "accent.wash", "accent.wash-hover"];

/// **The accent's washes** (#4666): the grounds a workspace's colour turns with the accent's
/// words, at their answer, unless that brings them nearer a signal's wash
/// ([`super::tint::NO_NEARER`]).
pub const ACCENT_WASHES: [&str; 2] = ["accent.wash", "accent.wash-hover"];

/// **The tokens drawn as what they follow**, each by its own token: the hot count and the
/// needs-you mark (#3098), and the accent's words and the accent (#3100). Where a theme draws one
/// of these alike, the first follows the second, so holding a floor or raising the contrast moves
/// the two together and they never disagree. A theme that draws the first in a colour of its own
/// has it held on its own.
pub const BOUND_KIN: [(&str, &str); 3] = [
    ("needs-you.count", "needs-you.base"),
    ("needs-you.count-text", "needs-you.text"),
    (ACCENT_WORDS, "accent.base"),
];

/// The pairs of [`BOUND_KIN`] that `theme` draws alike.
pub fn drawn_alike(
    values: &std::collections::BTreeMap<&'static str, String>,
) -> Vec<(&'static str, &'static str)> {
    BOUND_KIN
        .into_iter()
        .filter(|(count, mark)| values.get(count) == values.get(mark))
        .collect()
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
