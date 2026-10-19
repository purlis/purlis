// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Live reload at the engine's interface (#2324): which folders are watched, what a change asks
//! for, how a run of changes is paced, and that what a save redraws is held to the floors and
//! the font approvals as any theme drawn is.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::reload::{self, GAP, Look, MOST_WATCHED, Pace, Reload, ThemesOnly, Watched};
use super::*;

const PROJECT: &str = "01JABCDEFGHJKMNPQRSTVWXYZ0";

fn no_survey() -> Arc<[Offered]> {
    Arc::from(Vec::new())
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
        std::fs::create_dir_all(crate::machine::dir(&config)).unwrap();
        std::fs::write(
            plane.join("charter.toml"),
            format!("[project]\nid = \"{PROJECT}\"\n"),
        )
        .unwrap();
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

fn put(themes: &Path, folder: &str, document: &Value) {
    let dir = themes.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(THEME_JSON), document.to_string()).unwrap();
}

fn dark(name: &str, tokens: Value) -> Value {
    json!({
        "name": name,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    })
}

fn watched(places: &Places) -> Watched {
    Watched::of(&places.folders(), &[])
}

fn folders_of(watched: &Watched) -> Vec<PathBuf> {
    watched.folders().map(Path::to_path_buf).collect()
}

fn redraw() -> Reload {
    Reload::Redraw { survey: false }
}

/// What the window draws with `choice` made for the workspace `alpha`, which reaches a folder of
/// every level.
fn drawn_with(places: &Places, choice: &str) -> Drawn {
    let levels = Levels {
        workspace_choice: Choice::new(choice, None),
        ..Levels::default()
    };
    resolve_levels(
        &LevelInputs {
            levels: &levels,
            folders: &places.folders(),
            theirs: None,
            on: On::Every,
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        no_survey,
    )
}

// ---------------------------------------------------------------------------------------
// What is watched
// ---------------------------------------------------------------------------------------

#[test]
fn each_level_s_themes_its_theme_folders_and_the_folders_in_them_are_watched() {
    let places = Places::new();
    put(&places.workspace(), "brand", &dark("Brand", json!({})));
    std::fs::create_dir_all(places.workspace().join("brand/fonts")).unwrap();
    put(&places.project(), "team", &dark("Team", json!({})));
    std::fs::create_dir_all(places.project().join("team/icons")).unwrap();
    put(&places.machine(), "mine", &dark("Mine", json!({})));
    let got = folders_of(&watched(&places));
    for folder in [
        places.workspace(),
        places.workspace().join("brand"),
        places.workspace().join("brand/fonts"),
        places.project(),
        places.project().join("team"),
        places.project().join("team/icons"),
        places.machine(),
        places.machine().join("mine"),
    ] {
        assert!(got.contains(&folder), "{} in {got:?}", folder.display());
    }
    assert_eq!(got.len(), 8, "{got:?}");
}

#[test]
fn a_level_with_no_themes_folder_yet_hears_one_made_and_nothing_else_there() {
    let places = Places::new();
    let watched = watched(&places);
    let workspace = places.plane.join("workspaces/alpha");
    assert!(folders_of(&watched).contains(&workspace));
    assert!(folders_of(&watched).contains(&places.plane));
    assert_eq!(
        watched.decide([places.project().as_path()], false),
        redraw()
    );
    assert_eq!(
        watched.decide([workspace.join("themes").as_path()], false),
        redraw()
    );
    // The workspace's other files are the plane watch's, not the theme's.
    for other in [
        workspace.join("workspace.json"),
        places.plane.join("charter.toml"),
        workspace.join("todos"),
    ] {
        assert_eq!(watched.decide([other.as_path()], false), Reload::Nothing);
    }
}

#[test]
fn a_link_is_never_watched() {
    let places = Places::new();
    let elsewhere = tempfile::tempdir().unwrap();
    put(elsewhere.path(), "far", &dark("Far", json!({})));
    std::fs::create_dir_all(elsewhere.path().join("far/fonts")).unwrap();
    // The workspace's themes/ is a link; a project theme folder is one; a folder inside a
    // machine theme folder is one.
    std::os::unix::fs::symlink(elsewhere.path(), places.workspace()).unwrap();
    std::fs::create_dir_all(places.project()).unwrap();
    std::os::unix::fs::symlink(elsewhere.path().join("far"), places.project().join("far")).unwrap();
    put(&places.machine(), "mine", &dark("Mine", json!({})));
    std::os::unix::fs::symlink(
        elsewhere.path().join("far/fonts"),
        places.machine().join("mine/fonts"),
    )
    .unwrap();
    let got = folders_of(&watched(&places));
    assert_eq!(
        got,
        [
            places.project(),
            places.machine(),
            places.machine().join("mine")
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>(),
    );
    // Nothing reached through a link is looked at by the look either.
    let look = Look::of(&watched(&places));
    let none = Look::default();
    assert!(
        none.changed(&look)
            .iter()
            .all(|path| !path.starts_with(elsewhere.path())),
        "{:?}",
        none.changed(&look)
    );
}

#[test]
fn at_most_the_cap_of_folders_is_watched_the_nearest_level_s_first() {
    let places = Places::new();
    // Each theme folder with a folder in it: past the cap within the nearest level.
    for n in 0..levels::MOST_FOLDERS {
        put(
            &places.workspace(),
            &format!("w{n:03}"),
            &dark("W", json!({})),
        );
        std::fs::create_dir_all(places.workspace().join(format!("w{n:03}/fonts"))).unwrap();
    }
    const { assert!(1 + 2 * levels::MOST_FOLDERS > MOST_WATCHED) };
    put(&places.machine(), "mine", &dark("Mine", json!({})));
    let watched = watched(&places);
    let got = folders_of(&watched);
    assert_eq!(got.len(), MOST_WATCHED);
    assert!(got.contains(&places.workspace()));
    assert!(!got.contains(&places.machine().join("mine")));
    assert!(watched.left_out() > 0);
}

#[test]
fn a_theme_only_extension_s_folder_and_its_themes_folders_are_watched() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("themes/night")).unwrap();
    let one = ThemesOnly {
        dir: dir.path().to_path_buf(),
        files: vec!["themes/night/theme.json".into(), "top.json".into()],
    };
    let watched = Watched::of(&Folders::default(), std::slice::from_ref(&one));
    assert_eq!(
        folders_of(&watched),
        [
            dir.path().to_path_buf(),
            dir.path().join("themes"),
            dir.path().join("themes/night"),
        ]
    );
    assert_eq!(
        watched.decide(
            [dir.path().join("themes/night/theme.json").as_path()],
            false
        ),
        Reload::Redraw { survey: true }
    );
}

