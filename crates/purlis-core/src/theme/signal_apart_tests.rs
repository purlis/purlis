// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **High contrast's edges, and the accent's marks apart from the state colours** (#4921): an
//! input's edge holds 3:1 in both high-contrast appearances (WCAG 1.4.11), a link or the first
//! chart colour is never taken for a state colour under any tint, and waiting and success are
//! told apart in light high contrast.

use super::check::contrast;
use super::tint::{self, SIGNAL_MARKS, SIGNALS, oklab_of};
use super::*;

/// Every appearance purlis ships: Purlis's four and Classic's two.
fn shipped() -> Vec<&'static ResolvedTheme> {
    purlis().appearances.iter().chain(built_ins()).collect()
}

/// How far two colours are in OKLab.
fn apart(a: &str, b: &str) -> f64 {
    tint::distance(
        oklab_of(a).expect("a colour"),
        oklab_of(b).expect("a colour"),
    )
}

/// Purlis's appearance `id`.
fn appearance(id: &str) -> &'static ResolvedTheme {
    purlis()
        .appearances
        .iter()
        .find(|one| one.id == id)
        .unwrap_or_else(|| panic!("Purlis has no {id}"))
}

#[test]
fn an_input_s_edge_holds_three_to_one_in_both_high_contrast_appearances() {
    // An input is drawn on `surface.sunken` with a `border.control` edge, and stands on whatever
    // ground its view has; a button on `control.base`. The edge is what tells either from its
    // ground (WCAG 1.4.11).
    for id in ["dark-high-contrast", "light-high-contrast"] {
        let theme = appearance(id);
        let edge = theme.value("border.control");
        for ground in [
            "surface.base",
            "surface.deep",
            "surface.raised",
            "surface.overlay",
            "surface.sunken",
            "surface.hover",
            "control.base",
            "control.hover",
        ] {
            let ratio = contrast(edge, theme.value(ground));
            assert!(ratio >= 3.0, "{id}: border.control on {ground} is {ratio}");
        }
    }
}

#[test]
fn the_accent_s_marks_are_never_taken_for_a_state_colour_at_any_hue() {
    // A link and the first chart colour read as the accent (#3049): in Purlis, under no tint do
    // they come within 0.1 of the waiting, success, failed, unreadable or danger colour. In any
    // theme a tint brings them no nearer one than the theme writes them: Classic light writes its
    // link 0.08 from its success colour, and keeps it so.
    for theme in shipped() {
        let purlis = theme.name == "Purlis";
        for degree in 0..360 {
            let tinted = tint::tinted_at(theme, f64::from(degree));
            for mark in SIGNAL_MARKS {
                for state in SIGNALS.iter().chain(&["danger.base"]) {
                    let d = apart(tinted.value(mark), tinted.value(state));
                    let written = apart(theme.value(mark), theme.value(state));
                    let least = if purlis { 0.1 } else { written.min(0.1) };
                    assert!(
                        d >= least,
                        "{} {} at {degree}: {mark} {} is {d} from {state} {}",
                        theme.name,
                        theme.id,
                        tinted.value(mark),
                        tinted.value(state)
                    );
                }
            }
        }
    }
}

#[test]
fn the_marks_give_way_on_their_own_and_cost_the_accent_no_colour() {
    // Holding a link apart from success takes no colour from the accent or its kin: at every hue
    // they are drawn as they were before the marks were held (the shared answer).
    for theme in shipped() {
        for degree in 0..360 {
            let most = tint::most_at(theme, f64::from(degree));
            for mark in SIGNAL_MARKS {
                let at = tint::chroma_of(mark, most);
                assert!(
                    at.is_none() || most.shared.is_some_and(|shared| at.unwrap() <= shared),
                    "{} {} at {degree}: {mark} at {at:?}, shared {:?}",
                    theme.name,
                    theme.id,
                    most.shared
                );
            }
        }
    }
    // Purlis dark's green keeps its green accent, its link held off the success colour.
    let dark = appearance("dark");
    let green = tint::tinted(dark, "green");
    assert_ne!(green.value("accent.base"), dark.value("accent.base"));
    assert!(apart(green.value("markdown.link"), green.value("state.success")) >= 0.1);
}

#[test]
fn waiting_and_success_are_told_apart_in_every_purlis_appearance() {
    for theme in &purlis().appearances {
        let d = apart(theme.value("state.waiting"), theme.value("state.success"));
        assert!(d >= 0.1, "{}: waiting and success are {d} apart", theme.id);
    }
}
