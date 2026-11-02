// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A commit the app makes for a sandboxed chat in the branch folder it stands in: **a brokered
//! git action** (ADR 0067 §2 as amended on 2026-10-08, #1055, ruling V99h).
//!
//! A linked worktree keeps its index, its HEAD and its objects in its clone's `.git`, outside
//! the folder a chat may write, so `git add` and `git commit` are refused there. The worktree's
//! git directory is not opened to the chat. `purlis worktree commit` hands the app a message
//! and what to stage ([`crate::hookwire::CommitAsk`]), and the app stages and commits, outside
//! the sandbox.
//!
//! # What the app takes from the ask, and what it does not
//!
//! The message and the paths. **Where** is the app's own record of the chat, never the ask:
//! the folder the chat stands in, or for a chat standing above branch folders the one below it
//! that its paths name (`standing`, #1476), must be one purlis cut
//! (`<workspace>/.worktrees/<repo>/<piece>`),
//! its `.git` file must name that clone's worktree and be named back ([`link`]), and every git
//! call is given that git directory and work tree. **Which branch** is the one that folder is
//! on. **Who** is the identity the repository's own config gives, or the person's, and the
//! trailers are the chat's (ADR 0074).
//!
//! # What git may run
//!
//! Nothing but git. No global or system config is read, hooks and the file-system monitor are
//! off, the commit opens no editor and signs nothing, and git starts no git of its own inside a
//! submodule. A repository whose config names a program is refused before any git runs in it
//! ([`super::runs_a_program`]); so is one with a `config.worktree` in the worktree's git
//! directory, one with an `objects/info/alternates`, and one whose config signs its commits. Nothing else of the repository is refused: its
//! `info/attributes` is read as git reads it, and can name no program without a config that
//! defines one. So the repository's own `pre-commit` does not run, and the answer says so
//! each time one is there. purlis's own scan of what the commit would publish does run, here.
//!
//! # What staging reads
//!
//! The chat wrote the folder. A path is taken only as a name inside it: relative, no `..`, no
//! `.git`, none reached through a link. git stores a link as a link and never reads what it
//! points at. Every call is held to the checked git directory wherever in the folder the
//! chat stands, so nothing is found again from the folder after the checks. What was staged
//! is then read back, and the commit is unstaged and refused where it would record another
//! repository (a gitlink), change `.gitmodules`, add a name that differs from another only
//! by case, or hold a file its attributes mark for a content filter: the person's filters
//! (an LFS one, most often) live in the config this commit does not read, so the file would
//! be stored as it is on disk where their own git would store the filtered form.
//!
//! A commit holds itself to the plane's save's rule of one file, one name
//! (`planegit::ondisk`): a named file with a second name (a hard link) is refused before
//! anything is staged, and each file is staged from one open descriptor, asked that it is a
//! regular file with one name, so the bytes stored are the bytes of the file that was checked
//! and nothing is read again by its name. Once staged, a row that is not the file on disk now,
//! with one name, refuses the commit, is unstaged, and its blob is taken out of the object
//! store where it is a loose object nothing else reaches.
//!
//! # One at a time, and never left half done
//!
//! One commit runs in a branch folder at a time. Before it stages, the app leaves a note in
//! the worktree's git directory, which no chat writes, and takes it away when it has
//! committed or put the index back. An app that stopped in between is found out by the next
//! ask: what is staged is what that note says the app staged, so it is unstaged and the ask
//! carries on. Staged changes that are not the app's are someone's, and are left alone.
//!
//! # What it never does
//!
//! Amend, reset, rebase, merge, push, or commit on another branch or in another folder. The
//! ask has no word for any of them.

use std::path::{Component, Path, PathBuf};

use super::Asker;
use crate::counts::counted;
use crate::hookwire::{Answer, CommitAsk, FolderName, Stage};
use crate::planegit::ondisk;
use crate::repocmd::Say;
use crate::worktree::{self, git, link};

/// The longest message taken, in bytes: far past any commit message, and under what one ask
/// carries.
pub const MOST_MESSAGE: usize = 32 * 1024;

/// The most paths one ask names.
pub const MOST_PATHS: usize = 1000;

/// The longest one path, in bytes: the longest a path may be on the systems purlis runs on.
pub const MOST_PATH: usize = 4096;

/// What every refusal of a spelling the command does not have says.
pub const COMMITS_ONLY: &str = "purlis commits for a sandboxed chat and does nothing else to \
     history: it does not amend, reset, rebase, merge or push. Make a new commit, or ask the \
     person to run the command in their own terminal";

/// What a chat that is not sandboxed is told where its project turned the refusal on
/// ([`refuses_unsandboxed`]).
pub const PLAIN_GIT: &str = "this chat is not sandboxed, and this project's \
     `[plane] refuse_unsandboxed_commit` says such a chat commits with git: `git add` and `git \
     commit` work here as they are, so use them";

/// Whether the project at `root` turned on the refusal of [`PLAIN_GIT`] (#4534): by default a
/// chat with no sandbox commits with `purlis worktree commit` as any other does, since the
/// harness's own rules may refuse its plain git.
pub fn refuses_unsandboxed(root: &Path) -> bool {
    crate::planesave::Settings::read(root)
        .plane
        .refuse_unsandboxed_commit
        .value
}

/// How long git may take to stage: it reads every named file, so a read's deadline is not
/// its deadline.
pub const STAGING: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// How long git may take to write the commit.
pub const COMMITTING: std::time::Duration = std::time::Duration::from_secs(2 * 60);

/// The note the app leaves in a worktree's git directory while it commits there: empty until
/// staging ends, then the id of the tree it staged.
const NOTE: &str = "purlis-brokered-commit";

/// The commit itself, its message on standard input. Told on its own command line to start no
/// editor (`--no-edit`), no signer and no upkeep after it, whatever the repository's
/// configuration says (#1476). Its hooks are off as every call's are (the hooks folder), and a
/// hook a configuration defines is refused before any git runs ([`super::runs_a_program`]).
const COMMIT: [&str; 10] = [
    "-c",
    "gc.auto=0",
    "-c",
    "maintenance.auto=false",
    "commit",
    "--quiet",
    "--no-edit",
    "--no-gpg-sign",
    "--cleanup=whitespace",
    "--file=-",
];

