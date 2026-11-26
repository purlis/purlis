// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An issue or a pull request's number in a pane** (#6223, the person's ruling of 2026-11-16):
//! `#6223`, or `owner/repo#6223`, opened on its forge in the person's browser.
//!
//! - **The forge is the chat's folder's** ([`IssueRef::forge_of`]): the git remote `origin` of the
//!   folder a relative path starts from ([`super::Pane::starts_from`]), when it is on a forge the
//!   project knows ([`crate::forge::known`]: GitHub, GitLab and the hosts `charter.toml`
//!   declares), else the folder's one remote on such a forge. With none, or more than one and no
//!   `origin` among them, the number is no link and says nothing on its hover.
//! - **A bare `#N` is that remote's repo's**; `owner/repo#N` names its repo on the same forge.
//! - **It opens as a web address does** ([`super::WebAddress`]), without asking when it names no
//!   repo or one of the folder's remotes; `owner/repo#N` for any other repo asks first. The
//!   target is a `Target::Url`, built here from the forge's host and the repo's path, never from
//!   text the pane sent beyond the number and the repo's name. GitHub's `/issues/N` page shows a pull request's too; on
//!   GitLab `#N` is an issue (a merge request is `!N`).
//! - **Words with a path's look are a file first**: `owner/repo#N`'s words before the `#`
//!   (`docs/guide.md#12`) are a file link where that file is in the folder, and an issue's
//!   otherwise (`vercel/next.js#123`), which asks unless the repo is a remote
//!   ([`super::Resolver`] decides, on the disk).
//! - **Only folders in the chat's project are read**: a folder the chat moved to outside it
//!   (#6367) has no remote purlis reads, so its numbers are no link. The remotes are read from the
//!   repository's `config` files as bytes, parsed by gitoxide's config reader with includes off:
//!   no repository is opened, no git runs, and a `config` (or a `commondir`'s) that is not a
//!   regular file in the project once resolved reads nothing. A `.git` file, or its git folder's
//!   `commondir`, naming a folder outside the project is never followed ([`git_dir_of`]).

use std::path::{Path, PathBuf};

use super::{Asking, Kind, LinkKind, Opening, Pane, Resolved, Target, WebAddress};
use crate::forge::{Forge, Kind as ForgeKind};

/// The most digits an issue's number has, as a chat's number has.
pub const MOST_DIGITS: usize = 9;

/// The longest repo path read before `#`, in bytes.
const MOST_REPO: usize = 255;

/// **A repo's remotes**, as `(name, url)`, of the repository the folder is in, or none: asked with
/// the folder and the project it is in (both resolved).
pub type Remotes = dyn Fn(&Path, &Path) -> Vec<(String, String)> + Send + Sync;

/// **An issue or pull request's number** (#6223).
pub struct IssueRef {
    remotes: Box<Remotes>,
}

/// What `text` says: the repo's path, where it names one, and the number.
#[derive(Debug, PartialEq, Eq)]
struct Said<'a> {
    repo: Option<&'a str>,
    number: u64,
}

impl IssueRef {
    /// Numbers resolved with the remotes gitoxide reads from each folder's repository.
    pub fn here() -> Self {
        Self::with(read_remotes)
    }

    /// Numbers resolved with `remotes` as each folder's repository's remotes.
    pub fn with(
        remotes: impl Fn(&Path, &Path) -> Vec<(String, String)> + Send + Sync + 'static,
    ) -> Self {
        Self {
            remotes: Box::new(remotes),
        }
    }

    /// **The remotes of `pane`'s folder that are on a forge the project knows**, as `(name, forge,
    /// repo)`. None for a folder outside the project.
    fn on_a_forge(&self, pane: &Pane) -> Vec<(String, Forge, String)> {
        let (Some(project), Some(folder)) = (pane.project.as_ref(), pane.starts_from()) else {
            return Vec::new();
        };
        let (Ok(project), Ok(folder)) = (
            std::fs::canonicalize(project),
            std::fs::canonicalize(folder),
        ) else {
            return Vec::new();
        };
        if !folder.starts_with(&project) {
            return Vec::new();
        }
        let known = crate::forge::known(&project);
        (self.remotes)(&folder, &project)
            .into_iter()
            .filter_map(|(name, url)| {
                let forge = crate::forge::resolve_in(&url, &known)?;
                let repo = crate::forge::namespace_of(&url).filter(|repo| repo_ok(repo))?;
                Some((name, forge, repo))
            })
            .collect()
    }

