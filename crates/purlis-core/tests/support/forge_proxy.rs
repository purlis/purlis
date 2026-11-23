// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A forge proxy with one chat on four hosts, and the means to send it requests.
//!
//! The chat `chat-a` has one repo on each of github.com, a GitHub Enterprise Server, gitlab.com and
//! a self-hosted GitLab (a project in a subgroup). `chat-b` is another chat with a repo of its own.
//! The credential source hands out [`REAL`] and counts how often it is asked.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use purlis_core::forge::proxy::{
    AllowedRepo, ChatId, Credential, CredentialError, CredentialRequest, CredentialSource,
    ForgeProxy, Inbound, Kind, ObjectRepo, Outcome, RequestHead, Sentinel, TokenKind,
};
use purlis_core::secrets::keyring::Secret;

/// The person's real token. No test may find it anywhere a chat can read.
pub const REAL: &str = "REALforgeTOKEN-9d41c7";

pub const GHES: &str = "ghe.corp.example";
pub const GITLAB_SELF: &str = "git.corp.example";

pub struct Creds {
    pub asked: Arc<AtomicUsize>,
    pub token_kind: TokenKind,
}

impl CredentialSource for Creds {
    fn credential_for(&self, _: &CredentialRequest) -> Result<Credential, CredentialError> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        Ok(Credential {
            token_kind: self.token_kind,
            secret: Secret::new(REAL.to_string()),
            account: None,
        })
    }
}

/// Which repo each GraphQL object id is in.
#[derive(Default)]
pub struct Objects(pub HashMap<String, String>);

impl ObjectRepo for Objects {
    fn repo_of(&self, _: Kind, _: &str, id: &str) -> Option<String> {
        self.0.get(id).cloned()
    }
}

/// [`Objects`] that count each ask: how often the proxy resolved a node id.
pub struct CountingObjects {
    pub known: Objects,
    pub asked: Arc<AtomicUsize>,
}

impl ObjectRepo for CountingObjects {
    fn repo_of(&self, kind: Kind, host: &str, id: &str) -> Option<String> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        self.known.repo_of(kind, host, id)
    }
}

pub struct Scene {
    pub proxy: ForgeProxy,
    pub a: ChatId,
    pub b: ChatId,
    pub a_sentinel: Sentinel,
    pub b_sentinel: Sentinel,
    pub asked: Arc<AtomicUsize>,
}

