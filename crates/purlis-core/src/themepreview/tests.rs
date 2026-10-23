// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use serde_json::{Map, Value, json};

use super::*;

fn args(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => unreachable!("arguments are an object"),
    }
}

fn specimen(theme: &str, appearance: Option<Appearance>) -> Specimen {
    Specimen {
        theme: theme.to_owned(),
        appearance,
        high_contrast: false,
        options: Values::new(),
    }
}

#[test]
fn a_theme_alone_asks_for_one_specimen_in_its_first_appearance() {
    assert_eq!(
        asked(&args(json!({"theme": "purlis-classic"}))),
        Ok(vec![specimen("purlis-classic", None)])
    );
}

#[test]
fn purlis_classic_is_drawn_in_the_appearance_asked_for() {
    let dark = chosen(&specimen("purlis-classic", Some(Appearance::Dark))).expect("dark");
    let light = chosen(&specimen("purlis-classic", Some(Appearance::Light))).expect("light");
    let first = chosen(&specimen("purlis-classic", None)).expect("its first");
    assert_eq!(dark, theme::classic().first());
    assert_eq!(light, theme::classic().of(Appearance::Light).unwrap());
    assert_eq!(first, theme::classic().first());
}

#[test]
fn a_specimen_is_drawn_as_the_window_draws_its_theme_with_the_consent_floors_held() {
    for appearance in [None, Some(Appearance::Dark), Some(Appearance::Light)] {
        let asked = specimen("purlis-classic", appearance);
        let theme = chosen(&asked).expect("a theme");
        let drawn = resolved(&Reached::nowhere(), &asked).expect("drawn");
        assert_eq!(drawn, theme::draw(theme));
        assert_eq!(
            drawn.held, None,
            "Purlis Classic clears every consent floor"
        );
    }
}

#[test]
fn purlis_s_dark_and_light_are_named_by_their_old_ids_too() {
    // `purlis-dark` and `purlis-light`, and the ids they had, name Purlis's (#3034).
    let purlis = theme::purlis();
    assert_eq!(
        chosen(&specimen("purlis-light", None)),
        Ok(purlis.of(Appearance::Light).unwrap())
    );
    assert_eq!(
        chosen(&specimen("charter-dark", Some(Appearance::Dark))),
        Ok(purlis.first())
    );
}

#[test]
fn an_appearance_a_one_appearance_theme_lacks_is_refused_naming_the_one_that_has_it() {
    let why = chosen(&specimen("purlis-light", Some(Appearance::Dark))).expect_err("refused");
    assert!(why.contains("only a light appearance"), "{why}");
    assert!(
        why.contains("ask for purlis with appearance `dark`"),
        "{why}"
    );
}

#[test]
fn a_theme_no_level_within_reach_has_is_refused_naming_what_may_be_named() {
    // A folder's name is an id the app looks up where the chat works: here, nowhere.
    assert!(asked(&args(json!({"theme": "solarized"}))).is_ok());
    let why = resolved(&Reached::nowhere(), &specimen("solarized", None)).expect_err("refused");
    assert!(
        why.contains("no theme `solarized` within this chat's reach"),
        "{why}"
    );
    for id in [
        "purlis-classic",
        "purlis-dark",
        "purlis-light",
        "draft:<folder>",
    ] {
        assert!(why.contains(id), "{why}");
    }
}

#[test]
fn an_id_is_never_a_path_and_a_path_is_no_id() {
    for path in [
        "/etc/passwd",
        "../themes/dusk",
        "./dusk",
        "~/dusk",
        "project:../dusk",
        "draft:../dusk",
        "workspace:../alpha:dusk",
        "workspace:alpha:dusk:more",
        "machine:alpha:dusk",
        "system",
        "",
    ] {
        assert_eq!(named(path), None, "{path}");
    }
    let why = asked(&args(json!({"theme": "/etc/passwd"}))).expect_err("refused");
    assert!(why.contains("is not a theme's id"), "{why}");
    assert_eq!(
        named("workspace:alpha:dusk"),
        Some(Named::Folder {
            name: "dusk",
            level: Some(Reach::Workspace),
            workspace: Some("alpha"),
        })
    );
    assert_eq!(named("draft:dusk"), Some(Named::Draft("dusk")));
    assert_eq!(named("solarized/Solarized: Dark"), Some(Named::Extension));
    assert_eq!(named("purlis-light"), Some(Named::BuiltIn));
    // `<extension>/<theme>` is looked up among the extension themes in force, never read.
    assert_eq!(named("themes/dusk"), Some(Named::Extension));
}

#[test]
fn no_theme_or_an_odd_value_is_refused_in_words_that_say_what_to_give() {
    assert!(
        asked(&args(json!({})))
            .expect_err("refused")
            .contains("give `theme`")
    );
    assert!(
        asked(&args(json!({"theme": 3})))
            .expect_err("refused")
            .contains("as text")
    );
    assert!(
        asked(&args(
            json!({"theme": "purlis-classic", "appearance": "sepia"})
        ))
        .expect_err("refused")
        .contains("`dark`, `light` or `dark-high-contrast`")
    );
    assert!(
        asked(&args(
            json!({"theme": "purlis-classic", "workspace": "alpha"})
        ))
        .expect_err("refused")
        .contains("not `workspace`")
    );
}

