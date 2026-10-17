// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat planned on another machine reaches of the networks its runner is in** (#2445,
//! ruled 2026-10-17).
//!
//! A runner starts the confinement of a chat the desktop's window planned only where the plan
//! fits the runner ([`super::plan::Plan::fits`]). Public hosts are carried as planned. An
//! address of a private network, a cluster's or a link's ([`private_address`]), and every address
//! of the runner itself but its loopback, are carried only where the runner's own policy lists
//! them ([`super::policy`]'s `hosts`), by the name asked for or by that address. That is how a
//! cluster's administrator lets a runner's chats reach an internal service. A cloud metadata
//! service is never one.
//!
//! The same rule is held at plan time, for what the plan lists and what its names resolve to,
//! and at every connect ([`super::egress`]), since a name can resolve to another address later.
//! The loopback is the services the runner gives the chat ([`super::plan::Here::given`]).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;

use super::hosts::{Host, embedded_v4, metadata_address, refused_address};

/// **Whether `ip` is an address of a private network, a cluster's or a link's**: `10/8`,
/// `172.16/12`, `192.168/16`, the shared `100.64/10` many clusters give their pods and services,
/// IPv4 and IPv6 link-local, and IPv6's unique-local `fc00::/7` and site-local `fec0::/10`. An
/// IPv6 address that carries an IPv4 one is judged as that one.
pub fn private_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => private_v4(ip),
        IpAddr::V6(ip) => match embedded_v4(ip) {
            Some(carried) => private_v4(carried),
            None => {
                let first = ip.segments()[0];
                ip.is_unicast_link_local() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfec0
            }
        },
    }
}

fn private_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_private() || ip.is_link_local() || (a == 100 && b & 0xc0 == 64)
}

/// **The internal services this machine's policy lists** (module docs): what a chat planned on
/// another machine reaches past the public Internet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Internal {
    listed: Vec<Host>,
}

impl Internal {
    /// The services `listed` names.
    pub fn listing(listed: Vec<Host>) -> Self {
        Self { listed }
    }

    /// What this machine's policy lists for the project at `root`: its `hosts`, where it fixes
    /// them; none where it does not.
    pub fn of(root: &Path) -> Self {
        Self::listing(
            super::policy::Locks::of(root)
                .allowed_hosts()
                .map(<[Host]>::to_vec)
                .unwrap_or_default(),
        )
    }

    /// **Whether `host`, reached at `at`, is carried**: an address that is not internal is, and
    /// an internal one only where this machine's policy lists `host` or that address on that
    /// port. A cloud metadata service is never carried. `own` is this machine's interface
    /// addresses; where they could not be read, every address but the loopback is held as
    /// internal.
    pub fn carries(&self, host: &str, at: SocketAddr, own: Option<&[IpAddr]>) -> bool {
        if !held(at.ip(), own) {
            return true;
        }
        !metadata_address(at.ip()) && (self.lists(host, at.port()) || self.lists_address(at))
    }

    /// Whether this machine's policy lists `host` on `port`, or on every port.
    pub fn lists(&self, host: &str, port: u16) -> bool {
        Host::parse(&super::egress::host_and_port(host, port))
            .is_ok_and(|asked| self.listed.iter().any(|one| one.covers(&asked)))
    }

    /// **Whether this machine's policy lists `entry`**, a host as a plan lists one: never one
    /// that is or names a cloud metadata service's address.
    pub fn lists_entry(&self, entry: &str) -> bool {
        Host::parse(entry).is_ok_and(|asked| {
            !asked.address().is_some_and(metadata_address)
                && self.listed.iter().any(|one| one.covers(&asked))
        })
    }

    /// Whether this machine's policy lists the address `at`, as an address.
    fn lists_address(&self, at: SocketAddr) -> bool {
        Host::parse(&at.to_string())
            .is_ok_and(|asked| self.listed.iter().any(|one| one.covers(&asked)))
    }
}

/// Whether `ip` is internal (module docs): not the loopback, and an address of this machine,
/// a link-local or cloud metadata one ([`refused_address`]), or a private network's.
pub(crate) fn held(ip: IpAddr, own: Option<&[IpAddr]>) -> bool {
    if is_loopback(ip) {
        return false;
    }
    match own {
        Some(own) => refused_address(ip, own) || private_address(ip),
        None => true,
    }
}

/// The loopback as the services a chat is given are reached at: `127.0.0.0/8` and `::1`.
fn is_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
}

#[cfg(test)]
mod tests;