/// The repository's hooks a commit would have run, which a brokered one does not.
const HOOKS: [&str; 4] = [
    "pre-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
];

/// Commits for `asker` in the project at `root` what `ask` names, reading the person's
/// identity from `identity` and nothing else of their config, and answers the new commit, or
/// one sentence saying why not.
pub fn answer(root: &Path, asker: &Asker, ask: &CommitAsk, identity: &git::Isolated) -> Answer {
    answer_telling(root, asker, ask, identity).0
}

/// [`answer`], and the line of a needs-you item where purlis's scan refused the commit (#1476):
/// what a chat's own `pre-commit` tells the app when it refuses one
/// ([`crate::hookwire::CommitRefused`]), masked the same way.
pub fn answer_telling(
    root: &Path,
    asker: &Asker,
    ask: &CommitAsk,
    identity: &git::Isolated,
) -> (Answer, Option<String>) {
    match commit(root, asker, ask, identity, std::time::Instant::now()) {
        Ok(lines) => (
            Answer::Said {
                lines: super::fitted(lines, super::SAID_AT_MOST),
                code: 0,
            },
            None,
        ),
        Err(Refused { why, scan }) => (Answer::No { why }, scan),
    }
}

/// Why a commit was not made, and the needs-you line where the scan refused it.
#[derive(Debug)]
struct Refused {
    why: String,
    scan: Option<String>,
}

impl From<String> for Refused {
    fn from(why: String) -> Self {
        Self { why, scan: None }
    }
}

impl From<&str> for Refused {
    fn from(why: &str) -> Self {
        why.to_owned().into()
    }
}

fn commit(
    root: &Path,
    asker: &Asker,
    ask: &CommitAsk,
    identity: &git::Isolated,
    now: std::time::Instant,
) -> Result<Vec<Say>, Refused> {
    // The command commits by itself where its chat has no sandbox ([`plain`]), so what is asked
    // here with no sandbox on the app's record is a chat started in one the project has since
    // turned off, or at the floor. It is made as any other brokered commit, unless refused.
    if !sandboxed(root, asker)? && refuses_unsandboxed(root) {
        return Err(PLAIN_GIT.into());
    }
    message_ok(&ask.message)?;
    trailers_ok(&ask.message)?;
    if let Stage::Paths(paths) = &ask.stage {
        paths_ok(paths)?;
    }
    let cwd = asker.cwd.as_deref().ok_or(NO_RECORD)?;
    let (folder, stage) = match &ask.folder {
        Some(named) => named_folder(root, cwd, named, &ask.stage)?,
        None => {
            if let Stage::Paths(paths) = &ask.stage {
                for path in paths {
                    through_no_link(cwd, path)?;
                }
            }
            standing(root, cwd, &ask.stage)?
        }
    };
    let ask = CommitAsk {
        stage,
        ..ask.clone()
    };
    let here = Here::of(root, Some(&folder))?;
    paced(root, asker.chat, now)?;
    // One live writer per branch folder (#3901): a second chat's commit is refused, naming the
    // chat that holds it; a chat committing where nobody live holds the folder takes it.
    // `standing` answers only a folder purlis cut, so one it cannot place is refused, never
    // committed in unclaimed (fail closed).
    let at = worktree::locate(root, &folder).ok_or(NOT_PLACED)?;
    crate::piececlaim::take(
        root,
        &at,
        &super::who(asker),
        asker.live.as_deref(),
        crate::piececlaim::Act::Commit,
        chrono::Utc::now(),
    )?;
    // One commit at a time in a branch folder: a second ask waits for the first.
    let lock = lock_of(&here.git_dir);
    let _held = lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // Every call below reads the checked git directory, and no config but the repository's.
    let plain = git::Isolated::default().pinned(&here.top, &here.git_dir);
    git::isolated(&plain, || -> Result<Vec<Say>, Refused> {
        super::runs_a_program(&here.clone)?;
        made(root, asker.chat, &ask, &here, identity)
    })
}

/// What a commit is answered where purlis cannot say which branch folder it would be made in,
/// and so which chat holds it (#3901).
const NOT_PLACED: &str = "purlis cannot tell which of its branch folders this is, so it cannot \
    tell whether another chat works there, and will not commit in it. Nothing was committed";

/// What a commit asked of a chat the app has no folder for is answered.
const NO_RECORD: &str =
    "the app has no record of where this chat stands, so it will not commit for it";

/// Where the commit is made, and what is staged there, for a chat whose recorded folder is
/// `cwd` (#1476).
///
/// A chat standing in a branch folder commits in it, as the ask names it. A chat standing
/// above branch folders (a workspace, or the project's top) commits in the one branch folder
/// **below its own** that holds every path it names: each path's first parts name that folder
/// (`.worktrees/<repo>/<piece>/…`), and the rest is the path inside it. The ask still names
/// no folder: the folder is read from the paths, and held to the app's record of where the chat
/// stands, which it must be below. `--all` names no path, so it is refused there.
fn standing(root: &Path, cwd: &Path, stage: &Stage) -> Result<(PathBuf, Stage), String> {
    if worktree::locate(root, cwd).is_some() {
        return Ok((cwd.to_path_buf(), stage.clone()));
    }
    let Stage::Paths(paths) = stage else {
        return Err(format!(
            "{NOT_IN_A_BRANCH_FOLDER} From a folder above branch folders, name the paths to \
             commit from where the chat stands (`.worktrees/<repo>/<piece>/<path>`), or name the \
             branch folder itself for every change in it; `--all` names no folder"
        ));
    };
    let real = std::fs::canonicalize(cwd).map_err(|_| NOT_IN_A_BRANCH_FOLDER.to_owned())?;
    let workspaces = std::fs::canonicalize(root.join("workspaces"))
        .map_err(|_| NOT_IN_A_BRANCH_FOLDER.to_owned())?;
    let mut folder: Option<PathBuf> = None;
    let mut inside = Vec::with_capacity(paths.len());
    for path in paths {
        let (its, rest) = branch_folder_of(&workspaces, &real, path).ok_or_else(|| {
            format!(
                "{NOT_IN_A_BRANCH_FOLDER} The path {} is not inside a branch folder below it: \
                 {ACCEPTED}",
                shown(path)
            )
        })?;
        match &folder {
            Some(one) if *one != its => {
                return Err(
                    "the paths name more than one branch folder. One commit is made in one \
                     branch folder: ask once for each"
                        .to_owned(),
                );
            }
            _ => folder = Some(its),
        }
        inside.push(rest);
    }
    let folder = folder.ok_or("no path was named to stage")?;
    // Reached through no link: the folder the paths name is the one that is there.
    if std::fs::canonicalize(&folder).ok().as_deref() != Some(folder.as_path()) {
        return Err(format!(
            "{NOT_IN_A_BRANCH_FOLDER} The branch folder the paths name ({}) is reached through \
             a link, and purlis commits in no folder through a link",
            folder.display()
        ));
    }
    Ok((folder, Stage::Paths(inside)))
}

