// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A family is read only on grounds it stands on** (#4648): a see-through ground is laid over
//! what the window draws it on ([`super::WASH_LAID_ON`]), never over every opaque ground its
//! family stands on, so no part is read on a wash over a ground that wash is never drawn on.

use serde_json::{Value, json};

use super::super::check::{is_opaque, onto, solid};
use super::super::{ResolvedTheme, classic, load, purlis, tint};
use super::{MANIFEST, WASH_LAID_ON, fallen, grounds, hex};

/// A dark theme over Purlis Classic that sets `tokens` and `parts`, untinted, as read.
fn shaped(tokens: Value, parts: Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }],
        "parts": parts,
        "tint": { "on": false },
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded.theme.first().clone()
}

/// The floors a field's looks broke in `theme`.
fn field_fell(theme: &ResolvedTheme) -> Vec<String> {
    fallen(theme)
        .into_iter()
        .filter(|one| one.part.starts_with("field."))
        .map(|one| format!("{}: {}", one.part, one.floor))
        .collect()
}

/// Every appearance purlis ships, as written and in a tint at every twelfth of the hue circle.
fn shipped() -> Vec<ResolvedTheme> {
    let mut all = Vec::new();
    for theme in [purlis(), classic()] {
        for one in &theme.appearances {
            all.push(one.clone());
            all.extend((0..12).map(|step| tint::tinted_at(one, f64::from(step) * 30.0)));
        }
    }
    all
}

/// **What `family` stands on in `theme`**, as the window draws it: each opaque ground it names,
/// and each see-through one over each ground the window draws that one on.
fn stood_on(theme: &ResolvedTheme, tokens: &[&str]) -> Vec<String> {
    let mut on = Vec::new();
    for token in tokens {
        let Some(colour) = hex(theme, token) else {
            continue;
        };
        if is_opaque(colour) {
            on.push(solid(colour));
            continue;
        }
        let (_, under) = WASH_LAID_ON
            .iter()
            .find(|(wash, _)| wash == token)
            .unwrap_or_else(|| panic!("{token} is see-through, and nothing says what it is on"));
        on.extend(
            under
                .iter()
                .filter_map(|below| hex(theme, below))
                .map(|below| onto(colour, &solid(below))),
        );
    }
    on.sort();
    on.dedup();
    on
}

#[test]
fn a_theme_that_sets_only_a_field_s_error_keeps_its_colour() {
    // Danger's colour clears 4.5:1 on every ground a field that can refuse stands on; the field's
    // help, left as it is, clears it on every ground a field stands on.
    let theme = shaped(json!({}), json!({ "field.error.text": "danger.base" }));
    assert_eq!(field_fell(&theme), Vec::<String>::new());
}

#[test]
fn a_field_s_help_is_still_read_on_the_danger_wash() {
    // The control: a wash heavy enough to take the quiet words' contrast is where a field does
    // stand, in a pane's trouble Notice and a dialog, so the help on it is still read.
    let theme = shaped(
        json!({ "danger.wash": "#e8e8e8e6" }),
        json!({ "field.error.text": "danger.base" }),
    );
    let said = field_fell(&theme);
    assert!(said.iter().any(|one| one.contains("help")), "{said:?}");
}

#[test]
fn no_family_is_read_on_a_ground_it_never_stands_on() {
    for theme in shipped() {
        for family in MANIFEST {
            let mut read = grounds(&theme, family);
            read.sort();
            read.dedup();
            assert_eq!(
                read,
                stood_on(&theme, family.grounds),
                "{} in {} {}",
                family.name,
                theme.name,
                theme.id
            );
        }
    }
}

#[test]
fn no_shipped_look_falls() {
    // The check reads each family on fewer grounds than before #4648, never more, so a shipped
    // look that fell nowhere still falls nowhere: no shipped colour moves.
    for theme in shipped() {
        assert_eq!(fallen(&theme), Vec::new(), "{} {}", theme.name, theme.id);
    }
}

#[test]
fn every_see_through_ground_a_shipped_theme_draws_says_what_it_is_on() {
    // A ground a family stands on that purlis ships see-through is read over what it is drawn
    // on; one with nowhere said would be read over every ground its family names.
    for theme in shipped() {
        for family in MANIFEST {
            for token in family.grounds {
                let see_through = hex(&theme, token).is_some_and(|one| !is_opaque(one));
                assert!(
                    !see_through || WASH_LAID_ON.iter().any(|(wash, _)| wash == token),
                    "{token} ({}) in {} {}",
                    family.name,
                    theme.name,
                    theme.id
                );
            }
        }
    }
}

#[test]
fn a_wash_is_laid_only_on_opaque_grounds_a_family_stands_on() {
    // Each ground a wash is drawn on is one a control stands on, so the wash is read over a
    // ground the check already knows, and never over a wash.
    for (wash, under) in WASH_LAID_ON {
        for below in *under {
            assert!(
                !WASH_LAID_ON.iter().any(|(one, _)| one == below),
                "{wash} over {below}"
            );
            assert!(
                MANIFEST
                    .iter()
                    .any(|family| family.grounds.contains(wash) && family.grounds.contains(below)),
                "{wash} over {below}"
            );
        }
    }
}