    /// **The forge and repo `pane`'s folder's remote names** ([`IssueRef`]'s rule): `origin` when
    /// it is on a forge the project knows, else the folder's one remote on one. `None` for a
    /// folder outside the project, or with no such remote, or with several and no `origin`.
    fn forge_of(&self, pane: &Pane) -> Option<(Forge, String)> {
        let on_a_forge = self.on_a_forge(pane);
        let chosen = match on_a_forge.iter().find(|(name, ..)| name == "origin") {
            Some(origin) => origin,
            None if on_a_forge.len() == 1 => &on_a_forge[0],
            None => return None,
        };
        Some((chosen.1.clone(), chosen.2.clone()))
    }
}

/// **What `text` says**: `#N` or `<repo>#N`, the repo two or more path parts of letters, digits,
/// `.`, `_` and `-`, each starting with a letter or a digit; the number one to [`MOST_DIGITS`]
/// digits with no leading zero.
fn said(text: &str) -> Option<Said<'_>> {
    let (repo, number) = text.rsplit_once('#')?;
    if number.is_empty()
        || number.len() > MOST_DIGITS
        || number.starts_with('0')
        || !number.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let number = number.parse().ok()?;
    if repo.is_empty() {
        return Some(Said { repo: None, number });
    }
    repo_ok(repo).then_some(Said {
        repo: Some(repo),
        number,
    })
}

/// Whether `repo` is a repo's path as a forge names it: two or more parts, each of letters,
/// digits, `.`, `_` and `-` and starting with a letter or a digit, at most [`MOST_REPO`] bytes.
fn repo_ok(repo: &str) -> bool {
    let parts: Vec<&str> = repo.split('/').collect();
    repo.len() <= MOST_REPO
        && parts.len() >= 2
        && parts.iter().all(|part| {
            part.bytes()
                .next()
                .is_some_and(|b| b.is_ascii_alphanumeric())
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
                && !part.ends_with(".git")
        })
}

/// **The page of issue `number` of `repo` on `forge`**: GitHub's `/issues/N`, which shows a pull
/// request's number as its pull request; GitLab's `/-/issues/N`.
fn page(forge: &Forge, repo: &str, number: u64) -> Option<url::Url> {
    let path = match forge.kind {
        ForgeKind::GitHub => format!("{repo}/issues/{number}"),
        ForgeKind::GitLab => format!("{repo}/-/issues/{number}"),
    };
    url::Url::parse(&format!("https://{}/{path}", forge.host)).ok()
}

impl IssueRef {
    /// Whether `repo`, on the forge `target` is on, is one of `pane`'s folder's remotes' repos
    /// (letters of any case, as a forge reads them).
    fn is_a_remote(&self, repo: &str, target: &Target, pane: &Pane) -> bool {
        let Target::Url(url) = target else {
            return false;
        };
        self.on_a_forge(pane).iter().any(|(_, forge, remote)| {
            remote.eq_ignore_ascii_case(repo)
                && host_of(url).is_some_and(|host| host.eq_ignore_ascii_case(&forge.host))
        })
    }
}

/// **The repo and number `looked` says**, when it reads as `owner/repo#N` ([`super::osc8`] hands
/// it a word as it looks: each character outside ASCII as its UTS #39 skeleton). A part of the
/// repo is letters and digits of any script, `.`, `_` and `-`; one trailing `/` (`owner/repo/#N`) and a trailing `.git` are taken off the
/// repo; the number is one to [`MOST_DIGITS`] ASCII digits with no leading zero.
pub(super) fn issue_words(looked: &str) -> Option<(&str, u64)> {
    let (repo, number) = looked.rsplit_once('#')?;
    if number.is_empty()
        || number.len() > MOST_DIGITS
        || number.starts_with('0')
        || !number.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    // A slash before the `#` is no part: `owner/repo/#N` says what `owner/repo#N` does.
    let repo = repo.strip_suffix('/').unwrap_or(repo);
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let parts: Vec<&str> = repo.split('/').collect();
    let shaped = repo.len() <= MOST_REPO
        && parts.len() >= 2
        && parts.iter().all(|part| {
            part.chars().next().is_some_and(char::is_alphanumeric)
                && part
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
        });
    shaped.then(|| number.parse().ok().map(|number| (repo, number)))?
}

