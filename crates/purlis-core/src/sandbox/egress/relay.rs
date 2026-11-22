// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The app's own git reaches a chat's forge route through this relay** (one login, slice K,
//! ADR 0128 D3g).
//!
//! A chat's egress proxy hands a forge `CONNECT` to the route's door **with the run's hand-off
//! proof** ([`open_door`]), and since K3 the door answers `407` to any `CONNECT` without it. The
//! git `purlis worktree push` and `update` run for a chat cannot send that proof: it is a plain
//! `HTTPS_PROXY` client. This relay is its proxy. It listens on this machine's loopback, for the
//! life of the run's route, and for each `CONNECT`:
//!
//! 1. **asks the administrator's pin first**, for the name and the forge it belongs to
//!    ([`pinned_forge`]), on every connection, and answers a pinned forge `403` with the pin's
//!    sentence, sending nothing to the door;
//! 2. refuses a host the route does not serve;
//! 3. opens the door through [`open_door`], so the proof and the door's proof back are the same
//!    ones the chat's proxy makes, and carries bytes both ways only once the door has proved itself.
//!
//! It holds the hand-off secret, which never leaves the app's memory: the git it serves sees a
//! loopback port and the sentinel the route issued (which is the chat's own), and **no token of
//! the person's is ever here**: the door terminates, and the proxy core adds the credential on
//! the far side.
//!
//! **Tier** (ADR 0069): nothing is stored; a relay is a listener, for a run, in memory.

use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::{ForgeRoute, open_door, splice};
use crate::forge::pin::{Pin, pinned_forge};

/// The most of a `CONNECT` head read.
const MOST_HEAD: usize = 8 * 1024;
/// How long a client has to send its head.
const HEAD_WITHIN: Duration = Duration::from_secs(10);

/// A relay in front of one run's door, stopped when dropped.
pub struct DoorRelay {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl DoorRelay {
    /// Listens on this machine's loopback for `route`'s door, asking `pin` on every connection.
    pub fn start(route: ForgeRoute, pin: Arc<dyn Pin>, idle: Duration) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let route = Arc::new(route);
        let thread = std::thread::Builder::new()
            .name("forge-door-relay".into())
            .spawn(move || {
                for client in listener.incoming() {
                    if flag.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(client) = client else { continue };
                    let route = Arc::clone(&route);
                    let pin = Arc::clone(&pin);
                    let _ = std::thread::Builder::new()
                        .name("forge-door-relay-conn".into())
                        .spawn(move || serve(client, &route, pin.as_ref(), idle));
                }
            })?;
        Ok(Self {
            addr,
            stop,
            thread: Some(thread),
        })
    }

    /// Where it listens: the proxy address of the app's git.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for DoorRelay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept.
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_secs(1));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn answer(client: &mut TcpStream, status: &str, said: &str) {
    let body = format!("{{\"message\":{}}}", serde_json::Value::from(said));
    let _ = write!(
        client,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = client.flush();
    let _ = client.shutdown(Shutdown::Both);
}

/// The `CONNECT` head, read byte by byte so nothing past it is taken from the client.
fn read_head(client: &mut TcpStream) -> Option<String> {
    let _ = client.set_read_timeout(Some(HEAD_WITHIN));
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() >= MOST_HEAD || client.read(&mut byte).ok()? == 0 {
            return None;
        }
        head.push(byte[0]);
    }
    let _ = client.set_read_timeout(None);
    String::from_utf8(head).ok()
}

fn serve(mut client: TcpStream, route: &ForgeRoute, pin: &dyn Pin, idle: Duration) {
    let Some(head) = read_head(&mut client) else {
        let _ = client.shutdown(Shutdown::Both);
        return;
    };
    let mut line = head.lines().next().unwrap_or_default().split_whitespace();
    let (Some(method), Some(target)) = (line.next(), line.next()) else {
        return answer(&mut client, "400 Bad Request", "not a request purlis reads");
    };
    if !method.eq_ignore_ascii_case("CONNECT") {
        return answer(
            &mut client,
            "405 Method Not Allowed",
            "this port carries CONNECT to a forge and nothing else",
        );
    }
    let Some((host, port)) = target
        .rsplit_once(':')
        .and_then(|(host, port)| Some((host.to_ascii_lowercase(), port.parse::<u16>().ok()?)))
        .filter(|(host, _)| !host.is_empty() && !host.starts_with('['))
    else {
        return answer(&mut client, "400 Bad Request", "not a host and port");
    };
    // The administrator's pin, read for this connection, before the door is dialled.
    if let Some(said) = pinned_forge(pin, &host, port) {
        return answer(&mut client, "403 Forbidden", &said);
    }
    if !route.serves(&host) {
        return answer(
            &mut client,
            "403 Forbidden",
            "purlis carries this connection to a forge of the chat's repos only",
        );
    }
    let (door, answered) = match open_door(route, &host, port, &[], idle) {
        Ok(opened) => opened,
        Err(why) => return answer(&mut client, "502 Bad Gateway", why.said()),
    };
    if client.write_all(&answered).is_err() {
        return;
    }
    splice(client, door, u64::MAX, idle);
}
