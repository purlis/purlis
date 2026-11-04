// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A finished folder** (#5168, ADR 0110): a branch folder whose removal loses nothing and
//! takes nothing from under anyone.
//!
//! - no uncommitted change, less purlis's own layer;
//! - every commit on its base, or carried there by a squash merge;
//! - no ignored path beyond the folders only a build or the harness writes, and links;
//! - its task, where purlis cut it for one, has ended;
//! - no open chat that has declared neither `done` nor `abandoned` holds it;
//! - no other remaining folder links into it.
//!
//! This module reads the parts that are the folder's own ([`own`]) and the links between
//! folders ([`links`]). A task's end is [`crate::dispatchplace`]'s to read, and a chat's hold
//! [`crate::piececlaim`]'s, each where the act that needs it runs.

use std::path::{Path, PathBuf};

use crate::worktree::{self, git, standing};

/// How a finished folder's commits stand against its base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commits {
    /// Every commit of its branch is on its base: `git branch -d` deletes the branch.
    OnBase,
    /// Its branch landed by its files, as a squash merge leaves it
    /// ([`standing::Landed::Carried`]): only `git branch -D` deletes it.
    Carried,
    /// One commit already on its base makes its whole change, though later commits may have
    /// changed those files since ([`standing::change_in_history`]). Its branch is kept.
    InHistory,
    /// On no branch, at a commit its base has.
    NoBranch,
}

/// Why a folder is not finished, as it is said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotFinished {
    /// git did not say how it stands, or it is not a folder of the workspace's.
    Unread(String),
    /// Uncommitted changes, as `git status` prints them.
    Uncommitted(Vec<String>),
    /// A commit its base does not have, by any reading.
    Ahead,
    /// Ignored paths that are the task's, not a build's.
    Ignored(Vec<String>),
    /// Other folders link into it, each as `<repo> · <piece>`.
    LinkedInto(Vec<String>),
}

impl NotFinished {
    /// The reason, as a clause: "it holds uncommitted changes (`a.md`)".
    pub fn why(&self) -> String {
        let named = |paths: &[String]| {
            let mut named: Vec<String> = paths.iter().take(5).map(|p| format!("`{p}`")).collect();
            if paths.len() > 5 {
                named.push(format!("and {} more", paths.len() - 5));
            }
            named.join(", ")
        };
        match self {
            Self::Unread(why) => why.clone(),
            Self::Uncommitted(paths) => {
                format!("it holds uncommitted changes ({})", named(paths))
            }
            Self::Ahead => "its branch holds commits its base does not have".to_owned(),
            Self::Ignored(paths) => format!(
                "it holds files git ignores that are not a build's ({})",
                named(paths)
            ),
            Self::LinkedInto(by) => format!("{} links into it", by.join(" and ")),
        }
    }
}

/// **How the folder `piece` of `repo` stands by its own files and commits**: `Ok` with how
/// its commits stand where nothing in it would be lost with it, else the first reason it is
/// not finished. Links into it, its task and its chat are read apart.
pub fn own(plane: &Path, ws: &str, repo: &str, piece: &str) -> Result<Commits, NotFinished> {
    let unread = |why: String| NotFinished::Unread(why);
    let found = worktree::list(plane, ws, repo)
        .map_err(|why| unread(why.to_string()))?
        .into_iter()
        .find(|p| p.piece == piece && p.prunable.is_none())
        .ok_or_else(|| unread(format!("git has no folder '{piece}' of {repo}")))?;
    let risk = standing::at_risk(plane, ws, repo, piece)
        .map_err(|why| unread(why.to_string()))?
        .ok_or_else(|| unread(format!("{repo} · {piece} is not there")))?;
    let Some(changes) = risk.changes else {
        return Err(unread(format!(
            "git would not say what {repo} · {piece} holds"
        )));
    };
    let hidden = crate::guest::hidden_by_purlis(&found.path);
    let changes: Vec<String> = changes
        .iter()
        .map(|line| line.get(3..).unwrap_or(line).to_owned())
        .filter(|path| !crate::guest::is_purlis_own(&hidden, path))
        .collect();
    if !changes.is_empty() {
        return Err(NotFinished::Uncommitted(changes));
    }
    let commits = match found.branch.as_deref() {
        Some(branch) => match standing::landed(plane, ws, repo, piece, branch) {
            standing::Landed::Yes { .. } => Commits::OnBase,
            standing::Landed::Carried { .. } => Commits::Carried,
            _ if standing::change_in_history(plane, ws, repo, piece, branch) => Commits::InHistory,
            _ => return Err(NotFinished::Ahead),
        },
        None if detached_on_its_base(plane, ws, repo, &found.path) => Commits::NoBranch,
        None => return Err(NotFinished::Ahead),
    };
    match crate::piececmd::ignored_kept(plane, ws, repo, piece) {
        Some(Ok(kept)) if kept.is_empty() => Ok(commits),
        Some(Ok(kept)) => Err(NotFinished::Ignored(kept)),
        Some(Err(why)) => Err(unread(why)),
        None => Err(unread(format!("{repo} · {piece} is not there"))),
    }
}

