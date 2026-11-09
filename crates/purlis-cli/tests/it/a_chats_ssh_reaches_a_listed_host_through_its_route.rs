// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's ssh, run as a chat runs it, reaches a host through its route and the `purlis` this
//! build made as the route's proxy command (#3536), and nothing the route refuses.
//!
//! The route ran `/usr/bin/nc -X 5 -x` before, which Fedora's ncat refuses, and only the text it
//! wrote was tested. Here the system's own `/usr/bin/ssh` connects through the route's
//! configuration to a server that answers as an ssh server starts: its identification line
//! reaching the server is the connection carried. The nightly `fedora` job, whose `nc` is
//! ncat, runs it beside the Ubuntu runners.
//!
//! It binds the proxy's ports and a server's, which a sandboxed agent cannot: it first runs on
//! CI.

#![cfg(unix)]

use crate::support;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::time::Duration;

use purlis_core::sandbox::egress::{Proxy, Serving};
use purlis_core::sandbox::reach::Reach;
use purlis_core::sandbox::tunnel::SshRoute;

/// The system's ssh, which a route's `ssh` runs.
const SSH: &str = "/usr/bin/ssh";

/// A proxy whose SOCKS port carries `127.0.0.1:<port>` alone: the one way its local-address
/// check lets this machine be reached.
fn a_proxy_listing(port: u16) -> Proxy {
    Proxy::serving(Serving::of(Reach::open(vec![format!("127.0.0.1:{port}")]))).expect("a proxy")
}

/// The route's `ssh` to `127.0.0.1:<port>`, asking nothing of anyone and trusting no host key
/// file, as a test's must: what it prints and how it ended.
fn ssh_through(route: &SshRoute, port: u16) -> std::process::Output {
    assert!(
        Path::new(SSH).is_file(),
        "{SSH} is missing: install the OpenSSH client (openssh-client, openssh-clients)"
    );
    support::command(route.bin().join("ssh"))
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "UserKnownHostsFile=/dev/null",
            "-o",
            "ConnectTimeout=20",
            "-p",
            &port.to_string(),
            "127.0.0.1",
            "true",
        ])
        .output()
        .expect("ssh ran")
}

#[test]
fn a_chats_ssh_reaches_a_listed_host_through_its_route_with_purliss_own_proxy() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let port = listener.local_addr().expect("an address").port();
    // Told through a channel, so an ssh that never reached it fails the test, not hangs it.
    let (told, heard) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
        let _ = stream.write_all(b"SSH-2.0-purlis_route_test\r\n");
        let mut line = String::new();
        let _ = BufReader::new(stream).read_line(&mut line);
        let _ = told.send(line);
    });
    let proxy = a_proxy_listing(port);
    let tmp = tempfile::tempdir().expect("a temp dir");
    let route = SshRoute::write(
        tmp.path(),
        proxy.socks_port(),
        Some(Path::new(support::PURLIS)),
    )
    .expect("the route");
    let config = std::fs::read_to_string(route.config()).expect("its config");
    assert!(
        !config.contains("nc "),
        "the route runs no netcat:\n{config}"
    );
    let ran = ssh_through(&route, port);
    let line = heard
        .recv_timeout(Duration::from_secs(10))
        .unwrap_or_default();
    assert!(
        line.starts_with("SSH-2.0-"),
        "ssh's identification reached the host through the route: {line:?}\nssh said: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
}

#[test]
fn a_chats_ssh_to_a_host_its_route_refuses_fails_says_why_and_never_connects() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let port = listener.local_addr().expect("an address").port();
    listener.set_nonblocking(true).expect("nonblocking");
    // Lists another port of this machine, so this one is not the chat's to reach.
    let proxy = a_proxy_listing(port.wrapping_add(1).max(1));
    let tmp = tempfile::tempdir().expect("a temp dir");
    let route = SshRoute::write(
        tmp.path(),
        proxy.socks_port(),
        Some(Path::new(support::PURLIS)),
    )
    .expect("the route");
    let ran = ssh_through(&route, port);
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(!ran.status.success(), "ssh connected: {said}");
    assert!(
        said.contains("may not reach"),
        "the chat is told why: {said}"
    );
    std::thread::sleep(Duration::from_millis(200));
    assert!(listener.accept().is_err(), "the route connected to it");
}
