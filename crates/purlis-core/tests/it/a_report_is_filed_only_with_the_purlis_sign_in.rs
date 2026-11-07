// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis report` searches and files through the forge seam's work-item area, with purlis's
//! own sign-in, and never through `gh`** (#806, ADR 0059, ADR 0070 §4).
//!
//! Every case drives the Reports module through its interface (#5686): evidence from a source in,
//! an outcome out. The route is the module's to choose, never the case's.
//!
//! Each case signs in to github.com with a token through the account store, hands the sign-ins
//! over as the app does at start, and asks the tracker as the window does. The native route is a
//! recording of GitHub's answers, handed each request's token source, which checks the token it
//! is handed is the one signed in. The CLI route counts every request and login check it is
//! asked, so a report that reached for `gh`, under whatever login it holds, fails the case.
//!
//! The other side of the rule, in the same scene: a `purlis` command and a chat hold no sign-in,
//! and a window with no account has none to file with. Each is refused before anything is sent,
//! with neither route asked.
//!
//! The sign-ins are this process's, so the cases take turns.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use purlis_core::forge::accounts::{self, Pasted};
use purlis_core::forge::http::TokenSource;
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::route::HostScope;
use purlis_core::forge::signin;
use purlis_core::forge::transport::{Call, NoAnswer, Reply, Transport};
use purlis_core::forge::{Caller, Failure, Forge, ForgeError, Kind};
use purlis_core::report::{self, Evidence, Hit, Known, Outcome, Refusal, Reports, Route};
use secrecy::ExposeSecret;
use serde_json::json;

/// The token every case pastes. Not shaped like any forge's token: the commit hook scans for
/// those.
const TOKEN: &str = "pasted-for-the-recorded-tracker";

/// Every test here hands this process's sign-ins over, which is the whole process's state: each
/// runs in a process of its own ([`crate::alone`], #3613), and this orders them where nextest
/// or a re-run puts several in one.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// The CLI route: every request and login check refused, and counted.
#[derive(Default)]
struct CountedCli(AtomicUsize);
impl Transport for CountedCli {
    fn send(&self, _forge: &Forge, _call: &Call) -> Result<Reply, NoAnswer> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(NoAnswer::Missing("gh was asked".into()))
    }
    fn check_auth(&self, _forge: &Forge) -> Result<(), ForgeError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ForgeError::transport("gh was asked"))
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
    _home: tempfile::TempDir,
    recorded: Arc<Recorded>,
    cli: Arc<CountedCli>,
    read: Arc<AtomicUsize>,
}

impl Machine {
    /// A machine whose GitHub answers `exchanges`, signed in to github.com when `signed_in`, its
    /// sign-ins handed over as the app hands them over.
    fn new(exchanges: serde_json::Value, signed_in: bool) -> Machine {
        let turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let home = tempfile::tempdir().unwrap();
        if signed_in {
            let check = std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/forge_contract/accounts/github/token_check.json"),
            )
            .unwrap();
            let check = Arc::new(Recorded::parse(&check).unwrap());
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
        let text = json!({"source": "GitHub REST API version 2022-11-28, docs.github.com/en/rest, \
                                     search: Search issues and pull requests; issues: Create an \
                                     issue",
                          "exchanges": exchanges});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        let cli = Arc::new(CountedCli::default());
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
            _home: home,
            recorded,
            cli,
            read,
        }
    }

    /// Nothing was sent on either route.
    fn nothing_sent(&self) {
        assert_eq!(self.cli.0.load(Ordering::SeqCst), 0, "gh was asked");
        assert_eq!(self.read.load(Ordering::SeqCst), 0, "the token was read");
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        signin::take_back();
    }
}

fn evidence() -> Evidence {
    Evidence::PersonDraft {
        kind: report::Kind::Bug,
        text: "Open the picker with no repos.".into(),
        title: Some("The picker crashes on an empty list".into()),
    }
}

/// The Reports module knowing nothing of this machine, so the draft is the same on every one.
fn reports() -> Reports {
    Reports::new(Known::default())
}

/// What the module drafts of [`evidence`] for `source`, and the route it chose.
fn drafted(source: &Caller) -> (report::Draft, Route) {
    match reports().take(evidence(), source) {
        Outcome::Drafted { draft, route } => (draft, route),
        other => panic!("not drafted: {other:?}"),
    }
}

/// The refusal a route gave, and that it was a sign-in refusal naming where to sign in.
fn refused_for_the_sign_in(outcome: Outcome, said: &str) {
    match outcome {
        Outcome::Refused(Refusal::NotFiled { why, .. }) => {
            assert_eq!(why.failure(), &Failure::Auth);
            assert!(why.said().contains(said), "{why}");
        }
        other => panic!("not refused for the sign-in: {other:?}"),
    }
}

