// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A state's colour drawn as a part's words is held as words** (#3041): wherever a family of
//! the manifest draws a state's colour (waiting, running, failed, the needs-you hand) as its
//! words, in every look the window draws (at rest, under the pointer, pressed as a toggle and
//! with what it opens open, #3255), the check holds that colour on the part's fill at
//! [`aa_text`] for the window's text size, 4.5:1, not at the 3:1 a mark takes. The pairs are
//! [`PAIRS`], so the check names them and the consent floors fall back from them.
//!
//! **So is the danger colour drawn as words** (#3254): a danger answer's words, a danger row's
//! and a field's error, on every ground the window draws them on. A part's words with no fill of
//! their own are read on the grounds the part is drawn on ([`Family::grounds_of`]): a field's
//! error only under a field that can refuse, not on every ground a field stands on.

use super::super::check::{PAIRS, aa_text};
use super::{Family, MANIFEST, State, TRANSPARENT};

/// Whether `token` is a state's colour, which the consent floors hold as a state mark, or the
/// danger colour, which they hold as danger.
fn is_held_as_words(token: &str) -> bool {
    token.starts_with("state.") || token.starts_with("needs-you.") || token.starts_with("danger.")
}

/// Whether `token` is a wash, drawn see-through over what is under it.
fn is_wash(token: &str) -> bool {
    token.starts_with("wash.") || token.ends_with(".wash")
}

/// **The looks `variant`'s words are read in**: at rest, and under the pointer where the family
/// has one; and pressed as a toggle (selected) or with what it opens open, each at rest and under
/// the pointer, where the family marks the state. A look the window never draws in `variant`
/// ([`Family::draws`], #3255) is left out: a danger button is never a toggle.
fn read_in(family: &Family, variant: &str) -> Vec<Vec<State>> {
    let hover = family.states.contains(&State::Hover);
    let mut sets = vec![Vec::new()];
    if hover {
        sets.push(vec![State::Hover]);
    }
    for held in [State::Selected, State::Open] {
        if !family.states.contains(&held) {
            continue;
        }
        sets.push(vec![held]);
        if hover {
            sets.push(vec![State::Hover, held]);
        }
    }
    sets.retain(|states| family.draws(variant, states));
    sets
}

/// One place a state colour is drawn as words: the part, the colour, the fill it is read on,
/// and what that fill is laid over, for a wash.
type Words = (String, &'static str, &'static str, Option<&'static str>);

/// Every place the manifest draws a state colour as words: a look's words on its fill, and a
/// part's own words on its own fill or the root's; on each of the family's grounds where that is
/// none, and a wash over the root's fill, or each ground under none.
fn state_words() -> Vec<Words> {
    let mut found = Vec::new();
    for family in MANIFEST {
        for variant in family.looks() {
            for states in read_in(family, variant) {
                let look = (family.defaults)(variant, &states);
                // A root with no fill, or a wash, stands on each of the family's grounds.
                let under: Vec<&'static str> = if look.fill == TRANSPARENT || is_wash(look.fill) {
                    family.grounds.to_vec()
                } else {
                    vec![look.fill]
                };
                for (at, part) in family.parts.iter().enumerate() {
                    let words = if at == 0 {
                        look.text
                    } else {
                        match family.own_words(at) {
                            Some(token) => token,
                            None => continue,
                        }
                    };
                    if !is_held_as_words(words) {
                        continue;
                    }
                    let fill = family.own_fill(at).unwrap_or(look.fill);
                    let named = format!(
                        "{}.{}{}{}",
                        family.name,
                        part.name,
                        if variant.is_empty() {
                            String::new()
                        } else {
                            format!(".{variant}")
                        },
                        states
                            .iter()
                            .map(|one| format!(".{}", one.name()))
                            .collect::<String>()
                    );
                    if fill == TRANSPARENT {
                        found.extend(
                            family
                                .grounds_of(at)
                                .iter()
                                .map(|ground| (named.clone(), words, *ground, None)),
                        );
                    } else if is_wash(fill) {
                        found.extend(
                            under
                                .iter()
                                .map(|below| (named.clone(), words, fill, Some(*below))),
                        );
                    } else {
                        found.push((named, words, fill, None));
                    }
                }
            }
        }
    }
    found
}

