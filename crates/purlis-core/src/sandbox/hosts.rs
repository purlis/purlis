// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The hosts a project and a person add to a sandboxed chat's Internet access (ADR 0067 §1 and
//! §3 as amended 2026-10-06; spec #1330, #1341), beside the presets.
//!
//! **One entry is a [`Host`]**: a domain, `*.domain` (every name under it, never the domain
//! itself), an IPv4 address or a bracketed IPv6 address, each with an optional `:port`. Private
//! ranges are hosts like any other: a cluster reached over a VPN is the case this exists for.
//! What is never a host is refused with a sentence saying why ([`Host::parse`]): this machine
//! (loopback and the unspecified address), link-local addresses (where cloud metadata services
//! answer), broadcast and multicast, a range, a URL, and a single name with no dot.
//!
//! **A host without a port means what a preset's host means on each harness**: on Claude Code
//! any port (its own `network.allowedDomains` rule), and through purlis's egress proxy, which
//! the opencode and Codex wraps reach the network through, HTTPS's port for a tunnel and HTTP's
//! for a plain request. A host with a port is that port alone, on every harness.
//!
//! **Levels** ([`Level`]). The **Project**'s hosts are `[sandbox] hosts` in the committed
//! `charter.toml`: everyone who opens the project follows them, with no approval, and each
//! teammate is told once when they change ([`super::local::hosts_changed`]). **You** are this
//! machine's: `[sandbox] hosts` in `charter.local.toml`, read only while git leaves that file
//! alone ([`crate::settings::layer_text`]). A **Persona**'s hosts (#1362) are a third level:
//! `[sandbox.personas.<name>] hosts` in the committed file, granted only to a chat running as
//! that persona, and only once the person on this machine allowed the list as it stands
//! ([`super::persona::in_force_here`], D-1362-7).
//!
//! **No chat writes either list.** Both files are later-code names a sandboxed chat never writes
//! ([`super::PLANTED`]): that denial is the boundary today. [`super::changes_a_sandbox_key`] is
//! the rule a brokered write asks once #1333 builds one.
//!
//! **Policy** (#1343) is [`Locks`] ([`super::policy`]): an admin's locks, which [`in_force`]
//! asks of every host before it is granted.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

/// The key in `[sandbox]` that holds the hosts, in both files.
pub const KEY: &str = "hosts";

/// One host a chat may reach: its name, and the one port it is held to, where it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    name: Name,
    port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Name {
    Domain(String),
    /// `*.<suffix>`: every name under the suffix.
    Under(String),
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}

/// What a host is written like, said where one is refused for being something else.
const LOOKS_LIKE: &str = "a domain such as api.example.com, *.example.com for every name under \
                          it, an IP address such as 10.0.0.5, or any of them with :port";

impl Host {
    /// `typed` as a host, or the one sentence saying why it is not one. Spaces around it are
    /// dropped, letters are lowered and a final dot is dropped.
    pub fn parse(typed: &str) -> Result<Self, String> {
        let typed = typed.trim();
        if typed.is_empty() {
            return Err(format!("Type a host: {LOOKS_LIKE}."));
        }
        if typed.chars().any(char::is_whitespace) {
            return Err("A host has no spaces in it.".to_owned());
        }
        if typed.chars().any(char::is_control) {
            return Err("A host has no control characters in it.".to_owned());
        }
        if let Some((_, rest)) = typed.split_once("://") {
            let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
            return Err(format!(
                "That is a URL. Type its host alone, such as {}, without the scheme or a path.",
                if host.is_empty() {
                    "api.example.com"
                } else {
                    host
                }
            ));
        }
        if let Some((before, _)) = typed.split_once('/') {
            if before.parse::<Ipv4Addr>().is_ok() || before.starts_with('[') {
                return Err(
                    "A range of addresses is not a host. Add each address the chats need."
                        .to_owned(),
                );
            }
            return Err(format!(
                "Leave out the path: a host ends before the first /, so type {before}."
            ));
        }
        if typed.contains('@') {
            return Err("Leave out the user name: type the host alone.".to_owned());
        }
        let lowered = typed.to_ascii_lowercase();
        let (name, port) = split(&lowered)?;
        let port = port.map(port_of).transpose()?;
        let name = name_of(name)?;
        Ok(Self { name, port })
    }

