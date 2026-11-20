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
//! **Identity.** The chat is the endpoint's: with ADR 0067's per-chat egress proxy, the route for
//! forge hosts is this core keyed by that proxy's instance, and the secret arrives as
//! `Proxy-Authorization` (a `Basic` pair's password) or in any credential header, all of which
//! must be the sentinel, ten wrong ones in a row locking the chat out.

mod audit;
mod commands;
mod endpoint;
mod git;
mod graphql;
mod policy;
mod repo;
mod request;
mod rest;
mod sentinel;

pub use audit::{AuditLog, AuditRecord, Verdict};
#[cfg(unix)]
pub use endpoint::UnixEndpoint;
pub use endpoint::{ChatEndpoint, EndpointKind, LoopbackEndpoint, pick_endpoint};
pub use policy::{Allowed, BodyNeed, ChatPolicy, Class, Decision, NoObjects, ObjectRepo, Refusal};
pub use repo::{AddressError, AllowedRepo, Kind};
pub use request::RequestHead;
pub use sentinel::{ChatId, Sentinel, SentinelRegistry};

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

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
}

/// The real credential, held for one request.
#[derive(Debug)]
pub struct Credential {
    pub token_kind: TokenKind,
    pub secret: Secret,
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
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Forward(forwarded) => forwarded.fmt(f),
            Self::Refused(refusal) => f.debug_tuple("Refused").field(refusal).finish(),
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
}

impl ForgeProxy {
    pub fn new(credentials: Box<dyn CredentialSource>, objects: Box<dyn ObjectRepo>) -> Self {
        Self {
            policies: Mutex::new(HashMap::new()),
            sentinels: SentinelRegistry::default(),
            credentials,
            objects,
            audit: AuditLog::default(),
        }
    }

    /// A chat starts: its repos are the ones the person confirmed, and it gets a fresh sentinel
    /// (any it had is replaced). The sentinel is for the chat's environment; it is the only thing
    /// of the credential a chat ever holds.
    pub fn enrol(&self, chat: &ChatId, repos: Vec<AllowedRepo>) -> Sentinel {
        self.policies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(chat.clone(), Arc::new(ChatPolicy::new(repos)));
        self.sentinels.issue(chat)
    }

    /// A chat ends: its sentinel stops working at once, and its policy goes. Its audit stays.
    pub fn retire(&self, chat: &ChatId) {
        self.policies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(chat);
        self.sentinels.revoke(chat);
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
        if let Err(refusal) = self.sentinels.check(chat, inbound.headers) {
            return self.refuse(chat, inbound, None, refusal);
        }
        if !framing_is_unambiguous(inbound.headers) {
            return self.refuse(chat, inbound, None, Refusal::MalformedRequest);
        }
        let allowed = match policy.decide(inbound.head, inbound.body, self.objects.as_ref()) {
            Decision::Allow(allowed) => allowed,
            Decision::Refuse(refusal) => return self.refuse(chat, inbound, None, refusal),
        };
        if !allowed.credentialed() {
            // A public read: the chat's own headers without any credential, and none added.
            self.audit
                .record(chat, inbound.head, Some(&allowed), Verdict::Allowed);
            return Outcome::Forward(Forwarded {
                upstream: allowed.upstream,
                headers: without_credentials(inbound.headers),
                class: allowed.class,
                credentialed: false,
            });
        }
        let request = CredentialRequest {
            kind: allowed.kind,
            host: allowed.host.clone(),
        };
        let Ok(credential) = self.credentials.credential_for(&request) else {
            return self.refuse(chat, inbound, Some(&allowed), Refusal::NoCredential);
        };
        let Some(headers) = swap_credential(inbound.headers, &allowed, credential) else {
            return self.refuse(chat, inbound, Some(&allowed), Refusal::NoCredential);
        };
        self.audit
            .record(chat, inbound.head, Some(&allowed), Verdict::Allowed);
        Outcome::Forward(Forwarded {
            upstream: allowed.upstream,
            headers,
            class: allowed.class,
            credentialed: true,
        })
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
    NAMES.iter().any(|known| name.eq_ignore_ascii_case(known))
}

/// The chat's headers with the credential headers removed and the real credential set. `None` when
/// the secret would break a header (a line break in it).
fn swap_credential(
    sent: &[(String, String)],
    allowed: &Allowed,
    credential: Credential,
) -> Option<Vec<(String, String)>> {
    let secret = credential.secret.into_inner();
    if secret.is_empty() || secret.bytes().any(|b| b.is_ascii_control()) {
        return None;
    }
    let mut headers = without_credentials(sent);
    let (name, value) = match (allowed.surface, allowed.kind, credential.token_kind) {
        (policy::Surface::Git, kind, _) => {
            let user = match kind {
                Kind::GitHub => "x-access-token",
                Kind::GitLab => "oauth2",
            };
            let basic = data_encoding::BASE64.encode(format!("{user}:{secret}").as_bytes());
            ("Authorization", format!("Basic {basic}"))
        }
        (policy::Surface::Anonymous, _, _) => return None,
        (policy::Surface::Api, Kind::GitLab, TokenKind::Pat) => ("PRIVATE-TOKEN", secret),
        (policy::Surface::Api, _, _) => ("Authorization", format!("Bearer {secret}")),
    };
    headers.push((name.to_string(), value));
    Some(headers)
}

/// Headers that name where a request goes or which method it is, which the proxy decides itself:
/// the upstream is [`Forwarded::upstream`], and the method is the request line's.
fn is_routing_header(name: &str) -> bool {
    [
        "host",
        "x-http-method-override",
        "x-method-override",
        "x-http-method",
    ]
    .iter()
    .any(|known| name.eq_ignore_ascii_case(known))
}

/// What the chat sent, less every credential and every header that routes.
fn without_credentials(sent: &[(String, String)]) -> Vec<(String, String)> {
    sent.iter()
        .filter(|(name, _)| !is_credential_header(name) && !is_routing_header(name))
        .cloned()
        .collect()
}

/// A request whose body length is told once, or not at all, and never also as `chunked`: two
/// readers (this one's and the forge's) that frame it differently are a smuggled request.
fn framing_is_unambiguous(headers: &[(String, String)]) -> bool {
    let named = |wanted: &'static str| {
        headers
            .iter()
            .filter(move |(n, _)| n.eq_ignore_ascii_case(wanted))
            .map(|(_, v)| v.trim())
    };
    let lengths: Vec<&str> = named("content-length").collect();
    let encodings = named("transfer-encoding").count();
    let lengths_ok = lengths
        .iter()
        .all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_digit()))
        && lengths.windows(2).all(|w| w[0] == w[1]);
    lengths_ok && encodings <= 1 && !(encodings == 1 && !lengths.is_empty())
}