#[test]
fn side_by_side_asks_for_a_second_specimen_that_keeps_what_it_does_not_change() {
    assert_eq!(
        asked(&args(json!({
            "theme": "purlis-classic",
            "appearance": "dark",
            "side_by_side": {"appearance": "light"},
        }))),
        Ok(vec![
            specimen("purlis-classic", Some(Appearance::Dark)),
            specimen("purlis-classic", Some(Appearance::Light)),
        ])
    );
    assert_eq!(
        asked(&args(json!({
            "theme": "purlis-dark",
            "side_by_side": {"theme": "purlis-light"},
        }))),
        Ok(vec![
            specimen("purlis-dark", None),
            specimen("purlis-light", None)
        ])
    );
}

#[test]
fn side_by_side_that_changes_nothing_or_names_what_is_not_there_is_refused() {
    assert!(
        asked(&args(
            json!({"theme": "purlis-classic", "side_by_side": {}})
        ))
        .expect_err("refused")
        .contains("say what the second specimen changes")
    );
    assert!(
        asked(&args(
            json!({"theme": "purlis-classic", "side_by_side": {"theme": "purlis-dark", "appearance": "light"}})
        ))
        .expect_err("refused")
        .contains("only a dark appearance")
    );
    assert!(
        asked(&args(
            json!({"theme": "purlis-classic", "side_by_side": true})
        ))
        .expect_err("refused")
        .contains("is an object")
    );
}

#[test]
fn the_tool_takes_no_workspace_project_path_or_directory() {
    // The chat tools' boundary (`chattools`): names, never paths.
    let schema = (TOOLS[0].schema)();
    let text = schema.to_string();
    for word in ["workspace", "project", "path", "dir"] {
        assert!(!text.contains(&format!("\"{word}")), "{word} in {text}");
    }
    assert!(TOOLS[0].read_only);
    assert!(is_one(TOOL));
}

#[test]
fn the_model_is_told_what_each_picture_is_and_that_it_is_sample_content() {
    let painted = |theme: &str, appearance| Painted {
        theme: theme.to_owned(),
        appearance,
        high_contrast: false,
        preview: false,
        width: 2560,
        height: 1600,
        millis: 640,
        png: vec![1, 2, 3],
    };
    let one = said(&[painted("purlis-classic", Appearance::Dark)]);
    assert!(
        one.starts_with(
            "The theme specimen in purlis-classic (dark), 2560×1600 pixels, drawn in 640 ms."
        ),
        "{one}"
    );
    assert!(one.contains("never the person's window"), "{one}");
    let two = said(&[
        painted("purlis-classic", Appearance::Dark),
        painted("purlis-classic", Appearance::Light),
    ]);
    assert!(
        two.starts_with(
            "Two theme specimens, in the order of their pictures: purlis-classic (dark)"
        ),
        "{two}"
    );
    assert!(two.contains("then purlis-classic (light)"), "{two}");
}

#[test]
fn a_picture_never_travels_inside_the_answer_s_json() {
    let painted = Painted {
        theme: "purlis-classic".to_owned(),
        appearance: Appearance::Light,
        high_contrast: false,
        preview: false,
        width: 1,
        height: 1,
        millis: 1,
        png: vec![0x89, b'P', b'N', b'G'],
    };
    let wire = serde_json::to_value(&painted).expect("JSON");
    assert_eq!(
        wire,
        json!({"theme": "purlis-classic", "appearance": "light", "width": 1, "height": 1, "millis": 1})
    );
}

// ---- Theme folders, option values, drafts and fonts where the chat works (#2536) ----

use std::path::{Path, PathBuf};

use crate::theme::gallery::Shelf;
use crate::theme::levels::{self, Reach};
use crate::theme::{Choice, Folders, LevelInputs, Levels, Offered, On, THEME_JSON};

fn put(themes: &Path, folder: &str, document: &Value) {
    let dir = themes.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(THEME_JSON), document.to_string()).unwrap();
}

/// A theme over Purlis Classic's dark and light appearances whose dark base surface is
/// `surface`, and an option `accent` with two values.
fn dusk(surface: &str) -> Value {
    json!({
        "name": "Dusk",
        "extends": "purlis-classic",
        "appearances": [
            { "id": "dark", "appearance": "dark", "tokens": { "surface.base": surface } },
            { "id": "light", "appearance": "light" }
        ],
        "options": [{
            "id": "accent", "kind": "choice", "label": "Accent", "default": "lavender",
            "values": [
                { "id": "lavender", "label": "Lavender", "tokens": { "accent.base": "#b4a7f5" } },
                { "id": "teal", "label": "Teal", "tokens": { "accent.base": "#2bb3a3" } }
            ]
        }]
    })
}

fn woff2(body: &str) -> Vec<u8> {
    let mut bytes = b"wOF2".to_vec();
    bytes.extend_from_slice(body.as_bytes());
    bytes
}

/// A theme folder `folder` in `themes` whose terminal font is one face of `face`.
fn put_with_a_face(themes: &Path, folder: &str, face: &[u8]) {
    let mut document = dusk("#101010");
    document["fonts"] = json!({ "terminal": { "family": "Plex Mono", "faces": [
        { "file": "fonts/PlexMono.woff2" }
    ] } });
    put(themes, folder, &document);
    std::fs::create_dir_all(themes.join(folder).join("fonts")).unwrap();
    std::fs::write(themes.join(folder).join("fonts/PlexMono.woff2"), face).unwrap();
}

/// A machine config home, and a plane with the workspaces `alpha` and `beta`.
struct Places {
    _home: tempfile::TempDir,
    config: PathBuf,
    plane: PathBuf,
}

impl Places {
    fn new() -> Self {
        Self::inside("")
    }

