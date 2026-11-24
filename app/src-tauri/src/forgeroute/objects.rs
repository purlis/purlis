// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which repo a GraphQL node id is in** (the core's [`ObjectRepo`] seam), for one run's proxy.
//!
//! `gh pr merge` names its pull request by node id (`mergePullRequest(input:{pullRequestId})`, and
//! `enablePullRequestAutoMerge`, `enqueuePullRequest`, `closePullRequest`). The policy refuses a
//! mutation whose id it cannot tie to a recorded repo of the chat's own run, and only the forge
//! knows which repo an id is in. So the route asks it:
//!
//! ```graphql
//! query($id: ID!) { node(id: $id) { __typename
//!   ... on PullRequest { repository { nameWithOwner } }
//!   ... on Issue { repository { nameWithOwner } } } }
//! ```
//!
//! - **With this run's own credential**, the one the proxy gives the same chat for the recorded
//!   repo ([`ForgeLookup`]), so the repo is read as the person's confirmed account and a repo that
//!   account cannot see answers nothing. The answer is a repo's name, never used to send anything.
//! - **The core compares it** with each recorded repo's path, case-folded (both forges fold case),
//!   so the lookup only has to say which repo the id is in.
//! - **Fail closed.** An id that is not a pull request or an issue, one the forge does not know, a
//!   forge that could not be reached, a credential that was not had, an id of a shape no forge
//!   mints, and any forge but GitHub (GitLab names its project by path) answer `None`, and the core
//!   refuses the mutation as another repo.
//! - **Cached per run, bounded** ([`MOST_CACHED`], oldest out first): a document names an id
//!   twice (the hold and the merge stance read it again), and a chat repeats itself. A forge that
//!   could not be asked is **not** cached, so the next request asks again.
//!
//! **Tier** (ADR 0069): nothing is stored. The cache is this run's memory.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use purlis_core::forge::accounts::over_https;
use purlis_core::forge::http::TokenSource;
use purlis_core::forge::proxy::{
    AllowedRepo, ChatId, Credential, CredentialError, CredentialRequest, CredentialSource,
    ObjectRepo,
};
use purlis_core::forge::transport::{Call, Field};
use purlis_core::forge::{Failure, Forge, ForgeError, Kind as ForgeKind};
use purlis_core::forgewire::SecretString;

/// The most ids one run remembers. A run names a handful; the bound is for one that names
/// thousands.
pub const MOST_CACHED: usize = 256;

/// The longest node id asked about. GitHub's are under a hundred characters.
const MOST_ID_LEN: usize = 128;

/// How long the forge has to answer. A request waits on it, so it is short.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);

const QUERY: &str = "query($id: ID!) { node(id: $id) { __typename \
    ... on PullRequest { repository { nameWithOwner } } \
    ... on Issue { repository { nameWithOwner } } } }";

const NUMBER_QUERY: &str = "query($id: ID!) { node(id: $id) { ... on PullRequest { number } } }";

/// What asks the forge which repo a node id is in. A trait so a test stands where the network
/// would.
pub trait NodeRepos: Send + Sync {
    /// `Ok(Some(owner/name))` for a pull request or an issue, `Ok(None)` for anything else the
    /// forge answered (no such node, another kind of node), `Err` when it could not be asked.
    /// `request` carries the credential's terms: the run's chat and a recorded repo on the host.
    fn repo_of_node(&self, request: &CredentialRequest, id: &str)
    -> Result<Option<String>, String>;

    /// `Ok(Some(number))` for a pull request, `Ok(None)` for any other node, `Err` when the forge
    /// could not be asked: what a held merge names (#7543). A lookup that cannot say answers
    /// `Ok(None)`, and the merge is refused with a reason instead of asked about.
    fn pull_number_of_node(
        &self,
        _request: &CredentialRequest,
        _id: &str,
    ) -> Result<Option<u64>, String> {
        Ok(None)
    }
}

