// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat's egress proxy refuses of a forge** (ADR 0128 D3g, round 2 of #7292).
//!
//! The only route to a forge is the proxy's forge route, which terminates the connection itself
//! and puts the person's token on it. A chat's own `gh`, git or `curl`, pointed or told anything,
//! must meet a refusal here. A name alone is not enough, so three things are held:
//!
//! 1. **Every name under a forge host**, not only the host itself: `*.github.com`,
//!    `*.githubusercontent.com`, `*.gitlab.com` and `*.<a recorded GHES or GitLab host>`, less
//!    [`crate::forgewire::TUNNELLED`], which carry downloads and never a sign-in.
//! 2. **The addresses of the forge**: an allowed name that *resolves* to one is refused at
//!    connect, and so is an address literal that belongs to one ([`ForgeDenial::carries`]). A
//!    [`TUNNELLED`](crate::forgewire::TUNNELLED) name is not asked, since GitHub serves it from the
//!    same ranges. GitHub's published ranges for its web, api and git services are held as a
//!    snapshot ([`GITHUB_RANGES`]); every forge host is also resolved, and what it answers is held
//!    for [`TTL`], so a range GitHub adds is held as soon as a name resolves into it.
//! 3. **Port 22** to a forge name or address, a tunnelled name included: no ssh to a forge leaves
//!    the machine but through purlis's own ssh route.
//!
//! **Residual** (named in ADR 0128 and the pull request): a forge reached at an address neither
//! snapshotted nor resolved here (a forge behind a CDN address shared with other sites is
//! held at the shared address, so those sites are refused too); and a recorded host that is
//! never looked up while it is down.

use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::egress::allows;

/// How a name is looked up for a forge's addresses.
pub type Resolver = Arc<dyn Fn(&str) -> Vec<IpAddr> + Send + Sync + 'static>;

/// How long what a forge host resolved to is held before it is asked again.
pub const TTL: Duration = Duration::from_secs(300);

/// The port of ssh.
const SSH: u16 = 22;

/// A snapshot of GitHub's published web, api and git ranges (`api.github.com/meta`), less its
/// Pages and Actions ranges, which carry other people's sites. Held beside resolution, which is
/// what keeps it current.
pub const GITHUB_RANGES: [&str; 4] = [
    "192.30.252.0/22",
    "140.82.112.0/20",
    "143.55.64.0/20",
    "2a0a:a440::/29",
];

/// Forge hosts never looked up for their addresses: a domain that serves other people's content
/// from addresses shared with sites the chat may reach, and takes no sign-in.
const NOT_ADDRESSED: [&str; 1] = ["githubusercontent.com"];

/// One range of addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Range {
    net: u128,
    bits: u32,
    v6: bool,
}

impl Range {
    fn parse(text: &str) -> Option<Self> {
        let (net, len) = text.split_once('/')?;
        let ip: IpAddr = net.parse().ok()?;
        let len: u32 = len.parse().ok()?;
        let (v6, width) = match ip {
            IpAddr::V4(_) => (false, 32),
            IpAddr::V6(_) => (true, 128),
        };
        (len <= width).then(|| Self {
            net: Self::number(ip) & Self::mask(width, len),
            bits: len,
            v6,
        })
    }

    fn number(ip: IpAddr) -> u128 {
        match ip {
            IpAddr::V4(ip) => u128::from(u32::from(ip)),
            IpAddr::V6(ip) => u128::from(ip),
        }
    }

    fn mask(width: u32, len: u32) -> u128 {
        if len == 0 {
            0
        } else {
            (!0u128 >> (128 - width)) & !((1u128 << (width - len)) - 1)
        }
    }

    fn holds(&self, ip: IpAddr) -> bool {
        let ip = ip.to_canonical();
        if ip.is_ipv6() != self.v6 {
            return false;
        }
        let width = if self.v6 { 128 } else { 32 };
        Self::number(ip) & Self::mask(width, self.bits) == self.net
    }
}

/// The name a connection is asked for, as a host is listed: lower case, no final dot, and an IPv6
/// literal without its brackets.
fn plain(host: &str) -> String {
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .strip_suffix('.')
        .unwrap_or(host.trim_start_matches('[').trim_end_matches(']'))
        .to_ascii_lowercase()
}

