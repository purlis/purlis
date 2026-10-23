// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Settings › Theme's gallery at the engine's interface (#2331): which themes a level's
//! gallery holds, what each card says, what a card draws, and Duplicate as my theme.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::gallery::{self, Card, From, Shelf};
use super::levels::Reach;
use super::*;

fn put(themes: &Path, folder: &str, document: &Value) {
    let dir = themes.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(THEME_JSON), document.to_string()).unwrap();
}

/// A dark theme whose base surface is `surface`, extending Purlis Classic.
fn dusk(name: &str, surface: &str) -> Value {
    json!({
        "name": name,
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": { "surface.base": surface } }]
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

fn shelf<'a>(folders: &'a Folders, reach: Reach, offered: &'a [Offered]) -> Shelf<'a> {
    Shelf {
        folders,
        reach,
        on: On::Every,
        offered,
    }
}

fn ids(cards: &[Card]) -> Vec<&str> {
    cards.iter().map(|one| one.id.as_str()).collect()
}

#[test]
fn a_levels_gallery_holds_the_themes_a_choice_there_reaches() {
    let places = Places::new();
    put(&places.machine(), "mine", &dusk("Mine", "#101010"));
    put(&places.project(), "team", &dusk("Team", "#202020"));
    put(&places.workspace(), "client", &dusk("Client", "#303030"));
    let folders = places.folders();
    let offered = [Offered::of(
        "solarized".into(),
        "Solarized Dark".into(),
        &dusk("Solarized Dark", "#002b36").to_string(),
    )];
    let at = |reach| ids(&gallery::cards(&shelf(&folders, reach, &offered))).join(" ");
    assert_eq!(
        at(Reach::Machine),
        "purlis purlis-classic mine solarized/Solarized Dark"
    );
    assert_eq!(
        at(Reach::Project),
        "purlis purlis-classic team mine solarized/Solarized Dark"
    );
    assert_eq!(
        at(Reach::Workspace),
        "purlis purlis-classic client team mine solarized/Solarized Dark"
    );
    // A project keeps to the extensions it has on.
    let none: [String; 0] = [];
    let kept = gallery::cards(&Shelf {
        on: On::Only(&none),
        ..shelf(&folders, Reach::Project, &offered)
    });
    assert_eq!(ids(&kept), ["purlis", "purlis-classic", "team", "mine"]);
}

#[test]
fn a_name_two_levels_have_is_a_card_at_each_nearest_first() {
    let places = Places::new();
    put(&places.machine(), "brand", &dusk("Brand, mine", "#101010"));
    put(
        &places.project(),
        "brand",
        &dusk("Brand, the team's", "#202020"),
    );
    let folders = places.folders();
    let cards = gallery::cards(&shelf(&folders, Reach::Project, &[]));
    let brand: Vec<&Card> = cards.iter().filter(|one| one.id == "brand").collect();
    assert_eq!(brand.len(), 2);
    assert_eq!(brand[0].name, "Brand, the team's");
    assert_eq!(brand[0].from, From::Project);
    assert_eq!(brand[1].name, "Brand, mine");
    assert_eq!(brand[1].from, From::Machine);
}

#[test]
fn a_card_says_its_appearances_options_and_where_it_comes_from() {
    let places = Places::new();
    let folders = places.folders();
    let cards = gallery::cards(&shelf(&folders, Reach::Machine, &[]));
    let classic = &cards[1];
    assert_eq!(classic.name, "Purlis Classic");
    assert_eq!(classic.from, From::BuiltIn);
    let tags: Vec<(&str, Appearance)> = classic
        .appearances
        .iter()
        .map(|one| (one.id.as_str(), one.appearance))
        .collect();
    assert_eq!(
        tags,
        [("dark", Appearance::Dark), ("light", Appearance::Light)]
    );
    assert!(!classic.refused);
    assert!(classic.warnings.is_empty(), "{:?}", classic.warnings);
    assert!(classic.floors.is_empty());
    assert_eq!(classic.follows, None, "Classic has a dark and a light");
    assert!(classic.duplicable && !classic.folder);
}

#[test]
fn a_card_carries_the_checks_warnings_and_what_following_the_system_does() {
    let places = Places::new();
    // Text the colour of its ground: the check cannot miss it.
    put(
        &places.machine(),
        "murk",
        &json!({
            "name": "Murk",
            "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {
                "surface.base": "#202020", "text.primary": "#212121"
            } }]
        }),
    );
    let folders = places.folders();
    let cards = gallery::cards(&shelf(&folders, Reach::Machine, &[]));
    let murk = cards.iter().find(|one| one.id == "murk").unwrap();
    assert_eq!(murk.from, From::Machine);
    assert!(
        murk.warnings
            .iter()
            .any(|line| line.contains("text.primary on surface.base")),
        "{:?}",
        murk.warnings
    );
    assert_eq!(
        murk.follows.as_deref(),
        Some("Murk has only a dark appearance, so following the system stays dark")
    );
    // The same lines `purlis theme check` prints.
    let read = gallery::theme_of(&shelf(&folders, Reach::Machine, &[]), "murk", None).unwrap();
    assert_eq!(
        murk.warnings,
        warnings_said(&read.theme, &check_every(&read.theme))
    );
}

