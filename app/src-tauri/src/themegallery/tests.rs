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
fn the_machines_gallery_is_classic_and_the_persons_folders_and_says_what_is_drawn() {
    let home = tempfile::tempdir().unwrap();
    let place = machine(home.path());
    let shown = gallery_of(&place, None);
    let ids: Vec<&str> = shown.cards.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(ids, ["purlis-classic", "dusk"]);
    assert_eq!(shown.cards[1].from, "machine");
    assert_eq!(shown.cards[0].from, "built-in");
    assert_eq!(shown.yours, None);
    assert_eq!(shown.drawn.as_deref(), Some("purlis-classic"));
    assert_eq!(shown.level, "purlis");
    assert_eq!(shown.from, "nothing chose one");
}

#[test]
fn choosing_at_the_machines_level_is_the_machine_default_with_its_badge() {
    let home = tempfile::tempdir().unwrap();
    let place = machine(home.path());
    choose_machine_of(home.path(), Some("dusk"), Some("dark")).unwrap();
    let shown = gallery_of(&place, None);
    assert_eq!(
        shown.yours,
        Some(YourTheme {
            theme: "dusk".into(),
            appearance: Some("dark".into()),
        })
    );
    assert_eq!(shown.drawn.as_deref(), Some("dusk"));
    assert_eq!(shown.level, "machine-default");
    assert_eq!(shown.from, "your machine default");
    // Taken away, nothing is chosen again.
    choose_machine_of(home.path(), None, None).unwrap();
    assert_eq!(gallery_of(&place, None).yours, None);
    // A value no choice can hold keeps nothing.
    assert!(choose_machine_of(home.path(), Some("../dusk"), None).is_err());
}

#[test]
fn a_card_draws_its_theme_in_the_appearance_asked_for_and_nothing_else_is_drawn() {
    let home = tempfile::tempdir().unwrap();
    let place = machine(home.path());
    let dusk = card_drawn_of(&place, "dusk", None, false, false).unwrap();
    let theme = dusk.theme.unwrap();
    assert_eq!(theme.values["surface.base"], "#141414");
    let light = card_drawn_of(&place, "purlis-classic", Some("system"), true, false).unwrap();
    assert!(light.follows_system);
    assert_eq!(light.theme.unwrap().appearance, "light");
    assert!(card_drawn_of(&place, "nowhere", None, false, false).is_none());
}