/// `url`'s host, with its port where it has one, as a forge's host is kept.
fn host_of(url: &url::Url) -> Option<String> {
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

/// Whether `url` is the page, over https, of issue or pull request `number` of `repo`, on a forge
/// the project knows, as GitHub's `/issues/N` and `/pull/N` and GitLab's `/-/issues/N` are.
pub(super) fn is_page_of(url: &url::Url, repo: &str, number: u64, project: Option<&Path>) -> bool {
    if url.scheme() != "https" {
        return false;
    }
    let known = project.map_or_else(Default::default, |project| {
        crate::forge::known(&std::fs::canonicalize(project).unwrap_or_else(|_| project.to_owned()))
    });
    if !host_of(url).is_some_and(|host| known.keys().any(|one| one.eq_ignore_ascii_case(&host))) {
        return false;
    }
    let path = url.path().trim_matches('/');
    ["issues", "pull", "-/issues"]
        .iter()
        .any(|kind| path.eq_ignore_ascii_case(&format!("{repo}/{kind}/{number}")))
}

impl LinkKind for IssueRef {
    fn kind(&self) -> Kind {
        Kind::Issue
    }

    fn resolve(&self, text: &str, pane: &Pane) -> Resolved {
        let Some(said) = said(text) else {
            return Resolved::Quiet;
        };
        let Some((forge, origin)) = self.forge_of(pane) else {
            return Resolved::Quiet;
        };
        let repo = said.repo.unwrap_or(&origin);
        match page(&forge, repo, said.number) {
            Some(url) => Resolved::Link(Target::Url(url)),
            None => Resolved::Quiet,
        }
    }

    /// As a web address opens: in the person's browser, without asking, unless the number names
    /// a repo (`owner/repo#N`) that is not one of the folder's remotes: then it asks first, as a
    /// link in general words does.
    fn opening(&self, target: Target, text: &str, pane: &Pane) -> Opening {
        let opening = WebAddress.opening(target.clone(), text, pane);
        let Some(repo) = said(text).and_then(|said| said.repo) else {
            return opening;
        };
        if matches!(opening, Opening::Refused(_)) || self.is_a_remote(repo, &target, pane) {
            return opening;
        }
        Opening::Ask {
            reason: "it names a repository that is not one of this folder's remotes".into(),
            shown: text.to_owned(),
            target,
            token: None,
            about: Asking::Link,
        }
    }
}

/// **The remotes of the repository `folder` is in**, as `(name, fetch url)`, read from its config
/// files alone: no git runs and gitoxide opens no repository. The git folder is resolved by
/// [`git_dir_of`]; its `config` and its `commondir`'s are each resolved as the system resolves
/// them, must be regular files in `project` with one name each and within a size limit
/// ([`read_bounded`]), and are read here as bytes and parsed with includes off, so a config that
/// links out, or includes a file, reads nothing from outside. None where no repository holds the
/// folder in `project`. A file is resolved, then read: a file swapped for a link between the two
/// is not caught.
fn read_remotes(folder: &Path, project: &Path) -> Vec<(String, String)> {
    let Some(git) = git_dir_of(folder, project) else {
        return Vec::new();
    };
    let mut remotes: Vec<(String, String)> = Vec::new();
    for dir in config_dirs(&git, project) {
        for (name, url) in remotes_in(&dir.join("config"), project) {
            match remotes.iter_mut().find(|(known, _)| *known == name) {
                Some(one) => one.1 = url,
                None => remotes.push((name, url)),
            }
        }
    }
    remotes
}

/// The folders whose `config` holds the repository's remotes: its `commondir` (where a linked
/// worktree's git folder names one), then the git folder itself.
fn config_dirs(git: &Path, project: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(Some(common)) = commondir_of(git, project) {
        dirs.push(common);
    }
    dirs.push(git.to_path_buf());
    dirs
}

/// The most a `.git` file or a `commondir` is read to: a line names a folder.
const MOST_POINTER: u64 = 64 * 1024;
/// The most a `config` is read to.
const MOST_CONFIG: u64 = 1024 * 1024;

/// **What `path` holds**, read where it is a regular file of one name (one hard link) no longer
/// than `most` bytes. A pipe, a device, a folder, a file with another name that may be a file
/// outside the project, or an oversize file reads nothing: it is opened without waiting, and its
/// kind and size are asked of the descriptor opened, not of the path.
fn read_bounded(path: &Path, most: u64) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Opening a pipe for reading waits for a writer, unless it is asked not to.
        options.custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32);
    }
    let file = options.open(path).ok()?;
    let meta = file.metadata().ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() > 1 {
            return None;
        }
    }
    if !meta.is_file() || meta.len() > most {
        return None;
    }
    let mut bytes = Vec::new();
    // One byte more than allowed shows a file that grew since it was measured.
    file.take(most + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= most).then_some(bytes)
}

