// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary folder");
    for args in [
        &["init", "-q", "-b", "main"][..],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "one",
        ],
    ] {
        assert!(crate::testgit::run(dir.path(), args).ok(), "git {args:?}");
    }
    dir
}

#[test]
fn a_clean_folder_has_no_work_left_in_it() {
    let dir = repo();
    assert_eq!(left_in(dir.path()), Some(false));
}

#[test]
fn a_changed_or_new_file_is_work_left_in_it() {
    let dir = repo();
    std::fs::write(dir.path().join("new.txt"), "x").expect("a file is written");
    assert_eq!(left_in(dir.path()), Some(true));
}

#[test]
fn a_folder_git_cannot_read_says_nothing() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    assert_eq!(left_in(&dir.path().join("not-there")), None);
}
