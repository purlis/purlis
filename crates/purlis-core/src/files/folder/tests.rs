// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::super::{Kind, Opened};
use super::{Walked, open, walk};

/// A skill's folder: its `SKILL.md`, a script, two references and an eval file.
fn skill() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let folder = std::fs::canonicalize(dir.path()).unwrap();
    for (path, text) in [
        ("SKILL.md", "---\nname: deploy\n---\nShip it.\n"),
        ("scripts/run.sh", "#!/bin/sh\n"),
        ("references/10-later.md", "later\n"),
        ("references/2-first.md", "first\n"),
        ("evals/triggers.json", "[]\n"),
    ] {
        let at = folder.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
    (dir, folder)
}

fn paths(folder: &Path) -> Vec<(String, Kind)> {
    walk(folder)
        .unwrap()
        .entries
        .into_iter()
        .map(|Walked { path, kind, .. }| (path, kind))
        .collect()
}

#[test]
fn a_walk_lists_every_entry_nested_folders_first_in_reading_order() {
    let (_dir, folder) = skill();
    let f = |p: &str| (p.to_string(), Kind::Folder);
    let file = |p: &str| (p.to_string(), Kind::File);
    assert_eq!(
        paths(&folder),
        vec![
            f("evals"),
            file("evals/triggers.json"),
            f("references"),
            file("references/2-first.md"),
            file("references/10-later.md"),
            f("scripts"),
            file("scripts/run.sh"),
            file("SKILL.md"),
        ]
    );
}

#[test]
fn a_file_of_the_folder_opens_as_text() {
    let (_dir, folder) = skill();
    assert_eq!(
        open(&folder, "references/2-first.md").unwrap(),
        Opened::Text {
            text: "first\n".into()
        }
    );
}

#[cfg(unix)]
#[test]
fn a_link_is_listed_with_why_and_never_followed_or_opened() {
    let (_dir, folder) = skill();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(outside.path().join("secrets")).unwrap();
    std::fs::write(outside.path().join("secrets/key"), "hidden").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secrets"), folder.join("away")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("secrets/key"), folder.join("key")).unwrap();
    let walked = walk(&folder).unwrap();
    let away = walked
        .entries
        .iter()
        .find(|one| one.path == "away")
        .unwrap();
    assert_eq!(away.kind, Kind::Link);
    assert!(away.refused.as_deref().unwrap().contains("is a link"));
    assert!(
        !walked
            .entries
            .iter()
            .any(|one| one.path.starts_with("away/"))
    );
    assert!(open(&folder, "key").is_err());
    assert!(open(&folder, "away/key").is_err());
}

#[test]
fn a_path_that_leaves_the_folder_or_names_git_is_refused() {
    let (_dir, folder) = skill();
    std::fs::create_dir(folder.join(".git")).unwrap();
    std::fs::write(folder.join(".git/config"), "x").unwrap();
    for path in ["../SKILL.md", "/etc/hosts", ".git/config", ""] {
        assert_eq!(
            open(&folder, path).unwrap_err(),
            format!("'{path}' is not a path inside the skill's folder")
        );
    }
    assert!(
        !paths(&folder)
            .iter()
            .any(|(path, _)| path.starts_with(".git"))
    );
}

#[test]
fn a_walk_lists_the_first_most_entries_and_counts_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let folder = std::fs::canonicalize(dir.path()).unwrap();
    for n in 0..super::MOST + 3 {
        std::fs::write(folder.join(format!("{n}.md")), "").unwrap();
    }
    let walked = walk(&folder).unwrap();
    assert_eq!(walked.entries.len(), super::MOST);
    assert_eq!(walked.more, 3);
}

#[test]
fn a_file_that_went_missing_says_so() {
    let (_dir, folder) = skill();
    assert_eq!(
        open(&folder, "gone.md").unwrap_err(),
        "'gone.md' is not in the skill's folder any more"
    );
}

#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_read_is_one_refused_row_and_the_rest_is_listed() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_dir, folder) = skill();
    let shut = folder.join("scripts");
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000)).unwrap();
    let walked = walk(&folder);
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o755)).unwrap();
    // Root reads any folder: nothing to show there.
    if walked
        .as_ref()
        .is_ok_and(|w| w.entries.iter().any(|e| e.path == "scripts/run.sh"))
    {
        return;
    }
    let walked = walked.unwrap();
    let scripts = walked
        .entries
        .iter()
        .find(|one| one.path == "scripts")
        .unwrap();
    assert!(
        scripts
            .refused
            .as_deref()
            .unwrap()
            .starts_with("purlis could not read 'scripts'")
    );
    assert!(walked.entries.iter().any(|one| one.path == "SKILL.md"));
}
