// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One start of the app asks the Keychain about a forge sign-in once per process** (#2656).
//!
//! Each forge caller used to go to the keyring itself: every listing asked whether the item was
//! there, and every request read it again. On macOS each of those calls can make the Keychain
//! ask the person, so starting the app with a few projects asked several times for one item.
//! Each process now asks once and keeps the answer in its own memory for as long as it runs.
//!
//! The keyring here is the fenced build's stub, which counts what it is asked: each read of an
//! item's value, and each question about whether an item is there.
//!
//! The test that hands this process's sign-ins over, as the app does once, runs in a process of
//! its own ([`crate::alone`]): it was a test binary of its own until #3613.

use std::path::Path;
use std::sync::Arc;

use purlis_core::forge::accounts::{self, ForgeAccount, Pasted};
use purlis_core::forge::http::TokenSource;
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::refresh::Refresher;
use purlis_core::forge::route::HostScope;
use purlis_core::forge::transport::Transport;
use purlis_core::forge::{Forge, ForgeError, Kind, gitcredential, signin};
use purlis_core::secrets::keyring::{self, FileStore, STUB_FILE, Store};
use secrecy::ExposeSecret;

/// Not shaped like any forge's token: the commit hook scans for those.
const TOKEN: &str = "kept-for-one-start";

/// A config home whose account an earlier run of the app signed in: written straight to the
/// stub, so nothing of it is in this process's memory, as at a real start.
fn signed_in_by_an_earlier_run() -> (tempfile::TempDir, ForgeAccount) {
    let home = tempfile::tempdir().unwrap();
    let earlier = FileStore::at(stub(home.path()));
    let recording = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/accounts/github/token_check.json");
    let recorded = Arc::new(Recorded::parse(&std::fs::read_to_string(recording).unwrap()).unwrap());
    let reach = |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
        Ok(recorded.clone() as Arc<dyn Transport>)
    };
    let account = accounts::sign_in_with_token(
        &HostScope::for_a_test(),
        home.path(),
        &earlier,
        &reach,
        Kind::GitHub,
        "github.com",
        Pasted::new(TOKEN.to_string()),
    )
    .expect("the recorded forge takes the token");
    (home, account)
}

fn stub(home: &Path) -> std::path::PathBuf {
    purlis_core::machine::dir(home).join(STUB_FILE)
}

/// What the keychain was asked so far: reads of the value, and questions whether it is there.
fn asked(home: &Path) -> (usize, usize) {
    (
        keyring::stub_reads(&stub(home)),
        keyring::stub_asks(&stub(home)),
    )
}

#[test]
fn one_start_with_three_projects_asks_the_keychain_once() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let (home, account) = signed_in_by_an_earlier_run();
    let before = asked(home.path());

    // The app starts: it hands its sign-ins over, the window draws whether it holds one and
    // lists the accounts, each of three projects' first fetch names the helper, and the window's
    // first forge calls each take the token.
    let scope = HostScope::for_a_test();
    signin::hand_over(HostScope::for_a_test(), home.path());
    assert!(signin::holds_a_sign_in());
    assert_eq!(signin::accounts().unwrap().len(), 1);
    let exe = Path::new("/Applications/purlis.app/Contents/MacOS/purlis");
    for _project in 0..3 {
        let named = gitcredential::as_the_window(&scope, exe, home.path(), || {
            gitcredential::for_the_window("github.com")
        });
        assert!(
            named.is_some(),
            "a fetch to github.com names purlis's helper"
        );
    }
    let refresher = Refresher::new(&scope, home.path());
    for _request in 0..4 {
        let token = refresher.token(&account).unwrap();
        assert_eq!(token.expose_secret(), TOKEN);
    }

    let after = asked(home.path());
    let reads = after.0 - before.0;
    let asks = after.1 - before.1;
    assert_eq!(
        reads + asks,
        1,
        "one start asks the keychain once: one read answers both whether the item is there and \
         its token ({reads} reads, {asks} searches)"
    );
}

#[test]
fn the_credential_helper_reads_the_token_once_and_asks_nothing_else() {
    purlis_core::unsteered!();
    let (home, account) = signed_in_by_an_earlier_run();
    let before = asked(home.path());

    let args = [
        "--config".to_string(),
        home.path().display().to_string(),
        "--account".to_string(),
        account.id.clone(),
        "get".to_string(),
    ];
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = gitcredential::serve(
        &args,
        &mut "protocol=https\nhost=github.com\n\n".as_bytes(),
        &mut out,
        &mut err,
        &|_| None,
    );
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert!(String::from_utf8_lossy(&out).contains("password="));

    let after = asked(home.path());
    assert_eq!(
        (after.0 - before.0, after.1 - before.1),
        (1, 0),
        "git's helper is a process of its own: one read, and no separate question whether the \
         item is there"
    );
}

/// Over HTTP to a stand-in forge on this machine's loopback: a token the forge refuses is
/// forgotten, and the next request reads the item again, finding the one the window put there
/// since. Needs a socket, so it first runs on CI.
#[test]
fn a_token_the_forge_refuses_is_forgotten_and_the_next_request_reads_the_item_again() {
    purlis_core::unsteered!();
    use purlis_core::forge::http::{ApiRoot, Http};
    use purlis_core::forge::transport::Call;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let (home, account) = signed_in_by_an_earlier_run();
    let replaced = ["replaced", "by", "the", "window"].join("-");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = rt.block_on(MockServer::start());
    let refused = format!("Bearer {TOKEN}");
    let taken = format!("Bearer {replaced}");
    rt.block_on(async {
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", refused.as_str()))
            .respond_with(
                ResponseTemplate::new(401).set_body_string(r#"{"message":"Bad credentials"}"#),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", taken.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"login":"octocat"}"#))
            .expect(1)
            .mount(&server)
            .await;
    });
    let scope = HostScope::for_a_test();
    let tokens = Arc::new(signin::KeyringToken::new(
        account.clone(),
        Arc::new(Refresher::new(&scope, home.path())),
    ));
    let http = Http::new(Kind::GitHub, ApiRoot::at(&server.uri()), tokens).unwrap();
    let forge = Forge::default_of(Kind::GitHub);
    let user = Call::get("user", std::time::Duration::from_secs(10));

    let first = http.send(&forge, &user).unwrap();
    assert_eq!(first.status, Some(401));
    // The window signed in again meanwhile, in its own process: the item holds a new token.
    FileStore::at(stub(home.path()))
        .set(&accounts::service(&account), "token", &replaced)
        .unwrap();
    let second = http.send(&forge, &user).unwrap();
    assert_eq!(second.status, Some(200));
    rt.block_on(server.verify());
}
