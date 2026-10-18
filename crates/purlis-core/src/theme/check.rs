// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whether the words can be read**: the contrast check, for any theme.
//!
//! "Complete" is not "legible": a light theme made by inverting a dark one is complete and
//! unreadable. So each pair of tokens that ends up as something drawn on something is held to a
//! contrast ratio ([`PAIRS`]), the sixteen ANSI colours are held off the terminal's background,
//! and the selected row is held apart from the pointer's and the tab's.
//!
//! **WCAG 2.1 AA is 4.5:1 for body text and 3:1 for large text and for a control's outline.**
//! The window's text is 14px, so body text is held to 4.5. Marks (a dot, a chip, an icon) are held
//! to 3, the ratio the standard gives a non-text thing that has to be told apart. This is a
//! floor, not a target: it does not say a theme is nice; it says no word vanishes into its own
//! background.
//!
//! A theme is checked as it is and **tinted by every colour a workspace can name**, by its own
//! recipe (#2329): the tint turns the tokens the theme names, at the hues it gives, so every
//! pair with one of them in it is held again at every hue. The tint keeps each shade's luminance to make that hold by construction; this is the
//! check that the construction did.
//!
//! `app/src/theme/contrast.test.ts` reads [`PAIRS`] out of this file and fails when a stylesheet
//! draws a pair of tokens that is not in it.

use super::vocabulary::{TOKENS, is_platform};
use super::{ResolvedTheme, tint};

/// What a token of [`super::vocabulary::PLATFORM_DRAWN`] left to the platform is measured as
/// when it is drawn on something: selected text the theme leaves alone keeps the words' own
/// colour. One with no stand-in is the platform's, and a pair with it in is not measured.
const STANDS_IN: &[(&str, &str)] = &[
    ("selection.text", "text.primary"),
    ("terminal.selection-foreground", "terminal.foreground"),
];

/// One pair: `front` drawn on `back`, held to `floor`. `under` is what `back` is laid over, for
/// a `back` that is a wash with alpha. A `front` with alpha is measured laid over the `back`.
/// `fade` is the opacity a stylesheet draws `front` at, for words drawn faded: the front is
/// measured laid over the `back` at that opacity.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Pair {
    pub front: &'static str,
    pub back: &'static str,
    pub floor: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub under: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fade: Option<f64>,
}

impl Pair {
    /// The same pair, with `back` laid over `under` first.
    const fn over(self, under: &'static str) -> Self {
        Self {
            under: Some(under),
            ..self
        }
    }

    /// The same pair, with `front` drawn at `opacity`.
    const fn faded(self, opacity: f64) -> Self {
        Self {
            fade: Some(opacity),
            ..self
        }
    }
}

const fn p(front: &'static str, back: &'static str, floor: f64) -> Pair {
    Pair {
        front,
        back,
        floor,
        under: None,
        fade: None,
    }
}

/// WCAG 2.1 AA's floor for text of a given size: 3:1 once it is "large" (24px, or 18.66px
/// bold), 4.5:1 under that. A pair that is words is held to the floor for the size it is drawn
/// at, not to a floor picked for the pair.
pub const fn aa_text(px: f64, bold: bool) -> f64 {
    if px >= 24.0 || (bold && px >= 18.66) {
        3.0
    } else {
        4.5
    }
}

/// The window's text size: a button's label and a menu row are drawn at it, never larger.
const WINDOW_TEXT_PX: f64 = 14.0;

/// Words at the window's text size.
const TEXT: f64 = aa_text(WINDOW_TEXT_PX, false);

