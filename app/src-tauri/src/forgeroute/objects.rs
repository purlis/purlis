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

/// What asks the forge which repo a node id is in. A trait so a test stands where the network
/// would.
pub trait NodeRepos: Send + Sync {
    /// `Ok(Some(owner/name))` for a pull request or an issue, `Ok(None)` for anything else the
    /// forge answered (no such node, another kind of node), `Err` when it could not be asked.
    /// `request` carries the credential's terms: the run's chat and a recorded repo on the host.
    fn repo_of_node(&self, request: &CredentialRequest, id: &str)
    -> Result<Option<String>, String>;
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
            .map(|credential| SecretString::from(credential.secret.expose().to_owned()))
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
mod tests {
    use super::*;
    use purlis_core::forge::proxy::Kind;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Answers {
        by_id: HashMap<&'static str, Result<Option<&'static str>, &'static str>>,
        asked: AtomicUsize,
        terms: Mutex<Vec<CredentialRequest>>,
    }

    impl Answers {
        fn of(by_id: &[(&'static str, Result<Option<&'static str>, &'static str>)]) -> Arc<Self> {
            Arc::new(Self {
                by_id: by_id.iter().cloned().collect(),
                asked: AtomicUsize::new(0),
                terms: Mutex::new(Vec::new()),
            })
        }
    }

    impl NodeRepos for Answers {
        fn repo_of_node(
            &self,
            request: &CredentialRequest,
            id: &str,
        ) -> Result<Option<String>, String> {
            self.asked.fetch_add(1, Ordering::SeqCst);
            self.terms.lock().unwrap().push(request.clone());
            match self.by_id.get(id) {
                Some(Ok(repo)) => Ok(repo.map(str::to_owned)),
                Some(Err(why)) => Err((*why).to_owned()),
                None => Err("unknown to the test".to_owned()),
            }
        }
    }

    fn run(lookup: Arc<dyn NodeRepos>) -> RunObjects {
        let repo = AllowedRepo::new(Kind::GitHub, "github.com", 443, "Acme/Widgets", "main")
            .unwrap()
            .with_forge_id(purlis_core::forge::proxy::ForgeRepoId::for_a_test(7));
        RunObjects::new(ChatId::new("run-1"), vec![repo], lookup)
    }

    #[test]
    fn a_ported_repo_is_asked_in_the_terms_of_its_own_port() {
        let lookup = Answers::of(&[("PR_kwDOA", Ok(Some("acme/widgets")))]);
        let repo = AllowedRepo::new(
            Kind::GitHub,
            "ghes.example.com",
            8443,
            "acme/widgets",
            "main",
        )
        .unwrap();
        let objects = RunObjects::new(ChatId::new("run-1"), vec![repo], lookup.clone());

        assert!(
            objects
                .repo_of(Kind::GitHub, "ghes.example.com", "PR_kwDOA")
                .is_some()
        );
        assert_eq!(lookup.terms.lock().unwrap()[0].port, 8443);
    }

    #[test]
    fn a_ported_host_is_asked_on_its_port() {
        assert_eq!(api_host("ghes.example.com", 8443), "ghes.example.com:8443");
        assert_eq!(api_host("github.com", 443), "github.com");
    }

    #[test]
    fn an_id_in_a_repo_names_that_repo_in_the_terms_of_the_runs_own_recorded_repo() {
        let lookup = Answers::of(&[("PR_kwDOA", Ok(Some("acme/widgets")))]);
        let objects = run(lookup.clone());

        assert_eq!(
            objects
                .repo_of(Kind::GitHub, "github.com", "PR_kwDOA")
                .as_deref(),
            Some("acme/widgets")
        );
        let terms = lookup.terms.lock().unwrap();
        assert_eq!(terms[0].chat, ChatId::new("run-1"));
        assert_eq!(terms[0].repo, "acme/widgets");
        assert_eq!(terms[0].repo_id, Some(7));
    }

    #[test]
    fn the_answer_is_what_the_forge_said_so_another_repos_id_names_another_repo() {
        let lookup = Answers::of(&[("PR_other", Ok(Some("someone/else")))]);

        assert_eq!(
            run(lookup)
                .repo_of(Kind::GitHub, "github.com", "PR_other")
                .as_deref(),
            Some("someone/else")
        );
    }

    #[test]
    fn an_id_the_forge_does_not_place_names_no_repo() {
        let lookup = Answers::of(&[("U_user", Ok(None)), ("PR_down", Err("timed out"))]);
        let objects = run(lookup);

        assert_eq!(objects.repo_of(Kind::GitHub, "github.com", "U_user"), None);
        assert_eq!(objects.repo_of(Kind::GitHub, "github.com", "PR_down"), None);
        assert_eq!(
            objects.repo_of(Kind::GitHub, "github.com", "PR_never"),
            None
        );
    }

    #[test]
    fn an_id_no_forge_mints_a_gitlab_repo_and_a_host_with_no_recorded_repo_are_not_asked() {
        let lookup = Answers::of(&[("PR_a", Ok(Some("acme/widgets")))]);
        let objects = run(lookup.clone());

        for id in ["", "PR a", "PR_a\"}", &"x".repeat(MOST_ID_LEN + 1)] {
            assert_eq!(
                objects.repo_of(Kind::GitHub, "github.com", id),
                None,
                "{id}"
            );
        }
        assert_eq!(objects.repo_of(Kind::GitLab, "github.com", "PR_a"), None);
        assert_eq!(objects.repo_of(Kind::GitHub, "ghe.example", "PR_a"), None);
        assert_eq!(lookup.asked.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_repeated_id_is_asked_once_and_a_forge_that_failed_is_asked_again() {
        let lookup = Answers::of(&[("PR_a", Ok(Some("acme/widgets"))), ("PR_down", Err("no"))]);
        let objects = run(lookup.clone());

        for _ in 0..3 {
            objects.repo_of(Kind::GitHub, "GitHub.com", "PR_a");
        }
        assert_eq!(lookup.asked.load(Ordering::SeqCst), 1);

        objects.repo_of(Kind::GitHub, "github.com", "PR_down");
        objects.repo_of(Kind::GitHub, "github.com", "PR_down");
        assert_eq!(lookup.asked.load(Ordering::SeqCst), 3);
        assert_eq!(objects.cached(), 1);
    }

    #[test]
    fn the_cache_is_bounded_and_forgets_the_oldest_first() {
        struct Everywhere(AtomicUsize);
        impl NodeRepos for Everywhere {
            fn repo_of_node(
                &self,
                _: &CredentialRequest,
                _: &str,
            ) -> Result<Option<String>, String> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(Some("acme/widgets".to_owned()))
            }
        }
        let lookup = Arc::new(Everywhere(AtomicUsize::new(0)));
        let objects = run(lookup.clone());

        for n in 0..MOST_CACHED + 50 {
            objects.repo_of(Kind::GitHub, "github.com", &format!("PR_{n}"));
        }
        assert_eq!(objects.cached(), MOST_CACHED);

        let before = lookup.0.load(Ordering::SeqCst);
        objects.repo_of(
            Kind::GitHub,
            "github.com",
            &format!("PR_{}", MOST_CACHED + 49),
        );
        assert_eq!(
            lookup.0.load(Ordering::SeqCst),
            before,
            "the newest is kept"
        );
        objects.repo_of(Kind::GitHub, "github.com", "PR_0");
        assert_eq!(
            lookup.0.load(Ordering::SeqCst),
            before + 1,
            "the oldest is gone"
        );
    }

    #[test]
    fn the_forges_answer_is_read_as_the_repo_it_names_and_nothing_else_names_one() {
        let pr = r#"{"data":{"node":{"__typename":"PullRequest","repository":{"nameWithOwner":"Acme/Widgets"}}}}"#;
        assert_eq!(repo_in(pr), Ok(Some("acme/widgets".to_owned())));
        let issue =
            r#"{"data":{"node":{"__typename":"Issue","repository":{"nameWithOwner":"a/b"}}}}"#;
        assert_eq!(repo_in(issue), Ok(Some("a/b".to_owned())));
        // A node of another type has `__typename` and no repository.
        let user = r#"{"data":{"node":{"__typename":"User"}}}"#;
        assert_eq!(repo_in(user), Ok(None));
        assert_eq!(repo_in(r#"{"data":{"node":null}}"#), Ok(None));
        assert!(repo_in("not json").is_err());
        let odd = r#"{"data":{"node":{"repository":{"nameWithOwner":"a/b/c"}}}}"#;
        assert!(repo_in(odd).is_err());
        let odd = r#"{"data":{"node":{"repository":{"nameWithOwner":"../b"}}}}"#;
        assert!(repo_in(odd).is_err());
    }

    // ---- through the proxy: what `gh pr merge` sends, with this resolver where the forge is ----

    use purlis_core::enforcement::Mode;
    use purlis_core::forge::mergestance::PolicyMergeStance;
    use purlis_core::forge::proxy::{ForgeProxy, Inbound, Outcome, RequestHead, TokenKind};
    use purlis_core::secrets::keyring::Secret;

    struct Person;
    impl CredentialSource for Person {
        fn credential_for(&self, _: &CredentialRequest) -> Result<Credential, CredentialError> {
            Ok(Credential {
                token_kind: TokenKind::Oauth,
                secret: Secret::new("real-token".to_owned()),
            })
        }
    }

    /// The outcome of `mutation` sent by a chat whose one recorded repo is `acme/widgets`, under
    /// `mode`, where the forge places `PR_mine` in that repo, `PR_theirs` in another and nothing
    /// else.
    fn sent(mode: Mode, mutation: &str) -> (Outcome, usize) {
        let lookup = Answers::of(&[
            ("PR_mine", Ok(Some("acme/widgets"))),
            ("PR_theirs", Ok(Some("someone/else"))),
            ("U_user", Ok(None)),
            ("PR_down", Err("timed out")),
        ]);
        let objects = run(lookup.clone());
        let proxy = ForgeProxy::new(Box::new(Person), Box::new(objects)).with_merge_stance(
            Arc::new(PolicyMergeStance::reading(move || mode, |_, _| {})),
        );
        let chat = ChatId::new("run-1");
        let repo =
            AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/widgets", "main").unwrap();
        let sentinel = proxy.enrol(&chat, vec![repo]);
        let body =
            serde_json::to_vec(&serde_json::json!({ "query": mutation, "variables": {} })).unwrap();
        let head = RequestHead::new("POST", "api.github.com", 443, "/graphql");
        let headers = vec![
            (
                "Authorization".to_owned(),
                format!("Bearer {}", sentinel.expose()),
            ),
            ("Content-Type".to_owned(), "application/json".to_owned()),
        ];
        let out = proxy.handle(
            &chat,
            &Inbound {
                head: &head,
                headers: &headers,
                body: &body,
            },
        );
        (out, lookup.asked.load(Ordering::SeqCst))
    }

    fn by_id(name: &str, id: &str) -> String {
        format!("mutation{{ {name}(input:{{pullRequestId:\"{id}\"}}){{ clientMutationId }} }}")
    }

    const BY_ID: [&str; 4] = [
        "mergePullRequest",
        "enablePullRequestAutoMerge",
        "enqueuePullRequest",
        "closePullRequest",
    ];

    #[test]
    fn a_merge_by_node_id_on_the_recorded_repo_is_forwarded_under_log_and_off() {
        for mode in [Mode::Log, Mode::Off] {
            for name in &BY_ID[..3] {
                let (out, asked) = sent(mode, &by_id(name, "PR_mine"));
                assert!(
                    matches!(out, Outcome::Forward(_)),
                    "{name} {mode:?}: {out:?}"
                );
                assert_eq!(asked, 1, "{name}: the forge was asked for the id");
            }
        }
    }

    #[test]
    fn a_merge_by_node_id_is_held_under_enforce_and_never_forwarded() {
        for name in &BY_ID[..3] {
            let (out, _) = sent(Mode::Enforce, &by_id(name, "PR_mine"));
            assert!(!matches!(out, Outcome::Forward(_)), "{name}: {out:?}");
        }
    }

    #[test]
    fn closing_a_pull_request_by_node_id_on_the_recorded_repo_is_no_longer_another_repos() {
        // A close is no merge: the id is tied to the recorded repo and it goes through.
        let (out, _) = sent(Mode::Enforce, &by_id("closePullRequest", "PR_mine"));
        assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
    }

    #[test]
    fn a_node_id_of_another_repo_an_unplaced_id_and_a_forge_that_failed_are_refused_in_every_mode()
    {
        for mode in [Mode::Log, Mode::Off, Mode::Enforce] {
            for name in BY_ID {
                for id in ["PR_theirs", "U_user", "PR_down", "PR_unknown"] {
                    let (out, _) = sent(mode, &by_id(name, id));
                    assert!(
                        !matches!(out, Outcome::Forward(_)),
                        "{name} {id} {mode:?}: {out:?}"
                    );
                }
            }
        }
    }
}
