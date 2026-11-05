// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where a workspace's repo is** (ADR 0114), the one module that knows both layouts.
//!
//! A repo made from structure version 6 on is at `workspaces/<ws>/repos/<repo>/`. One made before
//! it is at `workspaces/<ws>/<repo>/`, and is read there for ever. A workspace may hold repos in
//! both places. What it answers:
//!
//! - [`folder`]: a repo's folder, from its name.
//! - [`holdings`]: the one walk of a workspace's two places, with what each scanner needs to know
//!   about every row. [`repos`] is its answer alone, and [`crate::repos::clones`] sits on it.
//! - [`new_folder`]: where a new repo goes.
//! - [`locate`] and [`locate_from`]: from a path inside a repo back to the repo.
//!
//! **One rule for what a repo is.** A folder reached through no link, whose `.git` is a directory,
//! with a name purlis takes for a repo ([`name_ok`]). [`repos`] lists those, and `repos/<name>`
//! wins over `<name>` only when it is one: an empty folder, a file, a link or a linked worktree
//! there beats nothing. **The rule decides the list and the precedence, not every answer:**
//! [`folder`] checks only the name, the workspace and what it prefers, so `<name>` comes back as
//! it stands, as the path a caller joined by hand did. [`locate`] answers any `.git` and says which.
//!
//! **An old clone in a kept name.** A clone at the root named as the workspace keeps (`work`,
//! `memory`) keeps working (ADR 0114, decision 3): [`folder`] and [`locate`] answer it, though
//! [`repos`] does not list it. `repos/<kept>` is never a repo.
//!
//! **An old repo named `repos`.** A `repos/` with `.git` directly in it is a repo cloned at the
//! root before version 6, never the folder new repos go in: what is inside it is that repo's own
//! files, so none of it is read as another repo of the workspace's.
//!
//! Nothing here follows a link: a `repos` that is a link is not the folder new repos go in, an
//! entry that is a link is no repo, and a `.git` that is a link is no repository. The walk says
//! each rather than dropping it.

use std::collections::BTreeSet;
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

/// One repo: its name and the folder that was checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// The directory's name, which is how charter addresses the clone.
    pub name: String,
    /// The path that was **checked**, which is the one a caller hands to git. Returning the
    /// checked path rather than letting the caller rebuild it is `within_workspace`'s own
    /// rule: a string checked and a different string used is how a link gets laundered.
    pub path: PathBuf,
}

/// What sits where a repo's `.git` would be, asked without following a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Git {
    /// A directory, which is how git draws the line: a repo.
    Directory,
    /// A file holding `gitdir:`: a linked worktree or a submodule.
    File,
    /// A link, which purlis will not run git through.
    Link,
}

/// One folder a workspace holds with a `.git` in it, repo or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub repo: Repo,
    pub layout: Layout,
    pub git: Git,
}

/// An entry the walk says something about rather than dropping it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// The entry's name, as far as it reads as text. Empty for an entry the system would not name.
    pub name: String,
    /// The place it is in.
    pub layout: Layout,
    pub why: String,
}

/// What the walk of a workspace's two places found, each list sorted by name.
///
/// Dot-names are in none of it: `.worktrees/` and purlis's own folders are nobody's repo, and
/// never were. A folder with no `.git` is in none of it either, being the person's own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Holdings {
    /// **The repos**, one for each name: [`Git::Directory`] and a name purlis takes for a repo.
    pub repos: Vec<Row>,
    /// A folder at the root with a `.git` of any kind, beside the repo of its name in `repos/`.
    /// The repo in `repos/` is the repo; this is what `doctor` warns about (ADR 0114, decision 3).
    pub shadowed: Vec<Row>,
    /// A folder with a `.git` that is no repo and shadows nothing: one whose `.git` is a file or
    /// a link, or whose name the workspace keeps for its own ([`contain::workspace_keeps`]). A
    /// scanner that has counted one of these as a clone keeps doing so from here.
    pub others: Vec<Row>,
    /// A name that is no repo's, or an entry that is a link.
    pub refused: Vec<Said>,
    /// What the system would not show: `repos/` itself, or an entry's `.git`. What could be read
    /// is still answered, so one locked folder does not lose a workspace its repos.
    pub unreadable: Vec<Said>,
}

