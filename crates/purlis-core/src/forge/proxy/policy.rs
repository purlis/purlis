// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The per-chat policy: one request in, allowed or refused out, with the reason.

use super::git::{self, GitOp, PushError};
use super::graphql;
use super::repo::{AllowedRepo, Kind, normalise_repo_path};
use super::request::{self, RequestHead};
use super::rest;

/// Why a request was refused. The words are stable: they are what the audit records and what a
/// chat is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The request carried no credential, so no sentinel.
    NoSentinel,
    /// A credential header held something that is not this chat's sentinel.
    WrongSentinel,
    /// The endpoint's chat has no policy or sentinel (it ended, or never started).
    UnknownChat,
    /// A host, port, target or header the proxy would not read one way.
    MalformedRequest,
    /// A host that is none of the chat's repos' hosts, to the letter.
    HostNotAllowed,
    /// A repo that is not one of the chat's, to the letter.
    OtherRepo,
    /// Merging, in any form: REST, GraphQL, auto-merge, merge when the pipeline succeeds.
    Merge,
    /// Approving or reviewing in a way that counts toward a merge.
    Approval,
    /// A release, or a tag.
    ReleaseOrTag,
    /// A ruleset, branch protection or any other rule over the repo.
    Ruleset,
    /// The repo's administration: settings, hooks, keys, secrets, collaborators.
    Admin,
    /// A push to the default branch or a protected one, or to a ref that is not a branch.
    ProtectedRef,
    /// A push whose ref commands were not read, or a request body that is too big or missing.
    BodyUnreadable,
    /// A GraphQL document that does not parse as one request.
    GraphqlParse,
    /// A GraphQL mutation that is not on the list.
    GraphqlUnknownMutation,
    /// A GraphQL document that reaches a repo, or an object, that is not the chat's.
    GraphqlOtherRepo,
    /// A route that is not on the list of what a chat may write, or a request of a kind the proxy
    /// does not carry.
    NotOnList,
    /// The request was allowed, and the person's credential could not be had.
    NoCredential,
}

/// What a request was, for the audit: a class, never a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// The ref advertisement of a fetch or a push.
    GitRefs,
    GitFetch,
    GitPush,
    ApiRead,
    PullRequestWrite,
    CommentWrite,
    IssueWrite,
    LabelWrite,
    GraphqlQuery,
    /// The mutation's name, which is on the list.
    GraphqlMutation(String),
}

/// Which of a repo's two doors a request came through: it decides the form of the credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Surface {
    Git,
    Api,
}

/// A request that may be answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allowed {
    pub kind: Kind,
    /// The repo's host, which the credential is looked up by.
    pub host: String,
    /// The allowed repo the request is for: `owner/repo`.
    pub repo: String,
    pub class: Class,
    pub(crate) surface: Surface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow(Allowed),
    Refuse(Refusal),
}

/// What of the body the policy needs to see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyNeed {
    /// None: the request line decides.
    Nothing,
    /// The pkt-lines of a push's commands, to the first flush packet (and the pack may follow).
    PushCommands,
    /// A GraphQL request's JSON, whole, up to 1 MiB.
    Json,
}

/// Which allowed repo, if any, a GraphQL object id belongs to. The forge alone knows; the proxy
/// asks. Until a resolver is wired in ([`NoObjects`]), a mutation on an object (not a new one,
/// which names its repo) is refused.
pub trait ObjectRepo: Send + Sync {
    /// The normalised `owner/repo` path of the repo `node_id` is in, on `host`.
    fn repo_of(&self, kind: Kind, host: &str, node_id: &str) -> Option<String>;
}

/// The resolver that knows nothing, so every mutation on an object is refused.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoObjects;

impl ObjectRepo for NoObjects {
    fn repo_of(&self, _: Kind, _: &str, _: &str) -> Option<String> {
        None
    }
}

/// A chat's repos, and the rule that decides every request against them.
#[derive(Debug, Clone)]
pub struct ChatPolicy {
    repos: Vec<AllowedRepo>,
}

