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
//! 2026-10-18), by the name asked for, by that address or by a range it is in ([`Entry`],
//! #2530). An IPv6 address a NAT64 prefix carries an IPv4 one in, this machine's own among them
//! ([`super::nat64`]), is judged as that one, and a Teredo or ISATAP address as each internal
//! IPv4 address it carries and as itself (#2678). That is how a cluster's administrator
//! lets a runner's relayed chats reach an internal service. A cloud metadata service is never
//! one.
//!
//! The same rule is held at plan time, for what the plan lists and what its names resolve to,
//! and at every connect ([`super::egress`]), since a name can resolve to another address later.
//! The loopback is the services the runner gives the chat ([`super::plan::Here::given`]).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};

use super::hosts::{Host, Network, carried_v4, metadata_address, refused_as_itself, tunnelled_v4};
use super::nat64::{self, Nat64};

mod entry;
pub use entry::{Entry, Range};

/// **Whether `ip` is an address of a private network, a cluster's or a link's**: `10/8`,
/// `172.16/12`, `192.168/16`, the shared `100.64/10` many clusters give their pods and services,
/// IPv4 and IPv6 link-local, and IPv6's unique-local `fc00::/7` and site-local `fec0::/10`. An
/// IPv6 address that carries an IPv4 one is judged as that one, and a Teredo or ISATAP address
/// as itself and as each IPv4 address it carries ([`tunnelled_v4`], #2678).
pub fn private_address(ip: IpAddr) -> bool {
    private_as_itself(ip) || tunnelled(ip).into_iter().any(private_v4)
}

/// [`private_address`], with a Teredo or ISATAP address judged as itself alone.
fn private_as_itself(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => private_v4(ip),
        IpAddr::V6(ip) => match carried_v4(ip).as_slice() {
            [] => {
                let first = ip.segments()[0];
                ip.is_unicast_link_local() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfec0
            }
            carried => carried.iter().any(|one| private_v4(*one)),
        },
    }
}

/// The IPv4 addresses `ip` carries as a Teredo or ISATAP address: none for any other.
fn tunnelled(ip: IpAddr) -> Vec<Ipv4Addr> {
    match ip {
        IpAddr::V4(_) => Vec::new(),
        IpAddr::V6(ip) => tunnelled_v4(ip),
    }
}

fn private_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_private() || ip.is_link_local() || (a == 100 && b & 0xc0 == 64)
}

/// **The internal services this machine's policy lists** (module docs): what a chat planned on
/// another machine reaches past the public Internet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Internal {
    listed: Source,
    /// The NAT64 prefixes an address is decoded through besides the well-known ones: `None` for
    /// this machine's own, read from it ([`nat64::of_this_machine`]).
    nat64: Option<Vec<Nat64>>,
}

