// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Host A relays its window to host B (ADR 0078 §4, #2042): what the window sends host A is
//! sent on to host B and answered with host B's answer, host B's views are drawn and
//! acknowledged in the window as if host A had opened them, and what host B pushes reaches the
//! window. Over the link between hosts, host B serves `remote-link`, which takes the window's
//! start and its replies to hook lines as its host relays them (ruled 2026-10-15), and host A
//! relays those for its window alone. Host B refuses a start without the sandbox whoever sent
//! it on.
//! Host B's programs are stand-ins, so none of this needs a terminal.

#![cfg(unix)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use purlis_host::standin::{Lines, StandIns};
use purlis_host::{Compose, Dial, Hosting, Relay, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};
use purlis_session_protocol::link::{self, Acceptor, Link};
use purlis_session_protocol::session::{
    self, Client, Events, Exit, ProgramKind, Pushed, StartProgram, Told, ViewKind,
};
use purlis_session_protocol::view::{Chunk, Viewer};
use purlis_terminal::session::Spec;
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// Composes every start as `/bin/echo`.
struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// A `local-ui` link to `host`.
async fn local_ui(host: impl session::Host) -> Link {
    as_scope(Scope::LocalUi, host).await
}

/// A link to `host` admitted as `scope`.
async fn as_scope(scope: Scope, host: impl session::Host) -> Link {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), host, None));
    client.unwrap()
}

fn identity(device: &str) -> Identity {
    Identity {
        device: device.into(),
        key: LinkKey::generate().unwrap(),
    }
}

/// A link between the hosts, as the link between hosts carries it: host B serves it as
/// `remote-link`.
async fn between_hosts(b: Hosting) -> Link {
    let (a_end, b_end) = duplex(256 * 1024);
    let host_a = identity("01JA0DESKT0P00000000000000");
    let host_b = identity("01JB0RUNNER0000000000000000");
    let pinned = |identity: &Identity| Pinned {
        device: identity.device.clone(),
        key: identity.key.public(),
    };
    let peers = pinned(&host_a);
    let b_pinned = pinned(&host_b);
    tokio::spawn(async move {
        let answered = hostlink::answer(
            b_end,
            Handshake::THIS_BUILD,
            &host_b,
            &peers,
            Liveness::default(),
        )
        .await
        .unwrap();
        session::serve(answered.link, b, None).await
    });
    hostlink::connect(
        a_end,
        Handshake::THIS_BUILD,
        &host_a,
        &b_pinned,
        Liveness::default(),
    )
    .await
    .unwrap()
}

struct Window {
    client: Client,
    views: Acceptor,
    events: Events,
    runs: StandIns,
}

/// A window on host A, which relays to host B over what `link_to_b` dials.
async fn a_window(link_to_b: impl Fn(Hosting) -> Dial) -> Window {
    let runs = StandIns::default();
    let b = Hosting::running_with(Echo, runs.clone());
    let relay = Relay::new(link_to_b(b));
    let (client, views, events) = Client::new(local_ui(relay).await);
    Window {
        client,
        views,
        events,
        runs,
    }
}

/// Host B reached over a `local-ui` link, so what the relay does is seen apart from what the
/// scope of a link between hosts lets through.
fn as_its_window(b: Hosting) -> Dial {
    Arc::new(move || {
        let b = b.clone();
        Box::pin(async move { Ok(local_ui(b).await) })
    })
}

fn over_the_link(b: Hosting) -> Dial {
    Arc::new(move || {
        let b = b.clone();
        Box::pin(async move { Ok(between_hosts(b).await) })
    })
}

async fn started(window: &Window) -> String {
    let project = tempfile::tempdir().unwrap();
    window
        .client
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .expect("host B holds the project");
    let program = window
        .client
        .start_program(StartProgram::of(PROJECT, ProgramKind::Harness))
        .await
        .expect("host B starts it");
    std::mem::forget(project);
    program
}