/// **The common folder `git` names**, resolved: `Some(None)` where it has no `commondir`,
/// `Some(Some(folder))` where it has one that is, and whose file is, in `project` once resolved
/// and holds a folder that is, and `None` where it has one that is not (a link out of the
/// project, a pipe, a hard link, oversize, or naming a folder outside).
fn commondir_of(git: &Path, project: &Path) -> Option<Option<PathBuf>> {
    let file = git.join("commondir");
    if std::fs::symlink_metadata(&file).is_err() {
        return Some(None);
    }
    let said = read_bounded(&within(&file, project)?, MOST_POINTER)?;
    let said = String::from_utf8(said).ok()?;
    Some(Some(within(&git.join(said.trim()), project)?))
}

/// The `(remote name, fetch url)` pairs in `config`, a regular file in `project` once resolved.
fn remotes_in(config: &Path, project: &Path) -> Vec<(String, String)> {
    let Some(real) = within(config, project) else {
        return Vec::new();
    };
    let Some(bytes) = read_bounded(&real, MOST_CONFIG) else {
        return Vec::new();
    };
    let meta = gix::config::file::Metadata::from(gix::config::Source::Local);
    let Ok(file) = gix::config::File::from_bytes_no_includes(
        &bytes,
        meta,
        gix::config::file::init::Options::default(),
    ) else {
        return Vec::new();
    };
    let mut found: Vec<(String, String)> = Vec::new();
    for section in file.sections_by_name("remote").into_iter().flatten() {
        let Some(name) = section.header().subsection_name() else {
            continue;
        };
        let Some(url) = section.values("url").into_iter().next() else {
            continue;
        };
        found.push((name.to_string(), url.to_string()));
    }
    found
}

/// **The git folder of the repository `folder` (resolved) is in**, found as git finds it, but
/// never outside `project` (resolved): the nearest folder from `folder` up to `project` holding a
/// `.git`, whose git folder, read from a `.git` file's `gitdir:` where it is one, and that git
/// folder's `commondir`, are both in `project`. Each is resolved as the system resolves it, links
/// before `..`, and the resolved path is the one returned. It must hold a `HEAD`. A `.git` file
/// anything wrote can name any folder, a guarded one too, and nothing outside the project is
/// read. `None` otherwise.
fn git_dir_of(folder: &Path, project: &Path) -> Option<PathBuf> {
    let work = folder
        .ancestors()
        .take_while(|at| at.starts_with(project))
        .find(|at| std::fs::symlink_metadata(at.join(".git")).is_ok())?;
    let dot_git = work.join(".git");
    let git = if std::fs::symlink_metadata(&dot_git).ok()?.is_dir() {
        within(&dot_git, project)?
    } else {
        let said = String::from_utf8(read_bounded(&dot_git, MOST_POINTER)?).ok()?;
        let named = said.lines().next()?.strip_prefix("gitdir:")?.trim();
        within(&work.join(named), project)?
    };
    // What is read is a git folder itself: a folder with no `HEAD` is none.
    if !std::fs::metadata(git.join("HEAD")).is_ok_and(|meta| meta.is_file()) {
        return None;
    }
    commondir_of(&git, project)?;
    Some(git)
}

/// `path` as the system resolves it (links followed before any `..`), where that is in `project`.
fn within(path: &Path, project: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path)
        .ok()
        .filter(|real| real.starts_with(project))
}

#[cfg(test)]
#[path = "issue_tests.rs"]
mod tests;
