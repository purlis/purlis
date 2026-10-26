// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A state's colour drawn as a part's words is held as words** (#3041): wherever a family of
//! the manifest draws a state's colour (waiting, running, failed, the needs-you hand) as its
//! words, at rest or under the pointer, the check holds that colour on the part's fill at
//! [`aa_text`] for the window's text size, 4.5:1, not at the 3:1 a mark takes. The pairs are
//! [`PAIRS`], so the check names them and the consent floors fall back from them.

use super::super::check::{PAIRS, aa_text};
use super::{Family, MANIFEST, State, TRANSPARENT};

/// Whether `token` is a state's colour: what the consent floors hold as a state mark.
fn is_state_colour(token: &str) -> bool {
    token.starts_with("state.") || token.starts_with("needs-you.")
}

/// Whether `token` is a wash, drawn see-through over what is under it.
fn is_wash(token: &str) -> bool {
    token.starts_with("wash.") || token.ends_with(".wash")
}

/// The looks a family's words are read in: at rest, and under the pointer where it has one.
fn read_in(family: &Family) -> Vec<Vec<State>> {
    let mut sets = vec![Vec::new()];
    if family.states.contains(&State::Hover) {
        sets.push(vec![State::Hover]);
    }
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
            for states in read_in(family) {
                let look = (family.defaults)(variant, &states);
                let under: Vec<&'static str> = if look.fill == TRANSPARENT {
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
                    if !is_state_colour(words) {
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
                                .grounds
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
        "state colours drawn as words but held below {text}:1:\n{}",
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
