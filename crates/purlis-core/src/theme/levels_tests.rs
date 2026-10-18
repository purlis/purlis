// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The theme levels at the engine's interface (#2323): theme folders at three levels, the
//! person's choices and the committed defaults, and the six-level precedence.

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use super::choices::{self, Choices, ProjectChoices, Slot};
use super::levels::{Project, adopt, move_into_a_folder, project_key};
use super::*;
use crate::extension::project::theme::Said;

const PROJECT: &str = "01JABCDEFGHJKMNPQRSTVWXYZ0";

fn no_survey() -> Arc<[Offered]> {
    Arc::from(Vec::new())
}

/// A one-appearance dark theme called `name`, whose base surface is `surface`.
fn named(name: &str, surface: &str) -> Value {
    json!({
        "name": name,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": { "surface.base": surface } }]
    })
}

/// A theme with a dark and a light appearance.
fn two(name: &str) -> Value {
    json!({
        "name": name,
        "appearances": [
            { "id": "night", "appearance": "dark", "tokens": { "surface.base": "#000001" } },
            { "id": "day", "appearance": "light", "tokens": { "surface.base": "#fffffe" } }
        ]
    })
}

fn put(themes: &Path, folder: &str, document: &Value) {
    let dir = themes.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(THEME_JSON), document.to_string()).unwrap();
}

/// A machine config home, a plane and its workspace `alpha`, with their `themes/` folders.
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

    fn folders(&self, workspace: Option<&str>) -> Folders {
        Folders::of(Some(&self.config), Some(&self.plane), workspace)
    }

    fn machine_themes(&self) -> std::path::PathBuf {
        crate::machine::dir(&self.config).join(levels::THEMES)
    }

    fn project_themes(&self) -> std::path::PathBuf {
        self.plane.join(levels::THEMES)
    }

    fn workspace_themes(&self) -> std::path::PathBuf {
        self.plane.join("workspaces/alpha").join(levels::THEMES)
    }
}

fn pick(theme: &str) -> Option<Choice> {
    Choice::new(theme, None)
}