/// **Every pair that ends up as something drawn on something**, and the floor it has to clear.
pub const PAIRS: &[Pair] = &[
    p("text.primary", "surface.base", 4.5),
    p("text.primary", "surface.sunken", 4.5),
    p("text.primary", "surface.deep", 4.5),
    p("text.primary", "surface.raised", 4.5),
    p("text.primary", "surface.overlay", 4.5),
    p("text.primary", "surface.hover", 4.5),
    p("text.primary", "control.base", 4.5),
    p("text.primary", "control.hover", 4.5),
    p("text.primary", "control.aimed", 4.5),
    p("text.primary", "control.count", 4.5),
    p("text.primary", "tab.active", 4.5),
    p("text.primary", "accent.surface", 4.5),
    p("text.secondary", "surface.base", 4.5),
    p("text.muted", "surface.base", 4.5),
    p("text.muted", "surface.overlay", 4.5),
    // The three strips are three surfaces, and a tab that is not the selected one is muted on
    // whichever its strip sits on. A pane's own controls are muted on `surface.raised` too.
    p("text.muted", "surface.deep", 4.5),
    p("text.muted", "surface.raised", 4.5),
    // A menu and a popover are `surface.overlay`: their details are secondary text on it.
    p("text.secondary", "surface.overlay", 4.5),
    // Settings (DS-3e): the level switcher's unchosen levels are secondary text on
    // `control.base`, and the group nav's unchosen groups are secondary text on the pane.
    p("text.secondary", "control.base", 4.5),
    p("text.secondary", "surface.raised", 4.5),
    p("needs-you.text", "needs-you.base", 4.5),
    p("danger.text", "danger.surface", 4.5),
    p("terminal.foreground", "terminal.background", 4.5),
    // A find in a pane draws its matches as the cells' background, under the terminal's text.
    p("terminal.foreground", "terminal.find-match", 4.5),
    p("terminal.foreground", "terminal.find-match-active", 4.5),
    // Marks rather than prose: a chip, a dot, a one-word CI state.
    p("state.running", "surface.base", 3.0),
    p("state.waiting", "surface.base", 3.0),
    p("state.failed", "surface.base", 3.0),
    p("state.success", "surface.base", 3.0),
    p("state.unreadable", "surface.base", 3.0),
    // A chat's state on a row of a list (#1484): its mark in the state's colour and its word in
    // secondary text, on the window and on a row that is current or under the pointer.
    p("needs-you.base", "surface.base", 3.0),
    // The needs-you mark on the strip it sits on.
    p("needs-you.base", "surface.deep", 3.0),
    // The title bar's needs-you hand on its button under the pointer.
    p("needs-you.base", "control.hover", 3.0),
    p("text.muted", "surface.hover", 3.0),
    p("state.running", "surface.hover", 3.0),
    p("state.waiting", "surface.hover", 3.0),
    p("state.failed", "surface.hover", 3.0),
    p("state.unreadable", "surface.hover", 3.0),
    p("needs-you.base", "surface.hover", 3.0),
    p("text.secondary", "surface.hover", 4.5),
    // An answer that ends something says so in `danger.base` words at the window's text size,
    // on each ground it is drawn on: its button, the button under the pointer, a menu's row, the
    // window and a pane (#1210).
    p("danger.base", "control.base", TEXT),
    p("danger.base", "danger.surface", TEXT),
    p("danger.base", "surface.overlay", TEXT),
    p("danger.base", "surface.base", TEXT),
    p("danger.base", "surface.raised", TEXT),
    // Trouble said in `danger.text` words on the ground around it, not only on its own surface:
    // a dialog, a strip, the window and a pane.
    p("danger.text", "surface.overlay", TEXT),
    p("danger.text", "surface.deep", TEXT),
    p("danger.text", "surface.base", TEXT),
    p("danger.text", "surface.raised", TEXT),
    // Marks on a menu or popover, and the status line's bell on its button.
    p("state.waiting", "surface.overlay", 3.0),
    p("state.failed", "surface.overlay", 3.0),
    // A dialog is `surface.overlay` too (#1845): the doctor's passed checks, a link's accent and
    // the focus ring are drawn on its ground.
    p("state.success", "surface.overlay", 3.0),
    p("accent.base", "surface.overlay", 3.0),
    p("focus.ring", "surface.overlay", 3.0),
    p("state.waiting", "control.base", 3.0),
    // An alert's mark in the Inbox's Notices (#1695), on the list's `surface.raised`.
    p("state.waiting", "surface.raised", 3.0),
    p("state.failed", "surface.raised", 3.0),
    // Words in a state's colour on a view tab (#1457), at the window's text size.
    p("state.running", "surface.raised", TEXT),
    p("state.waiting", "surface.raised", TEXT),
    p("state.failed", "surface.raised", TEXT),
    p("state.success", "surface.raised", TEXT),
    p("state.unreadable", "surface.raised", TEXT),
    p("accent.base", "surface.base", 3.0),
    p("focus.ring", "surface.base", 3.0),
    // The three strips are three shades (charter-app#193): a tab that is not the one you are on
    // is muted text on its strip, and the one you are on is primary text on `layer.selected`.
    p("text.muted", "layer.project", 4.5),
    // A read-only project's line (#826) on the project strip: its sentence in secondary text,
    // and its lock a mark in the waiting colour.
    p("text.secondary", "layer.project", 4.5),
    p("state.waiting", "layer.project", 3.0),
    p("text.muted", "layer.workspace", 4.5),
    p("text.muted", "layer.chat", 4.5),
    p("text.primary", "layer.selected", 4.5),
    // A tab's pin is a muted mark, and a live workspace's dot one in the accent (qw337).
    p("text.muted", "layer.selected", 3.0),
    p("accent.base", "layer.selected", 3.0),
    // A workspace's colour (charter-app#281): its tab in its own shade with a mark in its own
    // accent, and the one in front primary text on its tinted `layer.selected`.
    p("accent.base", "layer.workspace", 3.0),
    p("text.primary", "layer.workspace", 4.5),
    // A file's or a folder's icon (FM-3) is a mark, on the explorer's window and the sunken file
    // tab alike. `icon.motive` is drawn over a folder, never on the window.
    p("icon.folder", "surface.base", 3.0),
    p("icon.folder", "surface.sunken", 3.0),
    p("icon.grey", "surface.base", 3.0),
    p("icon.grey", "surface.sunken", 3.0),
    p("icon.red", "surface.base", 3.0),
    p("icon.red", "surface.sunken", 3.0),
    p("icon.orange", "surface.base", 3.0),
    p("icon.orange", "surface.sunken", 3.0),
    p("icon.yellow", "surface.base", 3.0),
    p("icon.yellow", "surface.sunken", 3.0),
    p("icon.green", "surface.base", 3.0),
    p("icon.green", "surface.sunken", 3.0),
    p("icon.teal", "surface.base", 3.0),
    p("icon.teal", "surface.sunken", 3.0),
    p("icon.blue", "surface.base", 3.0),
    p("icon.blue", "surface.sunken", 3.0),
    p("icon.purple", "surface.base", 3.0),
    p("icon.purple", "surface.sunken", 3.0),
    p("icon.pink", "surface.base", 3.0),
    p("icon.pink", "surface.sunken", 3.0),
    // The row a tree or a list has selected (#1672): everything a row draws is drawn on its
    // fill, and its edge is the shape that says "selected" without colour.
    p("text.primary", "list.selected", 4.5),
    p("text.secondary", "list.selected", 4.5),
    p("text.muted", "list.selected", 4.5),
    p("state.running", "list.selected", 3.0),
    p("state.waiting", "list.selected", 3.0),
    p("state.failed", "list.selected", 3.0),
    p("state.unreadable", "list.selected", 3.0),
    p("needs-you.base", "list.selected", 3.0),
    p("accent.base", "list.selected", 3.0),
    p("list.selected-edge", "list.selected", 3.0),
    // The palette's aimed row (#1682): its fill is the aim's, and its edge the selected rows'.
    p("list.selected-edge", "control.aimed", 3.0),
    p("list.selected-edge", "surface.base", 3.0),
    // The keyboard's ring on the selected row it is on.
    p("focus.ring", "list.selected", 3.0),
    // A branch's change marks in the explorer (FM-4, #2392): a letter on a file and its name in
    // the change's colour, on the window, the row under the pointer and the selected row.
    // Added is `state.success` and deleted `danger.base`, the editor's diff gutters' marks; the
    // `diff.*-text` tokens are washes under words, not colours to draw words in. Changed
    // (`state.waiting`) and renamed (`accent.base`) are held on these rows above.
    p("state.success", "surface.hover", 3.0),
    p("state.success", "list.selected", 3.0),
    p("danger.base", "surface.hover", 3.0),
    p("danger.base", "list.selected", 3.0),
    // A tree's guides: a hairline, quieter than a border and still there.
    p("tree.guide", "surface.base", 1.3),
    p("tree.guide", "surface.raised", 1.2),
    p("tree.guide", "surface.sunken", 1.2),
    p("border.subtle", "surface.base", 1.2),
    p("border.strong", "surface.base", 1.5),
    // DS-6 (#629): the pairs the stylesheets draw in one rule.
    // - A persona's mark is its initial, a word in the accent on the accent's surface.
    p("accent.base", "accent.surface", TEXT),
    // - A checklist's tick is a mark in the accent on a sunken box.
    p("accent.base", "surface.sunken", 3.0),
    // - What an extension runs as you, said inside its approval dialog on the danger wash.
    p("text.primary", "danger.wash", 4.5).over("surface.base"),
    // - The same, in a dialog: its refusal and its unrun command on the danger wash.
    p("text.primary", "danger.wash", 4.5).over("surface.overlay"),
    // - What a dialog is honest about (`.honest`): its words at the opacity it fades them to.
    p("text.primary", "surface.overlay", TEXT).faded(0.8),
    // - A task that is away draws its words on the terminal's own background.
    p("text.primary", "terminal.background", 4.5),
    // - A notice's "and N more" is secondary words on a bar of the hairline's colour.
    p("text.secondary", "border.subtle", 4.5),
    // - A count beside a workspace, a branch or a search is secondary words on a count chip.
    p("text.secondary", "control.count", 4.5),
    // DS-6 (#629): the pairs a state rule draws over its base rule.
    // - A notice's "and N more" under the pointer or the keyboard: primary words on its bar.
    p("text.primary", "border.subtle", TEXT),
    // - A list's load-more row under the pointer: words in the accent on the hover surface.
    p("accent.base", "surface.hover", TEXT),
    // - A panel's view button under the pointer: secondary words on the control's hover.
    p("text.secondary", "control.hover", TEXT),
    // - A vault's eye under the pointer: a muted mark on the control's hover.
    p("text.muted", "control.hover", 3.0),
    // #2327: the tokens that are not the window's chrome.
    // - The editor's code (`editor/look.ts`): each syntax role on the editor's ground. Held as
    //   the ANSI colours they were drawn in before are held: a mark that has to be told apart, at
    //   3:1.
    p("syntax.keyword", "terminal.background", 3.0),
    p("syntax.string", "terminal.background", 3.0),
    p("syntax.number", "terminal.background", 3.0),
    p("syntax.constant", "terminal.background", 3.0),
    p("syntax.comment", "terminal.background", 3.0),
    p("syntax.function", "terminal.background", 3.0),
    p("syntax.type", "terminal.background", 3.0),
    p("syntax.property", "terminal.background", 3.0),
    p("syntax.attribute", "terminal.background", 3.0),
    p("syntax.tag", "terminal.background", 3.0),
    p("syntax.heading", "terminal.background", 3.0),
    p("syntax.invalid", "terminal.background", 3.0),
    p("syntax.variable", "terminal.background", 3.0),
    p("syntax.operator", "terminal.background", 3.0),
    p("syntax.punctuation", "terminal.background", 3.0),
    // - A markdown render's link, code, quote and heading (`.release-notes`), words on the
    //   window, a pane and a dialog.
    p("markdown.link", "surface.base", TEXT),
    p("markdown.link", "surface.raised", TEXT),
    p("markdown.link", "surface.overlay", TEXT),
    p("markdown.code", "surface.base", TEXT),
    p("markdown.code", "surface.raised", TEXT),
    p("markdown.code", "surface.overlay", TEXT),
    p("markdown.code-block", "surface.base", TEXT),
    p("markdown.code-block", "surface.raised", TEXT),
    p("markdown.code-block", "surface.overlay", TEXT),
    p("markdown.quote", "surface.base", TEXT),
    p("markdown.quote", "surface.raised", TEXT),
    p("markdown.quote", "surface.overlay", TEXT),
    p("markdown.heading", "surface.base", TEXT),
    p("markdown.heading", "surface.raised", TEXT),
    p("markdown.heading", "surface.overlay", TEXT),
    // - A release's notes in the Updates dialog sit on a sunken box.
    p("markdown.link", "surface.sunken", TEXT),
    p("markdown.code", "surface.sunken", TEXT),
    p("markdown.code-block", "surface.sunken", TEXT),
    p("markdown.quote", "surface.sunken", TEXT),
    p("markdown.heading", "surface.sunken", TEXT),
    // - The window's own text selection, caret and placeholder, on a field's ground. A token
    //   left to the platform is not measured (`PLATFORM_DRAWN`); selected text left to it keeps
    //   its own colour, measured as the window's words.
    p("selection.text", "selection.background", TEXT).over("surface.base"),
    p("placeholder", "surface.sunken", TEXT),
    p("caret", "surface.sunken", 3.0),
    // - A scrollbar's thumb on its track, laid over the window: a hairline's floor.
    p("scrollbar.thumb", "scrollbar.track", 1.2).over("surface.base"),
    // - A chart's bars, on the track they are drawn in: marks.
    p("chart.1", "surface.sunken", 3.0),
    p("chart.2", "surface.sunken", 3.0),
    p("chart.3", "surface.sunken", 3.0),
    p("chart.4", "surface.sunken", 3.0),
    p("chart.5", "surface.sunken", 3.0),
    p("chart.6", "surface.sunken", 3.0),
    p("chart.7", "surface.sunken", 3.0),
    p("chart.8", "surface.sunken", 3.0),
    // - The terminal's selected text, focused and not, on the selection laid over the
    //   terminal; left to the platform, it keeps the cells' own colour, measured as the
    //   terminal's words.
    p("terminal.selection-foreground", "terminal.selection", TEXT).over("terminal.background"),
    p(
        "terminal.selection-foreground",
        "terminal.selection-inactive",
        TEXT,
    )
    .over("terminal.background"),
    // - The terminal's scrollbar, a hairline on the terminal.
    p("terminal.scrollbar", "terminal.background", 1.2),
    // An Allow (`.ui-button-grants`, ADR 0097 §6) is edged in the accent on the control's own
    // ground, at rest and under the pointer: the edge is a control's outline, held to 3:1.
    p("accent.base", "control.base", 3.0),
    p("accent.base", "control.hover", 3.0),
];

