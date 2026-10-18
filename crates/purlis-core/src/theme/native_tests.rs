// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The native window and more contrast at the engine's interface (#2328): which appearance a
//! system asking for more contrast draws, and what the operating system draws of the window.

use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::extension::project::theme::Pick;

/// A theme with a dark, a dark tagged high contrast and a light appearance, in that order.
fn sharp() -> Value {
    json!({
        "name": "Sharp",
        "appearances": [
            { "id": "dark", "appearance": "dark", "tokens": { "surface.base": "#202020" } },
            { "id": "contrast", "appearance": "dark", "high-contrast": true,
              "tokens": { "surface.base": "#000000", "window.background": "#000000" } },
            { "id": "light", "appearance": "light", "tokens": { "surface.base": "#f4f4f4" } }
        ]
    })
}

fn mine(document: &Value) -> Theme {
    let loaded = load(document);
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded.theme
}

fn inputs<'a>(
    pick: Option<&'a Pick>,
    theirs: Option<&'a Theme>,
    system: Appearance,
    contrast: Contrast,
) -> Inputs<'a> {
    Inputs {
        pick,
        theirs,
        on: On::Every,
        system,
        contrast,
    }
}

fn drawn_id(drawn: &Drawn) -> &str {
    drawn.theme.as_ref().map_or("", |theme| theme.id.as_str())
}

fn no_survey() -> Arc<[Offered]> {
    unreachable!("no survey for the person's own theme")
}

// ---------------------------------------------------------------------------------------
// More contrast
// ---------------------------------------------------------------------------------------

#[test]
fn more_contrast_following_the_system_draws_the_high_contrast_appearance_of_the_system_s_tag() {
    let theme = mine(&sharp());
    let system = Pick::System;
    let asked = |appearance, contrast| {
        resolve(
            &inputs(Some(&system), Some(&theme), appearance, contrast),
            no_survey,
        )
    };
    let dark = asked(Appearance::Dark, Contrast::More);
    assert_eq!(drawn_id(&dark), "contrast");
    assert!(dark.follows_system);
    // No light appearance is tagged high contrast: more contrast on a light system draws the
    // light one, and never a dark one in its place.
    assert_eq!(drawn_id(&asked(Appearance::Light, Contrast::More)), "light");
    assert_eq!(
        drawn_id(&asked(Appearance::Dark, Contrast::Standard)),
        "dark"
    );
}

#[test]
fn with_no_pick_more_contrast_draws_the_person_s_high_contrast_appearance() {
    let theme = mine(&sharp());
    let asked = |contrast| {
        resolve(
            &inputs(None, Some(&theme), Appearance::Light, contrast),
            no_survey,
        )
    };
    assert_eq!(drawn_id(&asked(Contrast::More)), "contrast");
    assert_eq!(drawn_id(&asked(Contrast::Standard)), "dark");
}

#[test]
fn more_contrast_draws_an_extension_s_high_contrast_appearance() {
    let text = sharp().to_string();
    let asked = |contrast| {
        resolve(&inputs(None, None, Appearance::Dark, contrast), || {
            Arc::from(vec![Offered::of("acme".into(), "Sharp".into(), &text)])
        })
    };
    assert_eq!(drawn_id(&asked(Contrast::More)), "contrast");
    assert_eq!(drawn_id(&asked(Contrast::Standard)), "dark");
}

#[test]
fn a_theme_with_no_high_contrast_appearance_draws_the_same_whatever_contrast_is_asked() {
    let plain = mine(&json!({
        "name": "Plain",
        "appearances": [
            { "id": "night", "appearance": "dark" },
            { "id": "day", "appearance": "light" }
        ]
    }));
    for system in [Appearance::Dark, Appearance::Light] {
        assert_eq!(
            follow(&plain, system, Contrast::More),
            follow(&plain, system, Contrast::Standard)
        );
    }
    let classic_pick = Pick::BuiltIn("purlis-light");
    let more = resolve(
        &inputs(Some(&classic_pick), None, Appearance::Dark, Contrast::More),
        || unreachable!("a built-in pick surveys nothing"),
    );
    assert_eq!(more.theme.as_ref(), built_in("purlis-light"));
}

#[test]
fn a_theme_whose_only_dark_is_high_contrast_draws_it_either_way() {
    let only = mine(&json!({
        "name": "Stark",
        "appearances": [
            { "id": "stark", "appearance": "dark", "high-contrast": true },
            { "id": "day", "appearance": "light" }
        ]
    }));
    assert_eq!(
        drawn_id(&follow(&only, Appearance::Dark, Contrast::Standard)),
        "stark"
    );
    assert_eq!(
        drawn_id(&follow(&only, Appearance::Dark, Contrast::More)),
        "stark"
    );
}

