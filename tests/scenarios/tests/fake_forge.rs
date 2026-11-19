// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The fake forge** (T1.1-50, #6897): purlis's own forge client reaches it and merges on it, it
//! refuses a merge as a ruleset does when a test asks, it speaks HTTPS under the test CA, and it
//! records the credential every request presented.
//!
//! These bind local ports, so they run in CI (`sandbox-scenarios-linux`, and the labelled macOS
//! run); a sandbox that refuses a bind fails them, never passes them. The two clients that are
//! not purlis's, `gh` and `glab` from inside a chat's sandbox, are the app's scenario cells
//! (`app/src-tauri/src/scenarios/forge.rs`).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;

use purlis_core::forge::http::{ApiRoot, TokenSource};
use purlis_core::forge::pr::{MergeAs, MergedAt, Pr};
use purlis_core::forge::route::{HostScope, Resolver, SignIn};
use purlis_core::forge::{Account, Caller, Forge, ForgeBackend, ForgeError, Kind};
use purlis_scenarios::net::{FORGE_HEADER, FakeForge, MERGE_SHA, Refusal, TestCa};
use secrecy::SecretString;

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

/// The token the account signed in with: what a cell says purlis's own client sent.
const TOKEN: &str = "person-token-9d41";

struct Fixed;
impl TokenSource for Fixed {
    fn token(&self) -> Result<SecretString, ForgeError> {
        Ok(SecretString::from(TOKEN))
    }
}

/// **The account and host seam purlis's forge client reaches a fake forge by** (the ticket's
/// spike, part 2): the client builds `https://<host>/api/v3` for a host and refuses `http`
/// (`forge/http.rs`), except through `ApiRoot::at`, which a fenced build has and a shipped one
/// does not. So the account is `octocat@forge.test`, its resolver's root is the fake's plain
/// HTTP address, and its token source is the sign-in's.
fn the_clients_backend(forge: &FakeForge) -> Box<dyn ForgeBackend> {
    let account = Account {
        kind: Kind::GitHub,
        host: "forge.test".into(),
        login: "octocat".into(),
    };
    let root = format!("http://{}/api/v3", forge.http_addr());
    let resolver = Resolver::new(Kind::GitHub, "forge.test")
        .at_root(ApiRoot::at(&root))
        .signed_in(
            &HostScope::for_a_test(),
            account,
            SignIn {
                tokens: Arc::new(Fixed),
                imported_from_cli: false,
            },
        );
    Forge::default_of(Kind::GitHub).backend_for(Arc::new(resolver))
}

fn the_person() -> Caller {
    Caller::window().as_account(Account {
        kind: Kind::GitHub,
        host: "forge.test".into(),
        login: "octocat".into(),
    })
}

fn how() -> MergeAs {
    MergeAs {
        squash: true,
        title: "Merge it".into(),
        message: "by the test".into(),
    }
}

fn pr() -> Pr {
    Pr {
        number: 7,
        url: String::new(),
    }
}

/// One request over plain HTTP, answered: its status and body.
fn http(addr: SocketAddr, request: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("the fake forge listens");
    stream.write_all(request.as_bytes()).expect("sent");
    let mut said = String::new();
    stream.read_to_string(&mut said).expect("answered");
    let status = said
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status in {said:?}"));
    let body = said.split_once("\r\n\r\n").map_or("", |(_, body)| body);
    (status, body.to_owned())
}

#[test]
fn purlis_forge_client_reaches_the_fake_forge() {
    let forge = FakeForge::start(None).expect("the fake forge");
    forge.add_pull_request("o/r", 7, HEAD);
    let github = the_clients_backend(&forge);

    let merged = github
        .merge_at(&the_person(), "o/r", &pr(), HEAD, &how())
        .expect("the client's merge is answered");
    assert_eq!(merged, MergedAt::Now);
    assert_eq!(
        forge.merged(purlis_scenarios::net::Forge::GitHub, "o/r", 7),
        Some(MERGE_SHA.to_owned()),
        "the fake forge merged what the client asked"
    );

    let seen = forge.requests();
    let merge = seen
        .iter()
        .find(|request| request.method == "PUT" && request.path.ends_with("/pulls/7/merge"))
        .unwrap_or_else(|| panic!("the client's merge was never recorded: {seen:?}"));
    assert_eq!(merge.path, "/api/v3/repos/o/r/pulls/7/merge");
    assert!(
        merge.body.contains(HEAD),
        "the merge names the head it saw: {}",
        merge.body
    );
    assert_eq!(
        merge.credential().map(|c| c.token),
        Some(TOKEN.to_owned()),
        "the token the account signed in with is the one presented"
    );
    assert_eq!(forge.tokens_seen(), vec![TOKEN.to_owned()]);
}

#[test]
fn purlis_forge_client_is_refused_a_moved_head_and_merges_nothing() {
    let forge = FakeForge::start(None).expect("the fake forge");
    forge.add_pull_request("o/r", 7, HEAD);
    let github = the_clients_backend(&forge);
    let stale = "f".repeat(40);
    let refused = github.merge_at(&the_person(), "o/r", &pr(), &stale, &how());
    assert!(refused.is_err(), "a moved head merged: {refused:?}");
    assert_eq!(
        forge.merged(purlis_scenarios::net::Forge::GitHub, "o/r", 7),
        None
    );
    assert!(
        forge.requests().iter().any(|request| request.status == 409),
        "the fake forge never answered 409: {:?}",
        forge.requests()
    );
}

