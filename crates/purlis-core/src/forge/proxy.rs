// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The forge proxy's core** (one login, slice A; ADR 0128 D2): the part that decides, per chat,
//! whether a request to a forge may be answered, and swaps the chat's stand-in for the person's
//! credential only after it may.
//!
//! A chat holds no forge token. It holds a **sentinel** ([`sentinel`]): a per-chat random string
//! that is worth nothing anywhere else. Every request it makes to its forge reaches this core on an
//! **endpoint made for that chat alone** ([`endpoint`]), and [`ForgeProxy::handle`] then:
//!
//! 1. refuses a request that carries no sentinel, another chat's, or anything but this chat's;
//! 2. decides it against the chat's own repos ([`policy`]): git smart-HTTP and the forge's API,
//!    **an allowlist of writes** that refuses merges, releases and tags, rulesets, repository
//!    administration and every other repo, whatever the token could do;
//! 3. only then asks the [`CredentialSource`] for the real credential and puts it in, after
//!    removing every credential header the chat sent;
//! 4. records the answer in the chat's audit ([`audit`]): method, host, class and verdict, never
//!    the token and never a raw path.
//!
//! **Who is asking is the endpoint's to say, never the request's.** The chat is the one the
//! endpoint was bound for; no header, port or id in a request chooses it ([`ForgeProxy::handle`]
//! takes the chat from the endpoint, and a request has no field that could name another).
//!
//! **Fail closed** wherever the design is silent: a path that does not normalise, a host that is
//! not exactly an allowed one, a GraphQL document that does not parse, a mutation not on the list,
//! an object id the proxy cannot tie to an allowed repo, a push whose ref commands cannot be read,
//! a body that is too large, and a credential that is not there are all refusals.
//!
//! **Tier** (ADR 0069): every piece of state here is memory, held by one running app and gone with
//! it: the sentinels, the policies and the audit. Nothing is written to the plane, a clone or the
//! machine, so no store is added to `docs/plane-format.md`. The credential is the
//! [`CredentialSource`]'s: slice C's, in the keyring.
//!
//! **Not here** (later slices): the listener's connection loop and its wiring into the chat's
//! sandbox, the credential sources, and how `gh` and `glab` reach an endpoint.
//!
//! **What the listener must do with the answer** ([`Forwarded`]): send the request to
//! [`Forwarded::upstream`] over TLS, one request per upstream connection, with the headers given
//! (the chat's `Host` and routing headers are gone; the upstream's own is set from the address);
//! **never follow a redirect with the credential** (follow one only when
//! [`Forwarded::credentialed`] is false, or drop the credential first); rewrite `Link` headers in
//! the answer to the canonical form; and stream bodies, reading first what
//! [`ForgeProxy::body_need`] asks for. This core decides on a request's line, its headers and that
//! prefix of its body. It runs in the app, which holds the keyring; a proxy held elsewhere
//! forwards forge calls to the app's, so this is never run where a chat could read its memory.
//!
//! **What the listener slice owes, from the final rulings** (this core gives it the decisions, not
//! the TLS): the per-chat CA lives in the app's memory only, and carries a *critical*
//! `nameConstraints` permitting exactly [`ChatPolicy::terminable_hosts`]; a leaf is made only for a
//! host [`ChatPolicy::connect`] says [`ConnectVerdict::Terminate`] for, and a
//! [`ConnectVerdict::Tunnel`] host is a plain tunnel with its own certificate. The client the proxy
//! uses upstream checks the chain and the host name strictly, falls back to nothing, ignores the
//! chat's environment and never trusts a per-chat CA. A unix socket is in a folder of mode 0700,
//! mode 0600, owned by the app ([`UnixEndpoint`]); where the platform gives peer credentials they
//! are checked too, and the sentinel always is.
//!
//! **Identity.** The chat is the endpoint's: with ADR 0067's per-chat egress proxy, the route for
//! forge hosts is this core keyed by that proxy's instance, and the secret arrives as
//! `Proxy-Authorization` (a `Basic` pair's password) or in any credential header, all of which
//! must be the sentinel, ten wrong ones in a row locking the chat out.

mod audit;
mod commands;
mod endpoint;
mod git;
mod graphql;
mod held;
#[cfg(feature = "forge-wiring")]
pub mod listener;
mod policy;
mod repo;
mod request;
mod rest;
mod schema;
mod sentinel;

pub use audit::{AuditLog, AuditRecord, Verdict};
pub use commands::has_merge_command;
#[cfg(unix)]
pub use endpoint::UnixEndpoint;
pub use endpoint::{ChatEndpoint, EndpointKind, LoopbackEndpoint, pick_endpoint};
pub use graphql::mutation_repo;
pub use held::{Held, HeldSink};
pub use policy::{
    Allowed, BodyNeed, ChatPolicy, Class, ConnectVerdict, Decision, GITLAB_COM, GithubAlias,
    NoObjects, ObjectRepo, RecordedBranches, Refusal, TipSource, TouchedRepo, is_ci_storage_host,
    is_download_host, is_gitlab_ci_storage,
};
pub use repo::{AddressError, AllowedRepo, ForgeRepoId, Kind};
pub use request::RequestHead;
use request::host as policy_host;
pub use sentinel::{ChatId, Sentinel, SentinelRegistry};

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use crate::enforcement::Mode;
use crate::forge::createdbranches::CreatedBranch;
use crate::forge::{mergestance, pin};
use crate::secrets::keyring::Secret;

