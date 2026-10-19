// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Purlis, the theme built from the foundation** (#2336), at the engine's interface: what it
//! draws, that every combination of it clears the check and every consent floor, that a pick
//! names it while Purlis Classic stays the default, and that its faces are served.

use std::sync::Arc;

use serde_json::Value;

use super::gallery::{self, Shelf};
use super::levels::Reach;
use super::*;
use crate::extension::project::theme::Pick;

fn no_survey() -> Arc<[Offered]> {
    Arc::from(Vec::new())
}

/// Purlis's `theme.json` as its folder holds it.
fn document() -> Value {
    let files = purlis_files();
    serde_json::from_slice(&files[THEME_JSON]).unwrap()
}

/// What the levels draw for `levels`, with no theme folder anywhere.
fn drawn_for(levels: &Levels, contrast: Contrast) -> Drawn {
    resolve_levels(
        &LevelInputs {
            levels,
            folders: &Folders::of(None, None, None),
            theirs: None,
            on: On::Every,
            system: Appearance::Dark,
            contrast,
        },
        no_survey,
    )
}

#[test]
fn purlis_is_a_dark_a_light_and_a_high_contrast_dark_appearance() {
    let theme = purlis();
    assert_eq!(theme.name, "Purlis");
    let shape: Vec<(&str, Appearance, bool)> = theme
        .appearances
        .iter()
        .map(|one| (one.id.as_str(), one.appearance, one.high_contrast))
        .collect();
    assert_eq!(
        shape,
        [
            ("dark", Appearance::Dark, false),
            ("light", Appearance::Light, false),
            ("dark-high-contrast", Appearance::Dark, true),
        ]
    );
}

