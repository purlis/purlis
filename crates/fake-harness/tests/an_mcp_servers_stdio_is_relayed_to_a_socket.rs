// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `--mcp-relay`: the harness starts its MCP server as its own child, as a harness does, and
//! hands the server's stdio to whoever listens on a socket, unchanged both ways (#2243).

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn a_line_sent_on_the_socket_reaches_the_server_and_its_answer_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("relay.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let mut harness = Command::new(env!("CARGO_BIN_EXE_fake-harness"))
        .arg("--mcp-relay")
        .arg(&socket)
        .args([
            "--mcp-server",
            "/bin/sh",
            "-c",
            "read line; echo \"server heard: $line\"; echo \"its parent: $PPID\"",
        ])
        .stdin(Stdio::null())
        .spawn()
        .unwrap();

    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(b"tools/list\n").unwrap();
    let mut lines = BufReader::new(stream.try_clone().unwrap()).lines();

    assert_eq!(lines.next().unwrap().unwrap(), "server heard: tools/list");
    assert_eq!(
        lines.next().unwrap().unwrap(),
        format!("its parent: {}", harness.id()),
        "the server is the harness's own child"
    );
    drop(lines);
    drop(stream);
    assert!(harness.wait().unwrap().success(), "it ends with its server");
}