#[tokio::test]
async fn what_the_window_starts_through_host_a_runs_on_host_b_and_is_listed() {
    let window = a_window(as_its_window).await;

    let program = started(&window).await;

    assert_eq!(window.runs.ran().len(), 1, "host B ran it");
    let chats = window.client.list().await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].chat, program);
}

#[tokio::test]
async fn host_bs_view_is_drawn_in_the_window_and_what_is_typed_reaches_its_program() {
    let mut window = a_window(as_its_window).await;
    let program = started(&window).await;
    window.runs.last().writes(b"ready> ");

    window
        .client
        .attach_view(&program, ViewKind::Terminal)
        .await
        .expect("host B opens a view");
    let mut reader = Viewer::default()
        .accept(
            timeout(PATIENCE, window.views.accept())
                .await
                .unwrap()
                .unwrap(),
        )
        .await
        .unwrap();
    window.client.write(&program, b"hello").await.unwrap();

    let (mut snapshot, mut live) = (Vec::new(), Vec::new());
    while live != b"hello" {
        let chunk = timeout(PATIENCE, reader.next())
            .await
            .expect("the view draws")
            .expect("the view is open")
            .unwrap();
        let drawn = match chunk {
            Chunk::Snapshot(bytes) => {
                snapshot.extend_from_slice(&bytes);
                bytes.len()
            }
            Chunk::Live(bytes) => {
                live.extend_from_slice(&bytes);
                bytes.len()
            }
        };
        reader
            .ack(drawn)
            .await
            .expect("the acknowledgement reaches host B");
    }
    assert_eq!(snapshot, b"ready> ");
    assert_eq!(window.runs.last().seen().typed, b"hello");
}

#[tokio::test]
async fn the_window_is_told_when_host_bs_program_ends() {
    let mut window = a_window(as_its_window).await;
    window
        .client
        .subscribe(session::Since::new())
        .await
        .unwrap();
    let program = started(&window).await;

    window.client.stop(&program).await.unwrap();

    let told = timeout(PATIENCE, window.events.recv())
        .await
        .expect("the window is told")
        .unwrap();
    assert_eq!(
        told,
        Told::Pushed(Pushed::Ended {
            program,
            run: None,
            exit: Exit::Signal("Interrupt".into()),
        })
    );
}

#[tokio::test]
async fn over_the_link_between_hosts_the_windows_start_runs_on_host_b() {
    let window = a_window(over_the_link).await;

    let program = started(&window).await;

    assert_eq!(window.runs.ran().len(), 1, "host B ran it");
    assert_eq!(window.client.list().await.unwrap()[0].chat, program);
}

#[tokio::test]
async fn the_windows_reply_to_host_bs_hook_line_reaches_host_b() {
    let window = a_window(over_the_link).await;
    started(&window).await;
    let reply = || session::Command::ReplyHeard {
        project: PROJECT.into(),
        heard: 1,
        reply: serde_json::json!("allow"),
    };

    let from_the_window = window.client.call(reply()).await;

    // Host B has no line 1 to reply to: what matters is that it heard the reply at all.
    if let Err(refused) = &from_the_window {
        assert_ne!(
            refused.refusal().map(|refusal| refusal.code.as_str()),
            Some(session::NOT_ALLOWED),
            "{refused}"
        );
    }
}

#[tokio::test]
async fn host_a_refuses_a_start_without_the_sandbox_from_any_client_but_its_window() {
    let runs = StandIns::default();
    let relay = Relay::new(over_the_link(Hosting::running_with(Echo, runs.clone())));
    let (terminal, _views, _events) = Client::new(as_scope(Scope::Terminal, relay).await);

    let refused = terminal
        .start_program(StartProgram {
            without_sandbox: true,
            ..StartProgram::of(PROJECT, ProgramKind::Shell)
        })
        .await
        .unwrap_err();

    let refusal = refused.refusal().expect("host A's refusal");
    assert_eq!(refusal.code, session::NOT_ALLOWED);
    assert!(runs.ran().is_empty());
}

