// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's confinement on a runner (#2217): host A, the desktop's, relays its window to host B,
//! the runner's, over the link between hosts. Host B starts the confinement the window plans
//! with its own binary's [`purlis_host::Confine`], tells the window what its proxy does through
//! host A, and takes the window's answers and a withdrawn port; host A relays all of it for its
//! window alone. A chat's harness on a runner is started by its names, composed by the runner
//! with its own sandbox: host B refuses the window's launch of one. The confinement is a
//! stand-in that listens on nothing, and the programs are stand-ins, so none of this needs a
//! terminal; the real proxy through a link is the app's behaviour suite, on CI.

#![cfg(unix)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use purlis_host::standin::{StandInConfines, StandIns};
use purlis_host::{Compose, Dial, Hosting, Planned, Relay, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::session::{
    self, Answer, Beside, Client, Command, Confine, ConfinePlan, Events, Launch, Layer, Listed,
    ProgramKind, Pushed, ReachChange, SandboxTold, StartProgram, Told,
};
use purlis_terminal::session::Spec;
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const CONFINEMENT: &str = "01J9CNFNMNT0000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// Composes every start as `/bin/echo`; one started by its names, in a confinement of the
/// runner's own, as its binary composes it with its own sandbox.
struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        if starting.start.launch.is_none() {
            starting.beside.confine(&planned().plan)?;
        }
        Ok(Spec::new("/bin/echo", starting.size))
    }
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

