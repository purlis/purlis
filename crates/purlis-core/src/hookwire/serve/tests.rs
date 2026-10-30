// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's server dials in (#3316): a `serve` line is admitted as every line is, its connection
//! handed on whole, and a session host hands it on to the window. First runs on CI: this sandbox
//! binds no socket.

use super::super::{Answer, ChatToken, Hearing, Listener, Reading, line_with};
use super::*;
use std::io::{BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn hearing(serve: crate::hookwire::Serves) -> Hearing {
    Hearing {
        secret_exec: Box::new(|_, _, writer| crate::secrets::brokered::not_answered(writer)),
        blocked: Box::new(|_| {}),
        doing: Box::new(|_| {}),
        output: Box::new(|_| {}),
        touching: Box::new(|_| {}),
        each: Box::new(|_| Ok(())),
        answer: Box::new(|_, _| Answer::No { why: String::new() }),
        noticed: Box::new(|_| {}),
        saved: Box::new(|_| {}),
        refused: Box::new(|_| Ok(())),
        tool: Box::new(|_| Ok(())),
        permission: Box::new(|_| None),
        serve,
    }
}

/// An app on `path` whose every slot is parked in `kept`, and chat 3's token.
fn an_app_parking(
    dir: &std::path::Path,
    path: &std::path::Path,
) -> (Reading, ChatToken, Arc<Mutex<Vec<(ServeAsk, Slot)>>>) {
    let listener = Listener::bind(dir, path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let kept: Arc<Mutex<Vec<(ServeAsk, Slot)>>> = Arc::default();
    let keeping = Arc::clone(&kept);
    let reading = listener.hear(hearing(Box::new(move |ask, mut slot| {
        parked(&mut slot).expect("told");
        keeping.lock().unwrap().push((ask, slot));
    })));
    (reading, token, kept)
}

fn dial(path: &std::path::Path, token: Option<&ChatToken>, port: u16) -> (UnixStream, String) {
    let mut socket = UnixStream::connect(path).expect("connected");
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("a deadline");
    socket
        .write_all(
            &line_with(
                token,
                &ServeAsk {
                    chat: 3,
                    serve: port,
                },
            )
            .expect("a line"),
        )
        .expect("sent");
    let mut said = String::new();
    let _ = std::io::BufReader::new(&socket).read_line(&mut said);
    (socket, said)
}

#[test]
fn a_serve_line_with_its_chats_token_hands_its_connection_on_whole() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let (_reading, token, kept) = an_app_parking(dir.path(), &path);
    let (mut server, said) = dial(&path, Some(&token), 5173);
    assert_eq!(said, "{\"taken\":true}\n");
    let (ask, mut slot) = kept.lock().unwrap().pop().expect("handed on");
    assert_eq!(
        ask,
        ServeAsk {
            chat: 3,
            serve: 5173
        }
    );
    // The connection itself: what one end writes, the other reads, both ways.
    slot.write_all(b"go").expect("written");
    let mut two = [0u8; 2];
    server.read_exact(&mut two).expect("read");
    assert_eq!(&two, b"go");
    server.write_all(b"ok").expect("written back");
    slot.read_exact(&mut two).expect("read back");
    assert_eq!(&two, b"ok");
}

#[test]
fn a_serve_line_without_its_chats_token_is_refused_and_closed_unparked() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let (_reading, _token, kept) = an_app_parking(dir.path(), &path);
    for token in [None, Some(ChatToken::from("nope"))] {
        let (mut server, said) = dial(&path, token.as_ref(), 5173);
        assert!(said.contains("token"), "{said}");
        let mut rest = Vec::new();
        assert_eq!(server.read_to_end(&mut rest).unwrap_or(0), 0, "closed");
    }
    assert!(kept.lock().unwrap().is_empty(), "nothing was parked");
}

#[test]
fn a_refused_slot_is_told_why_and_closed() {
    let (slot, mut server) = UnixStream::pair().expect("a pair");
    refused(slot, "no door");
    let mut said = String::new();
    server.read_to_string(&mut said).expect("read to its end");
    assert_eq!(said, "{\"taken\":false,\"refused\":\"no door\"}\n");
}

#[test]
fn a_host_hands_a_slot_on_to_the_window_beside_its_hook_socket() {
    let dir = tempfile::tempdir().expect("a directory");
    let hook = dir.path().join("app").join("hooks.sock");
    let kept: Arc<Mutex<Vec<(ServeAsk, Slot)>>> = Arc::default();
    let keeping = Arc::clone(&kept);
    let receiving = Receiving::bind(
        &hook,
        Box::new(move |ask, slot| keeping.lock().unwrap().push((ask, slot))),
    )
    .expect("the window's socket");
    assert!(beside(&hook).exists());
    let (slot, mut server) = UnixStream::pair().expect("a pair");
    let ask = ServeAsk {
        chat: 3,
        serve: 5173,
    };
    hand_on(&beside(&hook), &ask, &slot).expect("handed on");
    drop(slot);
    let (got, mut taken) = kept.lock().unwrap().pop().expect("taken");
    assert_eq!(got, ask);
    taken.write_all(b"go").expect("written");
    let mut two = [0u8; 2];
    server.read_exact(&mut two).expect("the same connection");
    assert_eq!(&two, b"go");
    drop(receiving);
    assert!(!beside(&hook).exists(), "its socket is let go");
}

#[test]
fn a_host_with_no_window_to_take_a_slot_is_told_so() {
    let dir = tempfile::tempdir().expect("a directory");
    let (slot, _server) = UnixStream::pair().expect("a pair");
    assert!(
        hand_on(
            &beside(&dir.path().join("hooks.sock")),
            &ServeAsk {
                chat: 3,
                serve: 5173
            },
            &slot
        )
        .is_err()
    );
}
