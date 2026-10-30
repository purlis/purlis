// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whether the words can be read**: the contrast check, for any theme.
//!
//! "Complete" is not "legible": a light theme made by inverting a dark one is complete and
//! unreadable. So each pair of tokens that ends up as something drawn on something is held to a
//! contrast ratio ([`PAIRS`]), the sixteen ANSI colours are held off the terminal's background,
//! and the selected row is held apart from the pointer's and the tab's.
//!
//! **A theme's fonts bring every style it draws** (#2736): a style drawn in a font the theme
//! brings with no face of its own ([`super::without_a_face`]) is made by the browser from
//! another face, slanted or thickened, so the check names its slot and style.
//!
//! **Geometry is held across tokens too** ([`FITS`]): a line of the window's words, its size
//! times its line height, fits inside the control or the strip it is drawn in, in both densities.
//! A theme may set each of the numbers within its own bounds and still make a line taller than
//! the box it sits in; the check names the sizes when it does.
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

use super::fonts::{Style, without_a_face};
use super::geometry::Geometry;
use super::vocabulary::{TOKENS, is_platform};
use super::{Density, ResolvedTheme, tint};

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tiers_tests.rs"]
mod tiers_tests;

#[cfg(test)]
#[path = "card_ground_tests.rs"]
mod card_ground_tests;

/// What a token of [`super::vocabulary::PLATFORM_DRAWN`] left to the platform is measured as
/// when it is drawn on something: selected text the theme leaves alone keeps the words' own
/// colour. One with no stand-in is the platform's, and a pair with it in is not measured.
const STANDS_IN: &[(&str, &str)] = &[
    ("selection.text", "text.primary"),
    ("terminal.selection-foreground", "terminal.foreground"),
];

/// A pair as the check names it, the way its warnings and its hard floors say it: `front` on
/// `back`, then each layer under `back` from the top down (`… over danger.wash over
/// surface.overlay`).
pub(super) fn laid(front: &str, back: &str, layers: [Option<&str>; 2]) -> String {
    let over: String = layers
        .into_iter()
        .flatten()
        .map(|layer| format!(" over {layer}"))
        .collect();
    format!("{front} on {back}{over}")
}

/// One pair: `front` drawn on `back`, held to `floor`. `under` is what `back` is laid over, for
/// a `back` that is a wash with alpha, and `beneath` what `under` is laid over in turn, for an
/// `under` that is a wash too (#2724): the layers are laid from the bottom up. A `front` with
/// alpha is measured laid over the `back`.
/// `fade` is the opacity a stylesheet draws `front` at, for words drawn faded: the front is
/// measured laid over the `back` at that opacity. `selected` marks words drawn selected in their
/// own colour: a theme that colours selected text draws them all in `selection.text`, which its
/// own pair holds, so a `selected` pair is measured only while selected text is the platform's.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Pair {
    pub front: &'static str,
    pub back: &'static str,
    pub floor: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub under: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beneath: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fade: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub selected: bool,
}

impl Pair {
    /// The same pair, with what it is drawn on laid over `layer` first: `back` over it, or, for a
    /// pair already laid over one layer, that layer over it. A pair names two layers at most, and
    /// [`PAIRS`] is built at compile time, so a third is a build error.
    const fn over(self, layer: &'static str) -> Self {
        match (self.under, self.beneath) {
            (None, _) => Self {
                under: Some(layer),
                ..self
            },
            (Some(_), None) => Self {
                beneath: Some(layer),
                ..self
            },
            (Some(_), Some(_)) => panic!("a pair is laid over two layers at most"),
        }
    }