/// A link to host B over the link between hosts, which host B serves as `remote-link`.
async fn between_hosts(b: Hosting) -> Link {
    let (a_end, b_end) = duplex(256 * 1024);
    let host_a = identity("01JA0DESKT0P00000000000000");
    let host_b = identity("01JB0RUNNER0000000000000000");
    let pinned = |identity: &Identity| Pinned {
        device: identity.device.clone(),
        key: identity.key.public(),
    };
    let (peer, b_pinned) = (pinned(&host_a), pinned(&host_b));
    tokio::spawn(async move {
        let answered = hostlink::answer(
            b_end,
            Handshake::THIS_BUILD,
            &host_b,
            &peer,
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

/// Host B, the runner, its confinements and programs stand-ins; and host A's relay to it,
/// counting each link it dials.
struct Hosts {
    relay: Relay,
    runner: Hosting,
    confines: StandInConfines,
    runs: StandIns,
    dials: Arc<AtomicUsize>,
    _project: tempfile::TempDir,
}

fn hosts() -> Hosts {
    let (confines, runs) = (StandInConfines::default(), StandIns::default());
    let b = Hosting::confining(Echo, confines.clone(), runs.clone());
    let b_itself = b.clone();
    let dials = Arc::new(AtomicUsize::new(0));
    let dial: Dial = {
        let dials = Arc::clone(&dials);
        Arc::new(move || {
            dials.fetch_add(1, Ordering::SeqCst);
            let b = b.clone();
            Box::pin(async move { Ok(between_hosts(b).await) })
        })
    };
    Hosts {
        relay: Relay::new(dial),
        runner: b_itself,
        confines,
        runs,
        dials,
        _project: tempfile::tempdir().unwrap(),
    }
}

/// A client of host A as `scope`, subscribed to what it is pushed.
async fn a_client(hosts: &Hosts, scope: Scope) -> (Client, Events) {
    let (client, _views, events) = Client::new(as_scope(scope, hosts.relay.clone()).await);
    client
        .subscribe(Default::default())
        .await
        .expect("subscribed");
    (client, events)
}

/// A client of host A whose scope may confine a chat on its own host, as a link between hosts
/// may, but is not its window; subscribed to what it is pushed.
async fn another_host(hosts: &Hosts) -> (Client, Events) {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect_as_a_device(a, session::speaks()),
        link::serve_as_a_device(b, session::speaks(), link::ProvenDevice::stand_in())
    );
    tokio::spawn(session::serve(served.unwrap(), hosts.relay.clone(), None));
    let (client, _views, events) = Client::new(client.unwrap());
    client
        .subscribe(Default::default())
        .await
        .expect("subscribed");
    (client, events)
}

/// The window on host A, holding the project on host B.
async fn the_window(hosts: &Hosts) -> (Client, Events) {
    let (window, events) = a_client(hosts, Scope::LocalUi).await;
    window
        .hold_project(PROJECT, hosts._project.path().to_str().unwrap())
        .await
        .expect("host B holds the project");
    (window, events)
}

fn planned() -> Confine {
    Confine {
        confinement: CONFINEMENT.into(),
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

async fn confine(client: &Client) -> Result<session::Confined, session::Refusal> {
    match client.call(Command::Confine(Box::new(planned()))).await {
        Ok(Answer::Confined(confined)) => Ok(confined),
        Ok(other) => panic!("answered {other:?}"),
        Err(why) => Err(why.refusal().cloned().expect("a refusal")),
    }
}

fn a_launch(harness: Option<&str>, confinement: Option<&str>) -> StartProgram {
    StartProgram {
        launch: Some(Launch {
            program: "/bin/echo".into(),
            args: Vec::new(),
            cwd: None,
            set: Vec::new(),
            strip: Vec::new(),
            pass: Vec::new(),
            declared: Vec::new(),
            harness: harness.map(str::to_owned),
            git_hooks: None,
            reporting: None,
            confinement: confinement.map(str::to_owned),
        }),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

/// The next sandbox notice `events` is pushed, past any other.
async fn next_sandbox(events: &mut Events) -> Option<Pushed> {
    timeout(PATIENCE, async {
        loop {
            match events.recv().await? {
                Told::Pushed(pushed @ Pushed::Sandbox { .. }) => return Some(pushed),
                _ => continue,
            }
        }
    })
    .await
    .ok()
    .flatten()
}

#[tokio::test]
async fn the_windows_confinement_is_started_on_the_runner_and_says_where_it_is() {
    let hosts = hosts();
    let (window, _events) = the_window(&hosts).await;

    let confined = confine(&window).await.expect("confined on host B");

    assert_eq!(confined.confinement, CONFINEMENT);
    assert_eq!((confined.http, confined.socks), (50811, 50812));
    assert_eq!(hosts.confines.seen()[0].confine, planned());
}

#[tokio::test]
async fn a_chat_runs_in_the_confinement_the_window_made_on_the_runner_and_its_proxy_asks() {
    let hosts = hosts();
    let (window, _events) = the_window(&hosts).await;
    confine(&window).await.expect("confined");

    window
        .start_program(a_launch(None, Some(CONFINEMENT)))
        .await
        .expect("it runs on host B");

    assert_eq!(hosts.runs.ran().len(), 1);
    assert!(
        hosts.confines.askable(CONFINEMENT),
        "a window hears its asks through host A"
    );
}

#[tokio::test]
async fn what_a_proxy_on_the_runner_did_is_told_to_the_window() {
    let hosts = hosts();
    let (window, mut events) = the_window(&hosts).await;
    confine(&window).await.expect("confined");
    let asked = SandboxTold::Asked {
        targets: vec!["pypi.example.com:443".into()],
    };

    hosts.confines.proxy_did(CONFINEMENT, asked.clone());

    assert_eq!(
        next_sandbox(&mut events).await,
        Some(Pushed::Sandbox {
            confinement: CONFINEMENT.into(),
            told: asked,
        })
    );
}

#[tokio::test]
async fn the_windows_answer_and_a_withdrawn_port_reach_the_confinement_on_the_runner() {
    let hosts = hosts();
    let (window, _events) = the_window(&hosts).await;
    confine(&window).await.expect("confined");

    let answered = window
        .call(Command::ChangeReach {
            confinement: CONFINEMENT.into(),
            change: ReachChange::Allow {
                host: "pypi.example.com".into(),
                layer: Layer::Chat,
            },
        })
        .await
        .expect("answered on host B");
    window
        .call(Command::ChangeReach {
            confinement: CONFINEMENT.into(),
            change: ReachChange::WithdrawPort { port: 5432 },
        })
        .await
        .expect("withdrawn on host B");
    let now = match window
        .call(Command::ReachOf {
            confinement: CONFINEMENT.into(),
        })
        .await
        .expect("read on host B")
    {
        Answer::Reach(now) => now,
        other => panic!("answered {other:?}"),
    };

    assert_eq!(
        answered,
        Answer::ReachChanged {
            retry: vec!["pypi.example.com:443".into()]
        }
    );
    assert_eq!(
        hosts.confines.seen()[0].changes,
        [
            ReachChange::Allow {
                host: "pypi.example.com".into(),
                layer: Layer::Chat,
            },
            ReachChange::WithdrawPort { port: 5432 },
        ]
    );
    assert_eq!(
        now.allowed_live,
        [Listed {
            host: "pypi.example.com".into(),
            layer: Layer::Chat
        }]
    );
}

#[tokio::test]
async fn host_a_relays_a_chats_confinement_for_its_window_alone() {
    let hosts = hosts();
    let (other, _events) = another_host(&hosts).await;
    let dialed = hosts.dials.load(Ordering::SeqCst);

    let refused = [
        confine(&other).await.unwrap_err(),
        other
            .call(Command::ChangeReach {
                confinement: CONFINEMENT.into(),
                change: ReachChange::Allow {
                    host: "pypi.example.com".into(),
                    layer: Layer::Chat,
                },
            })
            .await
            .unwrap_err()
            .refusal()
            .cloned()
            .expect("a refusal"),
    ];

    for refusal in refused {
        assert_eq!(refusal.code, session::NOT_ALLOWED);
        assert!(refusal.why.contains("window only"), "{}", refusal.why);
    }
    assert_eq!(
        hosts.dials.load(Ordering::SeqCst),
        dialed,
        "nothing was sent on"
    );
    assert!(hosts.confines.seen().is_empty());
}

#[tokio::test]
async fn a_client_of_host_a_that_is_not_its_window_hears_nothing_of_the_runners_sandbox() {
    let hosts = hosts();
    let (window, mut window_events) = the_window(&hosts).await;
    let (_other, mut other_events) = another_host(&hosts).await;
    confine(&window).await.expect("confined");

    hosts.confines.proxy_did(
        CONFINEMENT,
        SandboxTold::Refused {
            host: "tracker.example.com".into(),
            port: 443,
        },
    );

    assert!(next_sandbox(&mut window_events).await.is_some());
    assert!(
        timeout(Duration::from_millis(300), next_sandbox(&mut other_events))
            .await
            .ok()
            .flatten()
            .is_none(),
        "a client that may not confine hears nothing of a sandbox"
    );
}

#[tokio::test]
async fn the_runner_refuses_the_windows_launch_of_a_harness_which_it_composes_by_its_names() {
    let hosts = hosts();
    let (window, _events) = the_window(&hosts).await;

    let refused = window
        .start_program(a_launch(Some("claude"), None))
        .await
        .unwrap_err();

    let refusal = refused.refusal().expect("host B's refusal");
    assert_eq!(refusal.code, session::NOT_ALLOWED);
    assert!(refusal.why.contains("by its names"), "{}", refusal.why);
    assert!(hosts.runs.ran().is_empty());

    window
        .start_program(StartProgram::of(PROJECT, ProgramKind::Harness))
        .await
        .expect("a start by its names runs on host B");
    assert_eq!(hosts.runs.ran().len(), 1);
}

/// The runner's own window: a machine that is someone's desktop and a runner both. It holds
/// the project at the place host A's window holds it on host B.
async fn the_runners_own_window(hosts: &Hosts) -> (Client, Events) {
    let (window, _views, events) =
        Client::new(as_scope(Scope::LocalUi, hosts.runner.clone()).await);
    window
        .hold_project(PROJECT, hosts._project.path().to_str().unwrap())
        .await
        .expect("held on host B");
    window
        .subscribe(Default::default())
        .await
        .expect("subscribed");
    (window, events)
}

#[tokio::test]
async fn the_desktops_window_may_not_act_on_or_hear_a_confinement_the_runners_own_window_made() {
    let hosts = hosts();
    let (window, mut events) = the_window(&hosts).await;
    let (runners, _runners_events) = the_runners_own_window(&hosts).await;
    confine(&runners)
        .await
        .expect("confined by the runner's window");

    let refused = window
        .call(Command::ChangeReach {
            confinement: CONFINEMENT.into(),
            change: ReachChange::WithdrawPort { port: 5432 },
        })
        .await
        .unwrap_err();
    hosts.confines.proxy_did(
        CONFINEMENT,
        SandboxTold::Refused {
            host: "tracker.example.com".into(),
            port: 443,
        },
    );

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_CHAT)
    );
    assert!(hosts.confines.seen()[0].changes.is_empty(), "unchanged");
    assert!(
        timeout(Duration::from_millis(300), next_sandbox(&mut events))
            .await
            .ok()
            .flatten()
            .is_none(),
        "the desktop's window hears nothing of it"
    );
}

#[tokio::test]
async fn the_runners_confinement_of_a_chat_it_composed_is_not_the_desktop_windows_to_act_on() {
    let hosts = hosts();
    let (window, mut events) = the_window(&hosts).await;
    window
        .start_program(StartProgram::of(PROJECT, ProgramKind::Harness))
        .await
        .expect("composed on host B by its names");
    let composed = hosts.confines.seen()[0].confine.confinement.clone();

    let refused = window
        .call(Command::LetGoOfConfinement {
            confinement: composed.clone(),
        })
        .await;
    let withdrawn = window
        .call(Command::ChangeReach {
            confinement: composed.clone(),
            change: ReachChange::WithdrawPort { port: 5432 },
        })
        .await
        .unwrap_err();
    hosts.confines.proxy_did(
        &composed,
        SandboxTold::Refused {
            host: "tracker.example.com".into(),
            port: 443,
        },
    );

    assert_eq!(
        refused.unwrap_err().refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_CHAT)
    );
    assert!(
        !hosts.confines.ended(&composed),
        "the chat keeps its network"
    );
    assert_eq!(
        withdrawn.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_CHAT)
    );
    assert!(
        timeout(Duration::from_millis(300), next_sandbox(&mut events))
            .await
            .ok()
            .flatten()
            .is_none(),
        "nobody named it to the window"
    );
}

/// The id the window asks the runner to give the confinement it composes for a chat it starts
/// by its names (#2407).
const NAMED: &str = "01J9CNFNMNT0000000000000002";

/// A start by its names, naming the confinement the runner composes for it.
fn by_its_names() -> StartProgram {
    StartProgram {
        confinement: Some(NAMED.into()),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
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
async fn a_chat_the_runner_composes_by_its_names_is_heard_and_answered_by_the_window_that_started_it()
 {
    let hosts = hosts();
    let (window, mut events) = the_window(&hosts).await;

    let started = window
        .start_program_told(by_its_names())
        .await
        .expect("composed on host B");
    let asked = SandboxTold::Asked {
        targets: vec!["pypi.example.com:443".into()],
    };
    hosts.confines.proxy_did(NAMED, asked.clone());
    let answered = window
        .call(Command::ChangeReach {
            confinement: NAMED.into(),
            change: ReachChange::Allow {
                host: "pypi.example.com".into(),
                layer: Layer::Chat,
            },
        })
        .await
        .expect("answered on host B");
    window
        .call(Command::ChangeReach {
            confinement: NAMED.into(),
            change: ReachChange::WithdrawPort { port: 5432 },
        })
        .await
        .expect("withdrawn on host B");

    assert_eq!(
        started.confined.map(|confined| confined.confinement),
        Some(NAMED.to_owned())
    );
    assert!(hosts.confines.askable(NAMED), "its proxy asks the window");
    assert_eq!(
        next_sandbox(&mut events).await,
        Some(Pushed::Sandbox {
            confinement: NAMED.into(),
            told: asked,
        })
    );
    assert_eq!(
        answered,
        Answer::ReachChanged {
            retry: vec!["pypi.example.com:443".into()]
        }
    );
    assert_eq!(
        hosts.confines.seen()[0].changes,
        [
            ReachChange::Allow {
                host: "pypi.example.com".into(),
                layer: Layer::Chat,
            },
            ReachChange::WithdrawPort { port: 5432 },
        ]
    );
}

#[tokio::test]
async fn host_a_names_a_composed_confinement_for_its_window_alone() {
    let hosts = hosts();
    let (other, _events) = another_host(&hosts).await;
    let (terminal, _terminal_events) = a_client(&hosts, Scope::Terminal).await;
    let dialed = hosts.dials.load(Ordering::SeqCst);

    for client in [&other, &terminal] {
        let refused = client.start_program(by_its_names()).await.unwrap_err();
        assert_eq!(
            refused.refusal().map(|r| r.code.as_str()),
            Some(session::NOT_ALLOWED)
        );
    }

    assert_eq!(
        hosts.dials.load(Ordering::SeqCst),
        dialed,
        "nothing was sent on"
    );
    assert!(hosts.runs.ran().is_empty());
}

#[tokio::test]
async fn the_window_is_named_on_the_runner_only_the_confinement_of_the_chat_it_started() {
    let hosts = hosts();
    let (window, mut events) = the_window(&hosts).await;
    let (runners, _runners_events) = the_runners_own_window(&hosts).await;
    confine(&runners)
        .await
        .expect("confined by the runner's window");
    runners
        .start_program(a_launch(None, Some(CONFINEMENT)))
        .await
        .expect("the runner's own chat");
    window
        .start_program(by_its_names())
        .await
        .expect("composed on host B");

    let refused = window
        .call(Command::ChangeReach {
            confinement: CONFINEMENT.into(),
            change: ReachChange::WithdrawPort { port: 5432 },
        })
        .await
        .unwrap_err();
    let taken_back = window
        .call(Command::ProgramsOf {
            project: PROJECT.into(),
        })
        .await
        .unwrap_err();
    hosts.confines.proxy_did(
        CONFINEMENT,
        SandboxTold::Refused {
            host: "tracker.example.com".into(),
            port: 443,
        },
    );

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_CHAT)
    );
    assert_eq!(
        taken_back.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_ALLOWED),
        "a relayed link is never named a place's every confinement"
    );
    assert!(
        timeout(Duration::from_millis(300), next_sandbox(&mut events))
            .await
            .ok()
            .flatten()
            .is_none(),
        "the desktop's window hears nothing of the runner's own chat"
    );
}

#[tokio::test]
async fn a_runner_withdraws_the_ports_given_to_its_window_s_chats_once_that_window_s_link_ends() {
    let hosts = hosts();
    let (window, events) = the_window(&hosts).await;
    confine(&window).await.expect("confined");
    window
        .start_program(a_launch(None, Some(CONFINEMENT)))
        .await
        .expect("a chat in the window's confinement");
    window
        .start_program(by_its_names())
        .await
        .expect("a chat composed by its names");

    drop((window, events));

    let withdrawn = |id: &str| {
        hosts
            .confines
            .seen()
            .iter()
            .find(|one| one.confine.confinement == id)
            .is_some_and(|one| one.changes == [ReachChange::WithdrawPort { port: 5432 }])
    };
    until("both chats' given port withdrawn", || {
        withdrawn(CONFINEMENT) && withdrawn(NAMED)
    })
    .await;
    assert!(!hosts.confines.ended(CONFINEMENT), "each keeps its network");
    assert!(!hosts.confines.ended(NAMED));
}

#[tokio::test]
async fn a_chat_whose_given_port_cannot_be_withdrawn_once_its_window_is_gone_loses_its_network() {
    let hosts = hosts();
    let (window, events) = the_window(&hosts).await;
    window
        .start_program(by_its_names())
        .await
        .expect("a chat composed by its names");
    hosts.confines.refuse_changes("the proxy is not answering");

    drop((window, events));

    until("its confinement ended", || hosts.confines.ended(NAMED)).await;
}

#[tokio::test]
async fn a_second_window_relayed_to_the_runner_may_not_read_act_on_or_hear_the_first_s_chat() {
    let hosts = hosts();
    let (window, mut events) = the_window(&hosts).await;
    let (second, mut second_events) = the_window(&hosts).await;
    confine(&window).await.expect("confined");
    window
        .start_program(a_launch(None, Some(CONFINEMENT)))
        .await
        .expect("a chat in the first window's confinement");

    let read = second
        .call(Command::ReachOf {
            confinement: CONFINEMENT.into(),
        })
        .await
        .unwrap_err();
    let withdrawn = second
        .call(Command::ChangeReach {
            confinement: CONFINEMENT.into(),
            change: ReachChange::WithdrawPort { port: 5432 },
        })
        .await
        .unwrap_err();
    hosts.confines.proxy_did(
        CONFINEMENT,
        SandboxTold::Refused {
            host: "tracker.example.com".into(),
            port: 443,
        },
    );

    for refused in [read, withdrawn] {
        assert_eq!(
            refused.refusal().map(|r| r.code.as_str()),
            Some(session::NO_SUCH_CHAT)
        );
    }
    assert!(hosts.confines.seen()[0].changes.is_empty(), "unchanged");
    assert!(
        next_sandbox(&mut events).await.is_some(),
        "the first hears it"
    );
    assert!(
        timeout(Duration::from_millis(300), next_sandbox(&mut second_events))
            .await
            .ok()
            .flatten()
            .is_none(),
        "the second window hears nothing of it"
    );
    assert!(
        window
            .call(Command::ReachOf {
                confinement: CONFINEMENT.into(),
            })
            .await
            .is_ok(),
        "the first reads it through host A"
    );
}

/// Lets the stand-in's held starts go as it is dropped, so a test that fails while they are held
/// ends rather than waits on them.
struct LetsStartsGo(StandInConfines);

impl Drop for LetsStartsGo {
    fn drop(&mut self) {
        self.0.let_starts_go();
    }
}

#[tokio::test]
async fn a_chat_whose_peer_s_link_ends_while_the_runner_composes_it_has_its_given_port_withdrawn() {
    let hosts = hosts();
    // The runner's peer, on a link of its own to host B, whose end the test decides.
    let (peer, _views, events) = Client::new(between_hosts(hosts.runner.clone()).await);
    peer.hold_project(PROJECT, hosts._project.path().to_str().unwrap())
        .await
        .expect("held on host B");
    peer.subscribe(Default::default())
        .await
        .expect("subscribed");
    // A chat already running in a confinement the peer made: its port withdrawn says the runner
    // has heard the peer's link end.
    confine(&peer).await.expect("confined");
    peer.start_program(a_launch(None, Some(CONFINEMENT)))
        .await
        .expect("a chat in the peer's confinement");
    hosts.confines.hold_starts();
    let _lets_go = LetsStartsGo(hosts.confines.clone());
    let starting = tokio::spawn(async move {
        let _ = peer.start_program(by_its_names()).await;
        drop(events);
    });
    until("the composed confinement is being started", || {
        hosts.confines.waiting() == 1
    })
    .await;

    // The peer's link ends while the runner composes its chat.
    starting.abort();
    let _ = starting.await;
    let withdrawn = |id: &str| {
        hosts
            .confines
            .seen()
            .iter()
            .find(|one| one.confine.confinement == id)
            .is_some_and(|one| one.changes == [ReachChange::WithdrawPort { port: 5432 }])
    };
    until("the runner heard the peer's link end", || {
        withdrawn(CONFINEMENT)
    })
    .await;
    hosts.confines.let_starts_go();

    until("the composed chat's given port withdrawn", || {
        withdrawn(NAMED)
    })
    .await;
    assert!(!hosts.confines.ended(NAMED), "it keeps its network");
}

/// Why the runner's binary says a plan the window made on the desktop does not fit (#2445).
const NOT_ITS_OWN: &str = "it gives the chat the service on port 5432 of the runner";

#[tokio::test]
async fn a_plan_from_the_desktop_that_does_not_fit_the_runner_is_refused_and_nothing_starts() {
    let hosts = hosts();
    let (window, _events) = the_window(&hosts).await;
    hosts.confines.refuse_to_fit(NOT_ITS_OWN);

    let refused = confine(&window).await.unwrap_err();

    assert_eq!(refused.code, session::NOT_ALLOWED);
    assert!(refused.why.contains(NOT_ITS_OWN), "{}", refused.why);
    assert!(hosts.confines.seen().is_empty(), "nothing started");
    let started = window
        .start_program(a_launch(None, Some(CONFINEMENT)))
        .await;
    assert!(started.is_err(), "no chat runs in a confinement never made");
    assert!(hosts.runs.ran().is_empty());
}

#[tokio::test]
async fn a_plan_from_the_desktop_that_fits_the_runner_starts_as_it_is() {
    let hosts = hosts();
    let (window, _events) = the_window(&hosts).await;

    confine(&window).await.expect("confined on host B");

    assert_eq!(
        hosts.confines.fitted(),
        [(
            hosts._project.path().canonicalize().unwrap(),
            planned().plan
        )],
        "held to the project as host B holds it"
    );
    assert_eq!(hosts.confines.seen()[0].confine, planned());
    assert_eq!(hosts.confines.seen()[0].planned, Planned::Elsewhere);
}

#[tokio::test]
async fn a_confinement_the_runner_plans_itself_is_not_held_to_a_plan_made_elsewhere() {
    let hosts = hosts();
    let (window, _events) = the_window(&hosts).await;
    let (runners, _runners_events) = the_runners_own_window(&hosts).await;
    hosts.confines.refuse_to_fit(NOT_ITS_OWN);

    window
        .start_program(StartProgram::of(PROJECT, ProgramKind::Harness))
        .await
        .expect("composed on host B by its names, in its own confinement");
    confine(&runners)
        .await
        .expect("the runner's own window plans on this machine");

    assert_eq!(hosts.confines.seen().len(), 2);
    assert_eq!(hosts.confines.seen()[0].planned, Planned::Composed);
    assert_eq!(hosts.confines.seen()[1].planned, Planned::Here);
    assert!(hosts.confines.fitted().is_empty());
}

/// [`by_its_names`], with the services the person allowed the chat alone on the window's machine.
fn by_its_names_with_alone() -> StartProgram {
    StartProgram {
        alone: vec![5432],
        ..by_its_names()
    }
}

#[tokio::test]
async fn the_confinement_the_runner_composes_is_told_what_the_window_allowed_the_chat_alone() {
    let hosts = hosts();
    let (window, _events) = the_window(&hosts).await;

    window
        .start_program(by_its_names_with_alone())
        .await
        .expect("composed on host B by its names");

    let seen = hosts.confines.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].confine.confinement, NAMED);
    assert_eq!(seen[0].planned, Planned::Composed);
    assert_eq!(seen[0].alone, [5432]);
    assert!(seen[0].confine.asks, "its proxy asks the window");
}

