// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A grade asked for at a terminal (SK-61, #3241), over the session protocol on in-memory links:
//! the host relays the ask to one window that holds the project, carries the answer and the steps
//! back to the asking link alone, and stops the grade once that link goes.

#![cfg(unix)]

use std::sync::Arc;
use std::time::Duration;

use purlis_host::standin::StandIns;
use purlis_host::{Compose, Hosting, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Client, Command, Events, GradeAnswer, GradeAsk, GradeEndTold, GradeLevel, GradeOf,
    GradeStepTold, Pushed, Since, Told,
};
use purlis_terminal::session::Spec;
use tokio::io::duplex;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

async fn linked(host: &Hosting, scope: Scope) -> (Arc<Client>, Events) {
    let (ours, theirs) = duplex(1 << 16);
    let (linked, served) = tokio::join!(
        link::connect(ours, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(theirs, session::speaks(), &HELD),
    );
    tokio::spawn(session::serve(served.unwrap(), host.clone(), None));
    let (client, _views, events) = Client::new(linked.unwrap());
    client.subscribe(Since::new()).await.unwrap();
    (Arc::new(client), events)
}

fn of() -> GradeOf {
    GradeOf {
        level: GradeLevel::Project { local: false },
        skill: "deploy".into(),
        profile: None,
        model: None,
        evals: Vec::new(),
    }
}

fn step(ended: Option<GradeEndTold>) -> GradeStepTold {
    GradeStepTold {
        grade: "g".into(),
        ran: 1,
        of: 6,
        tokens: None,
        cost_micro_usd: None,
        asked: 0,
        ended,
        said: None,
    }
}

/// The next push of a grade this link hears.
async fn next_of_a_grade(events: &mut Events) -> Pushed {
    loop {
        match tokio::time::timeout(PATIENCE, events.recv()).await {
            Ok(Some(Told::Pushed(
                pushed @ (Pushed::GradeAsked { .. }
                | Pushed::StopGrade { .. }
                | Pushed::GradeStep { .. }),
            ))) => return pushed,
            Ok(Some(_)) => {}
            other => panic!("no grade's word came: {other:?}"),
        }
    }
}

/// Asks from `terminal`, and answers as `window` with `answer`: the terminal's answer, and the
/// question it was asked by.
async fn asked_and_answered(
    terminal: &Arc<Client>,
    window: &Arc<Client>,
    told: &mut Events,
    path: &str,
    ask: GradeAsk,
    answer: GradeAnswer,
) -> (GradeAnswer, u64) {
    let asking = {
        let (terminal, path) = (Arc::clone(terminal), path.to_owned());
        tokio::spawn(async move { terminal.ask_grade(PROJECT, &path, ask).await })
    };
    let Pushed::GradeAsked { question, .. } = next_of_a_grade(told).await else {
        panic!("the window was not asked");
    };
    window
        .call(Command::AnswerGrade { question, answer })
        .await
        .unwrap();
    (asking.await.unwrap().unwrap(), question)
}

struct Echoes;

impl Compose for Echoes {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

fn host() -> Hosting {
    Hosting::running_with(Echoes, StandIns::default())
}

#[tokio::test]
async fn no_window_holding_the_project_is_refused_in_words_that_say_to_open_it() {
    let project = tempfile::tempdir().unwrap();
    let host = host();
    let (terminal, _told) = linked(&host, Scope::Terminal).await;
    let refused = terminal
        .ask_grade(
            PROJECT,
            project.path().to_str().unwrap(),
            GradeAsk::Plan { of: of() },
        )
        .await
        .unwrap_err();
    let refusal = refused.refusal().expect("a refusal");
    assert_eq!(refusal.code, session::NO_SUCH_PROJECT);
    assert!(refusal.why.contains("open the project"), "{}", refusal.why);
}

#[tokio::test]
async fn a_window_holding_another_clone_of_the_project_is_not_asked() {
    let (held, other) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let host = host();
    let (window, _told) = linked(&host, Scope::LocalUi).await;
    window
        .hold_project(PROJECT, held.path().to_str().unwrap())
        .await
        .unwrap();
    let (terminal, _heard) = linked(&host, Scope::Terminal).await;
    let refused = terminal
        .ask_grade(
            PROJECT,
            other.path().to_str().unwrap(),
            GradeAsk::Plan { of: of() },
        )
        .await
        .unwrap_err();
    assert_eq!(
        refused.refusal().expect("a refusal").code,
        session::NO_SUCH_PROJECT
    );
}

#[tokio::test]
async fn the_answer_and_the_steps_reach_the_asking_terminal_alone() {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().to_str().unwrap();
    let host = host();
    let (window, mut told) = linked(&host, Scope::LocalUi).await;
    window.hold_project(PROJECT, path).await.unwrap();
    let (terminal, mut heard) = linked(&host, Scope::Terminal).await;
    let (bystander, mut unheard) = linked(&host, Scope::Terminal).await;

    let started = GradeAnswer::Started { grade: "g".into() };
    let (answer, question) = asked_and_answered(
        &terminal,
        &window,
        &mut told,
        path,
        GradeAsk::Start {
            of: of(),
            consent: "digest".into(),
        },
        started.clone(),
    )
    .await;
    assert_eq!(answer, started);

    window
        .call(Command::GradeStep {
            question,
            step: step(None),
        })
        .await
        .unwrap();
    assert_eq!(
        next_of_a_grade(&mut heard).await,
        Pushed::GradeStep { step: step(None) }
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(200), next_of_a_grade(&mut unheard))
            .await
            .is_err(),
        "another terminal hears nothing of it"
    );
    drop(bystander);
}

#[tokio::test]
async fn a_terminal_going_stops_its_grade() {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().to_str().unwrap();
    let host = host();
    let (window, mut told) = linked(&host, Scope::LocalUi).await;
    window.hold_project(PROJECT, path).await.unwrap();
    let (terminal, _heard) = linked(&host, Scope::Terminal).await;
    asked_and_answered(
        &terminal,
        &window,
        &mut told,
        path,
        GradeAsk::Start {
            of: of(),
            consent: "digest".into(),
        },
        GradeAnswer::Started { grade: "g".into() },
    )
    .await;

    terminal.end_link();

    assert_eq!(
        next_of_a_grade(&mut told).await,
        Pushed::StopGrade { grade: "g".into() }
    );
}

#[tokio::test]
async fn a_step_of_a_grade_nobody_follows_is_refused_so_the_window_stops_it() {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().to_str().unwrap();
    let host = host();
    let (window, mut told) = linked(&host, Scope::LocalUi).await;
    window.hold_project(PROJECT, path).await.unwrap();
    let (terminal, _heard) = linked(&host, Scope::Terminal).await;
    let (_, question) = asked_and_answered(
        &terminal,
        &window,
        &mut told,
        path,
        GradeAsk::Start {
            of: of(),
            consent: "digest".into(),
        },
        GradeAnswer::Started { grade: "g".into() },
    )
    .await;
    terminal.end_link();
    next_of_a_grade(&mut told).await;

    let said = window
        .call(Command::GradeStep {
            question,
            step: step(None),
        })
        .await;

    assert!(said.is_err(), "{said:?}");
}

#[tokio::test]
async fn a_window_going_tells_the_terminal_its_grade_ended() {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().to_str().unwrap();
    let host = host();
    let (window, mut told) = linked(&host, Scope::LocalUi).await;
    window.hold_project(PROJECT, path).await.unwrap();
    let (terminal, mut heard) = linked(&host, Scope::Terminal).await;
    asked_and_answered(
        &terminal,
        &window,
        &mut told,
        path,
        GradeAsk::Start {
            of: of(),
            consent: "digest".into(),
        },
        GradeAnswer::Started { grade: "g".into() },
    )
    .await;

    window.end_link();

    let Pushed::GradeStep { step } = next_of_a_grade(&mut heard).await else {
        panic!("the terminal was not told");
    };
    assert_eq!(step.ended, Some(GradeEndTold::WindowGone));
}
