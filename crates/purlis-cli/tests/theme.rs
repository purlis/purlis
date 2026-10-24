// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis theme list` and `purlis theme check` (#2321, #2518): every theme available on this
//! machine and in the plane, with its level and where it comes from, and the theme check of any
//! one, for a person and for a chat.
//! `purlis theme import` and `export` (#2334): a theme folder made from each kind of source,
//! and a theme folder's zip that imports back to the same folder.
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
        self.run_in(args, None)
    }

    /// `purlis theme <args>` as the chat `chat` purlis started in the workspace `alpha` runs it
    /// (#2332), or as no chat with `None`.
    fn run_in(&self, args: &[&str], chat: Option<&str>) -> Ran {
        let mut command = support::purlis();
        command
            .arg("theme")
            .args(args)
            .current_dir(self.plane())
            .env("PURLIS_ROOT", self.plane())
            .env("PURLIS_CONFIG_HOME", self.config())
            .env("PURLIS_DATA_HOME", self.dir.path().join("data"));
        // Whichever chat runs these tests is not the one a test speaks as.
        for name in [
            "PURLIS_CHAT_ULID",
            "CHARTER_CHAT_ULID",
            "PURLIS_CHAT_DIR",
            "CHARTER_CHAT_DIR",
            // Nor is its connection to the app this one's: a test hands nothing to a real app.
            "PURLIS_CHAT",
            "CHARTER_CHAT",
            "PURLIS_HOOK_SOCKET",
            "CHARTER_HOOK_SOCKET",
            "PURLIS_CHAT_TOKEN",
            "CHARTER_CHAT_TOKEN",
        ] {
            command.env_remove(name);
        }
        if let Some(chat) = chat {
            command
                .env("PURLIS_CHAT_ULID", chat)
                .env("PURLIS_CHAT_DIR", self.plane().join("workspaces/alpha"));
        }
        let out = command.output().expect("the binary runs");
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
fn the_built_in_themes_are_listed_with_their_appearances_and_where_they_come_from() {
    let rig = Rig::new();

    let ran = rig.run(&["list"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    // Purlis is the default and is listed first; Purlis Classic is the alternative (#3034).
    assert_eq!(
        ran.out,
        "  ID              NAME            APPEARANCES                                            FROM\n\
         \x20 purlis          Purlis          dark, light, dark-high-contrast (dark, high contrast)  built-in\n\
         \x20 purlis-classic  Purlis Classic  dark, light                                            built-in\n"
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
    assert_eq!(lines.len(), 5, "{}", ran.out);
    let words = |line: &str| {
        line.split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        words(lines[3]),
        [
            "mine",
            "Late",
            "dark",
            "machine",
            &path.display().to_string()
        ]
    );
    assert_eq!(
        words(lines[4]),
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
    assert_eq!(themes[0]["id"], "purlis");
    assert_eq!(themes[0]["name"], "Purlis");
    assert_eq!(themes[0]["combinations"], 3);
    // A built-in theme says whether it ships as a preview (#2336); none does since Purlis became
    // the default (#3034), and no other theme has the key.
    assert_eq!(themes[0]["preview"], false);
    assert_eq!(
        themes[1],
        serde_json::json!({
            "id": "purlis-classic", "name": "Purlis Classic",
            "appearances": [
                { "id": "dark", "appearance": "dark", "high_contrast": false },
                { "id": "light", "appearance": "light", "high_contrast": false }
            ],
            "options": [], "combinations": 2, "level": "built-in",
            "from": { "kind": "built-in" }, "refused": null, "preview": false
        })
    );
    assert!(themes[2].get("preview").is_none(), "{themes}");
    assert!(themes[3].get("preview").is_none(), "{themes}");
    assert_eq!(themes[2]["id"], "mine");
    assert_eq!(themes[2]["from"]["kind"], "mine");
    assert_eq!(themes[2]["name"], serde_json::Value::Null);
    assert!(
        themes[2]["refused"]
            .as_str()
            .is_some_and(|why| why.contains("a theme is a JSON object")),
        "{themes}"
    );
    assert_eq!(
        themes[3],
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
         consent floors: none held in 2 combinations\n\
         hard floors: none broken in 2 combinations\n"
    );
}

#[test]
fn purlis_checks_clean_as_json_with_no_error_note_warning_or_floor_held() {
    let rig = Rig::new();

    let ran = rig.run(&["check", "purlis", "--json"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    let json = ran.json();
    for key in ["errors", "notes", "warnings", "held"] {
        assert_eq!(json[key], serde_json::json!([]), "{key}: {json}");
    }
    assert_eq!(json["clean"], true, "{json}");
}

#[test]
fn purlis_checks_clean_in_every_appearance_and_exits_zero() {
    let rig = Rig::new();

    let ran = rig.run(&["check", "purlis"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        ran.out,
        "Purlis — built-in, dark, light, dark-high-contrast (dark, high contrast)\n\
         left out: nothing\n\
         warnings: none\n\
         consent floors: none held in 3 combinations\n\
         hard floors: none broken in 3 combinations\n"
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
    assert_eq!(
        said["held"],
        serde_json::json!([]),
        "no consent floor is broken"
    );
    assert_eq!(
        said["hard_floors"],
        serde_json::json!([]),
        "muted words are no hard floor's"
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
fn an_old_id_checks_purlis() {
    let rig = Rig::new();

    // `purlis-dark` and `purlis-light`, and the ids they had, name Purlis's dark and light (#3034).
    for old in ["charter-dark", "purlis-light"] {
        let ran = rig.run(&["check", old, "--json"]);

        assert_eq!(ran.code, 0, "{}", ran.err);
        assert_eq!(ran.json()["theme"]["id"], "purlis", "{old}");
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
        rig.folder(
            &rig.machine_themes(),
            "base",
            &a_folder("Base", None, &none),
        ),
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
            vec![
                "purlis",
                "Purlis",
                "dark,",
                "light,",
                "dark-high-contrast",
                "(dark,",
                "high",
                "contrast)",
                "built-in"
            ],
            vec![
                "purlis-classic",
                "Purlis",
                "Classic",
                "dark,",
                "light",
                "built-in"
            ],
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
            ("purlis", "built-in"),
            ("purlis-classic", "built-in"),
            ("machine:base", "machine"),
            ("project:team", "project"),
            ("workspace:alpha:client", "workspace"),
            ("workspace:beta:night", "workspace"),
            ("nord/Nord", "extension"),
        ]
    );
    assert_eq!(
        themes[4]["from"],
        serde_json::json!({
            "kind": "workspace", "workspace": "alpha", "folder": "client", "path": shown(&client)
        })
    );
    assert_eq!(
        themes[3]["from"],
        serde_json::json!({ "kind": "project", "folder": "team", "path": shown(&team) })
    );
    assert_eq!(themes[4]["refused"], serde_json::Value::Null, "{themes:?}");
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

    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("the listing as JSON");
    let ids: Vec<&str> = json["themes"]
        .as_array()
        .expect("the themes")
        .iter()
        .filter_map(|one| one["id"].as_str())
        .collect();
    assert_eq!(ids, ["purlis", "purlis-classic", "machine:base"]);
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

    for asked in [
        "project:base",
        "workspace:alpha:team",
        "machine:team",
        "workspace:gamma:x",
    ] {
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
        .split_once("consent floors: held:\n")
        .unwrap_or_else(|| panic!("{}", ran.out));
    assert!(
        held.starts_with("  danger is distinguishable from primary — purlis draws its own ")
            && held
                .lines()
                .next()
                .is_some_and(|line| line.contains("danger.base")),
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
fn the_check_says_the_consent_floors_held_in_each_option_combination_that_breaks_one() {
    let rig = Rig::new();
    // An option value, not an appearance, draws the danger words into the danger surface.
    rig.folder(
        &rig.project_themes(),
        "ember",
        &serde_json::json!({
            "name": "Ember",
            "appearances": [
                { "id": "dark", "appearance": "dark" },
                { "id": "light", "appearance": "light" }
            ],
            "options": [{
                "id": "ember", "kind": "toggle", "label": "Ember", "default": false,
                "on": { "tokens": { "danger.text": "#301010", "danger.surface": "#301010" } }
            }]
        }),
    );

    let ran = rig.run(&["check", "project:ember"]);

    assert_eq!(ran.code, 0, "a held floor is drawn: {}{}", ran.out, ran.err);
    let (_, held) = ran
        .out
        .split_once("consent floors: held in 2 of 4 combinations:\n")
        .unwrap_or_else(|| panic!("{}", ran.out));
    assert_eq!(
        held,
        "  dark, ember on: the danger colours contrast with what they are drawn on — purlis \
         draws its own danger.surface, danger.text\n  \
         light, ember on: the danger colours contrast with what they are drawn on — purlis \
         draws its own danger.surface, danger.text\n\
         hard floors: none broken in 4 combinations\n"
    );

    let json = rig.run(&["check", "project:ember", "--json"]).json();
    assert_eq!(
        json["held"],
        serde_json::json!([
            {
                "appearance": "dark", "options": { "ember": "on" },
                "floors": ["danger"], "tokens": ["danger.surface", "danger.text"]
            },
            {
                "appearance": "light", "options": { "ember": "on" },
                "floors": ["danger"], "tokens": ["danger.surface", "danger.text"]
            }
        ])
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

// ------------------------------------------------------------------------------------------
// import and export (#2334)
// ------------------------------------------------------------------------------------------

/// The engine's own fixtures, one small source of each kind.
const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../purlis-core/src/theme/import-fixtures"
);

impl Rig {
    /// The fixture `name` copied into the rig, where a run can name it.
    fn fixture(&self, name: &str) -> PathBuf {
        let at = self.dir.path().join("sources").join(name);
        std::fs::create_dir_all(at.parent().expect("a folder")).expect("the sources folder");
        std::fs::copy(Path::new(FIXTURES).join(name), &at).expect("a fixture");
        at
    }

    /// A `themes/` folder of the rig's own.
    fn themes(&self) -> PathBuf {
        self.dir.path().join("themes")
    }
}

/// Every file of the theme folder `folder`, by its path inside it, with its bytes.
fn folder_files(folder: &Path) -> Vec<(String, Vec<u8>)> {
    every_file(folder)
        .into_iter()
        .map(|(path, bytes)| {
            let inside = path
                .strip_prefix(folder)
                .expect("inside")
                .to_string_lossy()
                .into_owned();
            (inside, bytes)
        })
        .collect()
}

#[test]
fn each_kind_of_source_imports_into_a_theme_folder_the_check_draws() {
    let rig = Rig::new();
    for (fixture, kind, folder) in [
        ("ember.jsonc", "vscode", "ember"),
        ("harbor.yaml", "base16", "harbor"),
        ("dune.json", "base24", "dune"),
        ("tide.itermcolors", "iterm", "tide"),
        ("moss", "ghostty", "moss"),
    ] {
        let source = rig.fixture(fixture);
        let themes = rig.themes();

        let ran = rig.run(&[
            "import",
            source.to_str().expect("a path"),
            "--into",
            themes.to_str().expect("a path"),
        ]);

        assert_eq!(ran.code, 0, "{fixture}: {}{}", ran.out, ran.err);
        let first = ran.out.lines().next().unwrap_or_default();
        assert!(
            first.starts_with("imported ") && first.contains(&format!("({kind})")),
            "{first}"
        );
        assert!(ran.out.contains("not taken"), "{}", ran.out);
        assert!(
            ran.out.contains("hard floors:"),
            "the check is shown: {}",
            ran.out
        );
        let written = themes.join(folder);
        let hint = ran.out.lines().last().unwrap_or_default();
        assert!(
            hint.starts_with(&format!("the theme is in {}:", written.display())),
            "{hint}"
        );
        assert!(written.join("theme.json").is_file(), "{fixture}");
        let checked = rig.run(&["check", written.to_str().expect("a path")]);
        assert_eq!(checked.code, 0, "{}", checked.out);
        assert!(checked.out.contains("left out: nothing"), "{}", checked.out);
    }
}

#[test]
fn what_a_source_has_that_no_token_takes_is_listed() {
    let rig = Rig::new();
    let source = rig.fixture("harbor.yaml");
    let themes = rig.themes();

    let ran = rig.run(&[
        "import",
        source.to_str().expect("a path"),
        "--into",
        themes.to_str().expect("a path"),
    ]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    assert!(
        ran.out.contains("not taken (2):\n  author: theme.json has no place for it\n  base06: no token takes it\n"),
        "{}",
        ran.out
    );
}

#[test]
fn a_base16_scheme_imports_tuned_to_read_with_no_warnings() {
    let rig = Rig::new();
    let source = rig.fixture("harbor.yaml");
    let themes = rig.themes();

    let ran = rig.run(&[
        "import",
        source.to_str().expect("a path"),
        "--into",
        themes.to_str().expect("a path"),
    ]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    assert!(ran.out.contains("warnings: none\n"), "{}", ran.out);
    assert!(ran.out.contains("consent floors: none held"), "{}", ran.out);
}

#[test]
fn an_import_with_no_folder_named_goes_into_your_own_themes() {
    let rig = Rig::new();
    let source = rig.fixture("moss");

    let ran = rig.run(&[
        "import",
        source.to_str().expect("a path"),
        "--kind",
        "ghostty",
    ]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    assert!(
        purlis_core::machine::dir(&rig.config())
            .join("themes/moss/theme.json")
            .is_file(),
        "{}",
        ran.out
    );
}

#[test]
fn an_import_never_replaces_a_theme_folder_without_force() {
    let rig = Rig::new();
    let source = rig.fixture("ember.jsonc");
    let themes = rig.themes();
    std::fs::create_dir_all(themes.join("ember")).expect("a folder");
    std::fs::write(themes.join("ember/theme.json"), "mine").expect("a theme");
    let args = [
        "import",
        source.to_str().expect("a path"),
        "--into",
        themes.to_str().expect("a path"),
    ];

    let refused = rig.run(&args);
    let kept = std::fs::read_to_string(themes.join("ember/theme.json")).expect("read");
    let forced = rig.run(&[&args[..], &["--force"]].concat());

    assert_eq!(refused.code, 1);
    assert!(refused.err.contains("--force"), "{}", refused.err);
    assert_eq!(kept, "mine");
    assert_eq!(forced.code, 0, "{}", forced.err);
    let replaced = std::fs::read_to_string(themes.join("ember/theme.json")).expect("read");
    assert!(replaced.contains("\"Ember\""), "{replaced}");
    let names: Vec<_> = std::fs::read_dir(&themes)
        .expect("listed")
        .flatten()
        .map(|it| it.file_name())
        .collect();
    assert_eq!(names.len(), 1, "nothing staged is left: {names:?}");
}

#[test]
fn an_import_as_json_says_what_was_written_not_taken_and_checked() {
    let rig = Rig::new();
    let source = rig.fixture("tide.itermcolors");
    let themes = rig.themes();

    let ran = rig.run(&[
        "import",
        source.to_str().expect("a path"),
        "--into",
        themes.to_str().expect("a path"),
        "--json",
    ]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    let answer = ran.json();
    assert_eq!(answer["schema"], 1);
    assert_eq!(answer["kind"], "iterm");
    assert_eq!(answer["name"], "tide");
    assert_eq!(answer["folder"], "tide");
    assert_eq!(answer["written"], themes.join("tide").display().to_string());
    assert_eq!(
        answer["not_taken"],
        serde_json::json!([
            { "key": "Name", "why": "is not a colour" },
            { "key": "Badge Color", "why": "no token takes it" }
        ])
    );
    assert_eq!(answer["check"]["schema"], 1);
    assert_eq!(answer["check"]["refused"], serde_json::Value::Null);
    assert_eq!(answer["check"]["notes"], serde_json::json!([]));
}

#[test]
fn a_source_purlis_cannot_tell_asks_for_its_kind_and_a_missing_one_exits_two() {
    let rig = Rig::new();
    let odd = rig.dir.path().join("notes.txt");
    std::fs::write(&odd, "nothing a theme says").expect("a file");

    let unknown = rig.run(&["import", odd.to_str().expect("a path")]);
    let missing = rig.run(&["import", "/nowhere/at/all.json"]);

    assert_eq!(unknown.code, 1);
    assert!(unknown.err.contains("--kind"), "{}", unknown.err);
    assert_eq!(missing.code, 2);
    assert!(
        !purlis_core::machine::dir(&rig.config())
            .join("themes")
            .exists(),
        "nothing was written"
    );
}

#[test]
fn an_exported_theme_folder_imports_back_to_the_same_folder() {
    let rig = Rig::new();
    let made = rig.dir.path().join("made");
    let folder = made.join("kept");
    std::fs::create_dir_all(folder.join("icons")).expect("a folder");
    std::fs::write(
        folder.join("theme.json"),
        r##"{ "name": "Kept", "palette": { "ink": "#101418" },
              "appearances": [{ "id": "dark", "appearance": "dark", "tokens": { "surface.base": "ink" } }] }"##,
    )
    .expect("a theme");
    std::fs::write(folder.join("LICENSE"), "made up for a test\n").expect("a licence");
    std::fs::write(folder.join("icons/notes.json"), "{}").expect("a file");
    let zip = rig.dir.path().join("kept.zip");
    let back = rig.dir.path().join("back");

    let exported = rig.run(&[
        "export",
        folder.to_str().expect("a path"),
        "--to",
        zip.to_str().expect("a path"),
    ]);
    let imported = rig.run(&[
        "import",
        zip.to_str().expect("a path"),
        "--into",
        back.to_str().expect("a path"),
    ]);

    assert_eq!(exported.code, 0, "{}", exported.err);
    assert_eq!(
        exported.out.lines().next().unwrap_or_default(),
        format!("exported kept (3 files) to {}", zip.display())
    );
    assert_eq!(imported.code, 0, "{}{}", imported.out, imported.err);
    assert!(
        imported.out.starts_with("imported Kept (zip) into "),
        "{}",
        imported.out
    );
    assert_eq!(folder_files(&back.join("kept")), folder_files(&folder));
}

#[test]
fn purlis_classic_exports_as_a_copy_beside_the_built_in() {
    let rig = Rig::new();
    let zip = rig.dir.path().join("classic.zip");
    let back = rig.dir.path().join("back");

    let exported = rig.run(&[
        "export",
        "purlis-classic",
        "--to",
        zip.to_str().expect("a path"),
    ]);
    let imported = rig.run(&[
        "import",
        zip.to_str().expect("a path"),
        "--into",
        back.to_str().expect("a path"),
    ]);

    assert_eq!(exported.code, 0, "{}", exported.err);
    assert_eq!(imported.code, 0, "{}{}", imported.out, imported.err);
    assert_eq!(
        std::fs::read_to_string(back.join("purlis-classic-copy/theme.json")).expect("read"),
        purlis_core::theme::CLASSIC_THEME_JSON
    );
}

#[test]
fn purlis_exports_as_a_copy_with_its_fonts_icons_and_licence() {
    let rig = Rig::new();
    let zip = rig.dir.path().join("purlis.zip");
    let back = rig.dir.path().join("back");

    let exported = rig.run(&["export", "purlis", "--to", zip.to_str().expect("a path")]);
    let imported = rig.run(&[
        "import",
        zip.to_str().expect("a path"),
        "--into",
        back.to_str().expect("a path"),
    ]);

    assert_eq!(exported.code, 0, "{}", exported.err);
    assert_eq!(imported.code, 0, "{}{}", imported.out, imported.err);
    let copy = back.join("purlis-copy");
    for (name, bytes) in purlis_core::theme::purlis_files() {
        assert_eq!(
            std::fs::read(copy.join(&name)).expect("read"),
            bytes,
            "{name}"
        );
    }
}

/// A theme folder's files as the engine compares them: `theme.json` read as JSON, the rest as
/// bytes.
fn folder_as_read(folder: &Path) -> (String, Vec<(String, Vec<u8>)>) {
    let files = folder_files(folder);
    let theme: serde_json::Value = files
        .iter()
        .find(|(name, _)| name == "theme.json")
        .map(|(_, bytes)| serde_json::from_slice(bytes).expect("theme.json is JSON"))
        .expect("a theme.json");
    let rest = files
        .into_iter()
        .filter(|(name, _)| name != "theme.json")
        .collect();
    // Written as text, so the comparison sees the keys' order too.
    (serde_json::to_string(&theme).expect("JSON"), rest)
}

#[test]
fn a_theme_folder_exported_as_design_tokens_imports_back_to_the_same_folder() {
    let rig = Rig::new();
    let folder = rig.dir.path().join("made/kept");
    std::fs::create_dir_all(folder.join("icons")).expect("a folder");
    std::fs::write(
        folder.join("theme.json"),
        r##"{ "name": "Kept", "palette": { "ink": "#101418" },
              "appearances": [{ "id": "dark", "appearance": "dark",
                                "tokens": { "surface.base": "ink", "caret": "platform" } }] }"##,
    )
    .expect("a theme");
    std::fs::write(folder.join("LICENSE"), "made up for a test\n").expect("a licence");
    std::fs::write(folder.join("icons/notes.json"), "{}").expect("a file");
    let tokens = rig.dir.path().join("kept.tokens.json");
    let back = rig.dir.path().join("back");

    let exported = rig.run(&[
        "export",
        folder.to_str().expect("a path"),
        "--kind",
        "design-tokens",
        "--to",
        tokens.to_str().expect("a path"),
    ]);
    let imported = rig.run(&[
        "import",
        tokens.to_str().expect("a path"),
        "--into",
        back.to_str().expect("a path"),
    ]);

    assert_eq!(exported.code, 0, "{}", exported.err);
    assert_eq!(
        exported.out.lines().next().unwrap_or_default(),
        format!("exported kept (3 files) to {}", tokens.display())
    );
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&tokens).expect("read")).expect("JSON");
    assert_eq!(
        written["appearance"]["dark"]["surface"]["base"]["$value"], "{palette.ink}",
        "a palette colour is a reference to the palette"
    );
    assert_eq!(imported.code, 0, "{}{}", imported.out, imported.err);
    assert!(
        imported
            .out
            .starts_with("imported Kept (design-tokens) into "),
        "{}",
        imported.out
    );
    assert_eq!(folder_as_read(&back.join("kept")), folder_as_read(&folder));
}

#[test]
fn every_built_in_theme_exports_as_design_tokens_and_imports_back_the_same() {
    for (id, copy) in [
        ("purlis-classic", "purlis-classic-copy"),
        ("purlis", "purlis-copy"),
    ] {
        let rig = Rig::new();
        let back = rig.dir.path().join("back");

        let exported = rig.run(&["export", id, "--kind", "design-tokens"]);
        let tokens = rig.plane().join(format!("{copy}.tokens.json"));
        let imported = rig.run(&[
            "import",
            tokens.to_str().expect("a path"),
            "--into",
            back.to_str().expect("a path"),
        ]);

        assert_eq!(exported.code, 0, "{id}: {}", exported.err);
        assert_eq!(imported.code, 0, "{id}: {}{}", imported.out, imported.err);
        let files = if id == "purlis" {
            purlis_core::theme::purlis_files()
        } else {
            purlis_core::theme::Files::from([(
                "theme.json".to_owned(),
                purlis_core::theme::CLASSIC_THEME_JSON.as_bytes().to_vec(),
            )])
        };
        let (theme, rest) = folder_as_read(&back.join(copy));
        let expected: serde_json::Value =
            serde_json::from_slice(&files["theme.json"]).expect("JSON");
        assert_eq!(
            theme,
            serde_json::to_string(&expected).expect("JSON"),
            "{id}'s theme.json"
        );
        let expected_rest: Vec<(String, Vec<u8>)> = files
            .into_iter()
            .filter(|(name, _)| name != "theme.json")
            .collect();
        assert_eq!(rest, expected_rest, "{id}'s files");
    }
}

#[test]
fn an_export_writes_its_name_here_and_never_over_a_file_without_force() {
    let rig = Rig::new();

    let first = rig.run(&["export", "purlis-classic"]);
    let again = rig.run(&["export", "purlis-classic"]);
    let forced = rig.run(&["export", "purlis-classic", "--force"]);

    assert_eq!(first.code, 0, "{}", first.err);
    assert!(rig.plane().join("purlis-classic-copy.zip").is_file());
    assert_eq!(again.code, 1);
    assert!(again.err.contains("--force"), "{}", again.err);
    assert_eq!(forced.code, 0, "{}", forced.err);
}

#[test]
fn an_export_of_what_is_not_a_theme_folder_says_so_and_exits_two() {
    let rig = Rig::new();
    let file = rig.dir.path().join("one.json");
    std::fs::write(&file, a_theme("One", &serde_json::json!({}))).expect("a theme file");

    let ran = rig.run(&["export", file.to_str().expect("a path")]);

    assert_eq!(ran.code, 2);
    assert!(ran.err.contains("a folder of its own"), "{}", ran.err);
}

#[test]
fn a_level_s_theme_folder_is_exported_by_its_id() {
    let rig = Rig::new();
    let [base, team, client, _] = every_level(&rig);
    std::fs::write(team.join("LICENSE"), "made up for a test\n").expect("a licence");

    for (asked, folder, files) in [
        ("machine:base", &base, 1),
        ("project:team", &team, 2),
        ("workspace:alpha:client", &client, 1),
    ] {
        let zip = rig
            .dir
            .path()
            .join(format!("{}.zip", asked.replace(':', "-")));

        let exported = rig.run(&["export", asked, "--to", zip.to_str().expect("a path")]);

        assert_eq!(
            exported.code, 0,
            "{asked}: {}{}",
            exported.out, exported.err
        );
        let name = folder
            .file_name()
            .and_then(|it| it.to_str())
            .expect("a name");
        assert_eq!(
            exported.out.lines().next().unwrap_or_default(),
            format!(
                "exported {name} ({files} {}) to {}",
                if files == 1 { "file" } else { "files" },
                zip.display()
            ),
            "{asked}"
        );
        // The zip is that level's folder, file for file, under its name.
        let (top, files) =
            purlis_core::theme::unpack(&std::fs::read(&zip).expect("the zip")).expect("unpacked");
        assert_eq!(top, name, "{asked}");
        let packed: Vec<(String, Vec<u8>)> = files.into_iter().collect();
        assert_eq!(packed, folder_files(folder), "{asked}");
    }
}

#[test]
fn a_level_id_with_no_folder_at_that_level_is_refused_naming_the_level_s_form() {
    let rig = Rig::new();
    every_level(&rig);

    for (asked, form) in [
        // Each name is a folder of another level: it is never exported in this one's place.
        ("project:base", "`project:<folder>`"),
        ("workspace:alpha:team", "`workspace:<workspace>:<folder>`"),
        ("machine:team", "`machine:<folder>`"),
        ("workspace:gamma:night", "`workspace:<workspace>:<folder>`"),
    ] {
        let zip = rig.dir.path().join("never.zip");

        let ran = rig.run(&["export", asked, "--to", zip.to_str().expect("a path")]);

        assert_eq!(ran.code, 2, "{asked}: {}{}", ran.out, ran.err);
        assert_eq!(ran.out, "", "{asked}");
        assert!(ran.err.contains(form), "{asked}: {}", ran.err);
        assert!(
            ran.err.contains("never a farther level's of the same name"),
            "{}",
            ran.err
        );
        assert!(!zip.exists(), "{asked}: nothing is written");
    }
}

#[test]
fn a_level_s_folder_windows_cannot_hold_is_checked_and_refused_with_why() {
    let rig = Rig::new();
    rig.folder(
        &rig.machine_themes(),
        "con",
        &a_folder("Console", None, &serde_json::json!({})),
    );

    let ran = rig.run(&["check", "machine:con"]);

    assert_eq!(ran.code, 1, "{}{}", ran.out, ran.err);
    assert!(
        ran.out.contains("con is a name Windows keeps for a device"),
        "{}",
        ran.out
    );
}

#[test]
fn a_terminal_import_s_ansi_warning_names_the_palette_entry_to_change() {
    let rig = Rig::new();
    let source = rig.fixture("moss");
    let themes = rig.themes();

    let ran = rig.run(&[
        "import",
        source.to_str().expect("a path"),
        "--into",
        themes.to_str().expect("a path"),
    ]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    let black = ran
        .out
        .lines()
        .find(|line| {
            line.trim_start()
                .starts_with("terminal.ansi.black on terminal.background")
        })
        .unwrap_or_default();
    assert!(
        black.contains("change `palette-0` in theme.json"),
        "{}",
        ran.out
    );
    // `purlis theme check` of the folder says the check's own words.
    let written = themes.join("moss");
    let checked = rig.run(&["check", written.to_str().expect("a path")]);
    assert!(!checked.out.contains("palette-0"), "{}", checked.out);
}

/// A one-appearance dark theme folder's `theme.json` that sets `tokens`.
fn dark_folder(name: &str, tokens: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    })
}

#[test]
fn the_check_reports_each_hard_floor_a_combination_breaks() {
    let rig = Rig::new();
    // A light sunken ground under Classic's light words.
    rig.folder(
        &rig.project_themes(),
        "glare",
        &dark_folder("Glare", &serde_json::json!({ "surface.sunken": "#f2f2f2" })),
    );

    let ran = rig.run(&["check", "project:glare"]);

    assert_eq!(ran.code, 0, "a kept theme is drawn: {}{}", ran.out, ran.err);
    let (_, hard) = ran
        .out
        .split_once("hard floors: broken:\n")
        .unwrap_or_else(|| panic!("{}", ran.out));
    assert!(
        hard.starts_with(
            "  primary text is readable on every surface: text.primary on surface.sunken is "
        ),
        "{hard}"
    );

    let json = rig.run(&["check", "project:glare", "--json"]).json();
    let first = &json["hard_floors"][0];
    assert_eq!(first["floor"], "primary-text");
    assert_eq!(first["front"], "text.primary");
    assert_eq!(first["back"], "surface.sunken");
    assert_eq!(first["appearance"], "dark");
    assert_eq!(first["options"], serde_json::json!({}));
    assert_eq!(first["tint"], serde_json::Value::Null);
    // A pair on a plain ground is laid over no layer. (Every hard-floor pair over a wash is a
    // consent pair, which the window holds before the hard floors are measured.)
    assert!(first.get("beneath").is_none(), "{first:#}");
}

#[test]
fn the_check_warns_once_for_each_style_drawn_without_a_face_naming_its_slot_and_style() {
    let rig = Rig::new();
    // Code in JetBrains Mono's upright regular and bold, with no italic, and its comments drawn
    // italic.
    let themes = rig.dir.path().join("themes");
    let folder = themes.join("slant");
    std::fs::create_dir_all(folder.join("fonts")).expect("a fonts folder");
    let (regular, bold) = (
        "fonts/JetBrainsMono-Regular.woff2",
        "fonts/JetBrainsMono-Bold.woff2",
    );
    for face in [regular, bold] {
        std::fs::write(folder.join(face), &purlis_core::theme::purlis_files()[face])
            .expect("a face");
    }
    std::fs::write(
        folder.join("theme.json"),
        serde_json::json!({
            "name": "Slant",
            "fonts": { "mono": {
                "family": "JetBrains Mono",
                "faces": [
                    { "file": regular, "weight": 400 },
                    { "file": bold, "weight": 700 }
                ]
            } },
            "appearances": [{ "id": "dark", "appearance": "dark",
                "tokens": { "syntax.comment.italic": true } }]
        })
        .to_string(),
    )
    .expect("a theme.json");
    let at = folder.display().to_string();

    let ran = rig.run(&["check", &at]);

    assert_eq!(
        ran.code, 0,
        "a warning is not a refusal: {}{}",
        ran.out, ran.err
    );
    let said = "mono draws italic text at weight 400 with no face of its own, so the browser \
                makes it from another face";
    assert_eq!(ran.out.matches(said).count(), 1, "{}", ran.out);
    assert!(ran.out.contains("warnings (1):\n"), "{}", ran.out);

    let json = rig.run(&["check", &at, "--json"]).json();
    assert_eq!(
        json["warnings"],
        serde_json::json!([{
            "kind": "style-without-a-face",
            "slot": "mono",
            "weight": 400,
            "style": "italic",
            "appearance": "dark",
            "options": {},
        }])
    );
    assert_eq!(json["clean"], false);
}

#[test]
fn a_theme_draft_is_written_by_the_app_and_never_by_the_command_whatever_id_it_is_given() {
    let rig = Rig::new();
    // An id in the environment is no chat's say-so: with no app to hand it to, nothing is written.
    let chat = purlis_core::reopen::mint();
    for args in [&["draft", "dusk"][..], &["draft", "dusk", "--save"][..]] {
        let ran = rig.run_in(args, Some(&chat));
        assert_eq!(ran.code, 1, "{}{}", ran.out, ran.err);
    }
    let started = rig.run_in(&["draft", "dusk"], Some(&chat));
    assert!(
        started
            .err
            .contains("written by the app that started this chat"),
        "{}",
        started.err
    );
    let drafts =
        purlis_core::plane::state_dir(&rig.plane()).join(purlis_core::theme::drafts::DRAFTS);
    assert!(
        !drafts.exists(),
        "nothing is written in {}",
        drafts.display()
    );
}

#[test]
fn a_chat_s_theme_draft_is_checked_as_the_window_draws_it() {
    let rig = Rig::new();
    let chat = purlis_core::reopen::mint();
    // As the app writes it, for this chat.
    let path = purlis_core::theme::drafts::folder_of(&rig.plane(), &chat)
        .expect("a chat's")
        .join("dusk");
    std::fs::create_dir_all(&path).expect("the draft's folder");
    std::fs::write(
        path.join("theme.json"),
        dark_folder("Dusk", &serde_json::json!({})).to_string(),
    )
    .expect("the draft is written");

    let checked = rig.run_in(&["check", "draft:dusk"], Some(&chat));
    assert_eq!(checked.code, 0, "{}{}", checked.out, checked.err);
    assert!(
        checked.out.contains("hard floors: none broken"),
        "{}",
        checked.out
    );
    // No level chooses a draft: its check names none (#2707).
    let as_json = rig.run_in(&["check", "draft:dusk", "--json"], Some(&chat));
    assert_eq!(
        as_json.json()["chosen"],
        serde_json::json!([]),
        "{}",
        as_json.out
    );

    std::fs::write(
        path.join("theme.json"),
        dark_folder("Dusk", &serde_json::json!({ "surface.sunken": "#f2f2f2" })).to_string(),
    )
    .expect("the draft is written");
    let broken = rig.run_in(&["check", "draft:dusk"], Some(&chat));
    assert_eq!(broken.code, 1, "not drawn: {}{}", broken.out, broken.err);
    assert!(
        broken.out.contains("hard floors: broken:"),
        "{}",
        broken.out
    );
    assert!(broken.out.contains("not drawn"), "{}", broken.out);

    // Another chat has no such draft.
    let other = rig.run_in(&["check", "draft:dusk"], Some(&purlis_core::reopen::mint()));
    assert_eq!(other.code, 2, "{}{}", other.out, other.err);
}

#[test]
fn a_save_too_large_for_one_ask_is_refused_before_it_is_sent() {
    let rig = Rig::new();
    let chat = purlis_core::reopen::mint();
    let working = rig.dir.path().join("working");
    std::fs::create_dir_all(&working).expect("a working copy");
    let most = purlis_core::theme::drafts::MOST_SAVED_JSON_BYTES;
    // A theme.json whose save, as JSON writes it, is `bytes` long.
    let write = |bytes: usize| {
        let theme = |name: &str| {
            serde_json::json!({ "name": name, "appearances": [{ "id": "dark", "appearance": "dark" }] })
                .to_string()
        };
        let files = |text: String| {
            serde_json::to_string(&std::collections::BTreeMap::from([(
                "theme.json".to_owned(),
                text,
            )]))
            .expect("json")
            .len()
        };
        let name = "a".repeat(bytes - files(theme("")));
        assert_eq!(files(theme(&name)), bytes);
        std::fs::write(working.join("theme.json"), theme(&name)).expect("written");
    };
    let from = working.to_str().expect("a path");

    write(most + 1);
    let over = rig.run_in(&["draft", "dusk", "--save", "--from", from], Some(&chat));
    assert_eq!(over.code, 1, "{}{}", over.out, over.err);
    assert!(over.err.contains("a save carries at most"), "{}", over.err);

    // At the most, it is handed over: here no app takes it, and that is what is said.
    write(most);
    let at = rig.run_in(&["draft", "dusk", "--save", "--from", from], Some(&chat));
    assert_eq!(at.code, 1, "{}{}", at.out, at.err);
    assert!(at.err.contains("no app took this"), "{}", at.err);
}

// Each level's choice (#2707) ----------------------------------------------------------------

const PROJECT_ID: &str = "01JABCDEFGHJKMNPQRSTVWXYZ0";

/// A plane whose project keeps a theme folder `accented` with two options, and where every
/// level chose: the person's machine default (Purlis Classic, light), their choice for the
/// project (`accented`, lavender), the project's committed default (`accented`, teal) and the
/// workspace `alpha`'s pick, which is the person's choice there while `alpha` is LOCAL.
fn every_level_chose(rig: &Rig) {
    rig.folder(
        &rig.project_themes(),
        "accented",
        &serde_json::json!({
            "name": "Accented",
            "appearances": [{ "id": "dark", "appearance": "dark" }],
            "options": [
                { "id": "accent", "kind": "choice", "label": "Accent", "default": "lavender",
                  "values": [
                    { "id": "lavender", "label": "Lavender", "tokens": { "accent.base": "#b4a7f5" } },
                    { "id": "teal", "label": "Teal", "tokens": { "accent.base": "#2bb3a3" } }
                  ] },
                { "id": "loud", "kind": "toggle", "label": "Loud", "default": false,
                  "on": { "tokens": { "border.subtle": "#808080" } } }
            ]
        }),
    );
    std::fs::write(
        rig.plane().join("charter.toml"),
        format!(
            "schema = 1\n\n[project]\nid = \"{PROJECT_ID}\"\n\n[theme]\nuse = \"accented\"\n\
             options = {{ accent = \"teal\" }}\n"
        ),
    )
    .expect("a manifest");
    std::fs::write(
        rig.plane().join("workspaces/alpha/workspace.json"),
        r#"{"name": "alpha", "settings": {"theme": {"use": "accented", "appearance": "dark", "options": {"loud": "on"}}}}"#,
    )
    .expect("a workspace manifest");
    let held = purlis_core::machine::dir(&rig.config());
    std::fs::create_dir_all(&held).expect("the config folder");
    std::fs::write(
        held.join(purlis_core::theme::choices::FILE),
        serde_json::json!({
            "version": 1,
            "machine": { "theme": "purlis-classic", "appearance": "light" },
            "projects": {
                PROJECT_ID: { "theme": "accented", "from": "project",
                              "options": { "accent": "lavender" } }
            }
        })
        .to_string(),
    )
    .expect("the choices");
}

#[test]
fn the_listing_as_json_names_each_level_s_choice_its_from_level_and_option_values() {
    let rig = Rig::new();
    every_level_chose(&rig);

    let ran = rig.run(&["list", "--json"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(ran.json()["schema"], 1);
    assert_eq!(
        ran.json()["choices"],
        serde_json::json!([
            { "level": "workspace-choice", "workspace": "alpha", "theme": "accented",
              "id": "project:accented", "from": "project", "appearance": "dark",
              "options": { "loud": "on" } },
            { "level": "project-choice", "workspace": null, "theme": "accented",
              "id": "project:accented", "from": "project", "appearance": null,
              "options": { "accent": "lavender" } },
            { "level": "project-default", "workspace": null, "theme": "accented",
              "id": "project:accented", "from": "project", "appearance": null,
              "options": { "accent": "teal" } },
            { "level": "machine-default", "workspace": null, "theme": "purlis-classic",
              "id": "purlis-classic", "from": null, "appearance": "light", "options": {} }
        ])
    );
}

#[test]
fn the_listing_shows_each_level_s_choice_after_the_themes() {
    let rig = Rig::new();
    every_level_chose(&rig);

    let ran = rig.run(&["list"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    let chosen: Vec<Vec<&str>> = ran
        .out
        .lines()
        .skip_while(|line| *line != "chosen, nearest first:")
        .map(|line| line.split_whitespace().collect())
        .collect();
    assert_eq!(
        chosen,
        [
            vec!["chosen,", "nearest", "first:"],
            vec!["CHOSEN", "BY", "THEME", "FROM", "APPEARANCE", "OPTIONS"],
            vec![
                "workspace",
                "alpha,",
                "yours",
                "accented",
                "project",
                "dark",
                "loud=on"
            ],
            vec![
                "project,",
                "yours",
                "accented",
                "project",
                "-",
                "accent=lavender"
            ],
            vec![
                "project,",
                "committed",
                "accented",
                "project",
                "-",
                "accent=teal"
            ],
            vec!["machine,", "yours", "purlis-classic", "-", "light", "-"],
        ],
        "{}",
        ran.out
    );
}

#[test]
fn a_listing_where_nothing_chose_shows_no_choices() {
    let rig = Rig::new();

    let ran = rig.run(&["list", "--json"]);

    assert_eq!(ran.json()["choices"], serde_json::json!([]));
    assert!(!rig.run(&["list"]).out.contains("chosen"));
}

#[test]
fn the_check_names_each_level_that_chose_the_theme_with_its_values() {
    let rig = Rig::new();
    every_level_chose(&rig);

    let ran = rig.run(&["check", "project:accented", "--json"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    let chosen = ran.json()["chosen"].clone();
    let levels: Vec<&str> = chosen
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|one| one["level"].as_str())
        .collect();
    assert_eq!(
        levels,
        ["workspace-choice", "project-choice", "project-default"]
    );
    assert_eq!(
        chosen[2],
        serde_json::json!({ "level": "project-default", "workspace": null, "theme": "accented",
                            "id": "project:accented", "from": "project", "appearance": null,
                            "options": { "accent": "teal" } })
    );
    // Purlis Classic is chosen by the machine default alone.
    let classic = rig.run(&["check", "purlis-classic", "--json"]);
    assert_eq!(
        classic.json()["chosen"][0]["level"],
        "machine-default",
        "{}",
        classic.out
    );

    let text = rig.run(&["check", "project:accented"]);
    assert!(
        text.out.contains(
            "chosen by: workspace alpha, yours (loud=on); project, yours (accent=lavender); \
             project, committed (accent=teal)\n"
        ),
        "{}",
        text.out
    );
}

// ---------------------------------------------------------------------------------------
// Component parts (#2903)
// ---------------------------------------------------------------------------------------

#[test]
fn theme_parts_prints_every_family_part_state_and_property_as_json() {
    let rig = Rig::new();

    let ran = rig.run(&["parts"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    let json = ran.json();
    assert_eq!(json["schema"], 1);
    assert_eq!(json["version"], 13);
    assert_eq!(
        json["theme_schema"]
            .as_str()
            .map(|url| url.ends_with("theme.v2.schema.json")),
        Some(true)
    );
    assert_eq!(
        json["grammar"],
        "<family>.<part>[.<variant>][.<state>].<property>"
    );
    assert_eq!(json["families"][0]["name"], "button");
    // The window's own families (#2913), the tooltip (#2908), the popover and the menu (#2909),
    // and the project rail (#2915), and the list (#2911).
    let names: Vec<&str> = json["families"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|one| one["name"].as_str())
        .collect();
    // And the activity items of the four corners (#2917), and the resize handles (#3143).
    for family in [
        "panel",
        "panel-section",
        "empty-state",
        "status-item",
        "tooltip",
        "popover",
        "menu",
        "rail",
        "activity",
        "list",
        "panel-separator",
    ] {
        assert!(names.contains(&family), "{family}: {names:?}");
    }
    let corner = json["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "corner-radius")
        .expect("corner-radius");
    assert_eq!(corner["in_state"], false);
    assert_eq!(corner["bounds"]["or"][0], "full");
}

#[test]
fn a_theme_that_rounds_the_button_is_drawn_and_a_size_inside_a_state_is_refused() {
    let rig = Rig::new();
    let themes = rig.machine_themes();
    let pill = rig.folder(
        &themes,
        "pill",
        &serde_json::json!({
            "name": "Pill",
            "extends": "purlis-classic",
            "parts": { "button.root.corner-radius": "full" }
        }),
    );
    let shifty = rig.folder(
        &themes,
        "shifty",
        &serde_json::json!({
            "name": "Shifty",
            "extends": "purlis-classic",
            "parts": { "button.root.selected.font-weight": 600 }
        }),
    );

    let drawn = rig.run(&["check", pill.to_str().unwrap()]);
    assert_eq!(drawn.code, 0, "{}{}", drawn.out, drawn.err);

    let refused = rig.run(&["check", shifty.to_str().unwrap()]);
    assert_eq!(refused.code, 1, "{}{}", refused.out, refused.err);
    assert!(
        refused
            .out
            .contains("parts.button.root.selected.font-weight is refused: a state never changes"),
        "{}",
        refused.out
    );
}

// ---- the theme skill (#2333) ------------------------------------------------------------- //

/// The theme skill purlis's plugin ships: a chat runs what it says.
fn theme_skill() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../app/src-tauri/plugin/skills/theme/SKILL.md"),
    )
    .expect("the theme skill")
}

/// Each `purlis theme <command> …` the skill names, as `(command, its flags)`: in a code span,
/// or on a line of a code block, to the end of either.
fn skill_theme_commands(page: &str) -> Vec<(String, Vec<String>)> {
    let mut texts = Vec::new();
    let mut fenced = false;
    for line in page.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            texts.push(trimmed.to_owned());
        } else {
            texts.extend(line.split('`').skip(1).step_by(2).map(str::to_owned));
        }
    }
    texts
        .iter()
        .filter_map(|text| {
            let rest = text.split("purlis theme ").nth(1)?;
            let mut words = rest.split_whitespace();
            let command = words.next()?.to_owned();
            let flags = words
                .filter(|word| word.starts_with("--"))
                .map(|word| word.trim_end_matches(['`', ',', '.']).to_owned())
                .collect();
            Some((command, flags))
        })
        .collect()
}

#[test]
fn every_theme_command_the_theme_skill_names_is_one_purlis_has_with_its_flags_and_these_tests_run()
{
    let page = theme_skill();
    let named = skill_theme_commands(&page);
    let commands: std::collections::BTreeSet<&str> =
        named.iter().map(|(command, _)| command.as_str()).collect();
    assert_eq!(
        commands.into_iter().collect::<Vec<_>>(),
        ["check", "draft", "export", "import", "list", "parts"],
        "the skill's create, edit, check and share paths each name their command: {named:?}"
    );
    let rig = Rig::new();
    // The tests above this section: its own literals prove nothing.
    let whole = include_str!("theme.rs");
    let this = &whole[..whole
        .find("// ---- the theme skill")
        .expect("the section's marker")];
    for (command, flags) in &named {
        let help = rig.run(&[command, "--help"]);
        assert_eq!(help.code, 0, "purlis theme {command}: {}", help.err);
        for flag in flags {
            assert!(
                help.out.contains(flag.as_str()),
                "`purlis theme {command}` has no {flag}:\n{}",
                help.out
            );
            assert!(
                this.contains(&format!("\"{flag}\"")),
                "no test here runs {flag}"
            );
        }
        assert!(
            this.contains(&format!("run(&[\"{command}\""))
                || this.contains(&format!("run_in(&[\"{command}\""))
                || this.contains(&format!("&[\"{command}\", ")),
            "no test here runs `purlis theme {command}`"
        );
    }
}

#[test]
fn the_skill_s_scan_finds_a_command_in_a_code_span_and_a_code_block_with_its_flags() {
    let page = "Run `purlis theme check draft:x --json`.\n\n```bash\npurlis theme draft x --save --from \"$TMPDIR/x\"\n```\nThen purlis theme list is prose.\n";
    assert_eq!(
        skill_theme_commands(page),
        vec![
            ("check".to_owned(), vec!["--json".to_owned()]),
            (
                "draft".to_owned(),
                vec!["--save".to_owned(), "--from".to_owned()]
            ),
        ]
    );
}

#[test]
fn the_skill_s_import_and_export_write_into_a_folder_of_the_chat_s_own() {
    // The skill imports into, and exports to, a folder under the chat's temporary folder,
    // never into a level's themes or the plane.
    let rig = Rig::new();
    let scratch = rig.dir.path().join("tmp");
    let imported = scratch.join("purlis-theme-import");
    let source = rig.fixture("harbor.yaml");
    let ran = rig.run(&[
        "import",
        source.to_str().expect("a path"),
        "--into",
        imported.to_str().expect("a path"),
    ]);
    assert_eq!(ran.code, 0, "{}{}", ran.out, ran.err);
    assert!(imported.join("harbor/theme.json").is_file());
    assert!(
        !rig.config().join("themes").exists(),
        "nothing in your own themes"
    );

    let zip = scratch.join("purlis-classic.zip");
    let ran = rig.run(&[
        "export",
        "purlis-classic",
        "--to",
        zip.to_str().expect("a path"),
    ]);
    assert_eq!(ran.code, 0, "{}{}", ran.out, ran.err);
    assert!(zip.is_file());
    assert!(
        !rig.plane().join("purlis-classic.zip").exists(),
        "nothing written where the chat works"
    );
}

/// Each row of the skill's geometry table, as `(token, least, most, classic, compact)`: the
/// tokens in its first cell, and each of the next three cells' values in the same order.
fn skill_geometry_rows(page: &str) -> Vec<(String, f64, f64, f64, f64)> {
    let number = |text: &str| -> f64 {
        let text = text.trim().trim_end_matches("px").trim().replace('−', "-");
        text.parse()
            .unwrap_or_else(|_| panic!("not a number: {text:?}"))
    };
    let mut rows = Vec::new();
    let mut lines = page
        .lines()
        .skip_while(|line| !line.starts_with("| Token | Bounds | Classic | Compact |"))
        .skip(2);
    for line in lines.by_ref().take_while(|line| line.starts_with('|')) {
        let cells: Vec<&str> = line.trim_matches('|').split(" | ").collect();
        let tokens: Vec<&str> = cells[0].split('`').skip(1).step_by(2).collect();
        let bounds: Vec<&str> = cells[1].split(", ").collect();
        let classic: Vec<&str> = cells[2].split(", ").collect();
        let compact: Vec<&str> = cells[3].split(", ").collect();
        assert!(
            [bounds.len(), classic.len(), compact.len()] == [tokens.len(); 3],
            "one value per token in {line:?}"
        );
        for (at, token) in tokens.iter().enumerate() {
            let (least, most) = bounds[at]
                .split_once('–')
                .unwrap_or_else(|| panic!("a range: {:?}", bounds[at]));
            rows.push((
                (*token).to_owned(),
                number(least),
                number(most),
                number(classic[at]),
                number(compact[at]),
            ));
        }
    }
    rows
}

#[test]
fn the_theme_skill_s_corners_and_heights_are_the_engine_s_bounds_and_values() {
    // The skill keeps the corners and the heights; `purlis docs show themes` has every token,
    // and core's test holds that table to STEPS whole.
    let rows = skill_geometry_rows(&theme_skill());
    let steps = &purlis_core::theme::geometry::STEPS;
    let mut named: Vec<&str> = rows.iter().map(|row| row.0.as_str()).collect();
    named.sort_unstable();
    named.dedup();
    assert_eq!(named.len(), rows.len(), "every token once: {rows:?}");
    for (token, least, most, classic, compact) in &rows {
        let step = steps
            .iter()
            .find(|one| one.token == token)
            .unwrap_or_else(|| panic!("{token} is not a geometry token"));
        assert_eq!(
            (*least, *most, *classic, *compact),
            (step.least, step.most, step.classic, step.compact),
            "the skill's {token} is the engine's: its bounds, Classic's value and Classic's \
             compact value"
        );
    }
    for step in steps
        .iter()
        .filter(|one| one.token.starts_with("radius.") || one.token.starts_with("size."))
    {
        assert!(
            named.contains(&step.token),
            "the skill's table has every corner and height: {}",
            step.token
        );
    }
    // It points at the docs' Geometry section for the rest.
    let skill = theme_skill()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        skill.contains("`purlis docs show themes` has every one in its Geometry section"),
        "the skill points at the docs for the rest"
    );
    let docs = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../purlis-core/docs/themes.md"),
    )
    .expect("the themes docs");
    assert!(docs.contains("\n## Geometry\n"), "the docs have it");
    // The two rules it keeps inline: each line the check fits, and the tokens Classic's
    // compact moves.
    for fit in purlis_core::theme::FITS {
        let named = format!(
            "`{}` × `{}` within `{}`",
            fit.size, fit.line_height, fit.height
        );
        assert!(skill.contains(&named), "the skill says {named}");
    }
    let moved: Vec<&str> = steps
        .iter()
        .filter(|one| one.compact != one.classic)
        .map(|one| one.token)
        .collect();
    let spaces: Vec<&&str> = moved.iter().filter(|t| t.starts_with("space.")).collect();
    let mut parts = vec![format!(
        "`{}` to `{}`",
        spaces.first().expect("a spacing step"),
        spaces.last().expect("a spacing step")
    )];
    parts.extend(
        moved
            .iter()
            .filter(|t| !t.starts_with("space."))
            .map(|t| format!("`{t}`")),
    );
    let named = parts.join(", ");
    assert!(
        skill.contains(&named),
        "the skill's compact rule names {named}"
    );
}

#[test]
fn the_skill_s_geometry_scan_reads_one_value_per_token_in_order() {
    let page = "x\n\n| Token | Bounds | Classic | Compact | What it reaches |\n|---|---|---|---|---|\n| `a.b` `a.c` | 0–4 px, −4–0 px | 2, −2 | 1, −1 | x |\n\nafter\n| `not.read` | 1–2 | 1 | 1 | y |\n";
    assert_eq!(
        skill_geometry_rows(page),
        vec![
            ("a.b".to_owned(), 0.0, 4.0, 2.0, 1.0),
            ("a.c".to_owned(), -4.0, 0.0, -2.0, -1.0),
        ]
    );
}
