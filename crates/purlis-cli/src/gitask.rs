// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis clone` and `purlis worktree add` in a chat the app started: the work handed to the
//! app over the chat's hook socket, which runs the same core functions and answers every line
//! they said (#1335, ADR 0067 §2). A sandboxed chat may not write a clone's `.git/config`, its
//! hooks or the editor settings a checkout carries; the app does.
//!
//! **Forwarded whenever the command runs in a chat the app started** (`$PURLIS_HOOK_SOCKET` and
//! the chat's number), sandboxed or not, so a chat gets one behaviour (D-2). Outside a chat, or
//! where no app takes the ask, the command runs here as it always has.

use std::path::{Path, PathBuf};
use std::time::Duration;

use purlis_core::gitbroker::commit as broker;
use purlis_core::hookwire::{self, CommitAsk, FolderName, GitAsk, GitWork, SOCKET_ENV, Stage};
use purlis_core::repocmd::Say;
use purlis_core::worktree::git;

/// What became of a git action handed to the app.
pub enum Forwarded {
    /// The app ran it: the lines it said, to print as this process would have, and its status.
    Said { lines: Vec<Say>, code: u8 },
    /// The app read it and will not run it: the sentence says why.
    Refused(String),
    /// No app took it, so this process runs it.
    NotTaken,
    /// The app took it and did not answer in time: it may have run, so nothing runs here.
    Unsure(String),
}

/// How long the app may take to clone `repos` repos: git's own limit for each round of
/// [`purlis_core::repocmd::clone::WORKERS`] clones at once, and one more for the wiring and the
/// manifest after them (D-6). Above git's limit, so the asker never gives up on a clone git is
/// still allowed to finish.
pub fn a_clone_takes_at_most(repos: usize) -> Duration {
    let rounds = repos.max(1).div_ceil(purlis_core::repocmd::clone::WORKERS);
    git::NETWORK * u32::try_from(rounds + 1).unwrap_or(u32::MAX)
}

/// How long the app may take to cut a worktree: several local git calls, each held to
/// [`git::READ`] (D-6).
pub const A_WORKTREE_TAKES_AT_MOST: Duration = Duration::from_secs(4 * git::READ.as_secs());

/// Hands `work` in `workspace` to the app that started this chat, or answers
/// [`Forwarded::NotTaken`] where there is none.
pub fn forwarded(workspace: &str, work: GitWork, within: Duration) -> Forwarded {
    asked(
        |chat| {
            hookwire::Ask::Git(Box::new(GitAsk {
                chat,
                workspace: workspace.to_owned(),
                work,
            }))
        },
        within,
    )
}

/// Hands the app that started this chat the ask `of` makes for the chat's number, or answers
/// [`Forwarded::NotTaken`] where there is none.
fn asked(of: impl FnOnce(u32) -> hookwire::Ask, within: Duration) -> Forwarded {
    let socket = purlis_core::envvar::var_os(SOCKET_ENV).filter(|s| !s.is_empty());
    let (Some(socket), Some(chat)) = (socket, crate::session::chat_number()) else {
        return Forwarded::NotTaken;
    };
    let Ok(mut asking) = hookwire::Asking::on(Path::new(&socket), hookwire::ChatToken::from_env())
    else {
        return Forwarded::NotTaken;
    };
    answered(asking.ask(&of(chat), within))
}

/// How long the app may take to update or push a branch folder: a fetch and a push each held
/// to git's network limit, and a rebase and a handful of local calls (qw387).
pub const A_PIECE_UPDATE_TAKES_AT_MOST: Duration =
    Duration::from_secs(3 * git::NETWORK.as_secs() + 8 * git::READ.as_secs());

/// How long the app may take to commit: staging and the commit by their own deadlines, and a
/// handful of local git calls and the scan, each held to [`git::READ`]. Above what the app's
/// git may take, so the asker never gives up on a commit git is still allowed to finish.
pub const A_COMMIT_TAKES_AT_MOST: Duration = Duration::from_secs(
    broker::STAGING.as_secs() + broker::COMMITTING.as_secs() + 12 * git::READ.as_secs(),
);