/// Where the commit is made for a command that ran in branch folder `named` (#3927), for a chat
/// whose recorded folder is `cwd`: that folder, with `stage`'s paths relative to its top.
///
/// The ask names the folder, so it is held to what a path naming it from where the chat stands
/// would be held to ([`standing`]): one purlis cut, reached through no link, and the chat's own
/// folder or one below it. A chat in another branch folder, or in another workspace, is refused.
fn named_folder(
    root: &Path,
    cwd: &Path,
    named: &FolderName,
    stage: &Stage,
) -> Result<(PathBuf, Stage), String> {
    let (repo, piece) = (
        crate::shown::short(&named.repo),
        crate::shown::short(&named.piece),
    );
    let refused = |why: &str| {
        format!(
            "{why}, so the app will not commit in branch folder {repo} · {piece} for it: nothing was committed"
        )
    };
    worktree::path_for(root, &named.workspace, &named.repo, &named.piece)
        .map_err(|why| refused(&why.to_string()))?;
    let workspaces = std::fs::canonicalize(root.join("workspaces"))
        .map_err(|_| refused("the project has no workspaces folder"))?;
    let folder = workspaces
        .join(&named.workspace)
        .join(worktree::DIR_NAME)
        .join(&named.repo)
        .join(&named.piece);
    match std::fs::canonicalize(&folder) {
        Ok(real) if real == folder => {}
        Ok(_) => {
            return Err(refused(
                "that branch folder is reached through a link, and purlis commits in no folder \
                 through a link",
            ));
        }
        Err(_) => return Err(refused("that branch folder is not there")),
    }
    let here = std::fs::canonicalize(cwd)
        .map_err(|_| refused("the app could not read where this chat stands"))?;
    if !here.starts_with(&folder) && !folder.starts_with(&here) {
        return Err(refused(&format!(
            "this chat stands in {}, which is neither that folder nor above it",
            here.strip_prefix(&workspaces).unwrap_or(&here).display()
        )));
    }
    if let Stage::Paths(paths) = stage {
        for path in paths {
            through_no_link(&folder, path)?;
        }
    }
    Ok((folder, stage.clone()))
}

/// The branch folder below `cwd` that `path`, named from `cwd`, is in, and the path inside it
/// (`.` for the folder itself), by the names alone: `<workspaces>/<ws>/.worktrees/<repo>/<piece>`.
/// `cwd` and `workspaces` are as the kernel names them; `path` is relative, with no `..`
/// ([`paths_ok`]).
fn branch_folder_of(workspaces: &Path, cwd: &Path, path: &str) -> Option<(PathBuf, String)> {
    let named = cwd.join(path);
    let parts: Vec<&std::ffi::OsStr> = named
        .strip_prefix(workspaces)
        .ok()?
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    let [ws, dir, repo, piece, rest @ ..] = parts.as_slice() else {
        return None;
    };
    if *dir != worktree::DIR_NAME {
        return None;
    }
    let folder = workspaces.join(ws).join(dir).join(repo).join(piece);
    // Below the chat's own folder, never the folder itself or above it.
    if !folder.starts_with(cwd) || folder == cwd {
        return None;
    }
    let rest: Vec<String> = rest
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    let rest = if rest.is_empty() {
        ".".to_owned()
    } else {
        rest.join("/")
    };
    Some((folder, rest))
}

/// How many commits one chat may ask the app for within [`COMMITS_WITHIN`] (#1476): a commit
/// a minute and more, with room for one refused and asked again. A chat that asks in a loop is
/// stopped, and told why, long before its asks are a load on the machine.
pub const COMMITS_A_WHILE: usize = 30;

/// The while [`COMMITS_A_WHILE`] is counted over.
pub const COMMITS_WITHIN: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Counts one commit `chat` of the project at `root` asked for at `now`, or says why it is
/// refused. Counted for this process's life, per project.
fn paced(root: &Path, chat: u32, now: std::time::Instant) -> Result<(), String> {
    static PACE: std::sync::LazyLock<
        std::sync::Mutex<std::collections::HashMap<PathBuf, std::sync::Arc<crate::brokered::Rate>>>,
    > = std::sync::LazyLock::new(Default::default);
    let rate = PACE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entry(root.to_path_buf())
        .or_insert_with(|| {
            std::sync::Arc::new(crate::brokered::Rate::new(COMMITS_A_WHILE, COMMITS_WITHIN))
        })
        .clone();
    pace_of(&rate, chat, now)
}

/// [`paced`], against `rate`.
fn pace_of(rate: &crate::brokered::Rate, chat: u32, now: std::time::Instant) -> Result<(), String> {
    if rate.took(chat, now) {
        return Ok(());
    }
    Err(format!(
        "chat {chat} has asked the app for {} commits in the last {} minutes, which is its cap; \
         nothing was committed. Wait a little, and commit fewer, larger changes",
        rate.most(),
        rate.per().as_secs() / 60
    ))
}