#[test]
fn every_appearance_names_every_token_itself_so_nothing_is_purlis_classic_s() {
    // A token an appearance does not name would be Purlis Classic's, silently: Purlis names each.
    let classic: Value = serde_json::from_str(CLASSIC_THEME_JSON).unwrap();
    let every: Vec<&String> = classic["appearances"][0]["tokens"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    let document = document();
    for one in document["appearances"].as_array().unwrap() {
        let tokens = one["tokens"].as_object().unwrap();
        for token in &every {
            assert!(tokens.contains_key(*token), "{} has no {token}", one["id"]);
        }
        assert!(one["motion"].is_object(), "{} has no motion", one["id"]);
    }
}

#[test]
fn dark_draws_the_foundation_s_colours_fonts_and_icons() {
    let dark = purlis().of(Appearance::Dark).unwrap();
    for (token, value) in [
        ("window.background", "#121215"),
        ("surface.deep", "#141417"),
        ("surface.base", "#17171b"),
        ("surface.raised", "#1d1d22"),
        ("surface.overlay", "#24242a"),
        ("terminal.background", "#202026"),
        ("text.primary", "#e6e4ec"),
        ("text.secondary", "#c9c7d2"),
        ("accent.base", "#a99cf0"),
        ("focus.ring", "#a99cf0"),
        ("list.selected", "#2b2a37"),
        ("state.waiting", "#e5c08a"),
        ("state.success", "#8fc79a"),
        ("syntax.string", "#9fcfa8"),
        ("syntax.property", "#c3baf5"),
        ("markdown.link", "#b3a8f2"),
    ] {
        assert_eq!(dark.value(token), value, "{token}");
    }
    assert_eq!(dark.fonts.ui.family.as_deref(), Some("IBM Plex Sans"));
    let weights: Vec<u16> = dark.fonts.ui.faces.iter().map(|one| one.weight).collect();
    assert_eq!(weights, [400, 500, 600]);
    assert_eq!(dark.fonts.mono.family.as_deref(), Some("JetBrains Mono"));
    assert_eq!(
        dark.fonts.terminal.family.as_deref(),
        Some("JetBrains Mono")
    );
    assert!(!dark.icons.is_empty(), "Purlis draws Lucide's icons");
}

#[test]
fn every_combination_clears_the_check_and_holds_no_consent_floor() {
    let theme = purlis();
    let checked = check_every(theme);
    assert_eq!(checked.len(), theme.combinations());
    for one in &checked {
        assert!(
            one.report.passes(),
            "{}: {:?}",
            one.appearance,
            one.report.findings
        );
        assert_eq!(one.held, None, "{} holds a consent floor", one.appearance);
    }
    assert!(warnings_said(theme, &checked).is_empty());
    for one in &theme.appearances {
        assert_eq!(draw(one).held, None, "{}", one.id);
    }
}

#[test]
fn its_faces_are_served_by_their_digest_from_the_files_compiled_in() {
    let dark = purlis().first();
    let files = purlis_files();
    for face in dark.fonts.ui.faces.iter().chain(&dark.fonts.mono.faces) {
        let bytes = served(&face.asset).unwrap();
        assert!(bytes.starts_with(b"wOF2"));
        assert!(files.values().any(|one| *one == bytes));
    }
}

#[test]
fn a_pick_names_it_and_purlis_classic_stays_the_default() {
    assert_eq!(Pick::parse(PURLIS), Some(Pick::BuiltIn(PURLIS)));
    assert_ne!(default_theme().name, "Purlis");
    assert_eq!(
        name_of(&drawn_for(&Levels::default(), Contrast::Standard)),
        crate::names::THEME_DARK.write
    );
    let chosen = Levels {
        project_choice: Choice::new(PURLIS, None),
        ..Levels::default()
    };
    let drawn = drawn_for(&chosen, Contrast::Standard);
    let theme = drawn.theme.unwrap();
    assert_eq!((theme.name.as_str(), theme.id.as_str()), ("Purlis", "dark"));
    // More contrast asked for draws its high-contrast dark.
    let sharp = drawn_for(&chosen, Contrast::More).theme.unwrap();
    assert_eq!(sharp.id, "dark-high-contrast");
    // An appearance chosen by its id is drawn, and the window asks the engine for it.
    let light = Levels {
        project_choice: Choice::new(PURLIS, Some("light")),
        ..Levels::default()
    };
    assert_eq!(
        drawn_for(&light, Contrast::Standard).theme.unwrap().id,
        "light"
    );
    assert_eq!(light.project_choice.unwrap().told(), PURLIS);
}

fn name_of(drawn: &Drawn) -> &str {
    drawn.theme.as_ref().map_or("", |it| it.name.as_str())
}

#[test]
fn no_theme_folder_takes_its_id_and_a_theme_may_extend_it() {
    assert!(is_reserved(PURLIS));
    assert!(!is_folder_id(PURLIS));
    let child = serde_json::json!({
        "name": "Mine",
        "extends": PURLIS,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": { "accent.base": "#b3a8f2" } }]
    });
    let read = Library::new().load(None, &child);
    assert!(!read.refused, "{:?}", read.complaints);
    let dark = read.theme.of(Appearance::Dark).unwrap();
    assert_eq!(dark.value("accent.base"), "#b3a8f2");
    assert_eq!(dark.value("surface.base"), "#17171b");
    assert_eq!(dark.fonts, purlis().first().fonts);
    assert_eq!(read.theme.appearances.len(), 3);
}

#[test]
fn the_gallery_shows_it_beside_purlis_classic_as_a_preview() {
    let folders = Folders::of(None, None, None);
    let cards = gallery::cards(&Shelf {
        folders: &folders,
        reach: Reach::Machine,
        on: On::Every,
        offered: &[],
    });
    let built_in: Vec<(&str, bool)> = cards
        .iter()
        .take(2)
        .map(|one| (one.id.as_str(), one.preview))
        .collect();
    assert_eq!(built_in, [(CLASSIC, false), (PURLIS, true)]);
    let purlis = &cards[1];
    assert_eq!(purlis.name, "Purlis");
    assert!(purlis.warnings.is_empty() && purlis.floors.is_empty());
    assert!(purlis.duplicable && !purlis.folder);
}