// ---------------------------------------------------------------------------------------
// The native window
// ---------------------------------------------------------------------------------------

#[test]
fn the_native_window_is_drawn_in_the_appearance_s_tag_and_its_window_background() {
    let dark = built_in("purlis-dark").expect("Classic dark").native();
    assert_eq!(dark.appearance, Appearance::Dark);
    assert_eq!(dark.background, [0x19, 0x19, 0x17]);
    let light = built_in("purlis-light").expect("Classic light").native();
    assert_eq!(light.appearance, Appearance::Light);
    assert_eq!(light.background, [0xfc, 0xfb, 0xf8]);
}

#[test]
fn a_theme_s_window_background_reaches_the_native_window_opaque() {
    for (written, rgb) in [
        ("#123", [0x11, 0x22, 0x33]),
        ("#1238", [0x11, 0x22, 0x33]),
        ("#0a0b0c", [0x0a, 0x0b, 0x0c]),
        ("#0a0b0c00", [0x0a, 0x0b, 0x0c]),
    ] {
        let theme = mine(&json!({
            "name": "Mine",
            "appearances": [
                { "id": "day", "appearance": "light", "tokens": { "window.background": written } }
            ]
        }));
        let native = theme.first().native();
        assert_eq!(
            (native.appearance, native.background),
            (Appearance::Light, rgb),
            "{written}"
        );
    }
}

#[test]
fn what_the_page_hands_the_native_window_is_read_in_the_engine_s_grammar() {
    assert_eq!(
        Native::parse("light", "#fcfbf8"),
        Ok(built_in("purlis-light").expect("Classic light").native())
    );
    assert!(Native::parse("dark", "#191917").is_ok());
    for (appearance, background) in [
        ("sepia", "#191917"),
        ("Dark", "#191917"),
        ("dark", "black"),
        ("dark", "platform"),
        ("dark", "#19191"),
        ("dark", "rgb(0 0 0)"),
        ("dark", ""),
    ] {
        assert!(
            Native::parse(appearance, background).is_err(),
            "{appearance} {background}"
        );
    }
}

#[test]
fn the_first_frame_is_the_person_s_theme_for_the_contrast_asked_else_purlis() {
    let theme = mine(&sharp());
    assert_eq!(
        drawn_id(&first_frame(Some(&theme), Contrast::Standard)),
        "dark"
    );
    assert_eq!(
        drawn_id(&first_frame(Some(&theme), Contrast::More)),
        "contrast"
    );
    let none = first_frame(None, Contrast::More);
    assert_eq!(none.theme.as_ref(), Some(default_theme()));
    assert_eq!(
        none.theme.map(|theme| theme.native().background),
        Some([0x19, 0x19, 0x17])
    );
}

#[test]
fn the_first_frame_holds_the_consent_floors() {
    let lost = default_theme().value("surface.base").to_owned();
    let theme = mine(&json!({
        "name": "Mine",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": { "state.failed": lost } }]
    }));
    let drawn = first_frame(Some(&theme), Contrast::Standard);
    assert!(drawn.held.is_some());
    assert_eq!(drawn, draw(theme.first()));
}

#[test]
fn every_level_draws_more_contrast_unless_an_appearance_was_chosen_by_its_id() {
    // #2328 over the theme levels (#2323): the person's own theme at level 5, a level's choice
    // of a theme with no appearance or `system`, and a choice of an appearance by its id.
    let theme = mine(&sharp());
    let folders = Folders::default();
    let asked = |levels: &Levels, contrast| {
        resolve_levels(
            &LevelInputs {
                levels,
                folders: &folders,
                theirs: Some(&theme),
                on: On::Every,
                system: Appearance::Dark,
                contrast,
            },
            || unreachable!("no extension is named"),
        )
    };
    let nothing = Levels::default();
    assert_eq!(drawn_id(&asked(&nothing, Contrast::More)), "contrast");
    assert_eq!(drawn_id(&asked(&nothing, Contrast::Standard)), "dark");
    let following = Levels {
        project_default: Choice::new("system", None),
        ..Levels::default()
    };
    assert_eq!(drawn_id(&asked(&following, Contrast::More)), "contrast");
    let by_id = Levels {
        project_default: Choice::new("purlis-classic", Some("dark")),
        ..Levels::default()
    };
    assert_eq!(
        asked(&by_id, Contrast::More),
        asked(&by_id, Contrast::Standard)
    );
    // Whatever level draws it, the consent floors are held.
    let more = asked(&nothing, Contrast::More);
    let held = draw(
        theme
            .of_for(Appearance::Dark, Contrast::More)
            .expect("its sharp dark"),
    );
    assert_eq!((more.theme, more.held), (held.theme, held.held));
}