// ---------------------------------------------------------------------------------------
// What a change asks for
// ---------------------------------------------------------------------------------------

#[test]
fn a_save_in_a_theme_folder_in_use_redraws_and_a_change_elsewhere_does_not() {
    let places = Places::new();
    put(&places.project(), "team", &dark("Team", json!({})));
    std::fs::create_dir_all(places.project().join("team/fonts")).unwrap();
    let watched = watched(&places);
    for path in [
        places.project().join("team/theme.json"),
        places.project().join("team/fonts/ui.woff2"),
        places.project().join("team"),
        places.project().join("new-one"),
    ] {
        assert_eq!(
            watched.decide([path.as_path()], false),
            redraw(),
            "{path:?}"
        );
    }
    for path in [
        // An editor's scratch file.
        places.project().join("team/.theme.json.swp"),
        places.plane.join("workspaces/alpha/todos/a.md"),
        places.plane.join("README.md"),
        PathBuf::from("/somewhere/else/theme.json"),
    ] {
        assert_eq!(
            watched.decide([path.as_path()], false),
            Reload::Nothing,
            "{path:?}"
        );
    }
    // A burst whose paths the platform could not name redraws.
    assert_eq!(watched.decide(std::iter::empty(), true), redraw());
    // Many saves in one burst ask once.
    let many: Vec<PathBuf> = (0..50)
        .map(|n| places.project().join(format!("team/{n}.json")))
        .collect();
    assert_eq!(
        watched.decide(many.iter().map(PathBuf::as_path), false),
        redraw()
    );
}

#[test]
fn a_change_named_by_the_root_s_real_path_is_heard() {
    let places = Places::new();
    put(&places.project(), "team", &dark("Team", json!({})));
    let real = places.plane.canonicalize().unwrap();
    let watched = watched(&places);
    assert_eq!(
        watched.decide([real.join("themes/team/theme.json").as_path()], false),
        redraw()
    );
}