    /// **Whether `self`, as a policy lists it, allows `other`** (#1343): the same name, or
    /// for `*.domain` any name under it (a `*.` entry under it included); and any port where
    /// `self` names none, else that port alone.
    pub fn covers(&self, other: &Host) -> bool {
        let names = match (&self.name, &other.name) {
            (Name::Under(suffix), Name::Domain(name) | Name::Under(name)) => {
                name.strip_suffix(suffix.as_str())
                    .is_some_and(|head| head.ends_with('.'))
                    || matches!(&other.name, Name::Under(same) if same == suffix)
            }
            (mine, theirs) => mine == theirs,
        };
        names && self.port.is_none_or(|port| other.port == Some(port))
    }

    /// The address it is, where it is an address rather than a name.
    pub fn address(&self) -> Option<IpAddr> {
        match &self.name {
            Name::V4(ip) => Some(IpAddr::V4(*ip)),
            Name::V6(ip) => Some(IpAddr::V6(*ip)),
            Name::Domain(_) | Name::Under(_) => None,
        }
    }

    /// The port it is held to, where it has one.
    pub fn port(&self) -> Option<u16> {
        self.port
    }
}

/// The spelling every compiler writes: a domain or `*.domain`, an IPv4 address or a bracketed
/// IPv6 address, then `:port` where it has one. Claude Code's `network.allowedDomains` takes it
/// as written, and so does purlis's egress proxy ([`super::egress`]).
impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Name::Domain(name) => write!(f, "{name}")?,
            Name::Under(suffix) => write!(f, "*.{suffix}")?,
            Name::V4(ip) => write!(f, "{ip}")?,
            Name::V6(ip) => write!(f, "[{ip}]")?,
        }
        match self.port {
            Some(port) => write!(f, ":{port}"),
            None => Ok(()),
        }
    }
}

/// `typed`, lowered, as its name and the port written after it.
fn split(typed: &str) -> Result<(&str, Option<&str>), String> {
    if typed.starts_with('[') {
        let Some(close) = typed.find(']') else {
            return Err(format!(
                "{typed} opens a bracket and never closes it: an IPv6 address is written \
                 [fd00::1], or [fd00::1]:443."
            ));
        };
        let (name, after) = typed.split_at(close + 1);
        return match after {
            "" => Ok((name, None)),
            _ => after
                .strip_prefix(':')
                .map(|port| (name, Some(port)))
                .ok_or_else(port_refusal),
        };
    }
    match typed.matches(':').count() {
        0 => Ok((typed, None)),
        1 => {
            let (name, port) = typed.split_once(':').unwrap_or((typed, ""));
            Ok((name, Some(port)))
        }
        _ => Err(match typed.parse::<Ipv6Addr>() {
            Ok(ip) => format!(
                "Write an IPv6 address in brackets, such as [{ip}], or [{ip}]:443 with a port."
            ),
            Err(_) => format!("{typed} is not a host: {LOOKS_LIKE}."),
        }),
    }
}

fn port_refusal() -> String {
    "The port must be a number from 1 to 65535.".to_owned()
}

pub(super) fn port_of(typed: &str) -> Result<u16, String> {
    if typed.is_empty() || !typed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(port_refusal());
    }
    typed
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(port_refusal)
}

