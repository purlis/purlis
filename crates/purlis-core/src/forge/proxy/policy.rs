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

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use crate::forge::createdbranches::{CreatedBranch, OwnedTip};

use super::commands;
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
    /// A call that names no repo (`/user`, GraphQL), when the chat has repos on more than one host
    /// of that forge: which one is meant is not the chat's to say.
    AmbiguousHost,
    /// A repo that is not one of the chat's, to the letter.
    OtherRepo,
    /// Merging, in any form: REST, GraphQL, auto-merge, merge when the pipeline succeeds.
    Merge,
    /// Text that tells the forge or a merge bot to merge or approve: `/merge`, `bors r+`…
    MergeCommand,
    /// Text a GitLab quick action would act on (`/move`, `/relate`, `/clone`…): each writes
    /// outside the recorded repo or makes what the proxy has not seen.
    QuickAction,
    /// A write that names labels by id (`label_ids`, `labelIds`): the proxy cannot tell a merge
    /// label from another by its id.
    LabelById,
    /// Approving or reviewing in a way that counts toward a merge.
    Approval,
    /// A release, or a tag.
    ReleaseOrTag,
    /// A ruleset, branch protection or any other rule over the repo.
    Ruleset,
    /// The repo's administration: settings, hooks, keys, secrets, collaborators.
    Admin,
    /// A push to the default branch, a protected one, a ref that is not a branch, a delete, or a
    /// branch the chat owns but from a tip it did not leave there.
    ProtectedRef,
    /// An update to a branch that is not protected and that this chat has no record of owning:
    /// no one's, or another chat's. **The one push refusal the person may answer**: an ask in the
    /// window lets this chat push it ([`crate::windowonly::Control::LetChatPush`]); nothing else
    /// about the push has changed. A push with any other refusal in it is that refusal.
    NotYourBranch,
    /// A push that carries push options, which a forge reads as commands (`merge_request.*`).
    PushOption,
    /// A push whose ref commands were not read, or a request body that is too big or missing.
    BodyUnreadable,
    /// A GraphQL document that does not parse as one request.
    GraphqlParse,
    /// A pull request opened from the default branch, or an update-branch (GitHub) or rebase
    /// (GitLab) on a pull request whose head is not a branch of the chat's own repo that this chat
    /// put its own tip on, at the tip the forge reports now. The forge writes the base into the
    /// head, so anything else would land the chat's code in a branch it does not own (#7494,
    /// #7526).
    ProtectedPullHead,
    /// A GraphQL mutation that is not on the list.
    GraphqlUnknownMutation,
    /// A GraphQL document that reaches a repo, or an object, that is not the chat's.
    GraphqlOtherRepo,
    /// [`Self::GraphqlOtherRepo`] for a `createPullRequest` or `updatePullRequestBranch` mutation
    /// (`gh pr create`, `gh pr update-branch`), the two that have a REST route to be told of.
    GraphqlPullRequestOtherRepo,
    /// A credential in the query string, which the forge would read as the chat's own login.
    CredentialInQuery,
    /// A multipart body, which a chat may send only as one file to a route that takes an upload.
    Multipart,
    /// A credential as a key of a request body (`private_token`, `access_token`, `token`…).
    CredentialInBody,
    /// A push makes a branch of a repo whose refs the proxy was not told: whether the branch exists
    /// is not known, so it is not made.
    NoAdvertisement,
    /// A sign-in at the forge: the device flow and OAuth endpoints.
    OAuthFlow,
    /// Git LFS, whose answers carry credentials for another host.
    Lfs,
    /// A route that is not on the list of what a chat may write, or a request of a kind the proxy
    /// does not carry. The word is the audit's: the chat is told the request's route pattern and
    /// repo, and that no setting can allow it ([`super::ForgeProxy::refused_said`], #7779).
    NotOnList,
    /// The request was allowed, and the person's credential could not be had.
    NoCredential,
    /// An administrator's policy pins this forge as never allowed (ADR 0128 D3g): refused for
    /// the chat entirely, whatever its repos and whoever is signed in.
    Pinned,
}

impl Refusal {
    /// What a chat should do instead, for the refusals that have a way round.
    #[must_use]
    pub fn advice(self) -> Option<&'static str> {
        match self {
            Self::QuickAction => Some(
                "set labels through the API's labels field, not a /label, /unlabel or /relabel line, and leave other quick actions out of the text",
            ),
            Self::LabelById => Some("name labels in the API's labels field, not by id"),
            // A chat put back after a restart keeps the credential of a route that went with the
            // old app (ADR 0128 D3a); the app's own git has one for it (#7694, #7878).
            Self::UnknownChat | Self::WrongSentinel | Self::Locked => Some(
                "this chat's forge route no longer takes its credential (purlis restarted, or the chat ended or was locked out); push with `purlis worktree push`, or start a new chat",
            ),
            Self::GraphqlPullRequestOtherRepo => Some(
                "if the repo is this chat's own, the REST route for the same act is answered: `gh api repos/OWNER/REPO/pulls -X POST -f title=... -f head=BRANCH -f base=main` for gh pr create, `gh api -X PUT repos/OWNER/REPO/pulls/N/update-branch` for gh pr update-branch",
            ),
            Self::ProtectedPullHead => Some(
                "open the pull request from your feature branch into the default branch, never the other way round, and run update-branch or a rebase only on a pull request whose head is one of this chat's own branches in this repo, at the tip this chat pushed (push it first; an update-branch or rebase moves the head, so push again from the new tip)",
            ),
            _ => None,
        }
    }
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
    /// A merge `guards.merge` = `log` or `off` let through: a pull request or merge request merge,
    /// or GitLab's cancel of merge-when-pipeline-succeeds.
    MergeWrite,
    CommentWrite,
    IssueWrite,
    LabelWrite,
    /// A rerun of a workflow run of the chat's repo (#7358), or a retry or cancel of a GitLab
    /// pipeline or job.
    CiWrite,
    GraphqlQuery,
    /// The batch call of Git LFS, asking for download addresses only.
    LfsBatch,
    /// A file uploaded for a comment or a description (GitLab's `uploads`).
    UploadWrite,
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
    /// The port the request goes to; with `host`, the address the credential is looked up by.
    pub port: u16,
    /// The forge-read numeric id of the repo (`None` for a request outside a repo).
    pub repo_id: Option<u64>,
    /// The allowed repo the request is for: `owner/repo`. Empty for the few reads outside a repo
    /// and for a public repo that is not the chat's.
    pub repo: String,
    pub class: Class,
    /// Where to send it: `https://host[:port]/path?query`, built by the proxy.
    pub upstream: String,
    /// What to send as the body in place of the chat's: the body as it was read.
    pub canonical_body: Option<Vec<u8>>,
    /// The `Content-Type` that body is sent with, said once.
    pub canonical_media: Option<String>,
    /// Branches this push makes or moves, with the tip it leaves them at. They are recorded, and
    /// the tip moves, only when
    /// [`ForgeProxy::confirm_push`](super::ForgeProxy::confirm_push) is told the forge took the push.
    pub(crate) created: Vec<CreatedBranch>,
    /// The branches of this push whose update must be a fast-forward of the forge's tip: the
    /// chat owns them by the person's Allow ([`OwnedTip::fast_forward_only`]). The proxy sees
    /// old and new ids, never history, so the push's own objects are read for it before it is
    /// sent ([`super::listener::PushUpdate::fast_forward_only`]; #7841).
    pub(crate) fast_forward_only: Vec<String>,
    pub(crate) surface: Surface,
    /// Every repo a GraphQL document named (empty for any other request): the credential is asked
    /// for each, and the request is served only if one account serves them all.
    pub touched: Vec<TouchedRepo>,
}

