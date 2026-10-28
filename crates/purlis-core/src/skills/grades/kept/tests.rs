// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn project() -> tempfile::TempDir {
    tempfile::tempdir().expect("a project")
}

#[test]
fn a_grades_chat_is_kept_and_read_back_as_it_ended() {
    let plane = project();
    let root = plane.path();
    let screen = "\u{1b}[1mclaude\u{1b}[0m\r\n> ship it\r\n";
    keep(root, "deploy", "01GRADE", "01CHAT", screen).expect("kept");
    assert_eq!(
        kept(root, "deploy", "01GRADE", "01CHAT").as_deref(),
        Some(screen)
    );
    assert_eq!(
        kept(root, "deploy", "01GRADE", "01OTHER"),
        None,
        "never kept"
    );
    assert_eq!(
        kept(root, "other", "01GRADE", "01CHAT"),
        None,
        "another skill's"
    );
}

#[test]
fn a_name_that_is_not_plain_is_never_kept_or_read() {
    let plane = project();
    let root = plane.path();
    for (skill, grade, chat) in [
        ("../deploy", "g", "c"),
        ("deploy", "../g", "c"),
        ("deploy", "g", "c/../../x"),
        ("deploy", "", "c"),
        ("..", "..", "c"),
        (".", "g", "c"),
        ("deploy", "..", "c"),
        ("deploy", "g", ".."),
    ] {
        assert!(
            keep(root, skill, grade, chat, "x").is_err(),
            "{skill} {grade} {chat}"
        );
        assert_eq!(kept(root, skill, grade, chat), None);
    }
}

#[test]
fn a_screen_too_big_to_keep_is_not_kept() {
    let plane = project();
    let big = "x".repeat(MOST_SCREEN_BYTES + 1);
    assert!(keep(plane.path(), "deploy", "g", "c", &big).is_err());
    assert_eq!(kept(plane.path(), "deploy", "g", "c"), None);
}

#[cfg(unix)]
#[test]
fn a_kept_screen_that_is_a_link_is_never_read() {
    let plane = project();
    let root = plane.path();
    keep(root, "deploy", "g", "c", "mine").unwrap();
    let elsewhere = root.join("elsewhere");
    std::fs::write(&elsewhere, "not a screen").unwrap();
    let path = screen_path(root, "deploy", "g", "c").unwrap();
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &path).unwrap();
    assert_eq!(kept(root, "deploy", "g", "c"), None);
    assert!(
        keep(root, "deploy", "g", "c", "again").is_err(),
        "no write through a link"
    );
    assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), "not a screen");
}

#[test]
fn a_skills_newest_grades_keep_their_chats_and_the_older_ones_go() {
    let plane = project();
    let root = plane.path();
    // ULIDs sort by when they were made.
    let grades = ["01A", "01B", "01C", "01D", "01E"];
    for grade in grades {
        keep(root, "deploy", grade, "c", grade).unwrap();
    }
    keep(root, "other", "01A", "c", "kept").unwrap();
    let mut gone = sweep(root, "deploy");
    gone.sort();
    assert_eq!(gone, ["01A", "01B"]);
    for (grade, left) in grades.iter().zip([false, false, true, true, true]) {
        assert_eq!(kept(root, "deploy", grade, "c").is_some(), left, "{grade}");
    }
    assert_eq!(
        kept(root, "other", "01A", "c").as_deref(),
        Some("kept"),
        "another skill's grades are its own"
    );
}

#[test]
fn a_sweep_of_a_name_of_dots_takes_nothing() {
    let plane = project();
    let root = plane.path();
    for grade in ["01A", "01B", "01C", "01D"] {
        keep(root, "deploy", grade, "c", "x").unwrap();
    }
    for skill in [".", ".."] {
        assert!(sweep(root, skill).is_empty(), "{skill}");
    }
    assert!(
        kept(root, "deploy", "01A", "c").is_some(),
        "another skill's chats stay"
    );
}