fn name_of(name: &str) -> Result<Name, String> {
    if let Some(inside) = name.strip_prefix('[').and_then(|it| it.strip_suffix(']')) {
        let ip = inside
            .parse::<Ipv6Addr>()
            .map_err(|_| format!("{inside} is not an IPv6 address."))?;
        return v6(ip).map(Name::V6);
    }
    let name = name.strip_suffix('.').unwrap_or(name);
    if !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return name
            .parse::<Ipv4Addr>()
            .map_err(|_| {
                format!(
                    "{name} is not an IPv4 address: that is four numbers from 0 to 255 with no \
                     leading zeros, such as 10.0.0.5."
                )
            })
            .and_then(v4)
            .map(Name::V4);
    }
    // The WHATWG URL rule: a name whose last part is a number is an address in another
    // notation (`0x7f.1` is 127.0.0.1 to a resolver), and a wildcard over one names addresses.
    if ends_in_a_number(name) {
        return Err(format!(
            "{name} ends in a number, so it is an address written another way, not a domain. \
             Type the address as four numbers, such as 10.0.0.5."
        ));
    }
    if name == "localhost" || name.ends_with(".localhost") {
        return Err(this_machine(name));
    }
    if let Some(suffix) = name.strip_prefix("*.") {
        domain(suffix)?;
        if !suffix.contains('.') {
            return Err(format!(
                "*.{suffix} covers a whole top-level domain. Name the domain under it, such as \
                 *.example.{suffix}."
            ));
        }
        return Ok(Name::Under(suffix.to_owned()));
    }
    if name.contains('*') {
        return Err(
            "A wildcard is written at the front, as *.example.com, for every name under \
             example.com."
                .to_owned(),
        );
    }
    domain(name)?;
    if !name.contains('.') {
        return Err(format!(
            "{name} is a single name. Type its full name, such as {name}.internal.example, or \
             its IP address: Claude Code's sandbox allows only a name with a dot."
        ));
    }
    Ok(Name::Domain(name.to_owned()))
}

/// Why `name` is not a domain, if it is not one.
fn domain(name: &str) -> Result<(), String> {
    if !name.is_ascii() {
        return Err(format!(
            "{name} has letters outside ASCII. Type an internationalised name in its xn-- \
             form, as DNS has it."
        ));
    }
    if name.len() > 253 {
        return Err("A domain is at most 253 characters long.".to_owned());
    }
    for label in name.split('.') {
        if label.is_empty() {
            return Err(format!(
                "{name} has an empty part between dots: each part of a domain has a letter or \
                 digit."
            ));
        }
        if label.len() > 63 {
            return Err(format!(
                "A part of {name} is longer than 63 characters, which no domain's part is."
            ));
        }
        if !label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(format!(
                "{name} is not a domain: a domain holds letters, digits, hyphens and dots only."
            ));
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(format!(
                "{name} is not a domain: no part of a domain starts or ends with a hyphen."
            ));
        }
    }
    Ok(())
}

fn this_machine(name: &str) -> String {
    format!(
        "{name} is this machine. A chat's sandbox never reaches this machine's own services \
         as a host, since that would reach every one of them."
    )
}