/// Why `message` carries a line of purlis's own provenance (`Purlis-Chat`, `Purlis-Persona`,
/// `Purlis-Change`, under any name they have had, in any case), or nothing (#1476). purlis
/// writes those for the commit it makes; one the chat wrote would claim what purlis did not
/// check, and a reader cannot tell the two apart.
pub fn trailers_ok(message: &str) -> Result<(), String> {
    match crate::provenance::claims_ours(message) {
        Some(key) => Err(format!(
            "the commit message carries a `{key}:` line. purlis writes its own provenance \
             trailers on a commit it makes for a chat, and one the chat wrote would claim what \
             purlis did not check: leave the line out"
        )),
        None => Ok(()),
    }
}

/// The lock of the branch folder whose git directory is `git_dir`, for this process's life.
pub(crate) fn lock_of(git_dir: &Path) -> std::sync::Arc<std::sync::Mutex<()>> {
    static FOLDERS: std::sync::LazyLock<
        std::sync::Mutex<std::collections::HashMap<PathBuf, std::sync::Arc<std::sync::Mutex<()>>>>,
    > = std::sync::LazyLock::new(Default::default);
    FOLDERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entry(git_dir.to_path_buf())
        .or_default()
        .clone()
}

/// The git half: stages and commits what `ask` names in the checked folder `here`, for chat
/// `chat` of the project at `root`. Run with the thread's git held to `here`'s git directory.
fn made(
    root: &Path,
    chat: u32,
    ask: &CommitAsk,
    here: &Here,
    identity: &git::Isolated,
) -> Result<Vec<Say>, Refused> {
    let theirs = identity.pinned(&here.top, &here.git_dir);
    here.borrows_nothing()?;
    let top = here.top.as_path();
    if said(
        top,
        &[
            "config",
            "--local",
            "--type=bool",
            "--get",
            "commit.gpgsign",
        ],
    )?
    .is_some_and(|value| value == "true")
    {
        return Err(
            "this repository signs its commits (`commit.gpgsign`), and purlis does not \
             sign for a sandboxed chat: a signer is a program that may ask for a \
             passphrase. Commit this one from your own terminal"
                .into(),
        );
    }
    if let Some(key) = its_signer(top)? {
        return Err(Refused::from(format!(
            "this repository names a signer (`{key}`), so its commits are meant to be signed, \
             and purlis does not sign for a sandboxed chat: a signer is a program that may ask \
             for a passphrase. Commit this one from your own terminal, or remove the key from \
             the repository's config if its commits need no signature"
        )));
    }
    let branch = said(top, &["symbolic-ref", "--quiet", "--short", "HEAD"])?.ok_or(
        "this folder is not on a branch (a detached HEAD), so there is no branch to \
         commit on",
    )?;
    here.mid_nothing()?;
    let before = ran(
        top,
        &["diff", "--cached", "--quiet", "--ignore-submodules=none"],
    )?;
    let mut lines = Vec::new();
    match before.code {
        Some(0) => here.forget(),
        Some(1) if here.left_by_an_interrupted_commit() => {
            unstage(top);
            here.forget();
            lines.push(Say::Info(
                "an earlier commit for this chat was interrupted after it staged; what it \
                 had staged was unstaged first, and the files are as they were"
                    .into(),
            ));
        }
        Some(1) => {
            return Err(
                "this folder has staged changes that purlis did not stage for a chat. Commit \
                 or unstage them in your own terminal first"
                    .into(),
            );
        }
        _ => {
            return Err(format!(
                "git could not read this folder: {}",
                first_line(&before.err)
            )
            .into());
        }
    }
    // Before anything is staged: a named file with a second name refuses the commit here, so
    // its bytes never reach the repository's object store (`planegit::ondisk`).
    if let Stage::Paths(paths) = &ask.stage
        && let Some(refusal) = ondisk::refused_ahead(&ondisk::second_names_of(&here.cwd, paths))
    {
        return Err(not_the_file(&refusal).into());
    }
    here.note("")?;
    let mut queue = None;
    // From here the index may hold what the ask named: anything that stops the commit puts
    // it back and takes the note away.
    let staged = stage(here, &ask.stage)
        .and_then(|()| {
            #[cfg(test)]
            tests::once_staged(top);
            let tree = said(top, &["write-tree"])?.unwrap_or_default();
            here.note(&tree)
        })
        .and_then(|()| staged(top))
        // What is staged is the file on disk now, with one name: a row that is not is taken
        // out of the index again, and its blob out of the object store where nothing else
        // reaches it, before the rest is put back below.
        .and_then(|staged| {
            ondisk::refused(top, &ondisk::not_as_staged(top))
                .map_or(Ok(staged), |refusal| Err(not_the_file(&refusal)))
        })
        .and_then(|staged| {
            if staged.is_empty() {
                return Err(
                    "there is nothing to commit: no named path differs from the branch's \
                     last commit"
                        .to_owned(),
                );
            }
            if let Some(why) = records_another_repository(&staged) {
                return Err(why);
            }
            case_twin(top, &staged).map_or(Ok(()), Err)?;
            filtered(top, &staged).map_or(Ok(()), Err)?;
            scanned(top, &ask.message, &mut queue)?;
            Ok(staged)
        });
    let staged = match staged {
        Ok(staged) => staged,
        Err(why) => {
            unstage(top);
            here.forget();
            return Err(Refused { why, scan: queue });
        }
    };
    for hook in here.hooks_not_run() {
        lines.push(Say::Warn(
            format!(
                "this repository's {hook} hook was not run: for a sandboxed chat, purlis \
             commits with hooks off"
            )
            .into(),
        ));
    }
    let trailers = crate::provenance::trailers_for(root, top, chat);
    let message = crate::provenance::append(&ask.message, &trailers, None);
    // The repository's own identity where it has one, else the person's.
    let own = said(top, &["config", "--local", "--get", "user.name"])?.is_some()
        && said(top, &["config", "--local", "--get", "user.email"])?.is_some();
    let committed = git::within((!own).then_some(&theirs), || {
        git::run_with_input(top, &COMMIT, message.into_bytes(), COMMITTING)
    })
    .map_err(|e| e.to_string());
    let committed = match committed {
        Ok(committed) => committed,
        Err(why) => {
            unstage(top);
            here.forget();
            return Err(why.into());
        }
    };
    if committed.code != Some(0) {
        unstage(top);
        here.forget();
        if committed.code.is_none() {
            return Err(ran_out("write the commit", COMMITTING).into());
        }
        return Err(format!(
            "git did not make the commit: {}",
            first_line(&String::from_utf8_lossy(&committed.err))
        )
        .into());
    }
    here.forget();
    let id = said(top, &["rev-parse", "HEAD"])?.unwrap_or_default();
    lines.insert(
        0,
        Say::Done(
            format!(
                "committed {id} on {branch}: {}",
                counted(staged.len(), "file", "files")
            )
            .into(),
        ),
    );
    Ok(lines)
}