/// The floor the sixteen ANSI colours clear on the terminal's own background.
///
/// **`black` is held lower, and that is not a loophole.** ANSI black on a dark terminal is dim in
/// every theme there has ever been: it is the colour a program picks when it means "recede". It
/// still has to be visible, because `ESC[30m` must not be an invisible line, so it clears 1.5.
fn ansi_floor(token: &str) -> f64 {
    if token == "terminal.ansi.black" {
        1.5
    } else {
        3.0
    }
}

/// The tokens the selected row (`list.selected`) must never equal: the pointer's row, the tab
/// you are on, and the window (#1672).
const SELECTED_APART_FROM: [&str; 3] = ["surface.hover", "layer.selected", "surface.base"];

/// What [`check`] found: nothing, for a theme whose words can be read.
///
/// **This shape is today's (format 1) and will change for format 2** (spec #2313): typed values
/// beyond colour and motion (geometry, fonts, icons), keys for each appearance × option
/// combination, and schema errors beside the findings. Its users are the window's wire types and
/// the engine's own tests, which change with it.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Report {
    pub findings: Vec<Finding>,
}

impl Report {
    /// Whether the theme cleared every floor, as it is and in every tint.
    pub fn passes(&self) -> bool {
        self.findings.is_empty()
    }
}

/// One floor a theme does not clear, and where: `tint` is the workspace colour it was tinted by,
/// `None` for the theme as it is.
///
/// **This shape is today's (format 1) and will change for format 2** (spec #2313): typed values
/// beyond colour and motion (geometry, fonts, icons), keys for each appearance × option
/// combination, and schema errors beside the findings. Its users are the window's wire types and
/// the engine's own tests, which change with it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Finding {
    /// A pair of [`PAIRS`] under its floor, with the ratio it has (to two places).
    Contrast {
        tint: Option<&'static str>,
        pair: Pair,
        ratio: f64,
    },
    /// An ANSI colour too close to the terminal's background.
    Ansi {
        tint: Option<&'static str>,
        token: &'static str,
        ratio: f64,
        floor: f64,
    },
    /// The selected row drawn exactly as `other` is.
    SelectedNotApart {
        tint: Option<&'static str>,
        other: &'static str,
    },
    /// A colour for selected text with the selection left to the platform (#2327): the window
    /// draws selected text only on a selection the theme colours too, so this one is never drawn.
    SelectionTextAlone,
}