/// Whether `name`'s last part is a number: all digits, or `0x` and hex digits.
pub(super) fn ends_in_a_number(name: &str) -> bool {
    let last = name.rsplit('.').next().unwrap_or(name);
    if !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    last.strip_prefix("0x")
        .is_some_and(|hex| hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The cloud metadata addresses outside link-local that Claude Code's sandbox runtime refuses
/// (2.1.291): Alibaba's, Azure's wire server, and Oracle's.
const METADATA_V4: [Ipv4Addr; 3] = [
    Ipv4Addr::new(100, 100, 100, 200),
    Ipv4Addr::new(168, 63, 129, 16),
    Ipv4Addr::new(192, 0, 0, 192),
];

/// The IPv6 cloud metadata addresses the same runtime refuses, besides `fd00:ec2::/32` (AWS).
const METADATA_V6: [Ipv6Addr; 5] = [
    Ipv6Addr::new(0xfd20, 0xce, 0, 0, 0, 0, 0, 0x254),
    Ipv6Addr::new(0xfd00, 0xc1, 0, 0, 0, 0, 0xa9fe, 0xa9fe),
    Ipv6Addr::new(0xfd00, 0x42, 0, 0, 0, 0, 0, 0x42),
    Ipv6Addr::new(0xfd00, 0xa9fe, 0xa9fe, 0, 0, 0, 0, 1),
    Ipv6Addr::new(0xfd00, 0x100, 0, 0, 0, 0, 0x100, 0x200),
];

fn metadata_v4(ip: Ipv4Addr) -> bool {
    METADATA_V4.contains(&ip)
}

/// The IPv6 cloud metadata addresses as ranges: AWS's `fd00:ec2::/32`, and each of
/// [`METADATA_V6`] alone.
pub(super) fn metadata_v6_ranges() -> Vec<(Ipv6Addr, u8)> {
    let mut ranges = vec![(Ipv6Addr::new(0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0), 32)];
    ranges.extend(METADATA_V6.iter().map(|ip| (*ip, 128)));
    ranges
}

fn metadata_v6(ip: Ipv6Addr) -> bool {
    let [first, second, ..] = ip.segments();
    (first == 0xfd00 && second == 0x0ec2) || METADATA_V6.contains(&ip)
}

/// Whether `ip` is a cloud metadata service's (#2445), as itself or carried in an IPv6 address:
/// the link-local ones every cloud and its container service answer at, and the others
/// ([`METADATA_V4`], [`METADATA_V6`]).
pub(super) fn metadata_address(ip: IpAddr) -> bool {
    let v4 = |ip: Ipv4Addr| metadata_v4(ip) || METADATA_LINK_LOCAL.contains(&ip);
    match ip {
        IpAddr::V4(ip) => v4(ip),
        IpAddr::V6(ip) => {
            let as_itself = match carried_v4(ip).as_slice() {
                [] => metadata_v6(ip),
                carried => carried.iter().any(|one| v4(*one)),
            };
            as_itself || tunnelled_v4(ip).into_iter().any(v4)
        }
    }
}

/// The link-local addresses cloud metadata services answer at: every cloud's instance
/// metadata, and AWS's container metadata.
const METADATA_LINK_LOCAL: [Ipv4Addr; 2] = [
    Ipv4Addr::new(169, 254, 169, 254),
    Ipv4Addr::new(169, 254, 170, 2),
];

fn metadata(name: &str) -> String {
    format!("{name} is a cloud metadata address, so it is never a host a chat reaches.")
}

/// **Whether a chat is never let reach `ip`**, as a name resolves it (#1341): this machine
/// (loopback, `0.0.0.0/8`, the unspecified address, and each of `own`, this machine's
/// interface addresses), link-local, multicast and broadcast addresses, and the cloud metadata
/// addresses — the set Claude Code's sandbox runtime refuses. purlis's egress proxy asks it of
/// every address a listed name resolves to, at every connect, so a name that resolves, or is
/// rebound, to one of them reaches nothing. A Teredo or ISATAP address is refused as itself and
/// as each IPv4 address it carries ([`tunnelled_v4`]).
pub fn refused_address(ip: IpAddr, own: &[IpAddr]) -> bool {
    let tunnelled = match ip {
        IpAddr::V4(_) => Vec::new(),
        IpAddr::V6(ip) => tunnelled_v4(ip),
    };
    refused_as_itself(ip, own)
        || tunnelled
            .into_iter()
            .any(|one| refused_as_itself(IpAddr::V4(one), own))
}

/// **Whether `ip` is in a private or special-use range** (RFC 6890 and its updates): every
/// address [`refused_address`] refuses, and besides it the private networks (RFC 1918), shared
/// address space (RFC 6598), the IETF, documentation and benchmarking ranges, the reserved and
/// broadcast ones, unique local and discard-only IPv6 (RFC 4193, RFC 6666), and the IPv6
/// documentation range. An IPv6 address that carries an IPv4 one (mapped, NAT64, 6to4, Teredo)
/// is judged by that one too. What a confined extension's proxy never reaches (ADR 0101 §2).
pub fn special_use(ip: IpAddr) -> bool {
    if refused_address(ip, &[]) {
        return true;
    }
    let v4 = |ip: Ipv4Addr| {
        let [a, b, c, _] = ip.octets();
        ip.is_private()
            || (a == 100 && (64..128).contains(&b))
            || (a == 192 && b == 0 && c == 0)
            || (a == 192 && b == 0 && c == 2)
            || (a == 198 && (18..20).contains(&b))
            || (a == 198 && b == 51 && c == 100)
            || (a == 203 && b == 0 && c == 113)
            || (a == 192 && b == 88 && c == 99)
            || a >= 240
    };
    match ip {
        IpAddr::V4(ip) => v4(ip),
        IpAddr::V6(ip) => {
            let [first, second, ..] = ip.segments();
            let carried: Vec<Ipv4Addr> =
                carried_v4(ip).into_iter().chain(tunnelled_v4(ip)).collect();
            (first & 0xfe00) == 0xfc00
                || (first == 0x2001 && second == 0x0db8)
                || (first == 0x0100 && ip.segments()[1..4] == [0, 0, 0])
                || (first == 0x2001 && second < 0x0200)
                || carried.into_iter().any(v4)
        }
    }
}

/// [`refused_address`], with a Teredo or ISATAP address judged as itself alone.
pub(super) fn refused_as_itself(ip: IpAddr, own: &[IpAddr]) -> bool {
    if own.contains(&ip) {
        return true;
    }
    match ip {
        IpAddr::V4(ip) => {
            ip.is_loopback()
                || ip.octets()[0] == 0
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_broadcast()
                || metadata_v4(ip)
        }
        IpAddr::V6(ip) => match carried_v4(ip).as_slice() {
            [] => {
                ip.is_loopback()
                    || ip.is_unspecified()
                    || ip.is_unicast_link_local()
                    || ip.is_multicast()
                    || metadata_v6(ip)
            }
            carried => carried
                .iter()
                .any(|one| refused_as_itself(IpAddr::V4(*one), own)),
        },
    }
}

/// This machine's own interface addresses, as the system lists them now; none where it cannot.
pub fn own_addresses() -> Vec<IpAddr> {
    own_addresses_read().unwrap_or_default()
}

/// This machine's own interface addresses, as the system lists them now, or `None` where the
/// list cannot be read: what a caller that must refuse without it asks.
pub fn own_addresses_read() -> Option<Vec<IpAddr>> {
    own_networks_read().map(|networks| networks.iter().map(|one| one.address).collect())
}

/// **This machine's own networks** (#2445), as its interfaces are on them now: each interface
/// address with its prefix; or `None` where the list cannot be read.
pub fn own_networks_read() -> Option<Vec<Network>> {
    #[cfg(unix)]
    {
        let found = nix::ifaddrs::getifaddrs().ok()?;
        Some(
            found
                .filter_map(|one| {
                    let ip = |at: &nix::sys::socket::SockaddrStorage| {
                        at.as_sockaddr_in()
                            .map(|v4| IpAddr::V4(v4.ip()))
                            .or_else(|| at.as_sockaddr_in6().map(|v6| IpAddr::V6(v6.ip())))
                    };
                    let address = ip(one.address.as_ref()?)?;
                    Some(network_of(address, one.netmask.as_ref().and_then(ip)))
                })
                .collect(),
        )
    }
    #[cfg(not(unix))]
    None
}

/// **The network an interface at `address` with netmask `mask` is on**: the address alone where
/// it has no mask, or one of the other family ([`Network::of`] for a mask too short).
#[cfg_attr(not(unix), allow(dead_code))]
pub(super) fn network_of(address: IpAddr, mask: Option<IpAddr>) -> Network {
    match mask.filter(|mask| mask.is_ipv4() == address.is_ipv4()) {
        Some(mask) => Network::of(address, prefix_of(mask)),
        None => Network::host(address),
    }
}

/// The length of the prefix netmask `mask` writes, its leading ones.
#[cfg_attr(not(unix), allow(dead_code))]
fn prefix_of(mask: IpAddr) -> u8 {
    let ones = match mask {
        IpAddr::V4(mask) => u32::from(mask).leading_ones(),
        IpAddr::V6(mask) => u128::from(mask).leading_ones(),
    };
    u8::try_from(ones).unwrap_or(u8::MAX)
}

/// **A network an interface of this machine is on** (#2445): its address and the length of its
/// prefix. A prefix shorter than any one network a machine is on (`/8`, or `/16` for IPv6) is
/// read as the address alone, so a misread mask never takes in the whole Internet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Network {
    pub address: IpAddr,
    prefix: u8,
}

impl Network {
    /// The network `address` is on, `prefix` bits long.
    pub fn of(address: IpAddr, prefix: u8) -> Self {
        let (shortest, whole) = if address.is_ipv4() {
            (8, 32)
        } else {
            (16, 128)
        };
        Self {
            address,
            prefix: if prefix < shortest || prefix > whole {
                whole
            } else {
                prefix
            },
        }
    }

    /// `address` alone.
    pub fn host(address: IpAddr) -> Self {
        Self::of(address, if address.is_ipv4() { 32 } else { 128 })
    }

    /// Whether `ip` is on this network.
    pub fn holds(&self, ip: IpAddr) -> bool {
        match (self.address, ip) {
            (IpAddr::V4(mine), IpAddr::V4(theirs)) => {
                let mask = u32::MAX
                    .checked_shl(32 - u32::from(self.prefix))
                    .unwrap_or(0);
                u32::from(mine) & mask == u32::from(theirs) & mask
            }
            (IpAddr::V6(mine), IpAddr::V6(theirs)) => {
                let mask = u128::MAX
                    .checked_shl(128 - u32::from(self.prefix))
                    .unwrap_or(0);
                u128::from(mine) & mask == u128::from(theirs) & mask
            }
            _ => false,
        }
    }
}

fn v4(ip: Ipv4Addr) -> Result<Ipv4Addr, String> {
    if ip.is_loopback() || ip.octets()[0] == 0 {
        return Err(this_machine(&ip.to_string()));
    }
    if metadata_v4(ip) {
        return Err(metadata(&ip.to_string()));
    }
    if ip.is_link_local() {
        return Err(link_local(&ip.to_string()));
    }
    if ip.is_broadcast() || ip.is_multicast() {
        return Err(not_one_machine(&ip.to_string()));
    }
    Ok(ip)
}

/// **The IPv4 addresses an IPv6 address may carry**: the one of each form [`embedded_v4`]
/// decodes, or each NAT64's local-use prefix may carry at the lengths it may be used at
/// ([`super::nat64::local_use`], #2530). An address that carries one reaches it, so it is
/// judged as each of them.
pub(super) fn carried_v4(ip: Ipv6Addr) -> Vec<Ipv4Addr> {
    match embedded_v4(ip) {
        Some(carried) => vec![carried],
        None => super::nat64::local_use(ip),
    }
}

/// **The IPv4 addresses a Teredo or an ISATAP address carries** (#2678), which it reaches its
/// node through, so it is judged as itself and as each of them: Teredo's (`2001::/32`, RFC
/// 4380, and `3ffe:831f::/32`, the prefix Teredo used before it, #2741) server address in bits
/// 32 to 63 and its client's, every bit flipped, in the last 32;
/// ISATAP's (RFC 5214) in the last 32 bits behind a `0:5efe` or `200:5efe` interface identifier,
/// under any prefix (the group bit, `100:5efe` and `300:5efe`, is read too).
pub(super) fn tunnelled_v4(ip: Ipv6Addr) -> Vec<Ipv4Addr> {
    let s = ip.segments();
    let of = |hi: u16, lo: u16| {
        let ([a, b], [c, d]) = (hi.to_be_bytes(), lo.to_be_bytes());
        Ipv4Addr::new(a, b, c, d)
    };
    let mut carried = Vec::new();
    if matches!([s[0], s[1]], [0x2001, 0] | [0x3ffe, 0x831f]) {
        carried.push(of(s[2], s[3]));
        carried.push(of(!s[6], !s[7]));
    }
    if s[4] & 0xfcff == 0 && s[5] == 0x5efe {
        carried.push(of(s[6], s[7]));
    }
    carried
}

/// The IPv4 address an IPv6 address carries, in each form Claude Code's sandbox runtime
/// decodes (2.1.291): IPv4-mapped `::ffff:0:0/96`, SIIT `::ffff:0:0:0/96`, NAT64
/// `64:ff9b::/96`, IPv4-compatible `::/96` (not `::` or `::1`), and 6to4 `2002::/16`.
fn embedded_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = ip.segments();
    let of = |hi: u16, lo: u16| {
        let ([a, b], [c, d]) = (hi.to_be_bytes(), lo.to_be_bytes());
        Ipv4Addr::new(a, b, c, d)
    };
    let low = || of(s[6], s[7]);
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return Some(mapped);
    }
    match s {
        [0, 0, 0, 0, 0xffff, 0, _, _] => Some(low()),
        [0x64, 0xff9b, 0, 0, 0, 0, _, _] => Some(low()),
        [0, 0, 0, 0, 0, 0, _, _] if !ip.is_unspecified() && !ip.is_loopback() => Some(low()),
        [0x2002, hi, lo, ..] => Some(of(hi, lo)),
        _ => None,
    }
}