/// What a commit refused by `planegit::ondisk` says: a named file is not the one file on disk.
fn not_the_file(refusal: &ondisk::Refusal) -> String {
    format!(
        "a file to commit is not the one file on disk: {}",
        refusal.sentence()
    )
}

/// Whether `asker` runs sandboxed: on a harness, not started without the sandbox, in a project
/// where one is in force. A manifest that cannot be read may turn it on, so it refuses.
pub fn sandboxed(root: &Path, asker: &Asker) -> Result<bool, String> {
    if !asker.harnessed || asker.unsandboxed {
        return Ok(false);
    }
    let plane = crate::sandbox::Plane::read(root);
    if plane.unreadable() {
        return Err(crate::sandbox::NotStarted::PlaneUnreadable.to_string());
    }
    let locks = crate::sandbox::policy::Locks::of(root);
    Ok(plane.in_force(&locks).is_some())
}

/// Why `message` is not one to commit, or nothing.
pub fn message_ok(message: &str) -> Result<(), String> {
    if message.trim().is_empty() {
        return Err("the commit message is empty".to_owned());
    }
    if message.len() > MOST_MESSAGE {
        return Err(format!(
            "the commit message is longer than {MOST_MESSAGE} bytes"
        ));
    }
    if message.contains('\0') {
        return Err("the commit message holds a NUL byte".to_owned());
    }
    Ok(())
}

/// Why `paths` are not names inside the chat's folder, or nothing: each relative, with no
/// `..`, no `.git` at any depth in any case, no control character, and none too long.
///
/// A name that looks like an option (`-n`, `--amend`) is a name: paths reach git as data on
/// its standard input, never as arguments.
pub fn paths_ok(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("no path was named to stage".to_owned());
    }
    if paths.len() > MOST_PATHS {
        return Err(format!(
            "more than {MOST_PATHS} paths were named: stage a folder, or commit in parts"
        ));
    }
    for path in paths {
        let shown = shown(path);
        if path.is_empty() {
            return Err("an empty path was named".to_owned());
        }
        if path.len() > MOST_PATH {
            return Err(format!("the path {shown} is longer than {MOST_PATH} bytes"));
        }
        if path.chars().any(char::is_control) {
            return Err(format!("the path {shown} holds a control character"));
        }
        let outside = || format!("the path {shown} is not inside the branch folder: {ACCEPTED}");
        if Path::new(path).is_absolute() || path.starts_with(['/', '\\']) {
            return Err(outside());
        }
        for part in Path::new(path).components() {
            match part {
                Component::Normal(name) => {
                    if name.to_string_lossy().eq_ignore_ascii_case(".git") {
                        return Err(format!(
                            "the path {shown} names a `.git`, which is never committed"
                        ));
                    }
                }
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(outside());
                }
            }
        }
    }
    Ok(())
}

/// Why `path`, below `cwd`, is reached through a link, or nothing. Its last part may be a
/// link: git stores that as a link.
pub fn through_no_link(cwd: &Path, path: &str) -> Result<(), String> {
    let mut at = cwd.to_path_buf();
    let parts: Vec<Component<'_>> = Path::new(path)
        .components()
        .filter(|part| !matches!(part, Component::CurDir))
        .collect();
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        at.push(part);
        if std::fs::symlink_metadata(&at).is_ok_and(|found| found.file_type().is_symlink()) {
            return Err(format!(
                "the path {} is reached through a link ({}), and purlis stages nothing \
                 through a link",
                shown(path),
                at.display()
            ));
        }
    }
    Ok(())
}

/// `path` for a sentence: quoted, and cut where it is long.
fn shown(path: &str) -> String {
    let short: String = path.chars().take(120).collect();
    if short.len() < path.len() {
        format!("{short:?}…")
    } else {
        format!("{short:?}")
    }
}

/// How a path to commit is spelled, said wherever one is refused (#3927).
const ACCEPTED: &str = "name paths relative to the branch folder, running the command inside \
     it or with `env -C <branch folder>`, or from where the chat stands as \
     `.worktrees/<repo>/<piece>/<path>`, with no `..`";

/// What a chat that is in no branch folder purlis cut is told.
const NOT_IN_A_BRANCH_FOLDER: &str = "this chat does not stand in a branch folder purlis cut \
     (`<workspace>/.worktrees/<repo>/<piece>`), so the app will not commit for it. In a clone, \
     `git add` and `git commit` work as they are.";

/// The folder the commit is made in, as the app derived and checked it.
#[derive(Debug)]
struct Here {
    /// The branch folder's top, as the kernel names it.
    top: PathBuf,
    /// Where the chat stands, in it.
    cwd: PathBuf,
    /// The worktree's git directory, in its clone's `.git/worktrees`.
    git_dir: PathBuf,
    /// The clone's `.git`.
    common: PathBuf,
    /// The clone it was cut from.
    clone: PathBuf,
}