#[test]
fn a_refused_theme_is_a_card_that_says_why() {
    let places = Places::new();
    put(
        &places.machine(),
        "broken",
        &json!({ "name": "Broken", "extends": "purlis-classic", "appearances": [
            { "id": "dark", "tokens": { "surface.base": "red" } }
        ] }),
    );
    let folders = places.folders();
    let cards = gallery::cards(&shelf(&folders, Reach::Machine, &[]));
    let broken = cards.iter().find(|one| one.id == "broken").unwrap();
    assert!(broken.refused);
    assert!(!broken.said.is_empty());
    assert!(broken.appearances.is_empty());
    assert!(!broken.duplicable);
}

#[test]
fn a_card_draws_in_the_appearance_asked_for_with_the_floors_held() {
    let places = Places::new();
    put(&places.project(), "team", &dusk("Team", "#141414"));
    let folders = places.folders();
    let at = shelf(&folders, Reach::Project, &[]);
    let team = gallery::drawn(
        &at,
        "team",
        None,
        None,
        &Values::new(),
        Appearance::Light,
        Contrast::Standard,
    )
    .unwrap();
    assert_eq!(team.theme.unwrap().value("surface.base"), "#141414");
    let light = gallery::drawn(
        &at,
        "purlis-classic",
        None,
        Some("light"),
        &Values::new(),
        Appearance::Dark,
        Contrast::Standard,
    )
    .unwrap();
    assert_eq!(light.theme.unwrap().appearance, Appearance::Light);
    let followed = gallery::drawn(
        &at,
        "purlis-classic",
        None,
        Some("system"),
        &Values::new(),
        Appearance::Light,
        Contrast::Standard,
    );
    assert_eq!(
        followed.unwrap().theme.unwrap().appearance,
        Appearance::Light
    );
    // A theme the gallery does not hold draws nothing: the window keeps what it draws.
    assert!(
        gallery::drawn(
            &at,
            "nowhere",
            None,
            None,
            &Values::new(),
            Appearance::Dark,
            Contrast::Standard
        )
        .is_none()
    );
    // A workspace's folder is out of a project's reach.
    put(&places.workspace(), "client", &dusk("Client", "#303030"));
    assert!(
        gallery::drawn(
            &at,
            "client",
            None,
            None,
            &Values::new(),
            Appearance::Dark,
            Contrast::Standard
        )
        .is_none()
    );
}

#[test]
fn open_folder_shows_a_folder_within_reach_and_nothing_else() {
    let places = Places::new();
    put(&places.project(), "team", &dusk("Team", "#141414"));
    let folders = places.folders();
    let at = shelf(&folders, Reach::Project, &[]);
    assert_eq!(
        gallery::folder_of(&at, "team", None),
        Some(places.project().join("team"))
    );
    assert_eq!(gallery::folder_of(&at, "purlis-classic", None), None);
    assert_eq!(gallery::folder_of(&at, "missing", None), None);
    assert_eq!(gallery::folder_of(&at, "../team", None), None);
}

#[test]
fn duplicate_as_my_theme_makes_a_folder_of_my_own_that_extends_the_original() {
    let places = Places::new();
    put(&places.machine(), "dusk", &dusk("Dusk", "#141414"));
    let folders = places.folders();
    let at = shelf(&folders, Reach::Machine, &[]);
    let made = gallery::duplicate(&places.config, &at, "dusk", None).unwrap();
    assert_eq!(made, "dusk-mine");
    let written: Value = serde_json::from_str(
        &std::fs::read_to_string(places.machine().join("dusk-mine").join(THEME_JSON)).unwrap(),
    )
    .unwrap();
    assert_eq!(written["extends"], "dusk");
    assert_eq!(written["name"], "Dusk (mine)");
    assert_eq!(written["$schema"], SCHEMA_URL);
    // It is a card of the gallery at once, and draws what its parent draws.
    let cards = gallery::cards(&at);
    let mine = cards.iter().find(|one| one.id == "dusk-mine").unwrap();
    assert_eq!(mine.name, "Dusk (mine)");
    assert!(!mine.refused, "{:?}", mine.said);
    let drawn = gallery::drawn(
        &at,
        "dusk-mine",
        None,
        None,
        &Values::new(),
        Appearance::Dark,
        Contrast::Standard,
    )
    .unwrap();
    assert_eq!(drawn.theme.unwrap().value("surface.base"), "#141414");
    // A second duplicate takes the next free name; nothing is written over.
    assert_eq!(
        gallery::duplicate(&places.config, &at, "dusk", None).unwrap(),
        "dusk-mine-2"
    );
}