/// What a credential is, which decides the header the forge reads it from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// An OAuth access token (the person's sign-in).
    Oauth,
    /// A personal access token (a token connection).
    Pat,
}

/// What the proxy asks a [`CredentialSource`] for: the person's credential on one host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialRequest {
    pub kind: Kind,
    pub host: String,
    /// The port the request goes to: a token confirmed for `host:443` is not for `host:8443`.
    pub port: u16,
    /// The forge-read numeric id of the repo (the recorded [`AllowedRepo`]'s, never the request's
    /// own words), or `None` outside a repo. A source that serves by id refuses without it.
    pub repo_id: Option<u64>,
    /// The chat the request is for: the endpoint's, never the request's own. A source that
    /// serves only what the person confirmed answers for this chat's clones alone.
    pub chat: ChatId,
    /// The allowed repo the request is for (`owner/repo`), or empty for the few credentialed
    /// reads outside a repo.
    pub repo: String,
}

/// The real credential, held for one request.
#[derive(Debug)]
pub struct Credential {
    pub token_kind: TokenKind,
    pub secret: Secret,
    /// The id of the account the secret belongs to, or `None` for a source with one credential
    /// per host and no accounts (a token connection). A request that touches several repos is
    /// sent only when every repo is served by the same account: compared by this, never by the
    /// token's bytes, which a renewal between two lookups changes.
    pub account: Option<String>,
}

/// Why a credential could not be had. Either way the request is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    /// The person has no credential on this host, or it is not readable now.
    Unavailable,
}

/// Where the real credential comes from. Slice C implements it (the keyring's `purlis/@forge/…`
/// items, ADR 0128 D3); it reads what is stored and never refreshes.
pub trait CredentialSource: Send + Sync {
    fn credential_for(&self, request: &CredentialRequest) -> Result<Credential, CredentialError>;
}

/// The stub until slice C lands: nobody has a credential, so every allowed request is refused
/// with [`Refusal::NoCredential`].
#[derive(Debug, Default, Clone, Copy)]
pub struct NoCredentials;

impl CredentialSource for NoCredentials {
    fn credential_for(&self, _: &CredentialRequest) -> Result<Credential, CredentialError> {
        Err(CredentialError::Unavailable)
    }
}

/// The person's credential as slice C holds it ([`crate::forge::credential`]: the browser sign-in,
/// renewed, or a token connection), given to the proxy as the one it asks for. It asks the source
/// for each request, so a renewal that happened since is the token that is used.
///
/// A credential that is not the host's forge, or a CI job's GitLab token (which the forge reads
/// from a header the proxy does not set), is no credential here: the request is refused.
pub struct BrokeredCredentials {
    source: Arc<dyn crate::forge::credential::CredentialSource>,
}

impl BrokeredCredentials {
    pub fn new(source: Arc<dyn crate::forge::credential::CredentialSource>) -> Self {
        Self { source }
    }
}

impl CredentialSource for BrokeredCredentials {
    fn credential_for(&self, request: &CredentialRequest) -> Result<Credential, CredentialError> {
        use crate::forge::credential::TokenKind as Held;
        let held = self
            .source
            .credential_for(&request.host)
            .map_err(|_| CredentialError::Unavailable)?;
        let same_forge = matches!(
            (held.kind(), request.kind),
            (crate::forge::Kind::GitHub, Kind::GitHub) | (crate::forge::Kind::GitLab, Kind::GitLab)
        );
        let token_kind = match held.token_kind() {
            Held::SignIn | Held::GitHubUser => TokenKind::Oauth,
            Held::GitLabJob => return Err(CredentialError::Unavailable),
            Held::Pasted
            | Held::GitHubInstallation
            | Held::GitHubFineGrained
            | Held::GitLabAccess
            | Held::Unrecognised => TokenKind::Pat,
        };
        if !same_forge {
            return Err(CredentialError::Unavailable);
        }
        Ok(Credential {
            token_kind,
            secret: Secret::new(held.expose().to_string()),
            account: None,
        })
    }
}

/// One request as a chat's endpoint read it.
#[derive(Debug)]
pub struct Inbound<'a> {
    pub head: &'a RequestHead,
    /// The request's headers, in order, as sent.
    pub headers: &'a [(String, String)],
    /// The body, or the prefix [`ForgeProxy::body_need`] asked for.
    pub body: &'a [u8],
}

/// What [`ForgeProxy::handle`] answered.
pub enum Outcome {
    /// Send this to the forge.
    Forward(Forwarded),
    /// Answer the chat with a refusal, and send nothing.
    Refused(Refusal),
    /// A held act: answer the chat with this sentence, and send nothing. An ask was raised (or
    /// the sentence says why not), and the person's press in Asks is what performs it.
    Held(Held),
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Forward(forwarded) => forwarded.fmt(f),
            Self::Refused(refusal) => f.debug_tuple("Refused").field(refusal).finish(),
            Self::Held(held) => f.debug_tuple("Held").field(held).finish(),
        }
    }
}

