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

// ---------------------------------------------------------------------------------------
// The first frame: the levels resolved once at creation (#2602)
// ---------------------------------------------------------------------------------------

/// A machine config home and a plane with a workspace `alpha`.
struct Places {
    _home: tempfile::TempDir,
    config: std::path::PathBuf,
    plane: std::path::PathBuf,
}

impl Places {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config");
        let plane = home.path().join("plane");
        std::fs::create_dir_all(plane.join("workspaces/alpha")).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        Self {
            _home: home,
            config,
            plane,
        }
    }

    fn folders(&self) -> Folders {
        Folders::of(Some(&self.config), Some(&self.plane), Some("alpha"))
    }

    /// Puts the theme folder `name` holding `document` in the person's own `themes/`, the
    /// project's or the workspace's.
    fn put(&self, level: Level, name: &str, document: &Value) {
        let themes = match level {
            Level::MachineDefault => crate::machine::dir(&self.config).join(levels::THEMES),
            Level::ProjectChoice | Level::ProjectDefault => self.plane.join(levels::THEMES),
            _ => self.plane.join("workspaces/alpha").join(levels::THEMES),
        };
        let dir = themes.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(THEME_JSON), document.to_string()).unwrap();
    }
}

fn created<'a>(
    levels: &'a Levels,
    folders: &'a Folders,
    theirs: Option<&'a Theme>,
    system: Appearance,
    contrast: Contrast,
) -> LevelInputs<'a> {
    LevelInputs {
        levels,
        folders,
        theirs,
        on: On::Every,
        system,
        contrast,
    }
}

/// A one-appearance dark theme called `name`.
fn named(name: &str, surface: &str) -> Value {
    json!({
        "name": name,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": { "surface.base": surface } }]
    })
}

#[test]
fn the_first_frame_is_the_theme_each_level_chooses_and_what_the_window_is_answered_after() {
    let places = Places::new();
    let theirs = mine(&named("Mine", "#202020"));
    places.put(
        Level::MachineDefault,
        "machine",
        &named("Machine", "#101010"),
    );
    places.put(Level::ProjectDefault, "team", &named("Team", "#111111"));
    places.put(Level::WorkspaceChoice, "client", &sharp());
    let folders = places.folders();
    for (levels, name) in [
        (
            Levels {
                machine_default: Choice::new("machine", None),
                ..Levels::default()
            },
            "Machine",
        ),
        (
            Levels {
                project_default: Choice::new("team", None),
                machine_default: Choice::new("machine", None),
                ..Levels::default()
            },
            "Team",
        ),
        (
            Levels {
                project_choice: Choice::new("team", None),
                ..Levels::default()
            },
            "Team",
        ),
        (
            Levels {
                workspace_choice: Choice::new("client", Some("light")),
                project_default: Choice::new("team", None),
                ..Levels::default()
            },
            "Sharp",
        ),
    ] {
        let inputs = created(
            &levels,
            &folders,
            Some(&theirs),
            Appearance::Dark,
            Contrast::Standard,
        );
        let first = first_frame(&inputs);
        assert_eq!(first.theme.as_ref().map(|it| it.name.as_str()), Some(name));
        // What the window is answered once it asks (`theme_in_force`) is the frame it drew:
        // nothing is drawn again.
        let answered = resolve_levels(&inputs, || unreachable!("no extension is named"));
        assert_eq!(first.theme, answered.theme);
        // And the native window is created in that frame's appearance and colour.
        let native = first.theme.as_ref().expect("a theme").native();
        assert_eq!(native, answered.theme.expect("a theme").native());
    }
}

#[test]
fn the_first_frame_with_no_level_chosen_is_the_person_s_theme_for_the_contrast_else_purlis() {
    let theme = mine(&sharp());
    let nothing = Levels::default();
    let folders = Folders::default();
    let asked = |theirs, contrast| {
        first_frame(&created(
            &nothing,
            &folders,
            theirs,
            Appearance::Light,
            contrast,
        ))
    };
    assert_eq!(drawn_id(&asked(Some(&theme), Contrast::Standard)), "dark");
    assert_eq!(drawn_id(&asked(Some(&theme), Contrast::More)), "contrast");
    // Nothing chose and no theme of the person's: the default, Purlis, in its high-contrast
    // dark for more contrast (#3034).
    let none = asked(None, Contrast::More);
    assert_eq!(none.theme.as_ref(), Some(default_for(Contrast::More)));
    let drawn = none.theme.expect("drawn");
    assert_eq!(
        (drawn.name.as_str(), drawn.id.as_str()),
        ("Purlis", "dark-high-contrast")
    );
    let standard = asked(None, Contrast::Standard).theme.expect("drawn");
    assert_eq!(&standard, default_theme());
    assert_eq!(standard.native().background, [0x12, 0x12, 0x15]);
}

