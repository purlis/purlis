// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The gallery as the window is handed it, at the machine's level: its cards, the person's
//! machine default as it is chosen, and what a card draws.

use super::*;

/// The machine's level under a config home of its own, with a theme folder of the person's.
fn machine(config: &Path) -> Place {
    let themes = purlis_core::machine::dir(config).join(theme::levels::THEMES);
    let dusk = themes.join("dusk");
    std::fs::create_dir_all(&dusk).unwrap();
    std::fs::write(
        dusk.join(theme::THEME_JSON),
        serde_json::json!({
            "name": "Dusk",
            "extends": "purlis-classic",
            "appearances": [{ "id": "dark", "tokens": { "surface.base": "#141414" } }]
        })
        .to_string(),
    )
    .unwrap();
    Place {
        config: config.to_path_buf(),
        plane: None,
        workspace: None,
        folders: Folders::of(Some(config), None, None),
        reach: Reach::Machine,
        on: None,
        offered: Arc::from(Vec::new()),
    }
}

#[test]
fn the_machines_gallery_is_the_built_ins_and_the_persons_folders_and_says_what_is_drawn() {
    let home = tempfile::tempdir().unwrap();
    let place = machine(home.path());
    let shown = gallery_of(&place, None);
    let ids: Vec<&str> = shown.cards.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(ids, ["purlis", "purlis-classic", "dusk"]);
    assert_eq!(shown.cards[2].from, "machine");
    assert_eq!(shown.cards[0].from, "built-in");
    // Purlis is built in and the default, first and drawn where nothing chose (#3034); Purlis
    // Classic beside it, and neither is a preview.
    assert_eq!(shown.cards[1].from, "built-in");
    assert!(!shown.cards[0].preview && !shown.cards[1].preview);
    assert_eq!(shown.yours, None);
    assert_eq!(shown.drawn.as_deref(), Some("purlis"));
    assert_eq!(shown.chosen.as_deref(), Some("purlis"));
    assert_eq!(shown.level, "purlis");
    assert_eq!(shown.from, "nothing chose one");
}

#[test]
fn choosing_at_the_machines_level_is_the_machine_default_with_its_badge() {
    let home = tempfile::tempdir().unwrap();
    let place = machine(home.path());
    choose_machine_of(home.path(), Some("dusk"), Some("dark"), None).unwrap();
    let shown = gallery_of(&place, None);
    // Written, the choice names the level of the folder it picked (#2516).
    assert_eq!(
        shown.yours,
        Some(YourTheme {
            theme: "dusk".into(),
            appearance: Some("dark".into()),
            from: Some("machine".into()),
            options: Default::default(),
        })
    );
    assert_eq!(shown.drawn.as_deref(), Some("machine:dusk"));
    assert_eq!(shown.chosen.as_deref(), Some("machine:dusk"));
    assert_eq!(shown.level, "machine-default");
    assert_eq!(shown.from, "your machine default");
    // Taken away, nothing is chosen again.
    choose_machine_of(home.path(), None, None, None).unwrap();
    assert_eq!(gallery_of(&place, None).yours, None);
    // A value no choice can hold keeps nothing, nor a level the machine's choice cannot reach.
    assert!(choose_machine_of(home.path(), Some("../dusk"), None, None).is_err());
    assert!(choose_machine_of(home.path(), Some("dusk"), None, Some("project")).is_err());
    assert!(choose_machine_of(home.path(), Some("dusk"), None, Some("elsewhere")).is_err());
}

#[test]
fn a_card_draws_its_theme_in_the_appearance_asked_for_and_nothing_else_is_drawn() {
    let home = tempfile::tempdir().unwrap();
    let place = machine(home.path());
    let none = Values::new();
    let dusk = card_drawn_of(
        &place,
        "dusk",
        None,
        None,
        &none,
        false,
        false,
        &purlis_core::theme::Layer::default(),
    )
    .unwrap();
    let theme = dusk.theme.unwrap();
    assert_eq!(theme.values["surface.base"], "#141414");
    let light = card_drawn_of(
        &place,
        "purlis-classic",
        None,
        Some("system"),
        &none,
        true,
        false,
        &purlis_core::theme::Layer::default(),
    )
    .unwrap();
    assert!(light.follows_system);
    assert_eq!(light.theme.unwrap().appearance, "light");
    assert!(
        card_drawn_of(
            &place,
            "nowhere",
            None,
            None,
            &none,
            false,
            false,
            &purlis_core::theme::Layer::default()
        )
        .is_none()
    );
}