/// An allowed request, ready for the forge: the chat's own headers without any credential, and the
/// real one in.
pub struct Forwarded {
    /// Where to send it: the proxy's own address for the request, never the chat's.
    pub upstream: String,
    pub headers: Vec<(String, String)>,
    pub class: Class,
    /// Whether `headers` carry the person's credential. A public read has none.
    pub credentialed: bool,
    /// The body to send in place of the chat's, when the chat's was read: the canonical
    /// re-serialisation of what was checked. `headers` already carry its `Content-Length`.
    pub body: Option<Vec<u8>>,
    /// Branches a push makes or moves, for [`ForgeProxy::confirm_push`].
    pub(crate) created: Vec<CreatedBranch>,
    /// What the answer must never hold: the real credential, as it is sent and in the other forms
    /// a forge or a proxy may give it back. The listener reads the answer for these; memory only,
    /// and zeroed with the request.
    needles: Vec<zeroize::Zeroizing<Vec<u8>>>,
    /// The host the request is for, and the allowed repo (empty for a public one): what the
    /// listener tells the core of a push's advertisement and its outcome.
    pub host: String,
    pub repo: String,
    /// Which forge's rules the request was decided under; `None` for a fetch the listener makes
    /// for itself (an LFS object). A redirect is followed by the rules of this one.
    pub kind: Option<Kind>,
}

impl Forwarded {
    /// A fetch of an LFS object the listener holds the address of: no credential of the person's,
    /// no header but the ones the listener adds, and nothing to look for in the answer.
    #[cfg(feature = "forge-wiring")]
    pub(crate) fn for_storage(host: &str, url: &str) -> Forwarded {
        Forwarded {
            upstream: url.to_string(),
            headers: Vec::new(),
            class: Class::PublicRead,
            credentialed: false,
            body: None,
            created: Vec::new(),
            needles: Vec::new(),
            host: host.to_string(),
            repo: String::new(),
            kind: None,
        }
    }

    /// The request that follows a redirect of this one, to `url`: **no credential, no header and
    /// no body**, and the same needles, since what the storage host answers is read too.
    #[cfg(feature = "forge-wiring")]
    pub(crate) fn anonymous_follow(&self, url: &str) -> Forwarded {
        Forwarded {
            upstream: url.to_string(),
            headers: Vec::new(),
            class: self.class.clone(),
            credentialed: false,
            body: None,
            created: Vec::new(),
            needles: self
                .needles
                .iter()
                .map(|n| zeroize::Zeroizing::new(n.to_vec()))
                .collect(),
            host: self.host.clone(),
            repo: String::new(),
            kind: self.kind,
        }
    }

    /// The byte strings the answer to this request may not contain.
    pub fn leak_needles(&self) -> impl Iterator<Item = &[u8]> {
        self.needles.iter().map(|n| n.as_slice())
    }
}

impl std::fmt::Debug for Forwarded {
    /// Names only: a header's value may be the real credential.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("Forwarded")
            .field("upstream", &self.upstream)
            .field("header_names", &names)
            .field("class", &self.class)
            .field("credentialed", &self.credentialed)
            .field("body_bytes", &self.body.as_ref().map(Vec::len))
            .finish()
    }
}

/// The proxy's core: every chat's policy and sentinel, the credential source and the audit.
pub struct ForgeProxy {
    policies: Mutex<HashMap<ChatId, Arc<ChatPolicy>>>,
    sentinels: SentinelRegistry,
    credentials: Box<dyn CredentialSource>,
    objects: Box<dyn ObjectRepo>,
    audit: AuditLog,
    held: Option<Box<dyn HeldSink>>,
    /// The administrator's never-allow pin, asked on every request and connection (D3g).
    pin: Arc<dyn pin::Pin>,
    /// How `guards.merge` runs, asked on every merge a chat sends (ADR 0128 D8, 2026-11-22).
    merges: Arc<dyn mergestance::MergeStance>,
    /// Told the branches the forge confirmed a push made: the app's own record of them.
    created: Option<CreatedSink>,
    gone: Option<GoneSink>,
    /// Where each chat's policy reads the tip it last put on a branch, at every update.
    tips: Option<TipSource>,
    /// Where each chat's policy lists the branches the record holds for it, when an advertisement
    /// is noted.
    recorded: Option<RecordedBranches>,
}

/// What is told the branches, and the tips, a confirmed push made or moved.
pub type CreatedSink = Box<dyn Fn(&[CreatedBranch]) + Send + Sync>;

/// Told which recorded branches the forge no longer lists ([`ForgeProxy::note_advertised`]), so
/// their record ends ([`crate::forge::createdbranches::forget`]).
pub type GoneSink = Box<dyn Fn(&[CreatedBranch]) + Send + Sync>;

impl ForgeProxy {
    pub fn new(credentials: Box<dyn CredentialSource>, objects: Box<dyn ObjectRepo>) -> Self {
        Self {
            policies: Mutex::new(HashMap::new()),
            sentinels: SentinelRegistry::default(),
            credentials,
            objects,
            audit: AuditLog::default(),
            held: None,
            pin: Arc::new(pin::NoPin),
            merges: Arc::new(mergestance::HoldMerges),
            created: None,
            gone: None,
            tips: None,
            recorded: None,
        }
    }

    /// Let a merge through where `merges` says `guards.merge` is `log` (and record it) or `off`,
    /// asked on each merge and never kept. Without it, every merge is a held act. Only a recorded
    /// repo of the chat's own run and a valid sentinel ever reach it, and an administrator's pin
    /// and lock still win.
    #[must_use]
    pub fn with_merge_stance(mut self, merges: Arc<dyn mergestance::MergeStance>) -> Self {
        self.merges = merges;
        self
    }

