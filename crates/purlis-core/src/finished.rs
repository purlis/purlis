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

use crate::hookwire::alive;
use crate::worktree::{self, git, standing};

pub mod merged;
pub use merged::{Forge, Request};

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
    /// Neither on its base nor carried, but its tip is inside the head of a request that
    /// merged, by its number ([`merged::holding`], #5439): its commits are part of what merged.
    /// Only `git branch -D` deletes it.
    InMergedRequest(u64),
}

impl Commits {
    /// Why deleting a branch that stands so loses nothing, as a clause: "it is on its base".
    pub fn why_its_branch_goes(self) -> String {
        match self {
            Self::Carried => "it is carried by a squash merge".to_owned(),
            Self::InMergedRequest(number) => {
                format!("its commits are in pull request {number}, which merged")
            }
            Self::OnBase | Self::InHistory | Self::NoBranch => "it is on its base".to_owned(),
        }
    }
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
    /// A commit its base does not have, and whether a merged request holds it could not be
    /// told, and why: offline, or no forge for the clone (#5439).
    AheadUnasked(String),
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
            Self::AheadUnasked(why) => format!(
                "its branch holds commits its base does not have, and whether a merged pull \
                 request holds them could not be told ({why})"
            ),
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
/// not finished. A branch neither on its base nor carried is asked of `forge`: its tip may be
/// inside a merged request's head. Links into it, its task and its chat are read apart.
pub fn own(
    plane: &Path,
    ws: &str,
    repo: &str,
    piece: &str,
    forge: Forge<'_>,
) -> Result<Commits, NotFinished> {
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
            _ => match merged::holding(plane, ws, repo, branch, forge) {
                Ok(Some(number)) => Commits::InMergedRequest(number),
                Ok(None) => return Err(NotFinished::Ahead),
                Err(why) => return Err(NotFinished::AheadUnasked(why)),
            },
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

/// **How `branch` of `repo` stands, a branch purlis cut whose folder is gone** (#5439): `Ok`
/// where deleting it loses nothing, as it is on its base, carried there, or inside a merged
/// request's head ([`merged::holding`]), else why it stays. `None` where it is not a branch
/// purlis cut: no base is recorded for it.
pub fn branch_only(
    plane: &Path,
    ws: &str,
    repo: &str,
    branch: &str,
    forge: Forge<'_>,
) -> Option<Result<Commits, NotFinished>> {
    Some(match standing::branch_landed(plane, ws, repo, branch) {
        standing::Landed::Unknown => return None,
        standing::Landed::Yes { .. } => Ok(Commits::OnBase),
        standing::Landed::Carried { .. } => Ok(Commits::Carried),
        standing::Landed::No => match merged::holding(plane, ws, repo, branch, forge) {
            Ok(Some(number)) => Ok(Commits::InMergedRequest(number)),
            Ok(None) => Err(NotFinished::Ahead),
            Err(why) => Err(NotFinished::AheadUnasked(why)),
        },
    })
}

/// **The branches of `repo` purlis cut whose folder is gone** (#5439), sorted: each local
/// branch with a base recorded for it (`branch.<name>.charterBase`) that no folder of the
/// clone, the clone's own checkout included, is on. A removal by hand, or by an older purlis,
/// takes the folder and leaves its branch.
pub fn branches_without_a_folder(
    plane: &Path,
    ws: &str,
    repo: &str,
) -> Result<Vec<String>, String> {
    let clone = worktree::clone_dir(plane, ws, repo).map_err(|why| why.to_string())?;
    let read = |args: &[&str]| -> Result<String, String> {
        let run = git::run(&clone, args, git::READ).map_err(|why| why.to_string())?;
        // `config --get-regexp` exits 1 where nothing matches.
        if run.ok() || (run.code == Some(1) && run.err.trim().is_empty()) {
            Ok(run.out)
        } else {
            Err(format!(
                "git would not list the branches of {repo}: {}",
                run.err.lines().next().unwrap_or("").trim()
            ))
        }
    };
    let recorded = read(&["config", "--get-regexp", r"^branch\..*\.charterbase$"])?;
    let on: Vec<String> = git::run(&clone, &["worktree", "list", "--porcelain"], git::READ)
        .ok()
        .filter(git::Run::ok)
        .ok_or_else(|| format!("git would not list the folders of {repo}"))?
        .out
        .lines()
        .filter_map(|line| line.strip_prefix("branch refs/heads/"))
        .map(str::to_owned)
        .collect();
    let mut found: Vec<String> = recorded
        .lines()
        .filter_map(|line| line.split_once(' ').map(|(key, _)| key))
        .filter_map(|key| {
            key.strip_prefix("branch.")
                .and_then(|rest| rest.strip_suffix(".charterbase"))
        })
        .filter(|branch| worktree::name::branch_name_ok(branch).is_ok())
        .filter(|branch| {
            git::run(
                &clone,
                &[
                    "show-ref",
                    "--verify",
                    "--quiet",
                    &worktree::name::as_ref(branch),
                ],
                git::READ,
            )
            .is_ok_and(|run| run.ok())
        })
        .filter(|branch| !on.iter().any(|held| held == branch))
        .map(str::to_owned)
        .collect();
    found.sort();
    found.dedup();
    Ok(found)
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
    // Each folder's `git status`, read several at once and said alive after each (#5439).
    let targets = each_at_once(&folders, |(_, at)| link_targets(at));
    for ((from, _), targets) in folders.iter().zip(targets) {
        for target in targets {
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
    forge: Forge<'_>,
) -> Result<Commits, NotFinished> {
    let commits = own(plane, ws, repo, piece, forge)?;
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

/// The most folders read at once: each reading is a handful of git processes, and a reading
/// waits on the disk as much as on a core.
const AT_ONCE: usize = 8;

/// `judge` of each of `items`, in their order, read on as many threads as the machine has
/// cores, at most [`AT_ONCE`] (#5439): each folder's reading is its own and only reads, and a
/// workspace of about 290 folders took 21 minutes read one by one. A thread that panics
/// panics this one, once the others have read what is left.
///
/// Each thread runs its git as this one does ([`git::within`]) and says the work is alive
/// after each item it read ([`crate::hookwire::alive`]): a chat's wait on the app bounds a
/// silence, and a workspace may hold any number of folders.
pub(crate) fn each_at_once<T: Sync, R: Send>(
    items: &[T],
    judge: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let isolation = git::isolation();
    let pulse = alive::held();
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .clamp(1, AT_ONCE)
        .min(items.len());
    let next = AtomicUsize::new(0);
    let read: Vec<(usize, R)> = std::thread::scope(|scope| {
        let reading: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    git::within(isolation.as_ref(), || {
                        alive::within(pulse.as_ref(), || {
                            let mut read = Vec::new();
                            loop {
                                let at = next.fetch_add(1, Ordering::Relaxed);
                                let Some(item) = items.get(at) else {
                                    return read;
                                };
                                read.push((at, judge(item)));
                                alive::beat();
                            }
                        })
                    })
                })
            })
            .collect();
        reading
            .into_iter()
            .flat_map(|thread| {
                thread
                    .join()
                    .unwrap_or_else(|panicked| std::panic::resume_unwind(panicked))
            })
            .collect()
    });
    let mut out: Vec<Option<R>> = items.iter().map(|_| None).collect();
    for (at, judged) in read {
        out[at] = Some(judged);
    }
    out.into_iter()
        .map(|judged| judged.expect("every item is taken by one thread and read"))
        .collect()
}