#[test]
fn nothing_watched_asks_for_nothing() {
    let watched = Watched::of(&Folders::default(), &[]);
    assert!(watched.is_empty());
    assert_eq!(watched.decide(std::iter::empty(), true), Reload::Nothing);
}

#[test]
fn the_look_finds_a_save_the_platform_did_not_tell() {
    let places = Places::new();
    put(&places.project(), "team", &dark("Team", json!({})));
    let watched = watched(&places);
    let before = Look::of(&watched);
    assert_eq!(before.changed(&Look::of(&watched)), Vec::<PathBuf>::new());
    std::fs::write(places.project().join("team/theme.json"), "{}{}{}").unwrap();
    let changed = before.changed(&Look::of(&watched));
    assert_eq!(changed, [places.project().join("team/theme.json")]);
    assert_eq!(
        watched.decide(changed.iter().map(PathBuf::as_path), false),
        redraw()
    );
}

// ---------------------------------------------------------------------------------------
// How a run of changes is paced
// ---------------------------------------------------------------------------------------

#[test]
fn the_first_save_redraws_at_once_and_a_run_of_saves_is_one_more_redraw() {
    let mut pace = Pace::default();
    let start = Instant::now();
    assert_eq!(pace.due(start), None);
    pace.heard(redraw());
    assert_eq!(pace.wait(start), Some(Duration::ZERO));
    assert_eq!(pace.due(start), Some(redraw()));
    // Twenty saves inside the gap: none drawn until it is over, then one.
    for n in 0..20 {
        let now = start + Duration::from_millis(10 * n);
        pace.heard(Reload::Redraw { survey: n == 3 });
        assert_eq!(pace.due(now), None);
    }
    let wait = pace.wait(start + Duration::from_millis(200)).unwrap();
    assert_eq!(wait, GAP - Duration::from_millis(200));
    assert_eq!(pace.due(start + GAP), Some(Reload::Redraw { survey: true }));
    assert_eq!(pace.due(start + GAP * 3), None);
    assert_eq!(pace.wait(start + GAP * 3), None);
}

#[test]
fn a_change_that_asks_nothing_owes_no_redraw() {
    let mut pace = Pace::default();
    pace.heard(Reload::Nothing);
    assert_eq!(pace.due(Instant::now()), None);
}

#[test]
fn the_last_save_of_a_steady_stream_is_drawn_within_a_second() {
    // A gathered burst, then the gap: what the window waits at most after a save.
    const { assert!(reload::QUIET_FOR.as_millis() + GAP.as_millis() < 1000) };
}

// ---------------------------------------------------------------------------------------
// What a save redraws is held as any theme drawn is
// ---------------------------------------------------------------------------------------

#[test]
fn a_saved_theme_folder_is_drawn_again_as_written() {
    let places = Places::new();
    put(
        &places.project(),
        "team",
        &dark("Team", json!({ "surface.base": "#101010" })),
    );
    let first = drawn_with(&places, "team");
    assert_eq!(first.theme.unwrap().value("surface.base"), "#101010");
    put(
        &places.project(),
        "team",
        &dark("Team", json!({ "surface.base": "#202020" })),
    );
    let again = drawn_with(&places, "team");
    assert_eq!(again.theme.unwrap().value("surface.base"), "#202020");
    assert!(!again.refused);
}

#[test]
fn a_save_that_breaks_a_consent_floor_is_drawn_with_purlis_s_values_and_a_notice() {
    let places = Places::new();
    let ours = |token| built_in_for(Appearance::Dark).value(token).to_owned();
    for themes in [places.workspace(), places.project(), places.machine()] {
        put(&themes, "team", &dark("Team", json!({})));
        assert_eq!(drawn_with(&places, "team").held, None);
        put(
            &themes,
            "team",
            &dark(
                "Team",
                json!({ "danger.text": "#301010", "danger.surface": "#301010" }),
            ),
        );
        let drawn = drawn_with(&places, "team");
        let held = drawn.held.expect("the floor is held on a reload");
        assert_eq!(held.floors, [Floor::Danger]);
        let theme = drawn.theme.unwrap();
        assert_eq!(theme.value("danger.text"), ours("danger.text"));
        assert!(held.said().starts_with("Team breaks the consent floors"));
        std::fs::remove_dir_all(themes.join("team")).unwrap();
    }
}

