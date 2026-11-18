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
//! - **It opens as a web address does** ([`super::WebAddress`]): the target is a `Target::Url`,
//!   built here from the forge's host and the repo's path, never from text the pane sent beyond
//!   the number and the repo's name. GitHub's `/issues/N` page shows a pull request's too; on
//!   GitLab `#N` is an issue (a merge request is `!N`).
//! - **Only folders in the chat's project are read**: a folder the chat moved to outside it
//!   (#6367) has no remote purlis reads, so its numbers are no link. The remotes are read by
//!   gitoxide from the repository's own config, isolated from the system's and the person's, and
//!   no git runs.

use std::path::Path;

use super::{Kind, LinkKind, Opening, Pane, Resolved, Target, WebAddress};
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

    /// **The forge and repo `pane`'s folder's remote names** ([`IssueRef`]'s rule): `origin` when
    /// it is on a forge the project knows, else the folder's one remote on one. `None` for a
    /// folder outside the project, or with no such remote, or with several and no `origin`.
    fn forge_of(&self, pane: &Pane) -> Option<(Forge, String)> {
        let project = std::fs::canonicalize(pane.project.as_ref()?).ok()?;
        let folder = std::fs::canonicalize(pane.starts_from()?).ok()?;
        if !folder.starts_with(&project) {
            return None;
        }
        let known = crate::forge::known(&project);
        let on_a_forge: Vec<(String, Forge, String)> = (self.remotes)(&folder, &project)
            .into_iter()
            .filter_map(|(name, url)| {
                let forge = crate::forge::resolve_in(&url, &known)?;
                let repo = crate::forge::namespace_of(&url).filter(|repo| repo_ok(repo))?;
                Some((name, forge, repo))
            })
            .collect();
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

    /// As a web address opens: in the person's browser, without asking.
    fn opening(&self, target: Target, text: &str, pane: &Pane) -> Opening {
        WebAddress.opening(target, text, pane)
    }
}

/// **The remotes of the repository `folder` is in**, read by gitoxide from its config alone
/// (isolated: not the system's or the person's), as `(name, fetch url)`. None where no repository
/// whose work tree is in `project` holds it, or its config cannot be read.
fn read_remotes(folder: &Path, project: &Path) -> Vec<(String, String)> {
    let isolated = gix::sec::trust::Mapping {
        full: gix::open::Options::isolated(),
        reduced: gix::open::Options::isolated(),
    };
    let Ok(found) = gix::ThreadSafeRepository::discover_opts(
        folder,
        gix::discover::upwards::Options::default(),
        isolated,
    ) else {
        return Vec::new();
    };
    let repo = found.to_thread_local();
    let in_project = repo
        .workdir()
        .and_then(|work| std::fs::canonicalize(work).ok())
        .is_some_and(|work| work.starts_with(project));
    if !in_project {
        return Vec::new();
    }
    repo.remote_names()
        .into_iter()
        .filter_map(|name| {
            let remote = repo.find_remote(&**name).ok()?;
            let url = remote.url(gix::remote::Direction::Fetch)?;
            Some((name.to_string(), url.to_bstring().to_string()))
        })
        .collect()
}

#[cfg(test)]
#[path = "issue_tests.rs"]
mod tests;
