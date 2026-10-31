// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's plan handed to the session host and read back there, and what the host's proxy did
//! reaching the window's hearing (#1999). The proxies themselves listen on the loopback
//! interface, which the behaviour suite covers on CI (`crate::behaviour`).

use std::sync::{Arc, Mutex};

use purlis_core::sandbox::asks::Heard;
use purlis_core::sandbox::egress::{PLAIN_PORTS, Refusals, TUNNEL_PORTS};
use purlis_core::sandbox::hosts::Host;
use purlis_core::sandbox::plan::{Asking, Beside, Hearing, Plan};
use purlis_core::sandbox::reach::{By, Reach};
use purlis_session_protocol::session::SandboxTold;

use super::{Hears, plan_of, wire};

fn a_plan() -> Plan {
    let reach = Reach::of(vec![
        ("api.anthropic.com".to_owned(), By::Open),
        ("grafana.example.com".to_owned(), By::Persona),
        ("127.0.0.1:5432".to_owned(), By::You),
        ("one-off.example.com:8443".to_owned(), By::Chat),
        ("pypi.example.com".to_owned(), By::Project),
    ])
    .never(vec![Host::parse("blocked.example.com").expect("a host")]);
    reach.withdraw(6443);
    Plan {
        reach: reach.written(),
        tunnel_ports: TUNNEL_PORTS.to_vec(),
        plain_ports: PLAIN_PORTS.to_vec(),
        any_port: true,
        beside: Beside::ProxyOnly {
            routes: "/home/me/project/.purlis/app/ssh".into(),
        },
        given: vec![5432],
    }
}

/// The plan the window compiled is the plan the host starts: every layer in order, what policy
/// pins, the ports withdrawn and given, and what runs beside the proxy.
#[test]
fn a_plan_handed_to_the_session_host_reads_back_as_the_window_planned_it() {
    for plan in [
        a_plan(),
        Plan {
            reach: Reach::anything().written(),
            beside: Beside::Wrapped,
            any_port: false,
            given: Vec::new(),
            ..a_plan()
        },
    ] {
        let sent = serde_json::to_string(&wire(&plan)).expect("sent");

        let read = plan_of(&serde_json::from_str(&sent).expect("read")).expect("a plan");

        assert_eq!(read, plan);
    }
}

#[test]
fn a_plan_whose_pinned_host_is_not_a_host_is_refused_in_the_host() {
    let mut wired = wire(&a_plan());
    wired.never.push("not a host".into());

    assert!(plan_of(&wired).is_err());
}

#[derive(Default)]
struct Heard_ {
    refused: Mutex<Vec<String>>,
    local: Mutex<Vec<String>>,
    more: Mutex<Vec<usize>>,
    reached: Mutex<Vec<(Option<String>, &'static str, u64)>>,
    asked: Mutex<Vec<Heard>>,
}

fn hearing(heard: &Arc<Heard_>) -> Hearing {
    Hearing {
        refusals: Refusals::telling({
            let heard = Arc::clone(heard);
            Arc::new(move |host: &str, port: u16| {
                heard.refused.lock().unwrap().push(format!("{host}:{port}"));
            })
        })
        .telling_local({
            let heard = Arc::clone(heard);
            Arc::new(move |host: &str, port: u16| {
                heard.local.lock().unwrap().push(format!("{host}:{port}"));
            })
        }),
        more: Some({
            let heard = Arc::clone(heard);
            Arc::new(move |count| heard.more.lock().unwrap().push(count))
        }),
        reached: Some({
            let heard = Arc::clone(heard);
            Arc::new(move |target: Option<&str>, by: &'static str, times: u64| {
                heard
                    .reached
                    .lock()
                    .unwrap()
                    .push((target.map(str::to_owned), by, times));
            })
        }),
        asks: Some(Asking {
            tell: {
                let heard = Arc::clone(heard);
                Arc::new(move |one| heard.asked.lock().unwrap().push(one))
            },
            askable: Arc::new(|| true),
            known: Arc::new(|_| None),
        }),
    }
}