fn issue(number: u64, title: &str, state: &str) -> serde_json::Value {
    json!({"id": 5000 + number, "node_id": format!("I_kwDOPurlis{number}"), "number": number,
           "title": title, "state": state, "labels": [],
           "html_url": format!("https://github.com/purlis/purlis/issues/{number}")})
}

fn searched() -> serde_json::Value {
    json!({"call": {"endpoint": {"rest": {"method": null, "path":
              "search/issues?q=repo%3Apurlis%2Fpurlis%20is%3Aissue%20The%20picker%20crashes%20empty%20list&per_page=5"}},
              "fields": []},
           "reply": {"code": 0, "err": "", "out": json!({"total_count": 1,
              "incomplete_results": false,
              "items": [issue(41, "Picker crash", "closed")]}).to_string()}})
}

fn filed(body: &str) -> serde_json::Value {
    json!({"call": {"endpoint": {"rest": {"method": "POST", "path": "repos/purlis/purlis/issues"}},
                    "fields": [{"text": ["title", "The picker crashes on an empty list"]},
                               {"text": ["body", body]}]},
           "reply": {"code": 0, "err": "",
                     "out": issue(42, "The picker crashes on an empty list", "open").to_string()}})
}

#[test]
fn the_window_searches_and_files_a_report_with_the_purlis_sign_in_and_never_gh() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let (shown, _) = drafted(&Caller::window());
    let machine = Machine::new(json!([searched(), filed(&shown.body)]), true);
    let caller = Caller::window();
    let (draft, route) = drafted(&caller);
    assert_eq!(
        route,
        Route::AsTheReporter,
        "the person in the window, signed in"
    );
    assert!(
        draft
            .body
            .starts_with("Open the picker with no repos.\n\n---\npurlis "),
        "{}",
        draft.body
    );

    let hits = reports().duplicates(&draft, &caller).unwrap();
    assert_eq!(
        hits,
        [Hit {
            number: 41,
            title: "Picker crash".into(),
            state: "closed".into(),
            url: "https://github.com/purlis/purlis/issues/41".into(),
        }]
    );
    assert_eq!(
        reports().file(evidence(), &caller, &draft.digest()),
        Outcome::Sent {
            address: "https://github.com/purlis/purlis/issues/42".into()
        }
    );

    assert!(
        machine.recorded.unspent().is_empty(),
        "{:?}",
        machine.recorded.unspent()
    );
    assert_eq!(machine.cli.0.load(Ordering::SeqCst), 0, "gh was asked");
    assert_eq!(
        machine.read.load(Ordering::SeqCst),
        2,
        "each request read the token"
    );
}

#[test]
fn a_draft_other_than_the_one_shown_is_never_filed() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new(json!([]), true);
    let (draft, _) = drafted(&Caller::window());
    let other = "0".repeat(12);
    assert_ne!(draft.digest(), other);
    assert_eq!(
        reports().file(evidence(), &Caller::window(), &other),
        Outcome::Refused(Refusal::NotTheShownDraft {
            digest: draft.digest()
        })
    );
    machine.nothing_sent();
}

#[test]
fn a_purlis_command_and_a_chat_are_refused_before_anything_is_sent() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new(json!([]), true);
    for caller in [
        Caller::command(),
        Caller::chat("c1"),
        Caller::mcp("c1"),
        Caller::trigger(),
    ] {
        let (draft, route) = drafted(&caller);
        match route {
            Route::ByHand { why, link, .. } => {
                assert_eq!(why.failure(), &Failure::Auth, "{caller:?}");
                assert!(why.said().contains(signin::SIGN_IN_AT), "{why}");
                assert!(
                    link.starts_with("https://github.com/purlis/purlis/issues/new?title="),
                    "{link}"
                );
            }
            other => panic!("{caller:?} was routed {other:?}"),
        }
        refused_for_the_sign_in(
            reports().file(evidence(), &caller, &draft.digest()),
            signin::SIGN_IN_AT,
        );
    }
    machine.nothing_sent();
}

#[test]
fn a_window_with_no_purlis_sign_in_is_refused_and_never_falls_back_to_gh() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new(json!([]), false);
    let (draft, _) = drafted(&Caller::window());
    refused_for_the_sign_in(
        reports().file(evidence(), &Caller::window(), &draft.digest()),
        "GitHub on github.com",
    );
    machine.nothing_sent();
}

#[test]
fn where_no_sign_in_was_handed_over_a_report_is_refused() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new(json!([]), true);
    signin::take_back();
    let (draft, _) = drafted(&Caller::window());
    refused_for_the_sign_in(
        reports().file(evidence(), &Caller::window(), &draft.digest()),
        "",
    );
    machine.nothing_sent();
}