/// What `purlis worktree commit` was asked, as the ask it sends, or the sentence that refuses
/// the spelling before anything is sent.
pub fn commit_ask(
    message: Option<String>,
    all: bool,
    amend: bool,
    paths: Vec<String>,
) -> Result<(String, Stage), String> {
    if amend {
        return Err(broker::COMMITS_ONLY.to_owned());
    }
    let message = message.ok_or("give the commit message with -m")?;
    broker::message_ok(&message)?;
    let stage = match (all, paths.is_empty()) {
        (true, true) => Stage::Tracked,
        (false, false) => {
            broker::paths_ok(&paths)?;
            Stage::Paths(paths)
        }
        (true, false) => {
            return Err(
                "--all stages every change to a tracked file: pass it, or name paths, not both"
                    .to_owned(),
            );
        }
        (false, true) => {
            return Err(
                "name what to commit: paths, or --all for every change to a tracked file"
                    .to_owned(),
            );
        }
    };
    Ok((message, stage))
}

/// Where `purlis worktree commit` runs (#3927): the project at `root` and the folder `cwd`.
pub struct RunIn<'a> {
    pub root: &'a Path,
    pub cwd: &'a Path,
}

impl RunIn<'_> {
    /// The branch folder the command runs in, by its names, and its top and where the command
    /// stands, as the kernel names them; `None` outside one.
    fn folder(&self) -> Option<(FolderName, PathBuf, PathBuf)> {
        let at = purlis_core::worktree::locate(self.root, self.cwd)?;
        let top = purlis_core::worktree::path_for(self.root, &at.workspace, &at.repo, &at.piece)
            .ok()
            .and_then(|top| std::fs::canonicalize(top).ok())?;
        let here = std::fs::canonicalize(self.cwd).ok()?;
        Some((
            FolderName {
                workspace: at.workspace,
                repo: at.repo,
                piece: at.piece,
            },
            top,
            here,
        ))
    }
}

/// `paths`, named from `here` in the branch folder `named` whose top is `top`, as paths
/// relative to that top, as git takes them there (#3927): `..` within the folder, a path
/// written whole inside it, and one spelled from the workspace or the project
/// (`.worktrees/<repo>/<piece>/…`, `workspaces/<ws>/.worktrees/<repo>/<piece>/…`) that names
/// this very folder. A path that leaves the folder is refused, naming the spellings that work.
pub fn relative_to_the_folder(
    top: &Path,
    here: &Path,
    named: &FolderName,
    paths: Vec<String>,
) -> Result<Vec<String>, String> {
    use std::path::Component;
    let below = here.strip_prefix(top).unwrap_or(Path::new(""));
    let from_workspace = Path::new(purlis_core::worktree::DIR_NAME)
        .join(&named.repo)
        .join(&named.piece);
    let from_project = Path::new("workspaces")
        .join(&named.workspace)
        .join(&from_workspace);
    let outside = |path: &str| {
        format!(
            "the path {path:?} is outside branch folder {} · {}: name paths relative to the \
             branch folder, running the command inside it or with `env -C <branch folder>`, or \
             from the workspace as `{}/<path>`. Nothing was committed",
            purlis_core::shown::short(&named.repo),
            purlis_core::shown::short(&named.piece),
            from_workspace.display()
        )
    };
    paths
        .into_iter()
        .map(|path| {
            let given = Path::new(&path);
            let from_top = if given.is_absolute() {
                given
                    .strip_prefix(top)
                    .map_err(|_| outside(&path))?
                    .to_path_buf()
            } else if let Ok(rest) = given
                .strip_prefix(&from_workspace)
                .or_else(|_| given.strip_prefix(&from_project))
            {
                rest.to_path_buf()
            } else {
                below.join(given)
            };
            let mut parts: Vec<String> = Vec::new();
            for part in from_top.components() {
                match part {
                    Component::Normal(name) => parts.push(name.to_string_lossy().into_owned()),
                    Component::CurDir => {}
                    Component::ParentDir => {
                        parts.pop().ok_or_else(|| outside(&path))?;
                    }
                    Component::RootDir | Component::Prefix(_) => return Err(outside(&path)),
                }
            }
            Ok(if parts.is_empty() {
                ".".to_owned()
            } else {
                parts.join("/")
            })
        })
        .collect()
}

