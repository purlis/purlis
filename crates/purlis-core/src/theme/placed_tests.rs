// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Paths said relative to their level, at the engine's interface (#2763): what reading the
//! levels' theme folders says names each file by its level and its path there, and each
//! complaint carries that place as data.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::levels::{Found, Reach};
use super::*;

/// The name of the folder the levels sit in, and of one outside them: spaces, quotes and
/// brackets, as a person's folders have.
const AWKWARD: &str = "Client Werk 'Bobz' [vintage] (kopie) \"q\"";
const ELSEWHERE: &str = "Elsewhere [x] 'y' (z)";

/// Every word of the folders the levels sit in, none of which a sentence says.
const WORDS: [&str; 8] = [
    "Client",
    "Werk",
    "Bobz",
    "vintage",
    "kopie",
    "Elsewhere",
    "[x]",
    "(z)",
];

fn put(themes: &Path, folder: &str, document: &Value) {
    let dir = themes.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(THEME_JSON), document.to_string()).unwrap();
}

/// A one-appearance dark theme called `name`.
fn named(name: &str) -> Value {
    json!({ "name": name, "appearances": [{ "id": "dark", "appearance": "dark" }] })
}

/// A config home and a plane with the workspace `alpha`, both inside a folder of an awkward
/// name, and a folder beside it of another.
struct Places {
    home: tempfile::TempDir,
    config: PathBuf,
    plane: PathBuf,
}

