// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The behaviour suite: what a chat does, written once and run against every place a chat can
//! run (spec #1974, #1998). It is the gate each later step of moving chats into the session
//! host passes: a behaviour is added here once, and every target runs it.
//!
//! **The targets are the two ways the start-time switch runs a chat** (`crate::hostedsessions`):
//! in the window's own process ([`crate::sessions::Sessions`]), and in a session host the window
//! speaks the session protocol to ([`crate::hostedsessions::HostedSessions`], here over an
//! in-memory link to the host's own code, with real terminals). Each behaviour speaks only what
//! the window speaks to either, [`SessionHost`], and checks only what the window can observe:
//! what its pane draws, what it is told, what ends. When the in-app path is deleted (#2005), its
//! target goes with it, and the host's stays; Docker and a cluster join as targets of their own
//! (spec #2041).
//!
//! **The chat's program is the repo's fake harness** (`crates/fake-harness`), or a shell where a
//! behaviour needs the terminal itself (its size, its line discipline) or runs `purlis hook` as
//! a harness's hooks do. These open terminals and sockets, which a sandboxed agent cannot: they
//! first run on CI.
//!
//! **Each target has a project, and the window's own hooks for it** (#2000): the board and the
//! asks the window draws Needs you and its asks from. In the window's own process the window
//! binds the project's hook socket and issues each chat its token; in the session host, the
//! host does both and hands each line it admits on to the window. A behaviour checks only what
//! the window is told either way.
//!
//! **To add a behaviour:** write `fn the_behaviour(target: &Target)`, and name it in the
//! [`on_every_target!`] list at the bottom.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use purlis_core::engine::Size;
use purlis_core::sandbox::hosts::Host;
use purlis_core::sandbox::plan::{self, Asking, Hearing, Plan};
use purlis_core::sandbox::reach::{By, Reach};
use purlis_core::sandbox::{Confinement, egress};
use purlis_core::session::Exit;
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::{link, session};

use crate::hooks::{Hooks, Moved};
use crate::host::{Opening, SessionHost, Sink};
use crate::hostedsessions::{Connection, HostLink, HostedSessions};
use crate::sessions::{ENDED, Sessions};

const SIZE: Size = Size {
    columns: 80,
    rows: 24,
};
const PATIENCE: Duration = Duration::from_secs(20);

/// One place a chat runs, as the window reaches it.
pub(crate) struct Target {
    pub sessions: Box<dyn SessionHost>,
    /// The window's own hooks for the target's project: its board and its asks.
    pub hooks: Hooks,
    /// Every move of a chat the window was told, in order.
    pub moved: Arc<Mutex<Vec<Moved>>>,
    /// What keeps the target running for as long as the test does.
    _keeps: Vec<Box<dyn std::any::Any>>,
}

/// A project, its manifest written, and where it is.
fn a_project() -> (tempfile::TempDir, PathBuf) {
    let project = tempfile::tempdir().expect("a project");
    let root = std::fs::canonicalize(project.path()).expect("a resolved project");
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").expect("a manifest");
    (project, root)
}

/// The window's hooks for the project at `root`, bound by the window itself or heard from the
/// host, and what they tell the window.
fn hooks_of(root: &Path, relayed: bool) -> (Hooks, Arc<Mutex<Vec<Moved>>>) {
    let plane: crate::planes::PlaneId =
        serde_json::from_str(&serde_json::to_string(root).expect("a path")).expect("a plane");
    let moved: Arc<Mutex<Vec<Moved>>> = Arc::default();
    let tell: crate::planes::Teller = {
        let moved = Arc::clone(&moved);
        Arc::new(move |what| moved.lock().expect("the moves").push(what))
    };
    let at = crate::hooks::socket_for(Some(root));
    let hooks = if relayed {
        Hooks::relayed(plane, &at, tell, Arc::new(|_| {}))
    } else {
        Hooks::listening_on(plane, &at, tell, Arc::new(|_| {})).expect("the window listens")
    };
    (hooks, moved)
}