fn drawn(levels: &Levels, folders: &Folders, theirs: Option<&Theme>) -> Drawn {
    resolve_levels(
        &LevelInputs {
            levels,
            folders,
            theirs,
            on: On::Every,
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        no_survey,
    )
}

fn name_of(drawn: &Drawn) -> &str {
    &drawn.theme.as_ref().expect("a theme is drawn").name
}

#[test]
fn the_six_levels_are_taken_nearest_first() {
    let places = Places::new();
    for (folder, theme) in ["one", "two", "three", "four", "five"].iter().zip(1..) {
        // The machine's choice reaches the person's own folders alone.
        let level = if *folder == "five" {
            places.machine_themes()
        } else {
            places.project_themes()
        };
        put(&level, folder, &named(&format!("Level {theme}"), "#101010"));
    }
    let folders = places.folders(Some("alpha"));
    let theirs = load(&named("Mine", "#202020")).theme;
    let mut levels = Levels {
        workspace_choice: pick("one"),
        workspace_default: pick("two"),
        project_choice: pick("three"),
        project_default: pick("four"),
        machine_default: pick("five"),
    };
    let mut seen = Vec::new();
    let take = [
        |l: &mut Levels| l.workspace_choice = None,
        |l: &mut Levels| l.workspace_default = None,
        |l: &mut Levels| l.project_choice = None,
        |l: &mut Levels| l.project_default = None,
        |l: &mut Levels| l.machine_default = None,
    ];
    for next in take {
        let got = drawn(&levels, &folders, Some(&theirs));
        seen.push((got.level, name_of(&got).to_owned()));
        next(&mut levels);
    }
    // With no machine default chosen, level 5 is the person's own theme.json.
    let got = drawn(&levels, &folders, Some(&theirs));
    seen.push((got.level, name_of(&got).to_owned()));
    // And with nothing at all, Purlis.
    let got = drawn(&levels, &folders, None);
    seen.push((got.level, name_of(&got).to_owned()));
    assert_eq!(
        seen,
        [
            (Some(Level::WorkspaceChoice), "Level 1".to_owned()),
            (Some(Level::WorkspaceDefault), "Level 2".to_owned()),
            (Some(Level::ProjectChoice), "Level 3".to_owned()),
            (Some(Level::ProjectDefault), "Level 4".to_owned()),
            (Some(Level::MachineDefault), "Level 5".to_owned()),
            (Some(Level::MachineDefault), "Mine".to_owned()),
            (Some(Level::Purlis), default_theme().name.clone()),
        ]
    );
}

#[test]
fn at_every_level_the_person_s_choice_beats_the_committed_default() {
    let shared = "[theme]\nuse = \"purlis-light\"\n";
    let manifest = r#"{"settings": {"theme": {"use": "purlis-light"}}}"#;
    let said = Said::from_text(Some(shared), None).in_workspace("alpha", Some(manifest));
    let held = Choices {
        projects: [(
            PROJECT.to_owned(),
            ProjectChoices {
                project: pick("purlis-dark"),
                workspaces: [("alpha".to_owned(), pick("purlis-dark").unwrap())].into(),
                ..ProjectChoices::default()
            },
        )]
        .into(),
        ..Choices::default()
    };
    let levels = Levels::of(
        &held,
        Some(Project {
            key: Some(PROJECT),
            said: &said,
            workspace: Some(("alpha", true)),
        }),
    );
    assert_eq!(levels.workspace_choice, pick("purlis-dark"));
    assert_eq!(levels.workspace_default, pick("purlis-light"));
    assert_eq!(levels.project_choice, pick("purlis-dark"));
    assert_eq!(levels.project_default, pick("purlis-light"));
    assert_eq!(
        levels.chosen().map(|(level, _)| level),
        Some(Level::WorkspaceChoice)
    );
}

#[test]
fn the_plane_root_resolves_from_level_three_down() {
    let said = Said::from_text(Some("[theme]\nuse = \"purlis-dark\"\n"), None);
    let held = Choices {
        projects: [(
            PROJECT.to_owned(),
            ProjectChoices {
                workspaces: [("alpha".to_owned(), pick("purlis-light").unwrap())].into(),
                ..ProjectChoices::default()
            },
        )]
        .into(),
        ..Choices::default()
    };
    // The root is on no workspace: neither the workspace's choice nor its default is read.
    let at_root = Levels::of(
        &held,
        Some(Project {
            key: Some(PROJECT),
            said: &said,
            workspace: None,
        }),
    );
    assert_eq!(at_root.workspace_choice, None);
    assert_eq!(at_root.workspace_default, None);
    assert_eq!(
        at_root.chosen(),
        Some((Level::ProjectDefault, &pick("purlis-dark").unwrap()))
    );
}

#[test]
fn a_live_workspace_commits_a_default_and_a_local_one_s_pick_stays_the_person_s() {
    let manifest = r#"{"settings": {"theme": {"use": "brand", "appearance": "system"}}}"#;
    let said = Said::from_text(None, None).in_workspace("alpha", Some(manifest));
    let brand = Choice::new("brand", Some("system"));
    let of = |live| {
        Levels::of(
            &Choices::default(),
            Some(Project {
                key: Some(PROJECT),
                said: &said,
                workspace: Some(("alpha", live)),
            }),
        )
    };
    let live = of(true);
    assert_eq!(
        (live.workspace_default, live.workspace_choice),
        (brand.clone(), None)
    );
    let local = of(false);
    assert_eq!(
        (local.workspace_default, local.workspace_choice),
        (None, brand)
    );
}

#[test]
fn the_project_s_committed_default_is_its_manifest_s_theme_table() {
    let said = Said::from_text(
        Some("[theme]\nuse = \"dusk\"\nappearance = \"day\"\n"),
        None,
    );
    let levels = Levels::of(
        &Choices::default(),
        Some(Project {
            key: Some(PROJECT),
            said: &said,
            workspace: None,
        }),
    );
    assert_eq!(levels.project_default, Choice::new("dusk", Some("day")));
}

/// Three folders called `brand`, one at each level.
fn brands() -> Places {
    let places = Places::new();
    put(
        &places.machine_themes(),
        "brand",
        &named("Machine's", "#111111"),
    );
    put(
        &places.project_themes(),
        "brand",
        &named("Project's", "#222222"),
    );
    put(
        &places.workspace_themes(),
        "brand",
        &named("Workspace's", "#333333"),
    );
    places
}

#[test]
fn a_workspace_s_choice_draws_the_nearest_level_s_folder_of_its_name() {
    let places = brands();
    for levels in [
        Levels {
            workspace_choice: pick("brand"),
            ..Levels::default()
        },
        Levels {
            workspace_default: pick("brand"),
            ..Levels::default()
        },
    ] {
        assert_eq!(
            name_of(&drawn(&levels, &places.folders(Some("alpha")), None)),
            "Workspace's"
        );
    }
}

#[test]
fn a_project_s_choice_and_default_never_reach_a_workspace_s_folder() {
    let places = brands();
    for levels in [
        Levels {
            project_choice: pick("brand"),
            ..Levels::default()
        },
        Levels {
            project_default: pick("brand"),
            ..Levels::default()
        },
    ] {
        // In a workspace that holds its own `brand`, the project's choice is still the project's.
        assert_eq!(
            name_of(&drawn(&levels, &places.folders(Some("alpha")), None)),
            "Project's"
        );
    }
}

#[test]
fn the_machine_s_choice_reaches_the_person_s_own_folders_alone() {
    let places = brands();
    let levels = Levels {
        machine_default: pick("brand"),
        ..Levels::default()
    };
    assert_eq!(
        name_of(&drawn(&levels, &places.folders(Some("alpha")), None)),
        "Machine's"
    );
    // A name only a nearer level has is not the machine's to draw: it is passed over.
    put(
        &places.project_themes(),
        "projects-only",
        &named("Project's only", "#444444"),
    );
    let levels = Levels {
        machine_default: pick("projects-only"),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders(Some("alpha")), None);
    assert_eq!(got.theme.as_ref(), Some(default_theme()));
    assert_eq!(got.level, Some(Level::Purlis));
}

#[test]
fn a_folder_extends_only_its_own_level_and_farther_out() {
    let places = Places::new();
    put(&places.workspace_themes(), "base", &two("Workspace's base"));
    put(
        &places.project_themes(),
        "child",
        &json!({ "name": "Child", "extends": "base" }),
    );
    let levels = Levels {
        project_default: pick("child"),
        ..Levels::default()
    };
    // The project's folder cannot take the workspace's `base` for its parent.
    let got = drawn(&levels, &places.folders(Some("alpha")), None);
    assert_eq!(got.theme.as_ref(), Some(default_theme()));
    assert!(
        got.complaints.iter().any(|one| one
            .said
            .contains("extends \"base\", which is not a theme purlis has")),
        "{:?}",
        got.complaints
    );
    // A machine `base` it may take.
    put(&places.machine_themes(), "base", &two("Machine's base"));
    let got = drawn(&levels, &places.folders(Some("alpha")), None);
    assert_eq!(got.theme.unwrap().value("surface.base"), "#000001");
}

#[test]
fn a_theme_may_extend_a_folder_of_a_farther_level() {
    let places = Places::new();
    put(&places.machine_themes(), "base", &two("Base"));
    put(
        &places.workspace_themes(),
        "child",
        &json!({ "name": "Child", "extends": "base" }),
    );
    let levels = Levels {
        workspace_choice: Choice::new("child", Some("day")),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders(Some("alpha")), None);
    let theme = got.theme.unwrap();
    assert_eq!((theme.name.as_str(), theme.id.as_str()), ("Child", "day"));
    assert_eq!(theme.value("surface.base"), "#fffffe");
}

#[test]
fn a_choice_names_an_appearance_by_its_id_or_follows_the_system() {
    let places = Places::new();
    put(&places.project_themes(), "dusk", &two("Dusk"));
    let folders = places.folders(None);
    let with = |appearance: Option<&str>, system| {
        resolve_levels(
            &LevelInputs {
                levels: &Levels {
                    project_default: Choice::new("dusk", appearance),
                    ..Levels::default()
                },
                folders: &folders,
                theirs: None,
                on: On::Every,
                system,
                contrast: Contrast::Standard,
            },
            no_survey,
        )
    };
    let id = |drawn: Drawn| drawn.theme.unwrap().id;
    assert_eq!(id(with(None, Appearance::Light)), "night");
    assert_eq!(id(with(Some("day"), Appearance::Dark)), "day");
    let following = with(Some("system"), Appearance::Light);
    assert!(following.follows_system);
    assert_eq!(id(following), "day");
    let missing = with(Some("dawn"), Appearance::Dark);
    assert_eq!(
        missing.complaints[0].said,
        "Dusk has no appearance called dawn, so its first is drawn"
    );
    assert_eq!(id(missing), "night");
}

#[test]
fn purlis_classic_chosen_in_an_appearance_draws_that_appearance() {
    let folders = Folders::default();
    let levels = Levels {
        machine_default: Choice::new("purlis-classic", Some("light")),
        ..Levels::default()
    };
    let got = drawn(&levels, &folders, None);
    assert_eq!(got.theme.as_ref(), built_in("purlis-light"));
    // What the window is told, so it draws it in the same frame.
    assert_eq!(levels.machine_default.unwrap().told(), "purlis-light");
    assert_eq!(pick("dusk").unwrap().told(), "dusk");
    assert_eq!(
        Choice::new("purlis-classic", Some("system"))
            .unwrap()
            .told(),
        "system"
    );
}

#[test]
fn a_choice_naming_no_folder_within_reach_falls_to_the_next_level_with_a_sentence() {
    let places = Places::new();
    put(&places.project_themes(), "fine", &named("Fine", "#123456"));
    let levels = Levels {
        workspace_choice: pick("gone"),
        project_default: pick("fine"),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders(Some("alpha")), None);
    assert_eq!(got.level, Some(Level::ProjectDefault));
    assert_eq!(name_of(&got), "Fine");
    assert_eq!(
        got.complaints[0].said,
        "gone names no theme folder or theme purlis has, so it is passed over and the next \
         level's choice is used"
    );
}

#[test]
fn a_folder_that_is_there_and_refused_draws_purlis_and_says_why() {
    let places = Places::new();
    put(&places.project_themes(), "fine", &named("Fine", "#123456"));
    put(
        &places.workspace_themes(),
        "broken",
        &json!({ "name": "Broken", "appearances": "no" }),
    );
    let levels = Levels {
        workspace_choice: pick("broken"),
        project_default: pick("fine"),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders(Some("alpha")), None);
    assert_eq!(got.level, Some(Level::WorkspaceChoice));
    assert_eq!(got.theme.as_ref(), Some(default_theme()));
    assert!(!got.complaints.is_empty());
}

#[test]
fn an_extension_theme_chosen_while_its_extension_is_off_draws_purlis_and_says_why() {
    let levels = Levels {
        project_choice: pick("solarized/Solarized Dark"),
        ..Levels::default()
    };
    let only: Vec<String> = Vec::new();
    let got = resolve_levels(
        &LevelInputs {
            levels: &levels,
            folders: &Folders::default(),
            theirs: None,
            on: On::Only(&only),
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        || {
            Arc::from(vec![Offered::of(
                "solarized".into(),
                "Solarized Dark".into(),
                &named("Solarized Dark", "#002b36").to_string(),
            )])
        },
    );
    assert_eq!(got.theme.as_ref(), Some(default_theme()));
    assert_eq!(
        got.complaints[0].said,
        "solarized/Solarized Dark is not a theme in force here, so Purlis is drawn"
    );
}

#[cfg(unix)]
#[test]
fn a_themes_folder_reached_through_a_link_is_not_read() {
    let places = Places::new();
    let elsewhere = places.plane.join("elsewhere");
    put(&elsewhere, "brand", &named("Planted", "#999999"));
    std::os::unix::fs::symlink(&elsewhere, places.project_themes()).unwrap();
    let levels = Levels {
        project_default: pick("brand"),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders(None), None);
    assert_eq!(got.theme.as_ref(), Some(default_theme()));
    assert!(
        got.complaints[0].said.contains("link"),
        "{:?}",
        got.complaints
    );
}

#[cfg(unix)]
#[test]
fn a_workspace_reached_through_a_link_has_no_themes_read() {
    let places = Places::new();
    let elsewhere = places.plane.join("elsewhere");
    put(
        &elsewhere.join(levels::THEMES),
        "brand",
        &named("Planted", "#999999"),
    );
    std::os::unix::fs::symlink(&elsewhere, places.plane.join("workspaces/beta")).unwrap();
    let levels = Levels {
        workspace_choice: pick("brand"),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders(Some("beta")), None);
    assert_eq!(got.theme.as_ref(), Some(default_theme()));
}

#[cfg(unix)]
#[test]
fn a_machine_themes_folder_that_is_a_link_is_not_read() {
    let places = Places::new();
    let elsewhere = places.plane.join("elsewhere");
    put(&elsewhere, "brand", &named("Planted", "#999999"));
    std::fs::create_dir_all(crate::machine::dir(&places.config)).unwrap();
    std::os::unix::fs::symlink(&elsewhere, places.machine_themes()).unwrap();
    let levels = Levels {
        machine_default: pick("brand"),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders(None), None);
    assert_eq!(got.theme.as_ref(), Some(default_theme()));
    assert!(
        got.complaints.iter().any(|one| one.said.contains("link")),
        "{:?}",
        got.complaints
    );
}

#[cfg(unix)]
#[test]
fn a_workspaces_folder_that_is_a_link_has_no_themes_read() {
    let home = tempfile::tempdir().unwrap();
    let plane = home.path().join("plane");
    let elsewhere = home.path().join("elsewhere");
    put(
        &elsewhere.join("alpha").join(levels::THEMES),
        "brand",
        &named("Planted", "#999999"),
    );
    std::fs::create_dir_all(&plane).unwrap();
    std::os::unix::fs::symlink(&elsewhere, plane.join("workspaces")).unwrap();
    let levels = Levels {
        workspace_choice: pick("brand"),
        ..Levels::default()
    };
    let got = drawn(
        &levels,
        &Folders::of(None, Some(&plane), Some("alpha")),
        None,
    );
    assert_eq!(got.theme.as_ref(), Some(default_theme()));
}

#[cfg(unix)]
#[test]
fn a_theme_folder_or_its_theme_json_that_is_a_link_is_not_read() {
    let places = Places::new();
    let elsewhere = places.plane.join("elsewhere");
    put(&elsewhere, "brand", &named("Planted", "#999999"));
    std::fs::create_dir_all(places.project_themes().join("own")).unwrap();
    std::os::unix::fs::symlink(
        elsewhere.join("brand"),
        places.project_themes().join("brand"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        elsewhere.join("brand").join(THEME_JSON),
        places.project_themes().join("own").join(THEME_JSON),
    )
    .unwrap();
    for name in ["brand", "own"] {
        let levels = Levels {
            project_default: pick(name),
            ..Levels::default()
        };
        let got = drawn(&levels, &places.folders(None), None);
        assert_eq!(got.theme.as_ref(), Some(default_theme()), "{name}");
        assert!(
            got.complaints.iter().any(|one| one.said.contains("link")),
            "{name}: {:?}",
            got.complaints
        );
    }
}

#[test]
fn a_file_in_a_themes_folder_is_not_a_theme() {
    let places = Places::new();
    std::fs::create_dir_all(places.project_themes()).unwrap();
    std::fs::write(places.project_themes().join("loose"), "{}").unwrap();
    let levels = Levels {
        project_default: pick("loose"),
        ..Levels::default()
    };
    let got = drawn(&levels, &places.folders(None), None);
    assert_eq!(got.theme.as_ref(), Some(default_theme()));
}

// The person's choices, kept on this machine --------------------------------------------------

#[test]
fn the_person_s_choices_are_kept_in_machine_config_by_project_id() {
    let places = Places::new();
    choices::set(&places.config, Slot::Machine, pick("dusk")).unwrap();
    choices::set(&places.config, Slot::Project(PROJECT), pick("purlis-light")).unwrap();
    choices::set(
        &places.config,
        Slot::Workspace(PROJECT, "alpha"),
        Choice::new("brand", Some("system")),
    )
    .unwrap();
    let held = choices::read(&places.config);
    assert_eq!(held.machine, pick("dusk"));
    let mine = &held.projects[PROJECT];
    assert_eq!(mine.project, pick("purlis-light"));
    assert_eq!(
        mine.workspaces["alpha"],
        Choice::new("brand", Some("system")).unwrap()
    );
    // Never in the plane: the file is in purlis's config folder.
    assert!(
        crate::machine::dir(&places.config)
            .join(choices::FILE)
            .is_file()
    );
    // Taken away.
    choices::set(&places.config, Slot::Workspace(PROJECT, "alpha"), None).unwrap();
    assert!(
        choices::read(&places.config).projects[PROJECT]
            .workspaces
            .is_empty()
    );
}

#[test]
fn the_person_s_choices_survive_a_re_clone() {
    let places = Places::new();
    let manifest = format!("[project]\nid = \"{PROJECT}\"\n");
    let first = places.plane.join("first");
    let again = places.plane.join("again");
    for clone in [&first, &again] {
        std::fs::create_dir_all(clone).unwrap();
        std::fs::write(crate::names::manifest(clone), &manifest).unwrap();
    }
    let key = project_key(&first).unwrap();
    assert_eq!(key, PROJECT);
    choices::set(&places.config, Slot::Project(&key), pick("purlis-light")).unwrap();
    // A second clone of the same project, elsewhere on this machine, finds the choice.
    let levels = Levels::read(Some(&places.config), Some(&again), None);
    assert_eq!(levels.project_choice, pick("purlis-light"));
}

#[test]
fn a_choice_purlis_cannot_read_is_left_out() {
    let places = Places::new();
    let dir = crate::machine::private_dir(&places.config).unwrap();
    std::fs::write(
        dir.join(choices::FILE),
        json!({
            "version": 1,
            "machine": { "theme": "Not A Theme" },
            "projects": {
                "../../etc": { "theme": "dusk" },
                PROJECT: {
                    "theme": "dusk",
                    "appearance": "Day Light",
                    "workspaces": { "../x": { "theme": "dusk" }, "alpha": { "theme": "dusk" } }
                }
            }
        })
        .to_string(),
    )
    .unwrap();
    let held = choices::read(&places.config);
    assert_eq!(held.machine, None);
    assert_eq!(held.projects.keys().collect::<Vec<_>>(), [PROJECT]);
    let mine = &held.projects[PROJECT];
    assert_eq!(mine.project, None);
    assert_eq!(mine.workspaces.keys().collect::<Vec<_>>(), ["alpha"]);
}

#[test]
fn choices_purlis_cannot_read_are_never_written_over() {
    let places = Places::new();
    let dir = crate::machine::private_dir(&places.config).unwrap();
    std::fs::write(dir.join(choices::FILE), "not json").unwrap();
    assert!(choices::set(&places.config, Slot::Machine, pick("dusk")).is_err());
    assert_eq!(
        std::fs::read_to_string(dir.join(choices::FILE)).unwrap(),
        "not json"
    );
}

// Today's per-workspace pick, mapped once ------------------------------------------------------

#[test]
fn a_local_workspace_s_pick_is_mapped_once_with_a_sentence() {
    let places = Places::new();
    std::fs::write(
        crate::names::manifest(&places.plane),
        format!("[project]\nid = \"{PROJECT}\"\n"),
    )
    .unwrap();
    std::fs::write(
        places.plane.join("workspaces/alpha/workspace.json"),
        r#"{"settings": {"theme": {"use": "purlis-light"}}}"#,
    )
    .unwrap();
    let said = adopt(&places.config, &places.plane, Some("alpha")).unwrap();
    assert_eq!(
        said,
        [
            "The theme purlis-light picked in workspaces/alpha/workspace.json is now your choice \
          for the workspace alpha on this machine."
        ]
    );
    assert_eq!(
        choices::read(&places.config).projects[PROJECT].workspaces["alpha"],
        pick("purlis-light").unwrap()
    );
    // Once: the second time there is nothing left to map.
    assert!(
        adopt(&places.config, &places.plane, Some("alpha"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_live_workspace_s_pick_is_its_committed_default_and_is_not_copied() {
    let places = Places::new();
    std::fs::write(
        crate::names::manifest(&places.plane),
        format!("[project]\nid = \"{PROJECT}\"\n"),
    )
    .unwrap();
    std::fs::write(
        places.plane.join("workspaces/alpha/workspace.json"),
        r#"{"settings": {"theme": {"use": "purlis-light"}}}"#,
    )
    .unwrap();
    std::fs::write(
        places.plane.join(".gitignore"),
        format!(
            "{}\n!/workspaces/alpha/workspace.json\n{}\n",
            crate::names::LIVE_BEGIN.write,
            crate::names::LIVE_END.write
        ),
    )
    .unwrap();
    assert!(
        adopt(&places.config, &places.plane, Some("alpha"))
            .unwrap()
            .is_empty()
    );
    let levels = Levels::read(Some(&places.config), Some(&places.plane), Some("alpha"));
    assert_eq!(levels.workspace_default, pick("purlis-light"));
    assert_eq!(levels.workspace_choice, None);
}

// A format 1 theme.json, offered a move into a theme folder ------------------------------------

#[test]
fn a_format_1_theme_file_moves_into_a_theme_folder_and_draws_the_same() {
    let places = Places::new();
    let dir = crate::machine::private_dir(&places.config).unwrap();
    let file = json!({ "name": "My Dusk!", "tokens": { "surface.base": "#0a0b0c" } });
    std::fs::write(dir.join(crate::windowprefs::THEME), file.to_string()).unwrap();
    let before = read_theirs(Some(&places.config));
    assert!(before.format_1);
    let drew = before.theme.unwrap().first().clone();

    assert_eq!(
        move_into_a_folder(&places.config).unwrap(),
        levels::Moved {
            name: "my-dusk".to_owned(),
            chosen: true
        }
    );
    assert!(!dir.join(crate::windowprefs::THEME).exists());
    assert!(places.machine_themes().join("my-dusk/theme.json").is_file());

    let levels = Levels::read(Some(&places.config), None, None);
    let got = drawn(
        &levels,
        &Folders::of(Some(&places.config), None, None),
        None,
    );
    assert_eq!(got.level, Some(Level::MachineDefault));
    assert_eq!(got.theme.unwrap().values, drew.values);
    // There is nothing left to move.
    assert!(move_into_a_folder(&places.config).is_err());
}

#[test]
fn a_format_2_theme_file_is_not_offered_the_move() {
    let places = Places::new();
    let dir = crate::machine::private_dir(&places.config).unwrap();
    std::fs::write(dir.join(crate::windowprefs::THEME), two("Both").to_string()).unwrap();
    assert!(!read_theirs(Some(&places.config)).format_1);
    assert!(move_into_a_folder(&places.config).is_err());
    assert!(dir.join(crate::windowprefs::THEME).is_file());
}

#[test]
fn the_move_keeps_a_machine_default_already_chosen() {
    let places = Places::new();
    let dir = crate::machine::private_dir(&places.config).unwrap();
    std::fs::write(
        dir.join(crate::windowprefs::THEME),
        json!({ "name": "Old" }).to_string(),
    )
    .unwrap();
    choices::set(&places.config, Slot::Machine, pick("purlis-light")).unwrap();
    let moved = move_into_a_folder(&places.config).unwrap();
    assert!(!moved.chosen);
    assert_eq!(choices::read(&places.config).machine, pick("purlis-light"));
    assert!(places.machine_themes().join("old/theme.json").is_file());
}

#[test]
fn the_move_is_undone_when_choosing_it_fails() {
    let places = Places::new();
    let dir = crate::machine::private_dir(&places.config).unwrap();
    let file = json!({ "name": "Old" }).to_string();
    std::fs::write(dir.join(crate::windowprefs::THEME), &file).unwrap();
    // Choices purlis cannot read are never written over, so choosing fails.
    std::fs::write(dir.join(choices::FILE), "not json").unwrap();
    let why = move_into_a_folder(&places.config).unwrap_err();
    assert!(why.contains("moved it back"), "{why}");
    assert_eq!(
        std::fs::read_to_string(dir.join(crate::windowprefs::THEME)).unwrap(),
        file
    );
    assert!(!places.machine_themes().join("old").exists());
}

#[test]
fn a_move_that_cannot_happen_leaves_no_folder_behind() {
    let places = Places::new();
    let dir = crate::machine::private_dir(&places.config).unwrap();
    std::fs::write(
        dir.join(crate::windowprefs::THEME),
        json!({ "name": "Old" }).to_string(),
    )
    .unwrap();
    // Someone else's theme.json is already where the folder would be made next: the first
    // name is taken, so `old-2` is made and used; with `old` and every later name a file, the
    // move cannot happen.
    std::fs::create_dir_all(places.machine_themes()).unwrap();
    for n in 1..=99 {
        let name = if n == 1 {
            "old".to_owned()
        } else {
            format!("old-{n}")
        };
        std::fs::write(places.machine_themes().join(name), "taken").unwrap();
    }
    assert!(move_into_a_folder(&places.config).is_err());
    assert!(dir.join(crate::windowprefs::THEME).is_file());
}

// The cap, least recently used out first ---------------------------------------------------------

#[test]
fn at_the_cap_the_projects_changed_longest_ago_go_first() {
    let places = Places::new();
    let dir = crate::machine::private_dir(&places.config).unwrap();
    // Projects with a choice each, the first changed longest ago, until the file is within
    // 2 KiB of the cap.
    let mut ids: Vec<String> = Vec::new();
    let mut projects = serde_json::Map::new();
    let mut text = String::new();
    for n in 0..10_000u64 {
        let id = ulid::Ulid::from_parts(n + 1, u128::from(n)).to_string();
        projects.insert(
            id.clone(),
            json!({ "theme": "purlis-classic", "appearance": "light", "used": n + 1 }),
        );
        ids.push(id);
        text =
            serde_json::to_string_pretty(&json!({ "version": 1, "projects": projects })).unwrap();
        if text.len() > 62 * 1024 {
            break;
        }
    }
    assert!(
        text.len() < 64 * 1024,
        "the fixture is under the cap: {}",
        text.len()
    );
    std::fs::write(dir.join(choices::FILE), text).unwrap();
    // A project with many workspaces chosen takes the file past the cap.
    let newest = ulid::Ulid::from_parts(9_999, 9_999).to_string();
    for n in 0..60 {
        choices::set(
            &places.config,
            Slot::Workspace(&newest, &format!("workspace-{n}")),
            pick("purlis-dark"),
        )
        .unwrap();
    }
    let held = choices::read(&places.config);
    assert_eq!(held.projects[&newest].workspaces.len(), 60);
    assert!(!held.projects.contains_key(&ids[0]), "the oldest stayed");
    assert!(
        held.projects.contains_key(ids.last().unwrap()),
        "the newest went"
    );
}

// The LIVE workspace's default over the person's project choice, said once -----------------------

#[test]
fn a_live_workspace_s_default_now_over_the_person_s_local_pick_is_said_once() {
    let places = Places::new();
    std::fs::write(
        crate::names::manifest(&places.plane),
        format!("[project]\nid = \"{PROJECT}\"\n"),
    )
    .unwrap();
    std::fs::write(
        places.plane.join("workspaces/alpha/workspace.json"),
        r#"{"settings": {"theme": {"use": "purlis-light"}}}"#,
    )
    .unwrap();
    std::fs::write(
        places.plane.join(".gitignore"),
        format!(
            "{}\n!/workspaces/alpha/workspace.json\n{}\n",
            crate::names::LIVE_BEGIN.write,
            crate::names::LIVE_END.write
        ),
    )
    .unwrap();
    choices::set(&places.config, Slot::Project(PROJECT), pick("purlis-dark")).unwrap();
    let said = adopt(&places.config, &places.plane, Some("alpha")).unwrap();
    assert_eq!(
        said,
        [
            "The workspace alpha is LIVE and commits the theme purlis-light, which now comes before \
          your choice for the project (purlis-dark). Choose your theme for the workspace to draw \
          yours there."
        ]
    );
    assert!(
        adopt(&places.config, &places.plane, Some("alpha"))
            .unwrap()
            .is_empty()
    );
}

// Settings' pick after the mapping ------------------------------------------------------------

#[test]
fn a_settings_pick_takes_effect_after_the_mapping() {
    let places = Places::new();
    std::fs::write(
        crate::names::manifest(&places.plane),
        format!("[project]\nid = \"{PROJECT}\"\n"),
    )
    .unwrap();
    std::fs::write(
        places.plane.join("workspaces/alpha/workspace.json"),
        r#"{"settings": {"theme": {"use": "purlis-light"}}}"#,
    )
    .unwrap();
    assert!(
        !adopt(&places.config, &places.plane, Some("alpha"))
            .unwrap()
            .is_empty()
    );
    // What Settings › Appearance › Your theme writes.
    choices::set(
        &places.config,
        Slot::Workspace(PROJECT, "alpha"),
        Choice::new("purlis-dark", Some("system")),
    )
    .unwrap();
    let levels = Levels::read(Some(&places.config), Some(&places.plane), Some("alpha"));
    assert_eq!(
        levels.chosen(),
        Some((
            Level::WorkspaceChoice,
            &Choice::new("purlis-dark", Some("system")).unwrap()
        ))
    );
}

// Every level's theme is drawn with the consent floors held (#2478) ---------------------------

#[test]
fn every_level_s_theme_is_drawn_with_the_consent_floors_held() {
    let places = Places::new();
    let breaks = json!({
        "name": "Breaks",
        "appearances": [{
            "id": "dark",
            "appearance": "dark",
            "tokens": { "danger.text": "#301010", "danger.surface": "#301010" }
        }]
    });
    put(&places.machine_themes(), "breaks", &breaks);
    let theirs = load(&breaks).theme;
    for levels in [
        Levels {
            workspace_choice: pick("breaks"),
            ..Levels::default()
        },
        Levels {
            workspace_default: pick("breaks"),
            ..Levels::default()
        },
        Levels {
            project_choice: pick("breaks"),
            ..Levels::default()
        },
        Levels {
            project_default: pick("breaks"),
            ..Levels::default()
        },
        Levels {
            machine_default: pick("breaks"),
            ..Levels::default()
        },
    ] {
        let got = drawn(&levels, &places.folders(Some("alpha")), None);
        assert!(got.held.is_some(), "{:?}", got.level);
    }
    // Level 5 with no machine default: the person's own theme.json.
    let got = drawn(&Levels::default(), &places.folders(None), Some(&theirs));
    assert!(got.held.is_some());
}