/// **Only the window's start keeps the ids it names on host B** (#2446): host B keeps the ids
/// its peer sends on, so a start from any other client of host A is sent on without them, and
/// host B gives it ids of its own.
#[tokio::test]
async fn a_start_from_a_client_that_is_not_the_window_is_sent_on_without_its_ids() {
    let runs = StandIns::default();
    let log = Log::default();
    let b = Hosting::running_with(WritesItsRun, runs.clone()).recording_with(log.clone());
    let relay = Relay::new(over_the_link(b));
    let window = Client::new(local_ui(relay.clone()).await).0;
    let project = tempfile::tempdir().unwrap();
    window
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .expect("host B holds the project");
    let (terminal, _views, _events) = Client::new(as_scope(Scope::Terminal, relay).await);
    let named = || StartProgram {
        program: Some("01J9CHAT000000000000000000".into()),
        run: Some("01J9RUN0000000000000000000".into()),
        ..StartProgram::of(PROJECT, ProgramKind::Shell)
    };

    let theirs = terminal.start_program(named()).await.expect("started");
    let the_windows = window.start_program(named()).await.expect("started");

    assert_ne!(theirs, "01J9CHAT000000000000000000");
    assert_eq!(the_windows, "01J9CHAT000000000000000000");
    assert_eq!(runs.ran().len(), 2);
    // Each began a run host B wrote: the terminal's under a run host B minted.
    let written = log.0.lock().unwrap().clone();
    let run_of = |chat: &str| {
        written
            .iter()
            .find(|entry| entry["chat"] == chat)
            .map(|entry| entry["run"].as_str().unwrap_or_default().to_owned())
    };
    let theirs_ran = run_of(&theirs).expect("the terminal's start wrote its run");
    assert!(!theirs_ran.is_empty());
    assert_ne!(theirs_ran, "01J9RUN0000000000000000000");
    assert_eq!(
        run_of("01J9CHAT000000000000000000").as_deref(),
        Some("01J9RUN0000000000000000000")
    );
}

/// Composes every start as `/bin/echo`, writing its run as a host's binary does
/// (`purlis_core::hoststart`): a start that names no chat or run writes none.
struct WritesItsRun;

impl Compose for WritesItsRun {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        if let (Some(chat), Some(run)) = (&starting.start.program, &starting.start.run) {
            starting
                .beside
                .record(serde_json::json!({ "kind": "run.started", "chat": chat, "run": run }))?;
        }
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// Host B's event log: what was written in it.
#[derive(Clone, Default)]
struct Log(Arc<std::sync::Mutex<Vec<serde_json::Value>>>);

impl purlis_host::Record for Log {
    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        self.0.lock().unwrap().push(entry);
        Ok(())
    }

    fn ended(&self, _ended: &purlis_host::Ended<'_>) {}
}

/// A client of host A whose scope may hold a project and reply to hook lines there, as a link
/// between hosts may, but is not its window.
async fn another_host_on(relay: Relay) -> Client {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect_as_a_device(a, session::speaks()),
        link::serve_as_a_device(b, session::speaks(), link::ProvenDevice::stand_in())
    );
    tokio::spawn(session::serve(served.unwrap(), relay, None));
    Client::new(client.unwrap()).0
}

