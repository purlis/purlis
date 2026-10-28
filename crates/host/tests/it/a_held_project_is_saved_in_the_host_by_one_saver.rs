// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project's saving in the session host (#2002), spoken over the session protocol on an
//! in-memory link: the window asks the host to save a project it holds, the host runs its
//! binary's saver for it, once, and tells the window what each look did. Only a link that may
//! save a project (`local-ui`) and holds it hears how saving it goes: what each look did, and the
//! saver's question of who is mid-turn. The saver goes on when the window's link ends without a
//! word, and stops when the window says so. The saver here is a stand-in; the app's own, over a
//! real project, is the app's behaviour suite.

#![cfg(unix)]

use std::time::Duration;

use purlis_host::standin::{Savers, StandIns};
use purlis_host::{ComposesNothing, Dial, Hosting, Looked, Poke, Relay};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::session::{
    self, Client, Events, Launch, ProgramKind, Pushed, StartProgram, Told,
};
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// The place the host holds `folder` at: the folder, however its path is spelled.
fn place(folder: &tempfile::TempDir) -> std::path::PathBuf {
    std::fs::canonicalize(folder.path()).unwrap()
}

/// Runs a start's launch by its program.
struct ItsLaunch;

impl purlis_host::Compose for ItsLaunch {
    fn compose(
        &self,
        starting: &purlis_host::ToCompose<'_>,
    ) -> Result<purlis_terminal::session::Spec, String> {
        let launch: &Launch = starting.start.launch.as_ref().ok_or("no launch")?;
        Ok(purlis_terminal::session::Spec::new(
            &launch.program,
            starting.size,
        ))
    }
}

struct Host {
    hosting: Hosting,
    savers: Savers,
    project: tempfile::TempDir,
}

fn a_host() -> Host {
    let savers = Savers::default();
    Host {
        hosting: Hosting::running_with(ItsLaunch, StandIns::default()).saving_with(savers.clone()),
        savers,
        project: tempfile::tempdir().unwrap(),
    }
}

/// A window: a `local-ui` link to the host, with the project held on it.
async fn a_window(host: &Host) -> (Client, Events) {
    let (client, events) = a_link(host, Scope::LocalUi).await;
    client
        .hold_project(PROJECT, host.project.path().to_str().unwrap())
        .await
        .expect("the project is held");
    (client, events)
}

/// A link to the host admitted as `scope`, one of those proved by a credential.
async fn a_link(host: &Host, scope: Scope) -> (Client, Events) {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), host.hosting.clone(), None));
    let (client, _views, events) = Client::new(client.unwrap());
    (client, events)
}

/// A link to `hosting` from another host: `remote-link`, proved as a peer device.
async fn from_another_host(hosting: Hosting) -> Link {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect_as_a_device(a, session::speaks()),
        link::serve_as_a_device(b, session::speaks(), link::ProvenDevice::stand_in())
    );
    tokio::spawn(session::serve(served.unwrap(), hosting, None));
    client.unwrap()
}

/// A window on another host, which relays to this one: its holds reach this host over a
/// `remote-link` link, as a relayed window's do.
async fn a_window_relayed_from_another_host(host: &Host) -> (Client, Events) {
    let hosting = host.hosting.clone();
    let dial: Dial = std::sync::Arc::new(move || {
        let hosting = hosting.clone();
        Box::pin(async move { Ok(from_another_host(hosting).await) })
    });
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
    tokio::spawn(session::serve(served.unwrap(), Relay::new(dial), None));
    let (client, _views, events) = Client::new(client.unwrap());
    (client, events)
}

/// What `events` is pushed up to and including the first program's end.
async fn pushed_until_an_end(who: &str, events: &mut Events) -> Vec<Pushed> {
    let mut pushed = Vec::new();
    loop {
        let told = timeout(PATIENCE, events.recv())
            .await
            .unwrap_or_else(|_| panic!("{who} was never told of the end"))
            .expect("the link is up");
        if let Told::Pushed(one) = told {
            let ended = matches!(one, Pushed::Ended { .. });
            pushed.push(one);
            if ended {
                return pushed;
            }
        }
    }
}

fn a_launch() -> Launch {
    Launch {
        program: "/bin/sh".into(),
        args: Vec::new(),
        cwd: None,
        set: Vec::new(),
        strip: Vec::new(),
        pass: Vec::new(),
        declared: Vec::new(),
        harness: None,
        git_hooks: None,
        reporting: None,
        confinement: None,
    }
}

