// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A clone or a worktree a chat asks the app to make for it: **a brokered git action**
//! (ADR 0067 §2, #1335).
//!
//! A sandboxed chat may not write a clone's `.git/config`, its hooks, or the editor settings a
//! checkout carries (`.vscode/`, `.idea/`, `.claude/settings.json`), and `git clone` and
//! `git worktree add` write all of them. A branch folder's own git directory is in its clone's
//! `.git` too, so `purlis worktree update` and `push` are handed over the same way
//! ([`crate::pieceupdate`], qw387), for a chat standing in that folder or above it, and so is
//! `purlis worktree remove`, never forced, for a chat above the folder (#3879). So `purlis clone` and `purlis worktree add`, run in a
//! chat the app started, hand the work to the app over the chat's hook socket
//! ([`crate::hookwire::Ask::Git`]), and the app runs **the same core functions** the terminal's
//! commands run — [`crate::repocmd::clone::clone`] and [`crate::piececmd::add`] — so git is
//! hardened the one way it always is ([`crate::worktree::git`]). The chat's own writes to those
//! files stay denied.
//!
//! What the app takes from the ask is checked against its own record of the chat, never the
//! ask's word:
//! - **the workspace** must be one the chat already writes its ordinary files in: the chat
//!   stands at the project root, or inside that workspace ([`bound`]). The one exception is a
//!   workspace the ask itself makes (`purlis workspace create <name> <repo>…`,
//!   [`GitWork::Create`]): it must not be there yet, and is made by the brokered create's own
//!   rules before anything is cloned into it;
//! - **the clone's host** must be one the chat's sandbox may reach, where the chat runs
//!   sandboxed ([`hosts`]);
//! - **the piece log** credits the chat and the persona the app recorded for it;
//! - **no configuration the chat could have shaped names a program** (D-1335-7): every git
//!   call reads no global or system file ([`git::Isolated`]), and a repository whose own
//!   config defines a driver, a helper or an include is refused before any git runs in it
//!   ([`runs_a_program`]).

use std::path::{Path, PathBuf};

use crate::counts::plural;
use crate::hookwire::{Answer, GitAsk, GitWork};
use crate::pieces::Who;
use crate::repocmd::{self, Say};
use crate::worktree::git;

/// What the app knows of the chat that asks: its own record, never the ask's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asker {
    /// The app's number for the chat.
    pub chat: u32,
    /// Where the chat stands.
    pub cwd: Option<PathBuf>,
    /// The persona the app started it as.
    pub persona: Option<String>,
    /// Whether it runs a harness. A shell tab is the operator's own and is never sandboxed.
    pub harnessed: bool,
    /// Whether a person started this run of it without the sandbox (ruling V78 a).
    pub unsandboxed: bool,
    /// The machine store the app holds, whose device id names the piece log (FD-25), as the
    /// window's own pieces are named (`worktrees::window`); `None` on a machine that keeps none.
    pub config: Option<PathBuf>,
    /// The chat's stable id (ADR 0066), which a branch folder's claim names its one live writer
    /// by (#3901); `None` for a chat that has none.
    pub id: Option<String>,
    /// The chats the app has, live or not yet back since a restart, as a branch folder's claim
    /// is judged against them (#3901); `None` where no app said, and a folder another chat
    /// holds is then refused.
    pub live: Option<Vec<crate::piececlaim::Live>>,
}

/// Runs `ask` for `asker` in the project at `root`, every git call reading `isolation`'s
/// configuration and nothing else, and answers every line it said with its exit status, or
/// why it was not run.
pub fn answer(
    root: &Path,
    asker: &Asker,
    ask: &GitAsk,
    isolation: &git::Isolated,
    now: chrono::DateTime<chrono::Utc>,
) -> Answer {
    answer_telling(root, asker, ask, isolation, now).answer
}

/// What the app makes of a chat's git ask ([`answer_telling`]): the answer the chat reads, and
/// what the person is needed for, if anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    pub answer: Answer,
    /// **The need the person is told of** (#3877): only an update of the project root left for
    /// a reason no chat can settle ([`crate::planegit::Left::needs_the_person`]), which the
    /// app raises as a needs-you item naming the window's Update. Every other refusal is the
    /// chat's to retry, told in the answer alone.
    pub needs: Option<crate::state::Need>,
}

impl From<Answer> for Answered {
    fn from(answer: Answer) -> Self {
        Self {
            answer,
            needs: None,
        }
    }
}

/// [`answer`], and what the person is needed for ([`Answered::needs`]).
pub fn answer_telling(
    root: &Path,
    asker: &Asker,
    ask: &GitAsk,
    isolation: &git::Isolated,
    now: chrono::DateTime<chrono::Utc>,
) -> Answered {
    git::isolated(isolation, || isolated_answer(root, asker, ask, now))
}

