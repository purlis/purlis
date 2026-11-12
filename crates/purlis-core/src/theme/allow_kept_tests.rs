// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Allow against the primary is kept by what it reads** (#3597): [`allow_from_primary`] keeps
//! its answer by the values of [`allow_reads`], so those must be every token the measure reads.
//! A theme cut down to them answers as the whole theme does, as written and in every tint of its
//! own, for each built-in appearance and for each with every colour Allow reads drawn
//! see-through (the grounds then laid on what is under them); and the kept answer is the
//! measure's.

use super::super::{ResolvedTheme, classic, purlis, tint};
use super::{allow_from_primary, allow_from_primary_measured, allow_reads};

/// Every built-in appearance, as written and in each tint of its own recipe, and each of those
/// with every opaque colour Allow reads given half alpha.
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
    let see_through: Vec<ResolvedTheme> = drawn
        .iter()
        .map(|look| {
            let mut faded = look.clone();
            for token in allow_reads() {
                if let Some(value) = faded.values.get_mut(token)
                    && value.len() == 7
                {
                    value.push_str("80");
                }
            }
            faded
        })
        .collect();
    drawn.extend(see_through);
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
    // A theme that differs only where Allow reads nothing is answered from what was kept.
    let mut elsewhere = dark.clone();
    let unread = elsewhere
        .values
        .keys()
        .copied()
        .find(|token| !allow_reads().contains(token))
        .expect("a token Allow does not read");
    elsewhere.values.insert(unread, "#123456".to_owned());
    assert_eq!(allow_from_primary(&elsewhere), allow_from_primary(dark));

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