fn v6(ip: Ipv6Addr) -> Result<Ipv6Addr, String> {
    if ip.to_ipv4_mapped().is_some() {
        return Err(format!(
            "[{ip}] is an IPv4 address written as IPv6. Type it as IPv4, such as 10.0.0.5."
        ));
    }
    // An address that carries an IPv4 one reaches it, so it is judged as that one.
    for carried in carried_v4(ip).into_iter().chain(tunnelled_v4(ip)) {
        v4(carried).map_err(|why| format!("[{ip}] carries the IPv4 address {carried}. {why}"))?;
    }
    if ip.is_loopback() || ip.is_unspecified() {
        return Err(this_machine(&format!("[{ip}]")));
    }
    if ip.is_unicast_link_local() {
        return Err(link_local(&format!("[{ip}]")));
    }
    if metadata_v6(ip) {
        return Err(metadata(&format!("[{ip}]")));
    }
    if ip.is_multicast() {
        return Err(not_one_machine(&format!("[{ip}]")));
    }
    Ok(ip)
}

fn link_local(name: &str) -> String {
    format!(
        "{name} is a link-local address, where cloud metadata services answer, so it is never \
         a host a chat reaches."
    )
}

fn not_one_machine(name: &str) -> String {
    format!("{name} is a broadcast or multicast address, not one machine.")
}