fn isolated_answer(
    root: &Path,
    asker: &Asker,
    ask: &GitAsk,
    now: chrono::DateTime<chrono::Utc>,
) -> Answered {
    let no = |why: String| Answered::from(Answer::No { why });
    let mut needs = None;
    // The guard's question before plain git writes a branch folder (#3931): no git runs, so
    // none of the checks below on where git would run is asked.
    if let GitWork::WorktreeClaim { repo, piece, act } = &ask.work {
        return Answered::from(claim(root, asker, &ask.workspace, repo, piece, *act, now));
    }
    // A workspace the ask makes is one no chat works in yet: the chat need only stand in the
    // project. Every other ask writes a workspace the chat already writes.
    let placed = match &ask.work {
        GitWork::Create { .. } => in_the_project(root, asker.cwd.as_deref()),
        // A branch folder's own git: the chat stands in it, or above it in its workspace.
        GitWork::WorktreeUpdate { repo, piece, .. } | GitWork::WorktreePush { repo, piece, .. } => {
            bound(root, asker.cwd.as_deref(), &ask.workspace)
                .and_then(|()| at_or_above(root, asker.cwd.as_deref(), repo, piece))
        }
        // A branch folder of the chat's own workspace, from above it: never the one it stands
        // in, whose removal would take the chat's own folder from under it (#3879).
        GitWork::WorktreeRemove { repo, piece, .. } => {
            bound(root, asker.cwd.as_deref(), &ask.workspace)
                .and_then(|()| above(root, asker.cwd.as_deref(), repo, piece))
        }
        // The project root: only for the chat whose place it is.
        GitWork::RootUpdate {} => at_the_root(root, asker.cwd.as_deref()),
        // A clone's own checkout: the chat stands in it, or above it in its workspace.
        GitWork::CloneUpdate { repo, .. } => bound(root, asker.cwd.as_deref(), &ask.workspace)
            .and_then(|()| at_or_above_clone(root, asker.cwd.as_deref(), &ask.workspace, repo)),
        _ => bound(root, asker.cwd.as_deref(), &ask.workspace),
    };
    if let Err(why) = placed {
        return no(why);
    }
    let hosts = match hosts(root, asker) {
        Ok(hosts) => hosts,
        Err(why) => return no(why),
    };
    let mut checked = Vec::new();
    for repo in touched(root, &ask.workspace, &ask.work) {
        match checked_repo(&repo) {
            Ok(found) => checked.push((repo, found)),
            Err(why) => return no(why),
        }
    }
    let mut lines = Vec::new();
    let mut say = |line: Say| lines.push(line);
    let code = match &ask.work {
        GitWork::Create { vision, repos } => match crate::brokered::create_workspace_at(
            root,
            &ask.workspace,
            vision.as_deref(),
            repos,
            hosts.as_deref(),
            now,
            &mut say,
        ) {
            Ok(code) => code,
            Err(why) => return no(why),
        },
        GitWork::Clone { repos } => repocmd::clone::clone(
            &repocmd::clone::Request {
                root,
                ws: &ask.workspace,
                repos,
                now,
                author: &crate::wscmd::ensure::author(),
                hosts: hosts.as_deref(),
                watch: None,
            },
            &mut say,
        ),
        GitWork::WorktreeUpdate { repo, piece, .. } | GitWork::WorktreePush { repo, piece, .. } => {
            let sandboxed = match commit::sandboxed(root, asker) {
                Ok(sandboxed) => sandboxed,
                Err(why) => return no(why),
            };
            let reach = crate::pieceupdate::Reach {
                hosts: hosts.as_deref(),
                sandboxed_at: asker.cwd.as_deref().filter(|_| sandboxed),
            };
            let at = crate::pieceupdate::Piece {
                root,
                ws: &ask.workspace,
                repo,
                piece,
            };
            // One live writer per branch folder (#3901): a second chat is refused, naming the
            // chat that holds it; a chat writing in a folder nobody live holds takes it.
            let act = match &ask.work {
                GitWork::WorktreePush { .. } => crate::piececlaim::Act::Push,
                _ => crate::piececlaim::Act::Update,
            };
            let folder = crate::worktree::Located {
                workspace: ask.workspace.clone(),
                repo: repo.clone(),
                piece: piece.clone(),
            };
            if let Err(why) =
                crate::piececlaim::take(root, &folder, &who(asker), asker.live.as_deref(), act, now)
            {
                return no(why);
            }
            let held = git::isolation().unwrap_or_default();
            let ran = in_a_checked_folder(root, &ask.workspace, repo, piece, &held, || match &ask
                .work
            {
                GitWork::WorktreeUpdate { step, .. } => {
                    crate::pieceupdate::update(&at, step, &reach, &mut say)
                }
                // `lease` is the `--force-with-lease` typed, which asks for nothing a plain
                // push does not (#3895).
                GitWork::WorktreePush { .. } => crate::pieceupdate::push(&at, &reach, &mut say),
                _ => 1,
            });
            match ran {
                Ok(code) => code,
                Err(NotRun::Repo(why)) => return no(why),
                Err(NotRun::Folder(why)) => return no(why.to_string()),
            }
        }
        GitWork::WorktreeRemove {
            repo,
            piece,
            delete_branch,
        } => {
            // Never forced: the ask has no word for it. A folder holding an uncommitted change
            // or a commit no other ref reaches, its branch not landed by its files, is
            // refused, naming what would be lost, and a folder purlis cut for a task is
            // refused as it is from a terminal. And one live writer per branch folder (#3901):
            // a folder another live chat holds is not removed for this one, and a removal
            // ends the folder's claim.
            let folder = crate::worktree::Located {
                workspace: ask.workspace.clone(),
                repo: repo.clone(),
                piece: piece.clone(),
            };
            let held = git::isolation().unwrap_or_default();
            let mut not_run = None;
            let removed = crate::piececlaim::removing(
                root,
                &folder,
                &who(asker),
                crate::piececlaim::Judged::ByAChat(asker.live.as_deref()),
                now,
                || {
                    let ran = in_a_checked_folder(root, &ask.workspace, repo, piece, &held, || {
                        // What git ignores there goes with the folder: refused, named (#3879).
                        if let Some(why) =
                            crate::piececmd::ignored_would_go(root, &ask.workspace, repo, piece)
                        {
                            say(Say::Fail(why.into()));
                            return 1;
                        }
                        crate::piececmd::remove(
                            root,
                            &ask.workspace,
                            repo,
                            piece,
                            false,
                            *delete_branch,
                            &mut say,
                        )
                    });
                    match ran {
                        Ok(code) => code,
                        Err(NotRun::Repo(why)) => {
                            not_run = Some(why);
                            1
                        }
                        Err(NotRun::Folder(why)) => {
                            not_run = Some(why.to_string());
                            1
                        }
                    }
                },
            );
            match (removed, not_run) {
                (_, Some(why)) | (Err(why), _) => return no(why),
                (Ok(code), None) => code,
            }
        }
        // Answered before anything above was asked ([`claim`]).
        GitWork::WorktreeClaim { repo, piece, act } => {
            return Answered::from(claim(root, asker, &ask.workspace, repo, piece, *act, now));
        }
        GitWork::RootUpdate {} => {
            // Placed at the root above, so its own folder is the root.
            let chat_dir = asker.cwd.as_deref().unwrap_or(root);
            let updated = crate::planegit::update_for_a_chat(root, chat_dir, &mut say);
            let request = crate::forge::request_words_of(root, root).request_noun();
            let said = updated.said(request);
            if let crate::planegit::Updated::Left(left) = &updated {
                needs = left
                    .needs_the_person()
                    .then_some(crate::state::Need::RootUpdateRefused);
                say(Say::Fail(said.into()));
                1
            } else {
                say(Say::Done(said.into()));
                0
            }
        }
        GitWork::CloneUpdate { repo, step } => {
            let sandboxed = match commit::sandboxed(root, asker) {
                Ok(sandboxed) => sandboxed,
                Err(why) => return no(why),
            };
            let reach = crate::pieceupdate::Reach {
                hosts: hosts.as_deref(),
                sandboxed_at: asker.cwd.as_deref().filter(|_| sandboxed),
            };
            let at = crate::pieceupdate::CloneAt {
                root,
                ws: &ask.workspace,
                repo,
            };
            let held = git::isolation().unwrap_or_default();
            match in_a_checked_clone(root, &ask.workspace, repo, &held, || {
                crate::pieceupdate::update_clone(&at, step, &reach, &mut say)
            }) {
                Ok(code) => code,
                Err(why) => return no(why),
            }
        }
        GitWork::WorktreeAdd {
            repo,
            piece,
            branch,
        }
        | GitWork::WorktreeCut {
            repo,
            piece,
            branch,
            ..
        } => {
            // An ask with no base word is an older command's, which cut from the clone's HEAD.
            let base = match &ask.work {
                GitWork::WorktreeCut { base, .. } => base.as_deref(),
                _ => Some(crate::piececmd::HEAD),
            };
            // For a sandboxed chat (qw387): the remote-tracking branches a cut may start from
            // are the app's own fetch's, and the cut is judged as the chat's own checkout of
            // its start would be.
            let sandboxed = match commit::sandboxed(root, asker) {
                Ok(sandboxed) => sandboxed,
                Err(why) => return no(why),
            };
            if sandboxed
                && base != Some(crate::piececmd::HEAD)
                && let Ok(clone) = crate::worktree::clone_dir(root, &ask.workspace, repo)
                && let Some(remote) = crate::pieceupdate::remote_of(&clone, base.unwrap_or("HEAD"))
                && let Err(why) =
                    crate::repocmd::sync::fetch_remote(root, &clone, &remote, hosts.as_deref())
            {
                return no(format!("{why}. Nothing was cut"));
            }
            let judged = |clone: &Path, start: &str, folder: &Path| {
                crate::cloneupdate::judged_cut(clone, start, folder, root, asker.cwd.as_deref())
                    .map_or(Ok(()), Err)
            };
            let vet: Option<crate::worktree::Vet<'_>> = sandboxed.then_some(&judged);
            let add = |say: &mut dyn FnMut(Say)| {
                crate::piececmd::add(
                    root,
                    &ask.workspace,
                    repo,
                    piece,
                    branch.as_deref(),
                    base,
                    vet,
                    hosts.as_deref(),
                    &who(asker),
                    now,
                    say,
                )
            };
            // The clone's git calls are given the git directory the check resolved, so the
            // worktree is cut from the repository that was checked (D-1335-9) — that very
            // directory, compared again before every call and once after (#1415).
            let code = match (git::isolation(), checked.first()) {
                (Some(held), Some((tree, found))) => cut_pinned(&held, tree, found, &mut say, add),
                _ => add(&mut say),
            };
            // Cut by git that read none of your config, so it ran no filter of yours (#1413):
            // a checkout whose .gitattributes asks for one, Git LFS above all, holds what is
            // stored, and the answer says so.
            if code == 0
                && let Ok(folder) = crate::worktree::path_for(root, &ask.workspace, repo, piece)
            {
                let at = folder
                    .strip_prefix(root)
                    .unwrap_or(&folder)
                    .display()
                    .to_string();
                for note in repocmd::unread::filter_notes(&folder, piece, &at) {
                    say(Say::Warn(note.into()));
                }
            }
            code
        }
    };
    Answered {
        answer: Answer::Said {
            lines: fitted(lines, SAID_AT_MOST),
            code,
        },
        needs,
    }
}

