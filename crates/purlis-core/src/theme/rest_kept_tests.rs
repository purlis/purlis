// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A part that falls at rest keeps its states' own looks, in every family** (#4040): the rest
//! fallback is written with no state in the parts layer, above every family's own stylesheet,
//! so each state's change from the rest, and each set of states that draws a look of its own, is
//! drawn back after it, keyed on those states. Each family is read from the stylesheet the
//! engine writes: what an element of the family draws in each state, as the parts layer
//! resolves it, is its family default.

use std::collections::BTreeMap;

use serde_json::json;

use super::super::load;
use super::*;

/// What `token` is written as.
fn written(token: &str) -> String {
    if token == TRANSPARENT {
        TRANSPARENT.to_owned()
    } else {
        format!("var(--{})", token.replace('.', "-"))
    }
}

/// The CSS property a root colour is written to.
fn css_of(property: Prop) -> &'static str {
    match property {
        Prop::Fill => "background-color",
        Prop::Text => "color",
        _ => "border-color",
    }
}

/// The attributes an element is marked with, read from a selector's compound: each
/// `[name]` or `[name="value"]` it requires, and each `:not([...])` it refuses.
fn tokens(compound: &str) -> Option<(Vec<String>, Vec<String>)> {
    if compound.contains("::") || compound.contains(":is(") || compound.contains(":has(") {
        return None;
    }
    let (mut wanted, mut refused) = (Vec::new(), Vec::new());
    let mut at = 0;
    while let Some(open) = compound[at..].find('[') {
        let start = at + open;
        let end = start + compound[start..].find(']')?;
        let token = compound[start + 1..end].replace('"', "");
        if compound[..start].ends_with(":not(") {
            refused.push(token);
        } else {
            wanted.push(token);
        }
        at = end + 1;
    }
    Some((wanted, refused))
}

/// Whether an element marked `marks` is what a compound selector reaches.
fn reaches(compound: &str, marks: &[String]) -> bool {
    let Some((wanted, refused)) = tokens(compound) else {
        return false;
    };
    let has = |token: &String| -> bool {
        if token.contains('=') {
            marks.contains(token)
        } else {
            marks
                .iter()
                .any(|one| one.split('=').next() == Some(token.as_str()))
        }
    };
    wanted.iter().all(has) && !refused.iter().any(has)
}

/// The colours the parts layer draws on an element marked `marks`, or on a part marked `inner`
/// inside it: the last declaration of each property among the rules that reach it, in the
/// stylesheet's order.
fn resolved(sheet: &str, marks: &[String], inner: Option<&[String]>) -> BTreeMap<String, String> {
    let mut drawn = BTreeMap::new();
    for line in sheet.lines().map(str::trim) {
        let Some(rest) = line.strip_prefix(":where(") else {
            continue;
        };
        let Some((selector, body)) = rest.split_once(") {") else {
            continue;
        };
        let hit = selector
            .split(", ")
            .any(|one| match (one.split_once(' '), inner) {
                (None, None) => reaches(one, marks),
                (Some((outer, part)), Some(inner)) => {
                    !part.contains(' ') && reaches(outer, marks) && reaches(part, inner)
                }
                _ => false,
            });
        if !hit {
            continue;
        }
        for declaration in body.trim_end_matches('}').split(';') {
            if let Some((property, value)) = declaration.trim().split_once(": ") {
                drawn.insert(property.to_owned(), value.to_owned());
            }
        }
    }
    drawn
}

/// The marks an element of `family`'s root carries in `variant` while `states` hold.
fn marks_of(family: &Family, variant: &str, states: &[State]) -> Vec<String> {
    let mut marks = vec![
        format!("data-scope={}", family.name),
        format!("data-part={}", family.parts[0].name),
    ];
    if !variant.is_empty() {
        marks.push(format!("data-variant={variant}"));
    }
    for state in states {
        if let Some((wanted, _)) = tokens(family.attribute(*state)) {
            marks.extend(wanted);
        }
    }
    marks
}

