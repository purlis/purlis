// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A hosted program outlives the client that watches it (spec #1974; the reattach prototype of
//! 2026-10-14, made a test). Over a real socket, with the uid check and the credentials a host
//! holds: a client in another process attaches a view and is killed outright; the host closes
//! that link's views at once, with nothing written; the program writes on with nobody
//! attached; a new client reattaches, its snapshot shows everything written meanwhile, and then
//! live output continues.
//!
//! The client is `charter-session-peer --watch`, a separate process, so its death is a real
//! one: no goodbye on the link, only the socket closing. The program is the tests' stand-in, a
//! transcript whose snapshot is everything it wrote, as a terminal's screen and scrollback are;
//! a real terminal behind the same seam is the session host's (#1998).

// Unix sockets. Windows is not ported yet (ADR 0068, *Later decisions*).
#![cfg(unix)]

use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link;
use purlis_session_protocol::local::{Listener, NoChats};
use purlis_session_protocol::session::{self, Answer, Attached, Command, Host, Peer, Refusal};
use purlis_session_protocol::view::ViewId;
use tokio::io::{AsyncBufReadExt, BufReader, Lines};
use tokio::net::UnixListener;
use tokio::process::{Child, ChildStdout, Command as Process};
use tokio::time::timeout;

use crate::common;
use common::HELD;
use common::programs::{Program, Snapshot};

const PEER: &str = env!("CARGO_BIN_EXE_charter-session-peer");
const PROGRAM: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X4Z";
const WITHIN: Duration = Duration::from_secs(10);

/// A host holding one hosted program, which counts the links that ended.
#[derive(Clone)]
struct Ticking {
    program: Program,
    links_ended: Arc<Mutex<usize>>,
}

impl Host for Ticking {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        match command {
            Command::AttachView { program, kind, .. } if program == PROGRAM => {
                let view = peer
                    .views()
                    .open(Arc::new(self.program.clone()))
                    .await
                    .map_err(|e| Refusal {
                        code: "view".into(),
                        why: e.to_string(),
                    })?;
                Ok(Answer::Attached(Attached {
                    view: view.0,
                    cols: 80,
                    rows: 24,
                    kind: Some(kind),
                }))
            }
            Command::DetachView { view, .. } => {
                peer.views().close(ViewId(view));
                Ok(Answer::Done)
            }
            _ => Err(Refusal {
                code: session::NO_SUCH_CHAT.into(),
                why: "no such program".into(),
            }),
        }
    }

    async fn link_ended(&self, _peer: &Peer) {
        *self.links_ended.lock().unwrap() += 1;
    }
}

/// Serve every connection to `listener` as the session host does: the uid check, the
/// handshake and admission, then the session protocol.
fn serving(listener: Listener, host: Ticking) {
    tokio::spawn(async move {
        let chats = Arc::new(NoChats);
        loop {
            let Ok(Ok(io)) = listener.accept().await else {
                continue;
            };
            let (host, chats) = (host.clone(), Arc::clone(&chats));
            tokio::spawn(async move {
                if let Ok(link) = link::serve(io, session::speaks(), &HELD, &chats).await {
                    let _ = session::serve(link, host, None).await;
                }
            });
        }
    });
}

/// A watching client in another process, and the lines it prints.
struct Watching {
    child: Child,
    lines: Lines<BufReader<ChildStdout>>,
}

fn watch(socket: &std::path::Path) -> Watching {
    let mut child = Process::new(PEER)
        .arg("--watch")
        .arg(socket)
        .arg(PROGRAM)
        .env(
            "CHARTER_SESSION_PEER_CREDENTIAL",
            HELD.of(Scope::Terminal).expose(),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let lines = BufReader::new(child.stdout.take().unwrap()).lines();
    Watching { child, lines }
}

impl Watching {
    /// The next line it prints, within [`WITHIN`].
    async fn line(&mut self) -> String {
        timeout(WITHIN, self.lines.next_line())
            .await
            .expect("the client prints in time")
            .unwrap()
            .expect("the client is still printing")
    }

    /// Its snapshot, once complete: the first line it prints.
    async fn snapshot(&mut self) -> String {
        let line = self.line().await;
        line.strip_prefix("snapshot ")
            .unwrap_or_else(|| panic!("the snapshot comes first, whole: {line}"))
            .to_owned()
    }

    /// Live bytes, until `last` has been among them.
    async fn live_until(&mut self, last: &str) -> String {
        let mut live = String::new();
        while !live.contains(last) {
            let line = self.line().await;
            let bytes = line
                .strip_prefix("live ")
                .unwrap_or_else(|| panic!("only live bytes after the snapshot: {line}"));
            live.push_str(bytes);
        }
        live
    }
}

fn tick(program: &Program, n: u32) {
    program.write(format!("tick {n}\r\n").as_bytes());
}

async fn until(what: &str, mut done: impl FnMut() -> bool) {
    timeout(WITHIN, async {
        while !done() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what}"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_killed_outright_loses_its_views_and_the_next_one_reattaches_with_nothing_lost() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("purlisd.sock");
    let listener = Listener::new(UnixListener::bind(&socket).unwrap());
    let host = Ticking {
        program: Program::new(Snapshot::Transcript),
        links_ended: Arc::default(),
    };
    serving(listener, host.clone());
    let program = &host.program;

    // Client 1 attaches, sees ticks 1 to 3, and is killed outright.
    tick(program, 1);
    let mut first = watch(&socket);
    assert_eq!(first.snapshot().await, "tick 1\\r\\n");
    tick(program, 2);
    tick(program, 3);
    assert_eq!(first.live_until("tick 3").await, "tick 2\\r\\ntick 3\\r\\n");
    assert_eq!(program.watched_by(), 1);
    first.child.kill().await.unwrap();

    // The host notices the dead link at once and closes its view: nothing is written meanwhile,
    // so no view's queue filling up is what tells it.
    until("the dead client's link is noticed", || {
        *host.links_ended.lock().unwrap() == 1
    })
    .await;
    until("the dead client's view is closed", || {
        program.watched_by() == 0
    })
    .await;

    // The program runs on with nobody attached.
    for n in 4..=6 {
        tick(program, n);
    }

    // Client 2 reattaches: its snapshot holds everything, then live output continues.
    let mut second = watch(&socket);
    assert_eq!(
        second.snapshot().await,
        (1..=6)
            .map(|n| format!("tick {n}\\r\\n"))
            .collect::<String>(),
        "the snapshot shows everything written while nobody was attached"
    );
    tick(program, 7);
    assert_eq!(second.live_until("tick 7").await, "tick 7\\r\\n");
    second.child.kill().await.unwrap();
    until("the second client's view is closed too", || {
        program.watched_by() == 0
    })
    .await;
}