/// `purlis worktree commit`: hands the commit to the app that started this sandboxed chat
/// (#1055), prints what it answers, and answers the exit status. Nothing is committed by this
/// process: a chat that is not sandboxed is told to use git. Run inside a branch folder, its
/// paths are taken from where it runs there and the ask names that folder (#3927).
pub fn commit(
    run_in: &RunIn<'_>,
    message: Option<String>,
    all: bool,
    amend: bool,
    paths: Vec<String>,
    say: &mut dyn FnMut(Say),
) -> u8 {
    let folder = run_in.folder();
    let paths = match &folder {
        Some((named, top, here)) => match relative_to_the_folder(top, here, named, paths) {
            Ok(paths) => paths,
            Err(why) => {
                say(Say::Fail(why.into()));
                return 2;
            }
        },
        None => paths,
    };
    let (message, stage) = match commit_ask(message, all, amend, paths) {
        Ok(ask) => ask,
        Err(why) => {
            say(Say::Fail(why.into()));
            return 2;
        }
    };
    if !purlis_core::sandbox::chat_is_sandboxed() {
        say(Say::Fail(broker::PLAIN_GIT.into()));
        return 1;
    }
    let folder = folder.map(|(named, _, _)| named);
    let forwarded = asked(
        |chat| {
            hookwire::Ask::Commit(Box::new(CommitAsk {
                chat,
                message,
                stage,
                folder,
            }))
        },
        A_COMMIT_TAKES_AT_MOST,
    );
    told(forwarded, say).unwrap_or_else(|| {
        say(Say::Fail(
            "the app that started this chat did not take the commit: it may be older than \
             this command. Nothing was committed. Ask the person to update purlis, or to \
             commit from their own terminal"
                .into(),
        ));
        1
    })
}

/// What the app's answer, or the way asking failed, makes of a git action handed to it.
///
/// `purlis session record`'s rule (#1332): **only a connection the app ended at once is not
/// taken** — an app older than the ask drops a line it cannot read — and then this process
/// runs the command itself. A connection that went quiet is an app that may be cloning now,
/// and cloning here as well would race it into the same directory, so nothing runs here.
fn answered(answer: std::io::Result<hookwire::Answer>) -> Forwarded {
    use std::io::ErrorKind;
    match answer {
        Ok(hookwire::Answer::Said { lines, code }) => Forwarded::Said { lines, code },
        Ok(hookwire::Answer::No { why }) if why == hookwire::NOTHING_ANSWERS => Forwarded::NotTaken,
        Ok(hookwire::Answer::No { why }) => Forwarded::Refused(why),
        Ok(_) => Forwarded::NotTaken,
        Err(e)
            if matches!(
                e.kind(),
                ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe | ErrorKind::ConnectionReset
            ) =>
        {
            Forwarded::NotTaken
        }
        Err(e) => Forwarded::Unsure(format!(
            "the app took this and did not answer ({e}), so it may still be running it. Nothing \
             was run again here: `purlis status` shows what is in the workspace now."
        )),
    }
}

/// Prints what the app answered with `say`, the printer this command's own run uses, and
/// answers the exit status. A refusal and an unsure answer are failures said in its voice.
pub fn told(forwarded: Forwarded, say: &mut dyn FnMut(Say)) -> Option<u8> {
    match forwarded {
        Forwarded::Said { lines, code } => {
            for line in lines {
                say(line);
            }
            Some(code)
        }
        Forwarded::Refused(why) => {
            say(Say::Fail(
                format!("the app did not run this for the chat: {why}").into(),
            ));
            Some(1)
        }
        Forwarded::Unsure(why) => {
            say(Say::Fail(why.into()));
            Some(1)
        }
        Forwarded::NotTaken => None,
    }
}

#[cfg(test)]
mod tests {
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
        let placed =
            |here: &Path, of: &[&str]| relative_to_the_folder(top, here, &folder(), paths(of));
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
            let why =
                relative_to_the_folder(top, &here, &folder(), vec![path.to_owned()]).unwrap_err();
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
            crate::Cli::try_parse_from(["purlis", "worktree", "commit", "-m", "x", "--", "-n"])
                .is_ok()
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
}
