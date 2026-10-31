// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session protocol's commands say "start a hosted program" and "attach a view of kind X",
//! never "start a chat" (spec #1974, ADR 0068 amended for #1994). A host hears only those: the
//! 1.0 words a client one version behind still sends (`start`, `attach`, `detach`) reach it as
//! the hosted-program commands, and are answered in their own words. And a host is told when a
//! link ends, by which time every view on it is closed.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{
    self, Answer, Attached, Client, Command, Events, Host, Peer, ProgramKind, Refusal, Start,
    StartProgram, ViewKind,
};
use purlis_session_protocol::view::{Chunk, Viewer};
use tokio::io::duplex;
use tokio::sync::Notify;
use tokio::time::timeout;

use crate::common;
use common::HELD;
use common::programs::{Program, Snapshot};

const PROJECT: &str = "01J9PRJCT00000000000000000";
const SHELL: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X4Z";

/// A host with one hosted program, a shell, that keeps every command it heard.
#[derive(Clone)]
struct OneProgram {
    program: Program,
    heard: Arc<Mutex<Vec<Command>>>,
    ended: Arc<Mutex<Vec<usize>>>,
    told_ended: Arc<Notify>,
}

impl OneProgram {
    fn new() -> OneProgram {
        OneProgram {
            program: Program::new(Snapshot::Transcript),
            heard: Arc::default(),
            ended: Arc::default(),
            told_ended: Arc::default(),
        }
    }

    fn heard(&self) -> Vec<Command> {
        self.heard.lock().unwrap().clone()
    }
}

impl Host for OneProgram {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        self.heard.lock().unwrap().push(command.clone());
        let failed = |e: link::LinkError| Refusal {
            code: "view".into(),
            why: e.to_string(),
        };
        match command {
            Command::StartProgram(_) => Ok(Answer::ProgramStarted {
                program: SHELL.into(),
                number: None,
                process: None,
                confined: None,
                sandbox: None,
                prompt_key: None,
            }),
            Command::AttachView {
                program,
                kind: ViewKind::Terminal,
                ..
            } if program == SHELL => {
                let view = peer
                    .views()
                    .open(Arc::new(self.program.clone()))
                    .await
                    .map_err(failed)?;
                Ok(Answer::Attached(Attached {
                    view: view.0,
                    cols: 80,
                    rows: 24,
                    kind: Some(ViewKind::Terminal),
                }))
            }
            Command::DetachView { program, view } if program == SHELL => {
                peer.views()
                    .close(purlis_session_protocol::view::ViewId(view));
                Ok(Answer::Done)
            }
            _ => Err(Refusal {
                code: session::NO_SUCH_CHAT.into(),
                why: "no such program".into(),
            }),
        }
    }

    async fn link_ended(&self, peer: &Peer) {
        self.ended.lock().unwrap().push(peer.views().open_views());
        self.told_ended.notify_one();
    }
}

async fn linked(host: OneProgram) -> (Client, Acceptor, Events) {
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
    tokio::spawn(session::serve(served.unwrap(), host, None));
    Client::new(client.unwrap())
}

#[tokio::test]
async fn a_client_starts_a_hosted_program_of_a_kind() {
    let host = OneProgram::new();
    let (client, _views, _events) = linked(host.clone()).await;
    let started = client
        .start_program(StartProgram {
            workspace: None,
            harness: None,
            ..StartProgram::of(PROJECT, ProgramKind::Shell)
        })
        .await
        .unwrap();
    assert_eq!(started, SHELL);
    assert_eq!(
        host.heard(),
        [Command::StartProgram(Box::new(StartProgram {
            workspace: None,
            harness: None,
            ..StartProgram::of(PROJECT, ProgramKind::Shell)
        }))]
    );
}

#[tokio::test]
async fn a_start_from_a_client_a_version_behind_reaches_the_host_as_a_harness_program() {
    let host = OneProgram::new();
    let (client, _views, _events) = linked(host.clone()).await;
    let started = client
        .start(Start {
            project: PROJECT.into(),
            workspace: Some("login".into()),
            harness: Some("claude".into()),
        })
        .await
        .unwrap();
    assert_eq!(
        started, SHELL,
        "answered `started` with the program as its chat"
    );
    assert_eq!(
        host.heard(),
        [Command::StartProgram(Box::new(StartProgram {
            workspace: Some("login".into()),
            harness: Some("claude".into()),
            ..StartProgram::of(PROJECT, ProgramKind::Harness)
        }))]
    );
}

#[tokio::test]
async fn a_view_of_a_kind_opens_with_the_programs_snapshot_then_its_live_bytes() {
    let host = OneProgram::new();
    host.program.write(b"$ ");
    let (client, mut views, _events) = linked(host.clone()).await;
    let attached = client.attach_view(SHELL, ViewKind::Terminal).await.unwrap();
    assert_eq!(attached.kind, Some(ViewKind::Terminal));
    let mut reader = Viewer::default()
        .accept(views.accept().await.unwrap())
        .await
        .unwrap();
    assert_eq!(reader.view().0, attached.view);
    host.program.write(b"ls");

    let mut snapshot = Vec::new();
    while !reader.snapshot_complete() {
        let Chunk::Snapshot(bytes) = reader.next().await.unwrap().unwrap() else {
            panic!("live before the snapshot was complete");
        };
        snapshot.extend_from_slice(&bytes);
    }
    assert_eq!(snapshot, b"$ ");
    assert_eq!(
        reader.next().await.unwrap().unwrap(),
        Chunk::Live(bytes::Bytes::from_static(b"ls"))
    );
}

#[tokio::test]
async fn an_attach_from_a_client_a_version_behind_is_a_terminal_view() {
    let host = OneProgram::new();
    let (client, _views, _events) = linked(host.clone()).await;
    let attached = client.attach(SHELL).await.unwrap();
    assert_eq!(attached.kind, None, "answered in 1.0's words");
    client.detach(SHELL, attached.view).await.unwrap();
    assert_eq!(
        host.heard(),
        [
            Command::AttachView {
                program: SHELL.into(),
                kind: ViewKind::Terminal,
                role: None,
                rate: None,
            },
            Command::DetachView {
                program: SHELL.into(),
                view: attached.view,
            },
        ]
    );
}

async fn until_unwatched(program: &Program) {
    timeout(Duration::from_secs(5), async {
        while program.watched_by() > 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the program is let go of at once, with nothing written");
}

#[tokio::test]
async fn a_detached_view_lets_go_of_its_program() {
    let host = OneProgram::new();
    let (client, _views, _events) = linked(host.clone()).await;
    let attached = client.attach_view(SHELL, ViewKind::Terminal).await.unwrap();
    assert_eq!(host.program.watched_by(), 1);
    client.detach_view(SHELL, attached.view).await.unwrap();
    until_unwatched(&host.program).await;
}

#[tokio::test]
async fn when_a_link_ends_its_views_close_at_once_and_the_host_is_told() {
    let host = OneProgram::new();
    let (client, views, events) = linked(host.clone()).await;
    for _ in 0..3 {
        client.attach_view(SHELL, ViewKind::Terminal).await.unwrap();
    }
    assert_eq!(host.program.watched_by(), 3);

    drop((client, views, events));
    timeout(Duration::from_secs(5), host.told_ended.notified())
        .await
        .expect("the host is told the link ended");
    assert_eq!(
        *host.ended.lock().unwrap(),
        [0],
        "every view was closed before the host was told"
    );
    until_unwatched(&host.program).await;
    host.program.write(b"the program runs on");
}