/// What the host's proxy refused, carried and asks about reaches the window as it would from a
/// proxy of the window's own: the same closures, each told once.
#[test]
fn what_a_proxy_in_the_host_did_reaches_the_windows_hearing() {
    let heard = Arc::new(Heard_::default());
    let hears = Hears::of(hearing(&heard));

    for told in [
        SandboxTold::Refused {
            host: "tracker.example.com".into(),
            port: 443,
        },
        SandboxTold::Refused {
            host: "tracker.example.com".into(),
            port: 443,
        },
        SandboxTold::RefusedLocal {
            host: "169.254.169.254".into(),
            port: 80,
        },
        SandboxTold::RefusedMore { count: 3 },
        SandboxTold::Reached {
            target: Some("api.anthropic.com:443".into()),
            by: "open".into(),
            times: 12,
        },
        SandboxTold::Asked {
            targets: vec!["pypi.example.com:443".into()],
        },
        SandboxTold::TimedOut {
            target: "pypi.example.com:443".into(),
        },
    ] {
        hears.told(told);
    }

    assert_eq!(
        *heard.refused.lock().unwrap(),
        ["tracker.example.com:443"],
        "a host refused twice is one Block"
    );
    assert_eq!(*heard.local.lock().unwrap(), ["169.254.169.254:80"]);
    assert_eq!(*heard.more.lock().unwrap(), [3]);
    assert_eq!(
        *heard.reached.lock().unwrap(),
        [(Some("api.anthropic.com:443".to_owned()), "open", 12)]
    );
    assert_eq!(
        *heard.asked.lock().unwrap(),
        [
            Heard::Asked(vec!["pypi.example.com:443".into()]),
            Heard::TimedOut("pypi.example.com:443".into())
        ]
    );
}

/// The network record keeps a connection only by a decision's own word.
#[test]
fn a_connection_told_by_a_word_no_decision_says_is_not_recorded() {
    let heard = Arc::new(Heard_::default());
    let hears = Hears::of(hearing(&heard));

    hears.told(SandboxTold::Reached {
        target: Some("api.anthropic.com:443".into()),
        by: "everything".into(),
        times: 1,
    });

    assert!(heard.reached.lock().unwrap().is_empty());
}

const PROJECT: &str = "01J9PRJCT00000000000000000";

static HELD: std::sync::LazyLock<purlis_session_protocol::auth::Credentials> =
    std::sync::LazyLock::new(|| {
        purlis_session_protocol::auth::Credentials::mint().expect("credentials")
    });

/// The status line a proxy on `port` answers a tunnel to `target` with, or `None` where nothing
/// listens on `port`.
fn tunnel_through(port: u16, target: &str) -> Option<String> {
    use std::io::{BufRead, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(30)))
        .ok()?;
    stream
        .write_all(format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").as_bytes())
        .ok()?;
    let mut line = String::new();
    std::io::BufReader::new(stream).read_line(&mut line).ok()?;
    Some(line.trim_end().to_owned())
}