/// The production [`NodeRepos`]: GitHub's GraphQL, over HTTPS, sending the run's credential.
pub struct ForgeLookup {
    credentials: Arc<dyn CredentialSource>,
}

impl ForgeLookup {
    pub fn new(credentials: Arc<dyn CredentialSource>) -> Self {
        Self { credentials }
    }
}

/// The run's credential as the transport's token, read as the request is sent.
struct RouteToken {
    credentials: Arc<dyn CredentialSource>,
    request: CredentialRequest,
}

impl TokenSource for RouteToken {
    fn token(&self) -> Result<SecretString, ForgeError> {
        self.credentials
            .credential_for(&self.request)
            .map(|credential| SecretString::from(credential.secret.into_inner()))
            .map_err(|_| {
                ForgeError::of(
                    Failure::Auth,
                    "purlis has no sign-in to ask the forge with for this chat",
                )
            })
    }
}

impl NodeRepos for ForgeLookup {
    fn repo_of_node(
        &self,
        request: &CredentialRequest,
        id: &str,
    ) -> Result<Option<String>, String> {
        let forge = Forge {
            kind: ForgeKind::GitHub,
            host: api_host(&request.host, request.port),
        };
        let tokens = Arc::new(RouteToken {
            credentials: self.credentials.clone(),
            request: request.clone(),
        });
        let transport = over_https(&forge, tokens).map_err(|e| e.to_string())?;
        let call = Call::graphql(QUERY, vec![Field::text("id", id)], LOOKUP_TIMEOUT);
        let reply = transport
            .send(&forge, &call)
            .map_err(|why| why.said().to_owned())?;
        if !reply.ok() {
            return Err(reply.said(ForgeKind::GitHub));
        }
        repo_in(&reply.out)
    }

    fn pull_number_of_node(
        &self,
        request: &CredentialRequest,
        id: &str,
    ) -> Result<Option<u64>, String> {
        let forge = Forge {
            kind: ForgeKind::GitHub,
            host: api_host(&request.host, request.port),
        };
        let tokens = Arc::new(RouteToken {
            credentials: self.credentials.clone(),
            request: request.clone(),
        });
        let transport = over_https(&forge, tokens).map_err(|e| e.to_string())?;
        let call = Call::graphql(NUMBER_QUERY, vec![Field::text("id", id)], LOOKUP_TIMEOUT);
        let reply = transport
            .send(&forge, &call)
            .map_err(|why| why.said().to_owned())?;
        if !reply.ok() {
            return Err(reply.said(ForgeKind::GitHub));
        }
        number_in(&reply.out)
    }
}

/// The pull request number a lookup's answer names. `Ok(None)` where the node is not there or is
/// not a pull request; `Err` for an answer that is not JSON.
fn number_in(answer: &str) -> Result<Option<u64>, String> {
    let json: serde_json::Value = serde_json::from_str(answer)
        .map_err(|e| format!("the forge's answer was not JSON: {e}"))?;
    Ok(json["data"]["node"]["number"].as_u64().filter(|n| *n > 0))
}

/// The host the lookup's API root is made from: the recorded repo's own port is part of it unless
/// it is HTTPS's default, so a GHES on `:8443` is asked there and not on `:443`.
fn api_host(host: &str, port: u16) -> String {
    if port == 443 {
        host.to_owned()
    } else {
        format!("{host}:{port}")
    }
}

/// The repo a lookup's answer names, lower case. `Ok(None)` where the node is not there or holds
/// no repository (a node that is neither a pull request nor an issue); `Err` for an answer that is
/// not the shape the query asked for.
fn repo_in(answer: &str) -> Result<Option<String>, String> {
    let json: serde_json::Value = serde_json::from_str(answer)
        .map_err(|e| format!("the forge's answer was not JSON: {e}"))?;
    let node = &json["data"]["node"];
    if node.is_null() {
        return Ok(None);
    }
    let Some(name) = node["repository"]["nameWithOwner"].as_str() else {
        return Ok(None);
    };
    let name = name.to_ascii_lowercase();
    let mut parts = name.split('/');
    let plain = |s: &str| {
        !s.is_empty()
            && s != "."
            && s != ".."
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    };
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(repo), None) if plain(owner) && plain(repo) => Ok(Some(name)),
        _ => Err("the forge named a repo that is not owner/name".to_owned()),
    }
}

