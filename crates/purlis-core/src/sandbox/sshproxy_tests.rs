// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The ssh route's own proxy command (#3536): what it carries through a chat's SOCKS port, what
//! it refuses, and the `ProxyCommand` a route writes for it.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::time::Duration;

use super::egress::{Proxy, Serving};
use super::reach::Reach;
use super::sshproxy::{COMMAND, carry, proxy_command};

/// A proxy whose SOCKS port carries `127.0.0.1:<port>` alone: the one way the local-address
/// check lets this machine be reached, so it can be driven against a server here.
fn a_proxy_listing(port: u16) -> Proxy {
    Proxy::serving(Serving::of(Reach::open(vec![format!("127.0.0.1:{port}")]))).expect("a proxy")
}

/// A server here that echoes the first thing each connection sends, and its port.
fn an_echo_server() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let port = listener.local_addr().expect("an address").port();
    (listener, port)
}

#[test]
fn what_ssh_writes_reaches_a_listed_host_through_the_socks_port_and_its_answer_comes_back() {
    let (listener, port) = an_echo_server();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("the proxy's connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .expect("a timeout");
        let mut got = [0u8; 22];
        stream.read_exact(&mut got).expect("what ssh wrote");
        stream.write_all(&got).expect("echoed");
        got
    });
    let proxy = a_proxy_listing(port);
    let mut answer = Vec::new();
    carry(
        proxy.socks_port(),
        "127.0.0.1",
        port,
        &b"SSH-2.0-OpenSSH_9.9\r\n\0"[..],
        &mut answer,
    )
    .expect("carried");
    assert_eq!(
        server.join().expect("the server"),
        *b"SSH-2.0-OpenSSH_9.9\r\n\0"
    );
    assert_eq!(answer, b"SSH-2.0-OpenSSH_9.9\r\n\0");
}

#[test]
fn a_host_the_chat_may_not_reach_is_refused_with_why_and_never_connected_to() {
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    // Lists another port of this machine, so this one is not the chat's to reach.
    let proxy = a_proxy_listing(port.wrapping_add(1).max(1));
    let mut answer = Vec::new();
    let why = carry(
        proxy.socks_port(),
        "127.0.0.1",
        port,
        &b"x"[..],
        &mut answer,
    )
    .expect_err("refused");
    let said = why.to_string();
    assert!(said.contains(&format!("127.0.0.1:{port}")), "{said}");
    assert!(said.contains("may not reach"), "{said}");
    assert!(answer.is_empty(), "nothing came back: {answer:?}");
    std::thread::sleep(Duration::from_millis(200));
    assert!(listener.accept().is_err(), "the proxy connected to it");
}

#[test]
fn a_socks_port_nobody_listens_on_is_said_as_the_route_not_answering() {
    // Bound, then let go: nothing listens there now.
    let (listener, port) = an_echo_server();
    drop(listener);
    let mut answer = Vec::new();
    let why =
        carry(port, "example.com", 22, &b"x"[..], &mut answer).expect_err("nothing to carry it");
    let said = why.to_string();
    assert!(said.contains("example.com:22"), "{said}");
    assert!(said.contains("route"), "{said}");
}

#[test]
fn the_route_runs_purliss_own_proxy_where_one_is_found() {
    assert_eq!(
        proxy_command(Some(Path::new("/opt/purlis/bin/purlis")), 41999),
        format!("'/opt/purlis/bin/purlis' {COMMAND} 41999 %h %p")
    );
    // A `%` is ssh's own token there, so one in the path is doubled.
    assert_eq!(
        proxy_command(Some(Path::new("/tmp/100%/it's/purlis")), 1),
        format!("'/tmp/100%%/it'\\''s/purlis' {COMMAND} 1 %h %p")
    );
}

#[test]
fn with_no_proxy_found_the_route_refuses_every_connection_and_says_why() {
    let command = proxy_command(None, 41999);
    let ran = crate::forklock::output(
        std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(format!("exec {command}")),
    )
    .expect("ran");
    assert!(!ran.status.success(), "{command}");
    assert!(ran.stdout.is_empty(), "{command}");
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(said.starts_with("purlis: "), "{said}");
    assert!(said.contains("no purlis command line"), "{said}");
}