async fn until(what: &str, done: impl Fn() -> bool) {
    timeout(PATIENCE, async {
        while !done() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what} never happened"));
}

#[tokio::test]
async fn a_project_the_host_does_not_hold_is_not_saved() {
    let host = a_host();
    let (client, _events) = a_window(&host).await;

    let refused = client
        .start_saving("01J9OTHER0000000000000000")
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_PROJECT)
    );
    assert!(host.savers.started().is_empty());
}

#[tokio::test]
async fn a_held_project_is_saved_by_one_saver_however_often_it_is_asked() {
    let host = a_host();
    let (client, _events) = a_window(&host).await;

    client.start_saving(PROJECT).await.expect("it saves");
    client.start_saving(PROJECT).await.expect("it still saves");

    let started = host.savers.started();
    assert_eq!(started.len(), 1, "a second saver was started");
    assert_eq!(started[0].0, PROJECT);
    assert_eq!(started[0].1, place(&host.project));
    assert_eq!(host.savers.running(), 1);
}

#[tokio::test]
async fn what_each_look_did_reaches_the_window() {
    let host = a_host();
    let (client, mut events) = a_window(&host).await;
    client.subscribe(Default::default()).await.unwrap();
    client.start_saving(PROJECT).await.unwrap();

    host.savers.looks(Looked {
        saved: true,
        moved_tree: false,
    });

    let told = timeout(PATIENCE, events.recv())
        .await
        .expect("the window is told")
        .expect("the link is up");
    assert!(
        matches!(
            told,
            Told::Pushed(Pushed::Saving {
                ref project,
                saved: true,
                moved_tree: false,
            }) if project == PROJECT
        ),
        "{told:?}"
    );
}

#[tokio::test]
async fn each_clone_of_a_project_has_its_own_saver_and_only_its_window_hears_its_looks() {
    // #2178: one project id, two folders, each held on a link of its own.
    let host = a_host();
    let (first, mut first_told) = a_window(&host).await;
    first.subscribe(Default::default()).await.unwrap();
    let clone = Host {
        hosting: host.hosting.clone(),
        savers: host.savers.clone(),
        project: tempfile::tempdir().unwrap(),
    };
    let (second, mut second_told) = a_window(&clone).await;
    second.subscribe(Default::default()).await.unwrap();

    first.start_saving(PROJECT).await.unwrap();
    second.start_saving(PROJECT).await.unwrap();
    host.savers.looks_at(
        &place(&clone.project),
        Looked {
            saved: true,
            moved_tree: false,
        },
    );

    let started = host.savers.started();
    assert_eq!(started.len(), 2, "each clone has a saver of its own");
    assert_ne!(started[0].1, started[1].1);
    assert!(matches!(
        timeout(PATIENCE, second_told.recv()).await,
        Ok(Some(Told::Pushed(Pushed::Saving { saved: true, .. })))
    ));
    assert!(
        timeout(Duration::from_millis(200), first_told.recv())
            .await
            .is_err(),
        "the first clone's window was told the second's look"
    );
}

#[tokio::test]
async fn a_link_that_holds_neither_of_two_clones_stops_neither_saver() {
    let host = a_host();
    let (first, _first_told) = a_window(&host).await;
    first.start_saving(PROJECT).await.unwrap();
    let clone = Host {
        hosting: host.hosting.clone(),
        savers: host.savers.clone(),
        project: tempfile::tempdir().unwrap(),
    };
    let (_second, _second_told) = a_window(&clone).await;
    let (neither, _told) = a_link(&host, Scope::LocalUi).await;

    let stopped = neither.stop_saving(PROJECT).await;

    assert_eq!(
        stopped.unwrap_err().refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_PROJECT)
    );
    assert_eq!(
        host.savers.running(),
        1,
        "the first clone's saver was stopped"
    );
}

