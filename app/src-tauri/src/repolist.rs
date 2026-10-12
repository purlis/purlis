// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Your repos, beside Get a project's URL box** (#1818): the window's door to
//! `purlis_core::repolist`.
//!
//! One command, the window's alone (`WINDOW_ONLY`): it lists, as the person, what each forge
//! account they signed in to in purlis reaches, and says per host what to do without one. It
//! reads repo names, never a token. A pick is cloned through Get a project's own clone (#1815),
//! and opened through the trust question every open goes through.

use purlis_core::repolist::{self, Listed, Listing, Manifest, Reached};

/// One repo the person can get.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ListedRepo {
    /// `github` or `gitlab`.
    pub kind: String,
    pub host: String,
    /// The account that reaches it.
    pub login: String,
    /// `owner/name`; a GitLab group's whole path.
    pub path: String,
    pub owner: String,
    pub name: String,
    pub description: String,
    /// What Get a project clones.
    pub url: String,
    /// Whether it holds a purlis project's manifest.
    pub project: bool,
}

/// One account the list asked as, or a host with no account.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RepoReach {
    /// `github` or `gitlab`.
    pub kind: String,
    pub host: String,
    /// The account, or null for a host with no account.
    pub login: Option<String>,
    /// How many repos it listed.
    pub listed: u32,
    /// What to say instead: why it did not list, or what to do with no account.
    pub trouble: Option<String>,
}

/// The repos, projects first, and what each host said.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RepoListing {
    pub repos: Vec<ListedRepo>,
    pub reaches: Vec<RepoReach>,
}

impl From<Listed> for ListedRepo {
    fn from(repo: Listed) -> Self {
        ListedRepo {
            kind: repo.kind.word().to_string(),
            project: repo.manifest == Manifest::Found,
            host: repo.host,
            login: repo.login,
            path: repo.path,
            owner: repo.owner,
            name: repo.name,
            description: repo.description,
            url: repo.url,
        }
    }
}

impl From<Reached> for RepoReach {
    fn from(reach: Reached) -> Self {
        RepoReach {
            kind: reach.kind.word().to_string(),
            host: reach.host,
            login: reach.login,
            listed: u32::try_from(reach.listed).unwrap_or(u32::MAX),
            trouble: reach.trouble,
        }
    }
}

impl From<Listing> for RepoListing {
    fn from(listing: Listing) -> Self {
        RepoListing {
            repos: listing.repos.into_iter().map(ListedRepo::from).collect(),
            reaches: listing.reaches.into_iter().map(RepoReach::from).collect(),
        }
    }
}

/// The repos the person's signed-in forge accounts reach, purlis projects first.
#[tauri::command]
#[specta::specta]
pub async fn forge_repo_list() -> Result<RepoListing, String> {
    tauri::async_runtime::spawn_blocking(|| RepoListing::from(repolist::list()))
        .await
        .map_err(|err| format!("purlis could not list your repos: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use purlis_core::forge::Kind;

    #[test]
    fn a_row_says_whether_it_is_a_project_and_a_host_without_an_account_names_none() {
        let listing = RepoListing::from(Listing {
            repos: vec![Listed {
                kind: Kind::GitLab,
                host: "gitlab.com".into(),
                login: "jdoe".into(),
                path: "grp/sub/handbook".into(),
                owner: "grp/sub".into(),
                name: "handbook".into(),
                description: String::new(),
                url: "https://gitlab.com/grp/sub/handbook".into(),
                manifest: Manifest::Found,
            }],
            reaches: vec![Reached {
                kind: Kind::GitHub,
                host: "github.com".into(),
                login: None,
                listed: 0,
                trouble: Some("Not signed in to github.com.".into()),
            }],
        });
        assert_eq!(listing.repos[0].kind, "gitlab");
        assert!(listing.repos[0].project);
        assert_eq!(listing.reaches[0].kind, "github");
        assert_eq!(listing.reaches[0].login, None);
    }
}
