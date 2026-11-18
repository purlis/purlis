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
//!   (#6367) has no remote purlis reads, so its numbers are no link. The remotes are read by
//!   gitoxide from the repository's own config, isolated from the system's and the person's, and
//!   no git runs; a `.git` file, or its git folder's `commondir`, naming a folder outside the
//!   project is never followed ([`git_dir_of`]).

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

/// **The repo and number `word` names**, when it is `owner/repo#N` ([`said`] with a repo): what
/// a marked link's words say when they read as an issue ([`super::osc8::misleading`]).
pub(super) fn names_an_issue(word: &str) -> Option<(&str, u64)> {
    let said = said(word)?;
    Some((said.repo?, said.number))
}

/// Whether `word` has the shape of `owner/repo#N` but for a letter outside ASCII in its repo's
/// words (each part of letters and digits of any script, `.`, `_` and `-`): such words may only
/// look like a repo a forge names, so a marked link's words that read so name a target.
pub(super) fn reads_as_an_issue_beyond_ascii(word: &str) -> bool {
    let Some((repo, number)) = word.rsplit_once('#') else {
        return false;
    };
    let parts: Vec<&str> = repo.split('/').collect();
    !repo.is_ascii()
        && !number.is_empty()
        && number.len() <= MOST_DIGITS
        && !number.starts_with('0')
        && number.bytes().all(|b| b.is_ascii_digit())
        && parts.len() >= 2
        && parts.iter().all(|part| {
            part.chars().next().is_some_and(char::is_alphanumeric)
                && part
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
        })
}

/// `url`'s host, with its port where it has one, as a forge's host is kept.
fn host_of(url: &url::Url) -> Option<String> {
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

/// Whether `url` is the page of issue or pull request `number` of `repo`, on a forge the project
/// knows, as GitHub's `/issues/N` and `/pull/N` and GitLab's `/-/issues/N` are.
pub(super) fn is_page_of(url: &url::Url, repo: &str, number: u64, project: Option<&Path>) -> bool {
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

/// **The remotes of the repository `folder` is in**, read by gitoxide from its config alone
/// (isolated: not the system's or the person's), as `(name, fetch url)`. None where no repository
/// holds it in `project`, or its config cannot be read.
fn read_remotes(folder: &Path, project: &Path) -> Vec<(String, String)> {
    let Some(git) = git_dir_of(folder, project) else {
        return Vec::new();
    };
    // The path is a git folder as it stands (`open_path_as_is`): gitoxide looks for no `.git`
    // inside it, which a link or a `gitdir:` file there could point anywhere.
    let Ok(repo) = gix::open_opts(&git, gix::open::Options::isolated().open_path_as_is(true))
    else {
        return Vec::new();
    };
    repo.remote_names()
        .into_iter()
        .filter_map(|name| {
            let remote = repo.find_remote(&**name).ok()?;
            let url = remote.url(gix::remote::Direction::Fetch)?;
            Some((name.to_string(), url.to_bstring().to_string()))
        })
        .collect()
}

/// **The git folder of the repository `folder` (resolved) is in**, found as git finds it, but
/// never outside `project` (resolved): the nearest folder from `folder` up to `project` holding a
/// `.git`, whose git folder, read from a `.git` file's `gitdir:` where it is one, and that git
/// folder's `commondir`, are both in `project`. Each is resolved as the system resolves it, links
/// before `..`, and the resolved path is the one returned and read, so what was checked is what
/// is opened, as it stands: it must hold a `HEAD`, and gitoxide looks for no `.git` in it. A
/// `.git` file anything wrote can name any folder, a guarded one too, and nothing outside the
/// project is opened. `None` otherwise.
fn git_dir_of(folder: &Path, project: &Path) -> Option<PathBuf> {
    let work = folder
        .ancestors()
        .take_while(|at| at.starts_with(project))
        .find(|at| std::fs::symlink_metadata(at.join(".git")).is_ok())?;
    let dot_git = work.join(".git");
    let git = if std::fs::symlink_metadata(&dot_git).ok()?.is_dir() {
        within(&dot_git, project)?
    } else {
        let said = std::fs::read_to_string(&dot_git).ok()?;
        let named = said.lines().next()?.strip_prefix("gitdir:")?.trim();
        within(&work.join(named), project)?
    };
    // What is opened is a git folder itself: a folder with no `HEAD` is none.
    if !std::fs::metadata(git.join("HEAD")).is_ok_and(|meta| meta.is_file()) {
        return None;
    }
    if let Ok(said) = std::fs::read_to_string(git.join("commondir")) {
        within(&git.join(said.trim()), project)?;
    }
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