#[tokio::test]
async fn only_the_link_that_may_save_hears_how_saving_goes() {
    let host = a_host();
    let path = host.project.path().to_str().unwrap().to_owned();
    let (window, mut window_events) = a_window(&host).await;
    window.subscribe(Default::default()).await.unwrap();
    let mut others: Vec<(String, Client, Events)> = Vec::new();
    for scope in [
        Scope::Terminal,
        Scope::FleetMcp,
        Scope::Approval,
        Scope::Editor,
    ] {
        let (client, events) = a_link(&host, scope).await;
        let subscribed = client.subscribe(Default::default()).await;
        if scope == Scope::Editor {
            // An editor subscribes to nothing at all.
            let refused = subscribed.unwrap_err();
            assert_eq!(
                refused.refusal().map(|r| r.code.as_str()),
                Some(session::NOT_ALLOWED)
            );
            continue;
        }
        subscribed.unwrap();
        others.push((scope.to_string(), client, events));
    }
    // A window's link that may save but does not hold the project.
    let (not_holding, not_holding_events) = a_link(&host, Scope::LocalUi).await;
    not_holding.subscribe(Default::default()).await.unwrap();
    others.push((
        "a window's link that does not hold it".into(),
        not_holding,
        not_holding_events,
    ));
    // Another host's link, holding the project as a relayed hold does.
    let (remote, remote_events) = {
        let (client, _views, events) = Client::new(from_another_host(host.hosting.clone()).await);
        (client, events)
    };
    remote
        .hold_project(PROJECT, &path)
        .await
        .expect("a link from another host holds it");
    remote.subscribe(Default::default()).await.unwrap();
    others.push(("remote-link".into(), remote, remote_events));
    // And a window on another host, holding it here through its host's relay.
    let (relayed, relayed_events) = a_window_relayed_from_another_host(&host).await;
    relayed
        .hold_project(PROJECT, &path)
        .await
        .expect("a relayed window holds it");
    relayed.subscribe(Default::default()).await.unwrap();
    others.push(("a relayed window".into(), relayed, relayed_events));

    window.start_saving(PROJECT).await.unwrap();
    host.savers.looks(Looked {
        saved: true,
        moved_tree: true,
    });
    let savers = host.savers.clone();
    let asking = tokio::task::spawn_blocking(move || savers.asks_the_window("alpha"));
    let mut heard_a_look = false;
    loop {
        let told = timeout(PATIENCE, window_events.recv())
            .await
            .expect("the window is told how saving goes")
            .expect("the link is up");
        match told {
            Told::Pushed(Pushed::Saving { .. }) => heard_a_look = true,
            Told::Pushed(Pushed::WhoIsMidTurn { question, .. }) => {
                window
                    .call(session::Command::MidTurn {
                        question,
                        chats: Some(Vec::new()),
                    })
                    .await
                    .expect("the answer is taken");
                break;
            }
            _ => {}
        }
    }
    assert!(heard_a_look, "the window was not told what the look did");
    assert_eq!(asking.await.unwrap(), Some(Vec::new()));
    // Every subscriber is told of a program's end: what each heard before it is all it heard
    // of the save.
    let program = window
        .start_program(StartProgram {
            launch: Some(a_launch()),
            ..StartProgram::of(PROJECT, ProgramKind::Harness)
        })
        .await
        .unwrap();
    window.stop(&program).await.unwrap();

    for (who, _client, mut events) in others {
        let pushed = pushed_until_an_end(&who, &mut events).await;
        assert!(
            !pushed
                .iter()
                .any(|one| matches!(one, Pushed::Saving { .. } | Pushed::WhoIsMidTurn { .. })),
            "{who} heard how saving goes: {pushed:?}"
        );
    }
}

#[tokio::test]
async fn a_window_whose_link_ends_without_a_word_leaves_the_project_saved_and_the_next_one_finds_the_same_saver()
 {
    let host = a_host();
    let (client, events) = a_window(&host).await;
    client.start_saving(PROJECT).await.unwrap();

    // The window goes, as a crash takes it: no stop, no let go.
    drop((client, events));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(host.savers.running(), 1, "the saver ended with the link");

    let (again, _events) = a_window(&host).await;
    again.start_saving(PROJECT).await.unwrap();
    again.fetch_project(PROJECT).await.unwrap();

    assert_eq!(
        host.savers.started().len(),
        1,
        "two savers saved the project"
    );
    assert_eq!(host.savers.running(), 1);
    assert_eq!(host.savers.poked(), [Poke::Fetch]);
}

#[tokio::test]
async fn stopping_saves_nothing_more_and_a_stop_with_no_saver_is_done() {
    let host = a_host();
    let (client, _events) = a_window(&host).await;
    client.start_saving(PROJECT).await.unwrap();

    client.stop_saving(PROJECT).await.expect("it stops");

    assert_eq!(host.savers.running(), 0);
    client.stop_saving(PROJECT).await.expect("nothing to stop");
}

#[tokio::test]
async fn a_project_every_window_let_go_of_is_saved_no_more() {
    let host = a_host();
    let (client, _events) = a_window(&host).await;
    client.start_saving(PROJECT).await.unwrap();

    client.let_go_of_project(PROJECT).await.unwrap();

    until("the saver stops", || host.savers.running() == 0).await;
}

