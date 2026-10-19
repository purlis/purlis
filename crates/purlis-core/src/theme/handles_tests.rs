// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every theme file read through a handle (#2604, #2670), and the names every platform holds
//! (#2671): a theme folder named on the command line, the person's own `theme.json` and an
//! export by level id each read from the folder they opened, whatever is put in its place
//! after; and a name Windows cannot hold is refused by import, export and the folder reader.

use std::path::Path;

use serde_json::{Value, json};

use super::levels::Reach;
use super::{Folders, Named, Resolved, THEME_JSON, TheirsOpened, export, pack, unpack};

/// A format 2 theme named `name`, with one dark appearance.
fn theme(name: &str) -> Value {
    json!({
        "name": name,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {} }]
    })
}

/// The theme folder `folder` in `themes`, holding `document`.
fn put(themes: &Path, folder: &str, document: &Value) {
    std::fs::create_dir_all(themes.join(folder)).unwrap();
    std::fs::write(themes.join(folder).join(THEME_JSON), document.to_string()).unwrap();
}

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

/// `at` moved aside and a link to `to` put in its place.
#[cfg(unix)]
fn swap_for_a_link(at: &Path, to: &Path) {
    let mut aside = at.as_os_str().to_owned();
    aside.push("-before");
    std::fs::rename(at, aside).unwrap();
    std::os::unix::fs::symlink(to, at).unwrap();
}

// ------------------------------------------------------------------------------------------
// `purlis theme check <folder>` (#2604)
// ------------------------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn a_named_folder_is_read_through_its_handle_when_it_becomes_a_link_after_it_is_opened() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    put(dir.path(), "dusk", &theme("Opened"));
    put(elsewhere.path(), "dusk", &theme("Elsewhere"));

    let named = Named::open(&dir.path().join("dusk"));
    swap_for_a_link(&dir.path().join("dusk"), &elsewhere.path().join("dusk"));
    let resolved = named.load();

    assert_eq!(said(&resolved), Vec::<String>::new());
    assert_eq!(resolved.theme.name, "Opened");
}

#[cfg(unix)]
#[test]
fn a_named_folder_s_parent_is_read_from_the_folder_opened_when_that_becomes_a_link() {
    let dir = tempfile::tempdir().unwrap();
    let themes = dir.path().join("themes");
    let elsewhere = tempfile::tempdir().unwrap();
    let mut kid = theme("Kid");
    kid["extends"] = json!("base");
    put(&themes, "kid", &kid);
    let mut base = theme("Base");
    base["appearances"][0]["tokens"] = json!({ "accent.base": "#123456" });
    put(&themes, "base", &base);
    put(elsewhere.path(), "kid", &kid);
    let mut other = theme("Other");
    other["appearances"][0]["tokens"] = json!({ "accent.base": "#abcdef" });
    put(elsewhere.path(), "base", &other);

    let named = Named::open(&themes.join("kid"));
    swap_for_a_link(&themes, elsewhere.path());
    let resolved = named.load();

    assert_eq!(said(&resolved), Vec::<String>::new());
    assert_eq!(resolved.theme.first().value("accent.base"), "#123456");
}

#[cfg(unix)]
#[test]
fn a_named_folder_that_is_a_link_when_it_is_opened_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    put(elsewhere.path(), "dusk", &theme("Elsewhere"));
    std::os::unix::fs::symlink(elsewhere.path().join("dusk"), dir.path().join("dusk")).unwrap();

    let resolved = Named::open(&dir.path().join("dusk")).load();

    assert!(resolved.refused);
    assert!(
        said(&resolved)[0].ends_with("is a link or not a folder, so it is not read"),
        "{:?}",
        said(&resolved)
    );
}

