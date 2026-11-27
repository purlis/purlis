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
//!    [`crate::forgewire::TUNNELLED`], the download-only hosts. What holds those instead of a
//!    refusal: a chat carries no token for them (the forge route is the only place a token is put
//!    on a request), so a private archive on `codeload.github.com` answers a chat's own
//!    request as it answers any anonymous one.
//! 2. **The addresses of the forge**: an allowed name that *resolves* to one is refused at
//!    connect, and so is an address literal that belongs to one, in any spelling `getaddrinfo`
//!    reads (decimal, hex, octal, IPv4-mapped, NAT64) ([`ForgeDenial::carries`]). A
//!    [`TUNNELLED`](crate::forgewire::TUNNELLED) name is not asked, since GitHub serves it from the
//!    same ranges. GitHub's published ranges for its web, api and git services are a vendored
//!    snapshot ([`SNAPSHOT`], refreshed by `tools/github-ranges-refresh.mjs`, never fetched from
//!    a chat's path); every forge host, and the sibling names a forge serves beside it
//!    ([`SIBLINGS`]: `altssh.`, `ssh.`, `registry.`, `uploads.`, `gist.`, `raw.`, `www.`), is also
//!    resolved, and what it answers is held: each address for [`HELD`], the answers of one name
//!    added to what it answered before, asked again every [`TTL`]. A lookup that fails keeps what
//!    was known and is not asked again for [`NEGATIVE`].
//! 3. **The name inside the tunnel** ([`ForgeDenial::pins`]): GitHub serves `github.com` from
//!    many addresses no list holds (Azure `/32`s and `/28`s that come and go), so a tunnel is
//!    carried only where the SNI of its TLS ClientHello is no forge name, or is the exempt name
//!    the tunnel was asked for. A first record shaped like TLS (a content type of `0x14` to `0x18`,
//!    whatever its version) that is no readable ClientHello, an alert before the hello among them, is not carried.
//!    Bytes that are no TLS record at all are carried. A hello that hides its name (ECH) or has
//!    none is refused on a tunnel to an address literal, where nothing else names the server.
//! 4. **Port 22** to a forge name or address, a tunnelled name included: no ssh to a forge leaves
//!    the machine but through purlis's own ssh route.
//!
//! **Residual** (named in ADR 0128 and the pull request): a chat with the network family off
//! is not served by this proxy at all; a forge that publishes no ranges is held only by name, by
//! SNI and by what its host resolves to; the SNI pin does not hold a server that ignores SNI, and
//! a tunnel to a name no list holds that says another non-forge name inside is carried to an
//! address of an unheld forge, held by the address lists alone; a server-first protocol through
//! a tunnel to port 443 is not carried while a forge is denied, as its first bytes cannot be
//! read. **GitHub Pages and `ghcr.io` share GitHub's ranges** (`185.199.108.0/22` serves
//! `github.io` and custom Pages domains; `140.82.112.0/20` may serve `ghcr.io`): a connection to
//! one by name is refused at connect while a forge is denied, with a sentence that names the
//! forge's addresses (`Allowed::only_local`), and a chat reaches such a site through no proxy of
//! purlis's. That is the cost of holding the ranges, chosen over a carve-out by SNI, which would
//! carry any name said inside a tunnel to a forge address.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::egress::allows;

/// How a name is looked up for a forge's addresses.
pub type Resolver = Arc<dyn Fn(&str) -> Vec<IpAddr> + Send + Sync + 'static>;

/// How long what a forge host resolved to is held before it is asked again.
pub const TTL: Duration = Duration::from_secs(300);

/// How long an address a forge host answered is held after it last answered.
pub const HELD: Duration = Duration::from_secs(1200);

/// How long a name whose lookup answered nothing is left alone before it is asked again, so an
/// unreachable host adds no resolver wait to each connection.
pub const NEGATIVE: Duration = Duration::from_secs(10);

/// [`NEGATIVE`] for a sibling name ([`SIBLINGS`]): most forges answer for few of them, and a
/// name that does not exist is not worth a lookup on a connection's path every ten seconds.
pub const SIBLING_NEGATIVE: Duration = Duration::from_secs(600);

/// The names a forge serves beside its host, from addresses of their own at times (`altssh` is
/// not `gitlab.com`'s address): each is resolved for a forge host as the host is.
const SIBLINGS: [&str; 7] = ["altssh", "ssh", "registry", "uploads", "gist", "raw", "www"];

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
    /// A ClientHello that carries an encrypted one (ECH): its real name is not readable. The
    /// outer name is kept, since a server that does not speak ECH serves it.
    Ech(Option<String>),
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

