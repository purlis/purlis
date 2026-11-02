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
    let (path, _) = screen_path(root, &Of::Skill("deploy".to_owned()), "g", "c").unwrap();
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
fn a_drafts_grades_never_push_out_the_skills_own_chats() {
    let plane = project();
    let root = plane.path();
    keep(root, "deploy", "01A", "c", "own").unwrap();
    for grade in ["01B", "01C", "01D", "01E"] {
        keep_of(root, &of_draft("deploy"), grade, "c", grade).unwrap();
    }

    sweep(root, "deploy");
    sweep_of(root, &of_draft("deploy"));

    assert_eq!(kept(root, "deploy", "01A", "c").as_deref(), Some("own"));
    assert_eq!(
        kept(root, "deploy", "01E", "c").as_deref(),
        Some("01E"),
        "read by its id"
    );
    assert_eq!(
        kept(root, "deploy", "01B", "c"),
        None,
        "a draft keeps its own newest"
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

#[test]
fn a_kept_chat_in_the_plane_s_state_folder_is_never_read() {
    // #3718: the kept chats are read from this machine's app state alone.
    let plane = project();
    let root = plane.path();
    keep(root, "deploy", "01GRADE", "01CHAT", "screen").expect("kept");
    crate::appstate::put_in_the_plane(root, DIR_NAME);
    assert_eq!(kept(root, "deploy", "01GRADE", "01CHAT"), None);
}

/// The router `relevance` chosen by `workspace` and `persona`.
fn router(workspace: Option<&str>, persona: Option<&str>) -> Of {
    Of::Router {
        router: "relevance".to_owned(),
        workspace: workspace.map(str::to_owned),
        persona: persona.map(str::to_owned),
    }
}

/// #3617: a router's grade keeps each chat it ran, by the scope that chose the router, apart
/// from any skill's, and only its newest grades in that scope keep theirs.
#[test]
fn a_router_s_grade_keeps_its_chats_by_scope_and_sweeps_its_older_ones_3617() {
    let plane = project();
    let root = plane.path();
    let ops = router(Some("ops"), None);
    keep_of(root, &ops, "01A", "01CHAT", "routed").expect("kept");
    assert_eq!(
        kept_of(root, &ops, "01A", "01CHAT").as_deref(),
        Some("routed")
    );
    assert_eq!(
        kept_of(root, &router(None, None), "01A", "01CHAT"),
        None,
        "another scope's"
    );
    assert_eq!(
        kept_of(root, &router(Some("ops"), Some("ops")), "01A", "01CHAT"),
        None,
        "another scope's"
    );
    assert_eq!(kept(root, "relevance", "01A", "01CHAT"), None, "no skill's");
    assert!(
        crate::appstate::dir(root)
            .unwrap()
            .join("router-grade-chats/relevance/ops/_/01A/01CHAT.screen")
            .is_file()
    );

    // The project's own router line keeps its grades all the same.
    keep_of(root, &router(None, None), "01Z", "01CHAT", "root").expect("kept");
    for grade in ["01B", "01C", "01D"] {
        keep_of(root, &ops, grade, "01CHAT", grade).expect("kept");
    }
    assert_eq!(sweep_of(root, &ops), ["01A"], "its oldest goes");
    assert_eq!(kept_of(root, &ops, "01A", "01CHAT"), None);
    assert_eq!(kept_of(root, &ops, "01D", "01CHAT").as_deref(), Some("01D"));
    assert_eq!(
        kept_of(root, &router(None, None), "01Z", "01CHAT").as_deref(),
        Some("root"),
        "another scope's are its own to sweep"
    );
}

/// #3617: a router or a scope that is not a plain name is never kept or read.
#[test]
fn a_router_s_name_that_is_not_plain_is_never_kept_3617() {
    let plane = project();
    let root = plane.path();
    for of in [
        Of::Router {
            router: "../up".to_owned(),
            workspace: None,
            persona: None,
        },
        router(Some(".."), None),
        router(None, Some("a/b")),
    ] {
        assert!(keep_of(root, &of, "01A", "01CHAT", "x").is_err(), "{of:?}");
        assert_eq!(kept_of(root, &of, "01A", "01CHAT"), None);
    }
}