/// **A chat's forge denial**: built from the forge hosts its proxy denies
/// ([`crate::forgewire::denied_hosts`]).
pub struct ForgeDenial {
    /// The forge hosts, each standing for every name under it.
    bases: Vec<String>,
    /// [`crate::forgewire::TUNNELLED`]: a name, and the one port it is held to where it has one
    /// (none today).
    tunnelled: Vec<(String, Option<u16>)>,
    ranges: Vec<Range>,
    resolve: Resolver,
    /// What the forge hosts resolved to, and when.
    resolved: Mutex<Option<(Instant, HashSet<IpAddr>)>>,
}

impl ForgeDenial {
    /// The denial for `bases`, looking names up by the system's resolver.
    pub fn of(bases: &[String]) -> Self {
        Self::resolving(bases, Arc::new(system))
    }

    /// The denial for `bases`, looking names up by `resolve`.
    pub fn resolving(bases: &[String], resolve: Resolver) -> Self {
        Self {
            bases: bases.iter().map(|base| plain(base)).collect(),
            tunnelled: crate::forgewire::TUNNELLED
                .iter()
                .map(|entry| {
                    let (name, port) = super::egress::entry_parts(entry);
                    (name.to_ascii_lowercase(), port)
                })
                .collect(),
            ranges: GITHUB_RANGES
                .iter()
                .filter_map(|r| Range::parse(r))
                .collect(),
            resolve,
            resolved: Mutex::new(None),
        }
    }

    /// Whether `host` is a forge host, or a name under one.
    fn is_forge_name(&self, host: &str) -> bool {
        self.bases
            .iter()
            .any(|base| host == base || allows(&[format!("*.{base}")], host))
    }

    /// Whether `host` is one of the tunnelled names, on `port` where it is held to one.
    fn tunnels(&self, host: &str, port: Option<u16>) -> bool {
        self.tunnelled
            .iter()
            .any(|(name, held)| name == host && (port.is_none() || held.is_none() || *held == port))
    }

    /// Whether the forge is at `ip`: in a snapshotted range, or among what a forge host resolved
    /// to in the last [`TTL`].
    fn holds(&self, ip: IpAddr) -> bool {
        let ip = ip.to_canonical();
        if self.ranges.iter().any(|range| range.holds(ip)) {
            return true;
        }
        let fresh = |at: &Instant| at.elapsed() < TTL;
        {
            let seen = self
                .resolved
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some((at, addresses)) = &*seen
                && fresh(at)
            {
                return addresses.contains(&ip);
            }
        }
        // Looked up outside the lock, once for every host: a slow answer holds only this call.
        let found: HashSet<IpAddr> = self
            .bases
            .iter()
            .filter(|base| {
                base.parse::<IpAddr>().is_err() && !NOT_ADDRESSED.contains(&base.as_str())
            })
            .flat_map(|base| (self.resolve)(base))
            .map(|at| at.to_canonical())
            .collect();
        let held = found.contains(&ip);
        *self
            .resolved
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((Instant::now(), found));
        held
    }

    /// **Whether a connection to `host` on `port` is refused before any address is looked up**:
    /// port 22 to a forge name or a tunnelled one; a forge name that is not tunnelled on this
    /// port; an address literal the forge holds.
    pub fn refuses(&self, host: &str, port: u16) -> bool {
        let host = plain(host);
        if host.parse::<IpAddr>().is_ok() {
            // A listed address literal is a forge host as any name is; any other is one only if
            // the forge is at it.
            let ip: IpAddr = host.parse().expect("parsed above");
            return self.bases.contains(&host) || self.holds(ip);
        }
        let named = self.is_forge_name(&host) || self.tunnels(&host, None);
        if !named {
            return false;
        }
        port == SSH || !self.tunnels(&host, Some(port)) && self.is_forge_name(&host)
    }

    /// **Whether a connection for `host` may be made at `at`**: not at an address the forge holds,
    /// unless `host` is a tunnelled name, which GitHub serves from the same ranges.
    pub fn carries(&self, host: &str, at: IpAddr) -> bool {
        if self.tunnels(&plain(host), None) {
            return true;
        }
        !self.holds(at)
    }
}

/// The system resolver's answer for `name`, none where it does not resolve.
fn system(name: &str) -> Vec<IpAddr> {
    use std::net::ToSocketAddrs;
    (name, 443)
        .to_socket_addrs()
        .map(|found| found.map(|at| at.ip()).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
