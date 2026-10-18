// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One entry of `sandbox.relayed-internal`** (#2530): a host, written as `hosts` is, or a
//! range of addresses, `10.96.0.0/12` or `[fd00:10:96::/112]`, each with an optional `:port`.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::super::hosts::{Host, port_of};

/// One internal service, or range of them, a relayed chat may reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// A host, as `hosts` lists one.
    Host(Host),
    /// Every address of a range, on one port where it names one.
    Range(Range),
}

/// A range of addresses: its network, the length of its prefix, and the one port it is held to,
/// where it has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    network: IpAddr,
    prefix: u8,
    port: Option<u16>,
}

/// What a range is written like, said where one is refused.
const LOOKS_LIKE: &str = "a range is an address and the length of its prefix, such as \
                          10.96.0.0/12 or [fd00:10:96::/112], with :port where it is one port";

impl Entry {
    /// `typed` as an entry, or the one sentence saying why it is not one.
    pub fn parse(typed: &str) -> Result<Self, String> {
        let typed = typed.trim();
        if typed.contains('/') && !typed.contains("://") {
            return Range::parse(typed).map(Self::Range);
        }
        Host::parse(typed).map(Self::Host)
    }

    /// **Whether this entry lets a relayed chat reach `asked`**: a host as [`Host::covers`]
    /// says, and a range every address in it, on its port where it names one, and never a name.
    pub fn covers(&self, asked: &Host) -> bool {
        match self {
            Self::Host(host) => host.covers(asked),
            Self::Range(range) => {
                asked.address().is_some_and(|ip| range.holds(ip))
                    && range.port.is_none_or(|port| asked.port() == Some(port))
            }
        }
    }
}

impl Entry {
    /// Whether this entry is the host `ip` on `port` exactly: an address, with that port.
    pub fn names_exactly(&self, ip: IpAddr, port: Option<u16>) -> bool {
        match self {
            Self::Host(host) => host.address() == Some(ip) && port.is_some() && host.port() == port,
            Self::Range(_) => false,
        }
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host(host) => write!(f, "{host}"),
            Self::Range(range) => write!(f, "{range}"),
        }
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.network {
            IpAddr::V4(ip) => write!(f, "{ip}/{}", self.prefix)?,
            IpAddr::V6(ip) => write!(f, "[{ip}/{}]", self.prefix)?,
        }
        match self.port {
            Some(port) => write!(f, ":{port}"),
            None => Ok(()),
        }
    }
}

impl Range {
    fn parse(typed: &str) -> Result<Self, String> {
        let lowered = typed.to_ascii_lowercase();
        let (inside, port) = match lowered.strip_prefix('[') {
            Some(rest) => {
                let (inside, after) = rest.split_once(']').ok_or_else(|| {
                    format!("{typed} opens a bracket and never closes it: {LOOKS_LIKE}.")
                })?;
                let port = match after {
                    "" => None,
                    _ => Some(
                        after
                            .strip_prefix(':')
                            .ok_or_else(|| format!("{typed} is not a range: {LOOKS_LIKE}."))?,
                    ),
                };
                (inside.to_owned(), port.map(str::to_owned))
            }
            None => match lowered.split_once('/') {
                Some((address, rest)) if address.parse::<Ipv4Addr>().is_ok() => {
                    match rest.split_once(':') {
                        Some((length, port)) => {
                            (format!("{address}/{length}"), Some(port.to_owned()))
                        }
                        None => (lowered.clone(), None),
                    }
                }
                _ => (lowered.clone(), None),
            },
        };
        let port = port.as_deref().map(port_of).transpose()?;
        let (address, length) = inside
            .split_once('/')
            .ok_or_else(|| format!("{typed} is not a range: {LOOKS_LIKE}."))?;
        let network: IpAddr = address
            .parse()
            .map_err(|_| format!("{typed} is not a range: {LOOKS_LIKE}."))?;
        let whole: u8 = if network.is_ipv4() { 32 } else { 128 };
        let prefix = length
            .parse::<u8>()
            .ok()
            .filter(|prefix| {
                !length.is_empty() && length.bytes().all(|b| b.is_ascii_digit()) && *prefix <= whole
            })
            .ok_or_else(|| {
                format!(
                    "{typed} does not end in the length of a prefix: a number from 0 to {whole} \
                     after the /."
                )
            })?;
        let range = Self {
            network,
            prefix,
            port,
        };
        let start = range.start();
        if start != network {
            return Err(format!(
                "{typed} has bits set past its prefix. The range is written {}.",
                Self {
                    network: start,
                    ..range
                }
            ));
        }
        if let Some(why) = range.takes_in_what_no_host_is() {
            return Err(format!(
                "{range} takes in {why}, which a relayed chat never reaches. List a range \
                 without it."
            ));
        }
        Ok(range)
    }