/// Where an [`Internal`] reads what it lists.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// A fixed list.
    Listed(Vec<Entry>),
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
    /// The services `listed` names, with no NAT64 prefix of this machine's.
    pub fn listing(listed: Vec<Entry>) -> Self {
        Self {
            listed: Source::Listed(listed),
            nat64: Some(Vec::new()),
        }
    }

    /// What this machine's policy lists for the project at `root` (ruled 2026-10-18): its
    /// `relayed-internal`, read again at every question; none where it lists none. An address
    /// this machine's own NAT64 carries is judged as the IPv4 one it carries (#2530).
    pub fn of(root: &Path) -> Self {
        Self {
            listed: Source::Policy(root.to_path_buf()),
            nat64: None,
        }
    }

    /// These services, with `prefixes` as this machine's NAT64 prefixes.
    pub fn with_nat64(self, prefixes: Vec<Nat64>) -> Self {
        Self {
            nat64: Some(prefixes),
            ..self
        }
    }

    /// What it lists now.
    fn listed(&self) -> Vec<Entry> {
        match &self.listed {
            Source::Listed(listed) => listed.clone(),
            Source::Policy(root) => super::policy::Locks::of(root).relayed_internal().to_vec(),
        }
    }

    /// **The entries of this machine's policy it does not take**, each in a sentence naming it
    /// (#2530): none for a fixed list.
    pub fn not_taken(&self) -> Vec<String> {
        match &self.listed {
            Source::Listed(_) => Vec::new(),
            Source::Policy(root) => super::policy::Locks::of(root)
                .relayed_internal_refused()
                .to_vec(),
        }
    }

    /// The IPv4 addresses `ip` carries through this machine's own NAT64 prefixes.
    fn through_nat64(&self, ip: IpAddr) -> Vec<IpAddr> {
        let IpAddr::V6(ip) = ip else {
            return Vec::new();
        };
        let prefixes = self.nat64.clone().unwrap_or_else(nat64::of_this_machine);
        prefixes
            .iter()
            .filter_map(|prefix| prefix.carried(ip))
            .map(IpAddr::V4)
            .collect()
    }

    /// **Whether `ip` is internal** ([`held`]), as itself or as an IPv4 address this machine's
    /// own NAT64 carries in it.
    pub(crate) fn holds(&self, ip: IpAddr, own: Option<&[Network]>) -> bool {
        held(ip, own) || self.through_nat64(ip).into_iter().any(|one| held(one, own))
    }

    /// Whether `ip` is a cloud metadata service's, as itself or carried through this machine's
    /// own NAT64.
    pub(crate) fn metadata(&self, ip: IpAddr) -> bool {
        metadata_address(ip) || self.through_nat64(ip).into_iter().any(metadata_address)
    }

    /// **Whether `host`, reached at `at`, is carried**: an address that is not internal is, and
    /// an internal one only where this machine's policy lists it ([`Self::covers`]). A cloud
    /// metadata service is never carried. `own` is this machine's networks; where they could
    /// not be read, every address but `127.0.0.1` and `::1` is held as internal, and as one of
    /// this machine's own.
    pub fn carries(&self, host: &str, at: SocketAddr, own: Option<&[Network]>) -> bool {
        if !self.holds(at.ip(), own) {
            return true;
        }
        if self.metadata(at.ip()) {
            return false;
        }
        let name = Host::parse(&super::egress::host_and_port(host, at.port())).ok();
        self.covers(name.as_ref(), at.ip(), Some(at.port()), own)
    }

    /// **Whether this machine's policy lists `entry`**, a host as a plan lists one, with `own`
    /// this machine's networks: a name as [`Entry::covers`] says, and an address as
    /// [`Self::covers`] says; never one that is or carries a cloud metadata service's address.
    pub fn lists_entry(&self, entry: &str, own: Option<&[Network]>) -> bool {
        let Ok(asked) = Host::parse(entry) else {
            return false;
        };
        match asked.address() {
            Some(ip) => !self.metadata(ip) && self.covers(None, ip, asked.port(), own),
            None => self.listed().iter().any(|one| one.covers(&asked)),
        }
    }

    /// **Whether the listing covers `ip` on `port`, asked for by `name`** (#2530). An address
    /// under this machine's own NAT64 prefix is judged as each IPv4 address it carries, and
    /// every one of them must be covered. A Teredo or ISATAP address is judged as each internal
    /// IPv4 address it carries, and as itself where it is internal as itself (#2678). An address of this machine's own, or one where its
    /// addresses could not be read, is covered only by an entry naming that address and that
    /// port exactly, never by a range, a name or an address without a port. Any other address
    /// is covered by an entry that covers it, or the name asked for.
    fn covers(
        &self,
        name: Option<&Host>,
        ip: IpAddr,
        port: Option<u16>,
        own: Option<&[Network]>,
    ) -> bool {
        let listed = self.listed();
        if self.is_own(ip, own) && !listed.iter().any(|entry| entry.names_exactly(ip, port)) {
            return false;
        }
        let readings = self.through_nat64(ip);
        let tunnelled = tunnelled(ip);
        let judged = if !readings.is_empty() {
            readings
        } else if !tunnelled.is_empty() {
            // A Teredo or ISATAP address: itself where it is internal as itself, and each
            // internal IPv4 address it carries, as that address (#2678).
            held_as_itself(ip, own)
                .then_some(ip)
                .into_iter()
                .chain(
                    tunnelled
                        .into_iter()
                        .map(IpAddr::V4)
                        .filter(|one| held(*one, own)),
                )
                .collect()
        } else {
            vec![ip]
        };
        judged.iter().all(|one| {
            if self.is_own(*one, own) {
                return listed.iter().any(|entry| entry.names_exactly(*one, port));
            }
            let by_address = host_at(*one, port)
                .is_some_and(|asked| listed.iter().any(|entry| entry.covers(&asked)));
            by_address || name.is_some_and(|asked| listed.iter().any(|entry| entry.covers(asked)))
        })
    }

    /// **Whether `ip` is, or carries, one of this machine's own addresses**: every address,
    /// where they could not be read (`own` `None`).
    pub(crate) fn is_own(&self, ip: IpAddr, own: Option<&[Network]>) -> bool {
        let Some(own) = own else {
            return true;
        };
        let mine = |ip: IpAddr| own.iter().any(|one| one.address == ip);
        let carried = match ip {
            IpAddr::V4(_) => Vec::new(),
            IpAddr::V6(v6) => carried_v4(v6)
                .into_iter()
                .chain(tunnelled_v4(v6))
                .map(IpAddr::V4)
                .chain(self.through_nat64(ip))
                .collect(),
        };
        mine(ip) || carried.into_iter().any(mine)
    }
}

/// `ip` as a host, on `port` where there is one; none where it is never a host.
fn host_at(ip: IpAddr, port: Option<u16>) -> Option<Host> {
    let written = match (ip, port) {
        (IpAddr::V4(ip), None) => ip.to_string(),
        (IpAddr::V6(ip), None) => format!("[{ip}]"),
        (ip, Some(port)) => SocketAddr::new(ip, port).to_string(),
    };
    Host::parse(&written).ok()
}

/// Whether `ip` is internal (module docs): not `127.0.0.1` or `::1`, and an address of this
/// machine, of a network it is on, a link-local or cloud metadata one
/// ([`super::hosts::refused_address`]), or a private network's. A Teredo or ISATAP address is
/// held where it is as itself or as an IPv4 address it carries (#2678).
pub(crate) fn held(ip: IpAddr, own: Option<&[Network]>) -> bool {
    held_as_itself(ip, own)
        || tunnelled(ip)
            .into_iter()
            .any(|one| held_as_itself(IpAddr::V4(one), own))
}

/// [`held`], with a Teredo or ISATAP address judged as itself alone.
fn held_as_itself(ip: IpAddr, own: Option<&[Network]>) -> bool {
    if ip == Ipv4Addr::LOCALHOST || ip == Ipv6Addr::LOCALHOST {
        return false;
    }
    let Some(own) = own else {
        return true;
    };
    let addresses: Vec<IpAddr> = own.iter().map(|one| one.address).collect();
    refused_as_itself(ip, &addresses)
        || private_as_itself(ip)
        || own.iter().any(|one| one.holds(ip))
}

#[cfg(test)]
mod tests;
