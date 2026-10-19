// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a choice carries, at the engine's interface: the option values the person chose for its
//! theme at each level (#2527), and the level of the theme folder it picked (#2516).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use super::choices::{self, Slot};
use super::gallery::{self, From, Shelf};
use super::levels::Reach;
use super::*;

const PROJECT: &str = "01JABCDEFGHJKMNPQRSTVWXYZ0";

const LAVENDER: &str = "#b4a7f5";
const TEAL: &str = "#2bb3a3";

fn no_survey() -> Arc<[Offered]> {
    Arc::from(Vec::new())
}

fn put(themes: &Path, folder: &str, document: &Value) {
    let dir = themes.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(THEME_JSON), document.to_string()).unwrap();
}

/// A dark theme called `name` with an accent option (lavender by default, or teal), and a
/// `loud` toggle whose `on` draws danger's words in its own surface's colour.
fn accented(name: &str) -> Value {
    json!({
        "name": name,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {} }],
        "options": [
            { "id": "accent", "kind": "choice", "label": "Accent", "default": "lavender",
              "values": [
                { "id": "lavender", "label": "Lavender", "tokens": { "accent.base": LAVENDER } },
                { "id": "teal", "label": "Teal", "tokens": { "accent.base": TEAL } }
              ] },
            { "id": "loud", "kind": "toggle", "label": "Loud", "default": false,
              "on": { "tokens": { "danger.text": "#301010", "danger.surface": "#301010" } } }
        ]
    })
}

/// A one-appearance dark theme called `name`.
fn named(name: &str) -> Value {
    json!({
        "name": name,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {} }]
    })
}

/// A machine config home, a plane and its workspace `alpha`.
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

    fn themes(&self, level: Reach) -> PathBuf {
        match level {
            Reach::Machine => crate::machine::dir(&self.config).join(levels::THEMES),
            Reach::Project => self.plane.join(levels::THEMES),
            Reach::Workspace => self.plane.join("workspaces/alpha").join(levels::THEMES),
        }
    }
}

/// A plane whose every level has a theme folder `brand`, each named for its level.
fn brands() -> Places {
    let places = Places::new();
    for (level, name) in [
        (Reach::Machine, "Machine's"),
        (Reach::Project, "Project's"),
        (Reach::Workspace, "Workspace's"),
    ] {
        put(&places.themes(level), "brand", &named(name));
    }
    places
}