/// The check, as [`super::check`] describes it.
pub(super) fn check(theme: &ResolvedTheme) -> Report {
    let mut findings = Vec::new();
    check_one(theme, None, &mut findings);
    let platform = |token: &str| is_platform(token, theme.value(token));
    if platform("selection.background") && !platform("selection.text") {
        findings.push(Finding::SelectionTextAlone);
    }
    // A theme a workspace's colour does not tint is only ever drawn as it is.
    if !theme.tint.tabs().is_empty() {
        for one in &theme.tint.hues {
            check_one(
                &tint::tinted(theme, one.name),
                Some(one.name),
                &mut findings,
            );
        }
    }
    Report { findings }
}

fn check_one(theme: &ResolvedTheme, tint: Option<&'static str>, findings: &mut Vec<Finding>) {
    for pair in PAIRS {
        let Some(ratio) = ratio(theme, pair) else {
            continue;
        };
        if ratio < pair.floor {
            findings.push(Finding::Contrast {
                tint,
                pair: *pair,
                ratio,
            });
        }
    }
    let background = theme.value("terminal.background");
    for token in TOKENS.iter().filter(|t| t.starts_with("terminal.ansi.")) {
        let ratio = contrast(theme.value(token), background);
        let floor = ansi_floor(token);
        if ratio < floor {
            findings.push(Finding::Ansi {
                tint,
                token,
                ratio: two_places(ratio),
                floor,
            });
        }
    }
    let selected = theme.value("list.selected").to_lowercase();
    for other in SELECTED_APART_FROM {
        if theme.value(other).to_lowercase() == selected {
            findings.push(Finding::SelectedNotApart { tint, other });
        }
    }
}

