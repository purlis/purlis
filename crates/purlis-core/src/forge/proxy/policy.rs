// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The per-chat policy: one request in, allowed or refused out, with the reason.
//!
//! **Default deny.** A request is allowed only if a list names it: the git smart-HTTP requests of
//! a recorded repo, its REST routes ([`super::rest`]), a GraphQL document the gate reads
//! ([`super::graphql`]), the few reads outside a repo in [`NON_REPO_READS`], and, with no
//! credential at all, a public repo's fetch, archive and release download. What is allowed is
//! re-emitted as the proxy's own address ([`Allowed::upstream`]), built from the recorded repo's
//! host and port and the path as the policy read it: the chat's request-target, `Host` and
//! encoding never reach the forge.

use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

use super::git::{self, GitOp, PushError};
use super::graphql;
use super::repo::{AllowedRepo, Kind, normalise_repo_path};
use super::request::{self, RequestHead, Target};
use super::rest;

/// Why a request was refused. The words are stable: they are what the audit records and what a
/// chat is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The request carried no credential, so no sentinel.
    NoSentinel,
    /// A credential header held something that is not this chat's sentinel.
    WrongSentinel,
    /// Too many wrong sentinels in a row: the chat is locked out until it starts again.
    Locked,
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
    /// Text that tells the forge or a merge bot to merge or approve: `/merge`, `bors r+`…
    MergeCommand,
    /// Approving or reviewing in a way that counts toward a merge.
    Approval,
    /// A release, or a tag.
    ReleaseOrTag,
    /// A ruleset, branch protection or any other rule over the repo.
    Ruleset,
    /// The repo's administration: settings, hooks, keys, secrets, collaborators.
    Admin,
    /// A push to the default branch, a protected one, a ref that is not a branch, or a branch the
    /// chat did not create.
    ProtectedRef,
    /// A push that carries push options, which a forge reads as commands (`merge_request.*`).
    PushOption,
    /// A push whose ref commands were not read, or a request body that is too big or missing.
    BodyUnreadable,
    /// A GraphQL document that does not parse as one request.
    GraphqlParse,
    /// A GraphQL mutation that is not on the list.
    GraphqlUnknownMutation,
    /// A GraphQL document that reaches a repo, or an object, that is not the chat's.
    GraphqlOtherRepo,
    /// A sign-in at the forge: the device flow and OAuth endpoints.
    OAuthFlow,
    /// Git LFS, whose answers carry credentials for another host.
    Lfs,
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
    /// An archive or a release download of a repo.
    PublicRead,
    PullRequestWrite,
    CommentWrite,
    IssueWrite,
    LabelWrite,
    GraphqlQuery,
    /// The mutation's name, which is on the list.
    GraphqlMutation(String),
}

/// Which of a repo's doors a request came through: it decides the form of the credential, and
/// whether there is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Surface {
    Git,
    Api,
    /// A public read of a repo that is not the chat's: no credential is added.
    Anonymous,
}

/// A request that may be answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allowed {
    pub kind: Kind,
    /// The host the credential is looked up by.
    pub host: String,
    /// The allowed repo the request is for: `owner/repo`. Empty for the few reads outside a repo
    /// and for a public repo that is not the chat's.
    pub repo: String,
    pub class: Class,
    /// Where to send it: `https://host[:port]/path?query`, built by the proxy.
    pub upstream: String,
    pub(crate) surface: Surface,
}

impl Allowed {
    /// Whether the person's credential is put in.
    pub fn credentialed(&self) -> bool {
        self.surface != Surface::Anonymous
    }
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
    /// The pkt-lines of a push's commands and its push options, to the second flush packet (the
    /// pack follows it and is not read).
    PushCommands,
    /// A GraphQL request's JSON, or a REST write's body, whole, up to 1 MiB.
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

/// The reads outside any repo that `gh`, `glab` and git need, and nothing else.
const NON_REPO_READS: &[(&[&str], Kind)] = &[
    (&["user"], Kind::GitHub),
    (&["meta"], Kind::GitHub),
    (&["rate_limit"], Kind::GitHub),
    (&["api", "v4", "user"], Kind::GitLab),
    (&["api", "v4", "version"], Kind::GitLab),
];

/// A chat's repos, the branches it created, and the rule that decides every request against them.
#[derive(Debug)]
pub struct ChatPolicy {
    repos: Vec<AllowedRepo>,
    /// `(host, repo path, branch)`, lower case: the branches the chat may move and delete.
    own_branches: Mutex<HashSet<(String, String, String)>>,
}

/// What of the body a request needs, from its line alone.
pub(crate) fn body_need(head: &RequestHead) -> BodyNeed {
    let path = head.target.split(['?', '#']).next().unwrap_or_default();
    let last = path.rsplit('/').next().unwrap_or_default();
    let method = head.method.to_ascii_uppercase();
    if method == "POST" && last == "git-receive-pack" {
        BodyNeed::PushCommands
    } else if matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE") {
        BodyNeed::Json
    } else {
        BodyNeed::Nothing
    }
}

