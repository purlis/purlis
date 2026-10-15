// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's confinement lives in the session host (#1999), spoken over the session protocol on
//! an in-memory link: the host starts it from the window's plan and says where it is, tells the
//! window what its proxy refused, carried and asks about, takes the window's answers, and holds
//! it for as long as the program that runs in it, whether or not the window's link lasts. The
//! confinement is a stand-in that listens on nothing; the real proxy in the host is the app's
//! behaviour suite, on CI.

#![cfg(unix)]

use std::time::Duration;

use purlis_host::standin::{StandInConfines, StandIns};
use purlis_host::{Compose, Hosting, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Answer, Beside, Client, Command, Confine, ConfinePlan, Events, Launch, Layer, Listed,
    ProgramKind, Pushed, ReachChange, ReachNow, SandboxTold, StartProgram, Told,
};
use purlis_terminal::session::Spec;
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const CONFINEMENT: &str = "01J9CNFNMNT0000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

struct Linked {
    client: Client,
    events: Events,
    confines: StandInConfines,
    hosting: Hosting,
    _project: tempfile::TempDir,
}

async fn linked() -> Linked {
    let confines = StandInConfines::default();
    let hosting = Hosting::confining(Echo, confines.clone(), StandIns::default());
    let (client, events) = linked_as(&hosting, Scope::LocalUi).await;
    let project = tempfile::tempdir().unwrap();
    client
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .expect("held");
    client
        .subscribe(Default::default())
        .await
        .expect("subscribed");
    Linked {
        client,
        events,
        confines,
        hosting,
        _project: project,
    }
}

async fn linked_as(hosting: &Hosting, scope: Scope) -> (Client, Events) {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (client, _views, events) = Client::new(client.unwrap());
    (client, events)
}

fn planned(id: &str) -> Confine {
    Confine {
        confinement: id.into(),
        project: PROJECT.into(),
        plan: ConfinePlan {
            listed: vec![Listed {
                host: "api.anthropic.com".into(),
                layer: Layer::Open,
            }],
            never: Vec::new(),
            withdrawn: Vec::new(),
            anything: false,
            tunnel_ports: vec![443],
            plain_ports: vec![80],
            any_port: false,
            beside: Beside::Wrapped,
            given: vec![5432],
        },
        asks: true,
    }
}

async fn confine(client: &Client, id: &str) -> Result<session::Confined, String> {
    match client.call(Command::Confine(Box::new(planned(id)))).await {
        Ok(Answer::Confined(confined)) => Ok(confined),
        Ok(other) => panic!("answered {other:?}"),
        Err(why) => Err(why.refusal().map(|r| r.code.clone()).unwrap_or_default()),
    }
}