/// Runs `then` in the clone of `repo` in workspace `ws` **as a brokered git action is run**,
/// for work the app does there on its own account and not on a chat's ask (#1453): the
/// worktree it cuts for a persona chat a dispatch starts, and that worktree's removal.
///
/// The chat that will work there, or did, could have shaped the repository, so the two rules a
/// chat's own ask is held to hold here: every git call reads `isolation`'s configuration and
/// no global or system file, and a repository whose own config names a program is refused
/// before any git runs in it ([`runs_a_program`]), with the calls pinned to the git directory
/// that was checked. Nothing here reads an ask: which clone it is, is the app's to say.
pub fn in_a_checked_clone<T>(
    root: &Path,
    ws: &str,
    repo: &str,
    isolation: &git::Isolated,
    then: impl FnOnce() -> T,
) -> Result<T, String> {
    if !crate::contain::workspace_name_ok(ws) {
        return Err(format!(
            "'{}' cannot name a workspace",
            crate::shown::short(ws)
        ));
    }
    if !crate::contain::repo_name_ok(repo) {
        return Err(format!(
            "'{}' cannot name a repo",
            crate::shown::short(repo)
        ));
    }
    let clone = root.join("workspaces").join(ws).join(repo);
    git::isolated(isolation, || {
        let found = checked_repo(&clone)?;
        Ok(git::isolated(
            &isolation.pinned_as(&clone, &found.git_dir, found.id),
            then,
        ))
    })
}