// ------------------------------------------------------------------------------------------
// The person's own `theme.json` (#2604)
// ------------------------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn the_person_s_theme_is_read_through_the_config_folder_s_handle_when_it_becomes_a_link() {
    let home = tempfile::tempdir().unwrap();
    let config = crate::machine::dir(home.path());
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join(THEME_JSON), theme("Mine").to_string()).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(
        elsewhere.path().join(THEME_JSON),
        theme("Elsewhere").to_string(),
    )
    .unwrap();

    let opened = TheirsOpened::open(home.path());
    swap_for_a_link(&config, elsewhere.path());
    let theirs = opened.read();

    assert_eq!(theirs.trouble, None);
    assert_eq!(theirs.theme.expect("in force").name, "Mine");
}

#[cfg(unix)]
#[test]
fn the_person_s_config_folder_that_is_a_link_is_not_read() {
    let home = tempfile::tempdir().unwrap();
    let config = crate::machine::dir(home.path());
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(
        elsewhere.path().join(THEME_JSON),
        theme("Elsewhere").to_string(),
    )
    .unwrap();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), &config).unwrap();

    let theirs = super::read_theirs(Some(home.path()));

    assert!(theirs.found);
    assert!(theirs.theme.is_none());
    assert!(
        theirs
            .trouble
            .as_deref()
            .is_some_and(|why| why.contains("is reached through a link")),
        "{:?}",
        theirs.trouble
    );
}

// ------------------------------------------------------------------------------------------
// Export by level id (#2670)
// ------------------------------------------------------------------------------------------

/// What `zip` holds as `dusk/theme.json`'s name.
fn exported_name(zip: &[u8]) -> String {
    let (_, files) = unpack(zip).expect("unpacked");
    let document: Value = serde_json::from_slice(&files[THEME_JSON]).unwrap();
    document["name"].as_str().unwrap_or_default().to_owned()
}

#[cfg(unix)]
#[test]
fn an_export_by_level_id_packs_the_folder_found_when_a_folder_above_it_becomes_a_link() {
    for (reach, below) in [
        (Reach::Project, "themes"),
        (Reach::Workspace, "workspaces/alpha/themes"),
    ] {
        let plane = tempfile::tempdir().unwrap();
        let themes = plane.path().join(below);
        put(&themes, "dusk", &theme("Found"));
        std::fs::write(themes.join("dusk/LICENSE"), "found\n").unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        put(elsewhere.path(), "dusk", &theme("Elsewhere"));
        std::fs::write(elsewhere.path().join("dusk/LICENSE"), "elsewhere\n").unwrap();

        let folders = Folders::of(None, Some(plane.path()), Some("alpha"));
        let found = folders.folder_at("dusk", reach).expect("found");
        swap_for_a_link(&themes, elsewhere.path());
        let packed = found.export().expect("exported");

        assert_eq!(exported_name(&packed.bytes), "Found", "{below}");
        let (_, files) = unpack(&packed.bytes).unwrap();
        assert_eq!(files["LICENSE"], b"found\n", "{below}");
    }
}

#[test]
fn an_export_by_level_id_of_a_folder_the_level_lacks_is_refused() {
    let plane = tempfile::tempdir().unwrap();
    put(&plane.path().join("themes"), "dusk", &theme("Found"));
    let folders = Folders::of(None, Some(plane.path()), None);
    assert!(folders.folder_at("dawn", Reach::Project).is_err());
    assert!(folders.folder_at("dusk", Reach::Workspace).is_err());
}

// ------------------------------------------------------------------------------------------
// Characters Windows holds in no name (#2671)
// ------------------------------------------------------------------------------------------

/// Every character Windows holds in no name, with a control character among them.
const UNHELD: [char; 11] = [
    '<', '>', ':', '"', '/', '\\', '|', '?', '*', '\u{7}', '\u{1f}',
];

