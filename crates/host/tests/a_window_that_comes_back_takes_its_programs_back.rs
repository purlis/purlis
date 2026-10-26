// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A window that comes back to a session host that outlived it takes its programs back (#2006),
//! spoken to over the session protocol on in-memory links: `programs_of` answers every program a
//! held project still has, running or ended, with its number, its process, its run, how it
//! ended, the confinement it runs in and its size, so the window takes each back where it is
//! rather than starting it again. And `quit` is the window quitting: the host ends every program
//! as a quit would, answers once they are gone, and stops. The programs and confinements are
//! stand-ins; a real host process is the app's behaviour suite, on CI.

#![cfg(unix)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_host::standin::{StandInConfines, StandIns};
use purlis_host::{Compose, Ended, Hosting, Record, Stopped, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{
    self, Answer, Client, Command, Confine, ConfinePlan, Events, Exit, HeldProgram, Launch,
    ProgramKind, RunSandbox, StartProgram,
};
use purlis_session_protocol::view::{Chunk, Viewer};
use purlis_terminal::session::Spec;
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const OTHER: &str = "01J9PRJCT00000000000000001";
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

/// One program's end the host kept: where its project is, its number and the host's act that
/// ended it.
type Kept = (PathBuf, u32, Option<Stopped>);

/// Each program's end the host kept.
#[derive(Clone, Default)]
struct Ends(Arc<Mutex<Vec<Kept>>>);

impl Record for Ends {
    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Ok(())
    }

    fn ended(&self, ended: &Ended<'_>) {
        self.0
            .lock()
            .unwrap()
            .push((ended.project_path.to_owned(), ended.number, ended.stopped));
    }
}

impl Ends {
    fn kept(&self) -> Vec<(u32, Option<Stopped>)> {
        let mut kept: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|(_, number, stopped)| (*number, *stopped))
            .collect();
        kept.sort();
        kept
    }
}

struct Host {
    hosting: Hosting,
    runs: StandIns,
    confines: StandInConfines,
    ends: Ends,
    project: tempfile::TempDir,
    other: tempfile::TempDir,
}

fn a_host() -> Host {
    let runs = StandIns::default();
    let confines = StandInConfines::default();
    let ends = Ends::default();
    Host {
        hosting: Hosting::confining(Echo, confines.clone(), runs.clone())
            .recording_with(ends.clone()),
        runs,
        confines,
        ends,
        project: tempfile::tempdir().unwrap(),
        other: tempfile::tempdir().unwrap(),
    }
}

/// A window's link to the host, subscribed.
struct Window {
    client: Client,
    views: Acceptor,
    _events: Events,
}

impl Host {
    async fn window(&self) -> Window {
        let (a, b) = duplex(256 * 1024);
        let (client, served) = tokio::join!(
            link::connect(
                a,
                session::speaks(),
                Scope::LocalUi,
                HELD.of(Scope::LocalUi)
            ),
            link::serve_any(b, session::speaks(), &HELD)
        );
        tokio::spawn(session::serve(served.unwrap(), self.hosting.clone(), None));
        let (client, views, events) = Client::new(client.unwrap());
        client.subscribe(session::Since::new()).await.unwrap();
        Window {
            client,
            views,
            _events: events,
        }
    }

    async fn holding(&self) -> Window {
        let window = self.window().await;
        window
            .client
            .hold_project(PROJECT, self.project.path().to_str().unwrap())
            .await
            .expect("the project is held");
        window
    }
}

impl Window {
    async fn starts(&self, kind: ProgramKind, number: u32, run: &str) -> String {
        self.client
            .start_program(StartProgram {
                number: Some(number),
                run: Some(run.into()),
                cols: Some(120),
                rows: Some(40),
                ..StartProgram::of(PROJECT, kind)
            })
            .await
            .expect("it starts")
    }