/// Why [`in_a_checked_folder`] ran nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotRun {
    /// The clone is not one the brokered route runs git in ([`in_a_checked_clone`]'s sentence).
    Repo(String),
    /// The branch's folder does not name the git directory the clone keeps for it.
    Folder(crate::worktree::pointer::NotItsFolder),
}

/// [`in_a_checked_clone`] for work that also runs git **in the folder of `piece`**, a branch
/// cut from that clone: what the folder holds, whether its branch is merged, its removal.
///
/// The folder is where a chat wrote, so which repository it belongs to is not read from it.
/// Its git directory is derived from the clone that was just checked and the piece's name, the
/// folder is checked to name exactly that directory
/// ([`crate::worktree::pointer::verified`]), and every call run in the folder is pinned to it
/// beside the clone's own pin (D-1453-27). A folder that names anything else is
/// [`NotRun::Folder`], and no git runs in it or for it. A folder that is not there has nothing
/// to check and gets no pin: no call can run in it.
pub fn in_a_checked_folder<T>(
    root: &Path,
    ws: &str,
    repo: &str,
    piece: &str,
    isolation: &git::Isolated,
    then: impl FnOnce() -> T,
) -> Result<T, NotRun> {
    let folder = crate::worktree::path_for(root, ws, repo, piece)
        .map_err(|refusal| NotRun::Repo(refusal.to_string()))?;
    let clone = root.join("workspaces").join(ws).join(repo);
    git::isolated(isolation, || {
        let found = checked_repo(&clone).map_err(NotRun::Repo)?;
        let git_dir = found.git_dir;
        let mut held = isolation.pinned_as(&clone, &git_dir, found.id);
        if folder.symlink_metadata().is_ok() {
            let own = crate::worktree::pointer::verified(&git_dir, &folder, piece)
                .map_err(NotRun::Folder)?;
            held = held.pinned(&folder, &own);
        }
        Ok(git::isolated(&held, then))
    })
}

/// Whether `workspace` is one a chat standing at `cwd` already writes its ordinary files in:
/// the chat stands at the project root, which writes every workspace, or inside `workspace`
/// itself (D-3). Anything else is refused with a sentence.
pub fn bound(root: &Path, cwd: Option<&Path>, workspace: &str) -> Result<(), String> {
    let Some(cwd) = cwd else {
        return Err(
            "the app has no record of where this chat stands, so it will not write a workspace \
             for it"
                .to_owned(),
        );
    };
    match crate::active::workspace_of_tree(root, cwd) {
        Some(own) if own == workspace => Ok(()),
        Some(own) => Err(format!(
            "this chat works in workspace '{own}', so the app will not write workspace \
             '{workspace}' for it. Run the command from a chat in '{workspace}', or at the \
             project root"
        )),
        None if same_dir(root, cwd) => Ok(()),
        None => Err(format!(
            "this chat stands outside the project's workspaces, so the app will not write \
             workspace '{workspace}' for it. Run the command from a chat in '{workspace}', or \
             at the project root"
        )),
    }
}

