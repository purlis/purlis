// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where a workspace's repo is** (ADR 0113), the one module that knows both layouts.
//!
//! A repo made from structure version 6 on is at `workspaces/<ws>/repos/<repo>/`. One made before
//! it is at `workspaces/<ws>/<repo>/`, and is read there for ever. A workspace may hold repos in
//! both places. Three questions, and nothing else here:
//!
//! - [`folder`]: a repo's folder, from its name. `repos/<repo>` when that exists, else `<repo>`,
//!   so a repo in `repos/` wins over one of the same name at the root.
//! - [`repos`]: the repos a workspace holds, in both places, for the code that walks the
//!   workspace's folder for `.git`.
//! - [`new_folder`]: where a new repo goes, refused while an old repo named `repos` stands where
//!   the folder `repos/` would be.
//!
//! **An old repo named `repos`.** A `repos/` with `.git` directly in it is a repo cloned at the
//! root before version 6, never the folder new repos go in: what is inside it is that repo's own
//! files, so none of it is read as another repo of the workspace's.
//!
//! Nothing here follows a link: a `repos` that is a link is not the folder new repos go in, and
//! an entry that is a link is no repo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::contain;
use crate::worktree::confine::{self, Outside};

/// The folder a workspace's repos go in, from structure version 6 on.
pub const REPOS: &str = "repos";

/// Which of the two places a repo is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// `workspaces/<ws>/repos/<repo>`, where every repo goes from structure version 6 on.
    Repos,
    /// `workspaces/<ws>/<repo>`, where a repo made before it stays.
    Root,
}

/// One repo a workspace holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// The folder's name, which is how purlis addresses the repo in either place.
    pub name: String,
    /// The repo's folder, in the workspace's folder as [`confine::workspace_dir`] checked it.
    pub path: PathBuf,
    pub layout: Layout,
}

/// Why a question about a workspace's repo was not answered.
#[derive(Debug, thiserror::Error)]
pub enum Refused {
    #[error("'{0}' is not a name purlis will take for a repo")]
    NotARepoName(String),
    #[error(transparent)]
    Outside(#[from] Outside),
    #[error("could not read workspace '{ws}' ({why})")]
    Unreadable { ws: String, why: String },
    #[error(
        "workspace '{ws}' has a repo named 'repos' in its folder, where new repos now go, so no \
         repo can be added to it until you rename that one: move workspaces/{ws}/repos to \
         another name, run `git worktree repair` in it if it has branch folders, then add the \
         repo again"
    )]
    RepoNamedRepos { ws: String },
    #[error(
        "workspace '{ws}' has a 'repos' that is not a folder, where new repos now go: move \
         workspaces/{ws}/repos aside, then add the repo again"
    )]
    ReposNotAFolder { ws: String },
}

/// **Repo `repo`'s folder in workspace `ws`**: `repos/<repo>` when that exists, else `<repo>`,
/// whether or not anything is there. A caller asks of it what it asked of the path it joined by
/// hand before: whether it exists, whether it is a repo.
///
/// Refused only for a name that is not one folder's ([`contain::segment_ok`]) or a workspace's
/// name that is not one; the path is not checked against links, which is still the caller's.
pub fn folder(plane: &Path, ws: &str, repo: &str) -> Result<PathBuf, Refused> {
    if !contain::workspace_name_ok(ws) {
        return Err(Outside::NotAWorkspaceName(ws.to_string()).into());
    }
    if !contain::segment_ok(repo) {
        return Err(Refused::NotARepoName(repo.to_string()));
    }
    let dir = plane.join("workspaces").join(ws);
    if let Some(repos) = repos_folder(&dir) {
        let new = repos.join(repo);
        if std::fs::symlink_metadata(&new).is_ok() {
            return Ok(new);
        }
    }
    Ok(dir.join(repo))
}

/// **The repos workspace `ws` holds**, in both places, sorted by name: each folder whose `.git`
/// is a directory, reached through no link.
///
/// Left out, as the scanners before it left them out: a dot-name (`.worktrees/` and its branch
/// folders among them), a name purlis takes for no repo, and at the root a name the workspace
/// keeps for its own ([`contain::workspace_keeps`]). Where a name is in both places, the one in
/// `repos/` is the repo.
pub fn repos(plane: &Path, ws: &str) -> Result<Vec<Repo>, Refused> {
    let dir = confine::workspace_dir(plane, ws)?;
    let mut found = BTreeMap::new();
    for name in names(&dir, ws)? {
        // `repos` is an old repo's name, read as one, even once it is a name the workspace keeps.
        if contain::workspace_keeps(&name) && name != REPOS {
            continue;
        }
        let path = dir.join(&name);
        if is_repo(&path) {
            found.insert(
                name.clone(),
                Repo {
                    name,
                    path,
                    layout: Layout::Root,
                },
            );
        }
    }
    if let Some(folder) = repos_folder(&dir) {
        for name in names(&folder, ws)? {
            let path = folder.join(&name);
            if is_repo(&path) {
                found.insert(
                    name.clone(),
                    Repo {
                        name,
                        path,
                        layout: Layout::Repos,
                    },
                );
            }
        }
    }
    Ok(found.into_values().collect())
}

/// **Where a new repo named `repo` goes in workspace `ws`**: `repos/<repo>`, whatever the
/// workspace already holds. Whether something is already there is the caller's to say.
///
/// Refused while an old repo named `repos` stands where the folder would be, and while `repos`
/// is anything but a folder: a link would take the clone out of the workspace.
pub fn new_folder(plane: &Path, ws: &str, repo: &str) -> Result<PathBuf, Refused> {
    if !contain::repo_name_ok(repo) {
        return Err(Refused::NotARepoName(repo.to_string()));
    }
    let dir = confine::workspace_dir(plane, ws)?;
    let repos = dir.join(REPOS);
    match std::fs::symlink_metadata(&repos) {
        Ok(meta) if meta.is_dir() => {
            if std::fs::symlink_metadata(repos.join(".git")).is_ok() {
                return Err(Refused::RepoNamedRepos { ws: ws.to_string() });
            }
        }
        Ok(_) => return Err(Refused::ReposNotAFolder { ws: ws.to_string() }),
        Err(_) => {}
    }
    Ok(repos.join(repo))
}

/// `dir/repos` when it is the folder new repos go in: a folder, not a link, with no `.git` in it.
fn repos_folder(dir: &Path) -> Option<PathBuf> {
    let repos = dir.join(REPOS);
    let meta = std::fs::symlink_metadata(&repos).ok()?;
    let plain = meta.is_dir() && std::fs::symlink_metadata(repos.join(".git")).is_err();
    plain.then_some(repos)
}

/// The names in `dir` a repo may have, dot-names left out.
fn names(dir: &Path, ws: &str) -> Result<Vec<String>, Refused> {
    let reader = std::fs::read_dir(dir).map_err(|why| Refused::Unreadable {
        ws: ws.to_string(),
        why: why.to_string(),
    })?;
    Ok(reader
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.') && contain::repo_name_ok(name))
        .collect())
}

/// A folder reached through no link whose `.git` is a directory, which is how git tells a repo
/// from a linked worktree (whose `.git` is a file).
fn is_repo(path: &Path) -> bool {
    let plain_dir = |p: &Path| std::fs::symlink_metadata(p).is_ok_and(|meta| meta.is_dir());
    plain_dir(path) && plain_dir(&path.join(".git"))
}
