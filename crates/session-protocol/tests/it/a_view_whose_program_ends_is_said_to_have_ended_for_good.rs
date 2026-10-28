// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A view whose program ends sends what it held, and then the link is told it ended for good
//! (`view_ended`, 1.2), whether or not the client subscribed: a view that fell behind comes
//! back at its next epoch, and only this word tells a client that this one will not. A view the
//! client detached itself is not said to have ended: the client knows.

use std::sync::Arc;
use std::time::Duration;

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{
    self, Answer, Attached, Client, Command, Events, Host, Peer, Pushed, Refusal, Told, ViewEnd,
    ViewKind,
};
use purlis_session_protocol::view::{Chunk, ViewId, Viewer};
use tokio::io::duplex;
use tokio::time::timeout;

use crate::common;
use common::HELD;
use common::programs::{Program, Snapshot};

const PROGRAM: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X4Z";

#[derive(Clone)]
struct OneProgram(Program);

impl Host for OneProgram {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        match command {
            Command::AttachView { .. } => {
                let view = peer
                    .views()
                    .open(Arc::new(self.0.clone()))
                    .await
                    .map_err(|e| Refusal {
                        code: "view".into(),
                        why: e.to_string(),
                    })?;
                Ok(Answer::Attached(Attached {
                    view: view.0,
                    cols: 80,
                    rows: 24,
                    kind: Some(ViewKind::Terminal),
                }))
            }
            Command::DetachView { view, .. } => {
                peer.views().close(ViewId(view));
                Ok(Answer::Done)
            }
            _ => Ok(Answer::Done),
        }
    }
}

async fn linked(program: Program) -> (Client, Acceptor, Events) {
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
    tokio::spawn(session::serve(served.unwrap(), OneProgram(program), None));
    Client::new(client.unwrap())
}

#[tokio::test]
async fn a_view_whose_program_ends_sends_its_last_bytes_and_is_said_to_have_ended() {
    let program = Program::new(Snapshot::Transcript);
    program.write(b"before ");
    let (client, mut streams, mut events) = linked(program.clone()).await;
    let attached = client
        .attach_view(PROGRAM, ViewKind::Terminal)
        .await
        .unwrap();
    let mut reader = Viewer::default()
        .accept(streams.accept().await.unwrap())
        .await
        .unwrap();
    program.write(b"last words");
    program.end();

    let mut read = Vec::new();
    while let Some(chunk) = timeout(Duration::from_secs(5), reader.next())
        .await
        .expect("the view ends")
    {
        match chunk.unwrap() {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => read.extend_from_slice(&bytes),
        }
    }
    assert_eq!(read, b"before last words");

    let told = timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("the link is told")
        .expect("the events are open");
    assert_eq!(
        told,
        Told::Pushed(Pushed::ViewEnded {
            view: attached.view,
            why: ViewEnd::ProgramEnded,
        })
    );
}

#[tokio::test]
async fn a_view_the_client_detached_is_not_said_to_have_ended() {
    let program = Program::new(Snapshot::Transcript);
    let (client, mut streams, mut events) = linked(program.clone()).await;
    let attached = client
        .attach_view(PROGRAM, ViewKind::Terminal)
        .await
        .unwrap();
    let _reader = Viewer::default()
        .accept(streams.accept().await.unwrap())
        .await
        .unwrap();

    client.detach_view(PROGRAM, attached.view).await.unwrap();
    program.end();

    assert!(
        timeout(Duration::from_millis(300), events.recv())
            .await
            .is_err(),
        "nothing is pushed for a view the client closed"
    );
}
