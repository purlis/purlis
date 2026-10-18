// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The consent floors** (ADR 0097 §6, ADR 0041 as amended 2026-10-17): what holds for the
//! asks, the danger colours and the state marks whatever a theme says, every time a theme is
//! drawn.
//!
//! A theme may set every token, the consent surfaces' and the state marks' included. Five floors
//! hold for them:
//!
//! - **an ask contrasts with its surface** ([`Floor::Ask`]): an ask's words on the dialog it is
//!   asked in, its answers' words on their buttons, what it says on the danger wash, and the
//!   needs-you count's number on its chip;
//! - **Allow is distinguishable from Keep blocked** ([`Floor::AllowApart`]): the two carry equal
//!   weight on the control's own ground, and Allow is edged in `accent.base` where Keep blocked
//!   is edged in `border.strong`. The two edges are at least [`ALLOW_APART`] apart as OKLab draws
//!   colour, as the theme writes them, and the Allow's edge contrasts with its ground, at rest and
//!   under the pointer;
//! - **the danger colours contrast with what they are drawn on** ([`Floor::Danger`]): the
//!   trouble Notice's words on the danger surface, and the danger colour on each ground the
//!   window draws it on;
//! - **danger is distinguishable from primary** ([`Floor::DangerApart`]): the danger colour and
//!   the accent are at least [`DANGER_APART`] apart as OKLab draws colour, as the theme writes
//!   them;
//! - **the state marks meet the mark floor** ([`Floor::StateMarks`]): every state's colour and
//!   the needs-you mark on each ground they are drawn on.
//!
//! The contrast floors are pairs of the check's own [`PAIRS`], held to the floor the check holds
//! each to, so a pair is measured in one place. Each holds as the theme is and tinted by every
//! colour a workspace can name, as [`super::check`] measures.
//!
//! **A theme that breaks one is drawn with purlis's own values for those tokens**: Purlis
//! Classic's of the same appearance, for the consent and state tokens of each pair that fails.
//! When Classic's colour still fails on the ground the theme draws it on, the ground is drawn as
//! Classic's too. The rest of the theme draws as written, and [`Held`] says which floors and
//! which tokens, for one Notice per theme.

use std::collections::BTreeSet;

use super::check::{PAIRS, Pair, ratio};
use super::{Appearance, ResolvedTheme, built_in_for, tint};

/// One consent floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Floor {
    /// An ask contrasts with its surface.
    Ask,
    /// Allow is distinguishable from Keep blocked.
    AllowApart,
    /// The danger colours contrast with what they are drawn on.
    Danger,
    /// Danger is distinguishable from primary.
    DangerApart,
    /// The state marks meet the mark floor.
    StateMarks,
}

impl Floor {
    /// The floor in the words a Notice names it by.
    pub fn says(self) -> &'static str {
        match self {
            Self::Ask => "an ask contrasts with its surface",
            Self::AllowApart => "Allow is distinguishable from Keep blocked",
            Self::Danger => "the danger colours contrast with what they are drawn on",
            Self::DangerApart => "danger is distinguishable from primary",
            Self::StateMarks => "the state marks meet the mark floor",
        }
    }
}

/// **How far apart danger and primary are, at least**: the distance between `danger.base` and
/// `accent.base` in OKLab (Björn Ottosson's), where 1 is the distance from black to white and a
/// difference of about 0.02 is the least a person can see. 0.1 is five times that: two colours
/// told apart at a glance, not on a second look.
pub const DANGER_APART: f64 = 0.1;

/// **How far apart Allow's edge and Keep blocked's are, at least**: the distance between
/// `accent.base` and `border.strong` in OKLab, held as [`DANGER_APART`] is, for the same reason:
/// two answers told apart at a glance, not on a second look.
pub const ALLOW_APART: f64 = 0.1;

/// The ask's pairs, by `front` on `back`: its words on the dialog it is asked in, its answers'
/// words on their buttons (Allow's and Keep blocked's alike) at rest and under the pointer, what
/// it says on the danger wash, and the needs-you count on its chip.
const ASK: [(&str, &str); 5] = [
    ("text.primary", "surface.overlay"),
    ("text.primary", "control.base"),
    ("text.primary", "control.hover"),
    ("text.primary", "danger.wash"),
    ("needs-you.text", "needs-you.base"),
];

/// The pairs that keep Allow apart from Keep blocked, by `front` on `back`: the Allow's edge on
/// its own ground, at rest and under the pointer, so the edge that tells it apart is seen.
const ALLOW: [(&str, &str); 2] = [
    ("accent.base", "control.base"),
    ("accent.base", "control.hover"),
];

/// Whether `token` is a consent surface's or a state mark's: what purlis draws its own value of
/// first when a floor fails.
fn is_consent(token: &str) -> bool {
    ["danger.", "needs-you.", "state."]
        .iter()
        .any(|prefix| token.starts_with(prefix))
}

/// The floor a pair of [`PAIRS`] is held to as a consent floor, if it is one.
fn floor_of(pair: &Pair) -> Option<Floor> {
    if ASK.contains(&(pair.front, pair.back)) {
        Some(Floor::Ask)
    } else if ALLOW.contains(&(pair.front, pair.back)) {
        Some(Floor::AllowApart)
    } else if pair.front.starts_with("danger.") {
        Some(Floor::Danger)
    } else if pair.front.starts_with("state.") || pair.front == "needs-you.base" {
        Some(Floor::StateMarks)
    } else {
        None
    }
}