/// Who a host was added by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// The project's committed `charter.toml`: everyone who opens the project follows it.
    Project,
    /// This machine's `charter.local.toml`.
    You,
    /// The persona a chat runs as: `[sandbox.personas.<name>]` in the committed `charter.toml`
    /// (#1362).
    Persona,
}

impl Level {
    /// The word Settings and a Notice name it by.
    pub fn word(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::You => "you",
            Self::Persona => "persona",
        }
    }
}

/// A host a chat in this project may reach, and the level it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Granted {
    pub host: Host,
    pub level: Level,
}

pub use super::policy::Locks;

/// **The hosts in force**: the project's, then this machine's, then the persona's the chat runs
/// as, each once (where two levels list a host, it is the first's), less what `locks` refuse.
pub fn in_force(
    project: &[Host],
    personal: &[Host],
    persona: &[Host],
    locks: &Locks,
) -> Vec<Granted> {
    let mut out: Vec<Granted> = Vec::new();
    let levels = [
        (Level::Project, project),
        (Level::You, personal),
        (Level::Persona, persona),
    ];
    for (level, hosts) in levels {
        for host in hosts {
            if out.iter().any(|kept| kept.host == *host) {
                continue;
            }
            let granted = Granted {
                host: host.clone(),
                level,
            };
            if locks.refuses(&granted).is_none() {
                out.push(granted);
            }
        }
    }
    out
}