/// The name a connection is asked for, as a host is listed: lower case, no final dots, and an IPv6
/// literal without its brackets.
fn plain(host: &str) -> String {
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
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
    /// What each forge name resolved to.
    resolved: Mutex<HashMap<String, Known>>,
    /// Held while names are looked up, so connections arriving together share one round of
    /// lookups and not one each.
    asking: Mutex<()>,
}

/// What one forge name has resolved to.
struct Known {
    /// Each address it answered, and when it stops being held.
    addresses: HashMap<IpAddr, Instant>,
    /// When it is asked again: [`TTL`] after an answer, [`NEGATIVE`] after none.
    ask_after: Instant,
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
            asking: Mutex::new(()),
        }
    }

    /// Whether `host` is a forge host, or a name under one.
    fn is_forge_name(&self, host: &str) -> bool {
        self.bases
            .iter()
            .any(|base| host == base || allows(&[format!("*.{base}")], host))
    }

    /// Whether `host` is one of Claude Code's own hosts ([`crate::forgewire::HARNESS_HOSTS`]),
    /// which a CDN shared with a forge serves: never held back for an address.
    fn is_harness_host(host: &str) -> bool {
        crate::forgewire::HARNESS_HOSTS.contains(&host)
    }

    /// Whether `host` is one of the tunnelled names, on `port` where it is held to one.
    fn tunnels(&self, host: &str, port: Option<u16>) -> bool {
        self.tunnelled
            .iter()
            .any(|(name, held)| name == host && (port.is_none() || held.is_none() || *held == port))
    }

    /// The names looked up for addresses: each forge host and its [`SIBLINGS`], less what is never
    /// addressed and what is an address already.
    fn addressed(&self) -> Vec<String> {
        let mut names = Vec::new();
        for base in &self.bases {
            if literal(base).is_some() || NOT_ADDRESSED.contains(&base.as_str()) {
                continue;
            }
            names.push(base.clone());
            // A host under another forge host has the siblings of that one.
            let under = self
                .bases
                .iter()
                .any(|other| other != base && base.ends_with(&format!(".{other}")));
            if !under {
                names.extend(SIBLINGS.iter().map(|sibling| format!("{sibling}.{base}")));
            }
        }
        names
    }

    /// Whether `name` is one of a forge host's [`SIBLINGS`].
    fn is_sibling(&self, name: &str) -> bool {
        name.split_once('.').is_some_and(|(first, rest)| {
            SIBLINGS.contains(&first) && self.bases.iter().any(|base| base == rest)
        })
    }

    /// Asks again every name that is due, side by side, and adds what each answers to what it
    /// answered before. A name that answers nothing keeps what it had, and is left alone for
    /// [`NEGATIVE`], or for [`SIBLING_NEGATIVE`] where it is one of a forge host's siblings.
    fn refresh(&self) {
        let due: Vec<String> = {
            let seen = self
                .resolved
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let now = Instant::now();
            self.addressed()
                .into_iter()
                .filter(|name| seen.get(name).is_none_or(|known| now >= known.ask_after))
                .collect()
        };
        if due.is_empty() {
            return;
        }
        // Looked up outside the lock: a slow answer holds only this call.
        let ask = |name: &String| -> HashSet<IpAddr> {
            (self.resolve)(name).into_iter().map(canonical).collect()
        };
        let answers: Vec<HashSet<IpAddr>> = if let [only] = due.as_slice() {
            vec![ask(only)]
        } else {
            std::thread::scope(|scope| {
                let asking: Vec<_> = due.iter().map(|name| scope.spawn(|| ask(name))).collect();
                asking
                    .into_iter()
                    .map(|one| one.join().unwrap_or_default())
                    .collect()
            })
        };
        let mut seen = self
            .resolved
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        for (name, found) in due.into_iter().zip(answers) {
            let sibling = self.is_sibling(&name);
            let known = seen.entry(name).or_insert_with(|| Known {
                addresses: HashMap::new(),
                ask_after: now,
            });
            known.addresses.retain(|_, until| *until > now);
            if found.is_empty() {
                known.ask_after = now + if sibling { SIBLING_NEGATIVE } else { NEGATIVE };
            } else {
                known
                    .addresses
                    .extend(found.into_iter().map(|at| (at, now + HELD)));
                known.ask_after = now + TTL;
            }
        }
    }

    /// Whether the forge is at `ip`: in a snapshotted range, or among what a forge host or one of
    /// its siblings resolved to and is still held.
    fn holds(&self, ip: IpAddr) -> bool {
        let ip = canonical(ip);
        if self.ranges.iter().any(|range| range.holds(ip)) {
            return true;
        }
        // One round of lookups at a time, and always waited for: a connection that found another
        // asking and answered from what is known could answer from an empty set (every entry
        // expired after an idle chat, or a failed round), and let a connection through to a forge
        // address. Once the round is over this call's refresh finds nothing due. No `resolved`
        // guard is held here, and `refresh` takes `resolved` only under `asking`.
        let lookup = self
            .asking
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.refresh();
        drop(lookup);
        let now = Instant::now();
        self.resolved
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .any(|known| known.addresses.get(&ip).is_some_and(|until| *until > now))
    }

    /// **Whether a tunnel asked for `host`, whose first bytes were `hello`, is refused**
    /// (ADR 0128 D3g, round 3). An exempt name is carried only where the SNI is exactly it: GitHub
    /// serves every name from the same addresses, so an exempt name's connection could say
    /// `github.com` inside. Any other name is refused where the SNI names a forge host or a name
    /// under one, and wherever the first record cannot be read as a whole ClientHello, since a
    /// hello split across records hides its name. A hello that names nothing, or hides it (ECH),
    /// is carried to an address no list holds, as [`Self::carries`] has already refused every
    /// address a list does hold, but not on a tunnel to an address literal; and bytes that are no
    /// TLS record at all are carried, to anything but an exempt name.
    pub fn pins(&self, host: &str, hello: &Hello) -> bool {
        let host = plain(host);
        let exempt = self.tunnels(&host, None) || Self::is_harness_host(&host);
        match hello {
            Hello::Named(sni) => {
                let sni = plain(sni);
                if exempt {
                    sni != host
                } else {
                    self.is_forge_name(&sni) || literal(&sni).is_some_and(|ip| self.holds(ip))
                }
            }
            // A hello that says no name is no better than a literal: nothing else names the
            // server, so a tunnel to an address literal is carried only where a list says the
            // forge is not there, and a hello that hides or has no name does not say so.
            Hello::NoName | Hello::Ech(None) => exempt || literal(&host).is_some(),
            // A forge name beside an encrypted hello is what a server ignoring ECH would serve:
            // refused as a named hello is. Browsers' GREASE ECH to any other name passes.
            Hello::Ech(Some(sni)) => {
                let sni = plain(sni);
                exempt
                    || literal(&host).is_some()
                    || self.is_forge_name(&sni)
                    || literal(&sni).is_some_and(|ip| self.holds(ip))
            }
            Hello::NotTls => exempt,
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
        let host = plain(host);
        if self.tunnels(&host, None) || Self::is_harness_host(&host) {
            return true;
        }
        !self.holds(at)
    }
}