    /// Tell `sink` each branch the forge confirms a chat's push made or moved, so the app can keep its own
    /// record of what the chat created ([`crate::forge::createdbranches`]).
    #[must_use]
    pub fn with_created(mut self, sink: CreatedSink) -> Self {
        self.created = Some(sink);
        self
    }

    /// Where each update's old id is checked: the tip the chat last put on the branch, asked of
    /// `tips` at every update (the app's record, which every chat's confirmed push moves).
    /// Without it a policy checks against what it saw itself.
    #[must_use]
    pub fn with_tips(mut self, tips: TipSource) -> Self {
        self.tips = Some(tips);
        self
    }

    /// Where each chat's policy lists the branches the record holds for it, asked when an
    /// advertisement is noted, so a recorded branch gone from the forge ends its record.
    #[must_use]
    pub fn with_recorded(mut self, recorded: RecordedBranches) -> Self {
        self.recorded = Some(recorded);
        self
    }

    /// Where a recorded branch the forge no longer lists is told, so its record ends.
    #[must_use]
    pub fn with_gone(mut self, sink: GoneSink) -> Self {
        self.gone = Some(sink);
        self
    }

    /// Refuse every request, connection and alias that names a forge `pin` pins as never allowed
    /// ([`pin::pinned_forge`]), whatever the chat's repos, asked each time and never kept.
    #[must_use]
    pub fn with_pin(mut self, pin: Arc<dyn pin::Pin>) -> Self {
        self.pin = pin;
        self
    }

    /// The sentence for a pin refusal of `host:port`.
    pub fn pin_said(&self, host: &str, port: u16) -> String {
        pin::pinned_forge(self.pin.as_ref(), host, port).unwrap_or_else(|| {
            "An administrator's policy pins this forge as never allowed, and signing in does \
             not change that."
                .to_owned()
        })
    }

    /// The sentence for a pin refusal of the alias `host` (`github.localhost`) of `chat`: the
    /// forge it leads to is the one named.
    pub fn alias_pin_said(&self, chat: &ChatId, host: &str) -> String {
        let policy = self
            .policies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(chat)
            .cloned();
        match policy.and_then(|policy| policy.github_alias(host)) {
            Some(Ok(alias)) => self.pin_said(&alias.host, alias.port),
            _ => self.pin_said(host, 443),
        }
    }

    fn pinned(&self, host: &str, port: u16) -> bool {
        pin::pinned_forge(self.pin.as_ref(), host, port).is_some()
    }

    /// Hand the acts that are held (a merge, a tag, a release, a fork) to `sink`, which asks the
    /// person, instead of refusing them dead. Without one they are refused, as before.
    #[must_use]
    pub fn with_held(mut self, sink: Box<dyn HeldSink>) -> Self {
        self.held = Some(sink);
        self
    }

    /// The listener read a push that changes CI files (from its objects, on a quarantined tree):
    /// hand it over. What the chat is told, or `None` where nothing is held here.
    pub fn hand_over(
        &self,
        chat: &ChatId,
        requested: &crate::heldact::Requested,
    ) -> Option<String> {
        Some(self.held.as_ref()?.ask(chat, requested))
    }

    /// A chat starts: its repos are the ones the person confirmed, and it gets a fresh sentinel
    /// (any it had is replaced). The sentinel is for the chat's environment; it is the only thing
    /// of the credential a chat ever holds.
    ///
    /// **The caller enrols no repo on a pinned forge** (the app's `forgeroute` filters them, D3g).
    /// Whatever it enrols, the pin is read on every request and connection all the same, so a pin
    /// set later reaches the chat at once.
    pub fn enrol(&self, chat: &ChatId, repos: Vec<AllowedRepo>) -> Sentinel {
        let mut policy = ChatPolicy::new(repos);
        if let Some(tips) = &self.tips {
            policy = policy.with_tips(tips.clone());
        }
        if let Some(recorded) = &self.recorded {
            policy = policy.with_recorded(recorded.clone());
        }
        self.policies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(chat.clone(), Arc::new(policy));
        self.sentinels.issue(chat)
    }

    /// How long a chat that guessed wrong ten times in a row is locked out. A minute by default
    /// (ADR 0128), and a chat that starts again is not locked at all.
    #[must_use]
    pub fn with_lockout(mut self, lockout: std::time::Duration) -> Self {
        self.sentinels = SentinelRegistry::with_lockout(lockout);
        self
    }

    /// The listener read the ref advertisement of a push to `repo_path` on `host`: these are its
    /// branches. A push that makes a branch is decided against the latest one, and is refused if
    /// none was given or the branch is in it.
    pub fn note_advertised(
        &self,
        chat: &ChatId,
        host: &str,
        repo_path: &str,
        branches: Vec<String>,
    ) {
        let policy = self
            .policies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(chat)
            .cloned();
        if let Some(policy) = policy {
            let gone = policy.note_advertised(host, repo_path, branches);
            if let Some(sink) = &self.gone
                && !gone.is_empty()
            {
                sink(&gone);
            }
        }
    }