/// Whether the folder at `path`, on no branch, is at a commit its base has: the branch the
/// clone's `origin` names its default (`refs/remotes/origin/HEAD`, as the clone last fetched
/// it), or that branch of the clone's own, or else the branch the clone has checked out. Read
/// from the clone, never asked of the remote.
fn detached_on_its_base(plane: &Path, ws: &str, repo: &str, path: &Path) -> bool {
    let Ok(clone) = worktree::clone_dir(plane, ws, repo) else {
        return false;
    };
    let read = |args: &[&str]| {
        git::run(&clone, args, git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|run| run.out.trim().to_owned())
            .filter(|out| !out.is_empty())
    };
    let mut bases: Vec<String> = Vec::new();
    if let Some(remote) = read(&["symbolic-ref", "-q", "refs/remotes/origin/HEAD"]) {
        if let Some(name) = remote.strip_prefix("refs/remotes/origin/") {
            bases.push(format!("refs/heads/{name}"));
        }
        bases.push(remote);
    }
    if bases.is_empty()
        && let Some(own) = read(&["symbolic-ref", "-q", "HEAD"])
    {
        bases.push(own);
    }
    let Some(head) = git::run(path, &["rev-parse", "--verify", "-q", "HEAD"], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|run| run.out.trim().to_owned())
    else {
        return false;
    };
    bases.iter().any(|base| {
        git::run(
            &clone,
            &["merge-base", "--is-ancestor", &head, base],
            git::READ,
        )
        .is_ok_and(|run| run.ok())
    })
}

/// One branch folder of a workspace, by its repo and piece.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Folder {
    pub repo: String,
    pub piece: String,
}

impl Folder {
    /// `<repo> · <piece>`, as purlis names a branch folder in a sentence.
    pub fn named(&self) -> String {
        format!("{} · {}", self.repo, self.piece)
    }
}

/// A link in one branch folder (`from`) that points into another (`into`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub from: Folder,
    pub into: Folder,
}

/// The branch folders of a workspace, of every repo, as git lists them.
#[derive(Debug, Clone, Default)]
pub struct Listed {
    /// Every repo of the workspace, by name.
    pub repos: Vec<String>,
    /// Each folder, and whether only git's record of it is left (its folder is gone).
    pub folders: Vec<(Folder, bool)>,
    /// The repos whose folders git would not list, and why.
    pub unread: Vec<(String, String)>,
}

/// **Every branch folder of `ws`**, of every repo, sorted; `Err` where the workspace cannot be
/// read at all.
pub fn folders(plane: &Path, ws: &str) -> Result<Listed, String> {
    let clones = crate::repos::clones(plane, ws).map_err(|why| why.to_string())?;
    let mut listed = Listed::default();
    for repo in clones.repos {
        match worktree::list(plane, ws, &repo.name) {
            Ok(found) => listed.folders.extend(found.into_iter().map(|p| {
                (
                    Folder {
                        repo: repo.name.clone(),
                        piece: p.piece,
                    },
                    p.prunable.is_some(),
                )
            })),
            Err(why) => listed.unread.push((repo.name.clone(), why.to_string())),
        }
        listed.repos.push(repo.name);
    }
    listed.folders.sort();
    Ok(listed)
}

/// **Every link between the branch folders of `ws`**, of every repo: a symbolic link git
/// lists as untracked or ignored in one folder whose target is inside another. A folder links
/// into itself as it likes. An untracked folder is read file by file; an ignored one is not
/// read into, so a link inside it (a `node_modules/.bin`) is not one.
pub fn links(plane: &Path, ws: &str) -> Vec<Link> {
    let folders: Vec<(Folder, PathBuf)> = folders(plane, ws)
        .map(|listed| listed.folders)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, gone)| !gone)
        .filter_map(|(folder, _)| {
            let path = worktree::path_for(plane, ws, &folder.repo, &folder.piece).ok()?;
            Some((folder, std::fs::canonicalize(path).ok()?))
        })
        .collect();
    let mut found = Vec::new();
    for (from, at) in &folders {
        for target in link_targets(at) {
            if let Some((into, _)) = folders
                .iter()
                .filter(|(other, _)| other != from)
                .find(|(_, real)| target.starts_with(real))
            {
                found.push(Link {
                    from: from.clone(),
                    into: into.clone(),
                });
            }
        }
    }
    found.sort_by(|a, b| (&a.from, &a.into).cmp(&(&b.from, &b.into)));
    found.dedup();
    found
}

/// Where each link `git status` names as untracked or ignored in the folder at `at` points,
/// resolved; a link that points nowhere is left out.
fn link_targets(at: &Path) -> Vec<PathBuf> {
    let Ok(seen) = git::run(
        at,
        &[
            "status",
            "--porcelain",
            "-z",
            "--untracked-files=all",
            "--ignored=matching",
        ],
        git::READ,
    ) else {
        return Vec::new();
    };
    if !seen.ok() {
        return Vec::new();
    }
    seen.out
        .split('\0')
        .filter_map(|entry| {
            entry
                .strip_prefix("?? ")
                .or_else(|| entry.strip_prefix("!! "))
        })
        .filter(|path| !path.ends_with('/'))
        .map(|path| at.join(path))
        .filter(|path| {
            path.symlink_metadata()
                .is_ok_and(|meta| meta.file_type().is_symlink())
        })
        .filter_map(|path| std::fs::canonicalize(path).ok())
        .collect()
}

/// The folders of `links` that link into `folder`, named, sorted.
pub fn linking_into(links: &[Link], folder: &Folder) -> Vec<String> {
    let mut by: Vec<String> = links
        .iter()
        .filter(|link| &link.into == folder)
        .map(|link| link.from.named())
        .collect();
    by.dedup();
    by
}

/// **Why the folder `piece` of `repo` is not finished by its files, commits and the links of
/// the workspace's other folders into it**, or how its commits stand. Its task and its chat
/// are the caller's to read.
pub fn as_it_stands(
    plane: &Path,
    ws: &str,
    repo: &str,
    piece: &str,
) -> Result<Commits, NotFinished> {
    let commits = own(plane, ws, repo, piece)?;
    let by = linking_into(
        &links(plane, ws),
        &Folder {
            repo: repo.to_owned(),
            piece: piece.to_owned(),
        },
    );
    if by.is_empty() {
        Ok(commits)
    } else {
        Err(NotFinished::LinkedInto(by))
    }
}
