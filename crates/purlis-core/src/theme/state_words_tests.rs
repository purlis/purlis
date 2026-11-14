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
//!
//! **A part's words are its own, its variant's or its root's** (#6175), as the parts check draws
//! them, and a part with no fill of its own is read where its root stands, on the default fills
//! of its family's host surfaces too, unless grounds of its own say narrower ([`read_on`]).

use super::super::check::{PAIRS, aa_text};
use super::{Family, MANIFEST, Prop, State, TRANSPARENT, drawn, is_mark};

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

/// **The fills the surfaces `family` stands on draw by default** (#6163), where they are a
/// token: each look of each surface [`super::STANDS_ON`] places it on, at rest, chosen and under
/// the pointer ([`super::surface_states`]), but for a family drawn in the overlay
/// ([`super::PORTALED`]). The parts check reads what a theme's parts set on these; the token
/// floors hold what is drawn on their defaults, so they must hold it as the parts check would.
fn stood_on(family: &Family) -> Vec<&'static str> {
    let mut fills = Vec::new();
    if super::PORTALED.contains(&family.name) {
        return fills;
    }
    for (key, families) in super::STANDS_ON {
        if !families.contains(&family.name) {
            continue;
        }
        let (name, _) = super::surface_and_look(key);
        let surface = super::family(name).expect("a surface");
        for variant in surface
            .looks()
            .into_iter()
            .filter(|one| super::stands_for(key, surface.name, one))
        {
            for states in super::surface_states(surface) {
                let fill = (surface.defaults)(variant, states).fill;
                if fill != TRANSPARENT && !is_wash(fill) && !fills.contains(&fill) {
                    fills.push(fill);
                }
            }
        }
    }
    fills
}

/// **What `part`'s words are read on with no fill of their own**: its grounds
/// ([`Family::grounds_of`]), and the fills the surfaces its family stands on draw by default
/// ([`stood_on`]): a badge's words on a list's chosen row (#6163), and a part's words as its root's
/// (#6175). A part that names grounds of its own ([`Family::under`]) is read on such a fill only
/// where they name it, as the parts check reads it there: a field's error stands where a field
/// that can refuse stands, never in a list's or a table's row.
fn read_on(family: &Family, part: usize) -> Vec<&'static str> {
    let mut on = family.grounds_of(part).to_vec();
    let own = family.stands_on(part);
    for fill in stood_on(family) {
        if own.is_none_or(|grounds| grounds.contains(&fill)) && !on.contains(&fill) {
            on.push(fill);
        }
    }
    on
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
                let as_drawn = drawn(&[], family, variant, &states);
                // A root with no fill, or a wash, stands on each of the family's grounds.
                let under: Vec<&'static str> = if look.fill == TRANSPARENT || is_wash(look.fill) {
                    family.grounds.to_vec()
                } else {
                    vec![look.fill]
                };
                for (at, part) in family.parts.iter().enumerate() {
                    // A part drawn apart from the root is read on its own ground, below.
                    if super::drawn_apart(family, part.name).is_some() {
                        continue;
                    }
                    // Words as the parts check draws them (#6175): a part's own, its variant's,
                    // or the root's. A part that draws no words, or a mark, is held elsewhere.
                    if at > 0 && (!part.properties.contains(&Prop::Text) || is_mark(family, part)) {
                        continue;
                    }
                    let words = as_drawn.words[at];
                    if !is_held_as_words(words) {
                        continue;
                    }
                    // On what the parts check reads it on: its own fill, the one ground its parent paints,
                    // its variant's ground, else the root's fill.
                    let fill = as_drawn.grounds[at];
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
                            read_on(family, at)
                                .into_iter()
                                .map(|ground| (named.clone(), words, ground, None)),
                        );
                    } else if is_wash(fill) {
                        // Laid on its own ground where it has one (D-3345-3), else the root's.
                        let below: Vec<&'static str> = family
                            .laid_on(at)
                            .map_or_else(|| under.clone(), |ground| vec![ground]);
                        found.extend(
                            below
                                .iter()
                                .map(|below| (named.clone(), words, fill, Some(*below))),
                        );
                    } else {
                        found.push((named, words, fill, None));
                    }
                }
            }
        }
        // Each part drawn apart from the root, in each of its own states, on what it is drawn
        // on (#3949): a wash over each ground under it, or the ground itself.
        for part in family.parts {
            let Some(carried) = super::drawn_apart(family, part.name) else {
                continue;
            };
            for state in std::iter::once(None).chain(carried.iter().copied().map(Some)) {
                let look = super::apart_look(family.name, part.name, state);
                if !is_held_as_words(look.words) {
                    continue;
                }
                let named = format!(
                    "{}.{}{}",
                    family.name,
                    part.name,
                    state
                        .map(|one| format!(".{}", one.name()))
                        .unwrap_or_default()
                );
                if look.fill == TRANSPARENT {
                    found.extend(
                        look.under
                            .iter()
                            .map(|ground| (named.clone(), look.words, *ground, None)),
                    );
                } else if is_wash(look.fill) {
                    found.extend(
                        look.under
                            .iter()
                            .map(|below| (named.clone(), look.words, look.fill, Some(*below))),
                    );
                } else {
                    found.push((named, look.words, look.fill, None));
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
fn the_waiting_words_of_a_button_and_a_count_are_among_them() {
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
    assert!(
        has("titlebar.count", "wash.warning", Some("surface.deep")),
        "{found:?}"
    );
    // On its own ground, the bar's, whatever the chip's fill (D-3345-3): never on the card's
    // fill or the accent's wash.
    for chip in ["surface.overlay", "accent.wash", "accent.wash-hover"] {
        assert!(
            !has("titlebar.count", "wash.warning", Some(chip)),
            "{chip}: {found:?}"
        );
    }
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

#[test]
fn a_part_s_words_taken_from_its_root_are_read_too() {
    // #6175: a part with no words of its own draws the root's, as the parts check reads it
    // (`drawn`): a trouble Notice's message is in the danger colour, on the Notice's fill, and a
    // danger button's label in the danger colour on the button's.
    let found = state_words();
    let has = |part: &str, front: &str, back: &str| {
        found
            .iter()
            .any(|(named, on_front, on, _)| named == part && *on_front == front && *on == back)
    };
    assert!(
        has("notice.message.trouble", "danger.text", "danger.surface"),
        "{found:?}"
    );
    assert!(
        has("button.label.danger", "danger.base", "control.base"),
        "{found:?}"
    );
    // A mark is held at a mark's 3:1, not as words: a danger button's icon, a news Notice's mark.
    assert!(
        !found
            .iter()
            .any(|(named, ..)| named.starts_with("button.icon") || named.starts_with("notice.mark")),
        "{found:?}"
    );
}

#[test]
fn a_field_s_error_is_read_where_a_field_that_can_refuse_stands_and_never_in_a_row() {
    // #6175: a field stands in a list's and a table's rows, but a field that can refuse never
    // does (`refusingFields.guard.test.ts`), so its error is never drawn on a row's fill.
    let field = MANIFEST.iter().find(|one| one.name == "field").unwrap();
    let error = field
        .parts
        .iter()
        .position(|one| one.name == "error")
        .unwrap();
    let on = read_on(field, error);
    assert_eq!(on, super::FIELD_ERROR_GROUNDS, "{on:?}");
    // A field stands on a row's fills, chosen and hovered, and a Notice's; its error on none.
    let stood = stood_on(field);
    for row in ["list.selected", "surface.hover", "attention.surface"] {
        assert!(stood.contains(&row), "{stood:?}");
        assert!(!on.contains(&row), "{row}: {on:?}");
    }
}