/// The sets of states a family's look is read in after a fall at rest: each state, each set the
/// hold measures, and each of them while unavailable.
fn sets_of(family: &Family) -> Vec<Vec<State>> {
    let enabled: Vec<Vec<State>> = family
        .states
        .iter()
        .filter(|one| **one != State::Disabled)
        .map(|one| vec![*one])
        .chain(combined(family))
        .collect();
    let mut sets = enabled.clone();
    if family.states.contains(&State::Disabled) {
        sets.push(vec![State::Disabled]);
        sets.extend(enabled.iter().map(|set| {
            let mut with = set.clone();
            with.push(State::Disabled);
            with
        }));
    }
    sets
}

/// The families no fall at rest is read in, each with why.
const NOT_READ: &[(&str, &str)] = &[
    // Drawn in marks and read on its track: they fall by their own rules (D-4040-4).
    ("panel-separator", "drawn in marks: its rest never falls"),
    ("meter", "read on its track, with no state"),
];

#[test]
fn every_family_s_states_keep_their_own_looks_after_a_fall_at_rest() {
    let mut tested = Vec::new();
    let mut wrong = Vec::new();
    for family in MANIFEST {
        if NOT_READ.iter().any(|(name, _)| *name == family.name) {
            assert!(
                marked(family).is_some() || tracked(family).is_some(),
                "{} is read by its own rules",
                family.name
            );
            continue;
        }
        for variant in family.looks() {
            let rest = root_colours(family, variant, &[]);
            let sets: Vec<Vec<State>> = sets_of(family)
                .into_iter()
                .filter(|set| family.draws(variant, set))
                .filter(|set| {
                    root_colours(family, variant, set) != rest
                        || (family.defaults)(variant, set).text
                            != (family.defaults)(variant, &[]).text
                })
                .collect();
            if sets.is_empty() {
                continue;
            }
            // A rest that falls: words in the rest's own fill or the family's first ground, or,
            // for a root that takes no words, a fill in its own words' colour.
            let fill = rest.get(&Prop::Fill).copied().unwrap_or(TRANSPARENT);
            let root = family.parts[0].name;
            let at = |property: &str| {
                if variant.is_empty() {
                    format!("{}.{root}.{property}", family.name)
                } else {
                    format!("{}.{root}.{variant}.{property}", family.name)
                }
            };
            let (key, value) = if family.parts[0].properties.contains(&Prop::Text) {
                let words = if fill == TRANSPARENT {
                    family.grounds.first().copied().unwrap_or("surface.base")
                } else {
                    fill
                };
                (at("text"), words)
            } else {
                (at("fill"), (family.defaults)(variant, &[]).text)
            };
            let loaded = load(&json!({
                "name": "Shaped",
                "extends": "purlis",
                "parts": { key.clone(): value },
                "tint": { "on": false },
            }));
            let theme = loaded.theme.first();
            if !fallen(theme)
                .iter()
                .any(|one| one.part == named(family, variant, &[]))
            {
                wrong.push(format!("{key}: {value} did not make the rest fall"));
                continue;
            }
            tested.push(family.name);
            let sheet = stylesheet(theme);
            for set in sets {
                let marks = marks_of(family, variant, &set);
                // An unavailable control is never under the pointer or pressed: those keys are
                // `:not([data-disabled])`.
                let drawn_in: Vec<State> = if set.contains(&State::Disabled) {
                    set.iter()
                        .copied()
                        .filter(|one| !matches!(one, State::Hover | State::Pressed))
                        .collect()
                } else {
                    set.clone()
                };
                let look = (family.defaults)(variant, &drawn_in);
                let drawn = resolved(&sheet, &marks, None);
                for (property, token) in root_colours(family, variant, &drawn_in) {
                    // What the theme itself sets in one of these states is the theme's.
                    let own = theme.parts.keys().any(|one| {
                        one.family == family.name
                            && one.part == root
                            && one.property == property
                            && one.variant.is_none_or(|v| v == variant)
                            && one.state.is_some_and(|state| set.contains(&state))
                    });
                    // A tab's fill under the pointer is its pill (`::before`), which the engine
                    // writes there (`targets`), not on the tab itself.
                    let pill = family.name == "tabs"
                        && property == Prop::Fill
                        && set.contains(&State::Hover)
                        && !set.contains(&State::Selected);
                    if own || pill {
                        continue;
                    }
                    let got = drawn.get(css_of(property)).cloned().unwrap_or_default();
                    if got != written(token) {
                        wrong.push(format!(
                            "{key} {set:?} {}: {got} where the family draws {}",
                            css_of(property),
                            written(token)
                        ));
                    }
                }
                // The words of each part that follows the root's.
                for (at, part) in family.parts.iter().enumerate().skip(1) {
                    if !part.properties.contains(&Prop::Text)
                        || family.own_words(at).is_some()
                        || drawn_apart(family, part.name).is_some()
                    {
                        continue;
                    }
                    let inner = vec![
                        format!("data-scope={}", family.name),
                        format!("data-part={}", part.name),
                    ];
                    let drawn = resolved(&sheet, &marks, Some(&inner));
                    let got = drawn.get("color").cloned().unwrap_or_default();
                    if got != written(look.text) {
                        wrong.push(format!(
                            "{key} {set:?} {}'s words: {got} where the family draws {}",
                            part.name,
                            written(look.text)
                        ));
                    }
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // Every family with a look a state changes was read.
    let mut read: Vec<&str> = tested.clone();
    read.dedup();
    assert!(read.len() >= 20, "{read:?}");
    // The tab's pill under the pointer is drawn back on the pill, in every kind of tab.
    let tabs = family("tabs").expect("the tabs");
    for variant in tabs.looks() {
        let loaded = load(&json!({
            "name": "Shaped",
            "extends": "purlis",
            "parts": { format!("tabs.tab.{variant}.text"): "surface.deep" },
            "tint": { "on": false },
        }));
        let sheet = stylesheet(loaded.theme.first());
        let hover = (tabs.defaults)(variant, &[State::Hover]).fill;
        assert!(
            sheet.lines().any(
                |line| line.contains(&format!("[data-variant=\"{variant}\"]"))
                    && line.contains("::before")
                    && line.contains(&format!("background-color: {}", written(hover)))
            ),
            "{variant}: {sheet}"
        );
    }
}

#[test]
fn a_theme_s_own_state_colour_wins_in_every_set_that_holds_it() {
    // The dispatcher's ruling on #4180's review: a theme that fills a quiet button under the
    // pointer keeps that fill on one that is on and under the pointer, after its rest falls; the
    // family's own colour fills only what the theme leaves unset.
    let button = family("button").expect("the button");
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis",
        "parts": {
            "button.root.ghost.text": "surface.base",
            "button.root.ghost.hover.fill": "control.base",
        },
        "tint": { "on": false },
    }));
    let theme = loaded.theme.first();
    assert_eq!(
        fallen(theme)
            .iter()
            .map(|one| one.part.as_str())
            .collect::<Vec<_>>(),
        ["button.root.ghost"]
    );
    let sheet = stylesheet(theme);
    let fill = |states: &[State]| {
        resolved(&sheet, &marks_of(button, "ghost", states), None)
            .get("background-color")
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(fill(&[State::Hover]), "var(--control-base)", "{sheet}");
    assert_eq!(
        fill(&[State::Hover, State::Selected]),
        "var(--control-base)",
        "{sheet}"
    );
    assert_eq!(
        fill(&[State::Selected]),
        written((button.defaults)("ghost", &[State::Selected]).fill),
        "{sheet}"
    );
}

#[test]
fn a_refused_change_comes_after_every_set_of_states_without_it() {
    // D-4040-2: a box under the pointer, focused and refused draws its danger edge, whatever a
    // set of states written back without the refusal draws.
    let family = family("text-input").expect("a box");
    let set = Rule {
        family,
        part: 0,
        variant: None,
        state: Some(State::Hover),
        also: vec![State::Focused],
        values: BTreeMap::new(),
        fallback: true,
    };
    let refused = Rule {
        state: Some(State::Invalid),
        also: Vec::new(),
        ..set.clone()
    };
    assert!(refused.order() > set.order());
}

#[test]
fn a_set_of_three_states_a_part_drawn_apart_falls_in_is_keyed_on_all_three() {
    // #4117's review: a part drawn apart that falls while three states hold draws its look
    // back keyed on every one of them, never on the first two alone.
    let family = family("titlebar").expect("the title bar");
    let pin = family
        .parts
        .iter()
        .position(|one| one.name == "pin")
        .expect("the pin");
    let mut rules = Vec::new();
    let mut fell = Vec::new();
    Held {
        family,
        rules: &mut rules,
        fell: &mut fell,
        margin: 0.0,
    }
    .fall_apart(
        pin,
        &[State::Hover, State::Selected, State::Focused],
        3.0,
        1.0,
    );
    assert_eq!(fell.len(), 1, "{fell:?}");
    assert_eq!(fell[0].part, "titlebar.pin.hover+selected+focused");
    assert_eq!(rules.len(), 1, "{rules:?}");
    assert_eq!(rules[0].state, Some(State::Hover));
    assert_eq!(rules[0].also, [State::Selected, State::Focused]);
}

// ---------------------------------------------------------------------------------------
// A single state that falls (#4174)
// ---------------------------------------------------------------------------------------

#[test]
fn every_family_s_failing_state_falls_only_where_it_holds() {
    // In every family and look, a theme's words in one state vanish on that state's own fill:
    // the state falls. Where it holds, alone or with others, the family's default is drawn
    // (by the parts layer, or by the family's own stylesheet where the layer writes nothing);
    // where it does not hold, nothing of the fall is drawn.
    let mut wrong = Vec::new();
    let mut read = Vec::new();
    for family in MANIFEST {
        if NOT_READ.iter().any(|(name, _)| *name == family.name) {
            continue;
        }
        if !family.parts[0].properties.contains(&Prop::Text) {
            continue;
        }
        let root = family.parts[0].name;
        for variant in family.looks() {
            for state in family.states.iter().copied().filter(|one| {
                !matches!(one, State::Disabled | State::Invalid) && family.draws(variant, &[*one])
            }) {
                let look = (family.defaults)(variant, &[state]);
                let words = if look.fill == TRANSPARENT {
                    family.grounds.first().copied().unwrap_or("surface.base")
                } else {
                    look.fill
                };
                let key = if variant.is_empty() {
                    format!("{}.{root}.{}.text", family.name, state.name())
                } else {
                    format!("{}.{root}.{variant}.{}.text", family.name, state.name())
                };
                let loaded = load(&json!({
                    "name": "Shaped",
                    "extends": "purlis",
                    "parts": { key.clone(): words },
                    "tint": { "on": false },
                }));
                if !loaded.complaints.is_empty() {
                    continue;
                }
                let theme = loaded.theme.first();
                if !fallen(theme)
                    .iter()
                    .any(|one| one.part == named(family, variant, &[state]))
                {
                    continue;
                }
                read.push(format!("{}.{variant}.{}", family.name, state.name()));
                let sheet = stylesheet(theme);
                for set in sets_of(family)
                    .into_iter()
                    .filter(|set| family.draws(variant, set))
                {
                    let drawn_in: Vec<State> = if set.contains(&State::Disabled) {
                        set.iter()
                            .copied()
                            .filter(|one| !matches!(one, State::Hover | State::Pressed))
                            .collect()
                    } else {
                        set.clone()
                    };
                    let holds = drawn_in.contains(&state);
                    let marks = marks_of(family, variant, &set);
                    let drawn = resolved(&sheet, &marks, None);
                    let colours = root_colours(family, variant, &drawn_in);
                    for (property, token) in &colours {
                        // A colour the theme draws itself, built in (Purlis's card rows), is
                        // the theme's, wherever it is drawn.
                        let theirs = theme.parts.keys().any(|one| {
                            one.family == family.name
                                && one.part == root
                                && one.property == *property
                                && one.variant.is_none_or(|v| v == variant)
                                && one.state != Some(state)
                        });
                        if theirs {
                            continue;
                        }
                        let got = drawn.get(css_of(*property));
                        let ok = if holds {
                            got.is_none_or(|one| *one == written(token))
                        } else {
                            got.is_none()
                        };
                        if !ok {
                            wrong.push(format!(
                                "{key} {set:?} {}: {got:?} where the family draws {}",
                                css_of(*property),
                                written(token)
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert!(read.len() >= 20, "{read:?}");
}

#[test]
fn a_theme_s_colour_in_another_state_survives_a_state_s_fall() {
    // A quiet ghost button the theme fills under the pointer, whose words while on vanish: the
    // toggle that is on falls, and a toggle that is on and under the pointer keeps the theme's
    // fill, where the family's own colour fills only the words.
    let button = family("button").expect("the button");
    let selected = (button.defaults)("ghost", &[State::Selected]);
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis",
        "parts": {
            "button.root.ghost.hover.fill": "control.base",
            "button.root.ghost.selected.text": selected.fill,
        },
        "tint": { "on": false },
    }));
    let theme = loaded.theme.first();
    let fell: Vec<String> = fallen(theme).into_iter().map(|one| one.part).collect();
    assert!(
        fell.contains(&"button.root.ghost.selected".to_owned()),
        "{fell:?}"
    );
    let sheet = stylesheet(theme);
    let drawn = |states: &[State]| resolved(&sheet, &marks_of(button, "ghost", states), None);
    assert_eq!(
        drawn(&[State::Hover])
            .get("background-color")
            .map(String::as_str),
        Some("var(--control-base)"),
        "{sheet}"
    );
    assert_eq!(
        drawn(&[State::Hover, State::Selected])
            .get("background-color")
            .map(String::as_str),
        Some("var(--control-base)"),
        "{sheet}"
    );
    assert_eq!(
        drawn(&[State::Selected]).get("color").map(String::as_str),
        Some(written(selected.text).as_str()),
        "{sheet}"
    );
}

#[test]
fn a_state_that_falls_after_the_rest_never_covers_another_state_s_look() {
    // #4180's review, finding 5: a row whose rest falls keeps its pill under the pointer, and
    // when its focus falls after it, a row under the pointer and focused still draws that pill.
    let list = family("list").expect("the list");
    let rest = (list.defaults)("row", &[]);
    let focused = (list.defaults)("row", &[State::Focused]);
    let ground = list.grounds[0];
    let words = |look_fill: &'static str| {
        if look_fill == TRANSPARENT {
            ground
        } else {
            look_fill
        }
    };
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis",
        "parts": {
            "list.root.row.text": words(rest.fill),
            "list.root.row.focused.text": words(focused.fill),
        },
        "tint": { "on": false },
    }));
    let theme = loaded.theme.first();
    let fell: Vec<String> = fallen(theme).into_iter().map(|one| one.part).collect();
    assert!(fell.contains(&"list.root.row".to_owned()), "{fell:?}");
    assert!(
        fell.contains(&"list.root.row.focused".to_owned()),
        "{fell:?}"
    );
    let sheet = stylesheet(theme);
    let both = resolved(
        &sheet,
        &marks_of(list, "row", &[State::Hover, State::Focused]),
        None,
    );
    assert_eq!(
        both.get("background-color").cloned(),
        Some(written(
            (list.defaults)("row", &[State::Hover, State::Focused]).fill
        )),
        "{sheet}"
    );
}

// ---------------------------------------------------------------------------------------
// What falls is readable as it is drawn (#4196's review)
// ---------------------------------------------------------------------------------------

/// Every look of `theme` that fell, measured again on the rules the hold ended with: a look
/// said to be drawn back whose words still break their floor.
fn fell_unreadable(theme: &ResolvedTheme) -> Vec<String> {
    let (rules, fell) = (*held_once(theme)).clone();
    let mut wrong = Vec::new();
    for family in MANIFEST {
        let sets: Vec<Vec<State>> = std::iter::once(Vec::new())
            .chain(family.states.iter().map(|one| vec![*one]))
            .chain(combined(family))
            .collect();
        for variant in family.looks() {
            for states in &sets {
                if states.contains(&State::Disabled) || !family.draws(variant, states) {
                    continue;
                }
                let name = named(family, variant, states);
                if !fell.iter().any(|one| one.part == name) {
                    continue;
                }
                let look = drawn(&rules, family, variant, states);
                // A look drawn as the family's own default is held by the token floors, as the
                // hold itself takes it.
                if look == drawn(&[], family, variant, states) {
                    continue;
                }
                let on = grounds(theme, family);
                if let Some((floor, ratio)) =
                    unreadable(theme, &on, &rules, family, variant, states, &look)
                {
                    wrong.push(format!("{name}: {floor} ({ratio:?})"));
                }
            }
        }
    }
    wrong
}

#[test]
fn every_look_that_falls_is_readable_as_it_is_drawn() {
    // A theme's rest colour that reaches a state it does not read in, a part's own words that
    // vanish in a state, a part's own fill, and a set of states: each look that falls is
    // readable on the rules the hold ends with.
    for parts in [
        json!({ "button.root.text": "accent.base" }),
        json!({ "list.root.text": "text.quiet" }),
        json!({ "list.label.text": "surface.hover" }),
        json!({ "list.description.hover.text": "surface.hover" }),
        json!({ "checkbox.label.text": "accent.surface" }),
        json!({ "tabs.count.hover.text": "surface.overlay" }),
        json!({ "button.root.secondary.text": "tab.active" }),
        json!({ "menu.description.hover.text": "list.selected" }),
        // A part's rest words that reach a state they do not read in (#4196's review, 1 and 2).
        json!({ "button.label.text": "surface.raised" }),
        json!({ "button.icon.text": "accent.text" }),
        json!({ "button.icon.text": "accent.fill-hover" }),
        // A part's own words in a state (finding 2).
        json!({ "button.label.hover.text": "surface.base" }),
        json!({ "button.icon.pressed.text": "surface.base" }),
    ] {
        for base in ["purlis", "purlis-classic"] {
            let loaded = load(&json!({
                "name": "Shaped",
                "extends": base,
                "parts": parts.clone(),
                "tint": { "on": false },
            }));
            assert!(
                loaded.complaints.is_empty(),
                "{parts}: {:?}",
                loaded.complaints
            );
            for appearance in &loaded.theme.appearances {
                let wrong = fell_unreadable(appearance);
                assert!(
                    wrong.is_empty(),
                    "{base} {} {parts}: {wrong:?}",
                    appearance.id
                );
            }
        }
    }
}

/// The rules a theme's `parts` give over Purlis, as read, and the family named.
fn theme_rules(parts: serde_json::Value) -> Vec<Rule> {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis",
        "parts": parts,
        "tint": { "on": false },
    }));
    assert!(loaded.complaints.is_empty(), "{:?}", loaded.complaints);
    super::rules(&loaded.theme.first().parts)
}

