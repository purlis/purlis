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
//!    connect, and so is an address literal that belongs to one, in any spelling `getaddrinfo`
//!    reads (decimal, hex, octal, IPv4-mapped, NAT64) ([`ForgeDenial::carries`]). A
//!    [`TUNNELLED`](crate::forgewire::TUNNELLED) name is not asked, since GitHub serves it from the
//!    same ranges. GitHub's published ranges for its web, api and git services are a vendored
//!    snapshot ([`SNAPSHOT`], refreshed by `tools/github-ranges-refresh.mjs`, never fetched from
//!    a chat's path); every forge host is also resolved, and what it answers is held for
//!    [`TTL`] (a failed lookup is not held: it is asked again).
//! 3. **The name inside the tunnel** ([`ForgeDenial::pins`]): GitHub serves `github.com` from
//!    many addresses no list holds (Azure `/32`s and `/28`s that come and go), so a tunnel is
//!    carried only where the SNI of its TLS ClientHello is no forge name, or is the exempt name
//!    the tunnel was asked for. A ClientHello that cannot be read is not carried.
//! 4. **Port 22** to a forge name or address, a tunnelled name included: no ssh to a forge leaves
//!    the machine but through purlis's own ssh route.
//!
//! **Residual** (named in ADR 0128 and the pull request): a chat with the network family off
//! is not served by this proxy at all; a forge that publishes no ranges is held only by name, by
//! SNI and by what its host resolves to; a TLS handshake that hides its name (ECH) to an address
//! no list holds is carried; a server-first protocol through a tunnel is not carried while a
//! forge is denied, as its first bytes cannot be read.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::egress::allows;

/// How a name is looked up for a forge's addresses.
pub type Resolver = Arc<dyn Fn(&str) -> Vec<IpAddr> + Send + Sync + 'static>;

/// How long what a forge host resolved to is held before it is asked again.
pub const TTL: Duration = Duration::from_secs(300);

/// The port of ssh.
const SSH: u16 = 22;

/// GitHub's published web, api and git ranges (`api.github.com/meta`), one a line, less its
/// Pages, Actions and other lists, which carry other people's sites. Vendored, and refreshed by
/// `node tools/github-ranges-refresh.mjs`; resolution is held beside it.
pub const SNAPSHOT: &str = include_str!("forgedeny/github-ranges.txt");

/// What a tunnel's first bytes said of the server it is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hello {
    /// A whole ClientHello naming one server.
    Named(String),
    /// A whole ClientHello that names none.
    NoName,
    /// A ClientHello that carries an encrypted one (ECH): its real name is not readable.
    Ech,
    /// TLS that is no whole ClientHello in its first record, or is none that can be read.
    Unreadable,
    /// Bytes that do not start a TLS handshake.
    NotTls,
}

/// The snapshot's ranges, read once.
fn snapshot() -> &'static [Range] {
    static RANGES: OnceLock<Vec<Range>> = OnceLock::new();
    RANGES.get_or_init(|| {
        SNAPSHOT
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(Range::parse)
            .collect()
    })
}

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
        let ip = canonical(ip);
        if ip.is_ipv6() != self.v6 {
            return false;
        }
        let width = if self.v6 { 128 } else { 32 };
        Self::number(ip) & Self::mask(width, self.bits) == self.net
    }
}

/// An address as the one it stands for: IPv4-mapped and NAT64 (`64:ff9b::/96`) literals are the
/// IPv4 address they carry.
fn canonical(ip: IpAddr) -> IpAddr {
    let ip = ip.to_canonical();
    if let IpAddr::V6(six) = ip {
        let octets = six.octets();
        if octets[..4] == [0, 0x64, 0xff, 0x9b] && octets[4..12].iter().all(|b| *b == 0) {
            return IpAddr::V4(Ipv4Addr::new(
                octets[12], octets[13], octets[14], octets[15],
            ));
        }
    }
    ip
}