fn running_in(id: &str) -> StartProgram {
    StartProgram {
        launch: Some(Launch {
            program: "/bin/echo".into(),
            args: Vec::new(),
            cwd: None,
            set: Vec::new(),
            strip: Vec::new(),
            pass: Vec::new(),
            declared: Vec::new(),
            harness: None,
            git_hooks: None,
            reporting: None,
            confinement: Some(id.into()),
        }),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

async fn reach_of(client: &Client, id: &str) -> Result<ReachNow, String> {
    match client
        .call(Command::ReachOf {
            confinement: id.into(),
        })
        .await
    {
        Ok(Answer::Reach(now)) => Ok(now),
        Ok(other) => panic!("answered {other:?}"),
        Err(why) => Err(why.refusal().map(|r| r.code.clone()).unwrap_or_default()),
    }
}

async fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + PATIENCE;
    while !done() {
        assert!(
            std::time::Instant::now() < deadline,
            "{what} never happened"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_confinement_is_started_from_the_windows_plan_and_says_where_it_is() {
    let linked = linked().await;

    let confined = confine(&linked.client, CONFINEMENT)
        .await
        .expect("confined");

    assert_eq!(confined.confinement, CONFINEMENT);
    assert_eq!((confined.http, confined.socks), (50811, 50812));
    assert_eq!(confined.tmp.as_deref(), Some("/tmp/charter-chat-standin"));
    assert_eq!(linked.confines.seen()[0].confine, planned(CONFINEMENT));
}

#[tokio::test]
async fn a_confinement_for_a_project_the_host_does_not_hold_is_refused() {
    let linked = linked().await;
    linked.client.let_go_of_project(PROJECT).await.unwrap();

    let refused = confine(&linked.client, CONFINEMENT).await.unwrap_err();

    assert_eq!(refused, session::NO_SUCH_PROJECT);
    assert!(linked.confines.seen().is_empty());
}

#[tokio::test]
async fn a_confinement_its_binary_could_not_start_is_refused_with_why() {
    let linked = linked().await;
    linked.confines.refuse("no loopback port was free");

    let refused = confine(&linked.client, CONFINEMENT).await.unwrap_err();

    assert_eq!(refused, session::NOT_STARTED);
}

#[tokio::test]
async fn a_confinement_id_the_host_holds_already_is_refused() {
    let linked = linked().await;
    confine(&linked.client, CONFINEMENT)
        .await
        .expect("confined");

    let refused = confine(&linked.client, CONFINEMENT).await.unwrap_err();

    assert_eq!(refused, session::MALFORMED);
    assert_eq!(linked.confines.seen().len(), 1);
}

#[tokio::test]
async fn what_a_proxy_did_in_the_host_is_told_to_the_window_alone() {
    let mut linked = linked().await;
    let (_terminal, mut terminal_events) = linked_as(&linked.hosting, Scope::Terminal).await;
    _terminal.subscribe(Default::default()).await.unwrap();
    confine(&linked.client, CONFINEMENT)
        .await
        .expect("confined");
    let refused = SandboxTold::Refused {
        host: "tracker.example.com".into(),
        port: 443,
    };

    linked.confines.proxy_did(CONFINEMENT, refused.clone());

    let told = timeout(PATIENCE, linked.events.recv())
        .await
        .expect("the window is told")
        .expect("a notice");
    assert_eq!(
        told,
        Told::Pushed(Pushed::Sandbox {
            confinement: CONFINEMENT.into(),
            told: refused,
        })
    );
    assert!(
        timeout(Duration::from_millis(200), terminal_events.recv())
            .await
            .is_err(),
        "a link that may not confine hears nothing of a sandbox"
    );
}

#[tokio::test]
async fn the_windows_answer_to_an_ask_raised_in_the_host_reaches_its_board() {
    let linked = linked().await;
    confine(&linked.client, CONFINEMENT)
        .await
        .expect("confined");
    linked.confines.proxy_did(
        CONFINEMENT,
        SandboxTold::Asked {
            targets: vec!["pypi.example.com:443".into()],
        },
    );

    let answered = linked
        .client
        .call(Command::ChangeReach {
            confinement: CONFINEMENT.into(),
            change: ReachChange::Allow {
                host: "pypi.example.com".into(),
                layer: Layer::Chat,
            },
        })
        .await
        .expect("answered");

    assert_eq!(
        answered,
        Answer::ReachChanged {
            retry: vec!["pypi.example.com:443".into()]
        }
    );
    assert_eq!(
        reach_of(&linked.client, CONFINEMENT)
            .await
            .expect("read")
            .allowed_live,
        [Listed {
            host: "pypi.example.com".into(),
            layer: Layer::Chat
        }]
    );
}

#[tokio::test]
async fn an_answer_about_a_confinement_the_host_does_not_hold_is_refused() {
    let linked = linked().await;

    let refused = linked
        .client
        .call(Command::ChangeReach {
            confinement: CONFINEMENT.into(),
            change: ReachChange::WithdrawPort { port: 5432 },
        })
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_CHAT)
    );
}

#[tokio::test]
async fn its_proxy_asks_only_once_a_program_runs_in_it_and_a_window_hears() {
    let linked = linked().await;
    confine(&linked.client, CONFINEMENT)
        .await
        .expect("confined");
    assert!(
        !linked.confines.askable(CONFINEMENT),
        "nothing runs in it yet"
    );

    linked
        .client
        .start_program(running_in(CONFINEMENT))
        .await
        .expect("started");
    assert!(linked.confines.askable(CONFINEMENT));

    let Linked {
        client,
        events,
        confines,
        hosting,
        ..
    } = linked;
    drop((client, events));
    until("no window hears", || !confines.askable(CONFINEMENT)).await;
    drop(hosting);
}

#[tokio::test]
async fn a_confinement_a_program_runs_in_outlives_the_window_link_that_made_it() {
    let linked = linked().await;
    confine(&linked.client, CONFINEMENT)
        .await
        .expect("confined");
    confine(&linked.client, "01J9CNFNMNT0000000000000001")
        .await
        .expect("confined");
    linked
        .client
        .start_program(running_in(CONFINEMENT))
        .await
        .expect("started");

    let Linked {
        client,
        events,
        confines,
        hosting,
        ..
    } = linked;
    drop((client, events));

    until("the one nothing runs in ends with its link", || {
        confines.ended("01J9CNFNMNT0000000000000001")
    })
    .await;
    assert!(!confines.ended(CONFINEMENT), "the chat keeps its network");
    let (next, _events) = linked_as(&hosting, Scope::LocalUi).await;
    assert!(reach_of(&next, CONFINEMENT).await.is_ok());
}

#[tokio::test]
async fn closing_the_program_ends_its_confinement() {
    let linked = linked().await;
    confine(&linked.client, CONFINEMENT)
        .await
        .expect("confined");
    let program = linked
        .client
        .start_program(running_in(CONFINEMENT))
        .await
        .expect("started");

    linked.client.close(&program).await.expect("closed");

    until("its confinement ends", || {
        linked.confines.ended(CONFINEMENT)
    })
    .await;
    assert_eq!(
        reach_of(&linked.client, CONFINEMENT).await.unwrap_err(),
        session::NO_SUCH_CHAT
    );
}

#[tokio::test]
async fn letting_go_of_a_confinement_ends_it() {
    let linked = linked().await;
    confine(&linked.client, CONFINEMENT)
        .await
        .expect("confined");

    linked
        .client
        .call(Command::LetGoOfConfinement {
            confinement: CONFINEMENT.into(),
        })
        .await
        .expect("let go");

    until("it ends", || linked.confines.ended(CONFINEMENT)).await;
}

#[tokio::test]
async fn a_start_in_a_confinement_the_host_does_not_hold_runs_nothing() {
    let runs = StandIns::default();
    let hosting = Hosting::confining(Echo, StandInConfines::default(), runs.clone());
    let (client, _events) = linked_as(&hosting, Scope::LocalUi).await;
    let project = tempfile::tempdir().unwrap();
    client
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .unwrap();

    let refused = client
        .start_program(running_in(CONFINEMENT))
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_STARTED)
    );
    assert!(runs.ran().is_empty());
}

