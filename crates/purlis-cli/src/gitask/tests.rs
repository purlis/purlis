// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::io::{Error, ErrorKind};

use super::*;

fn folder() -> FolderName {
    FolderName {
        workspace: "alpha".to_owned(),
        repo: "thing".to_owned(),
        piece: "piece".to_owned(),
    }
}

#[test]
fn a_path_in_the_branch_folder_is_taken_from_where_the_command_runs_in_it() {
    // #3927: `env -C <folder>`, or a shell standing in it or below it.
    let top = Path::new("/p/workspaces/alpha/.worktrees/thing/piece");
    let paths = |of: &[&str]| of.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>();
    let placed = |here: &Path, of: &[&str]| relative_to_the_folder(top, here, &folder(), paths(of));
    assert_eq!(
        placed(top, &["crates/a.rs", "./b", "-n"]),
        Ok(paths(&["crates/a.rs", "b", "-n"]))
    );
    assert_eq!(
        placed(&top.join("crates"), &["a.rs", "../README.md", "."]),
        Ok(paths(&["crates/a.rs", "README.md", "crates"]))
    );
    assert_eq!(placed(top, &["."]), Ok(paths(&["."])));
    // Spelled from the workspace or the project, naming this very folder.
    assert_eq!(
        placed(
            &top.join("crates"),
            &[
                ".worktrees/thing/piece/crates/a.rs",
                "workspaces/alpha/.worktrees/thing/piece/b"
            ]
        ),
        Ok(paths(&["crates/a.rs", "b"]))
    );
    // Written whole, inside it.
    assert_eq!(
        placed(
            top,
            &["/p/workspaces/alpha/.worktrees/thing/piece/src/x.rs"]
        ),
        Ok(paths(&["src/x.rs"]))
    );
    assert_eq!(placed(top, &[]), Ok(Vec::new()));
}

#[test]
fn a_path_outside_the_branch_folder_is_refused_naming_the_spelling_that_works() {
    let top = Path::new("/p/workspaces/alpha/.worktrees/thing/piece");
    for (here, path) in [
        (top.to_path_buf(), "../other/a.rs"),
        (top.join("src"), "../../a.rs"),
        (top.to_path_buf(), "/p/workspaces/alpha/README.md"),
    ] {
        let why = relative_to_the_folder(top, &here, &folder(), vec![path.to_owned()]).unwrap_err();
        assert!(why.contains("thing · piece"), "{path}: {why}");
        assert!(
            why.contains("relative to the branch folder"),
            "{path}: {why}"
        );
        assert!(
            why.contains(".worktrees/thing/piece/<path>"),
            "{path}: {why}"
        );
    }
}

#[test]
fn a_message_that_starts_with_two_dashes_is_the_message() {
    use clap::Parser as _;
    for words in [
        vec![
            "purlis",
            "worktree",
            "commit",
            "-m",
            "--why it changed",
            "--all",
        ],
        vec![
            "purlis",
            "worktree",
            "commit",
            "--message",
            "--why it changed",
            "a",
        ],
    ] {
        let crate::Command::Worktree(crate::piece::WorktreeCommand::Commit { message, .. }) =
            crate::Cli::try_parse_from(&words).expect("parses").command
        else {
            panic!("{words:?} parsed as another command");
        };
        assert_eq!(message.as_deref(), Some("--why it changed"), "{words:?}");
    }
}

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
    assert!(A_CLONE_IS_SILENT_AT_MOST > purlis_core::cloning::STALL);
    assert!(A_CLONE_IS_SILENT_AT_MOST > purlis_core::hookwire::alive::EVERY * 2);
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