impl Here {
    /// The branch folder `cwd` is in, where it is one purlis cut in the project at `root`
    /// whose `.git` file names its clone's worktree and nothing else.
    fn of(root: &Path, cwd: Option<&Path>) -> Result<Self, String> {
        let cwd = cwd.ok_or(NO_RECORD)?;
        let not_a_branch_folder = || NOT_IN_A_BRANCH_FOLDER.to_owned();
        let piece = worktree::locate(root, cwd).ok_or_else(not_a_branch_folder)?;
        let (top, git_dir) = match link::of(cwd, link::Check::Whole) {
            Ok(link::Link::Linked { top, git_dir }) => (top, git_dir),
            Ok(_) => return Err(not_a_branch_folder()),
            Err(why) => return Err(why.to_owned()),
        };
        let recorded = root
            .join("workspaces")
            .join(&piece.workspace)
            .join(&piece.repo);
        let (Some(common), Some(clone)) = (link::worktree_of(&git_dir), link::cut_from(&top))
        else {
            return Err(not_a_branch_folder());
        };
        if !super::same_dir(&clone, &recorded) || !super::same_dir(&clone.join(".git"), &common) {
            return Err(link::CHANGED.to_owned());
        }
        Ok(Self {
            cwd: std::fs::canonicalize(cwd).map_err(|_| not_a_branch_folder())?,
            top,
            git_dir,
            common,
            clone,
        })
    }

    /// Why the repository reads objects or settings from somewhere `runs_a_program` did not
    /// look, or nothing: a per-worktree config, or objects borrowed from another repository.
    fn borrows_nothing(&self) -> Result<(), String> {
        for (file, what) in [
            (
                self.git_dir.join("config.worktree"),
                "a configuration of this worktree's own",
            ),
            (
                self.common.join("objects/info/alternates"),
                "objects borrowed from another repository",
            ),
        ] {
            if std::fs::symlink_metadata(&file).is_ok() {
                return Err(format!(
                    "this repository has {what} ({}), which the app does not read for a chat, \
                     so it will not commit here. Commit from your own terminal",
                    file.display()
                ));
            }
        }
        Ok(())
    }

    /// Why the folder is in the middle of something a commit would conclude, or nothing.
    fn mid_nothing(&self) -> Result<(), String> {
        for (entry, what) in [
            ("MERGE_HEAD", "a merge"),
            ("CHERRY_PICK_HEAD", "a cherry-pick"),
            ("REVERT_HEAD", "a revert"),
            ("rebase-merge", "a rebase"),
            ("rebase-apply", "a rebase"),
            ("BISECT_LOG", "a bisect"),
        ] {
            if std::fs::symlink_metadata(self.git_dir.join(entry)).is_ok() {
                return Err(format!(
                    "this folder is in the middle of {what}, which a commit here would \
                     conclude. Finish or abort it in your own terminal first"
                ));
            }
        }
        Ok(())
    }

    /// Leaves the app's note in the worktree's git directory: `tree` is what it staged, or
    /// nothing while it is staging.
    fn note(&self, tree: &str) -> Result<(), String> {
        std::fs::write(self.git_dir.join(NOTE), tree).map_err(|e| {
            format!(
                "the app could not write in this folder's git directory ({e}), so it did not commit"
            )
        })
    }

    /// Takes the note away.
    fn forget(&self) {
        let _ = std::fs::remove_file(self.git_dir.join(NOTE));
    }

    /// Whether what is staged is what an interrupted commit of the app's left: its note is
    /// there, and says it was still staging or names the tree the index holds now.
    fn left_by_an_interrupted_commit(&self) -> bool {
        let Ok(noted) = std::fs::read_to_string(self.git_dir.join(NOTE)) else {
            return false;
        };
        let noted = noted.trim();
        noted.is_empty()
            || said(&self.top, &["write-tree"])
                .ok()
                .flatten()
                .is_some_and(|now| now == noted)
    }

    /// The repository's own hooks a commit would have run, which this one does not.
    fn hooks_not_run(&self) -> Vec<&'static str> {
        let dir = said(&self.top, &["config", "--local", "--get", "core.hooksPath"])
            .ok()
            .flatten()
            .map_or_else(|| self.common.join("hooks"), |named| self.top.join(named));
        HOOKS
            .into_iter()
            .filter(|name| runs(&dir.join(name)))
            .collect()
    }
}

/// Whether `hook` is a file git would run.
fn runs(hook: &Path) -> bool {
    let Ok(found) = std::fs::metadata(hook) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        found.is_file() && found.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        found.is_file()
    }
}

/// One git call in `dir`, or the sentence saying git could not be run.
fn ran(dir: &Path, args: &[&str]) -> Result<git::Run, String> {
    git::run(dir, args, git::READ).map_err(|e| e.to_string())
}

/// The first key of the repository's own config at `top` that names a signer
/// ([`super::names_a_signer`]), or nothing. A config git cannot read refuses the commit.
fn its_signer(top: &Path) -> Result<Option<String>, String> {
    let run = ran(
        top,
        &[
            "config",
            "--local",
            "--name-only",
            "--get-regexp",
            r"^gpg\.",
        ],
    )?;
    match run.code {
        Some(0) => Ok(run
            .out
            .lines()
            .map(str::trim)
            .find(|key| super::names_a_signer(key))
            .map(str::to_owned)),
        Some(1) => Ok(None),
        _ => Err(format!(
            "git could not read this repository's config: {}",
            first_line(&run.err)
        )),
    }
}

/// What one git call printed, where it succeeded and printed something.
fn said(dir: &Path, args: &[&str]) -> Result<Option<String>, String> {
    let run = ran(dir, args)?;
    Ok(run
        .ok()
        .then(|| run.line().trim().to_owned())
        .filter(|line| !line.is_empty()))
}

/// The first line of what git said, for a sentence.
fn first_line(text: &str) -> String {
    let line = text.lines().find(|line| !line.trim().is_empty());
    line.unwrap_or("it gave no reason").trim().to_owned()
}

/// What a call git did not finish in time is answered with.
fn ran_out(what: &str, within: std::time::Duration) -> String {
    format!(
        "git did not {what} within {} minutes, so it was stopped and nothing was committed",
        within.as_secs() / 60
    )
}