    /// The config home and the plane inside the folder `folder` of a temporary one.
    fn inside(folder: &str) -> Self {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join(folder).join("config");
        let plane = home.path().join(folder).join("plane");
        std::fs::create_dir_all(plane.join(".purlis")).unwrap();
        std::fs::create_dir_all(plane.join("workspaces/alpha")).unwrap();
        std::fs::create_dir_all(plane.join("workspaces/beta")).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        Self {
            _home: home,
            config,
            plane,
        }
    }

    fn machine(&self) -> PathBuf {
        crate::machine::dir(&self.config).join(levels::THEMES)
    }

    fn project(&self) -> PathBuf {
        self.plane.join(levels::THEMES)
    }

    fn workspace(&self, name: &str) -> PathBuf {
        self.plane
            .join("workspaces")
            .join(name)
            .join(levels::THEMES)
    }

    fn folders(&self, workspace: Option<&str>) -> Folders {
        Folders::of(Some(&self.config), Some(&self.plane), workspace)
    }
}

/// What a chat `chat` working at `workspace` of `places` reaches, handed to `ask`.
fn at<T>(
    places: &Places,
    workspace: Option<&str>,
    chat: Option<&str>,
    ask: impl FnOnce(&Reached<'_>) -> T,
) -> T {
    let folders = places.folders(workspace);
    let offered: [Offered; 0] = [];
    let reached = Reached {
        config: Some(&places.config),
        plane: Some(&places.plane),
        workspace,
        chat,
        shelf: Shelf {
            folders: &folders,
            reach: if workspace.is_some() {
                Reach::Workspace
            } else {
                Reach::Project
            },
            on: On::Every,
            offered: &offered,
        },
    };
    ask(&reached)
}

fn named_specimen(
    theme: &str,
    appearance: Option<Appearance>,
    options: &[(&str, &str)],
) -> Specimen {
    Specimen {
        theme: theme.to_owned(),
        appearance,
        high_contrast: false,
        options: options
            .iter()
            .map(|(id, value)| ((*id).to_owned(), (*value).to_owned()))
            .collect(),
    }
}

fn base_of(drawn: &theme::Drawn) -> String {
    drawn
        .theme
        .as_ref()
        .unwrap()
        .value("surface.base")
        .to_owned()
}

#[test]
fn a_theme_folder_is_drawn_from_every_level_the_chat_s_workspace_reaches() {
    let places = Places::new();
    put(&places.machine(), "mine", &dusk("#111111"));
    put(&places.project(), "team", &dusk("#121212"));
    put(&places.workspace("alpha"), "client", &dusk("#131313"));
    // One name at two levels: the nearest by its name alone, each by its level.
    put(&places.project(), "both", &dusk("#141414"));
    put(&places.workspace("alpha"), "both", &dusk("#151515"));
    let cases = [
        ("mine", "#111111"),
        ("machine:mine", "#111111"),
        ("team", "#121212"),
        ("project:team", "#121212"),
        ("client", "#131313"),
        ("workspace:client", "#131313"),
        ("workspace:alpha:client", "#131313"),
        ("both", "#151515"),
        ("project:both", "#141414"),
        ("workspace:both", "#151515"),
    ];
    at(&places, Some("alpha"), None, |reached| {
        for (id, base) in cases {
            assert!(asked(&args(json!({ "theme": id }))).is_ok(), "{id}");
            let drawn = resolved(reached, &named_specimen(id, None, &[])).expect(id);
            assert_eq!(base_of(&drawn), base, "{id}");
        }
    });
}

#[test]
fn a_theme_beyond_the_chat_s_reach_is_refused() {
    let places = Places::new();
    put(&places.workspace("beta"), "other", &dusk("#131313"));
    put(&places.workspace("alpha"), "client", &dusk("#131313"));
    at(&places, Some("alpha"), None, |reached| {
        // Another workspace's folder, by its name or by its level and workspace.
        let why = resolved(reached, &named_specimen("other", None, &[])).expect_err("not here");
        assert!(
            why.contains("no theme `other` within this chat's reach"),
            "{why}"
        );
        let why = resolved(reached, &named_specimen("workspace:beta:other", None, &[]))
            .expect_err("another workspace's");
        assert!(why.contains("beyond this chat's reach"), "{why}");
        assert!(why.contains("in the workspace alpha"), "{why}");
    });
    // At the plane root no workspace's folder is within reach.
    at(&places, None, None, |reached| {
        for id in ["workspace:client", "workspace:alpha:client"] {
            let why = resolved(reached, &named_specimen(id, None, &[])).expect_err(id);
            assert!(why.contains("beyond this chat's reach"), "{why}");
            assert!(why.contains("at the plane root"), "{why}");
        }
        let why = resolved(reached, &named_specimen("client", None, &[])).expect_err("client");
        assert!(why.contains("no theme `client`"), "{why}");
    });
    // With no plane, only the built-ins.
    let why = resolved(
        &Reached::nowhere(),
        &named_specimen("project:team", None, &[]),
    )
    .expect_err("no project");
    assert!(why.contains("beyond this chat's reach"), "{why}");
}

#[test]
fn a_specimen_draws_what_the_window_draws_for_the_same_choice_at_the_chat_s_place() {
    let places = Places::new();
    put(&places.workspace("alpha"), "dusk", &dusk("#202028"));
    let folders = places.folders(Some("alpha"));
    for (appearance, id, values) in [
        (None, None, vec![]),
        (
            Some(Appearance::Light),
            Some("light"),
            vec![("accent", "teal")],
        ),
        (
            Some(Appearance::Dark),
            Some("dark"),
            vec![("accent", "lavender")],
        ),
    ] {
        let options: Values = values
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect();
        // The window: the person's choice for this workspace, with these values.
        let levels = Levels {
            workspace_choice: Some(
                Choice::new("dusk", id)
                    .unwrap()
                    .at(Reach::Workspace)
                    .with_options(options.clone()),
            ),
            ..Levels::default()
        };
        let window = theme::resolve_levels(
            &LevelInputs {
                levels: &levels,
                folders: &folders,
                theirs: None,
                on: On::Every,
                system: Appearance::Dark,
                contrast: theme::Contrast::Standard,
            },
            || std::sync::Arc::from(Vec::new()),
        );
        let specimen = at(&places, Some("alpha"), None, |reached| {
            resolved(reached, &named_specimen("dusk", appearance, &values)).expect("drawn")
        });
        assert_eq!(specimen.theme, window.theme, "{appearance:?} {values:?}");
        assert_eq!(specimen.held, window.held);
    }
}

#[test]
fn option_values_are_drawn_and_one_the_theme_lacks_is_refused() {
    let places = Places::new();
    put(&places.project(), "dusk", &dusk("#202028"));
    at(&places, None, None, |reached| {
        let accent = |values: &[(&str, &str)]| {
            resolved(reached, &named_specimen("dusk", None, values))
                .map(|drawn| drawn.theme.unwrap().value("accent.base").to_owned())
        };
        assert_eq!(accent(&[]).as_deref(), Ok("#b4a7f5"));
        assert_eq!(accent(&[("accent", "teal")]).as_deref(), Ok("#2bb3a3"));
        let why = accent(&[("borders", "on")]).expect_err("no such option");
        assert!(why.contains("has no option `borders`"), "{why}");
        assert!(why.contains("accent (lavender or teal)"), "{why}");
        let why = accent(&[("accent", "pink")]).expect_err("no such value");
        assert!(why.contains("has no value `pink`"), "{why}");
        // Purlis Classic has no options to give.
        let why = resolved(
            reached,
            &named_specimen("purlis-classic", None, &[("accent", "teal")]),
        )
        .expect_err("none");
        assert!(why.contains("it has none"), "{why}");
        // An appearance it lacks is refused, naming the ones it has.
        put(
            &places.project(),
            "night-only",
            &json!({
                "name": "Night",
                "appearances": [{ "id": "night", "appearance": "dark" }]
            }),
        );
    });
    at(&places, None, None, |reached| {
        let why = resolved(
            reached,
            &named_specimen("night-only", Some(Appearance::Light), &[]),
        )
        .expect_err("no light");
        assert!(
            why.contains("has no light appearance: it has night (dark)"),
            "{why}"
        );
    });
}

#[test]
fn option_values_are_asked_for_by_id_and_kept_by_a_second_specimen_that_does_not_change_them() {
    assert_eq!(
        asked(&args(json!({
            "theme": "draft:dusk",
            "options": { "accent": "teal" },
            "side_by_side": { "theme": "dusk" },
        }))),
        Ok(vec![
            named_specimen("draft:dusk", None, &[("accent", "teal")]),
            named_specimen("dusk", None, &[("accent", "teal")]),
        ])
    );
    assert_eq!(
        asked(&args(json!({
            "theme": "dusk",
            "options": { "accent": "teal" },
            "side_by_side": { "options": {} },
        }))),
        Ok(vec![
            named_specimen("dusk", None, &[("accent", "teal")]),
            named_specimen("dusk", None, &[]),
        ])
    );
    for (bad, said) in [
        (
            json!({"theme": "dusk", "options": ["teal"]}),
            "is an object",
        ),
        (
            json!({"theme": "dusk", "options": {"accent": 3}}),
            "as text",
        ),
        (
            json!({"theme": "dusk", "side_by_side": {"theme": "x", "path": "/"}}),
            "not `path`",
        ),
    ] {
        let why = asked(&args(bad)).expect_err("refused");
        assert!(why.contains(said), "{why}");
    }
    let many: serde_json::Map<String, Value> = (0..=MOST_OPTIONS)
        .map(|n| (format!("o{n}"), json!("on")))
        .collect();
    let why = asked(&args(json!({"theme": "dusk", "options": many}))).expect_err("too many");
    assert!(why.contains("at most 16"), "{why}");
}

/// Two chats' ids, as the app mints them.
fn two_chats() -> (String, String) {
    (crate::reopen::mint(), crate::reopen::mint())
}

#[test]
fn a_chat_s_own_draft_is_drawn_beside_the_theme_as_kept_and_no_other_chat_s() {
    let places = Places::new();
    put(&places.workspace("alpha"), "dusk", &dusk("#202028"));
    let (mine, theirs) = two_chats();
    let place = drafts::Place {
        plane: &places.plane,
        workspace: Some("alpha"),
    };
    let draft = drafts::start(Some(&places.config), place, &mine, "dusk")
        .unwrap()
        .path;
    put(draft.parent().unwrap(), "dusk", &dusk("#18181f"));
    at(&places, Some("alpha"), Some(&mine), |reached| {
        let draft = resolved(reached, &named_specimen("draft:dusk", None, &[])).expect("draft");
        let kept = resolved(reached, &named_specimen("dusk", None, &[])).expect("kept");
        assert_eq!(base_of(&draft), "#18181f");
        assert_eq!(base_of(&kept), "#202028");
    });
    // Another chat names the same draft: its own drafts are read, and it has none.
    at(&places, Some("alpha"), Some(&theirs), |reached| {
        let why =
            resolved(reached, &named_specimen("draft:dusk", None, &[])).expect_err("not its draft");
        assert!(why.contains("this chat has no theme draft dusk"), "{why}");
    });
    // A chat this purlis has no id for has no drafts.
    at(&places, Some("alpha"), None, |reached| {
        let why =
            resolved(reached, &named_specimen("draft:dusk", None, &[])).expect_err("no drafts");
        assert!(why.contains("keeps no theme drafts for this chat"), "{why}");
    });
    // And one working in another workspace never draws it either: its own folder is read.
    at(&places, Some("beta"), Some(&theirs), |reached| {
        assert!(resolved(reached, &named_specimen("draft:dusk", None, &[])).is_err());
    });
}

#[test]
fn a_draft_that_breaks_a_hard_floor_is_not_drawn_and_the_chat_is_told_which() {
    let places = Places::new();
    let (mine, _) = two_chats();
    let place = drafts::Place {
        plane: &places.plane,
        workspace: None,
    };
    let draft = drafts::start(Some(&places.config), place, &mine, "glare")
        .unwrap()
        .path;
    put(
        draft.parent().unwrap(),
        "glare",
        &json!({
            "name": "Glare", "extends": "purlis-classic",
            "appearances": [{ "id": "dark", "appearance": "dark",
                "tokens": { "surface.sunken": "#f2f2f2" } }]
        }),
    );
    at(&places, None, Some(&mine), |reached| {
        let why = resolved(reached, &named_specimen("draft:glare", None, &[])).expect_err("broken");
        assert!(why.contains("breaks a hard floor"), "{why}");
        assert!(why.contains("text.primary on surface.sunken"), "{why}");
    });
}

#[test]
fn a_project_s_font_the_approval_does_not_hold_is_not_drawn_and_the_specimen_says_so() {
    let places = Places::new();
    put_with_a_face(&places.project(), "typed", &woff2("team face, #2536"));
    at(&places, Some("alpha"), None, |reached| {
        let why = resolved(reached, &named_specimen("typed", None, &[])).expect_err("waits");
        assert!(why.contains("themes/typed/fonts/PlexMono.woff2"), "{why}");
        assert!(
            why.contains("the project's approval does not hold yet"),
            "{why}"
        );
    });
}

#[test]
fn a_theme_s_own_fonts_are_handed_to_its_specimen_the_terminal_s_among_them() {
    let places = Places::new();
    let face = woff2("the person's own face, #2536");
    put_with_a_face(&places.machine(), "typed", &face);
    let drawn = at(&places, Some("alpha"), None, |reached| {
        resolved(reached, &named_specimen("typed", None, &[])).expect("the person's own")
    });
    let theme = drawn.theme.unwrap();
    let terminal = &theme.fonts.terminal;
    assert_eq!(terminal.faces.len(), 1);
    assert!(
        terminal.stack.starts_with("\"purlis-terminal-"),
        "{}",
        terminal.stack
    );
    let handed = faces(&theme).expect("handed");
    assert_eq!(handed, [(terminal.faces[0].asset.clone(), face)]);
    // Purlis brings its own faces: each handed once, the terminal's among them.
    let purlis = theme::purlis().first();
    let handed = faces(purlis).expect("Purlis's");
    for face in &purlis.fonts.terminal.faces {
        assert!(handed.iter().any(|(asset, _)| *asset == face.asset));
    }
    let mut assets: Vec<&String> = handed.iter().map(|(asset, _)| asset).collect();
    assets.dedup();
    assert_eq!(assets.len(), handed.len());
    // Purlis Classic brings none.
    assert!(faces(theme::classic().first()).unwrap().is_empty());
}

#[test]
fn the_schema_takes_option_values_and_still_no_path() {
    let schema = (TOOLS[0].schema)();
    assert_eq!(schema["properties"]["options"]["type"], "object");
    assert_eq!(
        schema["properties"]["side_by_side"]["properties"]["options"]["maxProperties"],
        MOST_OPTIONS
    );
    // An ask written before option values reads the same, and one with them carries them.
    let old: Specimen = serde_json::from_value(json!({"theme": "purlis-classic"})).unwrap();
    assert_eq!(old, specimen("purlis-classic", None));
    assert_eq!(
        serde_json::to_value(specimen("purlis-classic", None)).unwrap(),
        json!({"theme": "purlis-classic"})
    );
}

// ---- High contrast (#2709), merged with #2536 ----

fn sharp(theme: &str) -> Specimen {
    Specimen {
        theme: theme.to_owned(),
        appearance: Some(Appearance::Dark),
        high_contrast: true,
        options: Values::new(),
    }
}

#[test]
fn purlis_is_drawn_in_its_high_contrast_dark_when_that_is_asked_for() {
    // #2709: the tool draws every appearance Purlis has.
    assert_eq!(
        asked(&args(
            json!({"theme": "purlis", "appearance": "dark-high-contrast"})
        )),
        Ok(vec![sharp("purlis")])
    );
    let theme = chosen(&sharp("purlis")).expect("its high-contrast dark");
    assert_eq!(
        (theme.name.as_str(), theme.id.as_str()),
        ("Purlis", "dark-high-contrast")
    );
    assert!(theme.high_contrast);
    assert_eq!(
        resolved(&Reached::nowhere(), &sharp("purlis"))
            .expect("drawn")
            .held,
        None
    );
    // Its plain dark stays the one tagged dark and not high contrast.
    let dark = chosen(&specimen("purlis", Some(Appearance::Dark))).expect("dark");
    assert_eq!((dark.id.as_str(), dark.high_contrast), ("dark", false));
}

#[test]
fn a_theme_with_no_high_contrast_appearance_is_refused_naming_the_one_that_has_it() {
    // `purlis-dark` names Purlis's dark, which has one (#3034).
    assert_eq!(
        chosen(&sharp("purlis-dark")).map(|one| one.id.as_str()),
        Ok("dark-high-contrast")
    );
    for theme in ["purlis-classic", "purlis-light"] {
        let why = chosen(&sharp(theme)).expect_err("refused");
        assert!(why.contains("no high-contrast appearance"), "{why}");
        assert!(
            why.contains("purlis with appearance `dark-high-contrast`"),
            "{why}"
        );
    }
}

#[test]
fn side_by_side_compares_purlis_s_dark_with_its_high_contrast_dark() {
    assert_eq!(
        asked(&args(json!({
            "theme": "purlis",
            "appearance": "dark",
            "side_by_side": {"appearance": "dark-high-contrast"},
        }))),
        Ok(vec![
            specimen("purlis", Some(Appearance::Dark)),
            sharp("purlis")
        ])
    );
    // A second specimen that changes only the theme keeps the first's appearance, high contrast
    // and all, and is refused when that theme has none.
    let why = asked(&args(json!({
        "theme": "purlis",
        "appearance": "dark-high-contrast",
        "side_by_side": {"theme": "purlis-classic"},
    })))
    .expect_err("refused");
    assert!(why.contains("no high-contrast appearance"), "{why}");
}

#[test]
fn the_tool_says_purlis_is_the_default_and_names_its_high_contrast_dark() {
    let schema = (TOOLS[0].schema)();
    assert_eq!(
        schema["properties"]["appearance"]["enum"],
        json!(["dark", "light", "dark-high-contrast"])
    );
    assert_eq!(
        schema["properties"]["side_by_side"]["properties"]["appearance"]["enum"],
        json!(["dark", "light", "dark-high-contrast"])
    );
    let theme = schema["properties"]["theme"]["description"]
        .as_str()
        .unwrap();
    assert!(theme.contains("purlis, Purlis, the default"), "{theme}");
    assert!(
        TOOLS[0]
            .description
            .contains("purlis is Purlis, the default theme")
    );
    assert!(!TOOLS[0].description.contains("preview theme"));
    // A specimen of a preview theme still says so, though no built-in ships as one now (#3034).
    let painted = Painted {
        theme: "purlis".to_owned(),
        appearance: Appearance::Dark,
        high_contrast: true,
        preview: true,
        width: 2560,
        height: 1600,
        millis: 640,
        png: Vec::new(),
    };
    let one = said(std::slice::from_ref(&painted));
    assert!(
        one.starts_with(
            "The theme specimen in purlis (a preview theme, dark-high-contrast), 2560×1600 pixels"
        ),
        "{one}"
    );
    assert_eq!(
        serde_json::to_value(&painted).expect("JSON"),
        json!({
            "theme": "purlis", "appearance": "dark", "high_contrast": true, "preview": true,
            "width": 2560, "height": 1600, "millis": 640
        })
    );
}

#[test]
fn an_ask_without_high_contrast_reads_as_it_did() {
    // The ask travels from the chat's tool to the app: one written before #2709 still reads.
    let old: Specimen =
        serde_json::from_value(json!({"theme": "purlis", "appearance": "dark"})).expect("read");
    assert_eq!(old, specimen("purlis", Some(Appearance::Dark)));
    assert_eq!(
        serde_json::to_value(&old).expect("JSON"),
        json!({"theme": "purlis", "appearance": "dark"})
    );
    assert_eq!(
        serde_json::to_value(sharp("purlis")).expect("JSON"),
        json!({"theme": "purlis", "appearance": "dark", "high_contrast": true})
    );
}

#[test]
fn a_theme_folder_s_high_contrast_dark_is_drawn_when_asked_and_never_its_plain_dark_instead() {
    let places = Places::new();
    put(
        &places.project(),
        "sharp",
        &json!({
            "name": "Sharp", "extends": "purlis-classic",
            "appearances": [
                { "id": "dark", "appearance": "dark", "tokens": { "surface.base": "#121212" } },
                { "id": "dark-hc", "appearance": "dark", "high-contrast": true,
                  "tokens": { "surface.base": "#000000" } }
            ]
        }),
    );
    put(&places.project(), "plain", &dusk("#121212"));
    let hc = |theme: &str| Specimen {
        high_contrast: true,
        ..named_specimen(theme, Some(Appearance::Dark), &[])
    };
    at(&places, None, None, |reached| {
        let drawn = resolved(reached, &hc("sharp")).expect("its high-contrast dark");
        assert_eq!(base_of(&drawn), "#000000");
        assert!(drawn.theme.as_ref().unwrap().high_contrast);
        let plain = resolved(
            reached,
            &named_specimen("sharp", Some(Appearance::Dark), &[]),
        )
        .expect("its plain dark");
        assert_eq!(base_of(&plain), "#121212");
        let why = resolved(reached, &hc("plain")).expect_err("none");
        assert!(
            why.contains("has no dark-high-contrast appearance"),
            "{why}"
        );
    });
    // Side by side: the kept theme's plain dark beside its high-contrast dark, options kept.
    assert_eq!(
        asked(&args(json!({
            "theme": "sharp",
            "appearance": "dark",
            "options": {"accent": "teal"},
            "side_by_side": {"appearance": "dark-high-contrast"},
        }))),
        Ok(vec![
            named_specimen("sharp", Some(Appearance::Dark), &[("accent", "teal")]),
            Specimen {
                high_contrast: true,
                ..named_specimen("sharp", Some(Appearance::Dark), &[("accent", "teal")])
            },
        ])
    );
}

// ---- The #2742 review: bounds, refusal text, caps, reach ----

fn digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn a_line_that_skipped_the_tool_is_held_to_the_tool_s_bounds() {
    let many: Values = (0..=MOST_OPTIONS)
        .map(|n| (format!("o{n}"), "on".to_owned()))
        .collect();
    let cases = [
        (vec![], "side by side"),
        (vec![specimen("purlis", None); 3], "side by side"),
        (vec![specimen("/etc/passwd", None)], "is not a theme's id"),
        (vec![specimen(" purlis", None)], "give `theme`"),
        (
            vec![Specimen {
                options: many,
                ..specimen("dusk", None)
            }],
            "at most 16 option values",
        ),
        (
            vec![Specimen {
                options: [(String::new(), "on".to_owned())].into(),
                ..specimen("dusk", None)
            }],
            "names an option",
        ),
        (
            vec![Specimen {
                high_contrast: true,
                ..specimen("purlis", None)
            }],
            "with an appearance",
        ),
        (
            vec![specimen("purlis-light", Some(Appearance::Dark))],
            "only a light",
        ),
    ];
    for (specimens, said) in cases {
        let why = checked(&specimens).expect_err(said);
        assert!(why.contains(said), "{why}");
    }
    assert_eq!(
        checked(&[specimen("draft:dusk", None), sharp("purlis")]),
        Ok(())
    );
}

#[test]
fn a_refusal_says_only_what_is_wrong_with_the_theme_named_and_no_path() {
    let places = Places::new();
    // The person's own themes folder reached through a link: what reading that level says
    // (its absolute path among it) is no other ask's to be told.
    let elsewhere = tempfile::tempdir().unwrap();
    put(elsewhere.path(), "secret-client", &dusk("#101010"));
    std::fs::create_dir_all(places.machine().parent().unwrap()).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(elsewhere.path(), places.machine()).unwrap();
    put(
        &places.project(),
        "broken",
        &json!({ "name": 3, "appearances": "x" }),
    );
    let noise = levels_said(places.folders(Some("alpha")).find("", Reach::Workspace));
    #[cfg(unix)]
    assert!(
        noise
            .iter()
            .any(|one| one.said.contains("your purlis config folder's themes")),
        "{noise:?}"
    );
    at(&places, Some("alpha"), None, |reached| {
        let why = resolved(reached, &named_specimen("broken", None, &[])).expect_err("refused");
        assert!(why.contains("`broken` is refused"), "{why}");
        assert!(why.contains("a theme has a name"), "{why}");
        assert!(
            !why.contains("private-notes") && !why.contains("secret-client"),
            "{why}"
        );
        let config = places.config.display().to_string();
        let plane = places.plane.display().to_string();
        assert!(!why.contains(&config) && !why.contains(&plane), "{why}");
    });
}

#[test]
fn a_workspace_s_font_the_approval_does_not_hold_is_never_served() {
    let places = Places::new();
    let face = woff2("a workspace's own face, #2742");
    put_with_a_face(&places.workspace("alpha"), "client", &face);
    at(&places, Some("alpha"), None, |reached| {
        let why = resolved(reached, &named_specimen("client", None, &[])).expect_err("waits");
        assert!(
            why.contains("workspaces/alpha/themes/client/fonts/PlexMono.woff2"),
            "{why}"
        );
    });
    assert!(theme::served(&digest(&face)).is_err());
    assert!(faces_of([digest(&face).as_str()]).is_err());
}

#[test]
fn an_extension_s_theme_is_drawn_only_while_its_extension_is_on_and_approved() {
    let places = Places::new();
    let text = json!({ "name": "Solar Dark", "tokens": { "surface.base": "#101010" } }).to_string();
    let offered = [Offered::of("solar".into(), "Solar Dark".into(), &text)];
    let folders = places.folders(Some("alpha"));
    let on = ["solar".to_owned()];
    let ask = |on: On<'_>, offered: &[Offered]| {
        let reached = Reached {
            config: Some(&places.config),
            plane: Some(&places.plane),
            workspace: Some("alpha"),
            chat: None,
            shelf: Shelf {
                folders: &folders,
                reach: Reach::Workspace,
                on,
                offered,
            },
        };
        resolved(&reached, &named_specimen("solar/Solar Dark", None, &[]))
    };
    assert!(ask(On::Only(&on), &offered).is_ok(), "on and approved");
    // Off in this project, or not approved (the survey offers only approved ones), or not there.
    for (on, offered) in [(On::Only(&[]), &offered[..]), (On::Every, &[][..])] {
        let why = ask(on, offered).expect_err("refused");
        assert!(
            why.contains("no theme `solar/Solar Dark` within this chat's reach"),
            "{why}"
        );
    }
    let why = ask(On::Every, &offered[..0]).expect_err("missing");
    assert!(why.contains("within this chat's reach"), "{why}");
}

