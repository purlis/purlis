// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host holds every client to its device's browser budget (#2611): it counts the
//! browsers it runs, whoever started them, and reserves a place under its own state lock before
//! a browser starts, so two clients never both take the last one. A start past the budget is
//! refused `device_full`. The browsers are stand-ins, so none of this needs an engine.

#![cfg(unix)]

use std::time::{Duration, Instant};

use purlis_host::standin::{StandInBrowsers, StandIns};
use purlis_host::{ComposesNothing, Hosting};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, CallError, Client, Exit, ProgramKind, StartProgram};
use tokio::io::duplex;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// A host whose browsers are `browsers`, and a client of it holding the project.
async fn client_of(hosting: &Hosting, project: &std::path::Path) -> Client {
    let (a, b) = duplex(4 << 20);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (client, _views, _events) = Client::new(client.unwrap());
    client
        .hold_project(PROJECT, project.to_str().unwrap())
        .await
        .expect("the project is held");
    client
}

fn hosting_with(browsers: &StandInBrowsers) -> Hosting {
    Hosting::running_with(ComposesNothing, StandIns::default()).with_browsers(browsers.clone())
}

fn a_browser() -> StartProgram {
    StartProgram::of(PROJECT, ProgramKind::Browser)
}

fn device_full(refused: Result<String, CallError>) -> session::Refusal {
    match refused {
        Err(CallError::Refused(refusal)) if refusal.code == session::DEVICE_FULL => refusal,
        other => panic!("refused device_full, not {other:?}"),
    }
}

/// Waits until `browsers` has `n` starts waiting to be let go.
async fn waiting(browsers: &StandInBrowsers, n: usize) {
    let until = Instant::now() + PATIENCE;
    while browsers.waiting() < n {
        assert!(Instant::now() < until, "{n} starts never came");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
async fn a_start_past_the_devices_budget_is_refused_device_full_whoever_sends_it() {
    let store = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let browsers = StandInBrowsers::in_folder(store.path().join("browsers")).budgeted(2);
    let hosting = hosting_with(&browsers);
    // Two clients of one host: the window, and another of the same scope, as a relayed one is.
    let window = client_of(&hosting, project.path()).await;
    let other = client_of(&hosting, project.path()).await;

    window
        .start_program(a_browser())
        .await
        .expect("the first starts");
    other
        .start_program(a_browser())
        .await
        .expect("the second starts");

    let refused = device_full(other.start_program(a_browser()).await);
    // A window before 1.32 shows its `why` as it is: it names the budget and where it is set.
    assert!(
        refused.why.contains("2 browsers")
            && refused.why.contains("Settings › This machine › Browsers"),
        "the refusal names the budget and its setting: {}",
        refused.why
    );
    device_full(window.start_program(a_browser()).await);
    assert_eq!(
        browsers.started().len(),
        2,
        "nothing past the budget started"
    );
}

#[tokio::test]
async fn two_starts_at_once_for_the_last_place_start_one_browser_and_refuse_the_other() {
    let store = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let browsers = StandInBrowsers::in_folder(store.path().join("browsers"))
        .budgeted(1)
        .holding_starts();
    let hosting = hosting_with(&browsers);
    let window = client_of(&hosting, project.path()).await;
    let other = client_of(&hosting, project.path()).await;

    // The first start takes the last place and is still starting.
    let first = tokio::spawn(async move { window.start_program(a_browser()).await });
    waiting(&browsers, 1).await;

    // The second is refused while the first has not yet started.
    let second = tokio::time::timeout(PATIENCE, other.start_program(a_browser())).await;
    browsers.let_starts_go();
    device_full(second.expect("the second start was answered while the first was starting"));

    first.await.unwrap().expect("the first starts");
    assert_eq!(browsers.started().len(), 1);
}

#[tokio::test]
async fn a_browser_that_ended_or_failed_to_start_gives_its_place_back() {
    let store = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let browsers = StandInBrowsers::in_folder(store.path().join("browsers")).budgeted(1);
    let hosting = hosting_with(&browsers);
    let window = client_of(&hosting, project.path()).await;

    let program = window.start_program(a_browser()).await.expect("it starts");
    device_full(window.start_program(a_browser()).await);

    // Its engine ends: its place is free again.
    browsers.started()[0].ends(Exit::Code(0));
    let again = starts_soon(&window).await;
    assert_ne!(again, program);

    // A start that fails keeps no place: the one after it starts.
    browsers.started().last().unwrap().ends(Exit::Code(0));
    browsers.fails_next();
    let until = Instant::now() + PATIENCE;
    let failed = loop {
        match window.start_program(a_browser()).await {
            Err(CallError::Refused(refusal))
                if refusal.code == session::DEVICE_FULL && Instant::now() < until =>
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            other => break other,
        }
    };
    match failed {
        Err(CallError::Refused(refusal)) => assert_eq!(refusal.code, session::NOT_STARTED),
        other => panic!("the failing start was refused not_started, not {other:?}"),
    }
    window
        .start_program(a_browser())
        .await
        .expect("the failed start held no place");
}

/// Starts a browser, trying again for a while as long as the host is full: a browser's end is
/// heard a moment after it ends.
async fn starts_soon(client: &Client) -> String {
    let until = Instant::now() + PATIENCE;
    loop {
        match client.start_program(a_browser()).await {
            Ok(program) => return program,
            Err(CallError::Refused(refusal))
                if refusal.code == session::DEVICE_FULL && Instant::now() < until =>
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(why) => panic!("no place came back: {why}"),
        }
    }
}

#[tokio::test]
async fn a_host_given_no_budget_starts_every_browser_it_is_asked_to() {
    let store = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let browsers = StandInBrowsers::in_folder(store.path().join("browsers"));
    let hosting = hosting_with(&browsers);
    let window = client_of(&hosting, project.path()).await;

    for _ in 0..5 {
        window.start_program(a_browser()).await.expect("it starts");
    }
}