#[tokio::test]
async fn a_program_that_ends_in_the_project_tells_its_saver_and_the_saver_sees_who_is_working() {
    let host = a_host();
    let (client, _events) = a_window(&host).await;
    client.start_saving(PROJECT).await.unwrap();
    let launch = Launch {
        program: "/bin/sh".into(),
        args: Vec::new(),
        cwd: Some("/somewhere/alpha".into()),
        set: Vec::new(),
        strip: Vec::new(),
        pass: Vec::new(),
        declared: Vec::new(),
        harness: None,
        git_hooks: None,
        reporting: None,
        confinement: None,
    };
    let program = client
        .start_program_told(StartProgram {
            launch: Some(launch),
            ..StartProgram::of(PROJECT, ProgramKind::Harness)
        })
        .await
        .expect("it starts");

    let working = host.savers.working();
    assert_eq!(working.len(), 1);
    assert_eq!(working[0].number, program.number.unwrap());
    assert_eq!(
        working[0].cwd.as_deref(),
        Some(std::path::Path::new("/somewhere/alpha"))
    );

    client.stop(&program.program).await.unwrap();

    until("the saver is told", || {
        host.savers.poked() == [Poke::ProgramEnded]
    })
    .await;
    assert!(
        host.savers.working().is_empty(),
        "an ended program works on"
    );
}

#[tokio::test]
async fn a_host_its_binary_gave_no_saver_refuses_to_save_with_why() {
    let hosting = Hosting::running_with(ComposesNothing, StandIns::default());
    let host = Host {
        hosting,
        savers: Savers::default(),
        project: tempfile::tempdir().unwrap(),
    };
    let (client, _events) = a_window(&host).await;

    let refused = client.start_saving(PROJECT).await.unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_STARTED)
    );
}

#[tokio::test]
async fn the_saver_asks_the_window_who_is_mid_turn_and_hears_its_answer() {
    let host = a_host();
    let (client, mut events) = a_window(&host).await;
    client.subscribe(Default::default()).await.unwrap();
    client.start_saving(PROJECT).await.unwrap();
    let savers = host.savers.clone();
    let asking = tokio::task::spawn_blocking(move || savers.asks_the_window("alpha"));

    let told = timeout(PATIENCE, events.recv())
        .await
        .expect("the window is asked")
        .expect("the link is up");
    let Told::Pushed(Pushed::WhoIsMidTurn {
        question,
        project,
        workspace,
    }) = told
    else {
        panic!("{told:?}");
    };
    assert_eq!((project.as_str(), workspace.as_str()), (PROJECT, "alpha"));
    client
        .call(session::Command::MidTurn {
            question,
            chats: Some(vec!["alpha 1".into()]),
        })
        .await
        .expect("the answer is taken");

    assert_eq!(
        asking.await.unwrap(),
        Some(vec!["alpha 1".to_owned()]),
        "the saver did not hear the window"
    );
}

#[tokio::test]
async fn with_no_window_linked_the_saver_is_told_nobody_can_say_at_once() {
    let host = a_host();
    let (client, events) = a_window(&host).await;
    client.start_saving(PROJECT).await.unwrap();
    drop((client, events));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let savers = host.savers.clone();

    let started = std::time::Instant::now();
    let said = tokio::task::spawn_blocking(move || savers.asks_the_window("alpha"))
        .await
        .unwrap();

    assert_eq!(said, None);
    assert!(
        started.elapsed() < purlis_host::ASK_WITHIN,
        "it waited for nobody"
    );
}

#[tokio::test]
async fn a_window_that_does_not_answer_leaves_the_saver_told_nobody_can_say_in_time() {
    let host = a_host();
    let (client, _events) = a_window(&host).await;
    client.subscribe(Default::default()).await.unwrap();
    client.start_saving(PROJECT).await.unwrap();
    let savers = host.savers.clone();

    let started = std::time::Instant::now();
    let said = tokio::task::spawn_blocking(move || savers.asks_the_window("alpha"))
        .await
        .unwrap();

    assert_eq!(said, None);
    assert!(started.elapsed() >= purlis_host::ASK_WITHIN);
    assert!(started.elapsed() < purlis_host::ASK_WITHIN + Duration::from_secs(2));
}

#[tokio::test]
async fn a_stop_that_arrives_mid_save_is_answered_once_the_save_has_let_go() {
    let host = a_host();
    let (client, _events) = a_window(&host).await;
    client.start_saving(PROJECT).await.unwrap();
    host.savers.mid_save();

    let mut stopping = Box::pin(client.stop_saving(PROJECT));
    assert!(
        timeout(Duration::from_millis(300), &mut stopping)
            .await
            .is_err(),
        "the stop was answered while a save still ran"
    );
    assert_eq!(host.savers.running(), 1);

    host.savers.finish_saving();
    timeout(PATIENCE, stopping)
        .await
        .expect("answered once the save let go")
        .expect("it stops");
    assert_eq!(host.savers.running(), 0);
}