    async fn programs(&self) -> Vec<HeldProgram> {
        let mut programs = self
            .client
            .programs_of(PROJECT)
            .await
            .expect("the programs are answered");
        programs.sort_by_key(|program| program.number);
        programs
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
async fn every_program_a_held_project_has_is_answered_with_what_takes_it_back() {
    let host = a_host();
    let window = host.holding().await;
    let harness = window.starts(ProgramKind::Harness, 3, "run-3").await;
    let shell = window.starts(ProgramKind::Shell, 4, "run-4").await;
    let closed = window.starts(ProgramKind::Shell, 5, "run-5").await;
    host.runs.ran()[1].1.ends(Exit::Code(7));
    window.client.close(&closed).await.expect("it closes");

    let programs = window.programs().await;

    assert_eq!(
        programs,
        [
            HeldProgram {
                program: harness,
                kind: ProgramKind::Harness,
                number: 3,
                process: Some(4242),
                run: Some("run-3".into()),
                exit: None,
                confined: None,
                plan: None,
                // Nothing confines a stand-in, and nothing wraps it.
                sandbox: Some(RunSandbox::Off),
                cols: 120,
                rows: 40,
                driver: None,
                note: None,
            },
            HeldProgram {
                program: shell,
                kind: ProgramKind::Shell,
                number: 4,
                process: None,
                run: Some("run-4".into()),
                exit: Some(Exit::Code(7)),
                confined: None,
                plan: None,
                // Nothing confines a stand-in, and nothing wraps it.
                sandbox: Some(RunSandbox::Off),
                cols: 120,
                rows: 40,
                driver: None,
                note: None,
            },
        ],
        "running and ended, never closed"
    );
}

#[tokio::test]
async fn a_program_in_a_confinement_is_answered_with_where_its_confinement_is() {
    let host = a_host();
    let window = host.holding().await;
    let confined = match window
        .client
        .call(Command::Confine(Box::new(Confine {
            confinement: CONFINEMENT.into(),
            project: PROJECT.into(),
            plan: ConfinePlan {
                listed: Vec::new(),
                never: Vec::new(),
                withdrawn: Vec::new(),
                anything: false,
                tunnel_ports: vec![443],
                plain_ports: vec![80],
                any_port: false,
                beside: session::Beside::Wrapped,
                given: Vec::new(),
            },
            asks: true,
        })))
        .await
        .expect("confined")
    {
        Answer::Confined(confined) => confined,
        other => panic!("answered {other:?}"),
    };
    window
        .client
        .start_program(StartProgram {
            number: Some(1),
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
                confinement: Some(CONFINEMENT.into()),
            }),
            ..StartProgram::of(PROJECT, ProgramKind::Harness)
        })
        .await
        .expect("it starts");

    let programs = window.programs().await;

    assert_eq!(programs.len(), 1);
    assert_eq!(programs[0].confined, Some(confined));
    assert_eq!(
        programs[0]
            .plan
            .as_ref()
            .map(|plan| plan.tunnel_ports.clone()),
        Some(vec![443]),
        "the plan it was started from"
    );
    // #2636: read from the confinement that holds it, though nothing on its line names a
    // sandbox: a confined run never reads as off.
    assert_eq!(programs[0].sandbox, Some(RunSandbox::By("proxy".into())));
    assert!(!host.confines.ended(CONFINEMENT));
}

#[tokio::test]
async fn only_a_link_that_holds_the_project_is_answered_its_programs() {
    let host = a_host();
    let window = host.holding().await;
    window.starts(ProgramKind::Shell, 1, "run-1").await;
    let elsewhere = host.window().await;
    elsewhere
        .client
        .hold_project(OTHER, host.other.path().to_str().unwrap())
        .await
        .expect("it holds another project");

    let refused = elsewhere.client.programs_of(PROJECT).await.unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_PROJECT)
    );
}

#[tokio::test]
async fn a_window_back_after_its_link_ended_takes_a_program_back_as_it_is_now() {
    let host = a_host();
    let program = {
        let first = host.holding().await;
        let program = first.starts(ProgramKind::Harness, 2, "run-2").await;
        host.runs.last().writes(b"before. ");
        program
    };
    // Written with no window there.
    host.runs.last().writes(b"while away. ");

    let mut back = host.holding().await;
    let programs = back.programs().await;
    assert_eq!(
        programs
            .iter()
            .map(|p| (&p.program, p.number))
            .collect::<Vec<_>>(),
        [(&program, 2)]
    );
    back.client
        .attach_view(&program, session::ViewKind::Terminal)
        .await
        .expect("a view of it");
    let stream = timeout(PATIENCE, back.views.accept())
        .await
        .expect("its view's stream")
        .expect("a stream");
    let mut reader = Viewer::default().accept(stream).await.expect("a view");
    let first = timeout(PATIENCE, reader.next())
        .await
        .expect("its snapshot")
        .expect("a chunk")
        .expect("read");
    assert_eq!(
        first,
        Chunk::Snapshot(bytes::Bytes::from_static(b"before. while away. "))
    );
    back.client
        .call(Command::Write {
            chat: program,
            bytes: b"typed".to_vec(),
        })
        .await
        .expect("typed");
    assert_eq!(host.runs.last().seen().typed, b"typed");
}

#[tokio::test]
async fn a_quit_ends_every_program_as_a_quit_and_answers_once_they_are_gone() {
    let host = a_host();
    let window = host.holding().await;
    window.starts(ProgramKind::Harness, 1, "run-1").await;
    window.starts(ProgramKind::Shell, 2, "run-2").await;

    timeout(PATIENCE, window.client.quit())
        .await
        .expect("the quit is answered")
        .expect("the host quits");

    assert!(host.runs.ran().iter().all(|(_, run)| run.seen().stopped));
    until("both ends are kept", || host.ends.kept().len() == 2).await;
    assert_eq!(
        host.ends.kept(),
        [(1, Some(Stopped::Quit)), (2, Some(Stopped::Quit))]
    );
    timeout(PATIENCE, host.hosting.until_it_quits())
        .await
        .expect("the host is told to stop");
}

#[tokio::test]
async fn a_host_nobody_told_to_quit_goes_on() {
    let host = a_host();
    let _window = host.holding().await;

    assert!(
        timeout(Duration::from_millis(200), host.hosting.until_it_quits())
            .await
            .is_err()
    );
}