#[tokio::test]
async fn host_a_relays_the_windows_own_commands_for_its_window_alone() {
    let runs = StandIns::default();
    let dials = Arc::new(AtomicUsize::new(0));
    let relay = Relay::new(counting(
        &dials,
        over_the_link(Hosting::running_with(Echo, runs.clone())),
    ));
    let another_host = another_host_on(relay).await;
    let project = tempfile::tempdir().unwrap();

    let held = another_host
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await;
    let replied = another_host
        .call(session::Command::ReplyHeard {
            project: PROJECT.into(),
            heard: 1,
            reply: serde_json::json!("allow"),
        })
        .await;

    for refused in [held.unwrap_err(), replied.unwrap_err()] {
        let refusal = refused.refusal().expect("host A's relay refuses it");
        assert_eq!(refusal.code, session::NOT_ALLOWED);
        assert!(refusal.why.contains("window only"), "{}", refusal.why);
    }
    assert_eq!(dials.load(Ordering::SeqCst), 0, "nothing was sent on");
    assert!(runs.ran().is_empty());
}

/// `dial`, counting each time it is called.
fn counting(dials: &Arc<AtomicUsize>, dial: Dial) -> Dial {
    let dials = Arc::clone(dials);
    Arc::new(move || {
        dials.fetch_add(1, Ordering::SeqCst);
        dial()
    })
}

/// A host that answers `list` and counts the links that ended on it.
#[derive(Clone, Default)]
struct Ends(Arc<AtomicUsize>);

impl session::Host for Ends {
    async fn call(
        &self,
        _command: session::Command,
        _peer: &session::Peer,
    ) -> Result<session::Answer, session::Refusal> {
        Ok(session::Answer::Chats(Vec::new()))
    }