#[test]
fn a_save_whose_option_breaks_a_floor_is_held_too() {
    let places = Places::new();
    let with = |default: bool| {
        json!({
            "name": "Ember",
            "appearances": [{ "id": "dark", "appearance": "dark" }],
            "options": [{
                "id": "ember", "kind": "toggle", "label": "Ember", "default": default,
                "on": { "tokens": { "danger.text": "#301010", "danger.surface": "#301010" } }
            }]
        })
    };
    put(&places.project(), "ember", &with(false));
    assert_eq!(drawn_with(&places, "ember").held, None);
    put(&places.project(), "ember", &with(true));
    let drawn = drawn_with(&places, "ember");
    assert_eq!(drawn.held.expect("held").floors, [Floor::Danger]);
    assert_eq!(
        drawn.theme.unwrap().value("danger.text"),
        built_in_for(Appearance::Dark).value("danger.text")
    );
}

#[test]
fn a_save_whose_tint_would_break_a_floor_is_held_under_its_own_recipe() {
    let places = Places::new();
    // A dim accent on its controls, on the floor as written and under it in a tinted workspace.
    let dim = |on: bool| {
        json!({
            "name": "Dim",
            "tint": { "on": on, "tokens": ["accent.base"] },
            "appearances": [{
                "id": "dark", "appearance": "dark",
                "tokens": { "accent.base": "#3365e6", "control.hover": "#262623" }
            }]
        })
    };
    put(&places.project(), "dim", &dim(false));
    assert_eq!(drawn_with(&places, "dim").held, None);
    put(&places.project(), "dim", &dim(true));
    let held = drawn_with(&places, "dim").held.expect("held");
    assert_eq!(held.untinted, ["accent.base"]);
}

#[test]
fn a_save_that_is_not_a_theme_draws_purlis_and_says_why() {
    let places = Places::new();
    put(
        &places.project(),
        "team",
        &dark("Team", json!({ "surface.base": "#101010" })),
    );
    assert!(!drawn_with(&places, "team").refused);
    std::fs::write(
        places.project().join("team").join(THEME_JSON),
        dark("Team", json!({ "surface.base": "not a colour" })).to_string(),
    )
    .unwrap();
    let drawn = drawn_with(&places, "team");
    assert!(drawn.refused);
    assert!(!drawn.complaints.is_empty());
    assert_eq!(drawn.theme.unwrap().name, default_theme().name);
}

/// A plain woff2 file's bytes: the signature, then `fill`.
fn woff2(fill: u8) -> Vec<u8> {
    let mut bytes = b"wOF2".to_vec();
    bytes.extend(std::iter::repeat_n(fill, 64));
    bytes
}

/// `places`' project, approved on this machine with what it contributes now.
fn approve(places: &Places) {
    let contributes = crate::machine::Contribution::of(&places.plane);
    crate::machine::update(&places.config, |store| {
        store.approve(&places.plane, 1, contributes);
    })
    .unwrap();
}

#[test]
fn a_project_font_changed_after_the_approval_stays_refused_and_says_so() {
    let places = Places::new();
    let with_font = json!({
        "name": "Typed",
        "fonts": { "ui": { "family": "Typed", "faces": [{ "file": "fonts/ui.woff2" }] } },
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    });
    put(&places.project(), "typed", &with_font);
    let fonts = places.project().join("typed/fonts");
    std::fs::create_dir_all(&fonts).unwrap();
    std::fs::write(fonts.join("ui.woff2"), woff2(1)).unwrap();
    approve(&places);
    let first = drawn_with(&places, "typed");
    assert!(!first.refused, "{:?}", first.complaints);
    // The font is saved again while the project is open: refused until approved again.
    std::fs::write(fonts.join("ui.woff2"), woff2(2)).unwrap();
    let again = drawn_with(&places, "typed");
    assert!(again.refused);
    let said: Vec<&str> = again
        .complaints
        .iter()
        .map(|one| one.said.as_str())
        .collect();
    assert!(
        said.iter()
            .any(|one| one.contains("themes/typed/fonts/ui.woff2 has changed since you approved")),
        "{said:?}"
    );
    // A font added is refused the same way.
    approve(&places);
    assert!(!drawn_with(&places, "typed").refused);
    let added = json!({
        "name": "Typed",
        "fonts": { "mono": { "family": "Mono", "faces": [{ "file": "fonts/mono.woff2" }] } },
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    });
    std::fs::write(fonts.join("mono.woff2"), woff2(3)).unwrap();
    put(&places.project(), "typed", &added);
    let drawn = drawn_with(&places, "typed");
    assert!(drawn.refused);
    assert!(
        drawn
            .complaints
            .iter()
            .any(|one| one.said.contains("is not a font file you approved")),
        "{:?}",
        drawn.complaints
    );
}

