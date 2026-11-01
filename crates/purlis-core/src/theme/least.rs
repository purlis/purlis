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
//! **Words come first.** Marks are raised before words in each round. Where a colour is both a
//! mark on the window and the ground of words (the needs-you count's chip) and the two cannot be
//! met on their sides, they swap: dark words on a light mark. A grey tier is raised as far as the
//! tier above it and never past it, so the tiers keep their order.
//!
//! **The consent floors are held at the raised floors** ([`super::floors`]): a consent colour
//! raised under a consent rule is drawn as purlis's own, raised.
//!
//! **It takes effect everywhere a theme's colours are drawn**: the tokens the window writes, the
//! terminal's colours, and the component parts' stylesheet, whose reading floors are raised the
//! same way ([`Least::floor`]), so a part a theme styles below the raised floor draws its family
//! default. The consent floors are held after it ([`super::Drawn::under`]).

use super::ResolvedTheme;
use super::check::{self, PAIRS, Pair};
use super::vocabulary::{drawn_alike, is_platform};

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

/// How many steps a colour takes from where it is to white or black, at most.
const STEPS: u32 = 25;

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
    // What follows a base is drawn in whatever the base is raised to (#3098, #3100), so the base
    // is raised on the grounds its follower stands on too, or the copy would lower the follower.
    let following = drawn_alike(&theme.values);
    let for_followers: Vec<Pair> = pairs
        .iter()
        .filter_map(|pair| {
            following
                .iter()
                .find(|(token, _)| *token == pair.front)
                .map(|(_, base)| Pair {
                    front: *base,
                    ..*pair
                })
        })
        .collect();
    pairs.extend(for_followers);
    // Marks first, so words are raised last in each round and their floor is the one held.
    pairs.sort_by(|a, b| a.floor.total_cmp(&b.floor));
    for _ in 0..ROUNDS {
        let mut moved = false;
        for pair in &pairs {
            moved |= lift(&mut drawn, pair, &pairs);
        }
        for (token, base) in &following {
            let raised = drawn.value(base).to_owned();
            drawn.values.insert(*token, raised);
        }
        if !moved {
            break;
        }
    }
    drawn
}

/// How many of `pairs` are under their raised floor in `theme`.
fn under(theme: &ResolvedTheme, pairs: &[Pair]) -> usize {
    pairs
        .iter()
        .filter(|pair| {
            check::ratio(theme, pair).is_some_and(|ratio| ratio < Least::More.floor(pair.floor))
        })
        .count()
}

/// Whether `token` is a ground of the window's, which every word is measured on: never moved.
fn a_ground(token: &str) -> bool {
    check::TIER_GROUNDS.contains(&token)
        || ["terminal.background", "window.background"].contains(&token)
}

/// Moves `pair`'s front away from its ground until it clears its raised floor, and then, when it
/// could not, its back the other way unless the back is a ground of the window's. Where the back
/// had to move, the two may swap sides instead (light words on a dark mark become dark words on
/// a light one), when that leaves fewer pairs under their raised floors. Whether anything moved.
fn lift(theme: &mut ResolvedTheme, pair: &Pair, pairs: &[Pair]) -> bool {
    let floor = Least::More.floor(pair.floor);
    let clears =
        |theme: &ResolvedTheme| check::ratio(theme, pair).is_none_or(|ratio| ratio >= floor);
    if is_platform(pair.front, theme.value(pair.front)) || clears(theme) {
        return false;
    }
    let Some((shown, ground)) = check::drawn(theme, pair) else {
        return false;
    };
    // Away from each other: the lighter of the two lighter, the darker darker.
    let lighter = check::luminance(&shown) >= check::luminance(&ground);
    let (light, dark) = ("#ffffff", "#000000");
    let (front_to, back_to) = if lighter {
        (light, dark)
    } else {
        (dark, light)
    };
    let movable_back = !a_ground(pair.back) && !is_platform(pair.back, theme.value(pair.back));
    let before = theme.clone();
    step(theme, pair, pair.front, front_to, floor);
    let front_alone = clears(theme);
    if !front_alone && movable_back {
        step(theme, pair, pair.back, back_to, floor);
    }
    // Where the back had to move too, the two swapped sides may leave the window's other pairs
    // better off: the one that leaves fewer under their raised floors is drawn.
    if !front_alone && movable_back {
        let mut swapped = before.clone();
        step(&mut swapped, pair, pair.front, back_to, floor);
        if !clears(&swapped) {
            step(&mut swapped, pair, pair.back, front_to, floor);
        }
        if clears(&swapped) && under(&swapped, pairs) < under(theme, pairs) {
            *theme = swapped;
        }
    }
    theme.values != before.values
}

/// **The tier above each grey tier**: a tier is raised as far as it at most, never past it, so
/// the tiers keep their order and muted words are never drawn louder than secondary ones.
const ABOVE: [(&str, &str); 4] = [
    ("text.secondary", "text.primary"),
    ("text.muted", "text.secondary"),
    ("text.quiet", "text.muted"),
    ("text.faint", "text.quiet"),
];

/// Moves `token` toward `toward`, a step at a time, until `pair` clears `floor`, all the way, or
/// as far as the grey tier above it ([`ABOVE`]), which it then takes; whether it moved.
fn step(
    theme: &mut ResolvedTheme,
    pair: &Pair,
    token: &'static str,
    toward: &str,
    floor: f64,
) -> bool {
    let original = theme.value(token).to_owned();
    let above = ABOVE
        .iter()
        .find(|(tier, _)| *tier == token)
        .map(|(_, above)| theme.value(above).to_owned());
    for at in 1..=STEPS {
        let candidate = toward_keeping_alpha(&original, toward, f64::from(at) / f64::from(STEPS));
        // A tier goes as far as the tier above it and no farther, on whichever side it is.
        if let Some(above) = &above {
            let (was, now, top) = (
                check::luminance(&original),
                check::luminance(&candidate),
                check::luminance(above),
            );
            if (was < top && now > top) || (was > top && now < top) {
                theme.values.insert(token, above.clone());
                break;
            }
        }
        theme.values.insert(token, candidate);
        if check::ratio(theme, pair).is_some_and(|ratio| ratio >= floor) {
            break;
        }
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