#[test]
fn each_character_windows_holds_in_no_name_is_refused_in_one_sentence() {
    for c in UNHELD {
        let name = format!("a{c}b");
        let why = super::portable::refusal(&name, &name).unwrap_or_else(|| panic!("{c:?}"));
        assert!(
            why.ends_with("in it, which Windows holds in no name, so no theme holds it"),
            "{why}"
        );
        assert!(!why.contains(". "), "one sentence: {why}");
    }
    assert_eq!(
        super::portable::refusal("a?b.json", "a?b.json").as_deref(),
        Some("a?b.json has ? in it, which Windows holds in no name, so no theme holds it")
    );
    assert_eq!(
        super::portable::refusal("a\u{7}b", "a\u{7}b").map(|why| why.contains("U+0007")),
        Some(true)
    );
    assert_eq!(super::portable::refusal("ab", "ab"), None);
}

#[test]
fn a_zip_holding_a_file_whose_name_windows_cannot_hold_is_refused_whole() {
    for c in ['<', '>', '"', '|', '?', '*', '\u{7}'] {
        let files = super::Files::from([
            (
                THEME_JSON.to_owned(),
                theme("Zipped").to_string().into_bytes(),
            ),
            (format!("fonts/a{c}b.woff2"), b"wOF2".to_vec()),
        ]);
        let zip = pack("dusk", &files).expect("packed");
        let why = unpack(&zip).expect_err("refused");
        assert!(
            why.contains("which Windows holds in no name"),
            "{c:?}: {why}"
        );
    }
}

#[cfg(unix)]
#[test]
fn an_export_leaves_out_a_file_whose_name_windows_cannot_hold_and_says_why() {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "dusk", &theme("Dusk"));
    let folder = dir.path().join("dusk");
    std::fs::create_dir(folder.join("fonts")).unwrap();
    std::fs::write(folder.join("notes?.txt"), "x").unwrap();
    std::fs::write(folder.join("fonts/a|b.woff2"), "x").unwrap();
    std::fs::write(folder.join("fonts/a\u{7}b.woff2"), "x").unwrap();

    let packed = export(&folder).expect("exported");

    assert_eq!(packed.files, 1);
    let left: Vec<(&str, &str)> = packed
        .left_out
        .iter()
        .map(|one| (one.key.as_str(), one.why.as_str()))
        .collect();
    assert_eq!(left.len(), 3, "{left:?}");
    for (key, why) in left {
        assert!(
            why.contains("which Windows holds in no name"),
            "{key}: {why}"
        );
    }
}

#[cfg(unix)]
#[test]
fn the_folder_reader_refuses_an_icon_set_whose_name_windows_cannot_hold() {
    for (reach, below) in [
        (Reach::Project, "themes"),
        (Reach::Workspace, "workspaces/alpha/themes"),
    ] {
        let plane = tempfile::tempdir().unwrap();
        let themes = plane.path().join(below);
        let mut dusk = theme("Dusk");
        dusk["icons"] = json!("icons/a*b.json");
        put(&themes, "dusk", &dusk);
        std::fs::create_dir(themes.join("dusk/icons")).unwrap();
        std::fs::write(themes.join("dusk/icons/a*b.json"), "{}").unwrap();

        let folders = Folders::of(None, Some(plane.path()), Some("alpha"));
        let super::levels::Found::Read(resolved) = folders.find_at("dusk", reach) else {
            panic!("found");
        };
        assert!(resolved.refused);
        assert!(
            said(&resolved)
                .iter()
                .any(|one| one.contains("has * in it, which Windows holds in no name")),
            "{:?}",
            said(&resolved)
        );
        // And a folder the person names says the same.
        let named = Named::open(&themes.join("dusk")).load();
        assert!(named.refused);
        assert!(
            said(&named)
                .iter()
                .any(|one| one.contains("has * in it, which Windows holds in no name")),
            "{:?}",
            said(&named)
        );
    }
}

#[test]
fn a_named_folder_whose_name_windows_cannot_hold_is_refused_before_it_is_opened() {
    let dir = tempfile::tempdir().unwrap();
    let resolved = Named::open(&dir.path().join("du?sk")).load();
    assert!(resolved.refused);
    assert_eq!(
        said(&resolved),
        ["du?sk has ? in it, which Windows holds in no name, so no theme holds it"]
    );
}
