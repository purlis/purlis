// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A test server on a port the system picks, on 127.0.0.1** (#3396, ruled 2026-10-30).
//!
//! On Linux a chat may bind a TCP socket to port 0 on 127.0.0.1, as Go's `httptest`, a Node test
//! server started on 127.0.0.1 and Python's `HTTPServer(("127.0.0.1", 0))` do, with no grant: the
//! supervisor binds a socket of its own there and hands it to the chat. It may then connect to
//! such a port only while the socket the supervisor made for it is the one socket listening on
//! it: never to another service on this machine, which a grant still names.
//!
//! The port is one of a pool the system picked when the wrap started, before the wrap held
//! itself with Landlock. Landlock names ports one by one, and no address: so the wrap's own
//! layer allows binding and connecting on these ports ([`Picked::ports`]) to any host, and the
//! supervisor's code is what holds each to 127.0.0.1. The chat's own layer allows no port.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
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

/// Every socket `table` (the text of `/proc/net/tcp`, of `/proc/net/tcp6`, or of both one after
/// the other) has listening on `port`, on whatever address: the address and the socket's inode,
/// in the table's order.
pub(crate) fn listening(table: &str, port: u16) -> Vec<(IpAddr, u64)> {
    table
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (address, state, inode) = (fields.get(1)?, fields.get(3)?, fields.get(9)?);
            let (at, on) = address.split_once(':')?;
            if u16::from_str_radix(on, 16).ok()? != port || *state != LISTENING {
                return None;
            }
            Some((address_of(at)?, inode.parse::<u64>().ok()?))
        })
        .collect()
}

/// An address as the kernel's tables write it: the 32-bit numbers it holds, each in this
/// machine's byte order, one for IPv4 and four for IPv6.
fn address_of(hex: &str) -> Option<IpAddr> {
    let mut octets = Vec::with_capacity(16);
    for at in (0..hex.len()).step_by(8) {
        let word = u32::from_str_radix(hex.get(at..at + 8)?, 16).ok()?;
        octets.extend_from_slice(&word.to_ne_bytes());
    }
    match octets.len() {
        4 => Some(IpAddr::V4(Ipv4Addr::new(
            octets[0], octets[1], octets[2], octets[3],
        ))),
        16 => <[u8; 16]>::try_from(octets.as_slice())
            .ok()
            .map(|it| IpAddr::V6(Ipv6Addr::from(it))),
        _ => None,
    }
}

/// What a connect to 127.0.0.1 at a port comes to, for the pool picked for a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PoolPort {
    /// Not a port of the pool: the profile alone judges it.
    NotPicked,
    /// The socket made for the chat there listens, and no other socket does: reached.
    Own,
    /// Nothing listens there: refused as the kernel refuses it, `ECONNREFUSED`, and never told,
    /// so no Notice offers to allow a port the system picked.
    Free,
    /// Another socket listens there: refused, and never told for the same reason.
    Taken,
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

    /// What a connect to 127.0.0.1 at `port` comes to by `table` (the text of
    /// `/proc/net/tcp` and `/proc/net/tcp6`, one after the other): the chat's own only where the
    /// socket made for it is the one socket listening on that port, on any address, either
    /// family.
    pub(crate) fn reach(&self, port: u16, table: &str) -> PoolPort {
        if !self.ports.contains(&port) {
            return PoolPort::NotPicked;
        }
        let listeners = listening(table, port);
        if listeners.is_empty() {
            return PoolPort::Free;
        }
        let own = self
            .owned
            .lock()
            .ok()
            .and_then(|owned| owned.get(&port).copied());
        match own {
            Some(inode) if listeners == [(IpAddr::V4(Ipv4Addr::LOCALHOST), inode)] => PoolPort::Own,
            _ => PoolPort::Taken,
        }
    }
}

#[cfg(test)]
mod tests;
