// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The tier floors** (#2940, ruling #2803·3, ADR 0098 §5 as amended 2026-10-21): no theme
//! draws a text tier under its floor.
//!
//! Each of the four text tiers ([`TEXT_TIERS`]) is held to 4.5:1 and the faint tier ([`FAINT`])
//! to 3:1 on every ground a part draws them on ([`TIER_GROUNDS`]), as the theme is and tinted by
//! every colour of its own recipe, a workspace's own colour included. The pairs are the check's
//! own [`PAIRS`], so a pair is measured in one place.
//!
//! **A tier under its floor is drawn as the nearest colour that clears it**: the theme's own
//! colour walked along its OKLab lightness, its hue kept, in the least steps that clear its floor
//! on every one of those grounds. A see-through tier keeps its alpha when that is enough, and is
//! drawn opaque when it is not. Nothing else in the theme moves, and the check names each tier it
//! lifts ([`super::Finding::TierFallsBack`]).
//!
//! **Where no lightness clears every ground** (a dark window with a light code ground, say), the
//! floor is held on the window's own surfaces: the grounds on the window's side of the middle.
//! A tier that already clears them keeps its colour, one that does not is walked until it does,
//! and the check names each ground on the other side it still falls under. A tier is never
//! moved to a colour that reads worse on the window's own surfaces.
//!
//! The tiers are held after the consent floors ([`super::floors`]), on the grounds those draw,
//! when the window draws a theme ([`super::draw`]). A theme draft's hard floors are measured
//! after the consent floors and before the tier floors ([`super::hard`]), so a draft that breaks
//! one is still not drawn.

use super::check::{FAINT, PAIRS, Pair, TEXT_TIERS, TIER_GROUNDS, ratio};
use super::{ResolvedTheme, floors, tint};

#[cfg(test)]
#[path = "tier_floors_tests.rs"]
mod tests;

/// The step, in OKLab lightness, a tier is walked by: finer than a byte of grey, so the walk
/// stops at the first byte that clears.
const STEP: f64 = 0.001;

/// One tier the floors drew as another colour, and why.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Lifted {
    /// The tier: one of [`TEXT_TIERS`] or [`FAINT`].
    pub tier: &'static str,
    /// Its colour as the theme gives it.
    pub written: String,
    /// The colour the window draws it in.
    pub drawn: String,
    /// The ground it fell furthest under: as written, or, where no lightness clears every
    /// ground, the ground off the window's own surfaces it still falls under as drawn.
    pub ground: &'static str,
    /// Its contrast there, to two places.
    pub ratio: f64,
    /// The floor it is held to.
    pub floor: f64,
    /// Whether only a workspace's tint took it under the floor.
    pub tinted: bool,
    /// Whether the colour drawn clears the floor on every ground: `false` only where no
    /// lightness does, and the floor is held on the window's own surfaces.
    pub clears: bool,
}

/// Every tier the floors hold, loudest first.
fn tiers() -> impl Iterator<Item = &'static str> {
    TEXT_TIERS.into_iter().chain([FAINT])
}

/// The pairs `tier` is held on: it, drawn straight on each of `grounds`.
fn pairs_of<'a>(
    tier: &'a str,
    grounds: &'a [&'static str],
) -> impl Iterator<Item = &'static Pair> + 'a {
    PAIRS.iter().filter(move |pair| {
        pair.front == tier
            && grounds.contains(&pair.back)
            && pair.under.is_none()
            && pair.fade.is_none()
            && !pair.selected
    })
}

