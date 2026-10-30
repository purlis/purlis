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

/// The longest a clone the app runs for this chat may show no life before this command stops
/// waiting on it (#4080). Not a bound on the whole clone: the app says the clone is alive
/// whenever git says something ([`hookwire::Answer::Alive`]), and stops a git that has said
/// nothing for [`purlis_core::cloning::STALL`] itself. Above that, so the app's own sentence for
/// a stalled clone reaches the chat, with git's network limit for the steps around a clone.
pub const A_CLONE_IS_SILENT_AT_MOST: Duration =
    Duration::from_secs(purlis_core::cloning::STALL.as_secs() + git::NETWORK.as_secs());

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
                // A clone's wait follows its liveness (#4080).
                alive: matches!(work, GitWork::Clone { .. } | GitWork::Create { .. }),
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
mod tests;
