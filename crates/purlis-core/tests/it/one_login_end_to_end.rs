// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice I: the scenario of ADR 0128, with slice A's policy deciding every request.
//!
//! A person signs in once to a forge, clones a repo as a project, and a chat runs `gh`, `glab` and
//! `git` against it. `tests/one-login-e2e/run.mjs` is the scenario: the real clients against a fake
//! forge of each shape. **What is real in this test:** the clients, the fake forge's wire, and the
//! policy: this file hosts a [`ForgeProxy`] (slice A, `forge::proxy`), and the scenario's
//! stand-in listener asks it, over a loopback socket, to decide **every request** a chat's client
//! makes; what it allows is forwarded as the core re-emitted it, with the credential the core put
//! in, and what it refuses is refused with the core's own refusal. **What is still a stand-in**
//! (`tests/one-login-e2e/seams.mjs`, each marked `TODO(slice X)`): the listeners and the CA
//! (slice A2, #7291), the app lifecycle (slice J, #7290), the chat's wiring (slice B), the keyring
//! and renewal (slice C), the readiness check (slice D), held acts (slice F: the stand-in raises
//! the ask for a merge the core refused) and the sign-in (slice G).
//!
//! It needs `gh`, `glab`, `git`, `openssl` and `node`, so it runs only behind the `forge-wiring`
//! feature, on the runners that have them (`.github/workflows/one-login-e2e.yml`).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use data_encoding::BASE64;
use purlis_core::forge::proxy::{
    AllowedRepo, ChatId, Credential, CredentialError, CredentialRequest, CredentialSource,
    ForgeProxy, Forwarded, Inbound, Kind, NoObjects, Outcome, Refusal, RequestHead, TokenKind,
    Verdict,
};
use purlis_core::secrets::keyring::Secret;
use serde_json::{Value, json};

/// The person's credentials as the stand-in keyring pushes them (slice C's job): host to token.
#[derive(Clone, Default)]
struct Tokens(Arc<Mutex<HashMap<String, String>>>);

impl CredentialSource for Tokens {
    fn credential_for(&self, request: &CredentialRequest) -> Result<Credential, CredentialError> {
        let map = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        map.get(&request.host)
            .map(|token| Credential {
                token_kind: TokenKind::Oauth,
                secret: Secret::new(token.clone()),
            })
            .ok_or(CredentialError::Unavailable)
    }
}

struct Bridge {
    proxy: ForgeProxy,
    tokens: Tokens,
    forwarded: Mutex<HashMap<u64, Forwarded>>,
    next: AtomicU64,
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_default()
}

fn port(v: &Value) -> u16 {
    u16::try_from(v["port"].as_u64().unwrap_or(443)).unwrap_or(443)
}

