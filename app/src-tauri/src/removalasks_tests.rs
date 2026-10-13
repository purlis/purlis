// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's ask that the person remove a workspace (qw356): what is held, what is asked, and
//! that nothing a chat sends removes anything.

use super::*;

/// A project with workspaces `alpha` and `beta`.
fn a_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a project");
    for ws in ["alpha", "beta"] {
        std::fs::create_dir_all(dir.path().join("workspaces").join(ws)).expect("a workspace");
    }
    dir
}

fn waits(session: u32, workspace: &str, since: u64) -> Waits {
    Waits {
        session,
        workspace: workspace.to_owned(),
        since,
    }
}

#[test]
fn asking_removes_nothing_and_says_the_person_was_asked() {
    // The dead end this ends: the chat's command was refused and told to ask the person.
    let dir = a_project();
    let asked = Asked::default();

    let (said, news) = asked_in(dir.path(), &asked, 3, "beta", 100);

    assert!(news);
    assert_eq!(said, removalask::asked_said("beta"));
    assert_eq!(asked.every(), [waits(3, "beta", 100)]);
    assert!(
        dir.path().join("workspaces/beta").is_dir(),
        "nothing is removed by asking"
    );
}

#[test]
fn a_second_ask_about_one_workspace_is_the_same_question() {
    let dir = a_project();
    let asked = Asked::default();
    asked_in(dir.path(), &asked, 3, "beta", 100);

    let (said, news) = asked_in(dir.path(), &asked, 3, "beta", 200);

    // Told the same, and no second question: it waits since the first.
    assert!(!news);
    assert_eq!(said, removalask::asked_said("beta"));
    assert_eq!(asked.every(), [waits(3, "beta", 100)]);
    // Another chat's ask about it is that chat's own question.
    assert!(asked_in(dir.path(), &asked, 4, "beta", 300).1);
    assert_eq!(asked.every().len(), 2);
}

#[test]
fn a_name_that_is_no_workspace_of_this_project_asks_the_person_nothing() {
    let dir = a_project();
    let asked = Asked::default();
    for name in [
        "gamma",
        "../beta",
        "beta/..",
        "/etc",
        "",
        "be ta",
        "beta\nRemove alpha",
    ] {
        let (said, news) = asked_in(dir.path(), &asked, 3, name, 100);
        assert!(!news, "{name:?}");
        assert!(
            said.contains("the person was not asked"),
            "{name:?}: {said}"
        );
    }
    assert_eq!(asked.every(), []);
}

#[test]
fn a_chat_holds_a_few_questions_at_most_and_the_oldest_goes_first() {
    let dir = tempfile::tempdir().expect("a project");
    let names: Vec<String> = (0..=AT_MOST_PER_CHAT).map(|n| format!("ws{n}")).collect();
    for name in &names {
        std::fs::create_dir_all(dir.path().join("workspaces").join(name)).expect("a workspace");
    }
    let asked = Asked::default();
    for (at, name) in names.iter().enumerate() {
        asked_in(dir.path(), &asked, 3, name, at as u64);
    }
    let held: Vec<String> = asked.every().into_iter().map(|one| one.workspace).collect();
    assert_eq!(held, names[1..]);
}

#[test]
fn an_answer_puts_one_question_away_and_a_closed_chat_takes_its_own() {
    let asked = Asked::default();
    asked.note(3, "alpha", 1);
    asked.note(3, "beta", 2);
    asked.note(4, "beta", 3);

    assert!(asked.answered(3, "beta"));
    assert!(!asked.answered(3, "beta"), "answered once");
    assert!(asked.holds(3, "alpha") && asked.holds(4, "beta"));

    asked.forget(3);
    assert_eq!(asked.every(), [waits(4, "beta", 3)]);
}

#[test]
fn a_workspace_that_is_gone_takes_every_question_about_it() {
    let asked = Asked::default();
    asked.note(3, "alpha", 1);
    asked.note(3, "beta", 2);
    asked.note(4, "beta", 3);

    // Removed in the window: the chats that asked are the ones to tell.
    assert_eq!(asked.gone("beta"), [3, 4]);
    assert_eq!(asked.every(), [waits(3, "alpha", 1)]);
    assert_eq!(asked.gone("beta"), Vec::<u32>::new());

    // Removed elsewhere: dropped when the list is read against the project's own listing.
    assert!(asked.only(&|workspace| workspace != "alpha"));
    assert_eq!(asked.every(), []);
    assert!(!asked.only(&|_| true));
}

#[test]
fn both_answers_are_the_window_s_alone() {
    // No chat, and no client but the window, reaches either: a chat never answers its own ask.
    for command in ["remove_asked_workspace", "keep_asked_workspace"] {
        assert!(
            purlis_session_protocol::ui::WINDOW_ONLY.contains(&command),
            "{command} is the window's alone"
        );
    }
}