/// **The window's link goes and the host stays: a chat's proxy still reaches its hosts**
/// (#1999). The window confines two chats in the host's own code, with its real proxy, and
/// starts a program in one; then its link is dropped, as a crashed window's is. The one nothing
/// ran in ends with the link; the chat's goes on carrying what its sandbox lists. Reattaching a
/// window is #646's: here only the proxy's survival. Binds the loopback interface: CI.
#[test]
fn a_chats_proxy_in_the_host_still_reaches_its_hosts_once_the_windows_link_is_gone() {
    use purlis_session_protocol::auth::Scope;
    use purlis_session_protocol::link;
    use purlis_session_protocol::session::{
        self, Answer, Client, Command, Confine, Launch, ProgramKind, StartProgram,
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let service = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a service");
    let port = service.local_addr().expect("its port").port();
    let project = tempfile::tempdir().expect("a project");
    let hosting = purlis_host::Hosting::confining(
        crate::sessionhost::Composes::as_the_tests_ship(),
        super::ConfinesHere,
        purlis_host::standin::StandIns::default(),
    );
    let plan = super::wire(&Plan {
        reach: Reach::of(vec![(format!("127.0.0.1:{port}"), By::You)]).written(),
        tunnel_ports: TUNNEL_PORTS.to_vec(),
        plain_ports: PLAIN_PORTS.to_vec(),
        any_port: false,
        beside: Beside::Wrapped,
        given: vec![port],
    });
    let (chats, nobodys) = runtime.block_on(async {
        let (a, b) = tokio::io::duplex(1 << 20);
        let (client, served) = tokio::join!(
            link::connect(
                a,
                session::speaks(),
                Scope::LocalUi,
                HELD.of(Scope::LocalUi)
            ),
            link::serve_any(b, session::speaks(), &HELD)
        );
        tokio::spawn(session::serve(
            served.expect("admitted"),
            hosting.clone(),
            None,
        ));
        let (window, _views, _events) = Client::new(client.expect("the window"));
        window
            .hold_project(PROJECT, project.path().to_str().expect("a path"))
            .await
            .expect("held");
        let mut confined = Vec::new();
        for id in ["01J9CNFNMNT0000000000000000", "01J9CNFNMNT0000000000000001"] {
            match window
                .call(Command::Confine(Box::new(Confine {
                    confinement: id.into(),
                    project: PROJECT.into(),
                    plan: plan.clone(),
                    asks: false,
                })))
                .await
            {
                Ok(Answer::Confined(one)) => confined.push(one),
                other => panic!("not confined: {other:?}"),
            }
        }
        window
            .start_program(StartProgram {
                launch: Some(Launch {
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
                    confinement: Some(confined[0].confinement.clone()),
                    prompt_key: None,
                }),
                ..StartProgram::of(PROJECT, ProgramKind::Harness)
            })
            .await
            .expect("the chat starts");
        // The window goes, as a crashed one does: nothing let go of, its link gone.
        drop(window);
        (confined[0].clone(), confined[1].clone())
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while tunnel_through(nobodys.http, &format!("127.0.0.1:{port}")).is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "the confinement nothing ran in outlived the window's link"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        tunnel_through(chats.http, &format!("127.0.0.1:{port}")).as_deref(),
        Some("HTTP/1.1 200 Connection established"),
        "the chat keeps its network"
    );
    assert_eq!(
        tunnel_through(chats.http, "refused.invalid:443").as_deref(),
        Some("HTTP/1.1 403 Forbidden"),
        "and its sandbox"
    );
    hosting.end_all();
    drop(service);
}

/// **A port the person removed is never carried on** (#1999): where the session host cannot be
/// told a port was withdrawn, the window lets go of the chat's whole confinement, so its proxy
/// carries nothing, and says so. The host is its own code over an in-memory link, with stand-ins
/// that listen on nothing.
#[test]
fn a_port_the_host_could_not_withdraw_ends_the_chats_network_and_is_said() {
    use crate::host::SessionHost;
    use crate::hostedsessions::HostedSessions;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let confines = purlis_host::standin::StandInConfines::default();
    let hosting = purlis_host::Hosting::confining(
        crate::sessionhost::Composes::as_the_tests_ship(),
        confines.clone(),
        purlis_host::standin::StandIns::default(),
    );
    let (connection, _window) = crate::hostedsessions::tests::in_memory(hosting, &runtime);
    let project = tempfile::tempdir().expect("a project");
    let root = std::fs::canonicalize(project.path()).expect("a resolved project");
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").expect("a manifest");
    let sessions = HostedSessions::new(connection, &root);
    let confinement = sessions
        .confine(&a_plan(), Hearing::default())
        .expect("confined");
    let id = confinement.held_as().expect("held in the host").to_owned();
    confines.refuse_changes("the host's board is gone");
    // A proxy's end waits on its listeners: the window is told only once it has ended.
    confines.take_to_end(std::time::Duration::from_millis(300));

    let said = confinement.withdraw_port(5432).unwrap_err();

    assert!(said.contains("ended the chat's network"), "{said}");
    assert!(confines.ended(&id), "its proxy carries nothing");
    drop(confinement);
    drop(sessions);
    drop(runtime);
}

/// **A runner starts a plan the desktop relays only where it fits the runner** (#2445): the
/// host's own code with the binary's real check ([`super::ConfinesHere`]), reached over the link
/// between hosts. A service on the runner its sandbox for the project does not give is refused
/// `not_allowed`, and nothing starts; once you give it on the runner, the same plan starts.
/// Binds the loopback interface: CI.
#[test]
fn a_runner_starts_a_relayed_plan_only_where_it_fits_the_runner() {
    use purlis_session_protocol::session::{self, Answer, Client, Command, Confine};

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let service = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a service");
    let port = service.local_addr().expect("its port").port();
    let project = tempfile::tempdir().expect("a project");
    let root = std::fs::canonicalize(project.path()).expect("a resolved project");
    let hosting = purlis_host::Hosting::confining(
        crate::sessionhost::Composes::as_the_tests_ship(),
        super::ConfinesHere,
        purlis_host::standin::StandIns::default(),
    );
    let plan = super::wire(&Plan {
        reach: Reach::of(vec![(format!("127.0.0.1:{port}"), By::You)]).written(),
        tunnel_ports: TUNNEL_PORTS.to_vec(),
        plain_ports: PLAIN_PORTS.to_vec(),
        any_port: false,
        beside: Beside::Wrapped,
        given: vec![port],
    });
    let confine = |id: &str| {
        Command::Confine(Box::new(Confine {
            confinement: id.into(),
            project: PROJECT.into(),
            plan: plan.clone(),
            asks: false,
        }))
    };
    runtime.block_on(async {
        let (peer, _views, _events) =
            Client::new(crate::behaviour::linked_to(hosting.clone()).await);
        peer.hold_project(PROJECT, root.to_str().expect("a path"))
            .await
            .expect("held on the runner");

        let refused = peer
            .call(confine("01J9CNFNMNT0000000000000000"))
            .await
            .expect_err("refused");
        let refusal = refused.refusal().expect("the runner's refusal");
        assert_eq!(refusal.code, session::NOT_ALLOWED);
        assert!(
            refusal
                .why
                .starts_with("the runner does not start the chat's confinement"),
            "{}",
            refusal.why
        );
        assert!(refusal.why.contains(&port.to_string()), "{}", refusal.why);

        purlis_core::sandbox::local::grant_port(&root, port).expect("you gave it on the runner");
        match peer.call(confine("01J9CNFNMNT0000000000000001")).await {
            Ok(Answer::Confined(_)) => {}
            other => panic!("not confined: {other:?}"),
        }
    });
    hosting.end_all();
    drop(service);
}

/// **A service the person allowed for this chat alone, on the window's machine, never keeps a
/// runner chat from starting** (#2510, ruled 2026-10-18): the runner starts the relayed plan
/// without it and says so in its answer; the person's Allow of it reaches the runner, which
/// takes it for that port alone and refuses any other port of its own. Binds the loopback
/// interface: CI.
#[test]
fn a_runner_starts_a_relayed_plan_without_a_service_allowed_for_this_chat_alone() {
    use purlis_session_protocol::session::{Answer, Client, Command, Confine, Layer, ReachChange};

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let service = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a service");
    let port = service.local_addr().expect("its port").port();
    let project = tempfile::tempdir().expect("a project");
    let root = std::fs::canonicalize(project.path()).expect("a resolved project");
    let hosting = purlis_host::Hosting::confining(
        crate::sessionhost::Composes::as_the_tests_ship(),
        super::ConfinesHere,
        purlis_host::standin::StandIns::default(),
    );
    let plan = super::wire(&Plan {
        reach: Reach::of(
            purlis_core::sandbox::localport::reach_entries(port)
                .into_iter()
                .map(|entry| (entry, By::Chat))
                .collect(),
        )
        .written(),
        tunnel_ports: TUNNEL_PORTS.to_vec(),
        plain_ports: PLAIN_PORTS.to_vec(),
        any_port: false,
        beside: Beside::Wrapped,
        given: vec![port],
    });
    runtime.block_on(async {
        let (peer, _views, _events) =
            Client::new(crate::behaviour::linked_to(hosting.clone()).await);
        peer.hold_project(PROJECT, root.to_str().expect("a path"))
            .await
            .expect("held on the runner");

        match peer
            .call(Command::Confine(Box::new(Confine {
                confinement: "01J9CNFNMNT0000000000000000".into(),
                project: PROJECT.into(),
                plan: plan.clone(),
                asks: true,
            })))
            .await
        {
            Ok(Answer::Confined(confined)) => assert_eq!(confined.left_out, [port]),
            other => panic!("not confined: {other:?}"),
        }
        let allow = |host: String| Command::ChangeReach {
            confinement: "01J9CNFNMNT0000000000000000".into(),
            change: ReachChange::Allow {
                host,
                layer: Layer::Chat,
            },
        };
        match peer.call(allow(format!("127.0.0.1:{port}"))).await {
            Ok(Answer::ReachChanged { .. }) => {}
            other => panic!("not allowed: {other:?}"),
        }
        let other = if port == u16::MAX { port - 1 } else { port + 1 };
        let refused = peer
            .call(allow(format!("127.0.0.1:{other}")))
            .await
            .expect_err("refused");
        assert!(
            refused
                .refusal()
                .is_some_and(|refusal| refusal.why.contains(&other.to_string())),
            "{refused:?}"
        );
    });
    hosting.end_all();
    drop(service);
}

/// **A runner's service left out of a relayed plan is reached once the window's board allows
/// it, and no other port of the runner's is** (#2510, ruled 2026-10-18), end to end: the
/// window's own board for the confinement ([`super::HeldInTheHost`]) sends the Allow over the
/// link between hosts, the runner takes it for that port alone and refuses another, and its
/// proxy then carries the chat to the service, which it refused before. Binds the loopback
/// interface: CI.
#[test]
fn the_windows_allow_of_a_service_a_runner_left_out_lets_the_chat_reach_it_there() {
    use std::io::{BufRead, BufReader, Read, Write};

    use purlis_core::sandbox::HeldElsewhere as _;
    use purlis_session_protocol::session::{Answer, Command, Confine};

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let service = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a service");
    let port = service.local_addr().expect("its port").port();
    let project = tempfile::tempdir().expect("a project");
    let root = std::fs::canonicalize(project.path()).expect("a resolved project");
    let hosting = purlis_host::Hosting::confining(
        crate::sessionhost::Composes::as_the_tests_ship(),
        super::ConfinesHere,
        purlis_host::standin::StandIns::default(),
    );
    let link = Arc::new(
        runtime
            .block_on(async {
                crate::hostedsessions::HostLink::over(
                    crate::behaviour::linked_to(hosting.clone()).await,
                    tokio::runtime::Handle::current(),
                    || {},
                )
                .await
            })
            .expect("the link between hosts"),
    );
    link.call(Command::HoldProject {
        project: PROJECT.into(),
        project_path: root.display().to_string(),
    })
    .expect("held on the runner");
    let plan = super::wire(&Plan {
        reach: Reach::of(
            purlis_core::sandbox::localport::reach_entries(port)
                .into_iter()
                .map(|entry| (entry, By::Chat))
                .collect(),
        )
        .written(),
        tunnel_ports: TUNNEL_PORTS.to_vec(),
        plain_ports: PLAIN_PORTS.to_vec(),
        any_port: true,
        beside: Beside::Wrapped,
        given: vec![port],
    });
    let id = "01J9CNFNMNT0000000000000000";
    let confined = match link.call(Command::Confine(Box::new(Confine {
        confinement: id.into(),
        project: PROJECT.into(),
        plan,
        asks: true,
    }))) {
        Ok(Answer::Confined(confined)) => confined,
        other => panic!("not confined: {other:?}"),
    };
    assert_eq!(confined.left_out, [port]);
    let tunnel = || {
        let mut proxy =
            std::net::TcpStream::connect(("127.0.0.1", confined.http)).expect("the proxy");
        proxy
            .set_read_timeout(Some(std::time::Duration::from_secs(15)))
            .expect("a timeout");
        write!(proxy, "CONNECT 127.0.0.1:{port} HTTP/1.1\r\n\r\n").expect("asked");
        let mut line = String::new();
        BufReader::new(proxy.try_clone().expect("a clone"))
            .read_line(&mut line)
            .expect("an answer");
        (line.trim_end().to_owned(), proxy)
    };
    assert_eq!(
        tunnel().0,
        "HTTP/1.1 403 Forbidden",
        "not before it is allowed"
    );

    let held = super::HeldInTheHost::new(id.into(), Arc::clone(&link), true);
    let board = held.board().expect("its board");
    assert!(board.allow_left_out(port).is_ok());
    let other = if port == u16::MAX { port - 1 } else { port + 1 };
    assert!(
        board.allow_left_out(other).is_err(),
        "another port of the runner's"
    );

    let (said, _proxy) = tunnel();
    assert_eq!(said, "HTTP/1.1 200 Connection established");
    let (mut reached, _) = service.accept().expect("the service is reached");
    let mut nothing = [0u8; 1];
    reached
        .set_read_timeout(Some(std::time::Duration::from_millis(50)))
        .expect("a timeout");
    let _ = reached.read(&mut nothing);
    drop(held);
    hosting.end_all();
}