impl ChatPolicy {
    pub fn new(repos: Vec<AllowedRepo>) -> Self {
        let own = repos
            .iter()
            .flat_map(|r| {
                r.own_branches
                    .iter()
                    .map(|b| (r.host.clone(), r.path.clone(), b.to_ascii_lowercase()))
            })
            .collect();
        Self {
            repos,
            own_branches: Mutex::new(own),
        }
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
        // A method chosen by the query is a second method: refused, not stripped.
        if target.has_query_key("_method") {
            return Err(Refusal::MalformedRequest);
        }
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
        let first = candidates[0];
        let segments = &target.segments;
        let on_api_host = host == first.api_host() && host != first.host;
        let strs: Vec<&str> = segments.iter().map(String::as_str).collect();

        // A sign-in at the forge is not a chat's to start (device-flow phishing).
        if matches!(
            strs.as_slice(),
            ["login", "device" | "oauth", ..] | ["oauth", ..]
        ) && !on_api_host
        {
            return Err(Refusal::OAuthFlow);
        }
        if strs.windows(2).any(|w| w == ["info", "lfs"]) {
            return Err(Refusal::Lfs);
        }

        // The few reads outside a repo.
        if method == "GET" || method == "HEAD" {
            let prefix: &[&str] = if kind == Kind::GitHub && !on_api_host && host != "github.com" {
                &["api", "v3"]
            } else {
                &[]
            };
            let wanted = NON_REPO_READS.iter().any(|(path, k)| {
                *k == kind
                    && match kind {
                        Kind::GitHub => {
                            (on_api_host || host != "github.com")
                                && strs.len() == prefix.len() + path.len()
                                && strs[..prefix.len()] == *prefix
                                && strs[prefix.len()..] == **path
                        }
                        Kind::GitLab => strs == **path,
                    }
            });
            if wanted {
                return Ok(Allowed {
                    kind,
                    host: first.host.clone(),
                    repo: String::new(),
                    class: Class::ApiRead,
                    upstream: upstream(first, Surface::Api, segments, &target),
                    surface: Surface::Api,
                });
            }
        }

        // GraphQL: one URL, decided by the document.
        let graphql_path = match kind {
            Kind::GitHub if on_api_host => strs == ["graphql"],
            _ => strs == ["api", "graphql"],
        };
        if graphql_path {
            if method != "POST" {
                return Err(Refusal::NotOnList);
            }
            let class = graphql::decide(kind, &candidates, body, objects)?;
            // Every id in the document was tied to one of the chat's repos.
            return Ok(Allowed {
                kind,
                host: first.host.clone(),
                repo: first.path.clone(),
                class,
                upstream: upstream(first, Surface::Api, segments, &target),
                surface: Surface::Api,
            });
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
            let class = rest::decide(kind, method, &rel, &target, body, repo)?;
            return Ok(allowed(repo, class, Surface::Api, segments, &target));
        }

        // Git over HTTP, and a repo's public downloads.
        if on_api_host {
            return Err(Refusal::NotOnList);
        }
        if let Some((repo_segments, op)) = git::classify(method, segments, &target) {
            let path = normalise_repo_path(kind, &repo_segments.join("/"))
                .map_err(|_| Refusal::OtherRepo)?;
            return match candidates.iter().find(|r| r.path == path) {
                Some(repo) => {
                    let class = match op {
                        GitOp::Advertise => Class::GitRefs,
                        GitOp::UploadPack => Class::GitFetch,
                        GitOp::ReceivePack => {
                            self.check_push(repo, body)?;
                            Class::GitPush
                        }
                    };
                    Ok(allowed(repo, class, Surface::Git, segments, &target))
                }
                None => {
                    // Not the chat's: a fetch of a public repo, with no credential; never a push.
                    let fetch = op == GitOp::UploadPack
                        || (op == GitOp::Advertise
                            && target.query_value("service") == Some("git-upload-pack"));
                    if !fetch {
                        return Err(Refusal::OtherRepo);
                    }
                    let class = if op == GitOp::UploadPack {
                        Class::GitFetch
                    } else {
                        Class::GitRefs
                    };
                    Ok(public(first, class, segments, &target))
                }
            };
        }
        if let Some(repo_segments) = git::public_download(kind, method, segments) {
            let path = normalise_repo_path(kind, &repo_segments.join("/"))
                .map_err(|_| Refusal::OtherRepo)?;
            return Ok(match candidates.iter().find(|r| r.path == path) {
                Some(repo) => allowed(repo, Class::PublicRead, Surface::Git, segments, &target),
                None => public(first, Class::PublicRead, segments, &target),
            });
        }
        Err(Refusal::NotOnList)
    }