/// What of the body a request needs, from its line alone.
pub(crate) fn body_need(head: &RequestHead) -> BodyNeed {
    let path = head.target.split(['?', '#']).next().unwrap_or_default();
    let last = path.rsplit('/').next().unwrap_or_default();
    if !head.method.eq_ignore_ascii_case("POST") {
        BodyNeed::Nothing
    } else if last == "git-receive-pack" {
        BodyNeed::PushCommands
    } else if last == "graphql" {
        BodyNeed::Json
    } else {
        BodyNeed::Nothing
    }
}

impl ChatPolicy {
    pub fn new(repos: Vec<AllowedRepo>) -> Self {
        Self { repos }
    }

    /// Decide one request. `body` is what [`body_need`] asked for: nothing for most requests.
    pub fn decide(&self, head: &RequestHead, body: &[u8], objects: &dyn ObjectRepo) -> Decision {
        match self.try_decide(head, body, objects) {
            Ok(allowed) => Decision::Allow(allowed),
            Err(refusal) => Decision::Refuse(refusal),
        }
    }

    fn try_decide(
        &self,
        head: &RequestHead,
        body: &[u8],
        objects: &dyn ObjectRepo,
    ) -> Result<Allowed, Refusal> {
        let method = request::method(head).ok_or(Refusal::NotOnList)?;
        let host = request::host(head)?;
        let target = request::parse_target(&head.target)?;
        let candidates: Vec<&AllowedRepo> = self
            .repos
            .iter()
            .filter(|r| r.serves(&host, head.port))
            .collect();
        let Some(first) = candidates.first() else {
            return Err(Refusal::HostNotAllowed);
        };
        let kind = first.kind;
        let candidates: Vec<&AllowedRepo> =
            candidates.into_iter().filter(|r| r.kind == kind).collect();
        let segments = &target.segments;
        let on_api_host = host == candidates[0].api_host() && host != candidates[0].host;

        // GraphQL: one URL, decided by the document.
        let graphql_path = match kind {
            Kind::GitHub if on_api_host => *segments == ["graphql"],
            Kind::GitHub => segments.len() == 2 && segments[0] == "api" && segments[1] == "graphql",
            Kind::GitLab => segments.len() == 2 && segments[0] == "api" && segments[1] == "graphql",
        };
        if graphql_path {
            if method != "POST" {
                return Err(Refusal::NotOnList);
            }
            let class = graphql::decide(kind, &candidates, body, objects)?;
            // The repo a GraphQL request is for is the document's to name, and every id in it was
            // tied to one of these: report the first for the audit.
            return Ok(allowed(candidates[0], class, Surface::Api));
        }

        // REST.
        if let Some((repo_path, rel)) = rest_path(kind, on_api_host, &host, segments)? {
            let repo = candidates
                .iter()
                .find(|r| repo_matches(r, &repo_path))
                .ok_or(Refusal::OtherRepo)?;
            // An encoded `/` stays inside its segment. GitLab names a file `docs%2Fkeys.md`; no
            // GitHub route has one, so there it is not a path as the forge would read it.
            if kind == Kind::GitHub && rel.iter().any(|s| s.contains('/')) {
                return Err(Refusal::MalformedRequest);
            }
            let class = rest::decide(kind, method, &rel, &target)?;
            return Ok(allowed(repo, class, Surface::Api));
        }

        // Git over HTTP.
        if on_api_host {
            return Err(Refusal::NotOnList);
        }
        let Some((repo_segments, op)) = git::classify(method, segments, &target) else {
            return Err(Refusal::NotOnList);
        };
        let path =
            normalise_repo_path(kind, &repo_segments.join("/")).map_err(|_| Refusal::OtherRepo)?;
        let repo = candidates
            .iter()
            .find(|r| r.path == path)
            .ok_or(Refusal::OtherRepo)?;
        let class = match op {
            GitOp::Advertise => Class::GitRefs,
            GitOp::UploadPack => Class::GitFetch,
            GitOp::ReceivePack => {
                check_push(repo, body)?;
                Class::GitPush
            }
        };
        Ok(allowed(repo, class, Surface::Git))
    }
}