fn drawn(levels: &Levels, folders: &Folders) -> Drawn {
    resolve_levels(
        &LevelInputs {
            levels,
            folders,
            theirs: None,
            on: On::Every,
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        no_survey,
    )
}

fn values(pairs: &[(&str, &str)]) -> Values {
    pairs
        .iter()
        .map(|(id, value)| ((*id).to_owned(), (*value).to_owned()))
        .collect()
}

fn choice(theme: &str, chosen: &[(&str, &str)]) -> Option<Choice> {
    Choice::new(theme, None).map(|one| one.with_options(values(chosen)))
}

fn accent_of(drawn: &Drawn) -> &str {
    drawn
        .theme
        .as_ref()
        .expect("a theme is drawn")
        .value("accent.base")
}

fn name_of(drawn: &Drawn) -> &str {
    &drawn.theme.as_ref().expect("a theme is drawn").name
}

/// Each level of the person's, with `chosen` made there and nothing anywhere else.
fn at_each_level(chosen: &Option<Choice>) -> Vec<(Level, Levels)> {
    vec![
        (
            Level::WorkspaceChoice,
            Levels {
                workspace_choice: chosen.clone(),
                ..Levels::default()
            },
        ),
        (
            Level::ProjectChoice,
            Levels {
                project_choice: chosen.clone(),
                ..Levels::default()
            },
        ),
        (
            Level::MachineDefault,
            Levels {
                machine_default: chosen.clone(),
                ..Levels::default()
            },
        ),
    ]
}

// Option values at each level (#2527) --------------------------------------------------------

#[test]
fn a_choice_s_option_values_are_drawn_at_every_level() {
    let places = Places::new();
    put(
        &places.themes(Reach::Machine),
        "accented",
        &accented("Accented"),
    );
    for (level, levels) in at_each_level(&choice("accented", &[("accent", "teal")])) {
        let got = drawn(&levels, &places.folders());
        assert_eq!(got.level, Some(level));
        assert_eq!(accent_of(&got), TEAL, "{level:?}");
        let theme = got.theme.as_ref().unwrap();
        let accent = theme.options.iter().find(|one| one.option.id == "accent");
        assert_eq!(
            accent.map(|one| (one.value.as_str(), one.chosen)),
            Some(("teal", true))
        );
        assert_eq!(got.complaints, [], "{level:?}");
    }
}

#[test]
fn an_option_with_no_value_chosen_draws_the_theme_s_default() {
    let places = Places::new();
    put(
        &places.themes(Reach::Machine),
        "accented",
        &accented("Accented"),
    );
    for (level, levels) in at_each_level(&choice("accented", &[])) {
        let got = drawn(&levels, &places.folders());
        assert_eq!(accent_of(&got), LAVENDER, "{level:?}");
        let theme = got.theme.as_ref().unwrap();
        assert!(theme.options.iter().all(|one| !one.chosen), "{level:?}");
    }
}

#[test]
fn the_nearest_level_s_values_are_drawn_and_never_a_farther_one_s() {
    let places = Places::new();
    put(
        &places.themes(Reach::Machine),
        "accented",
        &accented("Accented"),
    );
    // The workspace chose the theme and no value: its default, not the project's teal.
    let levels = Levels {
        workspace_choice: choice("accented", &[]),
        project_choice: choice("accented", &[("accent", "teal")]),
        ..Levels::default()
    };
    assert_eq!(accent_of(&drawn(&levels, &places.folders())), LAVENDER);
    // The workspace's value beats the project's.
    let levels = Levels {
        workspace_choice: choice("accented", &[("accent", "teal")]),
        project_choice: choice("accented", &[("accent", "lavender")]),
        ..Levels::default()
    };
    assert_eq!(accent_of(&drawn(&levels, &places.folders())), TEAL);
    // With nothing chosen nearer, the project's.
    let levels = Levels {
        project_choice: choice("accented", &[("accent", "teal")]),
        machine_default: choice("accented", &[("accent", "lavender")]),
        ..Levels::default()
    };
    assert_eq!(accent_of(&drawn(&levels, &places.folders())), TEAL);
}

#[test]
fn a_value_the_theme_does_not_have_draws_its_default_and_is_said() {
    let places = Places::new();
    put(
        &places.themes(Reach::Machine),
        "accented",
        &accented("Accented"),
    );
    let levels = Levels {
        project_choice: choice("accented", &[("accent", "pink"), ("gone", "on")]),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders());
    assert_eq!(accent_of(&got), LAVENDER);
    let said: Vec<&str> = got.complaints.iter().map(|one| one.said.as_str()).collect();
    assert_eq!(
        said,
        [
            "the option accent of Accented has no value \"pink\", so its default, lavender, is \
             drawn",
            "Accented has no option \"gone\", so it is left out",
        ]
    );
}

#[test]
fn option_values_on_purlis_classic_are_said_and_draw_it_unchanged() {
    let places = Places::new();
    let levels = Levels {
        project_choice: choice("purlis-light", &[("accent", "teal")]),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders());
    let plain = drawn(
        &Levels {
            project_choice: choice("purlis-light", &[]),
            ..Levels::default()
        },
        &places.folders(),
    );
    assert_eq!(got.theme, plain.theme);
    assert_eq!(got.complaints.len(), 1, "{:?}", got.complaints);
}

#[test]
fn a_chosen_value_that_breaks_a_consent_floor_is_drawn_with_the_floor_held() {
    let places = Places::new();
    put(
        &places.themes(Reach::Machine),
        "accented",
        &accented("Accented"),
    );
    let purlis = built_in_for(Appearance::Dark);
    for (level, levels) in at_each_level(&choice("accented", &[("loud", "on")])) {
        let got = drawn(&levels, &places.folders());
        let theme = got.theme.as_ref().unwrap();
        // The value was drawn onto the theme before the floors were held, so they hold over it.
        assert!(got.held.is_some(), "{level:?}");
        assert_eq!(theme.value("danger.text"), purlis.value("danger.text"));
        assert_eq!(
            theme.value("danger.surface"),
            purlis.value("danger.surface")
        );
    }
    // A card in the gallery, drawn with the same value, is held the same way.
    let folders = places.folders();
    let shelf = Shelf {
        folders: &folders,
        reach: Reach::Workspace,
        on: On::Every,
        offered: &[],
    };
    let card = gallery::drawn(
        &shelf,
        "accented",
        None,
        None,
        &values(&[("loud", "on")]),
        Appearance::Dark,
        Contrast::Standard,
    )
    .unwrap();
    assert!(card.held.is_some());
    assert_eq!(
        card.theme.as_ref().unwrap().value("danger.text"),
        purlis.value("danger.text")
    );
    // With the toggle off, nothing is held.
    let plain = gallery::drawn(
        &shelf,
        "accented",
        None,
        None,
        &Values::new(),
        Appearance::Dark,
        Contrast::Standard,
    )
    .unwrap();
    assert!(plain.held.is_none());
}

// The choices file ----------------------------------------------------------------------------

#[test]
fn option_values_and_the_folder_s_level_are_kept_in_the_choices_file() {
    let places = Places::new();
    let mine = choice("accented", &[("accent", "teal"), ("loud", "on")])
        .unwrap()
        .at(Reach::Project);
    choices::set(
        &places.config,
        Slot::Workspace(PROJECT, "alpha"),
        Some(mine.clone()),
    )
    .unwrap();
    choices::set(&places.config, Slot::Machine, choice("purlis-light", &[])).unwrap();
    let held = choices::read(&places.config);
    assert_eq!(held.projects[PROJECT].workspaces["alpha"], mine);
    assert_eq!(held.machine, choice("purlis-light", &[]));
    let file: Value = serde_json::from_str(
        &std::fs::read_to_string(crate::machine::dir(&places.config).join(choices::FILE)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        file["projects"][PROJECT]["workspaces"]["alpha"],
        json!({
            "theme": "accented",
            "from": "project",
            "options": { "accent": "teal", "loud": "on" }
        })
    );
    // A choice with no level and no values is written as before.
    assert_eq!(file["machine"], json!({ "theme": "purlis-light" }));
}

#[test]
fn a_level_that_is_none_leaves_the_choice_out_and_a_value_that_is_no_id_is_left_out() {
    let places = Places::new();
    let dir = crate::machine::private_dir(&places.config).unwrap();
    std::fs::write(
        dir.join(choices::FILE),
        json!({
            "version": 1,
            "machine": { "theme": "accented", "from": "elsewhere" },
            "projects": {
                PROJECT: {
                    "theme": "accented",
                    "options": { "accent": "teal", "Bad Id": "on", "loud": 1, "x": "../y" },
                    "workspaces": {
                        "alpha": { "theme": "accented", "options": ["teal"] },
                        "beta": { "theme": "accented", "from": 3 }
                    }
                }
            }
        })
        .to_string(),
    )
    .unwrap();
    let held = choices::read(&places.config);
    assert_eq!(held.machine, None);
    let mine = &held.projects[PROJECT];
    assert_eq!(mine.project, choice("accented", &[("accent", "teal")]));
    assert_eq!(mine.workspaces.keys().collect::<Vec<_>>(), ["alpha"]);
    assert_eq!(mine.workspaces["alpha"], choice("accented", &[]).unwrap());
}

#[test]
fn a_level_named_for_a_theme_that_is_no_folder_is_not_kept() {
    let chosen = Choice::new("purlis-light", None)
        .unwrap()
        .at(Reach::Project);
    assert_eq!(chosen.from, None);
    let chosen = Choice::new("brand", None).unwrap().at(Reach::Project);
    assert_eq!(chosen.from, Some(Reach::Project));
}

// The level of the folder a choice picked (#2516) -------------------------------------------

#[test]
fn a_choice_of_one_level_s_folder_draws_that_folder_whatever_shares_its_name() {
    let places = brands();
    for (level, name) in [
        (Reach::Workspace, "Workspace's"),
        (Reach::Project, "Project's"),
        (Reach::Machine, "Machine's"),
    ] {
        let levels = Levels {
            workspace_choice: Choice::new("brand", None).map(|one| one.at(level)),
            ..Levels::default()
        };
        assert_eq!(
            name_of(&drawn(&levels, &places.folders())),
            name,
            "{level:?}"
        );
    }
}

#[test]
fn a_folder_of_the_name_at_another_level_is_never_drawn_in_its_place() {
    let places = brands();
    std::fs::remove_dir_all(places.themes(Reach::Machine).join("brand")).unwrap();
    // The person's own `brand` is gone: the project's and the workspace's `brand` stay theirs,
    // and the choice is passed over with a sentence.
    let levels = Levels {
        workspace_choice: Choice::new("brand", None).map(|one| one.at(Reach::Machine)),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders());
    assert_eq!(got.level, Some(Level::Purlis));
    assert_eq!(got.theme.as_ref(), Some(default_theme()));
    assert!(
        got.complaints
            .iter()
            .any(|one| one.said.contains("brand names no theme folder")),
        "{:?}",
        got.complaints
    );
}

#[test]
fn a_choice_naming_a_level_nearer_than_it_reaches_is_passed_over() {
    let places = brands();
    let levels = Levels {
        machine_default: Choice::new("brand", None).map(|one| one.at(Reach::Project)),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders());
    assert_eq!(got.level, Some(Level::Purlis));
    assert!(
        got.complaints
            .iter()
            .any(|one| one.said.contains("project's theme folders")),
        "{:?}",
        got.complaints
    );
}

#[test]
fn a_choice_with_no_level_reads_the_nearest_as_before_and_gains_its_level_when_written() {
    let places = brands();
    let old = Choice::new("brand", None).unwrap();
    let levels = Levels {
        workspace_choice: Some(old.clone()),
        project_choice: Some(old.clone()),
        ..Levels::default()
    };
    assert_eq!(name_of(&drawn(&levels, &places.folders())), "Workspace's");
    // Written, it names the level it reads now: the workspace's for the workspace, the
    // project's for the project, the person's for the machine.
    let folders = places.folders();
    assert_eq!(
        old.clone().placed(&folders, Reach::Workspace).from,
        Some(Reach::Workspace)
    );
    assert_eq!(
        old.clone().placed(&folders, Reach::Project).from,
        Some(Reach::Project)
    );
    assert_eq!(
        old.clone().placed(&folders, Reach::Machine).from,
        Some(Reach::Machine)
    );
    // One that names its level keeps it; one no level has stays without one.
    let named = old.clone().at(Reach::Machine);
    assert_eq!(named.clone().placed(&folders, Reach::Workspace), named);
    let nowhere = Choice::new("nowhere", None).unwrap();
    assert_eq!(nowhere.clone().placed(&folders, Reach::Workspace), nowhere);
}

#[test]
fn an_in_plane_pick_copied_into_the_choices_names_its_folder_s_level() {
    let places = brands();
    std::fs::write(
        crate::names::manifest(&places.plane),
        format!("[project]\nid = \"{PROJECT}\"\n"),
    )
    .unwrap();
    std::fs::write(
        places.plane.join("workspaces/alpha/workspace.json"),
        r#"{"settings": {"theme": {"use": "brand"}}}"#,
    )
    .unwrap();
    levels::adopt(&places.config, &places.plane, Some("alpha")).unwrap();
    assert_eq!(
        choices::read(&places.config).projects[PROJECT].workspaces["alpha"].from,
        Some(Reach::Workspace)
    );
}

// Settings › Theme's gallery ------------------------------------------------------------------

fn shelf(folders: &Folders, reach: Reach) -> Shelf<'_> {
    Shelf {
        folders,
        reach,
        on: On::Every,
        offered: &[],
    }
}

#[test]
fn two_folders_of_one_name_are_two_cards_each_with_its_level() {
    let places = brands();
    let folders = places.folders();
    let cards = gallery::cards(&shelf(&folders, Reach::Workspace));
    let brand: Vec<(&From, &str)> = cards
        .iter()
        .filter(|card| card.id == "brand")
        .map(|card| (&card.from, card.name.as_str()))
        .collect();
    assert_eq!(
        brand,
        [
            (&From::Workspace, "Workspace's"),
            (&From::Project, "Project's"),
            (&From::Machine, "Machine's"),
        ]
    );
    // A project's gallery reaches no workspace's folder.
    let cards = gallery::cards(&shelf(&folders, Reach::Project));
    assert_eq!(cards.iter().filter(|card| card.id == "brand").count(), 2);
    // Each card is told apart by its level.
    let at = shelf(&folders, Reach::Workspace);
    for (level, name) in [(Reach::Project, "Project's"), (Reach::Machine, "Machine's")] {
        let read = gallery::theme_of(&at, "brand", Some(level)).unwrap();
        assert_eq!(read.theme.name, name);
        assert_eq!(
            gallery::folder_of(&at, "brand", Some(level)),
            Some(places.themes(level).join("brand"))
        );
    }
    // With no level, the nearest, as a choice reads it.
    assert_eq!(
        gallery::theme_of(&at, "brand", None).unwrap().theme.name,
        "Workspace's"
    );
    // A level nearer than the gallery reaches is no card of it.
    assert!(
        gallery::theme_of(
            &shelf(&folders, Reach::Project),
            "brand",
            Some(Reach::Workspace)
        )
        .is_none()
    );
}

#[test]
fn setting_an_option_keeps_the_value_and_names_the_folder_s_level() {
    let places = Places::new();
    put(
        &places.themes(Reach::Project),
        "accented",
        &accented("Accented"),
    );
    let folders = places.folders();
    let at = shelf(&folders, Reach::Workspace);
    let base = Choice::new("accented", None).unwrap();
    let (chosen, notes) = gallery::choose_option(&at, &base, "accent", Some("teal")).unwrap();
    assert_eq!(notes, Vec::<String>::new());
    assert_eq!(chosen.from, Some(Reach::Project));
    assert_eq!(chosen.options, values(&[("accent", "teal")]));
    // Back to the theme's default.
    let (back, _) = gallery::choose_option(&at, &chosen, "accent", None).unwrap();
    assert!(back.options.is_empty());
    // Drawn as kept.
    let levels = Levels {
        workspace_choice: Some(chosen),
        ..Levels::default()
    };
    assert_eq!(accent_of(&drawn(&levels, &folders)), TEAL);
}

#[test]
fn setting_an_option_drops_the_values_the_theme_no_longer_has_with_a_sentence() {
    let places = Places::new();
    put(
        &places.themes(Reach::Machine),
        "accented",
        &accented("Accented"),
    );
    let folders = places.folders();
    let at = shelf(&folders, Reach::Project);
    let base = choice("accented", &[("accent", "pink"), ("gone", "on")]).unwrap();
    let (chosen, notes) = gallery::choose_option(&at, &base, "loud", Some("on")).unwrap();
    assert_eq!(chosen.options, values(&[("loud", "on")]));
    assert_eq!(
        notes,
        [
            "The option accent of Accented has no value pink any more, so it was dropped and its \
             default is drawn.",
            "Accented has no option gone any more, so your value for it was dropped.",
        ]
    );
}

#[test]
fn an_option_or_a_value_the_theme_does_not_have_is_refused() {
    let places = Places::new();
    put(
        &places.themes(Reach::Machine),
        "accented",
        &accented("Accented"),
    );
    let folders = places.folders();
    let at = shelf(&folders, Reach::Project);
    let base = Choice::new("accented", None).unwrap();
    assert!(gallery::choose_option(&at, &base, "nope", Some("on")).is_err());
    assert!(gallery::choose_option(&at, &base, "accent", Some("pink")).is_err());
    assert!(
        gallery::choose_option(&at, &Choice::new("nowhere", None).unwrap(), "accent", None)
            .is_err()
    );
}

#[test]
fn choosing_the_same_theme_again_keeps_its_option_values_and_another_theme_starts_afresh() {
    let places = Places::new();
    let slot = Slot::Project(PROJECT);
    let valued = choice("accented", &[("accent", "teal")])
        .unwrap()
        .at(Reach::Machine);
    choices::choose(&places.config, slot, Some(valued.clone())).unwrap();
    // Another appearance of the same theme: the values stay.
    let night = Choice::new("accented", Some("dark"))
        .unwrap()
        .at(Reach::Machine);
    choices::choose(&places.config, slot, Some(night)).unwrap();
    let kept = choices::read(&places.config).projects[PROJECT]
        .project
        .clone()
        .unwrap();
    assert_eq!(kept.appearance.as_deref(), Some("dark"));
    assert_eq!(kept.options, valued.options);
    // Another theme: its defaults.
    choices::choose(&places.config, slot, choice("brand", &[])).unwrap();
    let kept = choices::read(&places.config).projects[PROJECT]
        .project
        .clone()
        .unwrap();
    assert_eq!(kept.theme, "brand");
    assert!(kept.options.is_empty());
    // The same name from another level's folder is another theme.
    choices::choose(
        &places.config,
        slot,
        Some(valued.clone().at(Reach::Project)),
    )
    .unwrap();
    choices::choose(
        &places.config,
        slot,
        Choice::new("accented", None).map(|one| one.at(Reach::Machine)),
    )
    .unwrap();
    let kept = choices::read(&places.config).projects[PROJECT]
        .project
        .clone()
        .unwrap();
    assert!(kept.options.is_empty());
    // Taken away.
    choices::choose(&places.config, slot, None).unwrap();
    assert_eq!(
        choices::read(&places.config)
            .projects
            .get(PROJECT)
            .and_then(|one| one.project.clone()),
        None
    );
}