/// A repo a GraphQL document named: what a credential is asked for it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchedRepo {
    /// The forge-read numeric id.
    pub repo_id: Option<u64>,
    /// `owner/repo`.
    pub repo: String,
}

impl Allowed {
    /// Whether the person's credential is put in.
    pub fn credentialed(&self) -> bool {
        self.surface != Surface::Anonymous
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    clippy::large_enum_variant,
    reason = "one decision is made per request and read at once: boxing the allowed request would only add a heap allocation to every allowed one"
)]
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

    /// The normalised `owner/repo` path and the number of the pull request `node_id` is, on
    /// `host`: what a held merge names (#7543). `None` where the id is not a pull request or the
    /// forge could not say, and the merge is then not asked about.
    fn pull_of(&self, _kind: Kind, _host: &str, _node_id: &str) -> Option<(String, u64)> {
        None
    }

    /// The head of the pull request `pull` names, on `host`, or `None` when the forge cannot say.
    /// **`None` refuses**: update-branch writes to the head, so a head that is not known is not
    /// written to (#7494). A resolver that does not implement it refuses every update-branch.
    fn pull_head(&self, _kind: Kind, _host: &str, _pull: PullRef<'_>) -> Option<PullHead> {
        None
    }
}

/// A pull request's head as the forge says it: the repo it is in (lower-case `owner/repo`, which
/// is not the base's when the pull request is from a fork) and the branch's short name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullHead {
    pub repo: String,
    pub branch: String,
    /// The head's tip as the forge says it now (`headRefOid`, GitLab's `diff_head_sha`): what the
    /// ownership record must equal for the update to be let through (#7526).
    pub oid: String,
}

/// How a pull request is named when its head is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRef<'a> {
    /// A GraphQL node id.
    Node(&'a str),
    /// A number in the repo `repo` (lower-case `owner/repo`), as REST names it.
    Number { repo: &'a str, number: u64 },
}

/// Whether the chat may write to a pull request's head: the question update-branch and a rebase
/// are asked ([`ChatPolicy::owns_head`]).
pub(super) type OwnsHead<'a> = &'a dyn Fn(&AllowedRepo, &PullHead) -> bool;

/// The resolver that knows nothing, so every mutation on an object is refused.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoObjects;

impl ObjectRepo for NoObjects {
    fn repo_of(&self, _: Kind, _: &str, _: &str) -> Option<String> {
        None
    }
}

/// Hosts that serve a repo's downloads from a signed address, which a chat may tunnel to and which
/// are never terminated.
const DOWNLOAD_HOSTS: &[&str] = &[
    "codeload.github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
];

/// Whether `host` only serves a repo's downloads from a signed address: tunnelled to, never
/// terminated, and the one place the proxy follows a redirect to, once and with no credential.
pub fn is_download_host(host: &str) -> bool {
    DOWNLOAD_HOSTS.contains(&host)
}

/// Whether `host` is where GitHub Actions keeps a run's logs and artifacts (#7358): `*.actions.githubusercontent.com`
/// (one label under it, as `pipelines.` and `results-receiver.`, but never `token.` or `vstoken.`) and Actions results storage,
/// `productionresultssa<digits>.blob.core.windows.net`. **Never a bare `*.blob.core.windows.net`**,
/// which is anyone's storage account. The address is pre-signed, so it is followed once, by a GET
/// and with no credential, and only for the chat on its unix socket: it is never tunnelled to, so
/// it is on no [`DOWNLOAD_HOSTS`] list and the chat's own egress still refuses it.
pub fn is_ci_storage_host(host: &str) -> bool {
    let label = |l: &str| {
        !l.is_empty()
            && l.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !l.starts_with('-')
    };
    if let Some(sub) = host.strip_suffix(".actions.githubusercontent.com") {
        // `token.` and `vstoken.` mint Actions tokens: never a place a log is kept.
        return label(sub) && !matches!(sub, "token" | "vstoken");
    }
    host.strip_suffix(".blob.core.windows.net")
        .and_then(|account| account.strip_prefix("productionresultssa"))
        .is_some_and(|n| (1..=5).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Where gitlab.com keeps a job's artifacts and archived log: Google Cloud Storage, in the
/// one bucket GitLab names for it, `gitlab-gprd-artifacts`. **Never a bare `storage.googleapis.com`**,
/// which is anyone's bucket. `path` is the address's path as the URL parser left it, so a `..` in it
/// is already resolved. Only gitlab.com's own redirect is followed ([`GITLAB_COM`]): a self-hosted
/// GitLab serves a job's log and artifacts itself, and its object storage (an operator's choice
/// of host) is not carried. As GitHub's, the address is pre-signed: followed once, by a GET, with
/// no credential.
pub fn is_gitlab_ci_storage(host: &str, path: &str) -> bool {
    host == "storage.googleapis.com"
        && path
            .strip_prefix("/gitlab-gprd-artifacts/")
            .is_some_and(|object| !object.is_empty())
}

/// The one GitLab host whose redirects to [`is_gitlab_ci_storage`] are followed.
pub const GITLAB_COM: &str = "gitlab.com";

/// The reads outside any repo that `gh`, `glab` and git need, and nothing else.
const NON_REPO_READS: &[(&[&str], Kind)] = &[
    // The API root, which `gh auth status` reads to check the token and its scopes: the same
    // fixed document for anyone, and the scopes header is the brokered token's own.
    (&[], Kind::GitHub),
    (&["user"], Kind::GitHub),
    (&["meta"], Kind::GitHub),
    (&["rate_limit"], Kind::GitHub),
    (&["api", "v4", "user"], Kind::GitLab),
    (&["api", "v4", "version"], Kind::GitLab),
];

/// What to do with a client's `CONNECT host:port` (git and `glab` through `HTTPS_PROXY`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectVerdict {
    /// Terminate TLS here with a leaf of the chat's own CA for `host`, then decide each decrypted
    /// request with [`ChatPolicy::decide`]: the same policy as a request on a unix socket.
    Terminate {
        host: String,
    },
    /// A plain tunnel, with the server's own certificate and nothing read: a host that only
    /// serves downloads, for a chat that has a repo on github.com. Never terminated, so no leaf
    /// is ever made for it and the chat's CA is not asked to vouch for it.
    Tunnel {
        host: String,
    },
    Refuse(Refusal),
}

/// Where the name `github.localhost` leads for a chat: the recorded GitHub host's API address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubAlias {
    /// The host its API answers on: `api.github.com`, or the GHES host.
    pub host: String,
    pub port: u16,
    /// A GHES host, whose API is at `/api/v3` and `/api/graphql`, where github.com's is at the
    /// root: `gh` speaks github.com's shape to `github.localhost`.
    pub enterprise: bool,
}

impl GithubAlias {
    /// The request-target a GitHub Enterprise Server serves for the one `gh` sent in
    /// github.com's shape (`/user` is `/api/v3/user`, `/graphql` is `/api/graphql`). On
    /// github.com it is the target as it is.
    #[must_use]
    pub fn target(&self, target: &str) -> String {
        if !self.enterprise {
            return target.to_string();
        }
        let (path, query) = match target.split_once('?') {
            Some((path, query)) => (path, Some(query)),
            None => (target, None),
        };
        let mapped = if path == "/graphql" {
            "/api/graphql".to_string()
        } else {
            format!("/api/v3{path}")
        };
        match query {
            Some(query) => format!("{mapped}?{query}"),
            None => mapped,
        }
    }
}

/// A chat's repos, the branches it created, and the rule that decides every request against them.
pub struct ChatPolicy {
    repos: Vec<AllowedRepo>,
    /// `(host, repo path, branch)`, the branch's case exact, and the tip this chat last put on it:
    /// the branches the chat may move, from that tip. Decides only where there is no `tips`.
    own_branches: Mutex<HashMap<(String, String, String), String>>,
    /// Where the tip the chat last put on a branch is read from at each update, when the app
    /// keeps it where every chat's pushes land. It decides over `own_branches`, so a push by
    /// another chat moves it under this one.
    tips: Option<TipSource>,
    /// Where the branches the app's record holds for this chat are read from when an
    /// advertisement is noted, so a branch recorded before this run, and gone from the forge
    /// since, has its record ended too.
    recorded: Option<RecordedBranches>,
    /// The branches each repo's last advertisement for a push named, as the listener told the
    /// proxy, keyed by `(host, repo path)`.
    advertised: Mutex<HashMap<(String, String), HashSet<String>>>,
}

/// The tip the chat last put on a branch, `(host, forge repo id, branch)`: asked at **every update**
/// and never kept, so another chat's push, a branch made again, or a record that ended is seen at
/// once. `None` is not this chat's, or no tip that reads: the update is refused. A tip the chat
/// holds by the person's Allow moves only forward ([`OwnedTip::fast_forward_only`]).
pub type TipSource = Arc<dyn Fn(&str, u64, &str) -> Option<OwnedTip> + Send + Sync>;

/// The branches of a repo the chat's record holds, `(host, forge repo id)`: asked each time an
/// advertisement is noted, never kept.
pub type RecordedBranches = Arc<dyn Fn(&str, u64) -> Vec<String> + Send + Sync>;

/// The kind, path segments, query and host of a request, for the hand-off of a held act.
pub(super) type HeldRoute = (Kind, Vec<String>, Vec<(String, String)>, String);

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

impl std::fmt::Debug for ChatPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatPolicy")
            .field("repos", &self.repos)
            .field("tips", &self.tips.is_some())
            .finish_non_exhaustive()
    }
}

