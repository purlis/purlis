// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window's forge calls use the person's purlis sign-in, and need no `gh` or `glab`**
//! (#1816, ADR 0070 §4, ADR 0077 §7), against the `forge_contract` recordings.
//!
//! Each case signs in with a token through the account store (the forge answering from
//! `tests/forge_contract/accounts/`), hands the sign-ins over as the app does at start, and runs
//! what the window runs: discover from the Doctor, the workspace repo picker, and the save
//! button's pull request. The native route is the recording itself, handed each request's token
//! source; it checks the token it is handed is the one signed in, read from the keyring for that
//! request. The CLI route stands for a machine with neither CLI: it answers every request and
//! every login check as a missing `gh` does, and counts each one, so a case that reached for the
//! CLI fails.
//!
//! The same scene shows the rule's other side: a `purlis` command and a chat, in this very
//! process, go to the CLI and never read the token; and a host with no account falls back to
//! the CLI, whose refusal names the purlis sign-in as the fix.
//!
//! The sign-ins are this process's, so the cases take turns.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use purlis_core::forge::accounts::{self, Pasted};
use purlis_core::forge::http::TokenSource;
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::route::HostScope;
use purlis_core::forge::signin;
use purlis_core::forge::transport::{Call, NoAnswer, Reply, Transport};
use purlis_core::forge::{Caller, Forge, ForgeError, Kind};
use purlis_core::planegit::Trigger;
use secrecy::ExposeSecret;

/// The token every case pastes. Not shaped like any forge's token: the commit hook scans for
/// those.
const TOKEN: &str = "pasted-for-the-recorded-forge";

/// What a missing `gh` says.
const NO_GH: &str = "purlis could not find gh on PATH — install it and log in (`gh auth login`)";

/// Every test here hands this process's sign-ins over, which is the whole process's state: each
/// runs in a process of its own ([`crate::alone`], #3613), and this orders them where nextest
/// or a re-run puts several in one.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn recording(path: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/forge_contract")
            .join(path),
    )
    .unwrap()
}

/// The CLI route of a machine with no `gh`: every request and login check refused, counted.
#[derive(Default)]
struct NoCli(AtomicUsize);
impl Transport for NoCli {
    fn send(&self, _forge: &Forge, _call: &Call) -> Result<Reply, NoAnswer> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(NoAnswer::Missing(NO_GH.into()))
    }
    fn check_auth(&self, _forge: &Forge) -> Result<(), ForgeError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ForgeError::transport(NO_GH))
    }
}

/// The native route: the recording, answering only a request sent with the signed-in token.
struct Native {
    tokens: Arc<dyn TokenSource>,
    recorded: Arc<Recorded>,
    read: Arc<AtomicUsize>,
}
impl Transport for Native {
    fn send(&self, forge: &Forge, call: &Call) -> Result<Reply, NoAnswer> {
        let token = self
            .tokens
            .token()
            .map_err(|e| NoAnswer::Refused(e.to_string()))?;
        assert_eq!(token.expose_secret(), TOKEN, "the request's token");
        self.read.fetch_add(1, Ordering::SeqCst);
        self.recorded.send(forge, call)
    }
    fn check_auth(&self, forge: &Forge) -> Result<(), ForgeError> {
        self.recorded.check_auth(forge)
    }
}

struct Machine {
    _turn: MutexGuard<'static, ()>,
    home: tempfile::TempDir,
    recorded: Arc<Recorded>,
    cli: Arc<NoCli>,
    /// How many requests read the token from the keyring.
    read: Arc<AtomicUsize>,
}

