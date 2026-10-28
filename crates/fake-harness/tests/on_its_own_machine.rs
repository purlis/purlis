// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a chat does on the machine it runs on, for the behaviour suite to see from its pane on
//! every target, a Docker pool's runner included (#3288): a service it serves, a tunnel it asks
//! its proxy for, and a listener it starts on a socket's path.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// The harness run with `args`, its standard output read line by line.
fn harness(args: &[&str]) -> (Child, std::io::Lines<BufReader<std::process::ChildStdout>>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fake-harness"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let said = BufReader::new(child.stdout.take().unwrap()).lines();
    (child, said)
}

#[test]
fn a_service_it_serves_says_its_port_and_takes_a_connection_there() {
    let (mut serving, mut said) = harness(&["--serve"]);

    let line = said.next().unwrap().unwrap();
    let port: u16 = line
        .strip_prefix("serving on port ")
        .and_then(|port| port.parse().ok())
        .unwrap_or_else(|| panic!("said {line:?}"));
    std::net::TcpStream::connect(("127.0.0.1", port)).expect("it takes a connection");

    serving.kill().unwrap();
    serving.wait().unwrap();
}

#[test]
fn a_tunnel_says_the_status_line_its_proxy_answers() {
    let proxy = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let at = format!("127.0.0.1:{}", proxy.local_addr().unwrap().port());
    let answering = std::thread::spawn(move || {
        let (stream, _) = proxy.accept().unwrap();
        let mut asked = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut asked)
            .unwrap();
        (&stream)
            .write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n")
            .unwrap();
        asked
    });

    let (mut tunnel, mut said) = harness(&["--tunnel", "refused.invalid:443", "--through", &at]);

    assert_eq!(
        said.next().unwrap().unwrap(),
        "tunnel to refused.invalid:443: HTTP/1.1 403 Forbidden"
    );
    assert_eq!(
        answering.join().unwrap(),
        "CONNECT refused.invalid:443 HTTP/1.1\r\n"
    );
    assert!(tunnel.wait().unwrap().success());
}

#[test]
fn a_listener_takes_a_sockets_path_over_and_ends_once_its_one_caller_has_gone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hooks.sock");
    let _first = UnixListener::bind(&path).unwrap();
    let held = path.to_str().unwrap().to_owned();

    let (mut listening, mut said) = harness(&["--listen-at", &held]);

    assert_eq!(said.next().unwrap().unwrap(), "listening beside the chat.");
    let mut caller = UnixStream::connect(&path).expect("its listener takes it");
    caller
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    caller.shutdown(std::net::Shutdown::Write).unwrap();
    let mut rest = Vec::new();
    let _ = caller.read_to_end(&mut rest);
    assert!(
        listening.wait().unwrap().success(),
        "it ends once its caller has gone"
    );
}