impl ChatPolicy {
    pub fn new(repos: Vec<AllowedRepo>) -> Self {
        let own = repos
            .iter()
            .flat_map(|r| {
                r.own_branches
                    .iter()
                    .map(|(b, tip)| ((r.host.clone(), r.path.clone(), b.clone()), tip.clone()))
            })
            .collect();
        Self {
            repos,
            own_branches: Mutex::new(own),
            tips: None,
            recorded: None,
            advertised: Mutex::new(HashMap::new()),
        }
    }

    /// Read the tip of a branch from `tips` at each update, in place of what this policy kept.
    #[must_use]
    pub fn with_tips(mut self, tips: TipSource) -> Self {
        self.tips = Some(tips);
        self
    }

    /// Read the branches the record holds for this chat from `recorded` when an advertisement is
    /// noted, so one gone from the forge ends its record whichever run recorded it.
    #[must_use]
    pub fn with_recorded(mut self, recorded: RecordedBranches) -> Self {
        self.recorded = Some(recorded);
        self
    }

    /// The forge kind, path segments, query and host a request of this chat is for, where one of
    /// the chat's repos is served at its host. For the hand-off of a held act.
    pub(super) fn route_of(&self, head: &RequestHead) -> Option<HeldRoute> {
        let host = request::host(head).ok()?;
        let target = request::parse_target(&head.target).ok()?;
        let first = self.repos.iter().find(|r| r.serves(&host, head.port))?;
        Some((first.kind, target.segments, target.query, host))
    }

    /// The chat's repos on `host:port`, as the recogniser asks about them.
    pub(super) fn recorded_on(&self, host: &str, port: u16) -> RecordedRepos {
        RecordedRepos(
            self.repos
                .iter()
                .filter(|r| r.serves(host, port))
                .cloned()
                .collect(),
        )
    }

    /// The listener read a repo's ref advertisement for a push, and these are its branches (short
    /// names). A push that makes a branch is decided against the latest one.
    ///
    /// **Returns the chat's own branches of that repo the advertisement does not list: they are
    /// gone from the forge**, so they are the chat's own no more (a name made again is not the
    /// chat's), and the caller ends their record. A repo always has its default branch, so an
    /// advertisement with no branch at all is one that did not read, and ends nothing.
    pub fn note_advertised(
        &self,
        host: &str,
        repo_path: &str,
        branches: Vec<String>,
    ) -> Vec<CreatedBranch> {
        let (host, repo_path) = (host.to_ascii_lowercase(), repo_path.to_ascii_lowercase());
        let listed: HashSet<String> = branches.into_iter().collect();
        let mut gone = Vec::new();
        if !listed.is_empty()
            && let Some(repo) = self
                .repos
                .iter()
                .find(|r| r.host == host && r.path == repo_path)
        {
            self.own_branches
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .retain(|(h, p, branch), _| {
                    let ours = *h == host && *p == repo_path;
                    if ours && !listed.contains(branch) {
                        gone.push(CreatedBranch {
                            host: host.clone(),
                            repo: repo_path.clone(),
                            repo_id: repo.project_id,
                            branch: branch.clone(),
                            tip: String::new(),
                            fast_forward_only: false,
                        });
                        return false;
                    }
                    true
                });
        }
        // The record may hold branches this run never saw made (an earlier run's): any of them the
        // advertisement does not list is gone from the forge too. Needs the repo's forge id, the
        // record's key, and an advertisement that read.
        if !listed.is_empty()
            && let (Some(recorded), Some(repo)) = (
                &self.recorded,
                self.repos
                    .iter()
                    .find(|r| r.host == host && r.path == repo_path),
            )
            && let Some(repo_id) = repo.project_id
        {
            for branch in recorded(&host, repo_id) {
                if !listed.contains(&branch) && !gone.iter().any(|g| g.branch == branch) {
                    gone.push(CreatedBranch {
                        host: host.clone(),
                        repo: repo_path.clone(),
                        repo_id: Some(repo_id),
                        branch,
                        tip: String::new(),
                        fast_forward_only: false,
                    });
                }
            }
        }
        self.advertised
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert((host, repo_path), listed);
        gone
    }

