// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The hard floors** (spec #2313, #2332, ADR 0098 §5): the few rules that keep the window
//! readable whatever a theme says.
//!
//! - **primary text is readable on every surface** ([`HardFloor::PrimaryText`]): `text.primary`
//!   on each ground the check measures it on;
//! - **the focus ring is seen on its surfaces** ([`HardFloor::FocusRing`]): `focus.ring` on each
//!   ground the check measures it on;
//! - **the terminal's words are readable** ([`HardFloor::TerminalText`]): `terminal.foreground`
//!   on the terminal's background and on its find matches.
//!
//! Each is held to [`LEAST`], 3:1, as it is drawn: the consent floors held first
//! ([`super::draw`]), as the theme is and tinted by every colour of its own recipe. The pairs are
//! the check's own [`PAIRS`], so a pair is measured in one place; the hard floor is the lower
//! line under the check's warning.
//!
//! **A theme draft that breaks one is not drawn** (`super::drafts`): the last good version stays,
//! and the chat that wrote it is told which floor. The theme check reports every theme's broken
//! hard floors, in every combination, as findings of their own.

use super::check::{PAIRS, Pair, ratio};
use super::{ResolvedTheme, tint};

/// The least contrast a hard floor holds: 3:1, WCAG's floor for large text and for a control's
/// outline.
pub const LEAST: f64 = 3.0;

/// One hard floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HardFloor {
    /// Primary text is readable on every surface.
    PrimaryText,
    /// The focus ring is seen on its surfaces.
    FocusRing,
    /// The terminal's words are readable.
    TerminalText,
}

impl HardFloor {
    /// Every hard floor, in the order they are said.
    pub const ALL: [Self; 3] = [Self::PrimaryText, Self::FocusRing, Self::TerminalText];

    /// The floor in the words the check and a chat are told it by.
    pub fn says(self) -> &'static str {
        match self {
            Self::PrimaryText => "primary text is readable on every surface",
            Self::FocusRing => "the focus ring is seen on its surfaces",
            Self::TerminalText => "the terminal's words are readable",
        }
    }

    /// The token whose pairs this floor holds.
    fn front(self) -> &'static str {
        match self {
            Self::PrimaryText => "text.primary",
            Self::FocusRing => "focus.ring",
            Self::TerminalText => "terminal.foreground",
        }
    }

    /// The pairs of [`PAIRS`] this floor holds.
    pub fn pairs(self) -> impl Iterator<Item = &'static Pair> {
        PAIRS.iter().filter(move |pair| pair.front == self.front())
    }
}

/// One hard floor a theme breaks, and where.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Broken {
    pub floor: HardFloor,
    /// The token drawn on `back`.
    pub front: &'static str,
    /// The ground it is drawn on.
    pub back: &'static str,
    /// What `back` is laid over, for a wash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub under: Option<&'static str>,
    /// The contrast it has, to two places.
    pub ratio: f64,
    /// The workspace colour it was tinted by, `None` for the theme as it is.
    pub tint: Option<&'static str>,
}

impl Broken {
    /// The sentence that names it: the floor, the pair, its ratio and where.
    pub fn said(&self) -> String {
        let tint = self
            .tint
            .map(|one| format!(" in the {one} tint"))
            .unwrap_or_default();
        format!(
            "{}: {} on {} is {:.2}, under {LEAST}{tint}",
            self.floor.says(),
            self.front,
            self.back,
            self.ratio
        )
    }
}

/// **Every hard floor `drawn` breaks**: `drawn` is the appearance as the window draws it, the
/// consent floors already held. Each pair as it is and tinted by every colour of the theme's own
/// recipe; one entry for each pair and tint that is under [`LEAST`]. Empty for a theme that
/// holds them.
pub fn broken(drawn: &ResolvedTheme) -> Vec<Broken> {
    let mut found = Vec::new();
    let tints = if drawn.tint.tabs().is_empty() {
        Vec::new()
    } else {
        drawn
            .tint
            .hues
            .iter()
            .map(|one| (Some(one.name), tint::tinted(drawn, one.name)))
            .collect()
    };
    for (tint, theme) in std::iter::once((None, drawn.clone())).chain(tints) {
        for floor in HardFloor::ALL {
            for pair in floor.pairs() {
                let Some(ratio) = ratio(&theme, pair) else {
                    continue;
                };
                if ratio < LEAST {
                    found.push(Broken {
                        floor,
                        front: pair.front,
                        back: pair.back,
                        under: pair.under,
                        ratio,
                        tint,
                    });
                }
            }
        }
    }
    found
}