/// How long a connection waits for the system resolver before the name counts as unresolved. A
/// blackholed name would otherwise stall a chat's new connections for the OS resolver's own
/// timeout (tens of seconds).
const LOOKUP_WAIT: Duration = Duration::from_secs(3);

/// The system resolver's answer for `name`, none where it does not resolve or does not answer
/// within [`LOOKUP_WAIT`].
fn system(name: &str) -> Vec<IpAddr> {
    bounded(name, LOOKUP_WAIT, |name| {
        use std::net::ToSocketAddrs;
        (name, 443)
            .to_socket_addrs()
            .map(|found| found.map(|at| at.ip()).collect())
            .unwrap_or_default()
    })
}

/// The names a lookup thread is still running for. **One thread in flight per name**: a name
/// that does not answer would otherwise leave a thread per new connection behind it, each held
/// by the system resolver's own timeout.
static IN_FLIGHT: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Takes `name` off [`IN_FLIGHT`] when the lookup thread ends, however it ends.
struct Asking(String);

impl Drop for Asking {
    fn drop(&mut self) {
        let mut asking = IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner());
        asking.retain(|name| *name != self.0);
    }
}

/// `ask`'s answer for `name`, run on a thread of its own and waited for at most `wait`. **A
/// timeout is a failed lookup**: it answers nothing, so the name stays refused by name, nothing
/// is held as empty (the name is asked again after [`NEGATIVE`]), and the late answer is dropped.
/// **A name already being asked on an earlier thread is a failed lookup too**, with no new thread.
fn bounded(
    name: &str,
    wait: Duration,
    ask: impl FnOnce(&str) -> Vec<IpAddr> + Send + 'static,
) -> Vec<IpAddr> {
    let (send, receive) = std::sync::mpsc::channel();
    let name = name.to_owned();
    {
        let mut asking = IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner());
        if asking.contains(&name) {
            return Vec::new();
        }
        asking.push(name.clone());
    }
    let guard = Asking(name.clone());
    let spawned = std::thread::Builder::new()
        .name("forge-lookup".to_owned())
        .spawn(move || {
            let _guard = guard;
            // The receiver is gone once the wait is over: the late answer is dropped.
            let _ = send.send(ask(&name));
        });
    match spawned {
        Ok(_) => receive.recv_timeout(wait).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests;