impl Machine {
    /// A machine whose forge answers `case`, signed in to github.com when `signed_in`, its
    /// sign-ins handed over as the app hands them over.
    fn new(case: &str, signed_in: bool) -> Machine {
        let turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let home = tempfile::tempdir().unwrap();
        if signed_in {
            let check =
                Arc::new(Recorded::parse(&recording("accounts/github/token_check.json")).unwrap());
            let reach = move |_: &Forge,
                              _: Arc<dyn TokenSource>|
                  -> Result<Arc<dyn Transport>, ForgeError> {
                Ok(check.clone() as Arc<dyn Transport>)
            };
            let keyring = accounts::keyring_at(home.path());
            accounts::sign_in_with_token(
                &HostScope::for_a_test(),
                home.path(),
                keyring.as_ref(),
                &reach,
                Kind::GitHub,
                "github.com",
                Pasted::new(TOKEN.to_string()),
            )
            .expect("signed in");
        }
        let recorded = Arc::new(Recorded::parse(&recording(case)).unwrap());
        let cli = Arc::new(NoCli::default());
        let read = Arc::new(AtomicUsize::new(0));
        let (served, counted) = (recorded.clone(), read.clone());
        signin::hand_over_for_a_test(
            HostScope::for_a_test(),
            home.path(),
            Arc::new(move |tokens| {
                Arc::new(Native {
                    tokens,
                    recorded: served.clone(),
                    read: counted.clone(),
                }) as Arc<dyn Transport>
            }),
            cli.clone(),
        );
        Machine {
            _turn: turn,
            home,
            recorded,
            cli,
            read,
        }
    }

    fn cli_asked(&self) -> usize {
        self.cli.0.load(Ordering::SeqCst)
    }

    fn token_read(&self) -> usize {
        self.read.load(Ordering::SeqCst)
    }

    /// Every recorded exchange was asked for, over the native route, and nothing of the CLI.
    fn all_native(&self) {
        assert!(
            self.recorded.unspent().is_empty(),
            "{:?}",
            self.recorded.unspent()
        );
        assert_eq!(self.cli_asked(), 0, "the CLI was asked");
        assert!(self.token_read() > 0, "no request read the token");
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        signin::take_back();
    }
}

/// A project declaring the GitHub organisation `acme`.
fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[[forge]]\nkind = \"github\"\nowner = \"acme\"\n",
    )
    .unwrap();
    (dir, root)
}

fn names(listed: &[serde_json::Value]) -> Vec<String> {
    let mut names: Vec<String> = listed
        .iter()
        .filter_map(|r| r.get("name").and_then(|n| n.as_str()).map(str::to_string))
        .collect();
    names.sort();
    names
}

#[test]
fn discover_from_the_window_lists_the_owners_repos_over_the_sign_in_with_no_cli() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new("github/owned.json", true);
    let (_dir, root) = project();
    let mut said = Vec::new();
    let code = purlis_core::repocmd::discover::discover_as(
        &root,
        purlis_core::repocmd::discover::Options {
            no_probe: true,
            no_docs: true,
        },
        &Caller::window(),
        &mut |line| said.push(line.to_string()),
    );
    assert_eq!(code, 0, "{said:?}");
    machine.all_native();
    let group = purlis_core::forge::group_of(&purlis_core::forge::load_config(&root).unwrap(), 0);
    let listed =
        purlis_core::inventory::listed(&purlis_core::inventory::load(&root, &group).unwrap());
    assert_eq!(names(&listed), ["api", "web"], "{said:?}");
}

#[test]
fn the_repo_picker_lists_what_the_sign_in_reaches_with_no_cli() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new("github/reachable.json", true);
    let (_dir, root) = project();
    let reached = purlis_core::repocmd::reachable::reachable(&root).unwrap();
    assert!(reached.trouble.is_empty(), "{:?}", reached.trouble);
    assert_eq!(names(&reached.repos), ["api"]);
    machine.all_native();
}

#[test]
fn the_save_button_opens_its_pull_request_over_the_sign_in_with_no_cli() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new("github/open_or_update.json", true);
    let opened = Forge::default_of(Kind::GitHub)
        .backend()
        .open_or_update(
            &Trigger::Manual.caller(),
            "acme/api",
            "charter/save",
            "main",
            "Save",
            "<!-- charter-save -->",
        )
        .unwrap();
    assert_eq!(opened.pr.number, 12);
    assert_eq!(opened.pr.url, "https://github.com/acme/api/pull/12");
    assert!(opened.ours);
    machine.all_native();
    assert_eq!(
        machine.token_read(),
        2,
        "the token is read once per request"
    );
}

