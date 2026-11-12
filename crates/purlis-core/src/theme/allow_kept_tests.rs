// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Allow against the primary is kept by what it reads** (#3597): [`allow_from_primary`] keeps
//! its answer by the values of [`allow_reads`], so those must be every token the measure reads.
//! A theme cut down to them answers as the whole theme does, as written and in every tint of its
//! own, for each built-in appearance; and the kept answer is the measure's.

use super::super::{ResolvedTheme, classic, purlis, tint};
use super::{allow_from_primary, allow_from_primary_measured, allow_reads};

/// Every built-in appearance, as written and in each tint of its own recipe.
fn every_drawn() -> Vec<ResolvedTheme> {
    let mut drawn = Vec::new();
    for theme in [purlis(), classic()] {
        for look in &theme.appearances {
            drawn.push(look.clone());
            for hue in &look.tint.hues {
                drawn.push(tint::tinted(look, hue.name));
            }
        }
    }
    drawn
}

#[test]
fn a_theme_cut_down_to_what_allow_reads_measures_as_the_whole_theme() {
    for drawn in every_drawn() {
        let mut cut = drawn.clone();
        cut.values.retain(|token, _| allow_reads().contains(token));
        assert!(cut.values.len() < drawn.values.len(), "{}", drawn.id);
        assert_eq!(
            allow_from_primary_measured(&cut),
            allow_from_primary_measured(&drawn),
            "{} reads a token allow_reads leaves out",
            drawn.id
        );
    }
}

#[test]
fn the_kept_answer_is_the_measure_and_a_changed_fill_is_measured_again() {
    let dark = purlis().first();
    assert_eq!(allow_from_primary(dark), allow_from_primary_measured(dark));
    assert_eq!(allow_from_primary(dark), allow_from_primary_measured(dark));

    let fill = allow_reads()
        .iter()
        .copied()
        .find(|token| dark.value(token).len() == 7)
        .expect("Allow reads an opaque colour of Purlis dark");
    let mut changed = dark.clone();
    changed.values.insert(fill, "#808080".to_owned());
    assert_ne!(
        allow_from_primary_measured(&changed),
        allow_from_primary_measured(dark),
        "{fill} moves the measure"
    );
    assert_eq!(
        allow_from_primary(&changed),
        allow_from_primary_measured(&changed)
    );
}