    /// Whether `ip` is in this range.
    pub fn holds(&self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(_), IpAddr::V4(_)) | (IpAddr::V6(_), IpAddr::V6(_)) => {
                masked(ip, self.prefix) == self.network
            }
            _ => false,
        }
    }

    /// The first address of the range: its network with every bit past the prefix cleared.
    fn start(&self) -> IpAddr {
        masked(self.network, self.prefix)
    }

    /// What no host may be that this range takes in, said, where it takes in any.
    fn takes_in_what_no_host_is(&self) -> Option<&'static str> {
        let overlaps = |network: IpAddr, prefix: u8| {
            network.is_ipv4() == self.network.is_ipv4() && {
                let shorter = prefix.min(self.prefix);
                masked(network, shorter) == masked(self.network, shorter)
            }
        };
        let v4 = |a, b, c, d| IpAddr::V4(Ipv4Addr::new(a, b, c, d));
        let v6 = |segments: [u16; 8]| IpAddr::V6(Ipv6Addr::from(segments));
        let never: [(IpAddr, u8, &str); 16] = [
            (v4(0, 0, 0, 0), 8, "this machine"),
            (v4(127, 0, 0, 0), 8, "this machine"),
            (v4(169, 254, 0, 0), 16, "link-local addresses"),
            (v4(224, 0, 0, 0), 4, "multicast addresses"),
            (v4(255, 255, 255, 255), 32, "the broadcast address"),
            (v4(100, 100, 100, 200), 32, "a cloud metadata address"),
            (v4(168, 63, 129, 16), 32, "a cloud metadata address"),
            (v4(192, 0, 0, 192), 32, "a cloud metadata address"),
            (
                v6([0; 8]),
                96,
                "IPv6 forms of IPv4 addresses and this machine",
            ),
            (
                v6([0, 0, 0, 0, 0xffff, 0, 0, 0]),
                96,
                "IPv6 forms of IPv4 addresses",
            ),
            (
                v6([0, 0, 0, 0, 0, 0xffff, 0, 0]),
                96,
                "IPv6 forms of IPv4 addresses",
            ),
            (
                v6([0x64, 0xff9b, 0, 0, 0, 0, 0, 0]),
                96,
                "NAT64 addresses, which carry IPv4 ones",
            ),
            (
                v6([0x64, 0xff9b, 1, 0, 0, 0, 0, 0]),
                48,
                "NAT64 addresses, which carry IPv4 ones",
            ),
            (
                v6([0x2002, 0, 0, 0, 0, 0, 0, 0]),
                16,
                "6to4 addresses, which carry IPv4 ones",
            ),
            (
                v6([0xfe80, 0, 0, 0, 0, 0, 0, 0]),
                10,
                "link-local addresses",
            ),
            (v6([0xff00, 0, 0, 0, 0, 0, 0, 0]), 8, "multicast addresses"),
        ];
        never
            .iter()
            .find(|(network, prefix, _)| overlaps(*network, *prefix))
            .map(|(_, _, why)| *why)
            .or_else(|| {
                super::super::hosts::metadata_v6_ranges()
                    .iter()
                    .any(|(network, prefix)| overlaps(IpAddr::V6(*network), *prefix))
                    .then_some("a cloud metadata address")
            })
    }
}

/// `ip` with every bit past the first `prefix` cleared.
fn masked(ip: IpAddr, prefix: u8) -> IpAddr {
    match ip {
        IpAddr::V4(ip) => {
            let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
            IpAddr::V4(Ipv4Addr::from(u32::from(ip) & mask))
        }
        IpAddr::V6(ip) => {
            let mask = u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0);
            IpAddr::V6(Ipv6Addr::from(u128::from(ip) & mask))
        }
    }
}