#[test]
fn faces_past_the_cap_are_not_handed_and_the_specimen_says_so() {
    let places = Places::new();
    let folder = places.machine().join("heavy");
    std::fs::create_dir_all(folder.join("fonts")).unwrap();
    let size = 2_000_000;
    let mut listed = Vec::new();
    for n in 0..9u8 {
        let mut bytes = b"wOF2".to_vec();
        bytes.resize(size, n);
        std::fs::write(folder.join(format!("fonts/F{n}.woff2")), &bytes).unwrap();
        listed.push(
            json!({ "file": format!("fonts/F{n}.woff2"), "weight": 100 + u16::from(n % 8) * 100 }),
        );
    }
    let mut document = dusk("#101010");
    document["fonts"] = json!({
        "ui": { "family": "Heavy", "faces": listed[..8] },
        "mono": { "family": "Heavy Mono", "faces": [listed[8]] },
    });
    std::fs::write(folder.join(THEME_JSON), document.to_string()).unwrap();
    let drawn = at(&places, None, None, |reached| {
        resolved(reached, &named_specimen("heavy", None, &[])).expect("drawn")
    });
    let why = faces(&drawn.theme.unwrap()).expect_err("past the cap");
    assert!(why.contains("more than 16 MiB"), "{why}");
}

#[test]
fn the_tool_waits_for_the_whole_turn_the_lookup_included() {
    // A gallery picture's give-way, then the lookup, the font read and the tint, then each
    // picture: the tool is still waiting when the app's last answer can come.
    assert_eq!(TURN, LOOKUP + BUDGET * AT_MOST as u32);
    assert!(ASKED_WITHIN > BUDGET + TURN);
}