/// A node id as a forge mints one: base64url-ish, short. Anything else is not asked about.
fn is_node_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MOST_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'='))
}

/// Answers kept, newest last: a map and the order its keys went in.
#[derive(Default)]
struct Cache {
    answers: HashMap<(String, String), Option<String>>,
    order: VecDeque<(String, String)>,
}

impl Cache {
    fn put(&mut self, key: (String, String), answer: Option<String>) {
        if self.answers.insert(key.clone(), answer).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > MOST_CACHED {
            if let Some(oldest) = self.order.pop_front() {
                self.answers.remove(&oldest);
            }
        }
    }
}

/// One run's [`ObjectRepo`]: asks `lookup` for the repo of a node id, in the terms of a recorded
/// repo of this run on the id's host, and remembers what the forge answered.
pub struct RunObjects {
    chat: ChatId,
    repos: Vec<AllowedRepo>,
    lookup: Arc<dyn NodeRepos>,
    cache: Mutex<Cache>,
}

impl RunObjects {
    pub fn new(chat: ChatId, repos: Vec<AllowedRepo>, lookup: Arc<dyn NodeRepos>) -> Self {
        Self {
            chat,
            repos,
            lookup,
            cache: Mutex::new(Cache::default()),
        }
    }

    /// How many answers are held.
    pub fn cached(&self) -> usize {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .answers
            .len()
    }
}

impl ObjectRepo for RunObjects {
    fn pull_of(
        &self,
        kind: purlis_core::forge::proxy::Kind,
        host: &str,
        node_id: &str,
    ) -> Option<(String, u64)> {
        let repo = self.repo_of(kind, host, node_id)?;
        let recorded = self
            .repos
            .iter()
            .find(|r| r.kind() == kind && r.host().eq_ignore_ascii_case(host))?;
        let request = CredentialRequest {
            kind,
            host: recorded.host().to_owned(),
            port: recorded.port(),
            repo_id: recorded.forge_id(),
            chat: self.chat.clone(),
            repo: recorded.path().to_owned(),
        };
        let number = self.lookup.pull_number_of_node(&request, node_id).ok()??;
        Some((repo, number))
    }

    fn repo_of(
        &self,
        kind: purlis_core::forge::proxy::Kind,
        host: &str,
        node_id: &str,
    ) -> Option<String> {
        if kind != purlis_core::forge::proxy::Kind::GitHub || !is_node_id(node_id) {
            return None;
        }
        let key = (host.to_ascii_lowercase(), node_id.to_owned());
        if let Some(answer) = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .answers
            .get(&key)
        {
            return answer.clone();
        }
        // The credential is the one a recorded repo of this run on that host is given; a host the
        // run has no repo on is nobody's to ask.
        let repo = self
            .repos
            .iter()
            .find(|r| r.kind() == kind && r.host().eq_ignore_ascii_case(host))?;
        let request = CredentialRequest {
            kind,
            host: repo.host().to_owned(),
            port: repo.port(),
            repo_id: repo.forge_id(),
            chat: self.chat.clone(),
            repo: repo.path().to_owned(),
        };
        // Not held across the network: another request may ask meanwhile, and both answers are
        // the forge's.
        let answer = self.lookup.repo_of_node(&request, node_id).ok()?;
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .put(key, answer.clone());
        answer
    }
}

/// A shared credential source as a boxed one: the proxy owns one handle and the lookup another.
pub struct Shared<T>(pub Arc<T>);

impl<T: CredentialSource> CredentialSource for Shared<T> {
    fn credential_for(&self, request: &CredentialRequest) -> Result<Credential, CredentialError> {
        self.0.credential_for(request)
    }
}

#[cfg(test)]
mod tests;
