// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `--mcp-relay`: the harness's MCP server, started as a harness starts one, and spoken to by a
//! test over a socket.
//!
//! A harness runs each MCP server it is configured with as its own child, on stdio, so what the
//! server does runs inside the chat: its hook lines come from a process below the chat's
//! program, which is what the chat's hook channel admits. Here the harness starts the server the
//! same way and hands its stdio to whoever listens on the socket, unchanged in both directions,
//! so a test drives the server's tools as the model would. It ends once the test closes the
//! socket and the server has ended.

use std::io;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

/// Relays the stdio of `server` (its program, then its words), started now, to the socket at
/// `socket`, until the test closes it and the server ends.
pub fn relay(socket: &Path, server: &[String]) -> Result<(), String> {
    let (program, words) = server
        .split_first()
        .ok_or_else(|| "--mcp-server names no program".to_owned())?;
    let stream = UnixStream::connect(socket)
        .map_err(|err| format!("cannot reach {}: {err}", socket.display()))?;
    let mut child = Command::new(program)
        .args(words)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|err| format!("cannot start {program}: {err}"))?;
    let mut to_server = child.stdin.take().expect("piped");
    let mut from_server = child.stdout.take().expect("piped");
    let mut asked = stream
        .try_clone()
        .map_err(|err| format!("cannot use the socket twice: {err}"))?;
    // What the test sends goes to the server until the test closes its end; then the server's
    // stdin closes, as a harness's does when it lets go, and the server ends.
    let asking = thread::spawn(move || {
        let _ = io::copy(&mut asked, &mut to_server);
    });
    let mut answers = stream;
    let _ = io::copy(&mut from_server, &mut answers);
    let _ = answers.shutdown(std::net::Shutdown::Write);
    let _ = asking.join();
    child
        .wait()
        .map_err(|err| format!("cannot wait for {program}: {err}"))?;
    Ok(())
}