    /// A push's commands against the repo: every ref a branch, none the default, a protected one
    /// or a pages branch; a branch may be created, and only one the chat created may be moved or
    /// deleted. That refuses a force-push and a non-fast-forward anywhere else: the proxy sees
    /// old and new ids, never history. Push options are refused. Branches this push creates are
    /// recorded only if every command in it is allowed.
    fn check_push(&self, repo: &AllowedRepo, body: &[u8]) -> Result<(), Refusal> {
        let pushed = git::pushed_commands(body).map_err(|e| match e {
            PushError::Unreadable | PushError::PushCert => Refusal::BodyUnreadable,
        })?;
        if pushed.push_options {
            return Err(Refusal::PushOption);
        }
        let mut own = self
            .own_branches
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut created = Vec::new();
        for command in &pushed.commands {
            let name = &command.name;
            if name.starts_with("refs/tags/") {
                return Err(Refusal::ReleaseOrTag);
            }
            let Some(branch) = name.strip_prefix("refs/heads/") else {
                return Err(Refusal::ProtectedRef);
            };
            if branch.is_empty() || repo.protects(branch) {
                return Err(Refusal::ProtectedRef);
            }
            let key = (
                repo.host.clone(),
                repo.path.clone(),
                branch.to_ascii_lowercase(),
            );
            if command.create {
                created.push(key);
            } else if !own.contains(&key) && !created.contains(&key) {
                return Err(Refusal::ProtectedRef);
            }
        }
        own.extend(created);
        Ok(())
    }
}

fn allowed(
    repo: &AllowedRepo,
    class: Class,
    surface: Surface,
    segments: &[String],
    target: &Target,
) -> Allowed {
    Allowed {
        kind: repo.kind,
        host: repo.host.clone(),
        repo: repo.path.clone(),
        class,
        upstream: upstream(repo, surface, segments, target),
        surface,
    }
}

/// A public read of a repo that is not the chat's, sent to the host of the chat's own repos.
fn public(host_of: &AllowedRepo, class: Class, segments: &[String], target: &Target) -> Allowed {
    Allowed {
        kind: host_of.kind,
        host: host_of.host.clone(),
        repo: String::new(),
        class,
        upstream: upstream(host_of, Surface::Anonymous, segments, target),
        surface: Surface::Anonymous,
    }
}

/// The address to send a request to: the recorded repo's own host (its API host for the API), its
/// port, and the path and query as the policy read them, encoded again. A repo's name keeps the
/// case the chat wrote: both forges fold it themselves.
fn upstream(repo: &AllowedRepo, surface: Surface, segments: &[String], target: &Target) -> String {
    use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
    const KEEP: &AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~');
    let host = match surface {
        Surface::Api => repo.api_host(),
        Surface::Git | Surface::Anonymous => &repo.host,
    };
    let mut url = format!("https://{host}");
    if repo.port != 443 {
        url.push_str(&format!(":{}", repo.port));
    }
    if segments.is_empty() {
        url.push('/');
    }
    for segment in segments {
        url.push('/');
        url.extend(utf8_percent_encode(segment, KEEP));
    }
    if !target.query.is_empty() {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in &target.query {
            query.append_pair(k, v);
        }
        url.push('?');
        url.push_str(&query.finish());
    }
    url
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