/// The words of the folder a test keeps the levels in, none of which a chat is told.
const AWKWARD_WORDS: [&str; 5] = ["Client", "Werk", "Bobz", "vintage", "kopie"];

#[cfg(unix)]
#[test]
fn a_face_handed_to_a_specimen_is_named_by_its_level_and_never_where_it_is() {
    let places = Places::inside("Client Werk 'Bobz' [vintage] (kopie)");
    let folder = places.machine().join("facey");
    let face = folder.join("fonts/Plex.woff2");
    std::fs::create_dir_all(face.parent().unwrap()).unwrap();
    std::fs::write(&face, woff2("the face as read")).unwrap();
    let mut document = dusk("#101010");
    document["fonts"] =
        json!({ "ui": { "family": "Plex", "faces": [{ "file": "fonts/Plex.woff2" }] } });
    std::fs::write(folder.join(THEME_JSON), document.to_string()).unwrap();
    let drawn = at(&places, None, None, |reached| {
        resolved(reached, &named_specimen("facey", None, &[])).expect("drawn")
    });
    let theme = drawn.theme.expect("a theme");
    assert_eq!(faces(&theme).map(|handed| handed.len()), Ok(1));
    let home = places._home.path().display().to_string();
    let told = |why: &str, says: &str| {
        assert!(why.contains(says), "{why}");
        assert!(!why.contains(&home), "{why}");
        for word in AWKWARD_WORDS {
            assert!(!why.contains(word), "{word}: {why}");
        }
    };
    // The face changes between the read and the hand-off: what the chat is told names it by
    // its level and its path there.
    std::fs::write(&face, woff2("the face changed")).unwrap();
    told(
        &faces(&theme).expect_err("changed"),
        "your purlis config folder's themes/facey/fonts/Plex.woff2 changed since purlis read the \
         theme",
    );
    // And when it is gone.
    std::fs::remove_file(&face).unwrap();
    told(
        &faces(&theme).expect_err("gone"),
        "your purlis config folder's themes/facey/fonts/Plex.woff2 is not in the theme's folder",
    );
}

