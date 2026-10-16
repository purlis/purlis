// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `themes` row: what sits in a plane's reserved `themes/` folders.

use std::path::Path;

use super::themes;
use crate::doctor::Status;
use crate::doctor::tests::{doctor, plane};

/// A theme folder `dir`: a folder holding a `theme.json`.
fn theme_folder(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("theme.json"), "{}").unwrap();
}

#[test]
fn a_plane_with_no_themes_folder_has_no_themes_row() {
    let (_d, root) = plane("schema = 1\n");
    std::fs::create_dir_all(root.join("workspaces/w")).unwrap();
    assert_eq!(themes(&doctor(&root)), None);
}

#[test]
fn theme_folders_at_the_plane_root_and_in_a_workspace_are_counted() {
    let (_d, root) = plane("schema = 1\n");
    theme_folder(&root.join("themes/nord"));
    theme_folder(&root.join("workspaces/client/themes/brand"));
    // A dot-file is how git keeps an empty folder, and is never a theme's.
    std::fs::write(root.join("themes/.gitkeep"), "").unwrap();
    let row = themes(&doctor(&root)).expect("a row");
    assert_eq!(row.status, Status::Ok, "{row:?}");
    assert_eq!(row.detail, "2 theme folders");
}

#[test]
fn what_is_not_a_theme_folder_is_named_by_its_path() {
    let (_d, root) = plane("schema = 1\n");
    theme_folder(&root.join("themes/nord"));
    // A bare theme.json is a theme with no folder of its own.
    std::fs::write(root.join("themes/theme.json"), "{}").unwrap();
    // A folder with no theme.json in it.
    std::fs::create_dir_all(root.join("themes/empty")).unwrap();
    // A theme.json that is a folder is not the file.
    std::fs::create_dir_all(root.join("themes/odd/theme.json")).unwrap();
    std::fs::create_dir_all(root.join("workspaces/w/themes")).unwrap();
    std::fs::write(root.join("workspaces/w/themes/notes.md"), "x").unwrap();
    let row = themes(&doctor(&root)).expect("a row");
    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert_eq!(
        row.detail,
        "4 entries in a themes/ folder are not theme folders: themes/empty, themes/odd, \
         themes/theme.json, workspaces/w/themes/notes.md"
    );
    assert!(
        row.hint.contains("themes/<name>/theme.json"),
        "{}",
        row.hint
    );
}

#[test]
fn a_themes_that_is_not_a_folder_is_named() {
    let (_d, root) = plane("schema = 1\n");
    std::fs::write(root.join("themes"), "x").unwrap();
    let row = themes(&doctor(&root)).expect("a row");
    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert_eq!(
        row.detail,
        "1 entry in a themes/ folder is not a theme folder: themes (not a folder)"
    );
}

#[cfg(unix)]
#[test]
fn a_link_is_never_a_theme_folder() {
    let (_d, root) = plane("schema = 1\n");
    theme_folder(&root.join("elsewhere"));
    std::fs::create_dir_all(root.join("themes")).unwrap();
    std::os::unix::fs::symlink(root.join("elsewhere"), root.join("themes/linked")).unwrap();
    theme_folder(&root.join("themes/inner"));
    std::fs::remove_file(root.join("themes/inner/theme.json")).unwrap();
    std::os::unix::fs::symlink(
        root.join("elsewhere/theme.json"),
        root.join("themes/inner/theme.json"),
    )
    .unwrap();
    let row = themes(&doctor(&root)).expect("a row");
    assert_eq!(
        row.detail,
        "2 entries in a themes/ folder are not theme folders: themes/inner, themes/linked"
    );
}

#[test]
fn a_long_list_is_cut_and_counted() {
    let (_d, root) = plane("schema = 1\n");
    std::fs::create_dir_all(root.join("themes")).unwrap();
    for n in 0..8 {
        std::fs::write(root.join(format!("themes/f{n}.json")), "{}").unwrap();
    }
    let row = themes(&doctor(&root)).expect("a row");
    assert_eq!(
        row.detail,
        "8 entries in a themes/ folder are not theme folders: themes/f0.json, themes/f1.json, \
         themes/f2.json, themes/f3.json, themes/f4.json, and 3 more"
    );
}

#[test]
fn the_row_is_in_the_doctors_run() {
    let (_d, root) = plane("schema = 1\n");
    std::fs::create_dir_all(root.join("themes/empty")).unwrap();
    let rows = doctor(&root).run();
    let row = rows
        .iter()
        .find(|r| r.name == "themes")
        .expect("a themes row");
    assert_eq!(row.status, Status::Warn);
}

#[cfg(unix)]
#[test]
fn a_themes_that_is_a_link_is_named_as_one() {
    let (_d, root) = plane("schema = 1\n");
    theme_folder(&root.join("elsewhere/nord"));
    std::os::unix::fs::symlink(root.join("elsewhere"), root.join("themes")).unwrap();
    let row = themes(&doctor(&root)).expect("a row");
    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert_eq!(
        row.detail,
        "1 entry in a themes/ folder is not a theme folder: themes (a link, not a folder)"
    );
}
