// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const PROJECT: &str = "01J9PRJCT00000000000000000";

fn tip(n: usize) -> String {
    format!("{n:040x}")
}

#[test]
fn a_tip_recorded_is_read_back_and_one_recorded_again_counts_as_newest() {
    let config = tempfile::tempdir().unwrap();

    record(config.path(), PROJECT, &[tip(1), tip(2)]).unwrap();
    record(config.path(), PROJECT, &[tip(1)]).unwrap();

    assert_eq!(read(config.path(), PROJECT), [tip(2), tip(1)]);
}

#[test]
fn each_project_keeps_its_own_tips() {
    let config = tempfile::tempdir().unwrap();
    let other = "01J9PRJCT00000000000000001";

    record(config.path(), PROJECT, &[tip(1)]).unwrap();
    record(config.path(), other, &[tip(2)]).unwrap();

    assert_eq!(read(config.path(), PROJECT), [tip(1)]);
    assert_eq!(read(config.path(), other), [tip(2)]);
}

#[test]
fn past_the_most_the_oldest_tips_are_let_go_of() {
    let config = tempfile::tempdir().unwrap();
    let tips: Vec<String> = (0..MOST + 2).map(tip).collect();

    record(config.path(), PROJECT, &tips).unwrap();

    let kept = read(config.path(), PROJECT);
    assert_eq!(kept.len(), MOST);
    assert_eq!(kept.first(), Some(&tip(2)));
    assert_eq!(kept.last(), Some(&tip(MOST + 1)));
}

#[test]
fn what_is_no_projects_id_or_no_commits_name_is_not_recorded() {
    let config = tempfile::tempdir().unwrap();

    record(config.path(), "../escape", &[tip(1)]).unwrap();
    record(
        config.path(),
        PROJECT,
        &["HEAD".to_owned(), "abc".to_owned()],
    )
    .unwrap();

    assert_eq!(read(config.path(), "../escape"), Vec::<String>::new());
    assert_eq!(read(config.path(), PROJECT), Vec::<String>::new());
    assert!(!folder(config.path()).join("..").join("escape").exists());
}
