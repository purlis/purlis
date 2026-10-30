// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Two session hosts link (#2042, ADR 0078 §3): host A dials host B through a connector, each
//! end's link key is checked against the one pinned at pairing, and the session protocol runs
//! inside, unchanged and sealed. Host B serves the link as `remote-link`, so what crosses is
//! what that scope's row grants (FD-27): the window's start and its replies to hook lines, as
//! its host relays them (ruled 2026-10-15), and never a start without the sandbox.

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::hostlink::{self, Handshake, Liveness};
use std::sync::{Arc, Mutex};

use purlis_session_protocol::session::{self, Client, Command, Launch, ProgramKind, StartProgram};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

use crate::common;
use common::hosts::{Board, CHAT, HOST_A, a_as_pinned, b_as_pinned, host_a, serve_b};

#[tokio::test]
async fn host_a_lists_host_bs_chats_through_the_link() {
    let board = Board::default();
    let (b, mut told) = serve_b(board.clone(), a_as_pinned(), Liveness::default());
    let (a_end, b_end) = duplex(1 << 16);
    b.send(b_end).unwrap();

    let link = hostlink::connect(
        a_end,
        Handshake::THIS_BUILD,
        &host_a(),
        &b_as_pinned(),
        Liveness::default(),
    )
    .await
    .expect("host B admits host A");
    let (client, _views, _events) = Client::new(link);

    let chats = client.list().await.expect("host B lists its chats");
    assert_eq!(chats[0].chat, CHAT);
    assert_eq!(client.scope(), Scope::RemoteLink);
    assert_eq!(told.recv().await.unwrap(), Ok(HOST_A.to_owned()));
    assert_eq!(*board.heard.lock().unwrap(), ["list"]);
}

async fn linked(board: &Board) -> Client {
    let (b, _told) = serve_b(board.clone(), a_as_pinned(), Liveness::default());
    let (a_end, b_end) = duplex(1 << 16);
    b.send(b_end).unwrap();
    let link = hostlink::connect(
        a_end,
        Handshake::THIS_BUILD,
        &host_a(),
        &b_as_pinned(),
        Liveness::default(),
    )
    .await
    .unwrap();
    Client::new(link).0
}

#[tokio::test]
async fn the_windows_start_its_host_relays_crosses_the_link() {
    let board = Board::default();
    let client = linked(&board).await;

    client
        .hold_project("01J9PRJCT00000000000000000", "/tmp")
        .await
        .expect("host B holds the project");
    client
        .call(Command::StartProgram(Box::new(StartProgram {
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
                confinement: None,
            }),
            ..StartProgram::of("01J9PRJCT00000000000000000", ProgramKind::Shell)
        })))
        .await
        .expect("host B starts it");

    assert_eq!(
        *board.heard.lock().unwrap(),
        ["hold_project", "start_program"]
    );
}

#[tokio::test]
async fn the_windows_reply_to_a_hook_line_crosses_the_link_as_its_host_relays_it() {
    let board = Board::default();
    let client = linked(&board).await;

    client
        .call(Command::ReplyHeard {
            project: "01J9PRJCT00000000000000000".into(),
            heard: 1,
            reply: serde_json::json!("allow"),
        })
        .await
        .expect("host B hears the reply");

    assert_eq!(*board.heard.lock().unwrap(), ["reply_heard"]);
}

#[tokio::test]
async fn what_the_person_allowed_of_a_persona_passes_a_link_to_the_host_at_the_far_end() {
    let board = Board::default();
    let client = linked(&board).await;

    let refused = client
        .call(Command::StartProgram(Box::new(StartProgram {
            tools_allowed: Some("digest".to_owned()),
            ..StartProgram::of("01J9PRJCT00000000000000000", ProgramKind::Shell)
        })))
        .await;

    if let Some(refusal) = refused.as_ref().err().and_then(|why| why.refusal()) {
        assert_ne!(refusal.code, session::NOT_ALLOWED, "{}", refusal.why);
    }
}