#[tokio::test]
async fn a_confinement_runs_one_program_at_most() {
    let linked = linked().await;
    confine(&linked.client, CONFINEMENT)
        .await
        .expect("confined");
    linked
        .client
        .start_program(running_in(CONFINEMENT))
        .await
        .expect("started");

    let refused = linked
        .client
        .start_program(running_in(CONFINEMENT))
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_STARTED)
    );
}

#[tokio::test]
async fn ending_the_host_ends_every_confinement() {
    let linked = linked().await;
    confine(&linked.client, CONFINEMENT)
        .await
        .expect("confined");
    linked
        .client
        .start_program(running_in(CONFINEMENT))
        .await
        .expect("started");

    let hosting = linked.hosting.clone();
    tokio::task::spawn_blocking(move || hosting.end_all())
        .await
        .unwrap();

    assert!(linked.confines.ended(CONFINEMENT));
}

#[tokio::test]
async fn a_confinement_whose_link_ends_while_it_starts_ends_with_it() {
    let linked = linked().await;
    linked.confines.hold_starts();
    let (other, _other_events) = linked_as(&linked.hosting, Scope::LocalUi).await;
    let Linked {
        client,
        events,
        confines,
        hosting,
        _project,
    } = linked;
    let asking = tokio::spawn(async move {
        let _ = confine(&client, CONFINEMENT).await;
        drop(events);
    });
    until("the start is under way", || confines.waiting() == 1).await;

    // The window goes while its confinement is being started.
    asking.abort();
    let _ = asking.await;
    // The host has heard: the project the window held is held no more.
    let deadline = std::time::Instant::now() + PATIENCE;
    loop {
        match other.start_program(running_in("none")).await {
            Err(refused)
                if refused.refusal().map(|r| r.code.as_str()) == Some(session::NO_SUCH_PROJECT) =>
            {
                break;
            }
            _ => assert!(
                std::time::Instant::now() < deadline,
                "the link's end was never heard"
            ),
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    confines.let_starts_go();

    until("it ends", || {
        confines
            .seen()
            .iter()
            .any(|one| one.confine.confinement == CONFINEMENT && one.ended)
    })
    .await;
    assert_eq!(
        reach_of(&other, CONFINEMENT).await.unwrap_err(),
        session::NO_SUCH_CHAT,
        "nobody holds a confinement for a link that is gone"
    );
    drop(hosting);
}

#[tokio::test]
async fn a_start_runs_only_in_a_confinement_its_own_link_made() {
    let linked = linked().await;
    let (other, _events) = linked_as(&linked.hosting, Scope::LocalUi).await;
    other
        .hold_project(PROJECT, linked._project.path().to_str().unwrap())
        .await
        .expect("held by the other window too");
    confine(&other, CONFINEMENT).await.expect("confined");

    let refused = linked
        .client
        .start_program(running_in(CONFINEMENT))
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_STARTED)
    );
    assert!(!linked.confines.askable(CONFINEMENT), "nothing runs in it");
    other
        .start_program(running_in(CONFINEMENT))
        .await
        .expect("its own link's start runs in it");
}

#[tokio::test]
async fn a_start_runs_only_in_a_confinement_made_for_its_project() {
    const ANOTHER: &str = "01J9PRJCT00000000000000001";
    let linked = linked().await;
    let another = tempfile::tempdir().unwrap();
    linked
        .client
        .hold_project(ANOTHER, another.path().to_str().unwrap())
        .await
        .expect("held");
    confine(&linked.client, CONFINEMENT)
        .await
        .expect("confined");

    let refused = linked
        .client
        .start_program(StartProgram {
            project: ANOTHER.into(),
            ..running_in(CONFINEMENT)
        })
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_STARTED)
    );
    assert!(!linked.confines.askable(CONFINEMENT));
}

#[tokio::test]
async fn a_link_that_is_not_the_windows_is_refused_a_confinement_and_starts_nothing() {
    let linked = linked().await;
    let (terminal, _events) = linked_as(&linked.hosting, Scope::Terminal).await;

    let refused = confine(&terminal, CONFINEMENT).await.unwrap_err();

    assert_eq!(refused, session::NOT_ALLOWED);
    assert!(linked.confines.seen().is_empty());
}