/// **Stages what `stage` names, each regular file from one open descriptor**
/// (`planegit::ondisk`): what `git add` of it would stage, its bytes read from the descriptor
/// that was asked it is a regular file with one name, and asked again just before they are
/// hashed, so a file with a second name never reaches the object store, and nothing is read
/// again by its name. A link is staged as the bytes it holds, a deletion as its row taken out;
/// another repository inside the folder is refused before anything is staged. Paths are read
/// letter for letter, so none is read as an option or as pathspec magic.
fn stage(here: &Here, stage: &Stage) -> Result<(), String> {
    let refused = |why: String| format!("git did not stage that: {}", first_line(&why));
    let paths = match stage {
        Stage::Tracked => ondisk::to_stage(&here.top, &[], ondisk::Listing::Tracked),
        Stage::Paths(paths) => {
            let below = said(&here.cwd, &["rev-parse", "--show-prefix"])?.unwrap_or_default();
            let named: Vec<String> = paths.iter().map(|path| format!("{below}{path}")).collect();
            each_named(&here.top, &named).map_err(refused)?;
            let specs: Vec<String> = named.iter().map(|path| literally(path)).collect();
            let specs: Vec<&str> = specs.iter().map(String::as_str).collect();
            ondisk::to_stage(&here.top, &specs, ondisk::Listing::All)
        }
    }
    .map_err(refused)?;
    // Looked at whole before anything is staged, so what would refuse the commit refuses it
    // with nothing staged: another repository (nothing of it is read), a file with a second
    // name, and what git would not stage. Nothing is held open here.
    let mut held = Vec::new();
    for path in &paths {
        match ondisk::opened(&here.top, path) {
            ondisk::OnDisk::Folder { nested: true } => return Err(another_repository(path)),
            ondisk::OnDisk::SecondName => held.push(ondisk::Held {
                path: path.clone(),
                why: ondisk::Why::SecondName,
            }),
            found => drop(ondisk::Row::of(path.clone(), found).map_err(refused)?),
        }
    }
    if let Some(refusal) = ondisk::refused_unstaged(&held, ondisk::PutBack::AsHead) {
        return Err(not_the_file(&refusal));
    }
    // Then staged a part at a time, each file from the descriptor it is opened as then.
    let held = ondisk::stage_listed(&here.top, paths, |path, found| {
        Ok(match found {
            ondisk::OnDisk::SecondName => ondisk::Take::Held(ondisk::Held {
                path,
                why: ondisk::Why::SecondName,
            }),
            ondisk::OnDisk::Folder { nested: true } => return Err(another_repository(&path)),
            found => ondisk::Take::Row(ondisk::Row::of(path, found)?),
        })
    })
    .map_err(refused)?;
    if let Some(refusal) = ondisk::refused_unstaged(&held, ondisk::PutBack::AsHead) {
        return Err(not_the_file(&refusal));
    }
    ondisk::refresh(&here.top, false).map_err(refused)
}

/// `path` as a pathspec git reads letter for letter.
fn literally(path: &str) -> String {
    format!(":(literal){path}")
}

/// Why `git add` of `named` (relative to `top`) would have refused, in git's words, or
/// nothing: a path reached through a link, one that names nothing on disk or in the index, or
/// an untracked file or folder the repository ignores, named as itself.
fn each_named(top: &Path, named: &[String]) -> Result<(), String> {
    for path in named {
        match ondisk::opened(top, &plainly(path)) {
            ondisk::OnDisk::Beyond => {
                return Err(format!(
                    "pathspec {} is beyond a symbolic link",
                    shown(path)
                ));
            }
            ondisk::OnDisk::Gone => {
                let run = git::run(top, &["ls-files", "-z", "--", &literally(path)], git::READ)
                    .map_err(|e| e.to_string())?;
                if !run.ok() || run.out.is_empty() {
                    return Err(format!("pathspec {} did not match any files", shown(path)));
                }
            }
            _ => {}
        }
    }
    let asked: std::collections::HashSet<String> = named.iter().map(|path| plainly(path)).collect();
    // Listed file by file, and again with an ignored folder as itself: a path named is refused
    // where it is either.
    for folders in [false, true] {
        let mut args = vec![
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
        ];
        if folders {
            args.push("--directory");
        }
        args.push("--");
        let specs: Vec<String> = named.iter().map(|path| literally(path)).collect();
        args.extend(specs.iter().map(String::as_str));
        let run = git::run(top, &args, git::READ).map_err(|e| e.to_string())?;
        if !run.ok() {
            return Err(run.err);
        }
        if let Some(ignored) = run
            .out
            .split('\0')
            .find(|path| !path.is_empty() && asked.contains(&plainly(path)))
        {
            return Err(format!(
                "{} is ignored by one of the repository's .gitignore files",
                shown(ignored)
            ));
        }
    }
    Ok(())
}

/// `path` as git names it: no `.` part, and no `/` at either end.
fn plainly(path: &str) -> String {
    path.split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// Puts the index back to the branch's last commit: what the ask staged is unstaged, and the
/// files are left as they are. The index held nothing else when staging began.
fn unstage(top: &Path) {
    let _ = git::run(top, &["reset", "--quiet"], git::READ);
}

/// One staged change: the mode it would be committed with, how it differs, and its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged {
    /// The mode before, as git writes it (`100644`, `120000`, `160000`, `000000`).
    pub was: String,
    /// The mode after.
    pub mode: String,
    /// git's letter: `A`, `M`, `D`, `T`.
    pub status: char,
    pub path: String,
}

/// What is staged in `top` against its last commit.
fn staged(top: &Path) -> Result<Vec<Staged>, String> {
    let run = git::run_with_input(
        top,
        &[
            "diff",
            "--cached",
            "--raw",
            "-z",
            "--no-renames",
            "--no-abbrev",
            "--ignore-submodules=none",
        ],
        Vec::new(),
        git::READ,
    )
    .map_err(|e| e.to_string())?;
    if run.code != Some(0) {
        return Err(format!(
            "git could not read what was staged: {}",
            first_line(&String::from_utf8_lossy(&run.err))
        ));
    }
    parse_raw(&run.out).ok_or_else(|| "git's list of what was staged could not be read".to_owned())
}