/// `granted` less each host that is one of `own`, this machine's interface addresses: a LAN or
/// VPN address of this machine reaches its own services, which no host may (#1341).
pub fn off_this_machine(granted: Vec<Granted>, own: &[IpAddr]) -> Vec<Granted> {
    granted
        .into_iter()
        .filter(|one| one.host.address().is_none_or(|ip| !own.contains(&ip)))
        .collect()
}

/// What a file's `[sandbox] hosts` holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listed {
    /// Each host it names, once.
    pub hosts: Vec<Host>,
    /// Each entry it does not take, as TOML writes it, and why.
    pub refused: Vec<(String, String)>,
}

/// `hosts` that is not a list: no host of that level is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotAList;

/// What a file's `[sandbox] hosts`, `value`, holds. Absent is no host.
pub fn read(value: Option<&toml::Value>) -> Result<Listed, NotAList> {
    let Some(value) = value else {
        return Ok(Listed::default());
    };
    let Some(entries) = value.as_array() else {
        return Err(NotAList);
    };
    let mut hosts: Vec<Host> = Vec::new();
    let mut refused = Vec::new();
    for entry in entries {
        match entry.as_str().map(Host::parse) {
            Some(Ok(host)) => {
                if !hosts.contains(&host) {
                    hosts.push(host);
                }
            }
            Some(Err(why)) => refused.push((entry.to_string(), why)),
            None => refused.push((
                entry.to_string(),
                "A host is written as text in quotes.".to_owned(),
            )),
        }
    }
    Ok(Listed { hosts, refused })
}