    /// The forge made or moved these branches (a push's report said so): they are the chat's own
    /// now, at the tip that push left them at.
    pub(crate) fn confirm_created(&self, created: &[CreatedBranch]) {
        self.own_branches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend(created.iter().map(|c| {
                (
                    (c.host.clone(), c.repo.clone(), c.branch.clone()),
                    c.tip.to_ascii_lowercase(),
                )
            }));
    }

    /// Whether `host` is the API host of one of the chat's GitHub repos on port 443: the one
    /// place a `Host` without a port on the plain door is read as meaning 443 (#7679).
    pub fn is_github_api_host(&self, host: &str) -> bool {
        self.repos.iter().any(|r| {
            r.kind == Kind::GitHub && r.port == 443 && r.api_host().eq_ignore_ascii_case(host)
        })
    }

    /// What `gh`'s `GH_HOST=github.localhost` means for this chat (ADR 0128 D3c): the one GitHub
    /// host its repos are on. `None` is: not that name. `Some(Err(AmbiguousHost))` is: two GitHub
    /// hosts are recorded (github.com and a GHES, or two GHES), which of them a call that names
    /// no host means is not the chat's to say, so it is refused; and `Some(Err(HostNotAllowed))`
    /// is: no GitHub repo at all.
    pub fn github_alias(&self, host: &str) -> Option<Result<GithubAlias, Refusal>> {
        let host = request::normalise_connect_host(host).ok()?;
        if host != "github.localhost" && host != "api.github.localhost" {
            return None;
        }
        let mut places: Vec<(&str, u16)> = self
            .repos
            .iter()
            .filter(|r| r.kind == Kind::GitHub)
            .map(|r| (r.host.as_str(), r.port))
            .collect();
        places.sort_unstable();
        places.dedup();
        Some(match places.as_slice() {
            [] => Err(Refusal::HostNotAllowed),
            [(recorded, port)] => {
                let repo = self
                    .repos
                    .iter()
                    .find(|r| r.kind == Kind::GitHub && r.host == *recorded && r.port == *port)
                    .ok_or(Refusal::HostNotAllowed);
                repo.map(|repo| GithubAlias {
                    host: repo.api_host().to_string(),
                    port: repo.port,
                    enterprise: repo.host != "github.com",
                })
            }
            _ => Err(Refusal::AmbiguousHost),
        })
    }

    /// Decide a `CONNECT`: only a host and port one of the chat's repos is on is terminated, so a
    /// leaf is never made for any other name. Anything else is refused, not tunnelled: a tunnel
    /// cannot be policed (anonymous reads of other hosts are a later, stated rule).
    pub fn connect(&self, host: &str, port: u16) -> ConnectVerdict {
        let Ok(host) = request::normalise_connect_host(host) else {
            return ConnectVerdict::Refuse(Refusal::MalformedRequest);
        };
        if self.repos.iter().any(|r| r.serves(&host, port)) {
            ConnectVerdict::Terminate { host }
        } else if port == 443
            && DOWNLOAD_HOSTS.contains(&host.as_str())
            && self
                .repos
                .iter()
                .any(|r| r.kind == Kind::GitHub && r.host == "github.com")
        {
            ConnectVerdict::Tunnel { host }
        } else {
            ConnectVerdict::Refuse(Refusal::HostNotAllowed)
        }
    }

