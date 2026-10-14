// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A service on this Mac** (qw384): something listening on this machine's loopback interface
//! at one port, such as a local Kubernetes cluster, a database in a container or a dev server,
//! as a grant names it ([`super::grant::What::Port`]).
//!
//! A sandboxed chat reaches this machine's own services by nothing but its own proxy's two
//! ports and the app's hook socket. A program that never uses a proxy for this machine (every
//! Go client: kubectl, helm, gh) connects to the port directly, and the chat's sandbox refuses
//! it. That refusal is a Block naming the port ([`crate::sandboxblock`]), and its Notice offers
//! Allow for this chat, or for every chat of this project on this machine.
//!
//! - **One port, never a range or every port** ([`port`]): a number from 1 to 65535, typed or
//!   named whole.
//! - **Never purlis's own** ([`own`]): a port one of purlis's proxies or tunnels listens on is
//!   each chat's own way out, and no other chat's. Refused when it is allowed, and left out when
//!   a chat starts. A proxy or tunnel purlis opens later never takes
//!   a port a chat's sandbox was started with ([`given`], [`bind_loopback`]).
//! - **This machine's alone**, as a socket is: kept for one chat in memory or in this machine's
//!   local file (`super::local`), never in the committed one.
//! - **What it reaches**: the chat's sandbox lets it connect to that port on the loopback
//!   interface, `localhost`, `127.0.0.1` and `::1` alike, and the chat's proxy carries a client
//!   that does use it there ([`reach_entries`]).

use std::collections::BTreeSet;
use std::net::{IpAddr, TcpListener};
use std::sync::{Mutex, PoisonError};

use super::grant::Refused;

/// What allowing a service on this Mac means, in one honest line.
pub const RISK: &str = "lets this chat connect to whatever listens on that port on this Mac, as \
                        you; a container engine's or a cluster's port can reach much of your \
                        machine";

/// The ports purlis itself listens on in this process: each chat's proxy and each tunnel.
static OWN: Mutex<BTreeSet<u16>> = Mutex::new(BTreeSet::new());

/// The ports a chat's sandbox was started with as a service on this Mac, in this process: no
/// proxy or tunnel of purlis's takes one of them later.
static GIVEN: Mutex<BTreeSet<u16>> = Mutex::new(BTreeSet::new());

/// **`typed` as the port of a service on this Mac**, or why not: one whole number from 1 to
/// 65535, never purlis's own.
pub fn port(typed: &str) -> Result<u16, Refused> {
    let typed = typed.trim();
    let number = typed
        .strip_prefix(':')
        .unwrap_or(typed)
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| {
            Refused::Not(
                "purlis allows a service on this Mac by one port, a number from 1 to 65535: \
                 never a range or every port."
                    .to_owned(),
            )
        })?;
    if is_own(number) {
        return Err(Refused::Never(OWN_SAID.to_owned()));
    }
    Ok(number)
}

/// Why purlis's own port is never allowed.
pub const OWN_SAID: &str = "That port is one purlis itself listens on for a chat, so no chat is \
                            allowed it. A service of your own on this Mac listens on a port of \
                            its own.";

/// Whether purlis listens on `port` in this process now.
pub fn is_own(port: u16) -> bool {
    OWN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains(&port)
}

/// The ports purlis listens on in this process now.
pub fn own() -> Vec<u16> {
    OWN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .copied()
        .collect()
}

/// `ports`, less each that is purlis's own now, each once, in order: what a start compiles in.
/// Each one kept is marked as given ([`given`]).
pub fn still_ports(ports: &[u16]) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    for port in ports {
        if *port != 0 && !is_own(*port) && !out.contains(port) {
            out.push(*port);
        }
    }
    given(&out);
    out
}

/// Marks `ports` as ports a chat's sandbox lets it reach: no proxy or tunnel purlis opens later
/// in this process listens on one.
pub fn given(ports: &[u16]) {
    GIVEN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .extend(ports.iter().copied());
}

/// Whether `port` was given to a chat's sandbox in this process.
fn is_given(port: u16) -> bool {
    GIVEN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains(&port)
}

/// How many times [`bind_loopback`] asks again for a port that was given to a chat.
const BIND_TRIES: usize = 32;

/// **A new port on the loopback interface for purlis's own listener**, never one a chat's
/// sandbox was given ([`given`]), marked as purlis's own until [`release`].
pub fn bind_loopback() -> std::io::Result<TcpListener> {
    let mut held = Vec::new();
    for _ in 0..BIND_TRIES {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        if is_given(port) {
            // Kept bound until a fit is found, so the system hands out another.
            held.push(listener);
            continue;
        }
        OWN.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(port);
        return Ok(listener);
    }
    Err(std::io::Error::other(
        "purlis found no loopback port free of the services chats may reach",
    ))
}

/// `port` is no longer purlis's own: its listener closed.
pub fn release(port: u16) {
    OWN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&port);
}

/// **The entries a chat's proxy lists for `port`**: this machine by each name a client says it
/// by, each held to that one port, so the local-address check carries exactly that address and
/// port ([`super::reach::Reach::lists_exactly`]) and nothing else of this machine.
pub fn reach_entries(port: u16) -> [String; 3] {
    [
        format!("127.0.0.1:{port}"),
        format!("[::1]:{port}"),
        format!("localhost:{port}"),
    ]
}

/// The Seatbelt rule that lets a wrapped chat connect to `port` on the loopback interface
/// (`localhost` in a Seatbelt address is the loopback interface, IPv4 and IPv6).
pub fn seatbelt_rule(port: u16) -> String {
    format!("(allow network-outbound (remote ip \"localhost:{port}\"))")
}

/// **The port of a service on this Mac that `target` names**, where it names one on the
/// loopback interface: `127.0.0.1:50795`, `[::1]:50795`, `localhost:50795`, any `127.x.y.z`.
/// None for any other address, a name, or no port.
pub fn loopback_port(target: &str) -> Option<u16> {
    let target = target.trim().trim_end_matches(['.', ',', ';']);
    let (host, port) = if let Some(rest) = target.strip_prefix('[') {
        let (inside, after) = rest.split_once(']')?;
        (inside, after.strip_prefix(':')?)
    } else {
        target.rsplit_once(':')?
    };
    let port = port.parse::<u16>().ok().filter(|port| *port != 0)?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| match ip {
            IpAddr::V4(v4) => v4.is_loopback(),
            IpAddr::V6(v6) => {
                v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
            }
        });
    loopback.then_some(port)
}

/// **The port a program's own refusal of a connection on this machine names** (qw384): Go's
/// `dial tcp 127.0.0.1:50795: connect: operation not permitted`, and Node's `connect EPERM
/// 127.0.0.1:50795`. None for any other line, and for an address that is not this machine's
/// loopback.
pub fn refused_in(line: &str) -> Option<u16> {
    let lower = line.to_ascii_lowercase();
    if let Some(at) = lower.find("dial tcp ")
        && lower.contains("connect: operation not permitted")
    {
        let rest = &line[at + "dial tcp ".len()..];
        // `dial tcp 127.0.0.1:50795: connect: …` and `dial tcp [::1]:50795: connect: …`.
        let address = rest.split(": ").next()?;
        return loopback_port(address);
    }
    if let Some(at) = lower.find("connect eperm ") {
        let rest = &line[at + "connect eperm ".len()..];
        return loopback_port(rest.split_whitespace().next()?);
    }
    None
}

#[cfg(test)]
#[path = "localport_tests.rs"]
mod tests;