#[test]
fn every_state_colour_a_part_draws_as_words_is_held_at_the_text_floor() {
    let text = aa_text(14.0, false);
    let missing: Vec<String> = state_words()
        .into_iter()
        .filter(|(_, front, back, under)| {
            !PAIRS.iter().any(|pair| {
                pair.front == *front
                    && pair.back == *back
                    && pair.under == *under
                    && pair.fade.is_none()
                    && pair.floor >= text
            })
        })
        .map(|(part, front, back, under)| {
            let over = under.map(|one| format!(" over {one}")).unwrap_or_default();
            format!("{part}: {front} on {back}{over}")
        })
        .collect();
    assert!(
        missing.is_empty(),
        "state or danger colours drawn as words but held below {text}:1:\n{}",
        missing.join("\n")
    );
}

#[test]
fn the_waiting_words_of_a_button_a_badge_and_a_count_are_among_them() {
    let found = state_words();
    let has = |part: &str, back: &str, under: Option<&str>| {
        found.iter().any(|(named, front, on, below)| {
            named.starts_with(part) && *front == "state.waiting" && *on == back && *below == under
        })
    };
    assert!(
        has("button.root.warning-soft", "control.base", None),
        "{found:?}"
    );
    assert!(
        has("button.root.warning-soft.hover", "control.hover", None),
        "{found:?}"
    );
    assert!(has("rail.badge", "surface.overlay", None), "{found:?}");
    assert!(
        has("titlebar.count", "wash.warning", Some("surface.deep")),
        "{found:?}"
    );
    assert!(
        has("titlebar.count", "wash.warning", Some("surface.overlay")),
        "{found:?}"
    );
}

#[test]
fn the_looks_read_are_the_ones_the_window_draws() {
    let family = |name: &str| MANIFEST.iter().find(|one| one.name == name).unwrap();
    let button = family("button");
    // A plain button is pressed as a toggle, and under the pointer while pressed.
    let plain = read_in(button, "secondary");
    assert!(plain.contains(&vec![State::Selected]), "{plain:?}");
    assert!(
        plain.contains(&vec![State::Hover, State::Selected]),
        "{plain:?}"
    );
    // A danger or a warning button never is: its pressed look is no look the window draws.
    for variant in ["danger", "warning-soft", "primary", "grants"] {
        let drawn = read_in(button, variant);
        assert!(drawn.contains(&vec![State::Hover]), "{variant}: {drawn:?}");
        assert!(
            !drawn.iter().any(|one| one.contains(&State::Selected)),
            "{variant}: {drawn:?}"
        );
    }
    // A menu's plain row opens a submenu; a danger row never does.
    let menu = family("menu");
    assert!(read_in(menu, "default").contains(&vec![State::Open]));
    assert!(
        !read_in(menu, "danger")
            .iter()
            .any(|one| one.contains(&State::Open))
    );
    assert!(read_in(menu, "danger").contains(&vec![State::Selected]));
}

#[test]
fn the_danger_words_of_an_answer_a_row_and_a_field_s_error_are_among_them() {
    let found = state_words();
    let has = |part: &str, front: &str, back: &str| {
        found.iter().any(|(named, on_front, on, _)| {
            named.starts_with(part) && *on_front == front && *on == back
        })
    };
    assert!(
        has("button.root.danger", "danger.base", "control.base"),
        "{found:?}"
    );
    assert!(
        has("button.root.danger.hover", "danger.base", "danger.surface"),
        "{found:?}"
    );
    assert!(
        has("menu.root.danger", "danger.base", "surface.overlay"),
        "{found:?}"
    );
    assert!(
        has("field.error", "danger.text", "surface.raised"),
        "{found:?}"
    );
    // A field's error is read only where a field that can refuse stands, never in its box.
    assert!(
        !has("field.error", "danger.text", "surface.sunken"),
        "{found:?}"
    );
    // A danger button is never pressed, and a danger row never opens a submenu (#3255).
    assert!(
        !has("button.root.danger", "danger.base", "accent.surface"),
        "{found:?}"
    );
    assert!(
        !has("menu.root.danger", "danger.base", "tab.active"),
        "{found:?}"
    );
}