/// Whether a chat standing at `cwd` may ask for the branch folder `<repo>/<piece>`'s own git
/// (qw387): it stands in that folder, or above branch folders in their workspace or the
/// project; never in another branch folder. As the brokered commit holds a chat to the folder
/// it stands in, or one below it.
pub fn at_or_above(root: &Path, cwd: Option<&Path>, repo: &str, piece: &str) -> Result<(), String> {
    let Some(cwd) = cwd else {
        return Err(
            "the app has no record of where this chat stands, so it will not run git for it"
                .to_owned(),
        );
    };
    match crate::worktree::locate(root, cwd) {
        Some(here) if here.repo != repo || here.piece != piece => Err(format!(
            "this chat stands in branch folder {} · {}, so the app will not run git in {repo} · \
             {piece} for it",
            here.repo, here.piece
        )),
        _ => Ok(()),
    }
}

/// Whether a chat standing at `cwd` may ask for the branch folder `<repo>/<piece>` to be
/// removed (#3879): [`at_or_above`], and never from inside that folder, which would go from
/// under the chat. A place that cannot be read is refused.
pub fn above(root: &Path, cwd: Option<&Path>, repo: &str, piece: &str) -> Result<(), String> {
    at_or_above(root, cwd, repo, piece)?;
    // A place that cannot be read is refused: whether it is the folder itself cannot be told.
    let Some(cwd) = cwd.filter(|cwd| std::fs::canonicalize(cwd).is_ok()) else {
        return Err(format!(
            "the app could not place where this chat stands, so it will not remove branch \
             folder {repo} · {piece} for it"
        ));
    };
    match crate::worktree::locate(root, cwd) {
        Some(_) => Err(format!(
            "this chat stands in branch folder {repo} · {piece}, so the app will not remove it \
             for it: removing it would take the folder this chat works in. The chat that cut \
             it, or any chat opened in the workspace's folder, removes it from there with \
             `purlis worktree remove {repo} {piece}` or the worktree_remove tool; or the \
             person, from the window: Remove folder {piece} in {repo}, in the palette"
        )),
        None => Ok(()),
    }
}

/// Whether a chat standing at `cwd` stands at the project root itself (#3831): the one chat
/// whose place the root's working tree is. Anything else, or a place that cannot be read, is
/// refused.
pub fn at_the_root(root: &Path, cwd: Option<&Path>) -> Result<(), String> {
    match cwd {
        Some(cwd) if same_dir(root, cwd) => Ok(()),
        _ => Err(
            "this chat does not stand at the project root, so the app will not update the \
             project for it: `purlis worktree update` in a workspace's clone or branch folder \
             updates that folder"
                .to_owned(),
        ),
    }
}

/// Whether a chat standing at `cwd` stands in clone `repo` of workspace `ws`, or above it: at
/// the project root or the workspace's own folder (#3831). A chat in a branch folder, or in
/// another folder of the workspace, is refused: the app updates the clone a chat works in.
pub fn at_or_above_clone(
    root: &Path,
    cwd: Option<&Path>,
    ws: &str,
    repo: &str,
) -> Result<(), String> {
    let Some(cwd) = cwd else {
        return Err(
            "the app has no record of where this chat stands, so it will not run git for it"
                .to_owned(),
        );
    };
    if let Some(here) = crate::worktree::locate(root, cwd) {
        return Err(format!(
            "this chat stands in branch folder {} · {}, so the app will not update clone {repo} \
             for it: `purlis worktree update` there brings that folder up instead",
            here.repo, here.piece
        ));
    }
    let unplaced = || {
        format!(
            "the app could not place where this chat stands in workspace '{}', so it will not \
             update clone {repo} for it",
            crate::shown::short(ws)
        )
    };
    let (Ok(project), Ok(workspace), Ok(here)) = (
        std::fs::canonicalize(root),
        std::fs::canonicalize(root.join("workspaces").join(ws)),
        std::fs::canonicalize(cwd),
    ) else {
        return Err(unplaced());
    };
    // At the project root: above every clone.
    if here == project {
        return Ok(());
    }
    let Ok(below) = here.strip_prefix(&workspace) else {
        return Err(format!(
            "this chat stands outside workspace '{}', so the app will not update clone {repo} \
             for it",
            crate::shown::short(ws)
        ));
    };
    match below.components().next() {
        // The workspace's own folder: above its clones.
        None => Ok(()),
        Some(first) if first.as_os_str() == repo => Ok(()),
        Some(first) => Err(format!(
            "this chat stands in {}, so the app will not update clone {repo} for it",
            crate::shown::short(&first.as_os_str().to_string_lossy())
        )),
    }
}

/// Whether a chat standing at `cwd` stands in the project at `root`: at its root, or in one of
/// its workspaces. Where a chat may ask for a workspace to be made (qw387).
pub fn in_the_project(root: &Path, cwd: Option<&Path>) -> Result<(), String> {
    let Some(cwd) = cwd else {
        return Err(
            "the app has no record of where this chat stands, so it will not make a workspace \
             for it"
                .to_owned(),
        );
    };
    if crate::active::workspace_of_tree(root, cwd).is_some() || same_dir(root, cwd) {
        Ok(())
    } else {
        Err(
            "this chat stands outside the project's workspaces, so the app will not make a \
             workspace for it. Run the command from a chat in the project"
                .to_owned(),
        )
    }
}