#[tokio::test]
async fn what_the_window_allowed_a_chat_alone_is_taken_only_beside_its_confinement() {
    let hosts = hosts();
    let (window, _events) = the_window(&hosts).await;

    for start in [
        StartProgram {
            alone: vec![5432],
            ..StartProgram::of(PROJECT, ProgramKind::Harness)
        },
        StartProgram {
            alone: vec![5432],
            ..a_launch(None, Some(NAMED))
        },
    ] {
        let refused = window.start_program(start).await.unwrap_err();
        assert_eq!(
            refused.refusal().map(|r| r.code.as_str()),
            Some(session::MALFORMED),
            "{refused:?}"
        );
    }
    assert!(hosts.confines.seen().is_empty(), "nothing was composed");
}

#[tokio::test]
async fn host_a_relays_what_a_chat_was_allowed_alone_for_its_window_alone() {
    let hosts = hosts();
    let (other, _events) = another_host(&hosts).await;
    let (terminal, _terminal_events) = a_client(&hosts, Scope::Terminal).await;
    let dialed = hosts.dials.load(Ordering::SeqCst);

    for client in [&other, &terminal] {
        let refused = client
            .start_program(StartProgram {
                alone: vec![5432],
                ..StartProgram::of(PROJECT, ProgramKind::Harness)
            })
            .await
            .unwrap_err();
        assert_eq!(
            refused.refusal().map(|r| r.code.as_str()),
            Some(session::NOT_ALLOWED)
        );
    }
    assert_eq!(
        hosts.dials.load(Ordering::SeqCst),
        dialed,
        "nothing was sent on"
    );
}
