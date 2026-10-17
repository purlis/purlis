// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis theme list` and `purlis theme check` (#2321): every theme available on this machine
//! with where it comes from, and the theme check of any one, for a person and for a chat.
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

    fn plane(&self) -> PathBuf {
        self.dir.path().join("plane")
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
fn the_built_ins_are_listed_with_where_they_come_from() {
    let rig = Rig::new();

    let ran = rig.run(&["list"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        ran.out,
        "  ID            NAME          APPEARANCE  FROM\n\
         \x20 purlis-dark   purlis-dark   dark        built-in\n\
         \x20 purlis-light  purlis-light  light       built-in\n"
    );
}

#[test]
fn the_person_s_own_theme_and_an_extension_s_are_listed_after_the_built_ins() {
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
        ["mine", "Late", "dark", &path.display().to_string()]
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
fn a_theme_that_is_not_one_is_listed_as_refused_with_why() {
    let rig = Rig::new();
    rig.mine("this is not json");

    let ran = rig.run(&["list"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    let (_, refused) = ran.out.split_once("refused:\n").expect("a refused section");
    assert!(refused.starts_with("  mine ("), "{refused}");
    assert!(refused.contains("is not JSON"), "{refused}");
}

#[test]
fn the_listing_as_json_says_each_theme_s_id_name_appearance_and_source() {
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
            "id": "purlis-dark", "name": "purlis-dark", "appearance": "dark",
            "from": { "kind": "built-in" }, "refused": null
        })
    );
    assert_eq!(themes[2]["id"], "mine");
    assert_eq!(themes[2]["from"]["kind"], "mine");
    assert_eq!(themes[2]["name"], serde_json::Value::Null);
    assert!(
        themes[2]["refused"]
            .as_str()
            .is_some_and(|why| why.contains("not a JSON object")),
        "{themes}"
    );
    assert_eq!(
        themes[3],
        serde_json::json!({
            "id": "nord/Nord", "name": "Nord", "appearance": "dark",
            "from": { "kind": "extension", "extension": "nord" }, "refused": null
        })
    );
}

#[test]
fn a_built_in_checks_clean_and_exits_zero() {
    let rig = Rig::new();

    let ran = rig.run(&["check", "purlis-light"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        ran.out,
        "purlis-light — built-in, light\n\
         errors: none\n\
         warnings: none\n\
         hard floors: not checked by this purlis yet (#2332)\n"
    );
}

#[test]
fn a_value_purlis_had_to_put_right_is_an_error_and_exits_one() {
    let rig = Rig::new();
    rig.mine(&a_theme(
        "Typo",
        &serde_json::json!({ "text.primary": "red", "text.primry": "#ffffff" }),
    ));

    let ran = rig.run(&["check", "mine"]);

    assert_eq!(ran.code, 1, "{}{}", ran.out, ran.err);
    assert!(ran.out.contains("errors (2):\n"), "{}", ran.out);
    assert!(
        ran.out
            .contains("  text.primary is \"red\", which is not #rgb"),
        "{}",
        ran.out
    );
    assert!(
        ran.out.contains("  text.primry is not a purlis token\n"),
        "{}",
        ran.out
    );
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
    assert!(ran.out.contains("errors: none\n"), "{}", ran.out);
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
fn the_check_as_json_has_errors_warnings_and_hard_floors_for_a_chat() {
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

    assert_eq!(ran.code, 1, "{}{}", ran.out, ran.err);
    let said = ran.json();
    assert_eq!(said["schema"], 1);
    assert_eq!(
        said["theme"],
        serde_json::json!({
            "id": "dim/Dim", "name": "Dim", "appearance": "dark",
            "from": { "kind": "extension", "extension": "dim" }
        })
    );
    assert_eq!(said["refused"], serde_json::Value::Null);
    assert_eq!(
        said["errors"],
        serde_json::json!([{ "said": "bogus is not a purlis token", "token": "bogus" }])
    );
    let first = &said["warnings"][0];
    assert_eq!(first["kind"], "contrast");
    assert_eq!(first["tint"], serde_json::Value::Null);
    assert_eq!(first["pair"]["front"], "text.muted");
    assert_eq!(first["pair"]["back"], "surface.base");
    assert_eq!(first["pair"]["floor"], 4.5);
    assert_eq!(first["ratio"], 2.38);
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
    assert!(ran.out.contains("\nrefused: "), "{}", ran.out);
    assert!(ran.out.contains("is not JSON"), "{}", ran.out);
    assert!(
        ran.out.contains("the window draws Purlis in its place"),
        "{}",
        ran.out
    );
    let json = rig.run(&["check", "mine", "--json"]).json();
    assert!(
        json["refused"]
            .as_str()
            .is_some_and(|why| why.contains("not JSON"))
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
fn an_old_id_checks_the_built_in_it_names_now() {
    let rig = Rig::new();

    let ran = rig.run(&["check", "charter-dark", "--json"]);

    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(ran.json()["theme"]["name"], "purlis-dark");
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