/// The hosts a clone made for `asker` may reach: its sandbox's egress where it runs sandboxed
/// (D-5), or `None`, no limit, where it does not — a shell tab, a chat a person started without
/// the sandbox, or a project where none is in force. A `charter.toml` that cannot be read
/// may turn it on, so it refuses rather than reading as no limit.
pub fn hosts(root: &Path, asker: &Asker) -> Result<Option<Vec<String>>, String> {
    if !asker.harnessed || asker.unsandboxed {
        return Ok(None);
    }
    sandboxed_hosts(root)
}

/// The hosts a chat started under the project's sandbox may reach, or `None` where no sandbox
/// is in force: [`hosts`] for a chat that carries no opt-out, as a dispatched persona chat
/// never does (#3903, [`crate::dispatchplace::cut`]).
pub fn sandboxed_hosts(root: &Path) -> Result<Option<Vec<String>>, String> {
    let plane = crate::sandbox::Plane::read(root);
    if plane.unreadable() {
        return Err(crate::sandbox::NotStarted::PlaneUnreadable.to_string());
    }
    // An administrator's policy holds a clone's egress as it holds the chat's (#1343).
    let locks = crate::sandbox::policy::Locks::of(root);
    Ok(plane
        .in_force(&locks)
        .map(|policy| crate::sandbox::hosts(&locks.presets(&policy.egress), &plane, &locks)))
}

/// Who the piece log credits: the chat, by the app's number for it and its stable id (#3901),
/// and its persona (D-4).
pub fn who(asker: &Asker) -> Who {
    Who::here(
        asker.config.as_deref(),
        Some(asker.chat.to_string()),
        asker.persona.clone(),
    )
    .for_chat(asker.id.clone())
}

/// The most an answer's lines may take as JSON, under the 64 KiB an asker reads one answer in
/// (`hookwire::Asking::ask`), with room for the rest of the line.
pub const SAID_AT_MOST: usize = 56 * 1024;

/// The longest one line is kept, in characters, when the lines would not fit whole.
const A_LINE_AT_MOST: usize = 2000;

/// `lines`, cut to fit `budget` bytes of JSON: whole when they fit; otherwise each line held to
/// [`A_LINE_AT_MOST`] characters, then the last ones dropped for one warning that says how many.
/// A clone of many repos whose git said a great deal still answers, rather than an answer too
/// long for the asker to read reading as no answer at all.
pub fn fitted(lines: Vec<Say>, budget: usize) -> Vec<Say> {
    let size = |lines: &[Say]| serde_json::to_vec(lines).map_or(usize::MAX, |json| json.len());
    if size(&lines) <= budget {
        return lines;
    }
    let mut kept: Vec<Say> = lines.into_iter().map(shortened).collect();
    let mut cut = 0;
    let note = |cut: usize| {
        Say::Warn(
            format!(
                "{} more {} cut here to fit the app's answer; the app's log has the run",
                cut,
                plural(cut, "line was", "lines were")
            )
            .into(),
        )
    };
    while !kept.is_empty() && size(&[kept.as_slice(), &[note(cut + 1)]].concat()) > budget {
        kept.pop();
        cut += 1;
    }
    if cut > 0 {
        kept.push(note(cut));
    }
    kept
}

fn shortened(line: Say) -> Say {
    let cut = |text: String| {
        if text.chars().count() <= A_LINE_AT_MOST {
            text
        } else {
            let mut short: String = text.chars().take(A_LINE_AT_MOST).collect();
            short.push_str(" …");
            short
        }
    };
    match line {
        Say::Info(text) => Say::Info(text.shortened(A_LINE_AT_MOST)),
        Say::Done(text) => Say::Done(text.shortened(A_LINE_AT_MOST)),
        Say::Warn(text) => Say::Warn(text.shortened(A_LINE_AT_MOST)),
        Say::Fail(text) => Say::Fail(text.shortened(A_LINE_AT_MOST)),
        Say::Plain(text) => Say::Plain(text.shortened(A_LINE_AT_MOST)),
        Say::Out(text) => Say::Out(cut(text)),
    }
}

/// The repositories the work runs git in that are there before it starts: for a worktree,
/// the clone it is cut from; for a clone, every clone of the workspace, which a clone wires
/// (and an asked repo that is already cloned). A clone made by this call is new, and git
/// writes its config.
fn touched(root: &Path, ws: &str, work: &GitWork) -> Vec<PathBuf> {
    match work {
        GitWork::WorktreeAdd { repo, .. }
        | GitWork::WorktreeCut { repo, .. }
        | GitWork::WorktreeUpdate { repo, .. }
        | GitWork::WorktreePush { repo, .. }
        | GitWork::WorktreeRemove { repo, .. }
        | GitWork::CloneUpdate { repo, .. } => {
            if crate::contain::repo_name_ok(repo) && crate::contain::workspace_name_ok(ws) {
                vec![root.join("workspaces").join(ws).join(repo)]
            } else {
                // `piececmd::add` refuses the name before git runs.
                Vec::new()
            }
        }
        GitWork::Clone { .. } => crate::repos::clones(root, ws)
            .map(|found| found.repos.into_iter().map(|repo| repo.path).collect())
            .unwrap_or_default(),
        // A workspace that is not there yet holds no clone; one that is there is refused.
        // The root is the window's own Update's to check, as it checks it for the person.
        GitWork::Create { .. } | GitWork::RootUpdate {} => Vec::new(),
        // Runs no git.
        GitWork::WorktreeClaim { .. } => Vec::new(),
    }
}