#[test]
fn purlis_classic_is_duplicated_into_a_themes_folder_made_for_it() {
    let places = Places::new();
    let folders = places.folders();
    let at = shelf(&folders, Reach::Workspace, &[]);
    assert!(!places.machine().exists());
    let made = gallery::duplicate(&places.config, &at, "purlis-classic", None).unwrap();
    assert_eq!(made, "purlis-classic-mine");
    let drawn = gallery::drawn(
        &at,
        &made,
        None,
        Some("light"),
        &Values::new(),
        Appearance::Dark,
        Contrast::Standard,
    )
    .unwrap();
    assert_eq!(
        drawn.theme.unwrap().values,
        classic().of(Appearance::Light).unwrap().values
    );
}

#[test]
fn a_theme_a_folder_of_my_own_cannot_extend_is_not_duplicated() {
    let places = Places::new();
    put(&places.project(), "team", &dusk("Team", "#141414"));
    let folders = places.folders();
    let offered = [Offered::of(
        "solarized".into(),
        "Solarized Dark".into(),
        &dusk("Solarized Dark", "#002b36").to_string(),
    )];
    let at = shelf(&folders, Reach::Project, &offered);
    let cards = gallery::cards(&at);
    for id in ["team", "solarized/Solarized Dark"] {
        let card = cards.iter().find(|one| one.id == id).unwrap();
        assert!(!card.duplicable, "{id}");
        assert!(
            gallery::duplicate(&places.config, &at, id, None).is_err(),
            "{id}"
        );
    }
    assert!(gallery::duplicate(&places.config, &at, "nowhere", None).is_err());
    assert!(!places.machine().exists(), "nothing was made");
}

#[cfg(unix)]
#[test]
fn duplicate_refuses_a_themes_folder_reached_through_a_link() {
    let places = Places::new();
    let elsewhere = places.plane.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::create_dir_all(crate::machine::dir(&places.config)).unwrap();
    std::os::unix::fs::symlink(&elsewhere, places.machine()).unwrap();
    let folders = places.folders();
    let at = shelf(&folders, Reach::Machine, &[]);
    assert!(gallery::duplicate(&places.config, &at, "purlis-classic", None).is_err());
    assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
}

#[test]
fn a_cards_stamp_changes_with_what_it_draws() {
    let places = Places::new();
    put(&places.machine(), "dusk", &dusk("Dusk", "#141414"));
    let folders = places.folders();
    let at = shelf(&folders, Reach::Machine, &[]);
    // Past the two built-in themes.
    let before = gallery::cards(&at)[2].stamp.clone();
    assert_eq!(gallery::cards(&at)[2].stamp, before);
    put(&places.machine(), "dusk", &dusk("Dusk", "#141415"));
    assert_ne!(gallery::cards(&at)[2].stamp, before);
}

#[cfg(unix)]
#[test]
fn a_failed_duplicate_takes_away_only_what_it_made() {
    let home = tempfile::tempdir().unwrap();
    let at = rustix::fs::open(
        home.path(),
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY,
        rustix::fs::Mode::empty(),
    )
    .unwrap();
    // A theme.json someone else wrote is kept, and so is its folder.
    put(home.path(), "theirs", &dusk("Theirs", "#141414"));
    gallery::write::discard(&at, "theirs", false);
    assert!(home.path().join("theirs").join(THEME_JSON).is_file());
    // The duplicate's own file and folder both go.
    put(home.path(), "mine", &dusk("Mine", "#141414"));
    gallery::write::discard(&at, "mine", true);
    assert!(!home.path().join("mine").exists());
    // An empty folder it made goes.
    std::fs::create_dir(home.path().join("empty")).unwrap();
    gallery::write::discard(&at, "empty", false);
    assert!(!home.path().join("empty").exists());
}
