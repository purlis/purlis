// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One level's theme folders at the engine's interface (#2518): each read as a choice made at
//! that level reads it, for `purlis theme list` and `purlis theme check`; and what the window
//! draws of each combination the check covers.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::levels::Reach;
use super::*;

fn put(themes: &Path, folder: &str, document: &Value) {
    let dir = themes.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(THEME_JSON), document.to_string()).unwrap();
}

/// A one-appearance dark theme called `name`, extending `extends` when one is named.
fn named(name: &str, extends: Option<&str>) -> Value {
    let mut theme = json!({
        "name": name,
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    });
    if let Some(parent) = extends {
        theme["extends"] = Value::from(parent);
    }
    theme
}

/// A config home, a plane and its workspace `alpha`.
struct Places {
    _home: tempfile::TempDir,
    config: PathBuf,
    plane: PathBuf,
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

    fn machine(&self) -> PathBuf {
        crate::machine::dir(&self.config).join(levels::THEMES)
    }

    fn project(&self) -> PathBuf {
        self.plane.join(levels::THEMES)
    }

    fn workspace(&self) -> PathBuf {
        self.plane.join("workspaces/alpha").join(levels::THEMES)
    }
}

fn read_name(found: levels::Found) -> String {
    match found {
        levels::Found::Read(resolved) => {
            assert!(!resolved.refused, "{:?}", resolved.complaints);
            resolved.theme.name
        }
        levels::Found::Missing(said) => panic!("missing: {said:?}"),
    }
}

#[test]
fn a_level_s_own_folder_is_found_at_that_level_and_a_farther_one_s_is_not() {
    let places = Places::new();
    put(&places.machine(), "dusk", &named("Machine dusk", None));
    put(&places.project(), "dusk", &named("Project dusk", None));
    put(&places.machine(), "only-mine", &named("Only mine", None));
    let folders = places.folders();

    assert_eq!(
        read_name(folders.find_at("dusk", Reach::Project)),
        "Project dusk"
    );
    assert_eq!(
        read_name(folders.find_at("dusk", Reach::Machine)),
        "Machine dusk"
    );
    // The workspace has no `dusk`: the project's is not read in its place.
    assert!(matches!(
        folders.find_at("dusk", Reach::Workspace),
        levels::Found::Missing(_)
    ));
    assert!(matches!(
        folders.find_at("only-mine", Reach::Project),
        levels::Found::Missing(_)
    ));
}

#[test]
fn every_folder_of_a_level_is_read_as_a_choice_there_reads_it() {
    let places = Places::new();
    put(&places.machine(), "base", &named("Base", None));
    put(&places.project(), "team", &named("Team", Some("base")));
    put(
        &places.workspace(),
        "client",
        &named("Client", Some("team")),
    );
    put(&places.workspace(), "broken", &json!({ "name": 7 }));
    let folders = places.folders();

    let at = |reach| -> Vec<(String, bool)> {
        folders
            .every_at(reach)
            .into_iter()
            .map(|(name, resolved)| (name, resolved.refused))
            .collect()
    };

    // Each level lists its own folders only, and a nearer one extends a farther one's.
    assert_eq!(
        at(Reach::Workspace),
        [("broken".to_owned(), true), ("client".to_owned(), false)]
    );
    assert_eq!(at(Reach::Project), [("team".to_owned(), false)]);
    assert_eq!(at(Reach::Machine), [("base".to_owned(), false)]);
    let client = folders
        .every_at(Reach::Workspace)
        .into_iter()
        .find(|(name, _)| name == "client")
        .expect("the workspace's folder");
    assert_eq!(
        client.1,
        match folders.find("client", Reach::Workspace) {
            levels::Found::Read(resolved) => resolved,
            levels::Found::Missing(said) => panic!("{said:?}"),
        },
        "listed exactly as the window reads it"
    );
}

#[test]
fn a_level_with_no_themes_folder_lists_nothing() {
    let places = Places::new();
    put(&places.machine(), "base", &named("Base", None));

    assert!(places.folders().every_at(Reach::Project).is_empty());
    assert!(places.folders().every_at(Reach::Workspace).is_empty());
    assert!(
        Folders::of(Some(&places.config), None, None)
            .every_at(Reach::Project)
            .is_empty(),
        "no plane, no project level"
    );
}

#[cfg(unix)]
#[test]
fn a_level_reached_through_a_link_lists_nothing_and_finds_nothing() {
    let places = Places::new();
    let elsewhere = places.plane.join("elsewhere");
    put(&elsewhere, "dusk", &named("Dusk", None));
    std::os::unix::fs::symlink(&elsewhere, places.project()).unwrap();

    let folders = places.folders();

    assert!(folders.every_at(Reach::Project).is_empty());
    match folders.find_at("dusk", Reach::Project) {
        levels::Found::Missing(said) => {
            assert!(
                said.iter().any(|one| one.said.contains("through a link")),
                "{said:?}"
            );
        }
        levels::Found::Read(resolved) => panic!("read through a link: {resolved:?}"),
    }
}

#[test]
fn the_check_says_which_consent_floors_the_window_holds_in_each_combination() {
    assert!(
        check_every(classic()).iter().all(|one| one.held.is_none()),
        "Purlis Classic is drawn as it is"
    );
    // Danger drawn in the accent's colour: told apart from primary by nothing.
    let resolved = load(&json!({
        "name": "Same",
        "appearances": [
            { "id": "dark", "appearance": "dark",
              "tokens": { "danger.base": built_in_for(Appearance::Dark).value("accent.base") } },
            { "id": "light", "appearance": "light" }
        ]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);

    let checked = check_every(&resolved.theme);

    let dark = checked
        .iter()
        .find(|one| one.appearance == "dark")
        .expect("the dark appearance");
    let held = dark.held.as_ref().expect("a floor is held");
    assert!(held.floors.contains(&Floor::DangerApart), "{held:?}");
    assert!(held.tokens.contains(&"danger.base"), "{held:?}");
    assert_eq!(
        Some(held),
        draw(&resolved.theme.appearances[0]).held.as_ref(),
        "what the window draws"
    );
    let light = checked
        .iter()
        .find(|one| one.appearance == "light")
        .expect("the light appearance");
    assert_eq!(light.held, None);
}