#[test]
fn the_fake_forge_returns_a_ruleset_refusal_on_request() {
    let forge = FakeForge::start(None).expect("the fake forge");
    forge.add_pull_request("o/r", 7, HEAD);
    forge.refuse_merge(
        purlis_scenarios::net::Forge::GitHub,
        "o/r",
        7,
        Refusal::Ruleset,
    );
    let merge = "PUT /api/v3/repos/o/r/pulls/7/merge HTTP/1.1\r\nHost: forge.test\r\n\
         Authorization: Bearer t\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";
    let (status, body) = http(forge.http_addr(), merge);
    assert_eq!(status, 405, "{body}");
    assert!(body.contains("Repository rule violations"), "{body}");
    assert_eq!(
        forge.merged(purlis_scenarios::net::Forge::GitHub, "o/r", 7),
        None,
        "a refused merge merged"
    );

    // The client reads the same refusal as a refusal, not as a merge.
    let github = the_clients_backend(&forge);
    let refused = github.merge_at(&the_person(), "o/r", &pr(), HEAD, &how());
    assert!(refused.is_err(), "{refused:?}");
}

#[test]
fn the_fake_forge_answers_gitlab_merges_and_records_a_private_token() {
    let forge = FakeForge::start(None).expect("the fake forge");
    forge.add_merge_request("group/project", 42, 3, HEAD);
    let merge = |sha: &str| {
        let body = format!("{{\"sha\":\"{sha}\"}}");
        format!(
            "PUT /api/v4/projects/group%2Fproject/merge_requests/3/merge HTTP/1.1\r\n\
             Host: gitlab.test\r\nPRIVATE-TOKEN: glpat-agent\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    };
    assert_eq!(http(forge.http_addr(), &merge(&"0".repeat(40))).0, 409);
    assert_eq!(http(forge.http_addr(), &merge(HEAD)).0, 200);
    assert_eq!(
        forge.merged(purlis_scenarios::net::Forge::GitLab, "group/project", 3),
        Some(MERGE_SHA.to_owned())
    );
    assert_eq!(forge.tokens_seen(), vec!["glpat-agent".to_owned()]);
    let first = &forge.requests()[0];
    assert_eq!(
        first.credential().expect("a credential").header,
        "private-token"
    );
}

#[test]
fn the_fake_forge_speaks_https_under_the_test_ca_and_records_the_name_it_was_asked_for() {
    let ca = TestCa::new().expect("the test CA");
    let forge = FakeForge::start(Some((&ca, &["forge.test"]))).expect("the fake forge");
    let addr = forge.https_addr().expect("an HTTPS listener");

    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca.der()).expect("the CA is a root");
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("protocol versions")
    .with_root_certificates(roots)
    .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from("forge.test").expect("a name");
    let conn = rustls::ClientConnection::new(Arc::new(config), name).expect("a client");
    let mut tls = rustls::StreamOwned::new(conn, TcpStream::connect(addr).expect("connects"));
    tls.write_all(
        b"GET /api/v3/user HTTP/1.1\r\nHost: forge.test\r\nAuthorization: token minted-1\r\n\
          Connection: close\r\n\r\n",
    )
    .expect("sent");
    let mut said = String::new();
    // A close without close_notify is how this server ends; the answer is already read.
    let _ = tls.read_to_string(&mut said);
    assert!(said.starts_with("HTTP/1.1 200"), "{said}");
    assert!(said.to_ascii_lowercase().contains(FORGE_HEADER), "{said}");

    let seen = forge.requests();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].sni.as_deref(), Some("forge.test"));
    assert_eq!(
        seen[0].credential().map(|c| c.token),
        Some("minted-1".to_owned())
    );
}

#[test]
fn a_request_with_no_credential_is_recorded_and_refused_401() {
    let forge = FakeForge::start(None).expect("the fake forge");
    forge.add_pull_request("o/r", 7, HEAD);
    let (status, _) = http(
        forge.http_addr(),
        "GET /repos/o/r/pulls/7 HTTP/1.1\r\nHost: github.localhost\r\nConnection: close\r\n\r\n",
    );
    assert_eq!(status, 401);
    let seen = forge.requests();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].credential(), None);
    assert!(forge.tokens_seen().is_empty());
}

#[test]
fn a_second_token_on_one_request_is_listed_and_fails_an_assertion_on_the_first_alone() {
    let forge = FakeForge::start(None).expect("the fake forge");
    forge.add_merge_request("g/p", 42, 7, HEAD);
    // The chat's token in the header the client uses, the person's beside it in the query.
    let (status, _) = http(
        forge.http_addr(),
        "GET /api/v4/projects/g%2Fp/merge_requests/7?private_token=person-token HTTP/1.1\r\n\
         Host: git.corp.test\r\nPRIVATE-TOKEN: chat-token\r\nAuthorization: Bearer other-token\r\n\
         Connection: close\r\n\r\n",
    );
    assert_eq!(status, 200);
    assert_eq!(
        forge.tokens_seen(),
        ["chat-token", "other-token", "person-token"],
        "every credential of the request is listed, in every place it came in"
    );
    assert_ne!(
        forge.tokens_seen(),
        ["chat-token"],
        "a cell asserting the chat's token alone fails on a request that carried another"
    );
}