#[tokio::test]
async fn a_start_without_the_sandbox_is_refused_by_the_host_at_the_far_end() {
    let board = Board::default();
    let client = linked(&board).await;

    let refused = client
        .call(Command::StartProgram(Box::new(StartProgram {
            without_sandbox: true,
            ..StartProgram::of("01J9PRJCT00000000000000000", ProgramKind::Shell)
        })))
        .await
        .unwrap_err();

    let refusal = refused.refusal().expect("host B's refusal");
    assert_eq!(refusal.code, session::NOT_ALLOWED);
    assert!(
        refusal.why.contains("without the sandbox"),
        "{}",
        refusal.why
    );
    assert!(board.heard.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_hello_changed_on_its_way_fails_the_handshake() {
    let (b, mut told) = serve_b(Board::default(), a_as_pinned(), Liveness::default());
    let (a_end, a_inside) = duplex(1 << 16);
    let (b_end, mut b_inside) = duplex(1 << 16);
    let (mut a_reads, mut a_writes) = tokio::io::split(a_inside);
    // On its way, host A's hello gains a version it never offered; everything after goes as
    // it came.
    tokio::spawn(async move {
        let mut magic = [0u8; 8];
        a_reads.read_exact(&mut magic).await.unwrap();
        let mut body = vec![0u8; usize::from(a_reads.read_u16().await.unwrap())];
        a_reads.read_exact(&mut body).await.unwrap();
        let mut hello: serde_json::Value = serde_json::from_slice(&body).unwrap();
        hello["versions"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"major": 1, "minor": 0}));
        let body = hello.to_string();
        b_inside.write_all(&magic).await.unwrap();
        b_inside
            .write_all(&u16::try_from(body.len()).unwrap().to_be_bytes())
            .await
            .unwrap();
        b_inside.write_all(body.as_bytes()).await.unwrap();
        let (mut b_reads, mut b_writes) = tokio::io::split(b_inside);
        // Either way ends when the link does; how it ended is not this test's.
        let _ = tokio::join!(
            tokio::io::copy(&mut a_reads, &mut b_writes),
            tokio::io::copy(&mut b_reads, &mut a_writes)
        );
    });
    b.send(b_end).unwrap();

    let dialed = hostlink::connect(
        a_end,
        Handshake::THIS_BUILD,
        &host_a(),
        &b_as_pinned(),
        Liveness::default(),
    )
    .await;

    assert!(
        matches!(dialed, Err(hostlink::LinkRefused::HandshakeFailed(_))),
        "{:?}",
        dialed.map(|_| ())
    );
    assert!(told.recv().await.unwrap().is_err(), "host B admits nothing");
}

#[tokio::test]
async fn nothing_the_session_protocol_says_crosses_the_hosts_stream_in_the_clear() {
    let board = Board::default();
    let (b, _told) = serve_b(board.clone(), a_as_pinned(), Liveness::default());
    // The stream between the hosts, with everything either end writes kept as it went.
    let (a_end, a_inside) = duplex(1 << 16);
    let (b_end, b_inside) = duplex(1 << 16);
    let seen: Arc<Mutex<Vec<u8>>> = Arc::default();
    let carry = |mut from: tokio::io::ReadHalf<tokio::io::DuplexStream>,
                 mut to: tokio::io::WriteHalf<tokio::io::DuplexStream>| {
        let seen = Arc::clone(&seen);
        tokio::spawn(async move {
            let mut buffer = vec![0u8; 1 << 14];
            while let Ok(read) = from.read(&mut buffer).await {
                if read == 0 || to.write_all(&buffer[..read]).await.is_err() {
                    return;
                }
                seen.lock().unwrap().extend_from_slice(&buffer[..read]);
            }
        })
    };
    let (a_reads, a_writes) = tokio::io::split(a_inside);
    let (b_reads, b_writes) = tokio::io::split(b_inside);
    carry(a_reads, b_writes);
    carry(b_reads, a_writes);
    b.send(b_end).unwrap();
    let link = hostlink::connect(
        a_end,
        Handshake::THIS_BUILD,
        &host_a(),
        &b_as_pinned(),
        Liveness::default(),
    )
    .await
    .unwrap();
    let (client, _views, _events) = Client::new(link);

    assert_eq!(client.list().await.unwrap()[0].chat, CHAT);

    let seen = seen.lock().unwrap();
    let said = |words: &[u8]| seen.windows(words.len()).any(|at| at == words);
    assert!(
        said(hostlink::MAGIC),
        "the hello is in the clear, as it must be"
    );
    assert!(!said(CHAT.as_bytes()), "a chat's id crossed in the clear");
    assert!(!said(b"\"call\""), "a command crossed in the clear");
    assert!(
        !said(&host_a().key.public().as_bytes()[..]),
        "a key crossed in the clear"
    );
}