    /// The forge's report of a push `forwarded` says it made or moved its branches: only now are
    /// they the chat's own to move, and their tip the one the next update must name. A push the forge refused, or one never sent, makes no branch the chat
    /// owns, so a branch cannot be claimed by asking for it.
    pub fn confirm_push(&self, chat: &ChatId, forwarded: &Forwarded) {
        let policy = self
            .policies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(chat)
            .cloned();
        if let Some(policy) = policy {
            policy.confirm_created(&forwarded.created);
            if let Some(sink) = &self.created
                && !forwarded.created.is_empty()
            {
                sink(&forwarded.created);
            }
        }
    }

    /// A chat ends: its sentinel stops working at once, and its policy goes. Its audit stays.
    pub fn retire(&self, chat: &ChatId) {
        self.policies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(chat);
        self.sentinels.revoke(chat);
    }

    /// Where `github.localhost` leads for `chat` (`gh`'s `GH_HOST`, on its unix socket): the one
    /// GitHub host its repos are on, and a refusal when it has two or none. `None` for any other
    /// name. The listener rewrites the request to that host before it asks [`Self::handle`], so
    /// the policy only ever sees a recorded host.
    pub fn github_alias(&self, chat: &ChatId, host: &str) -> Option<Result<GithubAlias, Refusal>> {
        let policy = self
            .policies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(chat)
            .cloned();
        match policy {
            // The pin is read on this connection, for the host the alias leads to.
            Some(policy) => match policy.github_alias(host)? {
                Ok(alias) if self.pinned(&alias.host, alias.port) => Some(Err(Refusal::Pinned)),
                other => Some(other),
            },
            None => Some(Err(Refusal::UnknownChat)),
        }
    }

    /// Every host a leaf certificate may be made for, for `chat`: the chat's per-chat CA is
    /// constrained to exactly these names.
    pub fn terminable_hosts(&self, chat: &ChatId) -> Vec<String> {
        self.policies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(chat)
            .map(|policy| policy.terminable_hosts())
            .unwrap_or_default()
    }

    /// Decide a `CONNECT host:port` that arrived on `chat`'s endpoint, and record it: a host of the
    /// chat's repos is terminated, a download-only host is tunnelled, anything else is refused.
    pub fn connect(&self, chat: &ChatId, host: &str, port: u16) -> ConnectVerdict {
        let policy = self
            .policies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(chat)
            .cloned();
        let verdict = match policy {
            _ if self.pinned(host, port) => ConnectVerdict::Refuse(Refusal::Pinned),
            Some(policy) => policy.connect(host, port),
            None => ConnectVerdict::Refuse(Refusal::UnknownChat),
        };
        let head = RequestHead::new("CONNECT", host, port, "/");
        let recorded = match &verdict {
            ConnectVerdict::Refuse(refusal) => Verdict::Refused(*refusal),
            _ => Verdict::Allowed,
        };
        self.audit.record(chat, &head, None, recorded);
        verdict
    }

    /// Record a request the listener answered itself, from an address the forge handed it.
    pub fn note_allowed(&self, chat: &ChatId, head: &RequestHead) {
        self.audit.record(chat, head, None, Verdict::Allowed);
    }

    /// Record a request the listener refused before the core could read it: a name it maps
    /// (`github.localhost`) that has no one host to map to.
    pub fn note_refused(&self, chat: &ChatId, head: &RequestHead, refusal: Refusal) {
        self.audit
            .record(chat, head, None, Verdict::Refused(refusal));
    }

    /// What of the body the listener must read before [`Self::handle`] can decide.
    pub fn body_need(&self, head: &RequestHead) -> BodyNeed {
        policy::body_need(head)
    }

    /// The chat's audit, oldest first.
    pub fn audit(&self, chat: &ChatId) -> Vec<AuditRecord> {
        self.audit.for_chat(chat)
    }