/// **The contrast `pair` has in `theme`**, to two places, as it is drawn: `back` laid over
/// `under` when it names one, and a see-through `front` laid over that. `None` when the platform
/// draws one of them ([`super::vocabulary::PLATFORM_DRAWN`]); a front left to the platform that
/// keeps the words' own colour is measured as those words ([`STANDS_IN`]).
pub(super) fn ratio(theme: &ResolvedTheme, pair: &Pair) -> Option<f64> {
    let (front, back) = measured(theme, pair)?;
    let back = theme.value(back);
    let drawn = match pair.under {
        Some(under) => onto(back, theme.value(under)),
        None => back.to_owned(),
    };
    let front = onto(theme.value(front), &drawn);
    let front = match pair.fade {
        Some(opacity) => mix(&front, &drawn, opacity),
        None => front,
    };
    Some(two_places(contrast(&front, &drawn)))
}

/// The tokens `pair` is measured between in `theme`: its own, a platform-drawn front's stand-in,
/// or `None` when the platform draws one of them.
fn measured(theme: &ResolvedTheme, pair: &Pair) -> Option<(&'static str, &'static str)> {
    let unset = |token: &str| is_platform(token, theme.value(token));
    if unset(pair.back) {
        return None;
    }
    if !unset(pair.front) {
        return Some((pair.front, pair.back));
    }
    STANDS_IN
        .iter()
        .find(|(token, _)| *token == pair.front)
        .map(|(_, stand_in)| (*stand_in, pair.back))
}

