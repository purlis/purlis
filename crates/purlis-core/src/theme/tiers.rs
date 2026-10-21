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
//! drawn opaque when it is not. Where no lightness clears every ground (grounds on both sides of
//! the middle), the tier is drawn at the lightness that comes nearest, and the check says it
//! still falls under. Nothing else in the theme moves, and the check names each tier it lifts
//! ([`super::Finding::TierFallsBack`]).
//!
//! The tiers are held after the consent floors ([`super::floors`]), on the grounds those draw,
//! when the window draws a theme ([`super::draw`]). A theme draft's hard floors are measured on
//! the theme as written ([`super::hard`]), so a draft that breaks one is still not drawn.

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
    /// The ground it fell furthest under.
    pub ground: &'static str,
    /// Its contrast there as written, to two places.
    pub ratio: f64,
    /// The floor it is held to.
    pub floor: f64,
    /// Whether only a workspace's tint took it under the floor.
    pub tinted: bool,
    /// Whether the colour drawn clears the floor on every ground: `false` only where no
    /// lightness does.
    pub clears: bool,
}

/// Every tier the floors hold, loudest first.
fn tiers() -> impl Iterator<Item = &'static str> {
    TEXT_TIERS.into_iter().chain([FAINT])
}

/// The pairs `tier` is held on: it, drawn straight on each of [`TIER_GROUNDS`].
fn pairs_of(tier: &str) -> impl Iterator<Item = &'static Pair> + '_ {
    PAIRS.iter().filter(move |pair| {
        pair.front == tier
            && TIER_GROUNDS.contains(&pair.back)
            && pair.under.is_none()
            && pair.fade.is_none()
            && !pair.selected
    })
}

/// The nearest `tier` comes to its floor in `theme` as it is: the least of its ratio over its
/// floor on its grounds, with that pair and ratio; `None` when nothing of it is measured.
fn lowest(theme: &ResolvedTheme, tier: &str) -> Option<(f64, &'static Pair, f64)> {
    pairs_of(tier)
        .filter_map(|pair| ratio(theme, pair).map(|ratio| (ratio / pair.floor, pair, ratio)))
        .min_by(|one, other| one.0.total_cmp(&other.0))
}

/// The nearest `tier` comes to its floor in every workspace tint of `theme`'s recipe: each
/// colour it names, and every whole degree for a pair a tint can move between hues; `None`
/// when no tint moves it.
fn lowest_tinted(theme: &ResolvedTheme, tier: &str) -> Option<(f64, &'static Pair, f64)> {
    if theme.tint.tabs().is_empty() {
        return None;
    }
    let mut found = theme
        .tint
        .hues
        .iter()
        .filter_map(|one| lowest(&tint::tinted(theme, one.name), tier))
        .collect::<Vec<_>>();
    let moving: Vec<&'static Pair> = pairs_of(tier)
        .filter(|pair| floors::moves_with_hue(theme, pair))
        .collect();
    if !moving.is_empty() {
        let only: Vec<&'static str> = moving.iter().flat_map(|pair| pair.tokens()).collect();
        for degree in 0..360_u32 {
            let degree = f64::from(degree);
            let most = tint::apart_chroma(theme, degree);
            let drawn = tint::turned_at(theme, degree, most, &only);
            found.extend(moving.iter().filter_map(|pair| {
                ratio(&drawn, pair).map(|ratio| (ratio / pair.floor, *pair, ratio))
            }));
        }
    }
    found
        .into_iter()
        .min_by(|one, other| one.0.total_cmp(&other.0))
}

/// Whether `tier` clears its floor on every ground in `theme`, as it is and in every tint.
fn clears(theme: &ResolvedTheme, tier: &str) -> bool {
    let under =
        |found: Option<(f64, &Pair, f64)>| found.is_some_and(|(_, pair, ratio)| ratio < pair.floor);
    !under(lowest(theme, tier)) && !under(lowest_tinted(theme, tier))
}

/// **`theme` with the tier floors held**: as it is when every tier clears its floor; otherwise
/// with each tier that does not drawn as the nearest colour that does, and what was lifted.
pub(super) fn hold(theme: &ResolvedTheme) -> (ResolvedTheme, Vec<Lifted>) {
    let mut drawn = theme.clone();
    let mut lifted = Vec::new();
    for tier in tiers() {
        if clears(&drawn, tier) {
            continue;
        }
        let written = drawn.value(tier).to_owned();
        let (fell, tinted) = match lowest(&drawn, tier) {
            Some(found) if found.2 < found.1.floor => (found, false),
            _ => match lowest_tinted(&drawn, tier) {
                Some(found) => (found, true),
                None => continue,
            },
        };
        let (colour, cleared) = nearest(&drawn, tier, &written);
        drawn.values.insert(tier, colour.clone());
        lifted.push(Lifted {
            tier,
            written,
            drawn: colour,
            ground: fell.1.back,
            ratio: fell.2,
            floor: fell.1.floor,
            tinted,
            clears: cleared,
        });
    }
    (drawn, lifted)
}

/// The tiers `theme` draws as another colour, as the window draws it: the consent floors held
/// first, as [`super::draw`] holds them.
pub(super) fn lifted(theme: &ResolvedTheme) -> Vec<Lifted> {
    hold(&floors::hold(theme).0).1
}

/// **The colour nearest `written` that clears `tier`'s floor** in `theme`, and whether one
/// does: `written` walked along its OKLab lightness both ways, a step at a time, its alpha kept
/// and then dropped. Where none clears, the lightness that comes nearest to the floor.
fn nearest(theme: &ResolvedTheme, tier: &'static str, written: &str) -> (String, bool) {
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
    let mut best: Option<(f64, String)> = None;
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
                let Some((margin, ..)) = lowest(&trial, tier) else {
                    continue;
                };
                if margin >= 1.0 && clears(&trial, tier) {
                    return (colour, true);
                }
                if best.as_ref().is_none_or(|(most, _)| margin > *most) {
                    best = Some((margin, colour));
                }
            }
        }
    }
    (
        best.map_or_else(|| written.to_owned(), |(_, colour)| colour),
        false,
    )
}