/// **Whether a chat's plain git may `act` in branch folder `<repo>/<piece>` of workspace `ws`**
/// (#3931): the one-live-writer check a brokered write makes ([`crate::piececlaim::take`]),
/// asked by the Bash guard of a chat that runs git itself, so plain git in a folder another
/// live chat holds is refused in the brokered refusal's words, and a chat that writes in a
/// folder no live chat holds takes it. An empty `Said` with exit 0 where it may.
///
/// **A shell tab is the person's own terminal** and is never held to a claim, nor takes one:
/// the person hands any folder over. A name that cannot be a workspace, a repo or a piece is
/// refused.
fn claim(
    root: &Path,
    asker: &Asker,
    ws: &str,
    repo: &str,
    piece: &str,
    act: crate::hookwire::ClaimAct,
    now: chrono::DateTime<chrono::Utc>,
) -> Answer {
    let may = Answer::Said {
        lines: Vec::new(),
        code: 0,
    };
    if !asker.harnessed {
        return may;
    }
    if !crate::contain::workspace_name_ok(ws)
        || !crate::contain::repo_name_ok(repo)
        || !crate::worktree::name::piece_name_ok(piece)
    {
        return Answer::No {
            why: format!(
                "'{}' · '{}' · '{}' cannot name a branch folder",
                crate::shown::short(ws),
                crate::shown::short(repo),
                crate::shown::short(piece)
            ),
        };
    }
    let folder = crate::worktree::Located {
        workspace: ws.to_owned(),
        repo: repo.to_owned(),
        piece: piece.to_owned(),
    };
    match crate::piececlaim::take(
        root,
        &folder,
        &who(asker),
        asker.live.as_deref(),
        act.act(),
        now,
    ) {
        Ok(()) => may,
        Err(why) => Answer::No { why },
    }
}

/// Why `repo`'s own configuration names a program git could run for this call, or nothing.
///
/// Read as git resolves it — following an include, and a `commondir` to wherever it points —
/// under the isolation this thread holds, so only the repository's own scopes are listed
/// (`local`, `worktree`). Refused: a content filter (`filter.*`, which a committed
/// `.gitattributes` turns on at checkout and at status), a diff or merge driver
/// (`diff.*.textconv`, `diff.*.command`, `diff.external`, `merge.*.driver`), a transport
/// program (`core.sshCommand`, `core.gitProxy`, `remote.*.uploadpack`, `remote.*.receivepack`,
/// `core.alternateRefsCommand`), any `include` or `includeIf`, a `credential` helper, and what
/// a later git or a commit could start (#1476): a trailer's command (`trailer.*.cmd`,
/// `trailer.*.command`) and a hook a configuration defines (`hook.*`). A signer is refused by
/// the brokered commit alone ([`names_a_signer`]), since nothing else starts one. An editor is
/// not among them: a brokered commit is given its message and never starts one. A configuration git cannot read is refused too:
/// it may say any of these.
///
/// The one helper let through is the one purlis's own git policy writes into every clone it
/// makes (`credential.helper = !gh auth git-credential`, or `glab`'s; [`crate::gitpolicy`]),
/// which no git call made here reaches anyway: none of them but a clone crosses a network, and
/// a clone's call names its own helper.
///
/// **Which repository is checked is git's answer, not the path's** (D-1335-8): git is asked,
/// under the same isolation, which git directory it uses at `repo`, and anything but
/// `<repo>/.git` is refused, as is a `repo` with no `.git` at all. A bare repository a chat made
/// in its workspace, or a directory with none that discovery would take past, is never the one
/// the action runs in. Every repository checked is a clone (a worktree is cut from its clone),
/// so a linked worktree's git directory is never the answer. Answers the git directory it
/// checked, for the action to be pinned to.
pub fn runs_a_program(repo: &Path) -> Result<PathBuf, String> {
    checked_repo(repo).map(|found| found.git_dir)
}

/// What [`checked_repo`] found: the git directory it checked, and which directory that was
/// ([`git::DirId`]), for the action to be pinned to (#1415).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub git_dir: PathBuf,
    pub id: Option<git::DirId>,
}

/// Runs `add` with every git call held to the git directory `found` checked for `tree`, that
/// very directory (#1415), and checks once more after it: an add during which the directory
/// was replaced says so and fails, whatever git answered.
fn cut_pinned(
    held: &git::Isolated,
    tree: &Path,
    found: &Checked,
    say: &mut dyn FnMut(Say),
    add: impl FnOnce(&mut dyn FnMut(Say)) -> u8,
) -> u8 {
    let pinned = held.pinned_as(tree, &found.git_dir, found.id);
    let code = git::isolated(&pinned, || add(&mut *say));
    if pinned.holds() {
        code
    } else {
        say(Say::Fail(REPLACED.into()));
        code.max(1)
    }
}

