// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A committed default theme's option values (#2707), at the engine's interface: the project's
//! `[theme] options` and a LIVE workspace's `settings.theme.options` are drawn as the default
//! is, the person's choice beating them and the nearest level first, with the consent floors
//! held over every value.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use super::levels::Project;
use super::*;
use crate::extension::project::theme::{Said, refusals, refusals_in_workspace};

const PROJECT: &str = "01JABCDEFGHJKMNPQRSTVWXYZ0";

const LAVENDER: &str = "#b4a7f5";
const TEAL: &str = "#2bb3a3";

fn no_survey() -> Arc<[Offered]> {
    Arc::from(Vec::new())
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

/// A config home and a plane whose project `themes/` holds `accented`.
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
        let folder = plane.join(levels::THEMES).join("accented");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(THEME_JSON), accented("Accented").to_string()).unwrap();
        Self {
            _home: home,
            config,
            plane,
        }
    }

    fn folders(&self) -> Folders {
        Folders::of(Some(&self.config), Some(&self.plane), Some("alpha"))
    }
}

/// The levels of a project whose `charter.toml` is `shared`, in the LIVE workspace `alpha`
/// whose `workspace.json` is `manifest`, with the person's `held` choices.
fn levels_of(held: &choices::Choices, shared: &str, manifest: Option<&str>) -> Levels {
    let mut said = Said::from_text(Some(shared), None);
    if let Some(manifest) = manifest {
        said = said.in_workspace("alpha", Some(manifest));
    }
    Levels::of(
        held,
        Some(Project {
            key: Some(PROJECT),
            said: &said,
            workspace: manifest.map(|_| ("alpha", true)),
        }),
    )
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

fn accent_of(drawn: &Drawn) -> &str {
    drawn
        .theme
        .as_ref()
        .expect("a theme is drawn")
        .value("accent.base")
}

fn values(pairs: &[(&str, &str)]) -> Values {
    pairs
        .iter()
        .map(|(id, value)| ((*id).to_owned(), (*value).to_owned()))
        .collect()
}

/// The person's choice of `theme` for the project, with `chosen` values.
fn yours_for_the_project(theme: &str, chosen: &[(&str, &str)]) -> choices::Choices {
    let mut held = choices::Choices::default();
    held.projects.insert(
        PROJECT.to_owned(),
        choices::ProjectChoices {
            project: Choice::new(theme, None).map(|one| one.with_options(values(chosen))),
            ..choices::ProjectChoices::default()
        },
    );
    held
}

const PROJECT_TEAL: &str = "[theme]\nuse = \"accented\"\noptions = { accent = \"teal\" }\n";

#[test]
fn a_project_s_committed_default_draws_its_option_values_where_nothing_is_chosen() {
    let places = Places::new();
    let levels = levels_of(&choices::Choices::default(), PROJECT_TEAL, None);
    assert_eq!(
        levels.project_default.as_ref().map(|one| &one.options),
        Some(&values(&[("accent", "teal")]))
    );

    let got = drawn(&levels, &places.folders());

    assert_eq!(got.level, Some(Level::ProjectDefault));
    assert_eq!(accent_of(&got), TEAL);
    let theme = got.theme.as_ref().unwrap();
    let accent = theme.options.iter().find(|one| one.option.id == "accent");
    assert_eq!(
        accent.map(|one| (one.value.as_str(), one.chosen)),
        Some(("teal", true))
    );
    assert_eq!(got.complaints, []);
}

#[test]
fn the_options_table_may_be_written_as_its_own_section() {
    let places = Places::new();
    let shared = "[theme]\nuse = \"accented\"\n\n[theme.options]\naccent = \"teal\"\n";
    assert!(refusals(shared, "charter.toml").is_empty());
    let got = drawn(
        &levels_of(&choices::Choices::default(), shared, None),
        &places.folders(),
    );
    assert_eq!(accent_of(&got), TEAL);
}

#[test]
fn a_live_workspace_s_committed_default_draws_its_values_before_the_project_s() {
    let places = Places::new();
    let manifest =
        r#"{"settings": {"theme": {"use": "accented", "options": {"accent": "lavender"}}}}"#;
    let levels = levels_of(&choices::Choices::default(), PROJECT_TEAL, Some(manifest));

    let got = drawn(&levels, &places.folders());

    assert_eq!(got.level, Some(Level::WorkspaceDefault));
    assert_eq!(accent_of(&got), LAVENDER);
    let accent = got
        .theme
        .as_ref()
        .unwrap()
        .options
        .iter()
        .find(|one| one.option.id == "accent")
        .map(|one| one.chosen);
    assert_eq!(accent, Some(true), "the workspace chose lavender itself");
    // A workspace default that names no values draws the theme's defaults, never the project's
    // teal: the values are the choice's own.
    let bare = r#"{"settings": {"theme": {"use": "accented"}}}"#;
    let got = drawn(
        &levels_of(&choices::Choices::default(), PROJECT_TEAL, Some(bare)),
        &places.folders(),
    );
    assert_eq!(accent_of(&got), LAVENDER);
}

#[test]
fn the_person_s_choice_beats_the_committed_default_s_values() {
    let places = Places::new();
    // The person chose the same theme with no values: its defaults, not the team's teal.
    let levels = levels_of(&yours_for_the_project("accented", &[]), PROJECT_TEAL, None);
    let got = drawn(&levels, &places.folders());
    assert_eq!(got.level, Some(Level::ProjectChoice));
    assert_eq!(accent_of(&got), LAVENDER);
    // And their own value is drawn over the team's.
    let shared = "[theme]\nuse = \"accented\"\noptions = { accent = \"lavender\" }\n";
    let levels = levels_of(
        &yours_for_the_project("accented", &[("accent", "teal")]),
        shared,
        None,
    );
    assert_eq!(accent_of(&drawn(&levels, &places.folders())), TEAL);
}

#[test]
fn a_committed_value_that_breaks_a_consent_floor_is_drawn_with_the_floor_held() {
    let places = Places::new();
    let shared = "[theme]\nuse = \"accented\"\noptions = { loud = \"on\" }\n";
    let manifest = r#"{"settings": {"theme": {"use": "accented", "options": {"loud": "on"}}}}"#;
    let purlis = built_in_for(Appearance::Dark);
    for (levels, level) in [
        (
            levels_of(&choices::Choices::default(), shared, None),
            Level::ProjectDefault,
        ),
        (
            levels_of(&choices::Choices::default(), "", Some(manifest)),
            Level::WorkspaceDefault,
        ),
    ] {
        let got = drawn(&levels, &places.folders());
        assert_eq!(got.level, Some(level));
        // The value was layered onto the theme (`Theme::with`) before it was drawn, so the
        // floors hold over it.
        assert!(got.held.is_some(), "{level:?}");
        let theme = got.theme.as_ref().unwrap();
        assert_eq!(theme.value("danger.text"), purlis.value("danger.text"));
        assert_eq!(
            theme.value("danger.surface"),
            purlis.value("danger.surface")
        );
    }
}

#[test]
fn a_committed_value_the_theme_does_not_have_draws_its_default_and_is_said() {
    let places = Places::new();
    let shared = "[theme]\nuse = \"accented\"\noptions = { accent = \"pink\" }\n";
    let got = drawn(
        &levels_of(&choices::Choices::default(), shared, None),
        &places.folders(),
    );
    assert_eq!(accent_of(&got), LAVENDER);
    let said: Vec<&str> = got.complaints.iter().map(|one| one.said.as_str()).collect();
    assert_eq!(
        said,
        ["the option accent of Accented has no value \"pink\", so its default, lavender, is drawn"]
    );
}

#[test]
fn values_that_are_not_ids_are_left_out_when_read_and_refused_when_saved() {
    let shared = "[theme]\nuse = \"accented\"\n\
                  options = { accent = \"teal\", \"Bad Id\" = \"on\", loud = 1 }\n";
    let levels = levels_of(&choices::Choices::default(), shared, None);
    assert_eq!(
        levels.project_default.as_ref().map(|one| &one.options),
        Some(&values(&[("accent", "teal")]))
    );
    assert_eq!(
        refusals(shared, "charter.toml"),
        [
            "theme.options.\"Bad Id\" in charter.toml is \"on\" for an option whose id is not \
             one — an option and its value are each named by their id",
            "theme.options.loud in charter.toml is 1, which is not the id of one of the \
             option's values",
        ]
    );
    assert_eq!(
        refusals("[theme]\noptions = \"teal\"\n", "charter.toml"),
        [
            "theme.options in charter.toml is \"teal\", which is not a table of option ids and \
          their values"
        ]
    );
    let top: toml::Table =
        toml::from_str("[theme]\nuse = \"accented\"\noptions = { accent = \"Teal!\" }\n").unwrap();
    assert_eq!(
        refusals_in_workspace(&top, "workspaces/alpha/workspace.json"),
        [
            "settings.theme.options.accent in workspaces/alpha/workspace.json is \"Teal!\", which \
          is not the id of one of the option's values"
        ]
    );
    let fine: toml::Table =
        toml::from_str("[theme]\nuse = \"accented\"\noptions = { accent = \"teal\" }\n").unwrap();
    assert!(refusals_in_workspace(&fine, "workspaces/alpha/workspace.json").is_empty());
}

#[test]
fn no_more_values_are_read_than_a_theme_has_options() {
    let many: Vec<String> = (0..10).map(|n| format!("o{n} = \"on\"")).collect();
    let shared = format!(
        "[theme]\nuse = \"accented\"\noptions = {{ {} }}\n",
        many.join(", ")
    );
    let levels = levels_of(&choices::Choices::default(), &shared, None);
    assert_eq!(
        levels.project_default.as_ref().map(|one| one.options.len()),
        Some(6)
    );
    assert_eq!(
        refusals(&shared, "charter.toml"),
        ["theme.options in charter.toml names 10 options, more than the 6 a theme may have"]
    );
}

#[test]
fn options_with_no_theme_choose_nothing() {
    let levels = levels_of(
        &choices::Choices::default(),
        "[theme]\noptions = { accent = \"teal\" }\n",
        None,
    );
    assert_eq!(levels.project_default, None);
}

#[test]
fn a_committed_default_s_values_are_read_from_the_plane_on_disk() {
    let places = Places::new();
    std::fs::write(places.plane.join("charter.toml"), PROJECT_TEAL).unwrap();
    std::fs::write(
        places.plane.join("workspaces/alpha/workspace.json"),
        r#"{"name": "alpha", "settings": {"theme": {"use": "accented", "options": {"loud": "on"}}}}"#,
    )
    .unwrap();
    let at_the_root = Levels::read(Some(&places.config), Some(&places.plane), None);
    assert_eq!(
        at_the_root.project_default.map(|one| one.options),
        Some(values(&[("accent", "teal")]))
    );
    // A LOCAL workspace's `settings.theme` is the person's choice there, with its values.
    let in_alpha = Levels::read(Some(&places.config), Some(&places.plane), Some("alpha"));
    assert_eq!(
        in_alpha.workspace_choice.map(|one| one.options),
        Some(values(&[("loud", "on")]))
    );
}