    /// Every host a leaf certificate may be made for: the hosts of the chat's repos, and the API
    /// host of each. The chat's CA is constrained to these names (and no others), and
    /// [`Self::connect`] terminates only these.
    pub fn terminable_hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = self
            .repos
            .iter()
            .flat_map(|r| [r.host.clone(), r.api_host().to_string()])
            .collect();
        hosts.sort();
        hosts.dedup();
        hosts
    }

    /// Whether a call that names no repo can only mean one host of `kind`: the chat's repos of
    /// that kind are all on the same host.
    fn has_one_host(&self, kind: Kind) -> bool {
        let mut hosts = self
            .repos
            .iter()
            .filter(|r| r.kind == kind)
            .map(|r| &r.host);
        match hosts.next() {
            Some(first) => hosts.all(|h| h == first),
            None => false,
        }
    }

    /// Decide one request. `body` is what [`body_need`] asked for: nothing for most requests.
    pub fn decide(&self, head: &RequestHead, body: &[u8], objects: &dyn ObjectRepo) -> Decision {
        self.decide_as(head, body, objects, false)
    }

    /// [`Self::decide`] with the merge routes open, for `guards.merge` = `log` or `off`: the REST
    /// and GraphQL merges, auto-merge and the merge queue of [`graphql::merge_mutation`] and
    /// [`rest::merge_class`], each still tied to a recorded repo of the chat by the same checks
    /// as any write. A request that is anything else is decided exactly as [`Self::decide`] does.
    pub fn decide_merging(
        &self,
        head: &RequestHead,
        body: &[u8],
        objects: &dyn ObjectRepo,
    ) -> Decision {
        self.decide_as(head, body, objects, true)
    }

    /// **The merge a GraphQL merge mutation asks for, to be held for the person** (#7543): what
    /// `enforce` refuses as [`Refusal::Merge`] and the person's Merge press can perform.
    ///
    /// - `None` where the request is not a merge mutation the merge routes would let through: it
    ///   passes every check of any write (a recorded repo, no merge command in its text), so a
    ///   document that is anything else is refused as it was and nobody is asked.
    /// - `Some(Err(why))` where it is one, but the pull request cannot be named (the forge did
    ///   not say its number): refused, and the chat is told why.
    /// - `Some(Ok(requested))` where it is one and the pull request is named.
    ///
    /// The merge and merge-queue mutations are each held as the act they name, which the
    /// person's press performs and no other. Auto-merge, and a GitLab accept that sets a merge
    /// strategy, are refused with a reason; undoing one is never held.
    pub(super) fn held_graphql_merge(
        &self,
        head: &RequestHead,
        body: &[u8],
        objects: &dyn ObjectRepo,
    ) -> Option<Result<crate::heldact::Requested, &'static str>> {
        let Decision::Allow(allowed) = self.decide_merging(head, body, objects) else {
            return None;
        };
        let Class::GraphqlMutation(name) = &allowed.class else {
            return None;
        };
        let host = request::host(head).ok()?;
        let repos: Vec<&AllowedRepo> = self
            .repos
            .iter()
            .filter(|r| r.serves(&host, head.port))
            .collect();
        let kind = repos.first()?.kind;
        if let Some(said) = graphql::unheld_merge_said(kind, name) {
            return Some(Err(said));
        }
        graphql::merge_act(kind, name)?;
        if let Some(said) = graphql::strategy_said(kind, body) {
            return Some(Err(said));
        }
        let named = graphql::merge_target(kind, &repos, body, objects)
            .and_then(|(path, number, act)| {
                let repo = crate::mergehold::Repo::try_from(path).ok()?;
                Some(crate::heldact::Requested::land_of(repo, number, act))
            })
            .ok_or("purlis could not read which pull request of this repo the document merges (it reads one mutation with one field); ask the person to merge it");
        Some(named)
    }

    fn decide_as(
        &self,
        head: &RequestHead,
        body: &[u8],
        objects: &dyn ObjectRepo,
        merging: bool,
    ) -> Decision {
        match self.try_decide(head, body, objects, merging) {
            Ok(allowed) => Decision::Allow(allowed),
            Err(refusal) => Decision::Refuse(refusal),
        }
    }

    /// **Where a refused request was going** (#7779), for what it is told and how it is counted:
    /// its method, host and port, the recorded repo it named, and its path as words a refusal may
    /// show ([`super::said`]). `None` for a request whose host or target does not read, or that
    /// is on none of the chat's hosts. Read the way [`Self::decide`] reads it, and decides nothing.
    pub(super) fn place(&self, head: &RequestHead) -> Option<super::said::Place> {
        let host = request::host(head).ok()?;
        let target = request::parse_target(&head.target).ok()?;
        let first = self.repos.iter().find(|r| r.serves(&host, head.port))?;
        let kind = first.kind;
        let on_api_host = host == first.api_host() && host != first.host;
        let segments = &target.segments;
        let named = rest_path(kind, on_api_host, &host, segments)
            .ok()
            .flatten()
            .and_then(|(named, rel)| {
                self.repos
                    .iter()
                    .filter(|r| r.kind == kind && r.serves(&host, head.port))
                    .find(|r| repo_matches(r, &named))
                    .map(|repo| (repo, named, rel))
            });
        let (repo, shown, rel) = match named {
            Some((repo, named_as, rel)) => {
                // The words before the repo, and the repo as placeholders: the recorded repo is
                // named apart, never the chat's spelling of it.
                let before = segments.len() - rel.len();
                let (words, placeholders): (usize, &[&str]) = match (kind, &named_as) {
                    (Kind::GitHub, RestRepo::Id(_)) => (before.saturating_sub(1), &["{id}"]),
                    (Kind::GitHub, RestRepo::Path(_)) => {
                        (before.saturating_sub(2), &["{owner}", "{repo}"])
                    }
                    (Kind::GitLab, _) => (before.saturating_sub(1), &["{project}"]),
                };
                let shown = segments[..words]
                    .iter()
                    .map(|s| super::said::word(s))
                    .chain(placeholders.iter().map(|p| (*p).to_owned()))
                    .collect();
                (
                    Some(format!("{}/{}", repo.host, repo.path)),
                    shown,
                    Some(rel),
                )
            }
            None => (
                None,
                segments.iter().map(|s| super::said::word(s)).collect(),
                None,
            ),
        };
        Some(super::said::Place {
            kind,
            method: request::method(head),
            host,
            port: head.port,
            repo,
            shown,
            rel,
        })
    }

    fn try_decide(
        &self,
        head: &RequestHead,
        body: &[u8],
        objects: &dyn ObjectRepo,
        merging: bool,
    ) -> Result<Allowed, Refusal> {
        let method = request::method(head).ok_or(Refusal::NotOnList)?;
        let host = request::host(head)?;
        let target = request::parse_target(&head.target)?;
        // A method chosen by the query is a second method: refused, not stripped.
        if target.has_query_key("_method") {
            return Err(Refusal::MalformedRequest);
        }
        // A credential in the query is the forge reading the chat's own login: never, on any route.
        if request::has_credential_param(&target) {
            return Err(Refusal::CredentialInQuery);
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
        // Git LFS is its batch call, asking to download, and nothing else of it: the locks API,
        // an upload batch and the object routes are refused.
        let lfs_repo = git::lfs_batch(method, segments);
        if lfs_repo.is_none() && strs.windows(2).any(|w| w == ["info", "lfs"]) {
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
                if kind == Kind::GitHub && !self.has_one_host(Kind::GitHub) {
                    return Err(Refusal::AmbiguousHost);
                }
                return Ok(Allowed {
                    kind,
                    host: first.host.clone(),
                    port: first.port,
                    repo_id: None,
                    repo: String::new(),
                    class: Class::ApiRead,
                    upstream: upstream(first, Surface::Api, segments, &target),
                    canonical_body: None,
                    canonical_media: None,
                    created: Vec::new(),
                    fast_forward_only: Vec::new(),
                    surface: Surface::Api,
                    touched: Vec::new(),
                });
            }
        }

        // `GET /search/issues?q=`: issues and pull requests of one of the chat's own repos, which
        // the query must name by exactly one `repo:` (the same reading as GraphQL's `search`).
        if method == "GET" && kind == Kind::GitHub {
            let prefix: &[&str] = if on_api_host || host == "github.com" {
                &[]
            } else {
                &["api", "v3"]
            };
            if (on_api_host || host != "github.com")
                && strs.len() == prefix.len() + 2
                && strs[..prefix.len()] == *prefix
                && strs[prefix.len()..] == ["search", "issues"]
            {
                if !self.has_one_host(Kind::GitHub) {
                    return Err(Refusal::AmbiguousHost);
                }
                // One `q`, and only the keys that page or sort its answer: another key may select
                // a different search (`advanced_search`).
                let only_known = target
                    .query
                    .iter()
                    .all(|(k, _)| ["q", "sort", "order", "per_page", "page"].contains(&k.as_str()));
                let one_q = target.query.iter().filter(|(k, _)| k == "q").count() == 1;
                let at = target
                    .query_value("q")
                    .and_then(|q| graphql::search_repo(q, &candidates));
                return match at {
                    Some(at) if only_known && one_q => Ok(allowed(
                        candidates[at],
                        Class::ApiRead,
                        Surface::Api,
                        segments,
                        &target,
                    )),
                    _ => Err(Refusal::OtherRepo),
                };
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
            if kind == Kind::GitHub && !self.has_one_host(Kind::GitHub) {
                return Err(Refusal::AmbiguousHost);
            }
            // The document is the whole request: a query string is a second one.
            if !target.query.is_empty() {
                return Err(Refusal::MalformedRequest);
            }
            let document = commands::parse(body, head.content_type.as_deref())?
                .ok_or(Refusal::GraphqlParse)?;
            if document.json.is_none() {
                return Err(Refusal::GraphqlParse);
            }
            let decided = graphql::decide(
                kind,
                &candidates,
                &document.canonical,
                objects,
                &|r, h| self.owns_head(r, h),
                merging,
            )?;
            // Every id in the document was tied to one of the chat's repos, and the credential is
            // asked for each of them, not for the first: a document naming repo B is never sent
            // with repo A's token. One that touches none is a request outside a repo.
            let touched: Vec<TouchedRepo> = decided
                .touched
                .iter()
                .map(|r| TouchedRepo {
                    repo_id: r.project_id,
                    repo: r.path.clone(),
                })
                .collect();
            let (repo_id, repo) = match touched.first() {
                Some(one) => (one.repo_id, one.repo.clone()),
                None => (None, String::new()),
            };
            return Ok(Allowed {
                kind,
                host: first.host.clone(),
                port: first.port,
                repo_id,
                repo,
                class: decided.class,
                upstream: upstream(first, Surface::Api, segments, &target),
                canonical_media: Some(document.media),
                canonical_body: Some(document.canonical),
                created: Vec::new(),
                fast_forward_only: Vec::new(),
                surface: Surface::Api,
                touched,
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
            let read = method == "GET" || method == "HEAD";
            // The one multipart a chat may send: a file for a comment, to GitLab's `uploads`.
            let upload = kind == Kind::GitLab
                && method == "POST"
                && rel.len() == 1
                && rel[0].eq_ignore_ascii_case("uploads");
            let parsed = if read {
                None
            } else if upload {
                Some(commands::parse_upload(body, head.content_type.as_deref())?)
            } else {
                commands::parse(body, head.content_type.as_deref())?
            };
            let class = rest::decide(
                kind,
                method,
                &rel,
                &target,
                parsed.as_ref(),
                repo,
                objects,
                &|r, h| self.owns_head(r, h),
                merging,
            )?;
            let mut answer = allowed(repo, class, Surface::Api, segments, &target);
            if let Some(b) = parsed {
                answer.canonical_media = Some(b.media);
                answer.canonical_body = Some(b.canonical);
            }
            return Ok(answer);
        }

        // Git over HTTP, and a repo's public downloads.
        if on_api_host {
            return Err(Refusal::NotOnList);
        }
        if let Some(repo_segments) = lfs_repo {
            let path = normalise_repo_path(kind, &repo_segments.join("/"))
                .map_err(|_| Refusal::OtherRepo)?;
            let batch = Self::lfs_download_batch(body, head.content_type.as_deref())?;
            let mut answer = match candidates.iter().find(|r| r.path == path) {
                Some(repo) => allowed(repo, Class::LfsBatch, Surface::Git, segments, &target),
                // Not the chat's: a public repo's files, asked for with no credential.
                None => public(first, Class::LfsBatch, segments, &target),
            };
            answer.canonical_media = Some(batch.media);
            answer.canonical_body = Some(batch.canonical);
            return Ok(answer);
        }
        if let Some((repo_segments, op)) = git::classify(method, segments, &target) {
            let path = normalise_repo_path(kind, &repo_segments.join("/"))
                .map_err(|_| Refusal::OtherRepo)?;
            return match candidates.iter().find(|r| r.path == path) {
                Some(repo) => {
                    let class = match op {
                        GitOp::Advertise => Class::GitRefs,
                        GitOp::UploadPack => Class::GitFetch,
                        GitOp::ReceivePack => Class::GitPush,
                    };
                    let mut answer = allowed(repo, class, Surface::Git, segments, &target);
                    if op == GitOp::ReceivePack {
                        let checked = self.check_push(repo, body)?;
                        answer.created = checked.created;
                        answer.fast_forward_only = checked.fast_forward_only;
                    }
                    Ok(answer)
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

    /// The body of an LFS batch call, which may only ask for download addresses. Git LFS says its own
    /// media type, which is JSON; it is read as JSON and sent as it was said.
    fn lfs_download_batch(
        body: &[u8],
        content_type: Option<&str>,
    ) -> Result<commands::Body, Refusal> {
        const LFS: &str = "application/vnd.git-lfs+json";
        let media = content_type
            .and_then(|ct| ct.split(';').next())
            .map(|m| m.trim().to_ascii_lowercase());
        let said = match media.as_deref() {
            Some(LFS) => LFS,
            Some("application/json") => "application/json",
            _ => return Err(Refusal::BodyUnreadable),
        };
        let mut batch =
            commands::parse(body, Some("application/json"))?.ok_or(Refusal::BodyUnreadable)?;
        let download = batch
            .json
            .as_ref()
            .and_then(|j| j.get("operation"))
            .is_some_and(|op| op == "download");
        if !download {
            return Err(Refusal::Lfs);
        }
        batch.media = said.to_string();
        Ok(batch)
    }

    /// A push's commands against the repo: every ref a branch, none the default, a protected one,
    /// a deploy trigger or a pages branch; no ref deleted; a branch may be made if the proxy knows
    /// from the repo's advertisement that it does not exist, and a branch may be moved only from
    /// the tip this chat last put on it (the record's last line, read at every update). Any other
    /// old id is refused: a branch someone else made or moved, or one made again after it was
    /// gone. The forge refuses the old id if the ref is no longer there, so together the two
    /// refuse a force-push and a non-fast-forward on anyone else's branch: the proxy sees old and
    /// new ids, never history. **A branch the chat holds by the person's Allow was someone
    /// else's**, so its update is returned as one that must be a fast-forward, which the push's
    /// own objects decide before it is sent (floor item 6 of ADR 0125's 2026-11-27 amendment;
    /// #7841). Push options are refused. The branches this push makes and moves
    /// are **returned, not recorded**: they are the chat's once the forge confirms the whole push
    /// ([`Self::confirm_created`]).
    fn check_push(&self, repo: &AllowedRepo, body: &[u8]) -> Result<CheckedPush, Refusal> {
        // git's probe moves nothing; refused, git never sends the push it announces (#7878).
        if git::is_probe(body) {
            return Ok(CheckedPush::default());
        }
        let pushed = git::pushed_commands(body).map_err(|e| match e {
            PushError::Unreadable | PushError::PushCert => Refusal::BodyUnreadable,
        })?;
        if pushed.push_options {
            return Err(Refusal::PushOption);
        }
        let own = self
            .own_branches
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let advertised = self
            .advertised
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let known = advertised.get(&(repo.host.clone(), repo.path.clone()));
        let mut checked = CheckedPush::default();
        // Every command is examined before the push is classified: a push is refused as the most
        // severe class among its commands, never as the first one met, so a tag beside a delete
        // of `main` is a protected ref and not a "release or tag" that could be held and allowed.
        let mut worst: Option<Refusal> = None;
        for command in &pushed.commands {
            match self.check_command(repo, command, known, &own) {
                Ok(Some((key, fast_forward_only))) => {
                    if fast_forward_only {
                        checked.fast_forward_only.push(key.branch.clone());
                    }
                    checked.created.push(key);
                }
                Ok(None) => {}
                Err(refusal) => {
                    if worst.is_none_or(|w| push_severity(refusal) > push_severity(w)) {
                        worst = Some(refusal);
                    }
                }
            }
        }
        if let Some(refusal) = worst {
            return Err(refusal);
        }
        Ok(checked)
    }

    /// One push command against the repo: the branch it makes or moves (if it does), with
    /// whether its update must be a fast-forward, or why it is refused.
    fn check_command(
        &self,
        repo: &AllowedRepo,
        command: &git::RefCommand,
        known: Option<&HashSet<String>>,
        own: &HashMap<(String, String, String), String>,
    ) -> Result<Option<(CreatedBranch, bool)>, Refusal> {
        let name = &command.name;
        if name.starts_with("refs/tags/") {
            return Err(Refusal::ReleaseOrTag);
        }
        let Some(branch) = name.strip_prefix("refs/heads/") else {
            return Err(Refusal::ProtectedRef);
        };
        if branch.is_empty() || repo.protects(branch) || command.delete {
            return Err(Refusal::ProtectedRef);
        }
        if command.create {
            let known = known.ok_or(Refusal::NoAdvertisement)?;
            // Exact, and case-folded too: a `Fix` beside a colleague's `fix` is not made.
            if known.iter().any(|b| b.eq_ignore_ascii_case(branch)) {
                return Err(Refusal::ProtectedRef);
            }
            Ok(Some((
                CreatedBranch {
                    host: repo.host.clone(),
                    repo: repo.path.clone(),
                    repo_id: repo.project_id,
                    branch: branch.to_string(),
                    tip: command.new_id.clone(),
                    fast_forward_only: false,
                },
                false,
            )))
        } else {
            // **An update is let through only from the tip this chat last put on the branch**, read
            // now: a branch deleted and made again, pushed to by anyone else (another chat
            // through the route included), or never advertised to this chat has another tip.
            // Nothing here depends on an advertisement having been read.
            let tip = self.tip_in(own, repo, branch);
            match tip {
                Some(tip) if tip.tip.eq_ignore_ascii_case(&command.old_id) => Ok(Some((
                    CreatedBranch {
                        host: repo.host.clone(),
                        repo: repo.path.clone(),
                        repo_id: repo.project_id,
                        branch: branch.to_string(),
                        tip: command.new_id.clone(),
                        fast_forward_only: tip.fast_forward_only,
                    },
                    // Owned by the person's Allow: it was another's, so only forward (#7841).
                    tip.fast_forward_only,
                ))),
                // The chat owns it, but not at this tip: someone moved it since. Not the person's
                // to answer here; the chat fetches, or the person pushes.
                Some(_) => Err(Refusal::ProtectedRef),
                // No tip of this chat's: the branch has no owner here, or another's. Where the
                // app keeps the record (`tips`) and the repo has its forge id, the person can let
                // this chat push it ([`Self::unowned_updates`]).
                None if self.tips.is_some() && repo.project_id.is_some() => {
                    Err(Refusal::NotYourBranch)
                }
                None => Err(Refusal::ProtectedRef),
            }
        }
    }
}

impl ChatPolicy {
    /// The tip this chat last put on `branch`, from the record the push rule reads
    /// (`tips`, else the policy's own, which holds only branches the chat made).
    fn tip_in(
        &self,
        own: &HashMap<(String, String, String), String>,
        repo: &AllowedRepo,
        branch: &str,
    ) -> Option<OwnedTip> {
        match (&self.tips, repo.project_id) {
            (Some(tips), Some(repo_id)) => tips(&repo.host, repo_id, branch),
            (Some(_), None) => None,
            (None, _) => own
                .get(&(repo.host.clone(), repo.path.clone(), branch.to_string()))
                .map(|tip| OwnedTip {
                    tip: tip.clone(),
                    fast_forward_only: false,
                }),
        }
    }

    /// **Whether this chat may write to a pull request's head** (update-branch, a GitLab rebase;
    /// #7526), by the record the push rule uses: the head is a branch of `repo` itself (not a
    /// fork's), it is not protected, the chat has a tip of its own on it, and that tip is the one
    /// the forge reports as the head now. Any other branch is somebody else's, whatever the pull
    /// request's base says, and the head moving since the chat pushed is refused as the push is.
    /// **A head the chat holds by the person's Allow is not written to here**: a rebase replaces
    /// its history, and only a push's own objects show a fast-forward (#7841).
    fn owns_head(&self, repo: &AllowedRepo, head: &PullHead) -> bool {
        if !head.repo.eq_ignore_ascii_case(&repo.path) || repo.protects(&head.branch) {
            return false;
        }
        let own = self
            .own_branches
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        self.tip_in(&own, repo, &head.branch).is_some_and(|tip| {
            !tip.fast_forward_only
                && !head.oid.is_empty()
                && tip.tip.eq_ignore_ascii_case(&head.oid)
        })
    }
}

/// What [`ChatPolicy::check_push`] answers for a push it lets through.
#[derive(Debug, Default)]
struct CheckedPush {
    /// The branches it makes and moves.
    created: Vec<CreatedBranch>,
    /// Those of them whose update must be a fast-forward of the forge's tip.
    fast_forward_only: Vec<String>,
}

/// One branch of a push whose update the chat is refused for having no tip of its own on it
/// ([`Refusal::NotYourBranch`]): what the ask in the window names, and what its Allow records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotYours {
    pub host: String,
    /// The lower-case repo path.
    pub repo: String,
    /// The forge's numeric id of the repo: the record's key.
    pub repo_id: u64,
    pub branch: String,
}

impl ChatPolicy {
    /// **The branches a push is refused for** ([`Refusal::NotYourBranch`]): each update, in a
    /// push to one of the chat's repos, of a branch that is not protected and that the chat has no
    /// tip of its own on. Read the way [`Self::decide`] reads the push, from the request alone and
    /// nothing a chat can add: a protected or default branch, a deploy or pages branch, a delete,
    /// a tag, a branch the chat owns at another tip and a branch made are never in it, so none of
    /// them raises an ask. Each branch once.
    pub fn unowned_updates(&self, head: &RequestHead, body: &[u8]) -> Vec<NotYours> {
        let (Some(method), Ok(host), Ok(target), Some(tips)) = (
            request::method(head),
            request::host(head),
            request::parse_target(&head.target),
            &self.tips,
        ) else {
            return Vec::new();
        };
        let candidates: Vec<&AllowedRepo> = self
            .repos
            .iter()
            .filter(|r| r.serves(&host, head.port))
            .collect();
        let Some(first) = candidates.first() else {
            return Vec::new();
        };
        let kind = first.kind;
        let Some((repo_segments, GitOp::ReceivePack)) =
            git::classify(method, &target.segments, &target)
        else {
            return Vec::new();
        };
        let Ok(path) = normalise_repo_path(kind, &repo_segments.join("/")) else {
            return Vec::new();
        };
        let Some(repo) = candidates
            .iter()
            .filter(|r| r.kind == kind)
            .find(|r| r.path == path)
        else {
            return Vec::new();
        };
        let (Some(repo_id), Ok(pushed)) = (repo.project_id, git::pushed_commands(body)) else {
            return Vec::new();
        };
        let advertised = self
            .advertised
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // A branch the repo does not list is not one to ask about: an invented name raises
        // nothing, and the push is refused as it was.
        let Some(listed) = advertised.get(&(repo.host.clone(), repo.path.clone())) else {
            return Vec::new();
        };
        let mut out: Vec<NotYours> = Vec::new();
        for command in &pushed.commands {
            let Some(branch) = command.name.strip_prefix("refs/heads/") else {
                continue;
            };
            if command.create
                || command.delete
                || branch.is_empty()
                || repo.protects(branch)
                || !listed.contains(branch)
                || !is_askable_branch(branch)
                || tips(&repo.host, repo_id, branch).is_some()
                || out.iter().any(|one| one.branch == branch)
            {
                continue;
            }
            out.push(NotYours {
                host: repo.host.clone(),
                repo: repo.path.clone(),
                repo_id,
                branch: branch.to_owned(),
            });
        }
        out
    }
}

/// Whether a branch name a chat sent may be said in an ask: the base branch rule
/// ([`crate::mergehold::Base`]), and printable ASCII only, so a name cannot dress up the ask.
fn is_askable_branch(branch: &str) -> bool {
    crate::mergehold::Base::try_from(branch).is_ok() && branch.bytes().all(|b| b.is_ascii_graphic())
}

/// How severe a push command's refusal is; a push is refused as its most severe. A tag is the
/// least (an act the person could be asked about), a missing advertisement next, and any
/// protected, deleted or non-branch ref the most.
fn push_severity(refusal: Refusal) -> u8 {
    match refusal {
        // The only one the person may answer, so it is only ever the refusal of a push whose
        // every other command is fine: any other refusal in the push is the push's.
        Refusal::NotYourBranch => 0,
        Refusal::ReleaseOrTag => 1,
        Refusal::NoAdvertisement => 2,
        _ => 3,
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
        port: repo.port,
        repo_id: repo.project_id,
        repo: repo.path.clone(),
        class,
        upstream: upstream(repo, surface, segments, target),
        canonical_body: None,
        canonical_media: None,
        created: Vec::new(),
        fast_forward_only: Vec::new(),
        surface,
        touched: Vec::new(),
    }
}

/// A public read of a repo that is not the chat's, sent to the host of the chat's own repos.
fn public(host_of: &AllowedRepo, class: Class, segments: &[String], target: &Target) -> Allowed {
    Allowed {
        kind: host_of.kind,
        host: host_of.host.clone(),
        port: host_of.port,
        repo_id: None,
        repo: String::new(),
        class,
        upstream: upstream(host_of, Surface::Anonymous, segments, target),
        canonical_body: None,
        canonical_media: None,
        created: Vec::new(),
        fast_forward_only: Vec::new(),
        surface: Surface::Anonymous,
        touched: Vec::new(),
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
    // `render_html` is never sent on: where a route lets it by (GitLab's reads), the forge is not
    // asked to render references into the answer.
    let sent: Vec<&(String, String)> = target
        .query
        .iter()
        .filter(|(k, _)| !k.eq_ignore_ascii_case("render_html"))
        .collect();
    if !sent.is_empty() {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in sent {
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
        // api.github.com/repositories/{id}/…: the same repo by its numeric id, the form GitHub's
        // own `Link` header names the next page by (#7958). Read as `/repos/{owner}/{repo}/…`.
        (Kind::GitHub, ["repositories", id, ..]) if on_api_host => {
            Ok(Some((github_id(id)?, segments[2..].to_vec())))
        }
        (Kind::GitHub, _) if on_api_host => Err(Refusal::NotOnList),
        // GHES: {host}/api/v3/repos/{owner}/{repo}/…
        (Kind::GitHub, ["api", "v3", "repos", owner, repo, ..]) if host != "github.com" => {
            let path = github_path(owner, repo)?;
            Ok(Some((RestRepo::Path(path), segments[5..].to_vec())))
        }
        (Kind::GitHub, ["api", "v3", "repositories", id, ..]) if host != "github.com" => {
            Ok(Some((github_id(id)?, segments[4..].to_vec())))
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

/// A GitHub repo's numeric id as a REST path names it: digits only, in the one spelling GitHub
/// writes (no leading zero), else no repo of the chat's. It matches only the id the forge's own
/// read recorded ([`AllowedRepo::forge_id`]).
fn github_id(id: &str) -> Result<RestRepo, Refusal> {
    if id.is_empty() || id.starts_with('0') || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Refusal::OtherRepo);
    }
    id.parse::<u64>()
        .map(RestRepo::Id)
        .map_err(|_| Refusal::OtherRepo)
}

fn github_path(owner: &str, repo: &str) -> Result<String, Refusal> {
    // `.git` is not part of an API repo name: `repo.git` is another repo.
    if repo.to_ascii_lowercase().ends_with(".git") || owner.contains('/') || repo.contains('/') {
        return Err(Refusal::OtherRepo);
    }
    normalise_repo_path(Kind::GitHub, &format!("{owner}/{repo}")).map_err(|_| Refusal::OtherRepo)
}

/// The repos of one chat on one host, as [`crate::heldact::Recorded`].
pub(super) struct RecordedRepos(Vec<AllowedRepo>);

impl crate::heldact::Recorded for RecordedRepos {
    fn resolve(&self, kind: crate::forge::Kind, id: &str) -> Option<crate::mergehold::Repo> {
        let kind = match kind {
            crate::forge::Kind::GitHub => Kind::GitHub,
            crate::forge::Kind::GitLab => Kind::GitLab,
        };
        let wanted = id.to_ascii_lowercase();
        self.0
            .iter()
            .find(|r| {
                r.kind == kind
                    && (r.path == wanted
                        || (kind == Kind::GitLab
                            && id.parse::<u64>().is_ok_and(|n| r.project_id == Some(n))))
            })
            .and_then(|r| crate::mergehold::Repo::try_from(r.path.clone()).ok())
    }

    fn holds(&self, repo: &crate::mergehold::Repo) -> bool {
        self.0
            .iter()
            .any(|r| r.path.eq_ignore_ascii_case(repo.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::{is_ci_storage_host, is_gitlab_ci_storage};

    #[test]
    fn gitlab_com_artifact_storage_is_named_exactly() {
        for (host, path) in [
            (
                "storage.googleapis.com",
                "/gitlab-gprd-artifacts/ab/cd/x.zip",
            ),
            ("storage.googleapis.com", "/gitlab-gprd-artifacts/x"),
        ] {
            assert!(is_gitlab_ci_storage(host, path), "{host}{path}");
        }
        for (host, path) in [
            ("storage.googleapis.com", "/"),
            ("storage.googleapis.com", "/gitlab-gprd-artifacts"),
            ("storage.googleapis.com", "/gitlab-gprd-artifacts/"),
            ("storage.googleapis.com", "/gitlab-gprd-artifacts-evil/x"),
            ("storage.googleapis.com", "/attacker-bucket/x"),
            ("storage.googleapis.com", "/other/gitlab-gprd-artifacts/x"),
            ("gitlab-gprd-artifacts.storage.googleapis.com", "/x"),
            (
                "storage.googleapis.com.evil.example",
                "/gitlab-gprd-artifacts/x",
            ),
            ("evilstorage.googleapis.com", "/gitlab-gprd-artifacts/x"),
            (
                "pipelines.actions.githubusercontent.com",
                "/gitlab-gprd-artifacts/x",
            ),
        ] {
            assert!(!is_gitlab_ci_storage(host, path), "{host}{path}");
        }
    }

    #[test]
    fn actions_log_and_artifact_storage_is_named_exactly() {
        for host in [
            "pipelines.actions.githubusercontent.com",
            "results-receiver.actions.githubusercontent.com",
            "productionresultssa0.blob.core.windows.net",
            "productionresultssa19.blob.core.windows.net",
        ] {
            assert!(is_ci_storage_host(host), "{host}");
        }
        for host in [
            "actions.githubusercontent.com",
            "a.b.actions.githubusercontent.com",
            "-x.actions.githubusercontent.com",
            "token.actions.githubusercontent.com",
            "vstoken.actions.githubusercontent.com",
            "productionresultssa123456.blob.core.windows.net",
            "pipelines.actions.githubusercontent.com.evil.example",
            "evilactions.githubusercontent.com",
            "productionresultssa.blob.core.windows.net",
            "productionresultssa1a.blob.core.windows.net",
            "attacker.blob.core.windows.net",
            "blob.core.windows.net",
            "productionresultssa1.blob.core.windows.net.evil.example",
            "raw.githubusercontent.com",
        ] {
            assert!(!is_ci_storage_host(host), "{host}");
        }
    }
}