/// The folder with a `.git` that a path is inside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    /// The plane, as it was given or as [`locate_from`] found it.
    pub plane: PathBuf,
    pub ws: String,
    /// The repo's folder, built from `plane`. A caller asking whether the path is the repo's own
    /// folder rather than one deep in it (a chat's working directory) asks it of this.
    pub repo: Repo,
    pub layout: Layout,
    /// What its `.git` is, of any kind: only [`Git::Directory`] is a repository.
    pub git: Git,
}

/// charter could not look at the workspace at all.
#[derive(Debug, thiserror::Error)]
pub enum Trouble {
    #[error(transparent)]
    Outside(#[from] Outside),
    #[error("could not read workspace '{ws}' ({why})")]
    Unreadable { ws: String, why: String },
}

/// Why a question about a workspace's repo was not answered.
#[derive(Debug, thiserror::Error)]
pub enum Refused {
    #[error("'{0}' is not a name purlis will take for a repo")]
    NotARepoName(String),
    #[error(transparent)]
    Trouble(#[from] Trouble),
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

/// **Repo `repo`'s folder in workspace `ws`**: `repos/<repo>` when [`repos`] would list that, else
/// `<repo>`, whether or not anything is there. A caller asks of it what it asked of the path it
/// joined by hand before: whether it exists, whether it is a repo.
///
/// **What it checks:** the name ([`contain::repo_name_ok`]), the workspace
/// ([`confine::workspace_dir`]) and `repos/<repo>` before preferring it. `<repo>` is not checked:
/// it may be a link, or hold a `.git` that is a link, which [`holdings`] refuses, and a caller
/// gates it as it gated the hand-joined path ([`confine::within_workspace`]).
///
/// **A name the workspace keeps** answers `<kept>`, never `repos/<kept>`, so an old clone there
/// keeps working. In a workspace with no clone there, that is still the hand-joined path, with no
/// repo implied (`memory` is the workspace's own): a caller who needs to know whether there is a
/// repo asks [`holdings`].
///
/// A workspace with no folder has the same answer, as a hand-joined path did.
pub fn folder(plane: &Path, ws: &str, repo: &str) -> Result<PathBuf, Refused> {
    if !contain::repo_name_ok(repo) {
        return Err(Refused::NotARepoName(repo.to_string()));
    }
    let Some(dir) = anchored(plane, ws)? else {
        return Ok(plane.join("workspaces").join(ws).join(repo));
    };
    if let Some(repos) = repos_folder(&dir).filter(|_| name_ok(repo)) {
        let new = repos.join(repo);
        if confine::within_workspace(plane, ws, &new).is_ok()
            && matches!(git_of(&new), Ok(Some(Git::Directory)))
        {
            return Ok(new);
        }
    }
    Ok(dir.join(repo))
}

/// **The walk of workspace `ws`'s two places**, the one place that walks them. See [`Holdings`].
///
/// A workspace with no folder holds nothing, as `codelink::clones_in` has it; a caller to whom
/// that is an error asks [`confine::workspace_dir`] first, as [`crate::repos::clones`] does. A
/// workspace reached through a link, or one that is a file, is refused.
pub fn holdings(plane: &Path, ws: &str) -> Result<Holdings, Trouble> {
    let Some(dir) = anchored(plane, ws)? else {
        return Ok(Holdings::default());
    };
    let mut found = Holdings::default();
    let root = read_names(&dir).map_err(|why| Trouble::Unreadable {
        ws: ws.to_string(),
        why: why.to_string(),
    })?;
    let at_root = walk(plane, ws, &dir, root, Layout::Root, &mut found);
    let mut in_repos = Vec::new();
    if let Some(folder) = repos_folder(&dir) {
        match read_names(&folder) {
            Ok(names) => in_repos = walk(plane, ws, &folder, names, Layout::Repos, &mut found),
            Err(why) => found.unreadable.push(Said {
                name: REPOS.to_string(),
                layout: Layout::Root,
                why: why.to_string(),
            }),
        }
    }
    let real = |row: &Row| row.git == Git::Directory && name_ok(&row.repo.name);
    let wins: BTreeSet<String> = in_repos
        .iter()
        .filter(|row| real(row))
        .map(|row| row.repo.name.clone())
        .collect();
    for row in at_root {
        if wins.contains(&row.repo.name) {
            found.shadowed.push(row);
        } else if real(&row) {
            found.repos.push(row);
        } else {
            found.others.push(row);
        }
    }
    for row in in_repos {
        if real(&row) {
            found.repos.push(row);
        } else {
            found.others.push(row);
        }
    }
    found.repos.sort_by(|a, b| a.repo.name.cmp(&b.repo.name));
    found.shadowed.sort_by(|a, b| a.repo.name.cmp(&b.repo.name));
    found.others.sort_by(|a, b| a.repo.name.cmp(&b.repo.name));
    found.refused.sort_by(|a, b| a.name.cmp(&b.name));
    found.unreadable.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found)
}

/// **The repos workspace `ws` holds**, in both places, sorted by name: [`Holdings::repos`].
pub fn repos(plane: &Path, ws: &str) -> Result<Vec<Row>, Trouble> {
    Ok(holdings(plane, ws)?.repos)
}

/// **The folder with a `.git` in `plane` that `path` is inside**, in either layout, or `None`.
///
/// Any `.git`, as [`Located::git`] says: a caller who wants a repository asks for
/// [`Git::Directory`]. At the root the name is one [`contain::repo_name_ok`] takes, so an old clone
/// in a kept name is answered; in `repos/` it is one [`name_ok`] takes.
///
/// Both sides resolved, as [`crate::active::workspace_of_tree`] has them, so a path that leaves
/// through a link is where the link goes. A path not there yet is judged by what is there above it,
/// and is in no repo if it walks up (`..`) or goes through a link that leads nowhere. `None` for a
/// path in no repo: a workspace's own folders, `repos/` itself, and a branch folder
/// (`.worktrees/<repo>/<piece>`, which is in neither layout and which [`crate::worktree::locate`]
/// answers).
///
/// A shadowed folder ([`Holdings::shadowed`]) is answered where it stands, at [`Layout::Root`], so
/// a caller tells it from the repo of that name by [`folder`]. A nested repository is inside the
/// repo that holds it.
pub fn locate(plane: &Path, path: &Path) -> Option<Located> {
    let workspaces = plane.join("workspaces");
    let base = std::fs::canonicalize(&workspaces).ok()?;
    let here = resolved(path)?;
    let mut parts = here.strip_prefix(&base).ok()?.components();
    let ws = parts.next()?.as_os_str().to_str()?;
    let first = parts.next()?.as_os_str().to_str()?;
    let dir = confine::workspace_dir(plane, ws).ok()?;
    let (folder, name, layout) = match repos_folder(&dir).filter(|repos| spelled(repos, first)) {
        Some(repos) => {
            let name = parts.next()?.as_os_str().to_str()?;
            (repos, name, Layout::Repos)
        }
        None => (dir, first, Layout::Root),
    };
    let named = match layout {
        Layout::Repos => name_ok(name),
        Layout::Root => contain::repo_name_ok(name),
    };
    if !named {
        return None;
    }
    let at = confine::within_workspace(plane, ws, &folder.join(name)).ok()?;
    let git = git_of(&at).ok()??;
    Some(Located {
        plane: plane.to_path_buf(),
        ws: ws.to_string(),
        repo: Repo {
            name: name.to_string(),
            path: at,
        },
        layout,
        git,
    })
}

/// **[`locate`] with no plane given**, for finding the plane from inside a clone: the nearest plane
/// above `path`, a path that is there, with a `.git` holding it. Nearest with one, not nearest: a
/// repo's tree may hold a plane of its own, as a fixture, and a path in that fixture is still in
/// the repo.
///
/// **`None` in a branch folder**, as [`locate`] is. A caller there, with its plane in hand, asks
/// [`crate::worktree::locate`] for the workspace, repo and piece, and [`folder`] for the clone.
pub fn locate_from(path: &Path) -> Option<Located> {
    let here = std::fs::canonicalize(path).ok()?;
    here.ancestors()
        .filter(|above| above.file_name().is_some_and(|name| name == "workspaces"))
        .filter_map(Path::parent)
        .filter(|plane| crate::names::has_manifest(plane))
        .find_map(|plane| locate(plane, &here))
}

/// **What sits where `path`'s `.git` would be**, without following a link. `None` is nothing there,
/// or a `path` that is no folder.
pub fn git_of(path: &Path) -> std::io::Result<Option<Git>> {
    match std::fs::symlink_metadata(path.join(".git")) {
        Ok(found) if found.file_type().is_symlink() => Ok(Some(Git::Link)),
        Ok(found) if found.is_dir() => Ok(Some(Git::Directory)),
        Ok(_) => Ok(Some(Git::File)),
        Err(why)
            if matches!(
                why.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(None)
        }
        Err(why) => Err(why),
    }
}

/// **Whether `name` is one a repo is listed under or placed in**: one [`contain::repo_name_ok`]
/// takes, and not one the workspace keeps for its own, `repos` apart. What decides [`repos`],
/// [`new_folder`] and anything in `repos/`. An old clone at the root may hold a kept name still,
/// which [`folder`] and [`locate`] answer.
pub fn name_ok(name: &str) -> bool {
    contain::repo_name_ok(name) && !kept(name, contain::workspace_keeps)
}

/// **Where a new repo named `repo` goes in workspace `ws`**: `repos/<repo>`, whatever the
/// workspace already holds. Whether something is already there is the caller's to say.
///
/// Refused while an old repo named `repos` stands where the folder would be, and while `repos`
/// is anything but a folder: a link would take the clone out of the workspace. Refused for the
/// name `repos` too, in any case: that is the folder's.
///
/// **An answer, not a reservation.** This creates nothing, since no `repos/` is made until a clone
/// needs one, so a link can still appear at `repos` after it answers. The caller creates `repos/`
/// without following a link (`mkdir` alone, never `create_dir_all` on a path it did not make) and
/// holds the result to [`confine::within_workspace`] before it writes.
pub fn new_folder(plane: &Path, ws: &str, repo: &str) -> Result<PathBuf, Refused> {
    if !name_ok(repo) || repo.eq_ignore_ascii_case(REPOS) {
        return Err(Refused::NotARepoName(repo.to_string()));
    }
    let dir = confine::workspace_dir(plane, ws).map_err(Trouble::from)?;
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

/// The workspace's folder, checked, or `None` where it has none. A link or a file there is refused.
fn anchored(plane: &Path, ws: &str) -> Result<Option<PathBuf>, Trouble> {
    match confine::workspace_dir(plane, ws) {
        Ok(dir) => Ok(Some(dir)),
        Err(Outside::NoWorkspace { .. })
            if std::fs::symlink_metadata(plane.join("workspaces").join(ws))
                .is_err_and(|why| why.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(None)
        }
        Err(why) => Err(why.into()),
    }
}

/// **`path`, resolved**: as it stands, or for a path not there yet, what is there above it with the
/// rest added. `None` for one that walks up, whose lexical shape says nothing of where it goes, and
/// for one through a link that leads nowhere.
fn resolved(path: &Path) -> Option<PathBuf> {
    if let Ok(here) = std::fs::canonicalize(path) {
        return Some(here);
    }
    if path
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return None;
    }
    for above in path.ancestors() {
        if let Ok(real) = std::fs::canonicalize(above) {
            return Some(real.join(path.strip_prefix(above).ok()?));
        }
        // There, and it would not resolve.
        if std::fs::symlink_metadata(above).is_ok() {
            return None;
        }
    }
    None
}

/// Whether a resolved path's entry `first` is the folder `repos`: on a disk that folds case, `repos`
/// answers for a folder spelled `Repos`, and a resolved path spells it as it stands.
fn spelled(repos: &Path, first: &str) -> bool {
    std::fs::canonicalize(repos).is_ok_and(|real| real.file_name().is_some_and(|n| n == first))
}

/// `dir/repos` when it is the folder new repos go in: a folder, not a link, with no `.git` in it.
fn repos_folder(dir: &Path) -> Option<PathBuf> {
    let repos = dir.join(REPOS);
    let meta = std::fs::symlink_metadata(&repos).ok()?;
    let plain = meta.is_dir() && matches!(git_of(&repos), Ok(None));
    plain.then_some(repos)
}

/// The entries of `dir`: their names as far as they read as text, or what stopped one being read.
fn read_names(dir: &Path) -> std::io::Result<Vec<std::io::Result<String>>> {
    Ok(std::fs::read_dir(dir)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect())
}

/// One place's entries, as rows and as what is said of the rest.
fn walk(
    plane: &Path,
    ws: &str,
    folder: &Path,
    names: Vec<std::io::Result<String>>,
    layout: Layout,
    found: &mut Holdings,
) -> Vec<Row> {
    let mut rows = Vec::new();
    let said = |name: &str, why: String| Said {
        name: name.to_string(),
        layout,
        why,
    };
    for name in names {
        let name = match name {
            Ok(name) => name,
            Err(why) => {
                found.unreadable.push(said("", why.to_string()));
                continue;
            }
        };
        if name.starts_with('.') {
            continue;
        }
        if !contain::repo_name_ok(&name) {
            let why = format!("'{name}' is not a name purlis will take for a repo");
            found.refused.push(said(&name, why));
            continue;
        }
        // The path git would be pointed at, gated as **itself** and never as its parent: a
        // link AT the entry redirects the read while the folder above it is perfectly ordinary.
        let path = match confine::within_workspace(plane, ws, &folder.join(&name)) {
            Ok(checked) => checked,
            Err(why) => {
                found.refused.push(said(&name, why.to_string()));
                continue;
            }
        };
        match git_of(&path) {
            Ok(Some(git)) => rows.push(Row {
                repo: Repo { name, path },
                layout,
                git,
            }),
            Ok(None) => {}
            Err(why) => found.unreadable.push(said(&name, why.to_string())),
        }
    }
    rows
}

/// Whether `keeps` holds `name` for the workspace's own. `repos` is an old repo's name, read as
/// one in any case, even once it is a name the workspace keeps (#5373).
fn kept(name: &str, keeps: impl Fn(&str) -> bool) -> bool {
    keeps(name) && !name.eq_ignore_ascii_case(REPOS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repos_is_an_old_repos_name_however_it_is_cased_even_once_the_workspace_keeps_it() {
        // #5373 adds `repos` to the names a workspace keeps, which are matched in any case. Asked
        // of that rule here, because the list does not hold `repos` yet.
        let keeps = |name: &str| {
            ["memory", "repos"]
                .iter()
                .any(|own| own.eq_ignore_ascii_case(name))
        };

        assert!(!kept("repos", keeps));
        assert!(
            !kept("Repos", keeps),
            "an old clone named Repos stays a repo"
        );
        assert!(!kept("REPOS", keeps));
        assert!(kept("memory", keeps));
        assert!(kept("Memory", keeps));
    }
}
