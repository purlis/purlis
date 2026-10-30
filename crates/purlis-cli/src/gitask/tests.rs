// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::io::{Error, ErrorKind};

use super::*;

#[test]
fn an_app_that_ends_the_connection_at_once_did_not_take_it_and_a_quiet_one_may_have() {
    for kind in [
        ErrorKind::UnexpectedEof,
        ErrorKind::BrokenPipe,
        ErrorKind::ConnectionReset,
    ] {
        assert!(matches!(
            answered(Err(Error::from(kind))),
            Forwarded::NotTaken
        ));
    }
    assert!(matches!(
        answered(Err(Error::from(ErrorKind::WouldBlock))),
        Forwarded::Unsure(_)
    ));
    assert!(matches!(
        answered(Ok(hookwire::Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned()
        })),
        Forwarded::NotTaken
    ));
}

#[test]
fn a_commit_is_asked_for_by_message_and_paths_and_other_spellings_are_refused() {
    let paths = |of: &[&str]| of.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        commit_ask(Some("one".to_owned()), true, false, Vec::new()),
        Ok(("one".to_owned(), Stage::Tracked))
    );
    assert_eq!(
        commit_ask(Some("one".to_owned()), false, false, paths(&["a", "-n"])),
        Ok(("one".to_owned(), Stage::Paths(paths(&["a", "-n"]))))
    );
    // An amend is refused in the sentence that names everything this never does.
    let amend = commit_ask(Some("one".to_owned()), true, true, Vec::new()).unwrap_err();
    assert_eq!(amend, broker::COMMITS_ONLY);
    for (message, all, of) in [
        (None, true, vec![]),
        (Some(" "), true, vec![]),
        (Some("one"), false, vec![]),
        (Some("one"), true, vec!["a"]),
        (Some("one"), false, vec!["../a"]),
        (Some("one"), false, vec![".git/config"]),
    ] {
        assert!(
            commit_ask(message.map(str::to_owned), all, false, paths(&of)).is_err(),
            "{message:?} {all} {of:?}"
        );
    }
}

#[test]
fn the_command_has_no_spelling_for_a_reset_or_a_forced_push() {
    use clap::Parser as _;
    for words in [
        vec!["purlis", "worktree", "reset"],
        vec!["purlis", "worktree", "push", "--force"],
        vec!["purlis", "worktree", "push", "-f"],
        vec!["purlis", "worktree", "push", "origin", "main"],
        vec!["purlis", "worktree", "update", "--continue", "--abort"],
        vec!["purlis", "worktree", "update", "--base", "main", "--abort"],
        vec![
            "purlis", "worktree", "commit", "-m", "x", "--all", "--reset",
        ],
        vec!["purlis", "worktree", "commit", "-m", "x", "--all", "--push"],
        vec![
            "purlis",
            "worktree",
            "commit",
            "-m",
            "x",
            "--all",
            "--no-verify",
        ],
        vec![
            "purlis", "worktree", "commit", "-m", "x", "--all", "--branch", "main",
        ],
        vec![
            "purlis", "worktree", "commit", "-m", "x", "--all", "--author", "a",
        ],
    ] {
        assert!(crate::Cli::try_parse_from(&words).is_err(), "{words:?}");
    }
    assert!(
        crate::Cli::try_parse_from(["purlis", "worktree", "commit", "-m", "x", "--", "-n"]).is_ok()
    );
    for words in [
        vec!["purlis", "worktree", "push"],
        vec!["purlis", "worktree", "push", "--force-with-lease"],
        vec!["purlis", "worktree", "update"],
        vec!["purlis", "worktree", "update", "--base", "main"],
        vec!["purlis", "worktree", "update", "--continue"],
        vec!["purlis", "worktree", "update", "--abort"],
        vec!["purlis", "worktree", "add", "r", "p", "--base", "HEAD"],
    ] {
        assert!(crate::Cli::try_parse_from(&words).is_ok(), "{words:?}");
    }
}

#[test]
fn the_wait_is_longer_than_git_may_take() {
    assert!(a_clone_takes_at_most(1) > git::NETWORK);
    assert!(a_clone_takes_at_most(purlis_core::repocmd::clone::WORKERS + 1) > git::NETWORK * 2);
    assert!(A_WORKTREE_TAKES_AT_MOST > git::READ);
    assert!(A_COMMIT_TAKES_AT_MOST > broker::STAGING + broker::COMMITTING);
}

#[test]
fn what_the_app_said_is_printed_line_for_line_and_its_status_is_the_commands() {
    let mut printed = Vec::new();
    let code = told(
        Forwarded::Said {
            lines: vec![
                Say::Info("a".to_owned().into()),
                Say::Fail("b".to_owned().into()),
            ],
            code: 2,
        },
        &mut |line| printed.push(line),
    );
    assert_eq!(code, Some(2));
    assert_eq!(
        printed,
        [
            Say::Info("a".to_owned().into()),
            Say::Fail("b".to_owned().into())
        ]
    );
    assert_eq!(told(Forwarded::NotTaken, &mut |_| {}), None);
}