    /// Every token the pair names: its front, its back and each layer under them.
    pub fn tokens(&self) -> impl Iterator<Item = &'static str> {
        [Some(self.front), Some(self.back), self.under, self.beneath]
            .into_iter()
            .flatten()
    }

    /// The same pair, with `front` drawn at `opacity`.
    const fn faded(self, opacity: f64) -> Self {
        Self {
            fade: Some(opacity),
            ..self
        }
    }

    /// The same pair, as words selected in their own colour.
    const fn selected(self) -> Self {
        Self {
            selected: true,
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
        beneath: None,
        fade: None,
        selected: false,
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

/// **The window's text tiers** (#2902), loudest first: titles and the words in front, body
/// text, labels, and quiet words — line numbers, placeholders, counts, paths, group labels and
/// the status bar's folder. Every one is words, so every one is held to [`TEXT`] on each of
/// [`TIER_GROUNDS`]; no informational text is exempt.
pub const TEXT_TIERS: [&str; 4] = ["text.primary", "text.secondary", "text.muted", "text.quiet"];

/// **The tier for what is not words** (#2902): a tab's ×, a separator, a pin, a chevron, a
/// checkbox's ring, and disabled words, which WCAG exempts. A mark has to be told apart, so it
/// is held to 3:1 (WCAG 1.4.11) on each of [`TIER_GROUNDS`]; words a person reads take a tier
/// of [`TEXT_TIERS`].
pub const FAINT: &str = "text.faint";

/// **Every ground a part draws a tier on** (#2902): the window, a field, a strip, a panel, a
/// card (a menu, a dialog, a Notice), the row under the pointer, the editor's code, the selected
/// row, the tab in front, and a control at rest and under the pointer. [`PAIRS`] holds each tier
/// on each of them, and a test keeps it so.
pub const TIER_GROUNDS: [&str; 11] = [
    "surface.base",
    "surface.sunken",
    "surface.deep",
    "surface.raised",
    "surface.overlay",
    "surface.hover",
    "terminal.background",
    "list.selected",
    "layer.selected",
    "control.base",
    "control.hover",
];

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
    // What needs a decision (#3678): an attention Notice's words and mark on its own ground, and
    // a card's title and sentence on it.
    p("attention.text", "attention.surface", 4.5),
    p("text.primary", "attention.surface", 4.5),
    p("text.muted", "attention.surface", 4.5),
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
    p("text.muted", "surface.hover", TEXT),
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
    // A refused box's edge (a field's, a select's, a combobox's, #3110) is the danger colour on
    // the box's own sunken ground: a mark, so 3:1.
    p("danger.base", "surface.sunken", 3.0),
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
    // A chat's state badge on the persona's mark (#3342), ringed in a Purlis panel's fill.
    p("needs-you.base", "surface.raised", 3.0),
    // Words in a state's colour on a view tab (#1457), at the window's text size.
    p("state.running", "surface.raised", TEXT),
    p("state.waiting", "surface.raised", TEXT),
    p("state.failed", "surface.raised", TEXT),
    p("state.success", "surface.raised", TEXT),
    p("state.unreadable", "surface.raised", TEXT),
    p("accent.base", "surface.base", 3.0),
    p("focus.ring", "surface.base", 3.0),
    // A resize handle's grip with the keyboard on it (#3143), on the window's frame a gutter
    // shows and on a panel Purlis floats.
    p("focus.ring", "surface.deep", 3.0),
    p("focus.ring", "surface.raised", 3.0),
    // The three strips are three shades (charter-app#193): a tab that is not the one you are on
    // is muted text on its strip, and the one you are on is primary text on `layer.selected`.
    p("text.muted", "layer.project", 4.5),
    // A read-only project's line (#826) on the project strip: its sentence in secondary text,
    // and its lock a mark in the waiting colour.
    p("text.secondary", "layer.project", 4.5),
    // The theme draft strip (#2332) is primary text on the workspace's shade (#2916, held with
    // a workspace's colour below): what a draft is and why it is not drawn are always read (a
    // hard floor). The read-only line is still on the project's.
    p("text.primary", "layer.project", 4.5),
    p("state.waiting", "layer.project", 3.0),
    p("text.muted", "layer.workspace", 4.5),
    p("text.muted", "layer.chat", 4.5),
    p("text.primary", "layer.selected", 4.5),
    // A tab's label behind is muted words on it (#2902); its pin is a faint mark, held with the
    // tiers below, and a live workspace's dot one in the accent (qw337).
    p("text.muted", "layer.selected", TEXT),
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
    // - The filled primary (#3177): its words on its fill, at rest and under the pointer.
    p("accent.fill-text", "accent.fill", TEXT),
    p("accent.fill-text", "accent.fill-hover", TEXT),
    // - A title-bar chip's notice dot (#3648): its fill, the accent, told apart from the bar's
    //   own ground it is ringed in, on a chip at rest, under the pointer and chosen alike; its
    //   shape on that fill is held below (`surface.base` on `accent.base`).
    p("accent.base", "surface.deep", 3.0),
    // - A warning's dot (#3649): the waiting amber, told apart from the same ground; its
    //   triangle on it is `surface.base` on `state.waiting`, held among the marks above.
    p("state.waiting", "surface.deep", 3.0),
    // - The update pill's words in the accent's words on the accent's surface (#3177).
    p("accent.text", "accent.surface", TEXT),
    // - What an extension runs as you, said inside its approval dialog on the danger wash.
    p("text.primary", "danger.wash", 4.5).over("surface.base"),
    // - The same, in a dialog: its refusal and its unrun command on the danger wash.
    p("text.primary", "danger.wash", 4.5).over("surface.overlay"),
    // - What waits on the person, a status pill on the amber wash, on the title bar (#2913).
    p("text.primary", "wash.warning", 4.5).over("surface.deep"),
    // - Its hand, a mark in the needs-you colour on the same wash: 3:1.
    p("needs-you.base", "wash.warning", 3.0).over("surface.deep"),
    // **A state's colour drawn as a part's words is held as words** (#3041): wherever a family
    // of the component parts manifest draws a state's colour as its words, at rest or under the
    // pointer, it is held at the window's text size on the part's fill, not at the 3:1 a mark
    // takes (`parts/state_words_tests.rs` keeps this list whole).
    // - A title-bar chip's waiting count (#2916, R101): its digits in amber on the amber wash,
    //   over the title bar, which backs it on every chip, lit or chosen (D-3345-3).
    p("state.waiting", "wash.warning", TEXT).over("surface.deep"),
    // - What waits said in a badge or a count (#2912): the same amber on its wash, on a popover's
    //   surface.
    p("state.waiting", "wash.warning", TEXT).over("surface.overlay"),
    // - A switcher row's count (#3947): the same, on the row Enter would go to.
    p("state.waiting", "wash.warning", TEXT).over("list.selected"),
    // - The rail's waiting badge (R79): its digits in amber on the overlay's pill.
    p("state.waiting", "surface.overlay", TEXT),
    // - The warning-soft button: amber words on the control, and under the pointer.
    p("state.waiting", "control.base", TEXT),
    p("state.waiting", "control.hover", TEXT),
    // - A chosen activity button: its icon in the accent's words on the accent's wash, laid on
    //   the window and the frame (#3177), held as words are, the icon being all it says.
    p("accent.text", "accent.wash", TEXT).over("surface.base"),
    p("accent.text", "accent.wash", TEXT).over("surface.deep"),
    // - The chosen title-bar chip (#3345, R101 as amended): its name in the primary words on the
    //   accent's wash laid on the frame, and on the stronger wash under the pointer.
    p("text.primary", "accent.wash", TEXT).over("surface.deep"),
    p("text.primary", "accent.wash-hover", TEXT).over("surface.deep"),
    // - What waits on the person said in a badge or a count, in the waiting colour on the amber
    //   wash, on a panel (#2912, R66: "Waiting for you").
    p("state.waiting", "wash.warning", TEXT).over("surface.raised"),
    p("state.waiting", "wash.warning", TEXT).over("surface.base"),
    // - What is settled said in a badge ("Confirmed", R66): the success colour as words, on the
    //   window and on a popover's surface (the raised one is held above).
    p("state.success", "surface.base", TEXT),
    p("state.success", "surface.overlay", TEXT),
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
    // - A pressed card's pick (#3885, `card-button.root.pick`): its words in the accent, on each
    //   ground it stands on, at rest; under the pointer and pressed on the hover surface, above.
    //   A door's words are the primary and its line the secondary tier on the control's ground,
    //   and its mark the accent on it, all held among the tiers and the Allow's edge.
    p("accent.base", "surface.base", TEXT),
    p("accent.base", "surface.raised", TEXT),
    p("accent.base", "surface.overlay", TEXT),
    // - A panel's view button under the pointer: secondary words on the control's hover.
    p("text.secondary", "control.hover", TEXT),
    // - A vault's eye under the pointer: a muted mark on the control's hover, held as muted
    //   words are (#2902).
    p("text.muted", "control.hover", TEXT),
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
    // - The editor's current line (`.cm-activeLine`, #2522) is `surface.hover` over the editor's
    //   ground, and every role is drawn on it at the same floor.
    p("syntax.keyword", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.string", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.number", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.constant", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.comment", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.function", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.type", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.property", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.attribute", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.tag", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.heading", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.invalid", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.variable", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.operator", "surface.hover", 3.0).over("terminal.background"),
    p("syntax.punctuation", "surface.hover", 3.0).over("terminal.background"),
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
    // - The window's own text selection, on each ground it is drawn on (#2706): the window, a
    //   field, a pane and a Notice, a dialog, and a trouble Notice. A see-through selection is
    //   measured laid over each. A token left to the platform is not measured (`PLATFORM_DRAWN`);
    //   selected text left to it keeps its own colour, measured as the window's words. Selected
    //   text a theme colours is what a trouble Notice's danger words and a dialog's ask are drawn
    //   in, so the consent floors hold it on those two grounds (`floors::SELECTED`).
    p("selection.text", "selection.background", TEXT).over("surface.base"),
    p("selection.text", "selection.background", TEXT).over("surface.sunken"),
    p("selection.text", "selection.background", TEXT).over("surface.raised"),
    p("selection.text", "selection.background", TEXT).over("surface.overlay"),
    p("selection.text", "selection.background", TEXT).over("danger.surface"),
    // - The command a profile will run as you (`.honest.approve code`, #2724) is selectable on
    //   the danger wash in its dialog: a see-through selection is laid over the wash, laid over
    //   the dialog. It is what the ask says, so the Ask floor holds it (`floors::SELECTED`).
    p("selection.text", "selection.background", TEXT)
        .over("danger.wash")
        .over("surface.overlay"),
    // - Words selected in their own colour (#2522): while selected text is the platform's, the
    //   window's secondary, muted, accent and state words keep their colours on a theme's
    //   selection, beside its primary words above, over the window, a sunken box (a release's
    //   notes, a finished report), a pane and a Notice, and a dialog.
    p("text.secondary", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("text.secondary", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("text.secondary", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("text.secondary", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("text.muted", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("text.muted", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("text.muted", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("text.muted", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("accent.base", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("accent.base", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("accent.base", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("accent.base", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("state.running", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("state.running", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("state.running", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("state.running", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("state.waiting", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("state.waiting", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("state.waiting", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("state.waiting", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("state.failed", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("state.failed", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("state.failed", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("state.failed", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("state.success", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("state.success", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("state.success", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("state.success", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("state.unreadable", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("state.unreadable", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("state.unreadable", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("state.unreadable", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    // - A markdown render's words, selected in their own colour, over each ground it is drawn on.
    p("markdown.link", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("markdown.link", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("markdown.link", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("markdown.link", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("markdown.code", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("markdown.code", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("markdown.code", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("markdown.code", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("markdown.code-block", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("markdown.code-block", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("markdown.code-block", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("markdown.code-block", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("markdown.quote", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("markdown.quote", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("markdown.quote", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("markdown.quote", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("markdown.heading", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("markdown.heading", "selection.background", TEXT)
        .over("surface.sunken")
        .selected(),
    p("markdown.heading", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("markdown.heading", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    // - Danger words selected in their own colour (#2706): a Notice's trouble and an answer
    //   that ends something stay readable while selected, over each ground they are drawn on.
    //   They are the danger colours, so the consent floors hold them.
    p("danger.text", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("danger.text", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("danger.text", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("danger.text", "selection.background", TEXT)
        .over("danger.surface")
        .selected(),
    p("danger.base", "selection.background", TEXT)
        .over("surface.base")
        .selected(),
    p("danger.base", "selection.background", TEXT)
        .over("surface.raised")
        .selected(),
    p("danger.base", "selection.background", TEXT)
        .over("surface.overlay")
        .selected(),
    p("danger.base", "selection.background", TEXT)
        .over("danger.surface")
        .selected(),
    // - A field's caret and placeholder, on its ground.
    p("placeholder", "surface.sunken", TEXT),
    p("caret", "surface.sunken", 3.0),
    // - A scrollbar's thumb on its track, laid over the window: a hairline's floor.
    p("scrollbar.thumb", "scrollbar.track", 1.2).over("surface.base"),
    // - A chart's bars, on the track they are drawn in: marks. Each point is its own series,
    //   the palette in turn (#2522).
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
    // **Every grey tier on every ground a part draws it on** (#2902, ruling #2803): the four text
    // tiers at the window's text size, faint at the mark floor, on each of `TIER_GROUNDS`. The
    // pairs above already hold the rest of them.
    p("text.secondary", "surface.sunken", TEXT),
    p("text.secondary", "surface.deep", TEXT),
    p("text.secondary", "terminal.background", TEXT),
    p("text.secondary", "layer.selected", TEXT),
    p("text.muted", "surface.sunken", TEXT),
    p("text.muted", "terminal.background", TEXT),
    p("text.muted", "control.base", TEXT),
    p("text.quiet", "surface.base", TEXT),
    p("text.quiet", "surface.sunken", TEXT),
    p("text.quiet", "surface.deep", TEXT),
    p("text.quiet", "surface.raised", TEXT),
    p("text.quiet", "surface.overlay", TEXT),
    p("text.quiet", "surface.hover", TEXT),
    p("text.quiet", "terminal.background", TEXT),
    p("text.quiet", "list.selected", TEXT),
    p("text.quiet", "layer.selected", TEXT),
    p("text.quiet", "control.base", TEXT),
    p("text.quiet", "control.hover", TEXT),
    p("text.faint", "surface.base", 3.0),
    p("text.faint", "surface.sunken", 3.0),
    p("text.faint", "surface.deep", 3.0),
    p("text.faint", "surface.raised", 3.0),
    p("text.faint", "surface.overlay", 3.0),
    p("text.faint", "surface.hover", 3.0),
    p("text.faint", "terminal.background", 3.0),
    p("text.faint", "list.selected", 3.0),
    p("text.faint", "layer.selected", 3.0),
    p("text.faint", "control.base", 3.0),
    p("text.faint", "control.hover", 3.0),
    // - A count or an age in the meta role is quiet words on each of the three strips, and a
    //   tab's × and its pin are faint marks on them.
    p("text.quiet", "layer.project", TEXT),
    p("text.quiet", "layer.workspace", TEXT),
    p("text.quiet", "layer.chat", TEXT),
    p("text.faint", "layer.project", 3.0),
    p("text.faint", "layer.workspace", 3.0),
    p("text.faint", "layer.chat", 3.0),
    // An Allow (the button's `grants` variant, ADR 0097 §6) is edged in the accent on the control's own
    // ground, at rest and under the pointer: the edge is a control's outline, held to 3:1.
    p("accent.base", "control.base", 3.0),
    p("accent.base", "control.hover", 3.0),
    // A ticked box, a picked radio and a switch that is on (#2907): the tick, the dot and the
    // thumb in the window's colour on the accent's fill, a mark held to 3:1.
    p("surface.base", "accent.base", 3.0),
    // A warning's notice dot (#3649): its triangle in the window's colour on the waiting amber, a
    // mark held to 3:1.
    p("surface.base", "state.waiting", 3.0),
];

/// The floor the sixteen ANSI colours clear on the terminal's own background.
///
/// **`black` is held lower, and that is not a loophole.** ANSI black on a dark terminal is dim in
/// every theme there has ever been: it is the colour a program picks when it means "recede". It
/// still has to be visible, because `ESC[30m` must not be an invisible line, so it clears 1.5.
/// **Each ANSI colour on the terminal's background, at its own floor** ([`ansi_floor`]), as a
/// pair: what the check measures of the terminal, in the shape the person's layer and the raised
/// contrast measure it in (#2925).
pub(super) fn ansi_pairs() -> impl Iterator<Item = Pair> {
    TOKENS
        .iter()
        .filter(|token| token.starts_with("terminal.ansi."))
        .map(|token| p(token, "terminal.background", ansi_floor(token)))
}

pub(super) fn ansi_floor(token: &str) -> f64 {
    if token == "terminal.ansi.black" {
        1.5
    } else {
        3.0
    }
}

/// **A line of words and the box it is drawn in** (#2522): a line `size` × `line_height` tall
/// fits inside `height`, in both densities.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Fit {
    /// The words' size, a `font-size.*` token.
    pub size: &'static str,
    /// Their line height, a `line-height.*` token.
    pub line_height: &'static str,
    /// The box's height, a `size.*` token.
    pub height: &'static str,
}

/// **Every line that has to fit in its box.** The control's box is a field (`.ui-input`): it is
/// `size.control` tall and its text is the window's words (`font-size.base`) on the body line
/// (`line-height.body`, the `:root`'s). A button is as tall, and its label sits on a line of 1
/// (`ui/button/button.css`), so it fits wherever a field's text does; a strip's largest words are a tab's
/// title (`font-size.md`, `.tabs [role="tab"]`) on the body line.
pub const FITS: &[Fit] = &[
    Fit {
        size: "font-size.base",
        line_height: "line-height.body",
        height: "size.control",
    },
    Fit {
        size: "font-size.md",
        line_height: "line-height.body",
        height: "size.chrome",
    },
];

/// The tokens the selected row (`list.selected`) must never equal: the pointer's row, the tab
/// you are on, and the window (#1672).
const SELECTED_APART_FROM: [&str; 3] = ["surface.hover", "layer.selected", "surface.base"];

/// **How far a side view's card row steps from the side panel it stands on** (#3774): a ground
/// step, held as the window's hairlines are (`border.subtle` on `surface.base` at 1.2), but over
/// a whole card's area, which reads at less. Purlis dark draws its cards 1.06 from its panels.
/// It is reported, never a pair of [`PAIRS`]: no consent floor, tier walk or person's colour acts
/// on it (#3735), so it never redraws a theme's grounds or refuses a person's colour.
pub const CARD_STEP: f64 = 1.05;

/// **A card row's edge, where it draws one, on the side panel**: a hairline, held as the
/// window's are, so a theme can draw its cards on the panel's own ground inside an edge.
pub const CARD_EDGE: f64 = 1.2;

/// What [`check`] found: nothing, for a theme whose words can be read.
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
    /// A side view's card row that does not stand apart from the side panel it is drawn on
    /// (#3774): its fill, `card`, is `ratio` from the panel's, `ground`, under [`CARD_STEP`], and
    /// it draws no edge that clears [`CARD_EDGE`] there, so its cards read as plain rows.
    CardNotApart {
        tint: Option<&'static str>,
        card: &'static str,
        ground: &'static str,
        ratio: f64,
        floor: f64,
    },
    /// A colour for selected text with the selection left to the platform (#2327): the window
    /// draws selected text only on a selection the theme colours too, so this one is never drawn.
    SelectionTextAlone,
    /// A line of [`FITS`] taller than the box it is drawn in, in `density`: `line` is the
    /// line's height and `room` the box's, in px at the default text size (to two places).
    LineDoesNotFit {
        density: Density,
        fit: Fit,
        line: f64,
        room: f64,
    },
    /// A style the theme draws in a font it brings that has no face for it (#2736): `slot` is
    /// `ui`, `mono` or `terminal`, and the browser makes the style from another of its faces.
    StyleWithoutAFace {
        slot: &'static str,
        weight: u16,
        style: Style,
    },
    /// A component part the theme sets that breaks a floor in a variant and state (#2903), so
    /// the window draws its family default there: `part` is `<family>.<part>.<variant>[.<state>]`.
    PartFallsBack {
        part: String,
        floor: String,
        ratio: Option<f64>,
    },
    /// A text tier under its floor on a ground it is drawn on (#2940), so the window draws it
    /// as the nearest colour that clears it: `drawn`, from the theme's `written`. `ground` is
    /// where it fell furthest under, at `ratio`, as written or, with `tinted`, only in a
    /// workspace's tint; `clears` is `false` only where no lightness clears every ground.
    TierFallsBack {
        tier: &'static str,
        written: String,
        drawn: String,
        ground: &'static str,
        ratio: f64,
        floor: f64,
        tinted: bool,
        clears: bool,
    },
}

/// The check, as [`super::check`] describes it.
pub(super) fn check(theme: &ResolvedTheme) -> Report {
    let mut findings = Vec::new();
    // The parts are the theme's in every tint: which tokens a card row and its panel draw is
    // read once, and measured in each.
    let card = super::parts::card_on_side(theme);
    check_one(theme, None, &mut findings);
    card_apart(theme, None, card, &mut findings);
    let platform = |token: &str| is_platform(token, theme.value(token));
    if platform("selection.background") && !platform("selection.text") {
        findings.push(Finding::SelectionTextAlone);
    }
    // A tint turns colours only, so the geometry and the faces are checked once.
    fits(Density::Comfortable, &theme.geometry, &mut findings);
    fits(Density::Compact, &theme.compact, &mut findings);
    findings.extend(
        without_a_face(theme)
            .into_iter()
            .map(|one| Finding::StyleWithoutAFace {
                slot: one.slot,
                weight: one.weight,
                style: one.style,
            }),
    );
    // A component part that breaks a floor draws its family default there (#2903).
    findings.extend(
        super::parts::fallen(theme)
            .into_iter()
            .map(|one| Finding::PartFallsBack {
                part: one.part,
                floor: one.floor,
                ratio: one.ratio,
            }),
    );
    // A text tier under its floor is drawn as the nearest colour that clears it (#2940).
    findings.extend(
        super::tiers::lifted(theme)
            .into_iter()
            .map(|one| Finding::TierFallsBack {
                tier: one.tier,
                written: one.written,
                drawn: one.drawn,
                ground: one.ground,
                ratio: one.ratio,
                floor: one.floor,
                tinted: one.tinted,
                clears: one.clears,
            }),
    );
    // A theme a workspace's colour does not tint is only ever drawn as it is.
    if !theme.tint.tabs().is_empty() {
        for one in &theme.tint.hues {
            let tinted = tint::tinted(theme, one.name);
            check_one(&tinted, Some(one.name), &mut findings);
            card_apart(&tinted, Some(one.name), card, &mut findings);
        }
    }
    Report { findings }
}

/// **Whether a side view's card row stands apart from its panel** (#3774): its fill a step from
/// the panel's ([`CARD_STEP`]), or an edge a hairline from it ([`CARD_EDGE`]). A see-through
/// panel is laid on the window's frame, a see-through card on the panel and a see-through edge on
/// the card. Held where the theme's parts, its own or inherited, choose a card row's or a side
/// panel's look ([`super::parts::card_on_side`]).
fn card_apart(
    theme: &ResolvedTheme,
    tint: Option<&'static str>,
    on: Option<super::parts::CardOnSide>,
    findings: &mut Vec<Finding>,
) {
    let Some(on) = on else {
        return;
    };
    let laid = |token: &str, under: &str| {
        let value = theme.value(token);
        if token == "transparent" || value.is_empty() || is_platform(token, value) {
            under.to_owned()
        } else {
            solid(&onto(value, under))
        }
    };
    let panel = laid(on.ground, &solid(theme.value("surface.deep")));
    let card = laid(on.card, &panel);
    let step = two_places(contrast(&card, &panel));
    // An edge is drawn over the card's own fill, and told from the panel around it.
    let edged = on
        .edge
        .is_some_and(|edge| two_places(contrast(&laid(edge, &card), &panel)) >= CARD_EDGE);
    if step < CARD_STEP && !edged {
        findings.push(Finding::CardNotApart {
            tint,
            card: on.card,
            ground: on.ground,
            ratio: step,
            floor: CARD_STEP,
        });
    }
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

/// Each line of [`FITS`] that is taller than its box in `geometry`. Every length here grows with
/// the text-size setting alike, so what fits at the default text size fits at every one.
fn fits(density: Density, geometry: &Geometry, findings: &mut Vec<Finding>) {
    let value = |token: &str| {
        geometry
            .get(token)
            .copied()
            .or_else(|| super::geometry::step_of(token).map(|step| step.classic))
            .unwrap_or_default()
    };
    for fit in FITS {
        let line = two_places(value(fit.size) * value(fit.line_height));
        let room = two_places(value(fit.height));
        if line > room {
            findings.push(Finding::LineDoesNotFit {
                density,
                fit: *fit,
                line,
                room,
            });
        }
    }
}

/// **The contrast `pair` has in `theme`**, to two places, as it is drawn: `back` laid over
/// `under` when it names one, itself laid over `beneath` when it names one, and a see-through
/// `front` laid over that. `None` when the platform draws one of them
/// ([`super::vocabulary::PLATFORM_DRAWN`]); a front left to the platform that keeps the words'
/// own colour is measured as those words ([`STANDS_IN`]); a `selected` pair is not measured
/// while the theme colours selected text.
pub(super) fn ratio(theme: &ResolvedTheme, pair: &Pair) -> Option<f64> {
    let (front, back) = drawn(theme, pair)?;
    Some(two_places(contrast(&front, &back)))
}

/// `pair`'s two colours as the screen draws them, opaque: its back laid over what it is laid on,
/// and its front laid over that and faded as the pair says. `None` when the platform draws one.
pub(super) fn drawn(theme: &ResolvedTheme, pair: &Pair) -> Option<(String, String)> {
    let (front, back) = measured(theme, pair)?;
    let back = theme.value(back);
    // From the bottom up: each layer laid over the one under it.
    let drawn = match (pair.under, pair.beneath) {
        (Some(under), Some(beneath)) => onto(back, &onto(theme.value(under), theme.value(beneath))),
        (Some(under), None) => onto(back, theme.value(under)),
        (None, _) => back.to_owned(),
    };
    let front = onto(theme.value(front), &drawn);
    let front = match pair.fade {
        Some(opacity) => mix(&front, &drawn, opacity),
        None => front,
    };
    Some((front, drawn))
}

/// The tokens `pair` is measured between in `theme`: its own, a platform-drawn front's stand-in,
/// or `None` when the platform draws one of them, or when the pair is of words selected in their
/// own colour and the theme colours selected text.
fn measured(theme: &ResolvedTheme, pair: &Pair) -> Option<(&'static str, &'static str)> {
    let unset = |token: &str| is_platform(token, theme.value(token));
    if unset(pair.back) || (pair.selected && !unset("selection.text")) {
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
pub(super) fn long(hex: &str) -> String {
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
pub(super) fn luminance(hex: &str) -> f64 {
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
pub(super) fn mix(top: &str, under: &str, opacity: f64) -> String {
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

/// Whether `hex` is drawn opaque: no alpha, or an alpha of `ff`.
pub(super) fn is_opaque(hex: &str) -> bool {
    let digits = long(hex);
    digits.len() == 6
        || digits
            .get(6..8)
            .is_some_and(|alpha| alpha.eq_ignore_ascii_case("ff"))
}

/// `hex` without its alpha: its colour drawn opaque.
pub(super) fn solid(hex: &str) -> String {
    let digits = long(hex);
    format!("#{}", digits.get(..6).unwrap_or(&digits))
}

/// `wash` laid over the opaque `under`, as the screen draws it. An opaque `wash` is itself.
pub(super) fn onto(wash: &str, under: &str) -> String {
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