/// An IPv4 address as `inet_aton` reads it, which is how `getaddrinfo` does: one to four parts,
/// each decimal, hex (`0x`) or octal (`0`), the last filling the bytes that are left.
fn numeric_v4(host: &str) -> Option<Ipv4Addr> {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() > 4 {
        return None;
    }
    let mut numbers = Vec::new();
    for part in &parts {
        let (digits, radix) =
            if let Some(hex) = part.strip_prefix("0x").or_else(|| part.strip_prefix("0X")) {
                (hex, 16)
            } else if part.len() > 1 && part.starts_with('0') {
                (&part[1..], 8)
            } else {
                (*part, 10)
            };
        if digits.is_empty() && radix != 16 {
            return None;
        }
        // `0x` alone is zero to `inet_aton`.
        let number = if digits.is_empty() {
            0
        } else {
            u64::from_str_radix(digits, radix).ok()?
        };
        numbers.push(number);
    }
    let (last, init) = numbers.split_last()?;
    if init.iter().any(|n| *n > 255) {
        return None;
    }
    let room = 8 * (4 - init.len() as u32);
    if *last >= 1u64 << room {
        return None;
    }
    let mut value = *last;
    for (at, n) in init.iter().enumerate() {
        value |= n << (8 * (3 - at as u32));
    }
    Some(Ipv4Addr::from(u32::try_from(value).ok()?))
}

/// The address a host is, where it is a literal in any of those spellings.
fn literal(host: &str) -> Option<IpAddr> {
    let host = plain(host);
    host.parse::<IpAddr>()
        .ok()
        .or_else(|| numeric_v4(&host).map(IpAddr::V4))
        .map(canonical)
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
    /// What each forge host resolved to, and when. A host whose lookup answered nothing has no
    /// entry, so it is asked again at the next question.
    resolved: Mutex<HashMap<String, (Instant, HashSet<IpAddr>)>>,
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
            ranges: snapshot().to_vec(),
            resolve,
            resolved: Mutex::new(HashMap::new()),
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

    /// What `base` resolves to: held for [`TTL`] where it answered, asked again where it did not.
    fn addresses_of(&self, base: &str) -> HashSet<IpAddr> {
        {
            let seen = self
                .resolved
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some((at, addresses)) = seen.get(base)
                && at.elapsed() < TTL
            {
                return addresses.clone();
            }
        }
        // Looked up outside the lock: a slow answer holds only this call.
        let found: HashSet<IpAddr> = (self.resolve)(base).into_iter().map(canonical).collect();
        if !found.is_empty() {
            self.resolved
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(base.to_owned(), (Instant::now(), found.clone()));
        }
        found
    }

    /// Whether the forge is at `ip`: in a snapshotted range, or among what a forge host resolved
    /// to in the last [`TTL`].
    fn holds(&self, ip: IpAddr) -> bool {
        let ip = canonical(ip);
        if self.ranges.iter().any(|range| range.holds(ip)) {
            return true;
        }
        self.bases
            .iter()
            .filter(|base| literal(base).is_none() && !NOT_ADDRESSED.contains(&base.as_str()))
            .any(|base| self.addresses_of(base).contains(&ip))
    }

    /// **Whether a tunnel asked for `host`, whose first bytes were `hello`, is refused**
    /// (ADR 0128 D3g, round 3). An exempt name is carried only where the SNI is exactly it: GitHub
    /// serves every name from the same addresses, so an exempt name's connection could say
    /// `github.com` inside. Any other name is refused where the SNI names a forge host or a name
    /// under one, and wherever the first record cannot be read as a whole ClientHello, since a
    /// hello split across records hides its name. A hello that names nothing, or hides it (ECH),
    /// is carried to an address no list holds, as [`Self::carries`] has already refused every
    /// address a list does hold; and bytes that start no TLS handshake are carried, to anything
    /// but an exempt name.
    pub fn pins(&self, host: &str, hello: &Hello) -> bool {
        let host = plain(host);
        let exempt = self.tunnels(&host, None);
        match hello {
            Hello::Named(sni) => {
                let sni = plain(sni);
                if exempt {
                    sni != host
                } else {
                    self.is_forge_name(&sni) || literal(&sni).is_some_and(|ip| self.holds(ip))
                }
            }
            Hello::NoName | Hello::Ech | Hello::NotTls => exempt,
            Hello::Unreadable => true,
        }
    }

    /// **Whether a connection to `host` on `port` is refused before any address is looked up**:
    /// port 22 to a forge name or a tunnelled one; a forge name that is not tunnelled on this
    /// port; an address literal the forge holds.
    pub fn refuses(&self, host: &str, port: u16) -> bool {
        let host = plain(host);
        if let Some(ip) = literal(&host) {
            // A listed address literal is a forge host as any name is; any other is one only if
            // the forge is at it.
            return self.bases.iter().any(|base| literal(base) == Some(ip)) || self.holds(ip);
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