/// A chat in the window's own process: the switch off, the default.
pub(crate) fn in_the_window() -> Target {
    let (project, root) = a_project();
    let (hooks, moved) = hooks_of(&root, false);
    Target {
        sessions: Box::new(Sessions::reporting_to(hooks.reporting())),
        hooks,
        moved,
        _keeps: vec![Box::new(project)],
    }
}

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().expect("credentials"));

/// A chat in the session host: the switch on. The host's own code, serving the session protocol
/// over an in-memory link, its programs each in a real terminal, composed as the window's binary
/// composes them.
pub(crate) fn in_the_host() -> Target {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let hosting = purlis_host::Hosting::with(
        purlis_host::FromTheWindow(crate::sessionhost::launched),
        purlis_host::Terminals,
        crate::sessionhost::HookChannels,
        crate::hostedconfinement::ConfinesHere,
    );
    let link = runtime.block_on(async {
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
            served.expect("the host admits the window"),
            hosting,
            None,
        ));
        HostLink::over(
            client.expect("the window is admitted"),
            tokio::runtime::Handle::current(),
            || {},
        )
        .await
        .expect("the window speaks to the host")
    });
    let connection = Arc::new(Connection::default());
    connection.ready(Arc::new(link));
    let (project, root) = a_project();
    let (hooks, moved) = hooks_of(&root, true);
    let sessions = HostedSessions::new(connection, &root);
    sessions.hear_through(hooks.relayed_from_the_host().expect("heard from the host"));
    Target {
        sessions: Box::new(sessions),
        hooks,
        moved,
        _keeps: vec![Box::new(project), Box::new(runtime)],
    }
}

/// The `purlis` binary, whose `purlis hook` a chat's harness runs, built once beside this
/// test's binary when it is not there yet.
fn purlis() -> String {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| built_beside("purlis", "purlis-cli"))
        .display()
        .to_string()
}

/// The binary `name` of package `package`, built beside this test's binary when it is not
/// there yet.
fn built_beside(name: &str, package: &str) -> PathBuf {
    let deps = std::env::current_exe().expect("this test's path");
    let beside = deps
        .parent()
        .and_then(Path::parent)
        .expect("the target folder")
        .join(name);
    if !beside.is_file() {
        let status = purlis_core::forklock::status(
            std::process::Command::new(env!("CARGO"))
                .args(["build", "--quiet", "-p", package, "--bin", name]),
        )
        .expect("cargo runs");
        assert!(status.success(), "{name} did not build: {status}");
    }
    beside
}

/// The repo's fake harness, built once beside this test's binary when it is not there yet.
fn fake_harness() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| built_beside("fake-harness", "fake-harness"))
        .clone()
}

fn a_harness(args: &[&str]) -> Opening {
    let mut opening = a_program(&fake_harness().display().to_string(), args);
    // A harness the plane does not know: the fake one runs as a program, as a chat on a
    // profile's own command does.
    opening.harness = None;
    opening
}

fn a_shell(script: &str) -> Opening {
    a_program("/bin/sh", &["-c", script])
}

fn a_program(program: &str, args: &[&str]) -> Opening {
    Opening {
        program: Some(program.to_owned()),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        cwd: None,
        size: SIZE,
        env: Vec::new(),
        env_strip: Vec::new(),
        harness: None,
        declared_env: Vec::new(),
        env_pass: Vec::new(),
        operator_shell: false,
        git_hooks: None,
        handed: Default::default(),
        confinement: None,
    }
}

/// A pane: what its view is sent, as it is sent.
struct Pane {
    seen: Arc<Mutex<String>>,
}

impl Pane {
    fn on(target: &Target, id: u32) -> Pane {
        let seen = Arc::new(Mutex::new(String::new()));
        let sink: Sink = {
            let seen = Arc::clone(&seen);
            Box::new(move |text: String| seen.lock().expect("the pane").push_str(&text))
        };
        target.sessions.watch(id, sink).expect("a view opens");
        Pane { seen }
    }

