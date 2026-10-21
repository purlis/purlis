// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The minimum contrast a person asks for** (#2925, ruling #2814·7): purlis's own setting,
//! layered on the system's Increase contrast.
//!
//! The engine holds every pair the check measures at a floor: words at 4.5:1 and marks at 3:1
//! (WCAG 2.2 AA, [`super::check`]). **More contrast raises those floors**: words to 7:1, WCAG's
//! enhanced contrast (1.4.6), and marks to 4.5:1. A pair under its raised floor has its front
//! drawn lighter on a darker ground, or darker on a lighter one, in small steps toward white or
//! black, until it clears the floor or can go no further. Each step mixes toward white or black,
//! so a colour keeps its hue as it brightens or darkens, and a wash keeps its alpha. A front that
//! can go no further has its back moved the other way, unless the back is one of the window's
//! own grounds, which every word is measured on.
//!
//! **Words come first.** Marks are raised before words in each round, so where one colour is a
//! mark on the window and the ground of words (the needs-you count's chip), the words' floor is
//! the one held; such a mark keeps at least its standard floor, which the consent floors hold.
//!
//! **It takes effect everywhere a theme's colours are drawn**: the tokens the window writes, the
//! terminal's colours, and the component parts' stylesheet, whose reading floors are raised the
//! same way ([`Least::floor`]), so a part a theme styles below the raised floor draws its family
//! default. The consent floors are held after it ([`super::Drawn::under`]).

use super::ResolvedTheme;
use super::check::{self, PAIRS, Pair};
use super::vocabulary::is_platform;

/// The contrast held, at least.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Least {
    /// The floors as the engine holds them: words at 4.5:1, marks at 3:1.
    #[default]
    Standard,
    /// The floors raised: words at 7:1, marks at 4.5:1.
    More,
}

/// Words' floor with more contrast: WCAG's enhanced contrast for text (1.4.6).
pub const WORDS_MORE: f64 = 7.0;

/// Marks' floor with more contrast: what AA asks of words, for a mark that has to be told apart.
pub const MARKS_MORE: f64 = 4.5;

impl Least {
    /// **The floor a pair held at `floor` is held at here**: as it is for the standard
    /// contrast; with more, words' 4.5:1 is 7:1 and marks' 3:1 is 4.5:1. A floor under a mark's
    /// (a scrollbar on the terminal, the terminal's black on its background) is a hint's, which
    /// WCAG does not hold, and it stays as it is.
    pub fn floor(self, floor: f64) -> f64 {
        match self {
            Self::Standard => floor,
            Self::More if floor >= 4.5 => WORDS_MORE.max(floor),
            Self::More if floor >= 3.0 => MARKS_MORE,
            Self::More => floor,
        }
    }
}

/// How far each step moves a colour toward white or black.
const STEP: f64 = 0.04;

/// How many times every pair is measured again: raising one front can lower another pair that
/// draws it as its ground, so the pairs are raised again, and the last round's colours are drawn.
const ROUNDS: usize = 3;

/// **`theme` with every pair the check measures raised to its raised floor** ([`Least::More`]),
/// and the ANSI colours on the terminal's background with them. A pair the platform draws is
/// not measured, so it is not raised.
pub fn raise(theme: &ResolvedTheme) -> ResolvedTheme {
    let mut drawn = theme.clone();
    drawn.least = Least::More;
    let mut pairs: Vec<Pair> = PAIRS.iter().copied().chain(check::ansi_pairs()).collect();
    // Marks first, so words are raised last in each round and their floor is the one held.
    pairs.sort_by(|a, b| a.floor.total_cmp(&b.floor));
    for _ in 0..ROUNDS {
        let mut moved = false;
        for pair in &pairs {
            moved |= lift(&mut drawn, pair);
        }
        if !moved {
            break;
        }
    }
    drawn
}

/// Moves `pair`'s front away from its ground until it clears its raised floor, and then, when it
/// could not, its back the other way unless the back is a ground of the window's; whether either
/// moved.
fn lift(theme: &mut ResolvedTheme, pair: &Pair) -> bool {
    let floor = Least::More.floor(pair.floor);
    if is_platform(pair.front, theme.value(pair.front))
        || check::ratio(theme, pair).is_none_or(|ratio| ratio >= floor)
    {
        return false;
    }
    let Some((shown, ground)) = check::drawn(theme, pair) else {
        return false;
    };
    // Away from each other: the lighter of the two lighter, the darker darker.
    let lighter = check::luminance(&shown) >= check::luminance(&ground);
    let (front_to, back_to) = if lighter {
        ("#ffffff", "#000000")
    } else {
        ("#000000", "#ffffff")
    };
    let moved = step(theme, pair, pair.front, front_to, floor);
    let a_ground = check::TIER_GROUNDS.contains(&pair.back)
        || ["terminal.background", "window.background"].contains(&pair.back);
    let moved_back = check::ratio(theme, pair).is_some_and(|ratio| ratio < floor)
        && !a_ground
        && !is_platform(pair.back, theme.value(pair.back))
        && step(theme, pair, pair.back, back_to, floor);
    moved || moved_back
}

/// Moves `token` toward `toward` in [`STEP`]s until `pair` clears `floor`, or all the way;
/// whether it moved.
fn step(
    theme: &mut ResolvedTheme,
    pair: &Pair,
    token: &'static str,
    toward: &str,
    floor: f64,
) -> bool {
    let original = theme.value(token).to_owned();
    let mut amount = STEP;
    while amount <= 1.0 + f64::EPSILON {
        theme
            .values
            .insert(token, toward_keeping_alpha(&original, toward, amount));
        if check::ratio(theme, pair).is_some_and(|ratio| ratio >= floor) {
            break;
        }
        amount += STEP;
    }
    theme.value(token) != original
}

/// `colour` mixed `amount` of the way toward the opaque `toward`, its alpha kept.
fn toward_keeping_alpha(colour: &str, toward: &str, amount: f64) -> String {
    let long = check::long(colour);
    let rgb = format!("#{}", long.get(..6).unwrap_or("000000"));
    let mixed = check::mix(toward, &rgb, amount.min(1.0));
    match long.get(6..8) {
        Some(alpha) => format!("{mixed}{alpha}"),
        None => mixed,
    }
}