    /// Decide one request that arrived on `chat`'s endpoint, and, only if it is allowed, give back
    /// what to send to the forge.
    pub fn handle(&self, chat: &ChatId, inbound: &Inbound<'_>) -> Outcome {
        let policy = self
            .policies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(chat)
            .cloned();
        let Some(policy) = policy else {
            return self.refuse(chat, inbound, None, Refusal::UnknownChat);
        };
        // An administrator's pin wins over everything the chat's repos allow (D3g): read for this
        // request, for the name asked and for the forge it belongs to, before any credential is.
        if self.pinned(&inbound.head.host, inbound.head.port) {
            return self.refuse(chat, inbound, None, Refusal::Pinned);
        }
        // Credentialed means a recorded repo and a valid sentinel. A request without one may still
        // take the anonymous route, which carries no authorization of any kind; a chat that is
        // locked out, or unknown, takes none.
        let sentinel = self.sentinels.check(chat, inbound.headers);
        if let Err(refusal @ (Refusal::Locked | Refusal::UnknownChat)) = sentinel {
            return self.refuse(chat, inbound, None, refusal);
        }
        if !framing_is_unambiguous(inbound.headers)
            || !header_names_are_plain(inbound.headers)
            || !host_agrees(inbound)
        {
            return self.refuse(chat, inbound, None, Refusal::MalformedRequest);
        }
        let head = inbound
            .head
            .clone()
            .with_content_type(header_value(inbound.headers, "content-type"));
        // `guards.merge`, asked now: a merge is let through under `log` or `off` for a chat that
        // proved itself, by the same recorded-repo checks as any write. Under `enforce`, and for
        // anything that is not one of the merge routes, nothing changes.
        //
        // **No lookup before the sentinel is proved**: resolving a node id asks the forge with the
        // person's credential, so a caller that has not proved itself is decided with
        // [`NoObjects`] and costs the forge, and the person, nothing. It is refused the same way
        // whatever the id is in.
        let objects: &dyn ObjectRepo = if sentinel.is_ok() {
            self.objects.as_ref()
        } else {
            &NoObjects
        };
        let mut logged_merge = false;
        let mut decision = policy.decide(&head, inbound.body, objects);
        if sentinel.is_ok()
            && decision == Decision::Refuse(Refusal::Merge)
            && let mode @ (Mode::Log | Mode::Off) = self.merges.mode()
        {
            let merging = policy.decide_merging(&head, inbound.body, objects);
            if matches!(merging, Decision::Allow(_)) {
                logged_merge = mode == Mode::Log;
                decision = merging;
            }
        }
        let mut allowed = match decision {
            Decision::Allow(allowed) => allowed,
            Decision::Refuse(refusal) => {
                // Only a chat that proved itself asks, and only for a held route.
                if sentinel.is_ok()
                    && held::is_a_held_route(refusal)
                    && let Some(told) = self.held_answer(chat, &policy, &head, inbound.body)
                {
                    self.audit
                        .record(chat, inbound.head, None, Verdict::Refused(refusal));
                    return Outcome::Held(Held {
                        refusal,
                        said: told,
                    });
                }
                return self.refuse(chat, inbound, None, refusal);
            }
        };
        // A read goes upstream with no body, whatever the chat sent and whatever its headers say
        // of one: a body on a `GET` is read by a forge or a cache in front of it as it likes.
        if request::method(inbound.head).is_some_and(request::is_read) {
            allowed.canonical_body = Some(Vec::new());
            allowed.canonical_media = None;
        }
        if let (true, Err(refusal)) = (allowed.credentialed(), sentinel) {
            return self.refuse(chat, inbound, None, refusal);
        }
        // A request body arrives encoded only for a fetch (git compresses its own request); on
        // anything else it is a body that was not read as it is sent.
        if has_header(inbound.headers, "content-encoding") && allowed.class != Class::GitFetch {
            return self.refuse(chat, inbound, None, Refusal::MalformedRequest);
        }
        if !allowed.credentialed() {
            // A public read: the chat's own headers without any credential, and none added.
            self.audit
                .record(chat, inbound.head, Some(&allowed), Verdict::Allowed);
            let mut headers = without_credentials(inbound.headers);
            apply_canonical(&mut headers, &allowed);
            return Outcome::Forward(Forwarded {
                upstream: allowed.upstream,
                headers,
                class: allowed.class,
                credentialed: false,
                body: allowed.canonical_body,
                created: Vec::new(),
                needles: Vec::new(),
                host: allowed.host,
                repo: allowed.repo,
                kind: Some(allowed.kind),
            });
        }
        let request = |repo_id: Option<u64>, repo: &str| CredentialRequest {
            kind: allowed.kind,
            host: allowed.host.clone(),
            port: allowed.port,
            repo_id,
            chat: chat.clone(),
            repo: repo.to_string(),
        };
        // A GraphQL document may name several of the chat's repos: each must be served, and by the
        // same account (the same token), or the request is refused. Anything else is one request.
        let credential = if allowed.touched.is_empty() {
            self.credentials
                .credential_for(&request(allowed.repo_id, &allowed.repo))
        } else {
            let mut first: Option<Credential> = None;
            let mut outcome = Ok(());
            for repo in &allowed.touched {
                match self
                    .credentials
                    .credential_for(&request(repo.repo_id, &repo.repo))
                {
                    Ok(next) => match &first {
                        None => first = Some(next),
                        // Both name an account, and the same one. A source that names none says
                        // nothing about who it serves, so two such answers are **not** the same
                        // account (`None == None` must not pass for a future source).
                        Some(held) if held.account.is_some() && held.account == next.account => {}
                        Some(_) => outcome = Err(CredentialError::Unavailable),
                    },
                    Err(why) => outcome = Err(why),
                }
            }
            outcome.and_then(|()| first.ok_or(CredentialError::Unavailable))
        };
        let Ok(credential) = credential else {
            return self.refuse(chat, inbound, Some(&allowed), Refusal::NoCredential);
        };
        let Some((mut headers, needles)) = swap_credential(inbound.headers, &allowed, credential)
        else {
            return self.refuse(chat, inbound, Some(&allowed), Refusal::NoCredential);
        };
        // One quiet line for a merge `log` let through, once the credential is in and it is sent.
        if logged_merge {
            self.merges
                .forwarded(chat.as_str(), &allowed.host, &allowed.repo);
        }
        self.audit
            .record(chat, inbound.head, Some(&allowed), Verdict::Allowed);
        apply_canonical(&mut headers, &allowed);
        Outcome::Forward(Forwarded {
            upstream: allowed.upstream,
            headers,
            class: allowed.class,
            credentialed: true,
            body: allowed.canonical_body,
            created: allowed.created,
            needles,
            host: allowed.host,
            repo: allowed.repo,
            kind: Some(allowed.kind),
        })
    }

    /// What a chat is told for a request that is a held act or one refused whole.
    fn held_answer(
        &self,
        chat: &ChatId,
        policy: &ChatPolicy,
        head: &RequestHead,
        body: &[u8],
    ) -> Option<String> {
        use crate::heldact::{Recognised, said};
        let sink = self.held.as_ref()?;
        match policy.held_reading(head, body)? {
            Recognised::Hold(requested) => Some(sink.ask(chat, &requested)),
            Recognised::Refuse(why) => Some(said::refused_request(&why)),
            Recognised::Other => None,
        }
    }