    fn shows(&self, text: &str) {
        let deadline = Instant::now() + PATIENCE;
        while !self.seen.lock().expect("the pane").contains(text) {
            assert!(
                Instant::now() < deadline,
                "{text:?} was never drawn, only {:?}",
                self.seen.lock().expect("the pane")
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// How each chat ended, as the window is told.
fn told_of_ends(target: &Target) -> Arc<Mutex<Vec<(u32, Exit)>>> {
    let ends = Arc::new(Mutex::new(Vec::new()));
    target.sessions.when_one_ends({
        let ends = Arc::clone(&ends);
        Arc::new(move |id, exit| ends.lock().expect("the ends").push((id, exit)))
    });
    ends
}

fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(10));
    }
}

// The behaviours.

fn a_chat_starts_and_its_tab_draws_what_it_wrote(target: &Target) {
    let id = target
        .sessions
        .open(
            None,
            &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
            &|_| {},
        )
        .expect("it starts");

    Pane::on(target, id).shows("HARNESS-READY");
    target.sessions.close(id).expect("it closes");
}

fn what_its_tab_types_reaches_it(target: &Target) {
    let id = target
        .sessions
        .open(
            None,
            &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
            &|_| {},
        )
        .expect("it starts");
    let pane = Pane::on(target, id);
    pane.shows("HARNESS-READY");

    target.sessions.input(id, b"hello there\r").expect("typed");

    pane.shows("you said: hello there");
    target.sessions.close(id).expect("it closes");
}

fn a_resize_reaches_its_terminal(target: &Target) {
    let id = target
        .sessions
        .open(
            None,
            &a_shell("printf 'size %s.' \"$(stty size)\"; read line; printf 'size %s.' \"$(stty size)\"; read line"),
            &|_| {},
        )
        .expect("it starts");
    let pane = Pane::on(target, id);
    pane.shows("size 24 80.");

    target
        .sessions
        .resize(
            id,
            Size {
                columns: 120,
                rows: 40,
            },
        )
        .expect("resized");
    target.sessions.input(id, b"\r").expect("typed");

    pane.shows("size 40 120.");
    target.sessions.close(id).expect("it closes");
}

fn a_chat_is_told_its_own_number(target: &Target) {
    let id = target
        .sessions
        .open(
            Some(7),
            &a_shell("printf 'chat %s.' \"$PURLIS_SESSION_ID\"; read line"),
            &|_| {},
        )
        .expect("it starts");

    assert_eq!(id, 7);
    Pane::on(target, id).shows("chat 7.");
    target.sessions.close(id).expect("it closes");
}

fn a_program_that_ends_is_told_with_its_code_and_its_tab_stays(target: &Target) {
    let ends = told_of_ends(target);
    let id = target
        .sessions
        .open(
            None,
            &a_harness(&[
                "--sentinel",
                "HARNESS-READY",
                "--interactive",
                "--exit-code",
                "7",
            ]),
            &|_| {},
        )
        .expect("it starts");
    let pane = Pane::on(target, id);
    pane.shows("HARNESS-READY");

    target.sessions.input(id, b"/quit\r").expect("typed");

    pane.shows(ENDED);
    until("the window is told", || {
        !ends.lock().expect("ends").is_empty()
    });
    assert_eq!(*ends.lock().expect("ends"), [(id, Exit::Code(7))]);
    assert!(target.sessions.running().contains(&id), "the tab stays");
    assert_eq!(target.sessions.process_id(id), None);
    target.sessions.close(id).expect("it closes");
}

fn stopping_every_program_ends_each_and_keeps_their_tabs(target: &Target) {
    let ends = told_of_ends(target);
    let ids: Vec<u32> = (0..2)
        .map(|_| {
            target
                .sessions
                .open(
                    None,
                    &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
                    &|_| {},
                )
                .expect("it starts")
        })
        .collect();
    for id in &ids {
        Pane::on(target, *id).shows("HARNESS-READY");
    }

    assert_eq!(target.sessions.stop_every_program(), 2);

    until("both are told", || ends.lock().expect("ends").len() == 2);
    for id in &ids {
        assert_eq!(target.sessions.process_id(*id), None);
        assert!(target.sessions.running().contains(id));
        target.sessions.close(*id).expect("it closes");
    }
}

fn a_closed_chat_is_ended_and_gone(target: &Target) {
    let id = target
        .sessions
        .open(
            None,
            &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
            &|_| {},
        )
        .expect("it starts");
    Pane::on(target, id).shows("HARNESS-READY");
    let pid = target.sessions.process_id(id).expect("it runs");

    target.sessions.close(id).expect("it closes");

    assert!(!target.sessions.running().contains(&id));
    assert!(target.sessions.input(id, b"x").is_err());
    until("its program is gone", || !purlis_core::process::alive(pid));
}

fn a_program_reading_a_line_is_not_yet_ready_to_be_typed_at_key_by_key(target: &Target) {
    let id = target
        .sessions
        .open(None, &a_shell("printf 'reading.'; read line"), &|_| {})
        .expect("it starts");
    Pane::on(target, id).shows("reading.");

    let readiness = target.sessions.readiness(id).expect("it answers");

    if let Some(edits_lines) = readiness.edits_lines {
        assert!(
            edits_lines,
            "a shell's `read` leaves the line discipline on"
        );
    }
    target.sessions.close(id).expect("it closes");
}

fn a_chat_starts_with_the_path_and_variables_the_window_gives_it(target: &Target) {
    // What the window's own composition gives a chat started with nothing of its own: every
    // target must give the same, whichever process starts it.
    let expected = purlis_core::chatspec::spec(
        std::env::vars_os(),
        &purlis_core::chatspec::Terminal {
            program: "/bin/sh",
            args: &[],
            cwd: None,
            size: SIZE,
            starting: purlis_core::chatenv::Starting {
                harness: None,
                operator: &[],
                strip: &[],
                set: &[],
                git_hooks: None,
                declared: &[],
            },
            number: 1,
            reporting: None,
        },
    );
    let value = |name: &str| {
        expected
            .env
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let (path, home) = (value("PATH"), value("HOME"));
    assert!(!path.is_empty(), "a chat is given a PATH");
    let id = target
        .sessions
        .open(
            None,
            &a_shell("read line; printf 'P[%s]H[%s].' \"$PATH\" \"$HOME\"; read line"),
            &|_| {},
        )
        .expect("it starts");
    let pane = Pane::on(target, id);

    target.sessions.input(id, b"\r").expect("typed");

    pane.shows(&format!("P[{path}]H[{home}]."));
    target.sessions.close(id).expect("it closes");
}

/// A harness whose hooks report and ask through `purlis hook`: the word a chat's environment
/// names it with (`PURLIS_HARNESS`), a permission ask's payload as its hook is handed it, the
/// option the window answers it with, and what its hook then prints for the harness.
struct Hooked {
    harness: &'static str,
    payload: &'static str,
    option: &'static str,
    prints: &'static str,
}

const HOOKED_HARNESSES: [Hooked; 3] = [
    Hooked {
        harness: "claude-code",
        payload: r#"{"tool_name":"Bash","tool_input":{"command":"npm test"}}"#,
        option: "allow",
        prints: r#""behavior":"allow""#,
    },
    Hooked {
        harness: "codex",
        payload: r#"{"tool_name":"Bash","tool_input":{"command":"npm test"}}"#,
        option: "allow",
        prints: r#""behavior":"allow""#,
    },
    // opencode's shell line is answered only with a refusal from the window (#1691).
    Hooked {
        harness: "opencode",
        payload: r#"{"permission":"bash","patterns":["npm test"]}"#,
        option: "reject",
        prints: r#"{"reply":"reject"}"#,
    },
];

/// Started as the window starts a chat: its number on the board before its program runs, so
/// its first hook lands.
fn a_chat_on_the_board(target: &Target, opening: &Opening) -> u32 {
    target
        .sessions
        .open(None, opening, &|id| {
            target.hooks.board().opened(id, None, None);
        })
        .expect("it starts")
}

fn a_chat_whose_turn_ends_needs_you_on_every_harness(target: &Target) {
    for Hooked { harness, .. } in HOOKED_HARNESSES {
        let id = a_chat_on_the_board(
            target,
            &a_shell(&format!(
                "PURLIS_HARNESS={harness} '{}' hook stop </dev/null; printf 'reported.'; read line",
                purlis()
            )),
        );

        Pane::on(target, id).shows("reported.");

        until(
            &format!("{harness}: the window is told the chat needs you"),
            || {
                target.moved.lock().expect("the moves").iter().any(|moved| {
                    moved.session == id && moved.needs_you && moved.queue.contains(&id)
                })
            },
        );
        target.sessions.close(id).expect("it closes");
    }
}

fn a_permission_ask_is_answered_from_the_window_on_every_harness(target: &Target) {
    for Hooked {
        harness,
        payload,
        option,
        prints,
    } in HOOKED_HARNESSES
    {
        let id = a_chat_on_the_board(
            target,
            &a_shell(&format!(
                "printf '%s' '{payload}' | PURLIS_HARNESS={harness} '{}' hook permissionrequest; \
                 printf 'decided.'; read line",
                purlis()
            )),
        );
        let pane = Pane::on(target, id);
        let ask = {
            let deadline = Instant::now() + PATIENCE;
            loop {
                let pending = target.hooks.asks().pending(Instant::now());
                if let Some(ask) = pending.into_iter().find(|ask| ask.chat == id.to_string()) {
                    break ask;
                }
                assert!(
                    Instant::now() < deadline,
                    "{harness}: the window never held the ask"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        target
            .hooks
            .answer(id, &ask.id.to_string(), option)
            .expect("answered in the window");

        pane.shows(prints);
        pane.shows("decided.");
        target.sessions.close(id).expect("it closes");
    }
}

/// Where [`a_listener_beside_the_chat`] binds, when this test binary is run as that helper.
const LISTEN_AT: &str = "PURLIS_BEHAVIOUR_LISTEN_AT";

/// Not a behaviour: this binary, run again as a child of the test, takes the hook socket's
/// path over with a listener of its own, which is beside every chat and below none, says so,
/// and waits to be killed.
#[test]
#[ignore = "a helper the behaviours start as a child"]
fn a_listener_beside_the_chat() {
    let Some(path) = std::env::var_os(LISTEN_AT) else {
        return;
    };
    let _ = std::fs::remove_file(&path);
    let _listener = std::os::unix::net::UnixListener::bind(&path).expect("bound");
    println!("up");
    std::thread::sleep(Duration::from_secs(60));
}

/// A listener at `socket` in a process of its own, beside the chats: ended as it is dropped.
struct Beside(std::process::Child);

impl Beside {
    fn listening_at(socket: &Path) -> Beside {
        let mut child = purlis_core::forklock::spawn(
            std::process::Command::new(std::env::current_exe().expect("this test"))
                .args([
                    "--exact",
                    "behaviour::a_listener_beside_the_chat",
                    "--ignored",
                    "--nocapture",
                ])
                .env(LISTEN_AT, socket)
                .stdout(std::process::Stdio::piped()),
        )
        .expect("the listener starts");
        let mut said = std::io::BufReader::new(child.stdout.take().expect("piped"));
        let mut line = String::new();
        while !line.contains("up") {
            line.clear();
            if std::io::BufRead::read_line(&mut said, &mut line).expect("it speaks") == 0 {
                break;
            }
        }
        Beside(child)
    }
}

impl Drop for Beside {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn a_hook_refuses_a_listener_that_is_not_its_chats_ancestor(target: &Target) {
    let payload = HOOKED_HARNESSES[0].payload;
    let id = a_chat_on_the_board(
        target,
        &a_shell(&format!(
            "read line; printf '%s' '{payload}' | '{}' hook permissionrequest; \
             printf 'left to its own prompt.'; read line",
            purlis()
        )),
    );
    let pane = Pane::on(target, id);
    let socket = target
        .hooks
        .socket()
        .expect("the project's hook socket")
        .to_path_buf();
    let _beside = Beside::listening_at(&socket);

    target.sessions.input(id, b"\r").expect("typed");

    pane.shows("is not the host that started this chat");
    pane.shows("left to its own prompt.");
    assert!(
        target.hooks.asks().pending(Instant::now()).is_empty(),
        "an ask reached the window through a listener that is not the chat's ancestor"
    );
    target.sessions.close(id).expect("it closes");
}

// A sandboxed chat's network (#1999): its confinement, started where its chat runs.

/// A chat's plan listing `listed`, as the window compiles one for a harness it wraps, giving it
/// the services on this machine at `given`.
fn a_plan(listed: Vec<(String, By)>, given: Vec<u16>) -> Plan {
    Plan {
        reach: Reach::of(listed).written(),
        tunnel_ports: egress::TUNNEL_PORTS.to_vec(),
        plain_ports: egress::PLAIN_PORTS.to_vec(),
        any_port: false,
        beside: plan::Beside::Wrapped,
        given,
    }
}

/// A service on this machine, which answers nothing: what a chat may be given by its port.
fn a_service() -> (std::net::TcpListener, u16) {
    let service = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a service");
    let port = service.local_addr().expect("its port").port();
    (service, port)
}

/// The status line a chat's proxy, on its HTTP `port`, answers a tunnel to `target` with.
fn tunnel_through(port: u16, target: &str) -> String {
    use std::io::{BufRead, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("the proxy");
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("a timeout");
    stream
        .write_all(format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").as_bytes())
        .expect("sent");
    let mut line = String::new();
    std::io::BufReader::new(stream)
        .read_line(&mut line)
        .expect("a status line");
    line.trim_end().to_owned()
}

/// What a chat's proxy told the window, by kind.
#[derive(Default)]
struct HeardOfIt {
    refused: Mutex<Vec<String>>,
    asked: Mutex<Vec<String>>,
}

fn hearing(heard: &Arc<HeardOfIt>, asks: bool) -> Hearing {
    Hearing {
        refusals: egress::Refusals::telling({
            let heard = Arc::clone(heard);
            Arc::new(move |host: &str, port: u16| {
                heard
                    .refused
                    .lock()
                    .expect("heard")
                    .push(egress::host_and_port(host, port));
            })
        }),
        asks: asks.then(|| Asking {
            tell: {
                let heard = Arc::clone(heard);
                Arc::new(move |told| {
                    if let purlis_core::sandbox::asks::Heard::Asked(targets) = told {
                        heard.asked.lock().expect("heard").extend(targets);
                    }
                })
            },
            askable: Arc::new(|| true),
            known: Arc::new(|_| None),
        }),
        ..Hearing::default()
    }
}

/// A chat that runs in `confinement`, as the window starts one.
fn in_it(target: &Target, confinement: &Confinement) -> u32 {
    let mut opening = a_shell("printf 'confined.'; read line");
    opening
        .env
        .push(("HTTPS_PROXY".into(), confinement.proxy_url()));
    opening.confinement = confinement.held_as().map(str::to_owned);
    let id = target
        .sessions
        .open(None, &opening, &|_| {})
        .expect("it starts");
    Pane::on(target, id).shows("confined.");
    id
}

fn a_sandboxed_chats_proxy_carries_what_it_lists_and_refuses_the_rest(target: &Target) {
    let (_service, port) = a_service();
    let heard = Arc::new(HeardOfIt::default());
    let confinement = target
        .sessions
        .confine(
            &a_plan(vec![(format!("127.0.0.1:{port}"), By::You)], vec![port]),
            hearing(&heard, false),
        )
        .expect("confined");
    let id = in_it(target, &confinement);

    assert_eq!(
        tunnel_through(confinement.proxy_port(), &format!("127.0.0.1:{port}")),
        "HTTP/1.1 200 Connection established"
    );
    assert_eq!(
        tunnel_through(confinement.proxy_port(), "refused.invalid:443"),
        "HTTP/1.1 403 Forbidden"
    );
    until("the window hears of the refusal", || {
        *heard.refused.lock().expect("heard") == ["refused.invalid:443"]
    });
    target.sessions.close(id).expect("it closes");
}

fn a_port_withdrawn_while_a_chat_runs_is_carried_no_longer(target: &Target) {
    let (_service, port) = a_service();
    let confinement = target
        .sessions
        .confine(
            &a_plan(vec![(format!("127.0.0.1:{port}"), By::You)], vec![port]),
            Hearing::default(),
        )
        .expect("confined");
    let id = in_it(target, &confinement);

    confinement.withdraw_port(port).expect("withdrawn");

    assert_eq!(
        tunnel_through(confinement.proxy_port(), &format!("127.0.0.1:{port}")),
        "HTTP/1.1 403 Forbidden"
    );
    target.sessions.close(id).expect("it closes");
}

fn an_ask_raised_by_a_chats_proxy_is_answered_from_the_window(target: &Target) {
    let heard = Arc::new(HeardOfIt::default());
    let confinement = target
        .sessions
        .confine(&a_plan(Vec::new(), Vec::new()), hearing(&heard, true))
        .expect("confined");
    let id = in_it(target, &confinement);
    let port = confinement.proxy_port();
    let held = std::thread::spawn(move || tunnel_through(port, "held.invalid:443"));
    until("the window is asked", || {
        *heard.asked.lock().expect("heard") == ["held.invalid:443"]
    });
    let board = confinement.asks().expect("its proxy asks live");
    assert!(board.asks_about(&Host::parse("held.invalid").expect("a host")));

    let retry = board.allow(&Host::parse("held.invalid").expect("a host"), By::Chat);

    // Carried: the proxy tried the host, which never resolves.
    assert_eq!(held.join().expect("answered"), "HTTP/1.1 502 Bad Gateway");
    assert!(retry.is_empty(), "nothing had given up");
    assert_eq!(
        board.allowed_live(),
        [("held.invalid".to_owned(), By::Chat)]
    );
    target.sessions.close(id).expect("it closes");
}

/// Each behaviour, as a test of its own on every target.
macro_rules! on_every_target {
    ($($behaviour:ident),* $(,)?) => {
        $(
            mod $behaviour {
                #[test]
                fn in_the_window() {
                    super::$behaviour(&super::in_the_window());
                }

                #[test]
                fn in_the_host() {
                    super::$behaviour(&super::in_the_host());
                }
            }
        )*
    };
}

on_every_target!(
    a_chat_starts_and_its_tab_draws_what_it_wrote,
    what_its_tab_types_reaches_it,
    a_resize_reaches_its_terminal,
    a_chat_is_told_its_own_number,
    a_program_that_ends_is_told_with_its_code_and_its_tab_stays,
    stopping_every_program_ends_each_and_keeps_their_tabs,
    a_closed_chat_is_ended_and_gone,
    a_program_reading_a_line_is_not_yet_ready_to_be_typed_at_key_by_key,
    a_chat_starts_with_the_path_and_variables_the_window_gives_it,
    a_chat_whose_turn_ends_needs_you_on_every_harness,
    a_permission_ask_is_answered_from_the_window_on_every_harness,
    a_hook_refuses_a_listener_that_is_not_its_chats_ancestor,
    a_sandboxed_chats_proxy_carries_what_it_lists_and_refuses_the_rest,
    a_port_withdrawn_while_a_chat_runs_is_carried_no_longer,
    an_ask_raised_by_a_chats_proxy_is_answered_from_the_window,
);