#[test]
fn a_command_a_trigger_and_a_chat_in_the_windows_own_process_never_read_the_sign_in() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new("github/open_or_update.json", true);
    for caller in [
        Trigger::Cli.caller(),
        Trigger::Quiet.caller(),
        Caller::chat("01J9CHAT"),
        Caller::mcp("01J9CHAT"),
    ] {
        let got = Forge::default_of(Kind::GitHub).backend().open_or_update(
            &caller,
            "acme/api",
            "charter/save",
            "main",
            "Save",
            "<!-- charter-save -->",
        );
        assert!(got.is_err(), "{caller:?} was answered: {got:?}");
    }
    assert_eq!(machine.token_read(), 0, "the sign-in token was read");
    assert!(machine.cli_asked() >= 4, "the CLI was not asked");
}

#[test]
fn with_no_account_the_window_falls_back_to_the_cli_and_names_the_sign_in() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new("github/reachable.json", false);
    let (_dir, root) = project();
    let reached = purlis_core::repocmd::reachable::reachable(&root).unwrap();
    assert!(reached.repos.is_empty());
    let [trouble] = reached.trouble.as_slice() else {
        panic!("{:?}", reached.trouble)
    };
    assert!(trouble.said.contains(signin::SIGN_IN_AT), "{trouble:?}");
    assert!(trouble.said.contains("could not find gh"), "{trouble:?}");
    assert!(machine.cli_asked() > 0, "the CLI was not asked");
    assert_eq!(machine.token_read(), 0);

    let forge = Forge::default_of(Kind::GitHub);
    let refused = signin::ready(&forge, &Caller::window()).unwrap_err();
    assert!(refused.said().contains(signin::SIGN_IN_AT), "{refused}");
}

#[test]
fn a_token_gone_from_the_keyring_is_refused_with_sign_in_again_and_never_sent() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new("github/open_or_update.json", true);
    let keyring = accounts::keyring_at(machine.home.path());
    let account = accounts::list(machine.home.path(), keyring.as_ref())
        .unwrap()
        .remove(0);
    assert!(
        keyring
            .delete(&accounts::service(&account), "token")
            .unwrap()
    );
    let got = Forge::default_of(Kind::GitHub).backend().open_or_update(
        &Caller::window(),
        "acme/api",
        "charter/save",
        "main",
        "Save",
        "<!-- charter-save -->",
    );
    let said = got.unwrap_err().to_string();
    assert!(said.contains("Sign in again"), "{said}");
    assert_eq!(machine.token_read(), 0);
    assert_eq!(
        machine.cli_asked(),
        0,
        "a failed native call is never retried on the CLI"
    );
}

/// Over the real native transport: the pull request is opened with the keyring's token in the
/// request's `Authorization` header, sent to a recorded server on this machine's loopback.
/// Needs a socket, so it first runs on CI.
#[test]
fn over_https_the_pull_request_carries_the_keyrings_token_as_its_bearer() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    purlis_core::unsteered!();
    let machine = Machine::new("github/open_or_update.json", true);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = rt.block_on(MockServer::start());
    let bearer = format!("Bearer {TOKEN}");
    rt.block_on(async {
        Mock::given(method("GET"))
            .and(path("/repos/acme/api/pulls"))
            .and(query_param("head", "acme:charter/save"))
            .and(header("authorization", bearer.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/acme/api/pulls"))
            .and(header("authorization", bearer.as_str()))
            .respond_with(ResponseTemplate::new(201).set_body_string(
                r#"{"number": 12, "html_url": "https://github.com/acme/api/pull/12", "state": "open"}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;
    });
    signin::hand_over_at_root_for_a_test(
        HostScope::for_a_test(),
        machine.home.path(),
        purlis_core::forge::http::ApiRoot::at(&server.uri()),
        machine.cli.clone(),
    );
    let opened = Forge::default_of(Kind::GitHub)
        .backend()
        .open_or_update(
            &Trigger::Manual.caller(),
            "acme/api",
            "charter/save",
            "main",
            "Save",
            "<!-- charter-save -->",
        )
        .unwrap();
    assert_eq!(opened.pr.number, 12);
    assert_eq!(machine.cli_asked(), 0, "the CLI was asked");
    rt.block_on(server.verify());
}