    fn refuse(
        &self,
        chat: &ChatId,
        inbound: &Inbound<'_>,
        allowed: Option<&Allowed>,
        refusal: Refusal,
    ) -> Outcome {
        self.audit
            .record(chat, inbound.head, allowed, Verdict::Refused(refusal));
        Outcome::Refused(refusal)
    }
}

/// A header's name as the forge's front end reads it: lower case, with `_` taken as `-`, since a
/// server that treats the two alike makes `private_token` the same header as `private-token`.
pub(crate) fn normal(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "-")
}

/// Every header a forge or a proxy reads a credential from, or that carries a session: removed
/// from what a chat sent, whatever it held.
pub(crate) fn is_credential_header(name: &str) -> bool {
    const NAMES: &[&str] = &[
        "authorization",
        "proxy-authorization",
        "private-token",
        "job-token",
        "deploy-token",
        "x-github-token",
        "x-gitlab-token",
        "cookie",
    ];
    NAMES.contains(&normal(name).as_str())
}

fn has_header(headers: &[(String, String)], wanted: &str) -> bool {
    headers.iter().any(|(n, _)| normal(n) == wanted)
}

fn header_value<'a>(headers: &'a [(String, String)], wanted: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| normal(n) == wanted)
        .map(|(_, v)| v.as_str())
}

/// When a body was read, it is sent in the chat's place with exactly one `Content-Type` (the type
/// it was read by) and a `Content-Length` of its own: none of the chat's variants of those headers
/// (case, `_` for `-`), its `Transfer-Encoding` or its `Content-Encoding` goes on.
fn apply_canonical(headers: &mut Vec<(String, String)>, allowed: &Allowed) {
    let Some(body) = &allowed.canonical_body else {
        return;
    };
    headers.retain(|(n, _)| {
        ![
            "content-length",
            "transfer-encoding",
            "content-encoding",
            "content-type",
        ]
        .contains(&normal(n).as_str())
    });
    if !body.is_empty() {
        let media = allowed
            .canonical_media
            .clone()
            .unwrap_or_else(|| "application/json".to_string());
        headers.push(("Content-Type".to_string(), media));
    }
    headers.push(("Content-Length".to_string(), body.len().to_string()));
}

/// A header name is a token, and a forge's front end may read `Content_Type` as `Content-Type`
/// while this reads it as another header: a name with an underscore is refused, as is a second
/// `Content-Type` of any spelling (two readers pick two types).
pub(crate) fn header_names_are_plain(headers: &[(String, String)]) -> bool {
    headers
        .iter()
        .all(|(n, _)| !n.is_empty() && !n.contains('_'))
        && headers
            .iter()
            .filter(|(n, _)| normal(n) == "content-type")
            .count()
            <= 1
}

/// A `Host` the request line does not agree with is a request for two places: refused. Without a
/// `Host` there is nothing to disagree with, and the address is the line's alone.
fn host_agrees(inbound: &Inbound<'_>) -> bool {
    let hosts: Vec<&str> = inbound
        .headers
        .iter()
        .filter(|(n, _)| normal(n) == "host")
        .map(|(_, v)| v.trim())
        .collect();
    if hosts.is_empty() {
        return true;
    }
    let Ok(want) = request_host(inbound.head) else {
        return false;
    };
    hosts.iter().all(|value| {
        let (host, port) = match value.rsplit_once(':') {
            Some((h, p)) => (h, p.parse::<u16>().ok()),
            None => (*value, Some(443)),
        };
        port == Some(inbound.head.port)
            && request_host(&RequestHead::new("GET", host, 0, "/")).is_ok_and(|h| h == want)
    })
}

fn request_host(head: &RequestHead) -> Result<String, ()> {
    policy_host(head).map_err(|_| ())
}

/// The chat's headers with the credential headers removed and the real credential set. `None` when
/// the secret would break a header (a line break in it).
#[allow(clippy::type_complexity)]
fn swap_credential(
    sent: &[(String, String)],
    allowed: &Allowed,
    credential: Credential,
) -> Option<(Vec<(String, String)>, Vec<zeroize::Zeroizing<Vec<u8>>>)> {
    let secret = zeroize::Zeroizing::new(credential.secret.into_inner());
    if secret.is_empty() || secret.bytes().any(|b| b.is_ascii_control()) {
        return None;
    }
    let needles = leak_needles_of(&secret);
    let mut headers = without_credentials(sent);
    let (name, value) = match (allowed.surface, allowed.kind, credential.token_kind) {
        (policy::Surface::Git, kind, _) => {
            let user = match kind {
                Kind::GitHub => GIT_LOGINS[0],
                Kind::GitLab => GIT_LOGINS[1],
            };
            let basic =
                data_encoding::BASE64.encode(format!("{user}:{}", secret.as_str()).as_bytes());
            ("Authorization", format!("Basic {basic}"))
        }
        (policy::Surface::Anonymous, _, _) => return None,
        (policy::Surface::Api, Kind::GitLab, TokenKind::Pat) => {
            ("PRIVATE-TOKEN", secret.to_string())
        }
        (policy::Surface::Api, _, _) => ("Authorization", format!("Bearer {}", secret.as_str())),
    };
    headers.push((name.to_string(), value));
    Some((headers, needles))
}

