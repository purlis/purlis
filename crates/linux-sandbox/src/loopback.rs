// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A test server on a port the system picks, on 127.0.0.1** (#3396, ruled 2026-10-30).
//!
//! On Linux a chat may bind port 0 on 127.0.0.1, as Go's `httptest`, a Node test server and
//! Python's `HTTPServer(("127.0.0.1", 0))` do, with no grant: the supervisor binds a socket of
//! its own there and hands it to the chat. It may then connect to such a port only while the
//! socket the supervisor made for it alone listens there: never to another service on this
//! machine, which a grant still names.
//!
//! The port is one of a pool the system picked when the wrap started, before the wrap held
//! itself with Landlock: Landlock names the ports a process may bind and connect to one by one,
//! so the wrap's own layer allows exactly these ([`Picked::ports`]) and no other.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The state `/proc/net/tcp` gives a listening socket.
const LISTENING: &str = "0A";

/// Whether a bind of `local` asks for a port the system picks on 127.0.0.1, the one address
/// a chat may take one on: never every interface, `::1` or another of 127/8 (the ruling names
/// 127.0.0.1 alone).
pub(crate) fn picks_on_the_loopback(local: SocketAddr) -> bool {
    local.ip() == IpAddr::V4(Ipv4Addr::LOCALHOST) && local.port() == 0
}

/// The inodes of the sockets `table` (the text of `/proc/net/tcp`) has listening at `ip` and
/// `port`, in its order.
pub(crate) fn listening_at(table: &str, ip: Ipv4Addr, port: u16) -> Vec<u64> {
    table
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (address, state, inode) = (fields.get(1)?, fields.get(3)?, fields.get(9)?);
            let (at, on) = address.split_once(':')?;
            // The address is the number the kernel holds, written in this machine's order.
            let at = Ipv4Addr::from(u32::from_str_radix(at, 16).ok()?.to_ne_bytes());
            let on = u16::from_str_radix(on, 16).ok()?;
            (at == ip && on == port && *state == LISTENING)
                .then(|| inode.parse::<u64>().ok())
                .flatten()
        })
        .collect()
}

/// The pool of ports the system picked for a wrap, and the socket the supervisor made on each
/// for the chat.
pub(crate) struct Picked {
    ports: Vec<u16>,
    next: AtomicUsize,
    owned: Mutex<HashMap<u16, u64>>,
}

impl Picked {
    pub(crate) fn new(ports: Vec<u16>) -> Picked {
        Picked {
            ports,
            next: AtomicUsize::new(0),
            owned: Mutex::new(HashMap::new()),
        }
    }

    /// Every port of the pool.
    pub(crate) fn ports(&self) -> &[u16] {
        &self.ports
    }

    /// The pool's ports in the order a bind tries them: each bind starts one further on, so a
    /// port a server just let go is the last tried again.
    pub(crate) fn turn(&self) -> Vec<u16> {
        if self.ports.is_empty() {
            return Vec::new();
        }
        let start = self.next.fetch_add(1, Ordering::Relaxed) % self.ports.len();
        self.ports[start..]
            .iter()
            .chain(&self.ports[..start])
            .copied()
            .collect()
    }

    /// Records that the supervisor made the socket `inode` on `port` for the chat.
    pub(crate) fn own(&self, port: u16, inode: u64) {
        if let Ok(mut owned) = self.owned.lock() {
            owned.insert(port, inode);
        }
    }

    /// Whether `port` is the chat's own by `table` (the text of `/proc/net/tcp`): a port of the
    /// pool where the socket made for the chat listens on 127.0.0.1, and no other does.
    pub(crate) fn owns(&self, port: u16, table: &str) -> bool {
        if !self.ports.contains(&port) {
            return false;
        }
        let Some(inode) = self
            .owned
            .lock()
            .ok()
            .and_then(|owned| owned.get(&port).copied())
        else {
            return false;
        };
        listening_at(table, Ipv4Addr::LOCALHOST, port) == [inode]
    }
}

#[cfg(test)]
mod tests;
