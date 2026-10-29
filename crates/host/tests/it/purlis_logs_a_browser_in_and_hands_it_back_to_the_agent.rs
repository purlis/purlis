// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis performs a login in a browser (#2016), over the session protocol on an in-memory
//! link: the browser is taken from the agent and handed back under a new lease, which the host
//! takes as the agent's, so the agent's next act is carried out; while the person drives, no
//! login runs. The browser is a stand-in; the login itself (the origin, the fill, the blanking,
//! the wait) is `purlis-browser-engine`'s tests.

#![cfg(unix)]

use std::time::Duration;

use purlis_host::standin::{StandInBrowsers, StandInScreen, StandIns};
use purlis_host::{ComposesNothing, Hosting};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::browsing::{Act, LogIn, Secret};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram};
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);
const ADDRESS: &str = "https://shop.example.test/login";

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

async fn window(hosting: &Hosting, project: &std::path::Path) -> Client {
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

fn login() -> LogIn {
    LogIn {
        address: ADDRESS.into(),
        user: Some(Secret::new("fixture-user".into())),
        password: Secret::new("fixture-word".into()),
        keep: None,
        identity: None,
    }
}

async fn a_browser(client: &Client, browsers: &StandInBrowsers) -> (String, StandInScreen) {
    let program = client
        .start_program(StartProgram::of(PROJECT, ProgramKind::Browser))
        .await
        .expect("a browser starts");
    (program, browsers.started().last().unwrap().clone())
}

fn hosted() -> (
    Hosting,
    StandInBrowsers,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    let store = tempfile::tempdir().unwrap();
    let browsers = StandInBrowsers::in_folder(store.path().join("browsers"));
    let hosting =
        Hosting::running_with(ComposesNothing, StandIns::default()).with_browsers(browsers.clone());
    (hosting, browsers, store, tempfile::tempdir().unwrap())
}

#[tokio::test]
async fn the_agents_next_act_is_made_under_the_lease_the_login_handed_back() {
    let (hosting, browsers, _store, project) = hosted();
    let client = window(&hosting, project.path()).await;
    let (program, screen) = a_browser(&client, &browsers).await;
    timeout(PATIENCE, client.log_in(&program, login()))
        .await
        .unwrap()
        .expect("the login answers");
    assert_eq!(screen.shown().logins, vec![ADDRESS.to_owned()]);
    let handed_back = screen.shown().epoch;
    timeout(PATIENCE, client.browse(&program, "c-1", Act::Pages))
        .await
        .unwrap()
        .expect("the agent acts again");
    assert_eq!(screen.shown().epochs, vec![handed_back]);
}

#[tokio::test]
async fn no_login_runs_while_the_person_drives() {
    let (hosting, browsers, _store, project) = hosted();
    let client = window(&hosting, project.path()).await;
    let (program, screen) = a_browser(&client, &browsers).await;
    client.take_over(&program).await.unwrap();
    let refused = timeout(PATIENCE, client.log_in(&program, login()))
        .await
        .unwrap()
        .expect_err("refused");
    let refusal = refused.refusal().expect("a refusal");
    assert_eq!(refusal.code, session::BROWSER_REFUSED);
    assert_eq!(refusal.why, purlis_host::A_BROWSER_THE_PERSON_DRIVES);
    assert!(screen.shown().logins.is_empty());
}

#[tokio::test]
async fn a_login_names_no_value_in_what_the_host_says_of_it() {
    let said = format!(
        "{:?}",
        session::Command::LogIn {
            program: "b".into(),
            login: login(),
        }
    );
    assert!(
        !said.contains("fixture-user") && !said.contains("fixture-word"),
        "{said}"
    );
}

#[tokio::test]
async fn a_browser_started_from_kept_logins_is_given_them_by_the_host() {
    let (hosting, browsers, _store, project) = hosted();
    let client = window(&hosting, project.path()).await;
    let kept = purlis_session_protocol::browsing::KeptLogin {
        profile: "claude".into(),
        persona: "steward".into(),
        login: "shop-login".into(),
        identity: "0123456789abcdef0123456789abcdef".into(),
        address: ADDRESS.into(),
    };
    client
        .start_program(StartProgram {
            kept: Some(kept.clone()),
            ..StartProgram::of(PROJECT, ProgramKind::Browser)
        })
        .await
        .expect("a browser starts");
    let screen = browsers.started().last().unwrap().clone();
    assert_eq!(screen.shown().kept, Some(kept));
}