/// A ratio to two places, as it is said and compared: 4.495 is under 4.5 only once rounded.
fn two_places(ratio: f64) -> f64 {
    format!("{ratio:.2}").parse().unwrap_or(ratio)
}

/// The two hex digits of `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` as six or eight.
fn long(hex: &str) -> String {
    let digits = hex.strip_prefix('#').unwrap_or(hex);
    if digits.len() <= 4 {
        digits.chars().flat_map(|d| [d, d]).collect()
    } else {
        digits.to_owned()
    }
}

fn byte(digits: &str, at: usize) -> f64 {
    digits
        .get(at..at + 2)
        .and_then(|two| u8::from_str_radix(two, 16).ok())
        .map_or(0.0, f64::from)
}

/// One channel of a colour, as the linear number WCAG's formula wants.
fn channel(hex: &str, at: usize) -> f64 {
    let value = byte(&long(hex), at * 2) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance.
fn luminance(hex: &str) -> f64 {
    0.2126 * channel(hex, 0) + 0.7152 * channel(hex, 1) + 0.0722 * channel(hex, 2)
}

/// WCAG contrast ratio, 1 (identical) to 21 (black on white). Alpha is ignored: a pair whose
/// background is a wash names what the wash is laid over, a see-through front is laid over its
/// background, and [`onto`] lays each.
pub(super) fn contrast(a: &str, b: &str) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    let (light, dark) = if x >= y { (x, y) } else { (y, x) };
    (light + 0.05) / (dark + 0.05)
}

/// The opaque `top` laid over the opaque `under` at `opacity`, as the screen draws it.
fn mix(top: &str, under: &str, opacity: f64) -> String {
    let (top, bottom) = (long(top), long(under));
    let mixed: String = [0, 2, 4]
        .iter()
        .map(|at| {
            let value = byte(&top, *at) * opacity + byte(&bottom, *at) * (1.0 - opacity);
            format!("{:02x}", (value + 0.5).floor() as u8)
        })
        .collect();
    format!("#{mixed}")
}

/// `wash` laid over the opaque `under`, as the screen draws it. An opaque `wash` is itself.
fn onto(wash: &str, under: &str) -> String {
    let top = long(wash);
    if top.len() == 6 {
        return wash.to_owned();
    }
    let alpha = byte(&top, 6) / 255.0;
    let bottom = long(under);
    let mixed: String = [0, 2, 4]
        .iter()
        .map(|at| {
            let value = byte(&top, *at) * alpha + byte(&bottom, *at) * (1.0 - alpha);
            // JavaScript's `Math.round`: a half rounds up.
            format!("{:02x}", (value + 0.5).floor() as u8)
        })
        .collect();
    format!("#{mixed}")
}