#[test]
fn the_first_frame_draws_the_high_contrast_appearance_a_level_s_choice_has_for_more_contrast() {
    let places = Places::new();
    places.put(Level::ProjectChoice, "sharp", &sharp());
    let folders = places.folders();
    let levels = Levels {
        project_choice: Choice::new("sharp", None),
        ..Levels::default()
    };
    let asked = |contrast| {
        first_frame(&created(
            &levels,
            &folders,
            None,
            Appearance::Dark,
            contrast,
        ))
    };
    let more = asked(Contrast::More);
    assert_eq!(drawn_id(&more), "contrast");
    assert_eq!(
        more.theme.expect("a theme").native().background,
        [0, 0, 0],
        "the window is created in the high-contrast appearance's window.background"
    );
    assert_eq!(drawn_id(&asked(Contrast::Standard)), "dark");
}

#[test]
fn a_first_frame_that_follows_the_system_is_drawn_for_the_system_s_appearance() {
    let places = Places::new();
    places.put(Level::ProjectDefault, "sharp", &sharp());
    let folders = places.folders();
    let levels = Levels {
        project_default: Choice::new("sharp", Some("system")),
        ..Levels::default()
    };
    for (system, id) in [(Appearance::Dark, "dark"), (Appearance::Light, "light")] {
        let first = first_frame(&created(
            &levels,
            &folders,
            None,
            system,
            Contrast::Standard,
        ));
        assert!(first.follows_system);
        assert_eq!(drawn_id(&first), id);
    }
}

#[test]
fn a_first_frame_that_needs_an_extension_s_theme_is_the_person_s_own_until_the_window_asks() {
    // The survey is a read and a fingerprint of every extension's files: the first frame never
    // waits for it, so a level that chose an extension's theme draws what the window drew
    // before the levels, and the window draws the answer once it asks.
    let theme = mine(&sharp());
    let folders = Folders::default();
    let levels = Levels {
        project_choice: Choice::new("acme/sharp", None),
        ..Levels::default()
    };
    let asked = |theirs, contrast| {
        first_frame(&created(
            &levels,
            &folders,
            theirs,
            Appearance::Dark,
            contrast,
        ))
    };
    assert_eq!(
        asked(Some(&theme), Contrast::More),
        draw(theme.first_for(Contrast::More))
    );
    let none = asked(None, Contrast::Standard);
    assert_eq!(none, draw(default_theme()));
    assert!(none.complaints.is_empty(), "{:?}", none.complaints);
}

#[test]
fn the_first_frame_holds_the_consent_floors() {
    let lost = default_theme().value("surface.base").to_owned();
    let breaks = json!({
        "name": "Mine",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": { "state.failed": lost } }]
    });
    // The person's own theme, at level 5.
    let theme = mine(&breaks);
    let nothing = Levels::default();
    let none = Folders::default();
    let drawn = first_frame(&created(
        &nothing,
        &none,
        Some(&theme),
        Appearance::Dark,
        Contrast::Standard,
    ));
    assert!(drawn.held.is_some());
    let held = draw(theme.first());
    assert_eq!((&drawn.theme, &drawn.held), (&held.theme, &held.held));
    // And a theme folder a level chose.
    let places = Places::new();
    places.put(Level::MachineDefault, "mine", &breaks);
    let folders = places.folders();
    let levels = Levels {
        machine_default: Choice::new("mine", None),
        ..Levels::default()
    };
    let chosen = first_frame(&created(
        &levels,
        &folders,
        None,
        Appearance::Dark,
        Contrast::Standard,
    ));
    assert!(chosen.held.is_some());
    assert_eq!(chosen.theme, draw(theme.first()).theme);
}

#[test]
fn an_appearance_a_theme_does_not_have_draws_its_first_for_the_contrast_asked() {
    let places = Places::new();
    places.put(Level::ProjectChoice, "sharp", &sharp());
    let folders = places.folders();
    let levels = Levels {
        project_choice: Choice::new("sharp", Some("dawn")),
        ..Levels::default()
    };
    let asked = |contrast| {
        resolve_levels(
            &created(&levels, &folders, None, Appearance::Dark, contrast),
            || unreachable!("no extension is named"),
        )
    };
    let more = asked(Contrast::More);
    assert_eq!(drawn_id(&more), "contrast");
    assert!(
        more.complaints
            .iter()
            .any(|one| one.said.contains("no appearance called dawn")),
        "{:?}",
        more.complaints
    );
    assert_eq!(drawn_id(&asked(Contrast::Standard)), "dark");
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