/// `git diff --raw -z --no-renames`: `:<was> <mode> <sha> <sha> <status>\0<path>\0`, repeated.
pub fn parse_raw(out: &[u8]) -> Option<Vec<Staged>> {
    let mut fields = out.split(|byte| *byte == 0);
    let mut all = Vec::new();
    while let Some(head) = fields.next() {
        if head.is_empty() {
            continue;
        }
        let head = std::str::from_utf8(head).ok()?.strip_prefix(':')?;
        let mut words = head.split(' ');
        let (was, mode) = (words.next()?, words.next()?);
        let status = words.nth(2)?.chars().next()?;
        let path = fields.next().filter(|path| !path.is_empty())?;
        let path = String::from_utf8(path.to_vec()).ok()?;
        all.push(Staged {
            was: was.to_owned(),
            mode: mode.to_owned(),
            status,
            path,
        });
    }
    Some(all)
}

/// git's mode for an entry that names a commit of another repository.
const GITLINK: &str = "160000";

/// Why `staged` would record another repository, or nothing: a gitlink entry (a folder with a
/// `.git` of its own, which git stages as a submodule), or a change to `.gitmodules`, which
/// tells a later git where to fetch one from and what to run on it.
pub fn records_another_repository(staged: &[Staged]) -> Option<String> {
    for change in staged {
        if change.mode == GITLINK || change.was == GITLINK {
            return Some(another_repository(&change.path));
        }
        let name = change.path.rsplit('/').next().unwrap_or(&change.path);
        if name.eq_ignore_ascii_case(".gitmodules") {
            return Some(format!(
                "{} declares submodules, and purlis does not commit a change to it for a \
                 chat. Commit it from your own terminal",
                shown(&change.path)
            ));
        }
    }
    None
}

/// What a commit that would record the repository at `path` inside this one is refused with.
fn another_repository(path: &str) -> String {
    format!(
        "{} is a repository of its own (it holds a `.git`), and purlis does not commit one \
         inside another for a chat. Remove its `.git`, or leave it out",
        shown(path)
    )
}

/// Why a path `staged` adds differs from another tracked path only by case, or nothing. On a
/// file system that ignores case the two are one file, and a checkout writes one over the other.
fn case_twin(top: &Path, staged: &[Staged]) -> Option<String> {
    if !staged.iter().any(|change| change.status == 'A') {
        return None;
    }
    let listed = git::run_with_input(top, &["ls-files", "-z"], Vec::new(), git::READ).ok()?;
    let tracked: Vec<String> = listed
        .out
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect();
    twin_among(&tracked, staged)
}

/// [`case_twin`], over the tracked paths.
pub fn twin_among(tracked: &[String], staged: &[Staged]) -> Option<String> {
    let mut folded: std::collections::HashMap<String, &str> = std::collections::HashMap::new();
    for path in tracked {
        if let Some(other) = folded.insert(path.to_lowercase(), path.as_str())
            && other != path.as_str()
            && staged.iter().any(|change| {
                change.status == 'A' && (change.path == *path || change.path == other)
            })
        {
            return Some(format!(
                "{} and {} differ only by case, and purlis does not add such a pair for a \
                 chat. Rename one",
                shown(other),
                shown(path)
            ));
        }
    }
    None
}

/// Why a path `staged` adds or changes is marked for a content filter, or nothing.
fn filtered(top: &Path, staged: &[Staged]) -> Option<String> {
    let paths: Vec<&str> = staged
        .iter()
        .filter(|change| change.status != 'D')
        .map(|change| change.path.as_str())
        .collect();
    if paths.is_empty() {
        return None;
    }
    let asked = git::run_with_input(
        top,
        &["check-attr", "-z", "--stdin", "filter"],
        paths.join("\0").into_bytes(),
        git::READ,
    );
    match asked {
        Ok(run) if run.code == Some(0) => filter_named(&run.out).map(|(path, filter)| {
            format!(
                "{} is marked for a content filter (`filter={filter}` in its attributes). A \
                 commit made for a sandboxed chat reads none of the person's filters, so the \
                 file would be stored as it is on disk. The person commits a filtered file \
                 from their own terminal",
                shown(&path)
            )
        }),
        _ => Some("git could not read the attributes of what was staged".to_owned()),
    }
}

/// The first path `git check-attr -z filter` answers a filter for, and the filter:
/// `<path>\0filter\0<value>\0`, repeated, where no filter is `unspecified` or `unset`.
pub fn filter_named(out: &[u8]) -> Option<(String, String)> {
    let fields: Vec<&[u8]> = out.split(|byte| *byte == 0).collect();
    fields.as_chunks::<3>().0.iter().find_map(|answer| {
        let value = String::from_utf8_lossy(answer[2]).into_owned();
        (!matches!(value.as_str(), "unspecified" | "unset"))
            .then(|| (String::from_utf8_lossy(answer[0]).into_owned(), value))
    })
}

/// purlis's scan of what the staged change would publish (ADR 0074), and of the `message` it
/// would be committed with (#1133): the checks a chat's own `pre-commit` and `commit-msg` make,
/// made here.
///
/// Where it refuses, `queue` is given the line of the needs-you item the app raises, as a
/// chat's own `pre-commit` gives it (#1476).
fn scanned(top: &Path, message: &str, queue: &mut Option<String>) -> Result<(), String> {
    let mut scan = crate::diffscan::checked(top)
        .map_err(|why| format!("the commit could not be scanned for secrets ({why})"))?;
    let said = crate::diffscan::message_checked(top, message);
    scan.refused.extend(said.refused);
    if scan.changes_the_allowlist {
        return Err(
            "the commit changes the scan's allowlist, which only a commit made outside a chat \
             may change. Leave the file out; the person reviews and commits an entry"
                .to_owned(),
        );
    }
    if scan.refused.is_empty() {
        return Ok(());
    }
    let summary = crate::diffscan::summary(crate::diffscan::Stopped::Commit, top, &scan.refused);
    *queue = Some(summary.clone());
    Err(format!(
        "{summary}. Fix what the scan found and commit again; `purlis scan --explain` names the \
         entry that would let a finding through"
    ))
}

pub mod plain;

#[cfg(test)]
mod tests;