#[cfg(unix)]
#[test]
fn an_outside_folder_s_path_is_never_told_to_a_chat_spaces_quotes_and_brackets_and_all() {
    // The levels sit in a folder of an awkward name, and a theme folder of the project's is a
    // link to one outside every level, in a folder of another.
    let places = Places::inside("Client Werk 'Bobz' [vintage] (kopie)");
    let outside = places._home.path().join("Elsewhere [x] 'y' (z)");
    put(&outside, "outer", &dusk("#101010"));
    std::fs::create_dir_all(places.project()).unwrap();
    std::os::unix::fs::symlink(outside.join("outer"), places.project().join("outer")).unwrap();
    std::fs::create_dir_all(places.project().join("broken")).unwrap();
    std::fs::write(
        places.project().join("broken").join(THEME_JSON),
        "{ not json",
    )
    .unwrap();
    let mut iconic = dusk("#101010");
    iconic["icons"] = Value::from("icons/gone.json");
    put(&places.machine(), "iconic", &iconic);
    let (chat, _) = two_chats();
    let place = drafts::Place {
        plane: &places.plane,
        workspace: Some("alpha"),
    };
    put(&places.workspace("alpha"), "dusk", &dusk("#202028"));
    let draft = drafts::start(Some(&places.config), place, &chat, "dusk")
        .unwrap()
        .path;
    std::fs::write(draft.join(THEME_JSON), "{ not json").unwrap();
    at(&places, Some("alpha"), Some(&chat), |reached| {
        for (id, names) in [
            (
                "outer",
                "the project's themes/outer is a link or not a folder",
            ),
            ("broken", "the project's themes/broken/theme.json: "),
            (
                "iconic",
                "your purlis config folder's themes/iconic has no icons/gone.json",
            ),
            ("draft:dusk", "the chat's theme draft dusk/theme.json: "),
        ] {
            let why = resolved(reached, &named_specimen(id, None, &[])).expect_err(id);
            // What the person opens to put it right is named, by its level and its path there.
            assert!(why.contains(names), "{why}");
            assert!(
                !why.contains(&places._home.path().display().to_string()),
                "{why}"
            );
            for word in [
                "Client",
                "Werk",
                "Bobz",
                "vintage",
                "kopie",
                "Elsewhere",
                "[x]",
                "(z)",
            ] {
                assert!(!why.contains(word), "{word}: {why}");
            }
        }
    });
}