/// A measure: the ratio over its floor, the pair, and the ratio.
type Low = (f64, &'static Pair, f64);

fn least(found: impl Iterator<Item = Low>) -> Option<Low> {
    found.min_by(|one, other| one.0.total_cmp(&other.0))
}

/// The nearest `tier` comes to its floor on `grounds` in `theme` as it is; `None` when nothing
/// of it is measured.
fn lowest(theme: &ResolvedTheme, tier: &str, grounds: &[&'static str]) -> Option<Low> {
    least(
        pairs_of(tier, grounds)
            .filter_map(|pair| ratio(theme, pair).map(|ratio| (ratio / pair.floor, pair, ratio))),
    )
}

/// `theme` tinted by each colour its recipe names: empty when no tint turns it.
fn named_tints(theme: &ResolvedTheme) -> Vec<ResolvedTheme> {
    if theme.tint.tabs().is_empty() {
        return Vec::new();
    }
    theme
        .tint
        .hues
        .iter()
        .map(|one| tint::tinted(theme, one.name))
        .collect()
}

/// The nearest `tier` comes to its floor on `grounds` at every whole degree, for the pairs a
/// tint can move between hues.
fn lowest_at_every_hue(theme: &ResolvedTheme, tier: &str, grounds: &[&'static str]) -> Option<Low> {
    if theme.tint.tabs().is_empty() {
        return None;
    }
    let moving: Vec<&'static Pair> = pairs_of(tier, grounds)
        .filter(|pair| floors::moves_with_hue(theme, pair))
        .collect();
    if moving.is_empty() {
        return None;
    }
    let only: Vec<&'static str> = moving.iter().flat_map(|pair| pair.tokens()).collect();
    let mut found = Vec::new();
    for degree in 0..360_u32 {
        let degree = f64::from(degree);
        let most = tint::most_at(theme, degree);
        let drawn = tint::turned_at(theme, degree, most, &only);
        found.extend(moving.iter().filter_map(|pair| {
            ratio(&drawn, pair).map(|ratio| (ratio / pair.floor, *pair, ratio))
        }));
    }
    least(found.into_iter())
}

/// The nearest `tier` comes to its floor on `grounds` in every workspace tint of `theme`'s
/// recipe: each colour it names, and every whole degree for a pair a tint can move.
fn lowest_tinted(theme: &ResolvedTheme, tier: &str, grounds: &[&'static str]) -> Option<Low> {
    let named = named_tints(theme);
    least(
        named
            .iter()
            .filter_map(|one| lowest(one, tier, grounds))
            .chain(lowest_at_every_hue(theme, tier, grounds)),
    )
}

fn under(found: Option<Low>) -> bool {
    found.is_some_and(|(_, pair, ratio)| ratio < pair.floor)
}

/// Whether `tier` clears its floor on `grounds` in `theme`, as it is and in every tint.
fn clears(theme: &ResolvedTheme, tier: &str, grounds: &[&'static str]) -> bool {
    !under(lowest(theme, tier, grounds)) && !under(lowest_tinted(theme, tier, grounds))
}

/// **The cheap first pass**: whether every tier clears every ground as written and in the named
/// tints, each tint drawn once for all of them, and no pair of any tier moves with the hue.
fn all_clear(theme: &ResolvedTheme) -> bool {
    let named = named_tints(theme);
    tiers().all(|tier| {
        !under(lowest(theme, tier, &TIER_GROUNDS))
            && named
                .iter()
                .all(|one| !under(lowest(one, tier, &TIER_GROUNDS)))
            && (named.is_empty()
                || !pairs_of(tier, &TIER_GROUNDS).any(|pair| floors::moves_with_hue(theme, pair)))
    })
}

/// **The window's own surfaces** in `theme`: the grounds on the window's side of the middle.
fn own_surfaces(theme: &ResolvedTheme) -> Vec<&'static str> {
    let side = |token: &str| tint::luminance_of(theme.value(token)).map(tint::light_side);
    let window = side("surface.base");
    TIER_GROUNDS
        .into_iter()
        .filter(|ground| side(ground).is_none() || side(ground) == window)
        .collect()
}

/// **`theme` with the tier floors held**: as it is when every tier clears its floor; otherwise
/// with each tier that does not drawn as the nearest colour that does, and what was lifted.
pub(super) fn hold(theme: &ResolvedTheme) -> (ResolvedTheme, Vec<Lifted>) {
    let mut drawn = theme.clone();
    let mut lifted = Vec::new();
    if all_clear(theme) {
        return (drawn, lifted);
    }
    for tier in tiers() {
        if clears(&drawn, tier, &TIER_GROUNDS) {
            continue;
        }
        let written = drawn.value(tier).to_owned();
        let (fell, tinted) = match lowest(&drawn, tier, &TIER_GROUNDS) {
            Some(found) if found.2 < found.1.floor => (found, false),
            _ => match lowest_tinted(&drawn, tier, &TIER_GROUNDS) {
                Some(found) => (found, true),
                None => continue,
            },
        };
        if let Some(colour) = nearest(&drawn, tier, &written, &TIER_GROUNDS) {
            drawn.values.insert(tier, colour.clone());
            lifted.push(Lifted {
                tier,
                written,
                drawn: colour,
                ground: fell.1.back,
                ratio: fell.2,
                floor: fell.1.floor,
                tinted,
                clears: true,
            });
            continue;
        }
        // No lightness clears every ground: the floor is held on the window's own surfaces,
        // and the tier keeps its colour when it clears them already.
        let own = own_surfaces(&drawn);
        if !clears(&drawn, tier, &own)
            && let Some(colour) = nearest(&drawn, tier, &written, &own)
        {
            drawn.values.insert(tier, colour);
        }
        let others: Vec<&'static str> = TIER_GROUNDS
            .into_iter()
            .filter(|ground| !own.contains(ground))
            .collect();
        let outlier = lowest(&drawn, tier, &others)
            .filter(|found| found.2 < found.1.floor)
            .or_else(|| lowest_tinted(&drawn, tier, &others))
            .unwrap_or(fell);
        lifted.push(Lifted {
            tier,
            written,
            drawn: drawn.value(tier).to_owned(),
            ground: outlier.1.back,
            ratio: outlier.2,
            floor: outlier.1.floor,
            tinted,
            clears: false,
        });
    }
    (drawn, lifted)
}

/// The tiers `theme` draws as another colour, as the window draws it: the consent floors held
/// first, as [`super::draw`] holds them.
pub(super) fn lifted(theme: &ResolvedTheme) -> Vec<Lifted> {
    hold(&floors::hold(theme).0).1
}

/// **The colour nearest `written` that clears `tier`'s floor on `grounds`** in `theme`:
/// `written` walked along its OKLab lightness both ways, a step at a time, its alpha kept and
/// then dropped. `None` when no lightness clears them.
fn nearest(
    theme: &ResolvedTheme,
    tier: &'static str,
    written: &str,
    grounds: &[&'static str],
) -> Option<String> {
    let digits = written.strip_prefix('#').unwrap_or(written);
    // A `#rgb` or `#rgba` is walked as its long form.
    let long = if digits.len() <= 4 {
        format!(
            "#{}",
            digits.chars().flat_map(|d| [d, d]).collect::<String>()
        )
    } else {
        written.to_owned()
    };
    let mut starts = vec![long.clone()];
    if long.len() == 9 {
        starts.push(long[..7].to_owned());
    }
    let mut trial = theme.clone();
    let mut seen = std::collections::BTreeSet::from([long.clone()]);
    for start in starts {
        let Some([from, ..]) = tint::oklab_of(&start) else {
            continue;
        };
        for step in 1..=((1.0 / STEP) as u32) {
            let away = f64::from(step) * STEP;
            for lightness in [from + away, from - away] {
                if !(0.0..=1.0).contains(&lightness) {
                    continue;
                }
                let Some(colour) = tint::at_lightness(&start, lightness) else {
                    continue;
                };
                if !seen.insert(colour.clone()) {
                    continue;
                }
                trial.values.insert(tier, colour.clone());
                if lowest(&trial, tier, grounds).is_some_and(|(margin, ..)| margin >= 1.0)
                    && clears(&trial, tier, grounds)
                {
                    return Some(colour);
                }
            }
        }
    }
    None
}
