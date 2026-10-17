// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat planned on another machine reaches of the networks its runner is in** (#2445,
//! ruled 2026-10-17).
//!
//! A runner starts the confinement of a chat the desktop's window planned only where the plan
//! fits the runner ([`super::plan::Plan::fits`]). Public hosts are carried as planned. An
//! address of a private network, a cluster's or a link's ([`private_address`]), and every address
//! of the runner itself or of a network it is on but `127.0.0.1` and `::1`, are carried only
//! where the runner's own policy lists them ([`super::policy`]'s `relayed-internal`, ruled
//! 2026-10-18), by the name asked for or by that address. That is how a cluster's administrator
//! lets a runner's relayed chats reach an internal service. A cloud metadata service is never
//! one.
//!
//! The same rule is held at plan time, for what the plan lists and what its names resolve to,
//! and at every connect ([`super::egress`]), since a name can resolve to another address later.
//! The loopback is the services the runner gives the chat ([`super::plan::Here::given`]).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};

use super::hosts::{Host, Network, embedded_v4, metadata_address, refused_address};

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Internal(Source);

/// Where an [`Internal`] reads what it lists.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// A fixed list.
    Listed(Vec<Host>),
    /// This machine's policy for the project at the path, read at every question, so a policy
    /// narrowed while a chat runs holds its next connection.
    Policy(PathBuf),
}

impl Default for Internal {
    fn default() -> Self {
        Self::listing(Vec::new())
    }
}

impl Internal {
    /// The services `listed` names.
    pub fn listing(listed: Vec<Host>) -> Self {
        Self(Source::Listed(listed))
    }

    /// What this machine's policy lists for the project at `root` (ruled 2026-10-18): its
    /// `relayed-internal`, read again at every question; none where it lists none.
    pub fn of(root: &Path) -> Self {
        Self(Source::Policy(root.to_path_buf()))
    }

    /// What it lists now.
    fn listed(&self) -> Vec<Host> {
        match &self.0 {
            Source::Listed(listed) => listed.clone(),
            Source::Policy(root) => super::policy::Locks::of(root).relayed_internal().to_vec(),
        }
    }

    /// **Whether `host`, reached at `at`, is carried**: an address that is not internal is, and
    /// an internal one only where this machine's policy lists `host` or that address on that
    /// port. A cloud metadata service is never carried. `own` is this machine's networks; where
    /// they could not be read, every address but `127.0.0.1` and `::1` is held as internal.
    pub fn carries(&self, host: &str, at: SocketAddr, own: Option<&[Network]>) -> bool {
        if !held(at.ip(), own) {
            return true;
        }
        if metadata_address(at.ip()) {
            return false;
        }
        let listed = self.listed();
        let covered = |asked: &str| {
            Host::parse(asked).is_ok_and(|asked| listed.iter().any(|one| one.covers(&asked)))
        };
        covered(&super::egress::host_and_port(host, at.port())) || covered(&at.to_string())
    }

    /// **Whether this machine's policy lists `entry`**, a host as a plan lists one: never one
    /// that is or names a cloud metadata service's address.
    pub fn lists_entry(&self, entry: &str) -> bool {
        Host::parse(entry).is_ok_and(|asked| {
            !asked.address().is_some_and(metadata_address)
                && self.listed().iter().any(|one| one.covers(&asked))
        })
    }
}

/// Whether `ip` is internal (module docs): not `127.0.0.1` or `::1`, and an address of this
/// machine, of a network it is on, a link-local or cloud metadata one ([`refused_address`]),
/// or a private network's.
pub(crate) fn held(ip: IpAddr, own: Option<&[Network]>) -> bool {
    if ip == Ipv4Addr::LOCALHOST || ip == Ipv6Addr::LOCALHOST {
        return false;
    }
    let Some(own) = own else {
        return true;
    };
    let addresses: Vec<IpAddr> = own.iter().map(|one| one.address).collect();
    refused_address(ip, &addresses) || private_address(ip) || own.iter().any(|one| one.holds(ip))
}

#[cfg(test)]
mod tests;