impl Places {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let inside = home.path().join(AWKWARD);
        let config = inside.join("config");
        let plane = inside.join("plane");
        std::fs::create_dir_all(plane.join("workspaces/alpha")).unwrap();
        std::fs::create_dir_all(plane.join(".purlis")).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        Self {
            home,
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

    fn elsewhere(&self) -> PathBuf {
        self.home.path().join(ELSEWHERE)
    }
}

/// What reading the folder `name` within a workspace's reach says.
fn said_of(folders: &Folders, name: &str) -> Vec<Complaint> {
    match folders.find(name, Reach::Workspace) {
        Found::Read(resolved) => resolved.complaints,
        Found::Missing(said) => said,
    }
}

/// `said` says no part of where the levels are.
fn no_outside_path(places: &Places, said: &[Complaint]) {
    let home = places.home.path().display().to_string();
    for one in said {
        assert!(!one.said.contains(&home), "{}", one.said);
        for word in WORDS {
            assert!(!one.said.contains(word), "{word}: {}", one.said);
        }
    }
}

fn at(level: PlacedIn, path: &str) -> Option<Placed> {
    Some(Placed {
        level,
        name: None,
        path: path.to_owned(),
    })
}

fn in_alpha(path: &str) -> Option<Placed> {
    Some(Placed {
        name: Some("alpha".to_owned()),
        ..at(PlacedIn::Workspace, path).unwrap()
    })
}

#[cfg(unix)]
#[test]
fn every_sentence_of_a_level_s_read_names_the_file_inside_its_level_and_never_where_it_is() {
    let places = Places::new();
    // The project's: a theme.json that is not JSON, and a folder that is a link to one outside.
    std::fs::create_dir_all(places.project().join("broken")).unwrap();
    std::fs::write(
        places.project().join("broken").join(THEME_JSON),
        "{ not json",
    )
    .unwrap();
    put(&places.elsewhere(), "outer", &named("Outer"));
    std::os::unix::fs::symlink(
        places.elsewhere().join("outer"),
        places.project().join("outer"),
    )
    .unwrap();
    // The person's own: an icon set that is not there.
    let mut iconic = named("Iconic");
    iconic["icons"] = Value::from("icons/gone.json");
    put(&places.machine(), "iconic", &iconic);
    // The workspace's: a face whose file is not there.
    let mut faceless = named("Faceless");
    faceless["fonts"] =
        json!({ "ui": { "family": "Plex", "faces": [{ "file": "fonts/Plex.woff2" }] } });
    put(&places.workspace(), "faceless", &faceless);

    let folders = places.folders();
    let broken = said_of(&folders, "broken");
    let outer = said_of(&folders, "outer");
    let iconic = said_of(&folders, "iconic");
    let faceless = said_of(&folders, "faceless");
    for said in [&broken, &outer, &iconic, &faceless] {
        no_outside_path(&places, said);
    }

    // Each names the file a person opens to put it right, by its level and its path there.
    let first = |said: &[Complaint]| said.first().cloned().expect("a complaint");
    let broken = first(&broken);
    assert!(
        broken
            .said
            .starts_with("the project's themes/broken/theme.json: "),
        "{}",
        broken.said
    );
    assert_eq!(broken.at, at(PlacedIn::Project, "themes/broken"));
    let outer = first(&outer);
    assert_eq!(
        outer.said,
        "the project's themes/outer is a link or not a folder, so it is not read"
    );
    assert_eq!(outer.at, at(PlacedIn::Project, "themes/outer"));
    let iconic = first(&iconic);
    assert_eq!(
        iconic.said,
        "your purlis config folder's themes/iconic has no icons/gone.json"
    );
    assert_eq!(iconic.at, at(PlacedIn::Machine, "themes/iconic"));
    let faceless = faceless
        .iter()
        .find(|one| one.said.contains("fonts/Plex.woff2"))
        .expect("the face is said");
    assert_eq!(faceless.at, in_alpha("themes/faceless"));
}

#[cfg(unix)]
#[test]
fn a_level_reached_through_a_link_is_said_by_its_level_and_carries_it() {
    let places = Places::new();
    std::fs::create_dir_all(places.elsewhere()).unwrap();
    std::fs::create_dir_all(places.machine().parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(places.elsewhere(), places.machine()).unwrap();
    let said = said_of(&places.folders(), "");
    no_outside_path(&places, &said);
    let linked = said
        .iter()
        .find(|one| one.said.contains("reached through a link"))
        .expect("the link is said");
    assert!(
        linked
            .said
            .starts_with("your purlis config folder's themes is reached through a link"),
        "{}",
        linked.said
    );
    assert_eq!(linked.at, at(PlacedIn::Machine, "themes"));
}

#[test]
fn a_parent_s_complaint_keeps_the_place_of_the_folder_it_was_said_of() {
    let places = Places::new();
    let mut parent = named("Parent");
    parent["appearances"][0]["tokens"] = json!({ "surface.base": "not a colour" });
    put(&places.machine(), "parent", &parent);
    let mut child = named("Child");
    child["extends"] = Value::from("parent");
    put(&places.project(), "child", &child);
    let said = said_of(&places.folders(), "child");
    no_outside_path(&places, &said);
    let of_parent = said
        .iter()
        .find(|one| one.said.starts_with("in \"parent\": "))
        .expect("the parent's complaint reaches its child");
    assert_eq!(of_parent.at, at(PlacedIn::Machine, "themes/parent"));
}

#[test]
fn the_place_is_data_beside_the_sentence() {
    let places = Places::new();
    put(
        &places.workspace(),
        "odd",
        &json!({ "name": 3, "appearances": [] }),
    );
    let said = said_of(&places.folders(), "odd");
    let json = serde_json::to_value(said.first().expect("a complaint")).unwrap();
    assert_eq!(
        json["at"],
        json!({ "level": "workspace", "name": "alpha", "path": "themes/odd" })
    );
    // A theme read from no folder carries no place.
    let unplaced = load(&json!({ "name": 3 }));
    let json = serde_json::to_value(&unplaced.complaints[0]).unwrap();
    assert!(json.get("at").is_none(), "{json}");
}

#[cfg(unix)]
#[test]
fn the_person_s_own_theme_is_said_in_their_purlis_config_folder() {
    let places = Places::new();
    let folder = crate::machine::dir(&places.config);
    std::fs::create_dir_all(&folder).unwrap();
    let mut mine = named("Mine");
    mine["icons"] = Value::from("gone.json");
    std::fs::write(folder.join(THEME_JSON), mine.to_string()).unwrap();
    let theirs = read_theirs(Some(&places.config));
    no_outside_path(&places, &theirs.complaints);
    let gone = theirs
        .complaints
        .iter()
        .find(|one| one.said.contains("gone.json"))
        .expect("the icon set is said");
    assert_eq!(gone.said, "your purlis config folder has no gone.json");
    assert_eq!(gone.at, at(PlacedIn::Machine, ""));
}

#[cfg(unix)]
#[test]
fn a_chat_s_draft_is_said_as_the_chat_s_theme_draft() {
    let places = Places::new();
    let chat = crate::reopen::mint();
    let place = drafts::Place {
        plane: &places.plane,
        workspace: Some("alpha"),
    };
    put(&places.workspace(), "dusk", &named("Dusk"));
    let started = drafts::start(Some(&places.config), place, &chat, "dusk")
        .unwrap()
        .path;
    std::fs::write(started.join(THEME_JSON), "{ not json").unwrap();
    let draft = drafts::Draft {
        chat: chat.clone(),
        folder: "dusk".to_owned(),
        kept_to: drafts::kept_to(&places.folders(), Some("alpha"), "dusk"),
    };
    let said = drafts::read(Some(&places.config), place, &draft, true)
        .map(|resolved| resolved.complaints)
        .unwrap_or_else(|said| said);
    no_outside_path(&places, &said);
    let broken = said
        .iter()
        .find(|one| one.said.contains("theme.json"))
        .expect("the draft's file is said");
    assert!(
        broken
            .said
            .starts_with("the chat's theme draft dusk/theme.json: "),
        "{}",
        broken.said
    );
    assert_eq!(broken.at, at(PlacedIn::Draft, "dusk"));
}
