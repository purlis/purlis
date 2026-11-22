// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's repo as the proxy knows it: the address the person confirmed, normalised once, and
//! what the proxy needs to tell a push to a protected ref.

use std::fmt;

/// Which forge's rules a host follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// github.com or GitHub Enterprise Server.
    GitHub,
    /// gitlab.com or a self-hosted GitLab.
    GitLab,
}

/// An address that is not one a chat's repo can have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressError(String);

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AddressError {}

/// One repo a chat may reach: the exact host, port and normalised path of the clone the person
/// confirmed. Matching is exact on all three, never by prefix (`group/proj-x` is not
/// `group/proj`), and the path is folded to lower case, which both forges do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedRepo {
    pub(crate) kind: Kind,
    pub(crate) host: String,
    pub(crate) port: u16,
    /// `owner/repo` on GitHub, `group/subgroup/project` on GitLab. Lower case, no `.git`.
    pub(crate) path: String,
    pub(crate) default_branch: String,
    pub(crate) protected_branches: Vec<String>,
    /// The repo's GraphQL node ids (GitHub's `repositoryId`), as the forge gave them.
    pub(crate) node_ids: Vec<String>,
    /// The forge's numeric id of the repo (GitHub's `id`, GitLab's project id): what a credential
    /// source checks the person's confirmation against, and which a GitLab REST path may use
    /// instead of the path.
    pub(crate) project_id: Option<u64>,
    /// Branches the chat may move and delete from the start: the ones purlis made for it.
    pub(crate) own_branches: Vec<String>,
}

impl AllowedRepo {
    /// The repo at `host:port` with `path`, whose default branch is `default_branch`. A push to the
    /// default branch is always refused, so it is required, not optional: a repo whose default
    /// branch is not known cannot be pushed to.
    pub fn new(
        kind: Kind,
        host: &str,
        port: u16,
        path: &str,
        default_branch: &str,
    ) -> Result<Self, AddressError> {
        let host = normalise_host(host)?;
        let path = normalise_repo_path(kind, path)?;
        if default_branch.is_empty() || default_branch.bytes().any(|b| b.is_ascii_control()) {
            return Err(AddressError(
                "a repo needs its default branch's name".into(),
            ));
        }
        Ok(Self {
            kind,
            host,
            port,
            path,
            default_branch: default_branch.to_string(),
            protected_branches: Vec::new(),
            node_ids: Vec::new(),
            project_id: None,
            own_branches: Vec::new(),
        })
    }

    /// Branches a push may never touch, besides the default one.
    #[must_use]
    pub fn with_protected_branches(mut self, names: &[&str]) -> Self {
        self.protected_branches = names.iter().map(|n| (*n).to_string()).collect();
        self
    }

    /// The repo's GraphQL node id, so a GraphQL mutation naming it is known to be this repo's.
    #[must_use]
    pub fn with_node_id(mut self, id: &str) -> Self {
        self.node_ids.push(id.to_string());
        self
    }

    /// The forge-read numeric id (GitHub's repo `id`, GitLab's project id), as only the forge's
    /// own read can say it ([`ForgeRepoId`]): a bare number is never taken for it.
    #[must_use]
    pub fn with_forge_id(mut self, id: ForgeRepoId) -> Self {
        self.project_id = Some(id.0);
        self
    }

    /// Branches the chat owns from the start (the one purlis cut for its task). A branch the chat
    /// creates later is recorded as it is pushed; any other it may not move or delete.
    #[must_use]
    pub fn with_own_branches(mut self, names: &[&str]) -> Self {
        self.own_branches = names.iter().map(|n| (*n).to_string()).collect();
        self
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The forge's numeric id of the repo, where it was recorded: what a credential request
    /// carries for it.
    pub fn forge_id(&self) -> Option<u64> {
        self.project_id
    }

    /// The host this repo's API answers on: `api.github.com` for github.com, the same host
    /// elsewhere (GHES serves `/api/v3`, GitLab `/api/v4`).
    pub(crate) fn api_host(&self) -> &str {
        if self.kind == Kind::GitHub && self.host == "github.com" {
            "api.github.com"
        } else {
            &self.host
        }
    }

    /// Whether a request to `host:port` can be for this repo, by git or by the API.
    pub(crate) fn serves(&self, host: &str, port: u16) -> bool {
        self.port == port && (host == self.host || host == self.api_host())
    }

    /// Whether `branch` (a short name) may not be pushed to: the default, a protected one, a pages
    /// branch, and the names a deploy or a release is triggered from (ADR 0128 D3h.8).
    pub(crate) fn protects(&self, branch: &str) -> bool {
        let lower = branch.to_ascii_lowercase();
        const NAMES: &[&str] = &[
            "main",
            "master",
            "prod",
            "production",
            "gh-pages",
            "release",
            "deploy",
        ];
        const PREFIXES: &[&str] = &["release/", "release-", "deploy/", "prod/", "production/"];
        lower == self.default_branch.to_ascii_lowercase()
            || NAMES.contains(&lower.as_str())
            || PREFIXES.iter().any(|p| lower.starts_with(p))
            || self
                .protected_branches
                .iter()
                .any(|p| lower == p.to_ascii_lowercase())
    }
}

/// **A repo's numeric id as the forge said it** when the person pressed (ADR 0128 D4): what a
/// credential is looked up by, never a number a chat, a config file or a caller of the proxy
/// supplies. Nothing outside this crate builds one from a bare number; the only source is a
/// [`confirmed::ConfirmedRepo`](crate::forge::confirmed::ConfirmedRepo) the person's press
/// recorded from the forge's read ([`ConfirmedRepo::forge_id`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForgeRepoId(u64);

impl ForgeRepoId {
    pub(crate) fn from_forge_read(id: u64) -> Self {
        Self(id)
    }

    /// An id for a test's repo, where no forge was read. Only a test build has it.
    #[cfg(feature = "fenced")]
    #[doc(hidden)]
    pub fn for_a_test(id: u64) -> Self {
        Self(id)
    }
}

/// A host as a request or a repo names it: letters, digits, dots and hyphens, lower case, with no
/// trailing dot, userinfo, port or bracket. Anything else is refused rather than guessed at.
pub(crate) fn normalise_host(host: &str) -> Result<String, AddressError> {
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    let ok = !host.is_empty()
        && host.len() <= 253
        && !host.starts_with(['.', '-'])
        && !host.contains("..")
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    if ok {
        Ok(host)
    } else {
        Err(AddressError("not a host name".into()))
    }
}

/// A repo's path: segments of letters, digits, `.`, `_` and `-`, none empty, `.` or `..`, joined
/// by `/`, lower case, one trailing `.git` removed. GitHub's is two segments; GitLab's two or more.
pub(crate) fn normalise_repo_path(kind: Kind, path: &str) -> Result<String, AddressError> {
    let path = path.trim_start_matches('/').to_ascii_lowercase();
    let path = path.strip_suffix(".git").unwrap_or(&path);
    let segments: Vec<&str> = path.split('/').collect();
    let count_ok = match kind {
        Kind::GitHub => segments.len() == 2,
        Kind::GitLab => segments.len() >= 2,
    };
    let segments_ok = segments.iter().all(|s| is_name_segment(s));
    if count_ok && segments_ok {
        Ok(segments.join("/"))
    } else {
        Err(AddressError("not a repo path".into()))
    }
}

fn is_name_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}