fn allowed(repo: &AllowedRepo, class: Class, surface: Surface) -> Allowed {
    Allowed {
        kind: repo.kind,
        host: repo.host.clone(),
        repo: repo.path.clone(),
        class,
        surface,
    }
}

/// Whether `repo` is the project a REST path names. GitLab's may be its numeric id.
fn repo_matches(repo: &AllowedRepo, named: &RestRepo) -> bool {
    match named {
        RestRepo::Path(path) => repo.path == *path,
        RestRepo::Id(id) => repo.project_id == Some(*id),
    }
}

enum RestRepo {
    Path(String),
    Id(u64),
}

/// The repo a REST path names and what follows it, if `segments` is a REST path of this forge.
/// `Ok(None)` is: not a REST path, so it may be a git one.
#[allow(clippy::type_complexity)]
fn rest_path(
    kind: Kind,
    on_api_host: bool,
    host: &str,
    segments: &[String],
) -> Result<Option<(RestRepo, Vec<String>)>, Refusal> {
    let strs: Vec<&str> = segments.iter().map(String::as_str).collect();
    match (kind, strs.as_slice()) {
        // api.github.com/repos/{owner}/{repo}/…
        (Kind::GitHub, ["repos", owner, repo, ..]) if on_api_host => {
            let path = github_path(owner, repo)?;
            Ok(Some((RestRepo::Path(path), segments[3..].to_vec())))
        }
        (Kind::GitHub, _) if on_api_host => Err(Refusal::NotOnList),
        // GHES: {host}/api/v3/repos/{owner}/{repo}/…
        (Kind::GitHub, ["api", "v3", "repos", owner, repo, ..]) if host != "github.com" => {
            let path = github_path(owner, repo)?;
            Ok(Some((RestRepo::Path(path), segments[5..].to_vec())))
        }
        (Kind::GitHub, ["api", "v3", ..]) if host != "github.com" => Err(Refusal::NotOnList),
        // GitLab: {host}/api/v4/projects/{id}/…
        (Kind::GitLab, ["api", "v4", "projects", id, ..]) => {
            let rest = segments[4..].to_vec();
            if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
                return id
                    .parse::<u64>()
                    .map(|n| Some((RestRepo::Id(n), rest)))
                    .map_err(|_| Refusal::OtherRepo);
            }
            // `.git` is not part of an API project path: `proj.git` is another project.
            if id.to_ascii_lowercase().ends_with(".git") {
                return Err(Refusal::OtherRepo);
            }
            let path = normalise_repo_path(Kind::GitLab, id).map_err(|_| Refusal::OtherRepo)?;
            Ok(Some((RestRepo::Path(path), rest)))
        }
        (Kind::GitLab, ["api", ..]) => Err(Refusal::NotOnList),
        _ => Ok(None),
    }
}

fn github_path(owner: &str, repo: &str) -> Result<String, Refusal> {
    // `.git` is not part of an API repo name: `repo.git` is another repo.
    if repo.to_ascii_lowercase().ends_with(".git") || owner.contains('/') || repo.contains('/') {
        return Err(Refusal::OtherRepo);
    }
    normalise_repo_path(Kind::GitHub, &format!("{owner}/{repo}")).map_err(|_| Refusal::OtherRepo)
}

/// A push's commands against the repo: every ref a branch, none the default or a protected one.
fn check_push(repo: &AllowedRepo, body: &[u8]) -> Result<(), Refusal> {
    let refs = git::pushed_refs(body).map_err(|e| match e {
        PushError::Unreadable | PushError::PushCert => Refusal::BodyUnreadable,
    })?;
    for name in &refs {
        if name.starts_with("refs/tags/") {
            return Err(Refusal::ReleaseOrTag);
        }
        let Some(branch) = name.strip_prefix("refs/heads/") else {
            return Err(Refusal::ProtectedRef);
        };
        if branch.is_empty() || repo.protects(branch) {
            return Err(Refusal::ProtectedRef);
        }
    }
    Ok(())
}