/// Every pair of [`PAIRS`] that is a consent floor, with its floor.
pub fn pairs() -> impl Iterator<Item = (Floor, &'static Pair)> {
    PAIRS
        .iter()
        .filter_map(|pair| floor_of(pair).map(|floor| (floor, pair)))
}

/// What holding the floors put right: which floors the theme broke, and the tokens drawn as
/// purlis's own in its place.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Held {
    /// The theme's name, as the Notice names it.
    pub theme: String,
    /// The floors it broke, each once.
    pub floors: Vec<Floor>,
    /// The tokens drawn as Purlis Classic's of the same appearance.
    pub tokens: Vec<&'static str>,
}

impl Held {
    /// The Notice's sentence: the theme, the floors it breaks, and what purlis draws instead.
    pub fn said(&self) -> String {
        let floors: Vec<&str> = self.floors.iter().map(|floor| floor.says()).collect();
        format!(
            "{} breaks the consent floors ({}), so purlis draws its own {}; the rest of the \
             theme draws as written",
            self.theme,
            floors.join("; "),
            self.tokens.join(", "),
        )
    }
}

/// One failure: the floor, and the tokens it is between, a wash's ground included.
struct Failure {
    floor: Floor,
    tokens: Vec<&'static str>,
}

/// Every consent floor `theme` breaks: the contrast floors as it is and tinted by every
/// workspace colour, and Allow apart from Keep blocked and danger apart from primary as the
/// theme writes them.
fn failures(theme: &ResolvedTheme) -> Vec<Failure> {
    let mut found = Vec::new();
    let tints = tint::PALETTE
        .iter()
        .map(|(colour, _)| tint::tinted(theme, colour));
    for drawn in std::iter::once(theme.clone()).chain(tints) {
        for (floor, pair) in pairs() {
            if ratio(&drawn, pair).is_some_and(|ratio| ratio < pair.floor) {
                found.push(Failure {
                    floor,
                    tokens: [Some(pair.front), Some(pair.back), pair.under]
                        .into_iter()
                        .flatten()
                        .collect(),
                });
            }
        }
    }
    if allow_apart(theme) < ALLOW_APART {
        found.push(Failure {
            floor: Floor::AllowApart,
            tokens: vec!["accent.base", "border.strong"],
        });
    }
    if apart(theme) < DANGER_APART {
        found.push(Failure {
            floor: Floor::DangerApart,
            tokens: vec!["danger.base", "accent.base"],
        });
    }
    found
}

/// How far two colours are in OKLab; none at all for a value that is not a colour.
fn apart_of(one: &str, other: &str) -> f64 {
    let (Some(one), Some(other)) = (tint::oklab_of(one), tint::oklab_of(other)) else {
        return 0.0;
    };
    one.iter()
        .zip(other)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// How far `danger.base` is from `accent.base` in OKLab, as the theme writes them.
pub(super) fn apart(theme: &ResolvedTheme) -> f64 {
    apart_of(theme.value("danger.base"), theme.value("accent.base"))
}

/// How far Allow's edge, `accent.base`, is from Keep blocked's, `border.strong`, in OKLab, as
/// the theme writes them.
pub(super) fn allow_apart(theme: &ResolvedTheme) -> f64 {
    apart_of(theme.value("accent.base"), theme.value("border.strong"))
}

/// **`theme` with the consent floors held**: as it is when it clears them; otherwise with
/// Purlis Classic's values of its appearance for the tokens of the pairs that fail, and what was
/// put right.
pub(super) fn hold(theme: &ResolvedTheme) -> (ResolvedTheme, Option<Held>) {
    let ours = built_in_for(theme.appearance);
    let mut drawn = theme.clone();
    let mut floors = BTreeSet::new();
    let mut taken: BTreeSet<&'static str> = BTreeSet::new();
    loop {
        let failing = failures(&drawn);
        if failing.is_empty() {
            break;
        }
        // Each round decides from what was purlis's before it, so a pair failing in several
        // tints is one failure and its ground waits for the round after its consent colour.
        let mut take: BTreeSet<&'static str> = BTreeSet::new();
        for failure in &failing {
            floors.insert(failure.floor);
            let open: Vec<&'static str> = failure
                .tokens
                .iter()
                .copied()
                .filter(|token| !taken.contains(token))
                .collect();
            let consent: Vec<&'static str> = open
                .iter()
                .copied()
                .filter(|token| is_consent(token))
                .collect();
            // Purlis's own consent colour first; the ground too only once that is not enough.
            take.extend(if consent.is_empty() { open } else { consent });
        }
        // Every token a failing floor names is purlis's already: there is nothing more to draw.
        if take.is_empty() {
            break;
        }
        for token in take {
            drawn.values.insert(token, ours.value(token).to_owned());
            taken.insert(token);
        }
    }
    if floors.is_empty() {
        return (drawn, None);
    }
    // A token the theme already gave purlis's value is not one it is drawn without.
    let tokens = taken
        .into_iter()
        .filter(|token| theme.value(token) != drawn.value(token))
        .collect();
    let held = Held {
        theme: theme_name(theme),
        floors: floors.into_iter().collect(),
        tokens,
    };
    (drawn, Some(held))
}

/// What a Notice calls a theme: its name, and its appearance's id when it has several.
fn theme_name(theme: &ResolvedTheme) -> String {
    let tag = match theme.appearance {
        Appearance::Dark => "dark",
        Appearance::Light => "light",
    };
    if theme.id == tag {
        theme.name.clone()
    } else {
        format!("{} ({})", theme.name, theme.id)
    }
}