    async fn link_ended(&self, _peer: &session::Peer) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn a_window_link_that_ends_while_host_a_dials_leaves_no_link_to_host_b() {
    let host_b = Ends::default();
    let dials = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let relay = Relay::new({
        let (host_b, dials, gate) = (host_b.clone(), Arc::clone(&dials), Arc::clone(&gate));
        Arc::new(move || {
            dials.fetch_add(1, Ordering::SeqCst);
            let (host_b, gate) = (host_b.clone(), Arc::clone(&gate));
            Box::pin(async move {
                let _open = gate.acquire().await.unwrap();
                Ok(local_ui(host_b).await)
            })
        })
    });
    let (window, views, events) = Client::new(local_ui(relay).await);
    // The window's first command makes host A dial, and the dial waits at the gate; a second
    // about the same program waits its turn behind it, so host A takes it up only after the
    // window has gone.
    let asked = |window: Arc<Client>| async move {
        window
            .call(session::Command::Readiness {
                program: "01J9ZQ3V7K8M2N4P6R8T0V2X4Z".into(),
            })
            .await
    };
    let window = Arc::new(window);
    let first = tokio::spawn(asked(Arc::clone(&window)));
    let second = tokio::spawn(asked(Arc::clone(&window)));
    while dials.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    first.abort();
    second.abort();
    drop(window);
    // The window goes while host A dials; then the dial finishes.
    drop((views, events));
    tokio::time::sleep(Duration::from_millis(200)).await;
    gate.add_permits(1);

    let ended = timeout(PATIENCE, async {
        while host_b.0.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        ended.is_ok(),
        "the link host A made for a gone window was left open"
    );
    assert_eq!(
        dials.load(Ordering::SeqCst),
        1,
        "host A dialed again for a gone window"
    );
}

/// A host that counts the commands it hears and the links that ended on it, and answers none.
#[derive(Clone, Default)]
struct HearsAndEnds {
    heard: Arc<AtomicUsize>,
    ended: Arc<AtomicUsize>,
}

impl session::Host for HearsAndEnds {
    async fn call(
        &self,
        _command: session::Command,
        _peer: &session::Peer,
    ) -> Result<session::Answer, session::Refusal> {
        self.heard.fetch_add(1, Ordering::SeqCst);
        std::future::pending().await
    }

    async fn link_ended(&self, _peer: &session::Peer) {
        self.ended.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn a_window_that_ends_while_host_a_dials_sends_host_b_nothing_and_its_link_is_closed() {
    let host_b = HearsAndEnds::default();
    let dials = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let relay = Relay::new({
        let (host_b, dials, gate) = (host_b.clone(), Arc::clone(&dials), Arc::clone(&gate));
        Arc::new(move || {
            dials.fetch_add(1, Ordering::SeqCst);
            let (host_b, gate) = (host_b.clone(), Arc::clone(&gate));
            Box::pin(async move {
                let _open = gate.acquire().await.unwrap();
                Ok(local_ui(host_b).await)
            })
        })
    });
    let (window, views, events) = Client::new(local_ui(relay).await);
    // The window's one command makes host A dial, and the dial waits at the gate.
    let window = Arc::new(window);
    let asking = tokio::spawn({
        let window = Arc::clone(&window);
        async move { window.list().await }
    });
    while dials.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // The window goes while host A dials; then the dial finishes.
    asking.abort();
    drop(window);
    drop((views, events));
    tokio::time::sleep(Duration::from_millis(200)).await;
    gate.add_permits(1);

    let ended = timeout(PATIENCE, async {
        while host_b.ended.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        ended.is_ok(),
        "the link host A dialed for a gone window was kept"
    );
    assert_eq!(
        host_b.heard.load(Ordering::SeqCst),
        0,
        "host B was sent a command of a gone window"
    );
}

/// Host B reached as by [`as_its_window`], over a line `lines` cuts.
fn cuttable(b: Hosting, lines: Lines) -> Dial {
    Arc::new(move || {
        let (b, lines) = (b.clone(), lines.clone());
        Box::pin(async move {
            let (a_end, b_end) = lines.line();
            let (client, served) = tokio::join!(
                link::connect(
                    a_end,
                    session::speaks(),
                    Scope::LocalUi,
                    HELD.of(Scope::LocalUi)
                ),
                link::serve_any(b_end, session::speaks(), &HELD)
            );
            tokio::spawn(session::serve(served.unwrap(), b, None));
            client.map_err(|why| why.to_string())
        })
    })
}

#[tokio::test]
async fn a_dialed_link_that_ends_is_told_to_the_window_and_so_is_the_next_one_up() {
    let lines = Lines::default();
    let mut window = a_window(|b| cuttable(b, lines.clone())).await;
    window
        .client
        .subscribe(session::Since::new())
        .await
        .unwrap();

    lines.cut();
    let told = timeout(PATIENCE, window.events.recv())
        .await
        .expect("the window is told")
        .unwrap();
    assert!(
        matches!(told, Told::Pushed(Pushed::LinkDown { .. })),
        "{told:?}"
    );

    // The next command dials again, and the window is told the link is back.
    timeout(PATIENCE, async {
        while window.client.list().await.is_err() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("host A dials again");
    let told = timeout(PATIENCE, window.events.recv())
        .await
        .expect("the window is told")
        .unwrap();
    assert_eq!(told, Told::Pushed(Pushed::LinkBack {}));
}

#[tokio::test]
async fn host_a_sends_no_start_from_kept_logins_on_to_host_b() {
    let runs = StandIns::default();
    let dials = Arc::new(AtomicUsize::new(0));
    let relay = Relay::new(counting(
        &dials,
        as_its_window(Hosting::running_with(Echo, runs.clone())),
    ));
    let (client, _views, _events) = Client::new(local_ui(relay).await);
    let refused = client
        .start_program(StartProgram {
            kept: Some(purlis_session_protocol::browsing::KeptLogin {
                profile: "claude".into(),
                persona: "steward".into(),
                login: "shop-login".into(),
                identity: "0123456789abcdef0123456789abcdef".into(),
                address: "https://shop.example.test/login".into(),
            }),
            ..StartProgram::of(PROJECT, ProgramKind::Browser)
        })
        .await
        .unwrap_err();
    let refusal = refused.refusal().expect("host A's relay refuses it");
    assert_eq!(refusal.code, session::NOT_ALLOWED);
    assert_eq!(dials.load(Ordering::SeqCst), 0, "nothing was sent on");
}