/// The machine's level with a theme folder `accented` of the person's, whose accent is an
/// option: lavender by default, or teal.
fn accented(config: &Path) -> Place {
    let place = machine(config);
    let folder = purlis_core::machine::dir(config)
        .join(theme::levels::THEMES)
        .join("accented");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join(theme::THEME_JSON),
        serde_json::json!({
            "name": "Accented",
            "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {} }],
            "options": [{
                "id": "accent", "kind": "choice", "label": "Accent", "default": "lavender",
                "values": [
                    { "id": "lavender", "label": "Lavender", "tokens": { "accent.base": "#b4a7f5" } },
                    { "id": "teal", "label": "Teal", "tokens": { "accent.base": "#2bb3a3" } }
                ]
            }]
        })
        .to_string(),
    )
    .unwrap();
    Place {
        folders: Folders::of(Some(config), None, None),
        ..place
    }
}

#[test]
fn an_option_set_at_the_level_is_kept_drawn_and_badged_with_the_level() {
    let home = tempfile::tempdir().unwrap();
    let place = accented(home.path());
    choose_machine_of(home.path(), Some("accented"), None, Some("machine")).unwrap();
    let shown = gallery_of(&place, None);
    assert_eq!(shown.options[0].value, "lavender");
    assert_eq!(shown.options[0].level, None);

    assert_eq!(
        choose_option_of(&place, None, "accent", Some("teal")).unwrap(),
        Vec::<String>::new()
    );
    let shown = gallery_of(&place, None);
    assert_eq!(shown.options[0].value, "teal");
    assert!(shown.options[0].chosen);
    assert_eq!(shown.options[0].level.as_deref(), Some("machine-default"));
    // A card of the chosen theme is drawn, pointed at and pictured, with the value chosen here.
    assert_eq!(
        place.values_for("accented", Some(Reach::Machine)),
        [("accent".to_owned(), "teal".to_owned())].into()
    );
    assert!(place.values_for("dusk", Some(Reach::Machine)).is_empty());
    // Choosing another appearance of the same theme keeps the value.
    choose_machine_of(home.path(), Some("accented"), Some("dark"), Some("machine")).unwrap();
    assert_eq!(gallery_of(&place, None).options[0].value, "teal");
    // Back to the theme's default.
    choose_option_of(&place, None, "accent", None).unwrap();
    assert_eq!(gallery_of(&place, None).options[0].value, "lavender");
    // A value the option does not have keeps nothing.
    assert!(choose_option_of(&place, None, "accent", Some("pink")).is_err());
}

#[test]
fn an_option_set_where_nothing_was_chosen_makes_the_drawn_choice_the_persons() {
    let home = tempfile::tempdir().unwrap();
    let place = accented(home.path());
    // Nothing is chosen: Purlis Classic, which has no options, refuses one.
    assert!(choose_option_of(&place, None, "accent", Some("teal")).is_err());
    assert_eq!(gallery_of(&place, None).yours, None);
}

/// The person's own `theme.json` of one dark appearance, called `Owl`.
fn owl() -> theme::Theme {
    theme::load(&serde_json::json!({
        "name": "Owl",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {} }]
    }))
    .theme
}