/// `family`'s `variant` falling while `states` hold, on `rules`: what the rules then draw
/// `property` of part `at` in, while `seen` hold.
fn after_fall(
    mut rules: Vec<Rule>,
    family: &'static Family,
    variant: &'static str,
    states: &[State],
    at: usize,
    seen: &[State],
    property: Prop,
) -> Option<&'static str> {
    let mut fell = Vec::new();
    Held {
        family,
        rules: &mut rules,
        fell: &mut fell,
        margin: 0.0,
    }
    .fall(variant, states, "a floor".to_owned(), None);
    drawn_now(&rules, family, at, variant, seen, property)
}

#[test]
fn a_state_that_falls_draws_back_what_the_theme_s_rest_colour_drew_there() {
    // #4196's review, 1: the theme's rest words reach a toggle that is on; when it falls, its
    // default words are drawn back there, though they are the rest's default too.
    let button = family("button").expect("the button");
    let rules = theme_rules(json!({ "button.root.text": "accent.base" }));
    let selected = (button.defaults)("secondary", &[State::Selected]).text;
    assert_eq!(
        after_fall(
            rules,
            button,
            "secondary",
            &[State::Selected],
            0,
            &[State::Selected],
            Prop::Text
        ),
        Some(selected)
    );
}

#[test]
fn a_state_that_falls_draws_back_a_part_s_own_words() {
    // #4196's review, 2: a list row's description has words of its own; its theme words in the
    // row under the pointer are drawn back when that state falls.
    let list = family("list").expect("the list");
    let at = list
        .parts
        .iter()
        .position(|one| one.name == "description")
        .expect("a description");
    let rules = theme_rules(json!({ "list.description.hover.text": "surface.hover" }));
    assert_eq!(
        after_fall(
            rules,
            list,
            "row",
            &[State::Hover],
            at,
            &[State::Hover],
            Prop::Text
        ),
        list.own_words_of(at)
    );
}

#[test]
fn a_set_that_falls_draws_its_parts_words_as_the_family_draws_the_set() {
    // #4196's review, 3: a chip checkbox under the pointer and on draws its label in the set's
    // words, not the first state's.
    let checkbox = family("checkbox").expect("the checkbox");
    let at = checkbox
        .parts
        .iter()
        .position(|one| one.name == "label")
        .expect("a label");
    let set = [State::Hover, State::Selected];
    let rules = theme_rules(json!({ "checkbox.root.chip.hover.fill": "accent.surface" }));
    assert_eq!(
        after_fall(rules, checkbox, "chip", &set, at, &set, Prop::Text),
        Some((checkbox.defaults)("chip", &set).text)
    );
}
