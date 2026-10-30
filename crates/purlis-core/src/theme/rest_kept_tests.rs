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

/// The colours the parts layer draws on an element marked `marks`: the last declaration of each
/// property among the rules that reach it, in the stylesheet's order.
fn resolved(sheet: &str, marks: &[String]) -> BTreeMap<String, String> {
    let mut drawn = BTreeMap::new();
    for line in sheet.lines().map(str::trim) {
        let Some(rest) = line.strip_prefix(":where(") else {
            continue;
        };
        let Some((selector, body)) = rest.split_once(") {") else {
            continue;
        };
        if selector.contains(' ') && !selector.contains(", ") {
            continue;
        }
        if !selector
            .split(", ")
            .any(|one| !one.contains(' ') && reaches(one, marks))
        {
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

#[test]
fn every_family_s_states_keep_their_own_looks_after_a_fall_at_rest() {
    let mut tested = Vec::new();
    let mut wrong = Vec::new();
    for family in MANIFEST {
        // A family drawn in marks or read on its track falls by its own rules (D-4040-4).
        if marked(family).is_some() || tracked(family).is_some() {
            continue;
        }
        if !family.parts[0].properties.contains(&Prop::Text) {
            continue;
        }
        for variant in family.looks() {
            let rest = root_colours(family, variant, &[]);
            // The states and the sets of them that draw a look of their own.
            let sets: Vec<Vec<State>> = family
                .states
                .iter()
                .map(|one| vec![*one])
                .chain(combined(family))
                .filter(|set| family.draws(variant, set))
                .filter(|set| root_colours(family, variant, set) != rest)
                .collect();
            if sets.is_empty() {
                continue;
            }
            // Words that vanish at rest: the rest's own fill, or the family's first ground.
            let fill = rest.get(&Prop::Fill).copied().unwrap_or(TRANSPARENT);
            let words = if fill == TRANSPARENT {
                family.grounds.first().copied().unwrap_or("surface.base")
            } else {
                fill
            };
            let key = if variant.is_empty() {
                format!("{}.{}.text", family.name, family.parts[0].name)
            } else {
                format!("{}.{}.{variant}.text", family.name, family.parts[0].name)
            };
            let loaded = load(&json!({
                "name": "Shaped",
                "extends": "purlis",
                "parts": { key.clone(): words },
                "tint": { "on": false },
            }));
            let theme = loaded.theme.first();
            if !fallen(theme)
                .iter()
                .any(|one| one.part == named(family, variant, &[]))
            {
                continue;
            }
            tested.push(format!("{}.{variant}", family.name));
            let sheet = stylesheet(theme);
            for set in sets {
                let drawn = resolved(&sheet, &marks_of(family, variant, &set));
                for (property, token) in root_colours(family, variant, &set) {
                    // What the theme itself sets in one of these states is the theme's.
                    let own = theme.parts.keys().any(|one| {
                        one.family == family.name
                            && one.part == family.parts[0].name
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
                    let want = written(token);
                    let got = drawn.get(css_of(property)).cloned().unwrap_or_default();
                    if got != want {
                        wrong.push(format!(
                            "{key} {set:?} {}: {got} where the family draws {want}",
                            css_of(property)
                        ));
                    }
                }
            }
        }
    }
    assert!(tested.len() >= 10, "too few families fell: {tested:?}");
    // The tab's pill under the pointer is drawn back on the pill.
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis",
        "parts": { "tabs.tab.chat.text": "surface.deep" },
        "tint": { "on": false },
    }));
    let sheet = stylesheet(loaded.theme.first());
    assert!(
        sheet
            .lines()
            .any(|line| line.contains("[data-variant=\"chat\"]")
                && line.contains("::before")
                && line.contains("background-color: var(--surface-overlay)")),
        "{sheet}"
    );
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
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