/// The hosts `top`'s `[sandbox] hosts` names, with nothing refused: what a reader takes.
pub fn of_table(top: Option<&toml::Table>) -> Vec<Host> {
    top.and_then(|top| top.get(super::TABLE))
        .and_then(toml::Value::as_table)
        .and_then(|table| read(table.get(KEY)).ok())
        .map(|listed| listed.hosts)
        .unwrap_or_default()
}

/// **This machine's hosts** for the project at `root`: `charter.local.toml`'s `[sandbox] hosts`,
/// each only once you confirmed it on this machine ([`confirmed_only`]).
///
/// **Confirmed, because nothing in git proves the file is yours.** A sandboxed chat can write
/// all of a clone's git state (its index, ignore files, objects and refs), so no check of git
/// can tell a host you added from one a teammate committed, or one a chat wrote. The record of
/// what you confirmed is kept in `.charter/app/sandbox.json`, which no sandboxed chat writes
/// (the integrity class), and only Settings writes it: adding a host there confirms it, and a
/// host the file holds that you did not add there is shown "not yet confirmed", with Confirm.
///
/// Read with the sandbox's own reader ([`super::read_plane_file`]): a link, or anything but a
/// regular file, grants nothing. A file git would carry grants nothing either.
pub fn personal(root: &Path) -> Vec<Host> {
    let path = crate::names::local_settings(root);
    let Ok(Some(text)) = super::read_plane_file(&path) else {
        return Vec::new();
    };
    if !crate::profiles::ignore_check(root).passes() {
        return Vec::new();
    }
    confirmed_only(
        of_table(text.parse::<toml::Table>().ok().as_ref()),
        &super::local::confirmed_hosts(root),
    )
}

/// The hosts of `listed` that `confirmed`, the record of what you confirmed, holds.
pub fn confirmed_only(listed: Vec<Host>, confirmed: &[String]) -> Vec<Host> {
    listed
        .into_iter()
        .filter(|host| confirmed.contains(&host.to_string()))
        .collect()
}

#[cfg(test)]
mod tests;