/// How a request presents its credential.
pub enum Auth<'a> {
    None,
    Bearer(&'a str),
    Token(&'a str),
    Basic(&'a str, &'a str),
    PrivateToken(&'a str),
}

impl Scene {
    pub fn new() -> Self {
        Self::build(TokenKind::Oauth, Objects::default(), true)
    }

    /// As [`Scene::new`], but with chat-a's GitHub repos on one host only, so a call that names no
    /// repo can only mean it.
    pub fn single() -> Self {
        Self::build(TokenKind::Oauth, Objects::default(), false)
    }

    pub fn with(token_kind: TokenKind, objects: Objects) -> Self {
        Self::build(token_kind, objects, false)
    }

    /// chat-a has repos on github.com and on a GHES.
    pub fn with_two_github_hosts(token_kind: TokenKind, objects: Objects) -> Self {
        Self::build(token_kind, objects, true)
    }

    /// As [`Scene::with`], with the proxy changed by `tune` before the chats are enrolled.
    pub fn tuned(
        objects: impl ObjectRepo + 'static,
        tune: impl FnOnce(ForgeProxy) -> ForgeProxy,
    ) -> Self {
        Self::build_tuned(TokenKind::Oauth, objects, false, tune)
    }

    fn build(token_kind: TokenKind, objects: Objects, ghes: bool) -> Self {
        Self::build_tuned(token_kind, objects, ghes, |proxy| proxy)
    }

    fn build_tuned(
        token_kind: TokenKind,
        objects: impl ObjectRepo + 'static,
        ghes: bool,
        tune: impl FnOnce(ForgeProxy) -> ForgeProxy,
    ) -> Self {
        let asked = Arc::new(AtomicUsize::new(0));
        let proxy = ForgeProxy::new(
            Box::new(Creds {
                asked: Arc::clone(&asked),
                token_kind,
            }),
            Box::new(objects),
        );
        let proxy = tune(proxy);
        let a = ChatId::new("chat-a");
        let b = ChatId::new("chat-b");
        let mut a_repos = vec![
            AllowedRepo::new(Kind::GitHub, "github.com", 443, "Acme/App", "main")
                .unwrap()
                .with_node_id("R_acme_app")
                .with_protected_branches(&["release"]),
            AllowedRepo::new(Kind::GitLab, "gitlab.com", 443, "group/proj", "main")
                .unwrap()
                .with_forge_id(purlis_core::forge::proxy::ForgeRepoId::for_a_test(42))
                .with_own_branches(&[("work", &"1".repeat(40))]),
            AllowedRepo::new(Kind::GitLab, GITLAB_SELF, 8443, "grp/sub/proj", "main").unwrap(),
        ];
        if ghes {
            a_repos.push(AllowedRepo::new(Kind::GitHub, GHES, 443, "acme/app", "trunk").unwrap());
        }
        let advertised: Vec<(String, String)> = a_repos
            .iter()
            .map(|r| (r.host().to_string(), r.path().to_string()))
            .collect();
        let a_sentinel = proxy.enrol(&a, a_repos);
        // Every repo advertised no branch but the ones that cannot be pushed to.
        for (host, path) in &advertised {
            proxy.note_advertised(&a, host, path, vec![]);
        }
        let b_sentinel = proxy.enrol(
            &b,
            vec![AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/other", "main").unwrap()],
        );
        Self {
            proxy,
            a,
            b,
            a_sentinel,
            b_sentinel,
            asked,
        }
    }

    /// The listener read a repo's advertisement for a push: these are its branches.
    pub fn advertise(&self, host: &str, path: &str, branches: &[&str]) {
        let branches = branches.iter().map(|b| b.to_string()).collect();
        self.proxy.note_advertised(&self.a, host, path, branches);
    }

    /// The forge said the push `out` was made: its branches are the chat's own now.
    pub fn confirm(&self, out: &Outcome) {
        if let Outcome::Forward(f) = out {
            self.proxy.confirm_push(&self.a, f);
        }
    }

    /// `chat-a` sends a request with its own sentinel as a bearer token.
    pub fn send(&self, method: &str, host: &str, target: &str, body: &[u8]) -> Outcome {
        self.send_as(
            &self.a,
            method,
            host,
            443,
            target,
            Auth::Bearer(self.a_sentinel.expose()),
            body,
        )
    }

    pub fn send_port(&self, method: &str, host: &str, port: u16, target: &str) -> Outcome {
        self.send_as(
            &self.a,
            method,
            host,
            port,
            target,
            Auth::Bearer(self.a_sentinel.expose()),
            b"",
        )
    }

    /// `chat-a` sends a write whose `Content-Type` the test names, whatever the body looks like.
    pub fn send_typed(&self, host: &str, target: &str, content_type: &str, body: &[u8]) -> Outcome {
        let head = RequestHead::new("POST", host, 443, target);
        let mut headers = headers(Auth::Bearer(self.a_sentinel.expose()));
        headers.push(("Content-Type".into(), content_type.into()));
        self.proxy.handle(
            &self.a,
            &Inbound {
                head: &head,
                headers: &headers,
                body,
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send_as(
        &self,
        chat: &ChatId,
        method: &str,
        host: &str,
        port: u16,
        target: &str,
        auth: Auth<'_>,
        body: &[u8],
    ) -> Outcome {
        let head = RequestHead::new(method, host, port, target);
        let mut headers = headers(auth);
        if let Some(ct) = content_type_for(target, body) {
            headers.push(("Content-Type".into(), ct.into()));
        }
        self.proxy.handle(
            chat,
            &Inbound {
                head: &head,
                headers: &headers,
                body,
            },
        )
    }
}

pub fn headers(auth: Auth<'_>) -> Vec<(String, String)> {
    let mut h = vec![("Accept".to_string(), "application/json".to_string())];
    match auth {
        Auth::None => {}
        Auth::Bearer(t) => h.push(("Authorization".into(), format!("Bearer {t}"))),
        Auth::Token(t) => h.push(("authorization".into(), format!("token {t}"))),
        Auth::Basic(u, p) => h.push((
            "Authorization".into(),
            format!(
                "Basic {}",
                data_encoding::BASE64.encode(format!("{u}:{p}").as_bytes())
            ),
        )),
        Auth::PrivateToken(t) => h.push(("PRIVATE-TOKEN".into(), t.into())),
    }
    h
}

/// A GraphQL request body.
pub fn graphql(query: &str, variables: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "query": query, "variables": variables })).unwrap()
}

/// The pkt-lines of a push's commands, then the flush and a stand-in for the pack.
pub fn push_body(commands: &[(&str, &str)]) -> Vec<u8> {
    let zero = "0".repeat(40);
    let mut out = Vec::new();
    for (i, (kind, name)) in commands.iter().enumerate() {
        // An update names the tip the chat's own create (or its given branch) left: `1` * 40.
        let old = if *kind == "create" {
            zero.clone()
        } else {
            "1".repeat(40)
        };
        let new = if *kind == "delete" {
            zero.clone()
        } else {
            "1".repeat(40)
        };
        let mut line = format!("{old} {new} {name}");
        if i == 0 {
            line.push('\0');
            line.push_str(" report-status side-band-64k agent=git/2");
        }
        out.extend(format!("{:04x}{line}", line.len() + 4).as_bytes());
    }
    out.extend(b"0000PACK....");
    out
}

pub fn is_refused(outcome: &Outcome, reason: purlis_core::forge::proxy::Refusal) -> bool {
    matches!(outcome, Outcome::Refused(r) if *r == reason)
}

/// The `Content-Type` a client would send with `body`: JSON, a form, or git's own for a push.
fn content_type_for(target: &str, body: &[u8]) -> Option<&'static str> {
    if body.is_empty() {
        None
    } else if target.contains("git-receive-pack") || target.contains("git-upload-pack") {
        Some("application/x-git-receive-pack-request")
    } else if body.first().is_some_and(|b| *b == b'{' || *b == b'[') {
        Some("application/json")
    } else if body.contains(&b'=') && !body.contains(&b'{') {
        Some("application/x-www-form-urlencoded")
    } else {
        Some("application/json")
    }
}
