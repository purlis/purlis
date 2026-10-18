// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis theme list` and `purlis theme check` (#2321, #2518): every theme available on this
//! machine and in the plane, with its level and where it comes from, and the theme check of any
//! one, for a person and for a chat.
//!
//! Every run is against a temporary plane and a temporary config home, started unsteered
//! (`support::purlis`, which clears every spelling of the variables that choose a project,
//! #2292), so nothing here can read or write a real plane or a real machine's themes.

mod support;

use std::path::{Path, PathBuf};

use purlis_core::extension;

struct Rig {
    dir: tempfile::TempDir,
}

struct Ran {
    code: i32,
    out: String,
    err: String,
}

impl Ran {
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.out).unwrap_or_else(|why| panic!("{why}: {}", self.out))
    }
}

impl Rig {
    /// A plane with one workspace, and a config home with nothing in it.
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a directory");
        let plane = dir.path().join("plane");
        std::fs::create_dir_all(plane.join("workspaces/alpha")).expect("a workspace");
        std::fs::write(plane.join("charter.toml"), "schema = 1\n").expect("a manifest");
        std::fs::create_dir_all(dir.path().join("config")).expect("a config home");
        Self { dir }
    }

    /// The plane, as purlis finds it: through every link on the way (a temporary directory is
    /// reached through one on macOS).
    fn plane(&self) -> PathBuf {
        self.dir
            .path()
            .join("plane")
            .canonicalize()
            .expect("the plane")
    }

    fn config(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    /// The person's own theme file, holding `text`.
    fn mine(&self, text: &str) -> PathBuf {
        let path = purlis_core::windowprefs::theme_path(&self.config());
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the config folder");
        std::fs::write(&path, text).expect("a theme file");
        path
    }

    /// An installed, approved extension `id` whose one theme `name` is `text`.
    fn extension_theme(&self, id: &str, name: &str, text: &str) {
        let at = self.dir.path().join(format!("ext-{id}"));
        std::fs::create_dir_all(&at).expect("the extension's directory");
        std::fs::write(
            at.join(extension::MANIFEST),
            serde_json::json!({
                "version": 1,
                "id": id,
                "name": id,
                "contributes": { "themes": [{ "name": name, "file": "theme.json" }] }
            })
            .to_string(),
        )
        .expect("a manifest");
        std::fs::write(at.join("theme.json"), text).expect("a theme");
        let found = extension::install(&self.config(), &extension::BuiltIn::none(), &at)
            .expect("installed");
        extension::approve(&self.config(), found.id(), &found.path, &found.fingerprint)
            .expect("approved");
    }

    /// The person's own `themes/`, in purlis's config folder.
    fn machine_themes(&self) -> PathBuf {
        purlis_core::machine::dir(&self.config()).join("themes")
    }

    /// The project's `themes/`, at the plane root.
    fn project_themes(&self) -> PathBuf {
        self.plane().join("themes")
    }

    /// The workspace `name`'s `themes/`.
    fn workspace_themes(&self, name: &str) -> PathBuf {
        self.plane().join("workspaces").join(name).join("themes")
    }

    /// The theme folder `name` in the level's `themes`, its `theme.json` holding `document`.
    fn folder(&self, themes: &Path, name: &str, document: &serde_json::Value) -> PathBuf {
        let folder = themes.join(name);
        std::fs::create_dir_all(&folder).expect("a theme folder");
        std::fs::write(folder.join("theme.json"), document.to_string()).expect("a theme.json");
        folder
    }

    fn run(&self, args: &[&str]) -> Ran {
        let out = support::purlis()
            .arg("theme")
            .args(args)
            .current_dir(self.plane())
            .env("PURLIS_ROOT", self.plane())
            .env("PURLIS_CONFIG_HOME", self.config())
            .env("PURLIS_DATA_HOME", self.dir.path().join("data"))
            .output()
            .expect("the binary runs");
        Ran {
            code: out.status.code().unwrap_or(-1),
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }
}

/// A theme as the person writes one: a name, an appearance and the tokens it changes.
fn a_theme(name: &str, tokens: &serde_json::Value) -> String {
    serde_json::json!({ "name": name, "appearance": "dark", "tokens": tokens }).to_string()
}

/// Every file under `dir`, with its bytes, to tell that a command wrote nothing.
fn every_file(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut left = vec![dir.to_path_buf()];
    while let Some(at) = left.pop() {
        for entry in std::fs::read_dir(&at).expect("a directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                left.push(path);
            } else {
                found.push((path.clone(), std::fs::read(&path).unwrap_or_default()));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn purlis_classic_is_listed_with_its_appearances_and_where_it_comes_from() {
    let rig = Rig::new();

    let ran = rig.run(&["list"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        ran.out,
        "  ID              NAME            APPEARANCES  FROM\n\
         \x20 purlis-classic  Purlis Classic  dark, light  built-in\n"
    );
}

#[test]
fn the_person_s_own_theme_and_an_extension_s_are_listed_after_the_built_in() {
    let rig = Rig::new();
    let path = rig.mine(&a_theme("Late", &serde_json::json!({})));
    rig.extension_theme(
        "solarized",
        "Solarized Light",
        r#"{"name":"Solarized Light","appearance":"light","tokens":{}}"#,
    );

    let ran = rig.run(&["list"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    let lines: Vec<&str> = ran.out.lines().collect();
    assert_eq!(lines.len(), 4, "{}", ran.out);
    let words = |line: &str| {
        line.split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        words(lines[2]),
        ["mine", "Late", "dark", "machine", &path.display().to_string()]
    );
    assert_eq!(
        words(lines[3]),
        [
            "solarized/Solarized",
            "Light",
            "Solarized",
            "Light",
            "light",
            "extension",
            "solarized"
        ]
    );
}

#[test]
fn a_theme_that_is_refused_is_listed_as_refused_with_why() {
    let rig = Rig::new();
    rig.mine("this is not json");

    let ran = rig.run(&["list"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    let (_, refused) = ran.out.split_once("refused:\n").expect("a refused section");
    assert!(refused.starts_with("  mine ("), "{refused}");
    assert!(refused.contains("a theme is JSON"), "{refused}");
}

#[test]
fn the_listing_as_json_says_each_theme_s_id_name_appearances_and_source() {
    let rig = Rig::new();
    rig.mine("[]");
    rig.extension_theme("nord", "Nord", &a_theme("Nord", &serde_json::json!({})));

    let ran = rig.run(&["list", "--json"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(ran.json()["schema"], 1);
    let themes = ran.json()["themes"].clone();
    assert_eq!(
        themes[0],
        serde_json::json!({
            "id": "purlis-classic", "name": "Purlis Classic",
            "appearances": [
                { "id": "dark", "appearance": "dark", "high_contrast": false },
                { "id": "light", "appearance": "light", "high_contrast": false }
            ],
            "options": [], "combinations": 2, "level": "built-in",
            "from": { "kind": "built-in" }, "refused": null
        })
    );
    assert_eq!(themes[1]["id"], "mine");
    assert_eq!(themes[1]["from"]["kind"], "mine");
    assert_eq!(themes[1]["name"], serde_json::Value::Null);
    assert!(
        themes[1]["refused"]
            .as_str()
            .is_some_and(|why| why.contains("a theme is a JSON object")),
        "{themes}"
    );
    assert_eq!(
        themes[2],
        serde_json::json!({
            "id": "nord/Nord", "name": "Nord",
            "appearances": [{ "id": "dark", "appearance": "dark", "high_contrast": false }],
            "options": [], "combinations": 1, "level": "extension",
            "from": { "kind": "extension", "extension": "nord" }, "refused": null
        })
    );
}

#[test]
fn purlis_classic_checks_clean_in_both_appearances_and_exits_zero() {
    let rig = Rig::new();

    let ran = rig.run(&["check", "purlis-classic"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        ran.out,
        "Purlis Classic — built-in, dark, light\n\
         left out: nothing\n\
         warnings: none\n\
         floors held: none\n\
         hard floors: not checked by this purlis yet\n"
    );
}

#[test]
fn a_value_outside_its_type_refuses_the_theme_and_exits_one() {
    let rig = Rig::new();
    rig.mine(&a_theme(
        "Typo",
        &serde_json::json!({ "text.primary": "red" }),
    ));

    let ran = rig.run(&["check", "mine"]);

    assert_eq!(ran.code, 1, "{}{}", ran.out, ran.err);
    assert!(
        ran.out
            .contains("refused — the window draws Purlis in its place:\n"),
        "{}",
        ran.out
    );
    assert!(
        ran.out
            .contains("  text.primary is \"red\", which is not #rgb"),
        "{}",
        ran.out
    );
    let json = rig.run(&["check", "mine", "--json"]).json();
    assert_eq!(json["errors"][0]["token"], "text.primary");
    assert_eq!(json["theme"]["name"], serde_json::Value::Null);
    assert_eq!(json["clean"], false);
}

#[test]
fn a_name_purlis_does_not_have_is_left_out_as_a_note_and_exits_zero() {
    let rig = Rig::new();
    rig.mine(&a_theme(
        "Typo",
        &serde_json::json!({ "text.primry": "#ffffff" }),
    ));

    let ran = rig.run(&["check", "mine"]);

    assert_eq!(ran.code, 0, "{}{}", ran.out, ran.err);
    assert!(ran.out.contains("left out (1):\n"), "{}", ran.out);
    assert!(
        ran.out
            .contains("  text.primry is not a purlis token, so it is left out\n"),
        "{}",
        ran.out
    );
    let json = rig.run(&["check", "mine", "--json"]).json();
    assert_eq!(json["errors"], serde_json::json!([]));
    assert_eq!(json["notes"][0]["token"], "text.primry");
    assert_eq!(json["clean"], false, "a note is not clean");
}

#[test]
fn a_contrast_failure_is_a_warning_with_its_pair_and_ratio_and_exits_zero() {
    let rig = Rig::new();
    // Grey on near-black: #555555 on #181818 is 2.38:1, under body text's 4.5.
    let file = rig.dir.path().join("dim.json");
    std::fs::write(
        &file,
        a_theme(
            "Dim",
            &serde_json::json!({ "text.muted": "#555555", "surface.base": "#181818" }),
        ),
    )
    .expect("a theme file");

    let ran = rig.run(&["check", &file.display().to_string()]);

    assert_eq!(ran.code, 0, "a warning is the author's call: {}", ran.out);
    assert!(ran.out.contains("left out: nothing\n"), "{}", ran.out);
    // One line per pair: the theme as it is, then every workspace tint it fails in too.
    let line = ran
        .out
        .lines()
        .find(|line| line.starts_with("  text.muted on surface.base:"))
        .unwrap_or_else(|| panic!("{}", ran.out));
    assert_eq!(
        line,
        "  text.muted on surface.base: 2.38, under 4.5; in every workspace tint too, down to 2.38"
    );
    assert_eq!(
        ran.out.matches("text.muted on surface.base").count(),
        1,
        "{}",
        ran.out
    );
}

#[test]
fn a_theme_folder_is_checked_in_each_of_its_appearances() {
    let rig = Rig::new();
    let themes = rig.dir.path().join("themes");
    std::fs::create_dir_all(themes.join("dusk")).expect("a theme folder");
    std::fs::write(
        themes.join("dusk/theme.json"),
        serde_json::json!({
            "name": "Dusk",
            "extends": "purlis-classic",
            "palette": { "grey": "#555555" },
            "appearances": [
                { "id": "dark", "appearance": "dark", "tokens": { "text.muted": "grey" } },
                { "id": "light", "appearance": "light" }
            ]
        })
        .to_string(),
    )
    .expect("a theme.json");

    let ran = rig.run(&["check", &themes.join("dusk").display().to_string()]);

    assert_eq!(ran.code, 0, "{}{}", ran.out, ran.err);
    assert!(ran.out.starts_with("Dusk — "), "{}", ran.out);
    assert!(
        ran.out.contains("\n  dark: text.muted on surface.base: "),
        "a finding names its appearance when there is more than one: {}",
        ran.out
    );
    assert!(!ran.out.contains("light: "), "{}", ran.out);
    let json = rig
        .run(&[
            "check",
            &themes.join("dusk").display().to_string(),
            "--json",
        ])
        .json();
    assert_eq!(json["theme"]["id"], "dusk");
    assert_eq!(json["theme"]["from"]["kind"], "folder");
    assert_eq!(json["warnings"][0]["appearance"], "dark");
}

#[test]
fn a_theme_folder_extends_the_folder_beside_it_that_it_names() {
    let rig = Rig::new();
    let themes = rig.dir.path().join("themes");
    for (name, json) in [
        (
            "dusk",
            serde_json::json!({ "name": "Dusk", "extends": "purlis-classic",
                "appearances": [{ "id": "dark", "appearance": "dark" }] }),
        ),
        (
            "night",
            serde_json::json!({ "name": "Night", "extends": "dusk",
                "appearances": [{ "id": "dark", "appearance": "dark" }] }),
        ),
    ] {
        std::fs::create_dir_all(themes.join(name)).expect("a theme folder");
        std::fs::write(themes.join(name).join("theme.json"), json.to_string())
            .expect("a theme.json");
    }
    // A folder nothing extends, which is not a theme at all: never read, so it refuses nothing.
    std::fs::create_dir_all(themes.join("stray")).expect("a stray folder");
    std::fs::write(themes.join("stray/theme.json"), "{ not json").expect("a stray file");

    let ran = rig.run(&[
        "check",
        &themes.join("night").display().to_string(),
        "--json",
    ]);

    assert_eq!(ran.code, 0, "{}{}", ran.out, ran.err);
    assert_eq!(ran.json()["theme"]["name"], "Night");
    assert_eq!(ran.json()["refused"], serde_json::Value::Null);
}

#[test]
fn the_check_as_json_has_errors_notes_warnings_and_hard_floors_for_a_chat() {
    let rig = Rig::new();
    rig.extension_theme(
        "dim",
        "Dim",
        &a_theme(
            "Dim",
            &serde_json::json!({ "text.muted": "#555555", "surface.base": "#181818", "bogus": "#000" }),
        ),
    );

    let ran = rig.run(&["check", "dim/Dim", "--json"]);

    assert_eq!(ran.code, 0, "{}{}", ran.out, ran.err);
    let said = ran.json();
    assert_eq!(said["schema"], 1);
    assert_eq!(
        said["theme"],
        serde_json::json!({
            "id": "dim/Dim", "name": "Dim",
            "appearances": [{ "id": "dark", "appearance": "dark", "high_contrast": false }],
            "options": [],
            "combinations": 1,
            "level": "extension",
            "from": { "kind": "extension", "extension": "dim" }
        })
    );
    assert_eq!(said["refused"], serde_json::Value::Null);
    assert_eq!(said["errors"], serde_json::json!([]));
    assert_eq!(
        said["notes"],
        serde_json::json!([{ "said": "bogus is not a purlis token, so it is left out", "token": "bogus" }])
    );
    let first = &said["warnings"][0];
    assert_eq!(first["appearance"], "dark");
    assert_eq!(first["kind"], "contrast");
    assert_eq!(first["tint"], serde_json::Value::Null);
    assert_eq!(first["pair"]["front"], "text.muted");
    assert_eq!(first["pair"]["back"], "surface.base");
    assert_eq!(first["pair"]["floor"], 4.5);
    assert_eq!(first["ratio"], 2.38);
    assert_eq!(said["held"], serde_json::json!([]), "no consent floor is broken");
    assert_eq!(
        said["hard_floors"],
        serde_json::Value::Null,
        "not checked is not a pass"
    );
    assert_eq!(said["clean"], false);
}

#[test]
fn a_theme_that_is_not_json_is_refused_and_exits_one() {
    let rig = Rig::new();
    rig.mine("{ not json");

    let ran = rig.run(&["check", "mine"]);

    assert_eq!(ran.code, 1, "{}", ran.out);
    assert!(ran.out.contains("a theme is JSON"), "{}", ran.out);
    assert!(
        ran.out.contains("the window draws Purlis in its place"),
        "{}",
        ran.out
    );
    let json = rig.run(&["check", "mine", "--json"]).json();
    assert!(
        json["refused"]
            .as_str()
            .is_some_and(|why| why.contains("a theme is JSON"))
    );
    assert_eq!(json["clean"], false);
}

#[test]
fn a_theme_that_is_not_here_is_said_and_exits_two() {
    let rig = Rig::new();

    let ran = rig.run(&["check", "nowhere"]);

    assert_eq!(ran.code, 2);
    assert_eq!(ran.out, "", "nothing for a reader of the answer");
    assert!(ran.err.contains("no theme 'nowhere'"), "{}", ran.err);
    assert!(ran.err.contains("purlis theme list"), "{}", ran.err);
}

#[test]
fn an_old_id_checks_purlis_classic() {
    let rig = Rig::new();

    for old in ["charter-dark", "purlis-light"] {
        let ran = rig.run(&["check", old, "--json"]);

        assert_eq!(ran.code, 0, "{}", ran.err);
        assert_eq!(ran.json()["theme"]["id"], "purlis-classic", "{old}");
        assert_eq!(ran.json()["clean"], true, "{old}");
    }
}

#[test]
fn listing_and_checking_write_nothing_in_the_plane_or_the_config_home() {
    let rig = Rig::new();
    rig.mine(&a_theme("Late", &serde_json::json!({})));
    rig.extension_theme("nord", "Nord", &a_theme("Nord", &serde_json::json!({})));
    let before = every_file(rig.dir.path());

    rig.run(&["list"]);
    rig.run(&["list", "--json"]);
    rig.run(&["check", "mine"]);
    rig.run(&["check", "nord/Nord", "--json"]);

    assert_eq!(every_file(rig.dir.path()), before);
}

/// A theme folder `dusk` with a dark and a light appearance and `options`.
fn options_folder(rig: &Rig, options: &serde_json::Value) -> PathBuf {
    let folder = rig.dir.path().join("themes/dusk");
    std::fs::create_dir_all(&folder).expect("a theme folder");
    std::fs::write(
        folder.join("theme.json"),
        serde_json::json!({
            "name": "Dusk",
            "appearances": [
                { "id": "dark", "appearance": "dark" },
                { "id": "light", "appearance": "light" }
            ],
            "options": options
        })
        .to_string(),
    )
    .expect("a theme.json");
    folder
}

#[test]
fn the_check_runs_every_appearance_and_option_combination_and_names_the_one_that_fails() {
    let rig = Rig::new();
    // Quiet text is drawn in a grey that fails on the dark window, and only when it is on.
    let folder = options_folder(
        &rig,
        &serde_json::json!([
            {
                "id": "quiet", "kind": "toggle", "label": "Quiet text", "default": false,
                "on": { "appearances": { "dark": { "tokens": { "text.muted": "#555555" } } } }
            },
            {
                "id": "accent", "kind": "choice", "label": "Accent", "default": "classic",
                "values": [
                    { "id": "classic", "label": "Classic" },
                    { "id": "same", "label": "Same" }
                ]
            }
        ]),
    );
    let at = folder.display().to_string();

    let ran = rig.run(&["check", &at]);

    assert_eq!(ran.code, 0, "{}{}", ran.out, ran.err);
    assert!(
        ran.out.contains(
            "options: quiet (a toggle, off by default); accent (one of classic, same; classic \
             by default) — 8 combinations checked\n"
        ),
        "{}",
        ran.out
    );
    // Found in both of the combinations quiet is on in, on the dark window, and in none other.
    let failing: Vec<&str> = ran
        .out
        .lines()
        .filter(|line| line.contains("text.muted on surface.base:"))
        .collect();
    assert_eq!(failing.len(), 2, "{}", ran.out);
    assert!(
        failing
            .iter()
            .all(|line| line.starts_with("  dark, quiet on, accent ")),
        "{}",
        ran.out
    );

    let json = rig.run(&["check", &at, "--json"]).json();
    assert_eq!(json["theme"]["combinations"], 8);
    assert_eq!(json["theme"]["options"][0]["id"], "quiet");
    assert_eq!(json["theme"]["options"][0]["kind"], "toggle");
    assert_eq!(json["theme"]["options"][1]["values"][1]["id"], "same");
    let first = &json["warnings"][0];
    assert_eq!(first["appearance"], "dark");
    assert_eq!(first["options"]["quiet"], "on");
    // In the theme's order of its options, not by name.
    let named: Vec<&str> = first["options"]
        .as_object()
        .expect("the option values")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(named, ["quiet", "accent"]);
}

#[test]
fn a_theme_past_64_combinations_is_refused_with_its_count_and_exits_one() {
    let rig = Rig::new();
    let choice = |id: &str| {
        serde_json::json!({
            "id": id, "kind": "choice", "label": id, "default": "a",
            "values": [
                { "id": "a", "label": "A" }, { "id": "b", "label": "B" },
                { "id": "c", "label": "C" }
            ]
        })
    };
    // Two appearances × 3 × 3 × 3 × 3 = 162.
    let folder = options_folder(
        &rig,
        &serde_json::json!([
            choice("one"),
            choice("two"),
            choice("three"),
            choice("four")
        ]),
    );

    let ran = rig.run(&["check", &folder.display().to_string()]);

    assert_eq!(ran.code, 1, "{}{}", ran.out, ran.err);
    assert!(
        ran.out.contains(
            "Dusk has 162 combinations of appearance and option values (2 appearances x 3 x 3 \
             x 3 x 3), and the theme check covers at most 64"
        ),
        "{}",
        ran.out
    );
}

#[test]
fn both_answers_name_the_theme_schema_of_this_purlis() {
    let rig = Rig::new();

    for args in [
        &["list", "--json"][..],
        &["check", "purlis-classic", "--json"],
    ] {
        let ran = rig.run(args);

        assert_eq!(ran.code, 0, "{}", ran.err);
        assert_eq!(
            ran.json()["theme_schema"],
            purlis_core::theme::SCHEMA_URL,
            "{args:?}"
        );
    }
    assert!(purlis_core::theme::SCHEMA_URL.contains(concat!("/v", env!("CARGO_PKG_VERSION"), "/")));
}

/// A one-appearance dark theme folder's `theme.json` called `name`, extending `extends`.
fn a_folder(name: &str, extends: Option<&str>, tokens: &serde_json::Value) -> serde_json::Value {
    let mut theme = serde_json::json!({
        "name": name,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    });
    if let Some(parent) = extends {
        theme["extends"] = serde_json::Value::from(parent);
    }
    theme
}

/// A plane with a theme folder at every level: the person's `base`, the project's `team`
/// (extending `base`), the workspace `alpha`'s `client` (extending `team`) and the workspace
/// `beta`'s `night`.
fn every_level(rig: &Rig) -> [PathBuf; 4] {
    let none = serde_json::json!({});
    std::fs::create_dir_all(rig.plane().join("workspaces/beta")).expect("a workspace");
    [
        rig.folder(&rig.machine_themes(), "base", &a_folder("Base", None, &none)),
        rig.folder(
            &rig.project_themes(),
            "team",
            &a_folder("Team", Some("base"), &none),
        ),
        rig.folder(
            &rig.workspace_themes("alpha"),
            "client",
            &a_folder("Client", Some("team"), &none),
        ),
        rig.folder(
            &rig.workspace_themes("beta"),
            "night",
            &a_folder("Night", None, &none),
        ),
    ]
}

#[test]
fn the_theme_folders_of_every_level_are_listed_with_their_level() {
    let rig = Rig::new();
    let [base, team, client, night] = every_level(&rig);
    rig.extension_theme("nord", "Nord", &a_theme("Nord", &serde_json::json!({})));

    let ran = rig.run(&["list"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    let rows: Vec<Vec<&str>> = ran
        .out
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().collect())
        .collect();
    let shown = |path: &PathBuf| path.display().to_string();
    assert_eq!(
        rows,
        [
            vec!["purlis-classic", "Purlis", "Classic", "dark,", "light", "built-in"],
            vec!["machine:base", "Base", "dark", "machine", &shown(&base)],
            vec!["project:team", "Team", "dark", "project", &shown(&team)],
            vec![
                "workspace:alpha:client",
                "Client",
                "dark",
                "workspace",
                "alpha",
                &shown(&client)
            ],
            vec![
                "workspace:beta:night",
                "Night",
                "dark",
                "workspace",
                "beta",
                &shown(&night)
            ],
            vec!["nord/Nord", "Nord", "dark", "extension", "nord"],
        ],
        "{}",
        ran.out
    );

    let json = rig.run(&["list", "--json"]).json();
    let themes = json["themes"].as_array().expect("the themes").clone();
    let levels: Vec<(&str, &str)> = themes
        .iter()
        .map(|one| {
            (
                one["id"].as_str().unwrap_or_default(),
                one["level"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        levels,
        [
            ("purlis-classic", "built-in"),
            ("machine:base", "machine"),
            ("project:team", "project"),
            ("workspace:alpha:client", "workspace"),
            ("workspace:beta:night", "workspace"),
            ("nord/Nord", "extension"),
        ]
    );
    assert_eq!(
        themes[3]["from"],
        serde_json::json!({
            "kind": "workspace", "workspace": "alpha", "folder": "client", "path": shown(&client)
        })
    );
    assert_eq!(
        themes[2]["from"],
        serde_json::json!({ "kind": "project", "folder": "team", "path": shown(&team) })
    );
    assert_eq!(themes[3]["refused"], serde_json::Value::Null, "{themes:?}");
}

#[test]
fn outside_a_plane_only_the_machine_s_theme_folders_are_listed() {
    let rig = Rig::new();
    every_level(&rig);
    let elsewhere = rig.dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("a directory");

    let out = support::purlis()
        .args(["theme", "list", "--json"])
        .current_dir(&elsewhere)
        .env("PURLIS_CONFIG_HOME", rig.config())
        .env("PURLIS_DATA_HOME", rig.dir.path().join("data"))
        .output()
        .expect("the binary runs");

    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("the listing as JSON");
    let ids: Vec<&str> = json["themes"]
        .as_array()
        .expect("the themes")
        .iter()
        .filter_map(|one| one["id"].as_str())
        .collect();
    assert_eq!(ids, ["purlis-classic", "machine:base"]);
}

#[test]
fn a_level_s_theme_folder_is_checked_by_its_level_and_name_as_the_window_reads_it() {
    let rig = Rig::new();
    every_level(&rig);

    let ran = rig.run(&["check", "workspace:alpha:client", "--json"]);

    assert_eq!(ran.code, 0, "{}{}", ran.out, ran.err);
    let said = ran.json();
    assert_eq!(said["theme"]["id"], "workspace:alpha:client");
    assert_eq!(said["theme"]["name"], "Client");
    assert_eq!(said["theme"]["level"], "workspace");
    assert_eq!(said["theme"]["from"]["workspace"], "alpha");
    assert_eq!(said["refused"], serde_json::Value::Null, "{said}");
    assert_eq!(said["clean"], true, "{said}");

    let text = rig.run(&["check", "project:team"]);
    assert_eq!(text.code, 0, "{}{}", text.out, text.err);
    assert!(
        text.out.starts_with(&format!(
            "Team — project {}, dark\n",
            rig.project_themes().join("team").display()
        )),
        "{}",
        text.out
    );
}

#[test]
fn a_level_s_folder_extends_only_its_own_level_and_the_levels_farther_out() {
    let rig = Rig::new();
    let none = serde_json::json!({});
    // The project's folder names a parent only the workspace has: a choice made for the project
    // never reads a workspace's folder, so the window refuses it, and so does the check.
    rig.folder(
        &rig.project_themes(),
        "team",
        &a_folder("Team", Some("client"), &none),
    );
    rig.folder(
        &rig.workspace_themes("alpha"),
        "client",
        &a_folder("Client", None, &none),
    );

    let ran = rig.run(&["check", "project:team"]);

    assert_eq!(ran.code, 1, "{}{}", ran.out, ran.err);
    assert!(
        ran.out
            .contains("refused — the window draws Purlis in its place when it is chosen:\n"),
        "{}",
        ran.out
    );
}

#[test]
fn a_level_without_the_folder_named_is_not_read_in_a_farther_level_s_place() {
    let rig = Rig::new();
    every_level(&rig);

    for asked in ["project:base", "workspace:alpha:team", "machine:team", "workspace:gamma:x"] {
        let ran = rig.run(&["check", asked]);

        assert_eq!(ran.code, 2, "{asked}: {}{}", ran.out, ran.err);
        assert_eq!(ran.out, "", "{asked}");
        assert!(ran.err.contains("project:<folder>"), "{}", ran.err);
    }
}

#[test]
fn the_check_says_the_consent_floors_the_window_holds_and_is_not_clean() {
    let rig = Rig::new();
    // Danger in the accent's own colour: not told apart from primary, so the window draws
    // purlis's own danger colour.
    let accent = purlis_core::theme::built_in_for(purlis_core::theme::Appearance::Dark)
        .value("accent.base")
        .to_owned();
    rig.folder(
        &rig.project_themes(),
        "same",
        &a_folder("Same", None, &serde_json::json!({ "danger.base": accent })),
    );

    let ran = rig.run(&["check", "project:same"]);

    assert_eq!(ran.code, 0, "a held floor is drawn: {}{}", ran.out, ran.err);
    let (_, held) = ran
        .out
        .split_once("floors held (1):\n")
        .unwrap_or_else(|| panic!("{}", ran.out));
    assert!(
        held.starts_with("  danger is distinguishable from primary — purlis draws its own ")
            && held.lines().next().is_some_and(|line| line.contains("danger.base")),
        "{held}"
    );

    let json = rig.run(&["check", "project:same", "--json"]).json();
    let first = &json["held"][0];
    assert_eq!(first["appearance"], "dark");
    assert!(
        first["floors"]
            .as_array()
            .is_some_and(|floors| floors.contains(&serde_json::json!("danger-apart"))),
        "{first}"
    );
    assert!(
        first["tokens"]
            .as_array()
            .is_some_and(|tokens| tokens.contains(&serde_json::json!("danger.base"))),
        "{first}"
    );
    assert_eq!(json["clean"], false, "a held floor is not clean");
}

#[test]
fn a_refused_level_folder_is_listed_as_refused_and_its_check_exits_one() {
    let rig = Rig::new();
    rig.folder(
        &rig.workspace_themes("alpha"),
        "typo",
        &a_folder("Typo", None, &serde_json::json!({ "text.primary": "red" })),
    );

    let ran = rig.run(&["list"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    let (_, refused) = ran.out.split_once("refused:\n").expect("a refused section");
    assert!(
        refused.starts_with("  workspace:alpha:typo (workspace alpha "),
        "{refused}"
    );
    let checked = rig.run(&["check", "workspace:alpha:typo", "--json"]);
    assert_eq!(checked.code, 1, "{}{}", checked.out, checked.err);
    assert_eq!(checked.json()["errors"][0]["token"], "text.primary");
}

#[test]
fn listing_and_checking_the_levels_write_nothing() {
    let rig = Rig::new();
    every_level(&rig);
    let before = every_file(rig.dir.path());

    rig.run(&["list"]);
    rig.run(&["list", "--json"]);
    rig.run(&["check", "machine:base"]);
    rig.run(&["check", "workspace:alpha:client", "--json"]);
    rig.run(&["check", "project:nowhere"]);

    assert_eq!(every_file(rig.dir.path()), before);
}