/// What a worktree add says when the clone's git directory was replaced while it ran.
const REPLACED: &str = "the clone's git directory was replaced while the app worked in it, so \
    what it made may be another repository's. Look at the clone before you use it";

/// [`runs_a_program`], answering the identity of the git directory too. **Pinned by
/// identity** (#1415): the directory at `<repo>/.git` is taken by its device and inode before
/// anything is asked of it, and a check that finds another directory there by the end refuses.
/// **A narrowing, not a closed race** (ADR 0067, noted 2026-10-09): git opens the path again
/// after each comparison, so a swap after one, or a rename away and back between two, gets
/// past it. The boundary stays the denial of `.git` writes.
pub fn checked_repo(repo: &Path) -> Result<Checked, String> {
    let shown = repo.display();
    let not_a_clone = || {
        format!(
            "{shown} is not a clone with its own `.git`, so the app will not run git there for the \
             chat"
        )
    };
    // A real folder, never a link to one: where git keeps a clone is not for a chat to point.
    if !crate::worktree::is_own_git_dir(&repo.join(".git")) {
        return Err(not_a_clone());
    }
    let own = std::fs::canonicalize(repo.join(".git")).map_err(|_| not_a_clone())?;
    let id = git::DirId::of(&own);
    let resolved = git::run(repo, &["rev-parse", "--absolute-git-dir"], git::READ)
        .map_err(|e| e.to_string())?;
    let git_dir = std::fs::canonicalize(resolved.line()).map_err(|_| not_a_clone())?;
    if !resolved.ok() || git_dir != own {
        return Err(not_a_clone());
    }
    let listed = git::isolated(
        &git::isolation()
            .unwrap_or_default()
            .pinned_as(repo, &git_dir, id),
        || {
            git::run(
                repo,
                &["config", "--list", "--show-origin", "--show-scope", "-z"],
                git::READ,
            )
        },
    )
    .map_err(|e| e.to_string())?;
    if !listed.ok() {
        return Err(format!(
            "git could not read the configuration of {shown}, so the app will not run git there \
             for the chat: {}",
            listed.err.trim()
        ));
    }
    let mut fields = listed.out.split('\0');
    while let (Some(scope), Some(origin), Some(entry)) =
        (fields.next(), fields.next(), fields.next())
    {
        if scope == "command" {
            continue;
        }
        let (key, value) = entry.split_once('\n').unwrap_or((entry, ""));
        if names_a_program(key) && !is_the_policys_helper(key, value) {
            return Err(format!(
                "{shown} sets `{key}` ({origin}), which names a program git would run outside \
                 the chat's sandbox, so the app will not run git there for the chat. Run the \
                 command in your own terminal, or remove the key"
            ));
        }
    }
    if git::DirId::of(&git_dir) != id {
        return Err(format!(
            "{shown}'s `.git` was replaced while the app checked it, so the app will not run git \
             there for the chat. Run the command again"
        ));
    }
    Ok(Checked { git_dir, id })
}

/// Whether config key `key` (as git lists it: section and name lowercased) names a program.
fn names_a_program(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    let (section, rest) = key.split_once('.').unwrap_or((key.as_str(), ""));
    let name = rest.rsplit('.').next().unwrap_or(rest);
    let sub = rest.contains('.');
    match section {
        "filter" | "include" | "includeif" => true,
        "diff" => key == "diff.external" || (sub && matches!(name, "textconv" | "command")),
        "merge" => sub && name == "driver",
        "core" => matches!(name, "sshcommand" | "gitproxy" | "alternaterefscommand"),
        "remote" => sub && matches!(name, "uploadpack" | "receivepack" | "vcs"),
        // Commands the gc a commit can start runs (#1853).
        "gc" => !sub && name == "recentobjectshook",
        "credential" => name == "helper",
        // A trailer's command, which `commit --trailer` and `interpret-trailers` run.
        "trailer" => sub && matches!(name, "cmd" | "command"),
        // A hook a configuration defines, which a later git runs whatever the hooks folder is:
        // the whole section, its event as well as its command.
        "hook" => sub,
        _ => false,
    }
}

/// Whether config key `key` names a signer (`gpg.program`, `gpg.<format>.program`,
/// `gpg.ssh.defaultKeyCommand`). Only a commit could start one, so only the brokered commit
/// refuses it ([`commit`]): a repository that names a signer means its commits to be signed,
/// and a commit the app makes for a chat is never signed (#1476). A clone or a worktree add
/// starts no signer, so it is not refused there.
fn names_a_signer(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    let Some(rest) = key.strip_prefix("gpg.") else {
        return false;
    };
    rest == "program"
        || rest == "ssh.defaultkeycommand"
        || rest
            .strip_suffix(".program")
            .is_some_and(|format| !format.is_empty() && !format.contains('.'))
}

/// Whether `key = value` is the credential helper purlis's git policy writes into a clone.
fn is_the_policys_helper(key: &str, value: &str) -> bool {
    key.eq_ignore_ascii_case("credential.helper")
        && crate::forge::KINDS
            .iter()
            .any(|kind| value == format!("!{} auth git-credential", kind.cli()))
}

/// Whether two paths name one directory, links resolved.
fn same_dir(a: &Path, b: &Path) -> bool {
    let real = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    real(a) == real(b)
}

pub mod commit;

#[cfg(test)]
mod tests;