#[test]
fn the_persons_own_theme_json_drawn_is_said_with_what_following_does_with_it() {
    let home = tempfile::tempdir().unwrap();
    let place = machine(home.path());
    let mine = owl();
    // Drawn with no choice: theirs, in its one appearance, no card chosen.
    let shown = gallery_of(&place, Some(&mine));
    assert_eq!(shown.chosen, None);
    assert_eq!(
        shown.theirs,
        Some(TheirsDrawn {
            follows_system: false,
            note: None
        })
    );
    // The old `system` id chosen: theirs, followed, staying in its one appearance (#2615).
    choose_machine_of(home.path(), Some("system"), None, None).unwrap();
    let shown = gallery_of(&place, Some(&mine));
    assert_eq!(shown.chosen, None);
    assert_eq!(
        shown.theirs,
        Some(TheirsDrawn {
            follows_system: false,
            note: Some("Owl has only a dark appearance, so following the system stays dark".into()),
        })
    );
    // With no theme of the person's, the old id draws Purlis Classic's card.
    let shown = gallery_of(&place, None);
    assert_eq!(shown.theirs, None);
    assert_eq!(shown.chosen.as_deref(), Some("purlis-classic"));
}

#[test]
fn a_place_offers_an_extension_s_themes_once_approved_and_has_them_on_where_its_project_does() {
    // #2751: what a chat's specimen and the gallery reach of an extension's themes is the
    // place's `on` and `offered`, read here as the window reads them.
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join("config");
    let ext = home.path().join("ext");
    std::fs::create_dir_all(&ext).unwrap();
    std::fs::write(
        ext.join(extension::MANIFEST),
        r#"{"version":1,"id":"solarized","name":"Solarized",
            "contributes":{"themes":[{"name":"Solarized Dark","file":"dark.json"}],
                           "panels":[{"id":"p","title":"P"}]}}"#,
    )
    .unwrap();
    std::fs::write(
        ext.join("dark.json"),
        r##"{"name":"Solarized Dark","appearance":"dark","tokens":{"surface.base":"#101010"}}"##,
    )
    .unwrap();
    let plane = home.path().join("plane");
    for dir in [".purlis", "workspaces/alpha", "workspaces/beta"] {
        std::fs::create_dir_all(plane.join(dir)).unwrap();
    }
    std::fs::write(
        plane.join("workspaces/beta/workspace.json"),
        r#"{"settings": {"extensions": {"solarized": {"enabled": false}}}}"#,
    )
    .unwrap();
    let survey = |config: &Path| {
        crate::extensions::offered_of(&extension::survey(config, &extension::BuiltIn::none()))
    };
    let place = |workspace: &str| {
        Place::offering(
            config.clone(),
            Some(plane.clone()),
            Some(workspace.to_owned()),
            survey,
        )
    };
    let drawn = |place: &Place, workspace: &str| {
        purlis_core::themepreview::resolved(
            &purlis_core::themepreview::Reached {
                config: Some(&config),
                plane: Some(&plane),
                workspace: Some(workspace),
                chat: None,
                shelf: place.shelf(),
            },
            &purlis_core::themepreview::Specimen {
                theme: "solarized/Solarized Dark".to_owned(),
                appearance: None,
                high_contrast: false,
                options: Values::new(),
            },
        )
        .map(|drawn| {
            drawn
                .theme
                .expect("a theme")
                .value("surface.base")
                .to_owned()
        })
    };
    let found = extension::install(&config, &extension::BuiltIn::none(), &ext).unwrap();
    // Installed and on, but not approved (it brings a panel, so it is asked about): nothing of
    // it is offered, so nothing is drawn.
    let unapproved = place("alpha");
    assert!(unapproved.offered.is_empty());
    assert!(drawn(&unapproved, "alpha").is_err());
    extension::approve(&config, found.id(), &found.path, &found.fingerprint).unwrap();
    let alpha = place("alpha");
    let offered: Vec<(&str, &str)> = alpha
        .offered
        .iter()
        .map(|one| (one.extension.as_str(), one.name.as_str()))
        .collect();
    assert_eq!(offered, [("solarized", "Solarized Dark")]);
    assert_eq!(alpha.on.as_deref(), Some(&["solarized".to_owned()][..]));
    assert_eq!(drawn(&alpha, "alpha").as_deref(), Ok("#101010"));
    // Off where the workspace turned it off: offered still, on nowhere there, and not drawn.
    let beta = place("beta");
    assert_eq!(beta.offered.len(), 1);
    assert_eq!(beta.on.as_deref(), Some(&[][..]));
    assert!(drawn(&beta, "beta").is_err());
}