/// The forms of a credential an answer must not hold: as it is, percent-encoded, in Base64
/// (standard and URL alphabets), and the Base64 of the whole `Basic` pair it is sent in.
fn leak_needles_of(secret: &str) -> Vec<zeroize::Zeroizing<Vec<u8>>> {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    let mut forms: Vec<Vec<u8>> = vec![secret.as_bytes().to_vec()];
    let encoded = utf8_percent_encode(secret, NON_ALPHANUMERIC).to_string();
    if encoded != secret {
        forms.push(encoded.into_bytes());
    }
    forms.push(data_encoding::BASE64.encode(secret.as_bytes()).into_bytes());
    forms.push(
        data_encoding::BASE64URL_NOPAD
            .encode(secret.as_bytes())
            .into_bytes(),
    );
    // The `Basic` pair the proxy sends git, as a whole: Base64 of `login:secret` is not Base64 of
    // the secret, and inside a longer Base64 text it is spelled one of three ways, by where its
    // first byte falls in a group of three.
    for login in GIT_LOGINS {
        let pair = format!("{login}:{secret}");
        for encoding in [data_encoding::BASE64, data_encoding::BASE64URL_NOPAD] {
            for pad in 0..3 {
                let mut bytes = vec![0u8; pad];
                bytes.extend_from_slice(pair.as_bytes());
                let text = encoding.encode(&bytes);
                // Only the characters whose six bits all come from the pair: after the first one
                // that holds a padding byte's bits and before the last that holds a byte after it.
                let first = (pad * 8).div_ceil(6);
                let last = (bytes.len() * 8) / 6;
                let kept = text.get(first..last).unwrap_or_default();
                if !kept.is_empty() {
                    forms.push(kept.as_bytes().to_vec());
                }
            }
        }
    }
    forms.sort();
    forms.dedup();
    forms.into_iter().map(zeroize::Zeroizing::new).collect()
}

/// The logins the proxy sends a git request's `Basic` pair under, one per forge.
const GIT_LOGINS: [&str; 2] = ["x-access-token", "oauth2"];

/// Headers that name where a request goes or which method it is, which the proxy decides itself:
/// the upstream is [`Forwarded::upstream`], and the method is the request line's.
fn is_routing_header(name: &str) -> bool {
    let name = normal(name);
    name.starts_with("x-forwarded-")
        || [
            "host",
            "forwarded",
            "x-original-url",
            "x-rewrite-url",
            "x-real-ip",
            "x-http-method-override",
            "x-method-override",
            "x-http-method",
            // GitLab: act as another user.
            "sudo",
        ]
        .contains(&name.as_str())
}

/// What the chat sent, less every credential and every header that routes.
fn without_credentials(sent: &[(String, String)]) -> Vec<(String, String)> {
    sent.iter()
        .filter(|(name, _)| !is_credential_header(name) && !is_routing_header(name))
        .cloned()
        .collect()
}

/// A request whose body length is told once, or not at all, and never also as `chunked`: two
/// readers (this one's and the forge's) that frame it differently are a smuggled request. The only
/// transfer coding is exactly `chunked`.
pub(crate) fn framing_is_unambiguous(headers: &[(String, String)]) -> bool {
    let named = |wanted: &'static str| {
        headers
            .iter()
            .filter(move |(n, _)| normal(n) == wanted)
            .map(|(_, v)| v.trim())
    };
    let lengths: Vec<&str> = named("content-length").collect();
    let encodings: Vec<&str> = named("transfer-encoding").collect();
    let lengths_ok = lengths
        .iter()
        .all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_digit()))
        && lengths.windows(2).all(|w| w[0] == w[1]);
    let encodings_ok =
        encodings.len() <= 1 && encodings.iter().all(|e| e.eq_ignore_ascii_case("chunked"));
    lengths_ok && encodings_ok && !(encodings.len() == 1 && !lengths.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Base64 of the whole `login:secret` pair, in a longer Base64 text at each of the three
    /// alignments and in both alphabets, is found; a different secret's is not.
    #[test]
    fn the_base64_of_a_whole_basic_pair_is_a_needle_at_every_alignment() {
        let secret = ["tok", "en-for-the-test-0123"].concat();
        let needles = leak_needles_of(&secret);
        let found = |text: &str| {
            needles
                .iter()
                .any(|n| memchr::memmem::find(text.as_bytes(), n).is_some())
        };
        for login in GIT_LOGINS {
            let pair = format!("{login}:{secret}");
            for encoding in [data_encoding::BASE64, data_encoding::BASE64URL_NOPAD] {
                for pad in 0..3 {
                    for tail in 0..3 {
                        let mut bytes = vec![b'q'; pad + 5];
                        bytes.extend_from_slice(pair.as_bytes());
                        bytes.extend(std::iter::repeat_n(b'z', tail));
                        let text = format!("x{}y", encoding.encode(&bytes));
                        assert!(found(&text), "{login} pad {pad} tail {tail}: {text}");
                    }
                }
            }
        }
        let other = format!("x-access-token:{secret}x");
        let unrelated = data_encoding::BASE64.encode(other.replace("test", "tset").as_bytes());
        assert!(!found(&unrelated), "{unrelated}");
    }
}