fn header_pairs(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .map(|list| {
            list.iter()
                .filter_map(|p| {
                    let p = p.as_array()?;
                    Some((
                        p.first()?.as_str()?.to_string(),
                        p.get(1)?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn repo_of(r: &Value) -> AllowedRepo {
    let kind = if text(r, "kind") == "github" {
        Kind::GitHub
    } else {
        Kind::GitLab
    };
    let mut repo = AllowedRepo::new(
        kind,
        text(r, "host"),
        port(r),
        text(r, "path"),
        text(r, "default_branch"),
    )
    .expect("a repo the scenario recorded is a valid address");
    if let Some(id) = r["node_id"].as_str() {
        repo = repo.with_node_id(id);
    }
    if let Some(id) = r["project_id"].as_u64() {
        repo = repo.with_forge_id(purlis_core::forge::proxy::ForgeRepoId::for_a_test(id));
    }
    repo
}

impl Bridge {
    fn handle(&self, chat: &ChatId, msg: &Value) -> Value {
        let head = RequestHead::new(
            text(msg, "method"),
            text(msg, "host"),
            port(msg),
            text(msg, "target"),
        );
        let headers = header_pairs(&msg["headers"]);
        let body = BASE64
            .decode(text(msg, "body").as_bytes())
            .unwrap_or_default();
        let inbound = Inbound {
            head: &head,
            headers: &headers,
            body: &body,
        };
        match self.proxy.handle(chat, &inbound) {
            Outcome::Refused(refusal) => json!({
                "outcome": "refused",
                "refusal": format!("{refusal:?}"),
            }),
            Outcome::Forward(forwarded) => {
                let id = self.next.fetch_add(1, Ordering::SeqCst);
                let reply = json!({
                    "outcome": "forward",
                    "id": id,
                    "upstream": forwarded.upstream,
                    "headers": forwarded.headers,
                    "body": forwarded.body.as_ref().map(|b| BASE64.encode(b)),
                    "class": format!("{:?}", forwarded.class),
                    "credentialed": forwarded.credentialed,
                });
                self.forwarded
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(id, forwarded);
                reply
            }
        }
    }

    fn answer(&self, msg: &Value) -> Value {
        let chat = ChatId::new(text(msg, "chat"));
        match text(msg, "op") {
            "credential" => {
                self.tokens
                    .0
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(
                        text(msg, "host").to_string(),
                        text(msg, "token").to_string(),
                    );
                json!({ "ok": true })
            }
            "enrol" => {
                let repos = msg["repos"]
                    .as_array()
                    .map(|list| list.iter().map(repo_of).collect())
                    .unwrap_or_default();
                json!({ "sentinel": self.proxy.enrol(&chat, repos).expose() })
            }
            "advertised" => {
                let branches = msg["branches"]
                    .as_array()
                    .map(|b| {
                        b.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                self.proxy.note_advertised(
                    &chat,
                    text(msg, "host"),
                    text(msg, "repo_path"),
                    branches,
                );
                json!({ "ok": true })
            }
            "confirm" => {
                let id = msg["id"].as_u64().unwrap_or_default();
                let held = self
                    .forwarded
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                if let Some(forwarded) = held.get(&id) {
                    self.proxy.confirm_push(&chat, forwarded);
                }
                json!({ "ok": true })
            }
            "handle" => self.handle(&chat, msg),
            other => json!({ "error": format!("no such op {other}") }),
        }
    }
}

/// One JSON line in, one JSON line out, per connection.
fn serve(listener: &TcpListener, bridge: &Arc<Bridge>) {
    for stream in listener.incoming().flatten() {
        let bridge = Arc::clone(bridge);
        std::thread::spawn(move || {
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            let reply = match serde_json::from_str::<Value>(&line) {
                Ok(msg) => bridge.answer(&msg),
                Err(e) => json!({ "error": e.to_string() }),
            };
            let mut out: &TcpStream = &stream;
            let _ = writeln!(out, "{reply}");
        });
    }
}

#[test]
fn a_person_signs_in_once_and_a_chat_works_the_forge_through_the_real_policy() {
    purlis_core::unsteered!();
    let tokens = Tokens::default();
    let bridge = Arc::new(Bridge {
        proxy: ForgeProxy::new(Box::new(tokens.clone()), Box::new(NoObjects)),
        tokens,
        forwarded: Mutex::new(HashMap::new()),
        next: AtomicU64::new(1),
    });
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port for the decisions");
    let addr = listener.local_addr().expect("its address");
    let serving = Arc::clone(&bridge);
    std::thread::spawn(move || serve(&listener, &serving));

    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = purlis_core::forklock::output(
        Command::new("node")
            .arg("tests/one-login-e2e/run.mjs")
            .env("PURLIS_DECIDE_ADDR", addr.to_string())
            .current_dir(&repo),
    )
    .expect("node runs the scenario");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the scenario failed ({}):\n{stdout}\n{stderr}",
        out.status
    );
    assert!(
        stdout.contains("every step passed"),
        "the scenario did not say every step passed:\n{stdout}"
    );

    // Where the core refused something the scenario needs, the stand-in judged it and said so (`GAP:`).
    // Only GraphQL may be a gap: `gh` asks for `repository.parent` and introspection, which the core
    // refuses today (a finding for slice A). A gap of any other kind is a failure, and when none is left
    // the stand-in's GraphQL judge in `seams.mjs` is to be removed.
    let gaps: Vec<&str> = stdout.lines().filter(|l| l.starts_with("GAP: ")).collect();
    for gap in &gaps {
        assert!(
            gap.starts_with("GAP: Graphql"),
            "the core refused something that is not GraphQL, and the scenario needs it: {gap}"
        );
    }
    println!("{} GraphQL gap kind(s) in slice A: {gaps:#?}", gaps.len());

    // What the core itself recorded: the policy's own account of the run.
    let audit = bridge.proxy.audit(&ChatId::new("e2e-chat"));
    for wanted in [Refusal::Merge, Refusal::ProtectedRef, Refusal::OtherRepo] {
        assert!(
            audit
                .iter()
                .any(|r| matches!(&r.verdict, Verdict::Refused(got) if *got == wanted)),
            "the core never refused with {wanted:?}: {audit:#?}"
        );
    }
    assert!(
        audit.iter().any(|r| matches!(r.verdict, Verdict::Allowed)),
        "the core allowed nothing"
    );
    let recorded = format!("{audit:?}");
    let tokens = bridge
        .tokens
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    assert!(
        !tokens.is_empty(),
        "the scenario never gave the core a credential"
    );
    for token in tokens.values() {
        assert!(
            !recorded.contains(token.as_str()),
            "the audit holds a token"
        );
    }
}