#[test]
fn colours_saved_in_an_unapproved_project_s_folder_draw_with_no_approval() {
    // A theme folder holds no font: nothing in it asks.
    let places = Places::new();
    put(
        &places.workspace(),
        "brand",
        &dark("Brand", json!({ "surface.base": "#121212" })),
    );
    let levels = Levels {
        workspace_choice: Choice::new("brand", None),
        ..Levels::default()
    };
    let drawn = resolve_levels(
        &LevelInputs {
            levels: &levels,
            folders: &places.folders(),
            theirs: None,
            on: On::Every,
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        no_survey,
    );
    assert!(!drawn.refused);
    assert_eq!(drawn.theme.unwrap().value("surface.base"), "#121212");
}

#[test]
fn the_extensions_watched_are_the_installed_ones_of_themes_alone() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join("config");
    let make = |id: &str, more: &str| {
        let dir = home.path().join(id);
        std::fs::create_dir_all(dir.join("themes")).unwrap();
        std::fs::write(
            dir.join("themes/dark.json"),
            r#"{"name":"Dark","appearance":"dark","tokens":{}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join(crate::extension::MANIFEST),
            format!(
                r#"{{"version":1,"id":"{id}","name":"{id}","contributes":{{"themes":[{{"name":"Dark","file":"themes/dark.json"}}]{more}}}}}"#
            ),
        )
        .unwrap();
        crate::extension::install(&config, &crate::extension::BuiltIn::none(), &dir).unwrap();
        dir
    };
    let only = make("only", "");
    make("panelled", r#","panels":[{"id":"p","title":"P"}]"#);
    assert_eq!(
        ThemesOnly::installed(&config, &crate::extension::BuiltIn::none()),
        [ThemesOnly {
            dir: only,
            files: vec!["themes/dark.json".to_owned()],
        }]
    );
}

/// Every path the platform names for one save of `file`, by the shapes notify reports: a plain
/// write (the file), an atomic save (a scratch file, then the rename onto the file), the file
/// made, and its metadata changed — each as the level spells the root and as the disk does.
fn shapes_of_a_save(file: &Path, real: &Path, given: &Path) -> Vec<Vec<PathBuf>> {
    let respelled = real.join(file.strip_prefix(given).unwrap());
    let mut shapes = Vec::new();
    for one in [file.to_path_buf(), respelled] {
        let scratch = one.with_file_name("theme.json.tmp-4182");
        shapes.push(vec![one.clone()]);
        shapes.push(vec![scratch.clone(), one.clone()]);
        shapes.push(vec![scratch]);
        shapes.push(vec![one.parent().unwrap().to_path_buf(), one.clone()]);
    }
    shapes
}

#[test]
fn every_shape_of_a_save_in_a_theme_folder_redraws_even_one_made_after_the_watch_was_set() {
    let places = Places::new();
    // What the window watches, set while no level has a `themes/` yet.
    let watched = watched(&places);
    let real = places.plane.canonicalize().unwrap();
    let home = places.plane.parent().unwrap().to_path_buf();
    let real_home = home.canonicalize().unwrap();
    for themes in [places.machine(), places.project(), places.workspace()] {
        put(&themes, "made-later", &dark("Later", json!({})));
        for file in [
            themes.join("made-later/theme.json"),
            themes.join("made-later/fonts/ui.woff2"),
            themes.join("made-later/icons/set.json"),
        ] {
            for shape in shapes_of_a_save(&file, &real_home, &home) {
                assert_eq!(
                    watched.decide(shape.iter().map(PathBuf::as_path), false),
                    redraw(),
                    "{shape:?}"
                );
            }
        }
    }
    // An editor's own scratch file alone, and anything outside a level's `themes/`, is not.
    for path in [
        places.project().join("made-later/.theme.json.swp"),
        real.join("README.md"),
        places.plane.join("workspaces/alpha/workspace.json"),
    ] {
        assert_eq!(
            watched.decide([path.as_path()], false),
            Reload::Nothing,
            "{path:?}"
        );
    }
}
