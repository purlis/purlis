// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The sandbox's laws, as properties** (T1.1-23, #6870; M74.1 spec, Solution item 2): the
//! decision code is attacked with generated inputs, not only exercised with chosen ones.
//!
//! **Under which template.** Every property here runs under the `strict` template (Q308): every
//! block family enforced, as purlis decided before `[policy]` existed. The decision code these
//! properties call (`Reach`, the board of live asks, the hosts matcher, the proxy) takes no
//! template: it is what `strict` runs, and what the other templates switch off or log.
//!
//! **The laws as the code orders them**, checked against the code first and confirmed by the
//! orchestrator before they were written (2026-11-16):
//!
//! - **Scopes** form one chain per kind, not one chain across kinds (D1): a host once ⊂ chat ⊂ you
//!   ⊂ project, a socket or a service once ⊂ chat ⊂ you ⊂ machine. Machine and project are both
//!   above you, and no kind is kept at both. A policy's `allow-scopes` removes any scope on its
//!   own, which is the policy's choice, so these laws run with no lock.
//! - **Keep blocked** dominates every Allow given before it, on its chat's board, for each target
//!   it covers, until a later Allow covers that target: the later answer wins (D2). It lives in
//!   the chat's memory alone. Its persistence, as a sealed Never that holds in every mode, is
//!   ticket 73's and M75's (Q308b, Q314.12).
//! - **Special-use addresses** are the ones `hosts::refused_address` refuses, in every spelling
//!   (D3). A private range is never carried for a chat unless it is listed (it is asked about),
//!   and is refused for a confined extension (`hosts::special_use`).
//! - **The decided host is the host looked up and dialled** by both kinds of proxy, and the TLS
//!   server name only for a confined extension's proxy (D4): a chat's proxy does not read SNI,
//!   a declared known gap, #6996.
//!
//! Each property runs at least 10 000 cases (more with `PROPTEST_CASES`, as the nightly entry
//! `property-tests` sets), from a fresh seed each run.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use proptest::prelude::*;
use proptest::test_runner::Config;

use purlis_core::sandbox::asks::{Answer, Asks, Heard};
use purlis_core::sandbox::egress::{
    self, ProviderAddresses, Proxy, Serving, TUNNEL_PORTS, allows, allows_on,
};
use purlis_core::sandbox::grant::{Level, What};
use purlis_core::sandbox::hosts::{Host, refused_address, special_use};
use purlis_core::sandbox::names::Names;
use purlis_core::sandbox::policy::Locks;
use purlis_core::sandbox::reach::{By, Decision, Reach};
use purlis_core::sandbox::timings::Timings;

/// At least 10 000 cases, or `PROPTEST_CASES` where it asks for more.
fn config() -> Config {
    let default = Config::default();
    Config {
        cases: default.cases.max(10_000),
        ..default
    }
}

// ---------------------------------------------------------------------------------------------
// What the properties are made of.
// ---------------------------------------------------------------------------------------------

/// Labels drawn from a small pool, so generated hosts collide, nest and overlap often.
fn label() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => proptest::sample::select(vec!["a", "b", "api", "ex", "example", "com", "test", "evil"])
            .prop_map(str::to_owned),
        1 => "[a-z]([a-z0-9-]{0,3}[a-z0-9])?",
    ]
}

/// A domain of two to four labels, as a grant writes one.
fn domain() -> impl Strategy<Value = String> {
    proptest::collection::vec(label(), 2..=4)
        .prop_map(|labels| labels.join("."))
        .prop_filter("a domain the hosts' rules accept", |name| Host::parse(name).is_ok())
}

fn port() -> impl Strategy<Value = u16> {
    prop_oneof![
        3 => proptest::sample::select(vec![80u16, 443, 8080]),
        1 => 1u16..=65535,
    ]
}

/// A host a list may hold, as the hosts' rules write it: a domain, a wildcard over one, with or
/// without a port. Only what `Host::parse` accepts, so a list is one a person could have made.
fn listed_host() -> impl Strategy<Value = String> {
    (domain(), any::<bool>(), proptest::option::of(port())).prop_filter_map(
        "a host the hosts' rules accept",
        |(name, wildcard, port)| {
            let name = if wildcard { format!("*.{name}") } else { name };
            let typed = match port {
                Some(port) => format!("{name}:{port}"),
                None => name,
            };
            Host::parse(&typed).ok().map(|host| host.to_string())
        },
    )
}

/// One exact `IP:port` entry, as a grant of a service writes one.
fn exact_entry(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(ip) => format!("{ip}:{port}"),
        IpAddr::V6(ip) => format!("[{ip}]:{port}"),
    }
}

fn by() -> impl Strategy<Value = By> {
    proptest::sample::select(vec![By::Open, By::Persona, By::You, By::Chat, By::Project])
}

/// A connection's host: a listed-looking one, a literal, or anything at all.
fn asked_host() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => domain(),
        2 => (domain(), any::<bool>()).prop_map(|(name, upper)| {
            if upper { name.to_ascii_uppercase() } else { format!("{name}.") }
        }),
        1 => any::<Ipv4Addr>().prop_map(|ip| ip.to_string()),
        1 => any::<Ipv6Addr>().prop_map(|ip| ip.to_string()),
        1 => ".{0,40}",
    ]
}

/// A board that answers at once: where nothing on it decides, nobody can be asked.
fn board(heard: &Arc<Mutex<Vec<Heard>>>) -> Asks {
    let heard = Arc::clone(heard);
    Asks::new(
        Arc::new(move |one| heard.lock().unwrap().push(one)),
        &Timings::production(),
    )
    .asking_while(Arc::new(|| false))
}

fn carries(answer: Answer) -> bool {
    matches!(answer, Answer::Allowed(_))
}

// ---------------------------------------------------------------------------------------------
// 1. The grant scope lattice, and Keep blocked.
// ---------------------------------------------------------------------------------------------

/// A host's scopes, narrowest first, as the code keeps a host at each.
const HOST_CHAIN: [Level; 4] = [Level::Once, Level::Chat, Level::You, Level::Project];

/// Whether `granted` (a host and port, as a block names one) allowed at `level` carries a
/// connection to `asked` on `port`: Once is the board's Allow next time; the others are listed
/// at the layer their scope is carried at.
fn carried_at(level: Level, granted: &Host, asked: &str, port: u16) -> bool {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let asks = board(&heard);
    let reach = match level {
        Level::Once => {
            asks.allow_next(granted);
            Reach::of(Vec::new())
        }
        _ => Reach::of(vec![(granted.to_string(), By::from(level))]),
    };
    match reach.decide(asked, port, &TUNNEL_PORTS, &[]) {
        Decision::Ask => carries(asks.hold(asked, port, &TUNNEL_PORTS)),
        decision => decision.carries(),
    }
}

/// What a board is told, in order.
#[derive(Debug, Clone)]
enum Op {
    Allow(String, By),
    AllowNext(String),
    KeepBlocked(String),
}

/// A target as a block names one: a host and a port, never a wildcard.
fn target() -> impl Strategy<Value = String> {
    (domain(), port()).prop_map(|(name, port)| format!("{name}:{port}"))
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => (prop_oneof![target(), listed_host()], by()).prop_map(|(host, by)| Op::Allow(host, by)),
        1 => target().prop_map(Op::AllowNext),
        2 => target().prop_map(Op::KeepBlocked),
    ]
}

fn host_of(typed: &str) -> Option<Host> {
    Host::parse(typed).ok()
}

proptest! {
    #![proptest_config(config())]

    /// **Scopes form a chain per kind, and Keep blocked dominates what came before it.**
    ///
    /// - A host allowed at a wider scope carries every connection the same host allowed at a
    ///   narrower one carries, and a host listed at several scopes decides at the widest.
    /// - The code keeps each kind where its chain says: a host never at machine, a socket or a
    ///   service at machine, and nothing but a host in the committed file.
    /// - On a chat's board, after Keep blocked of a target, no connection to it is carried
    ///   unless a later Allow covers it; and where the last answer covering it is an Allow, it is.
    #[test]
    fn grant_scopes_form_a_lattice_and_keep_blocked_dominates(
        granted in (domain(), port()),
        asked in prop_oneof![
            3 => Just(None),
            1 => asked_host().prop_map(Some),
        ],
        asked_port in port(),
        socket in "/[a-z]{1,8}/[a-z]{1,8}\\.sock",
        service in "com\\.[a-z]{1,8}\\.[a-z]{1,8}",
        ops in proptest::collection::vec(op(), 0..12),
        query in (domain(), port()),
    ) {
        // The chain, by what is carried.
        let host = Host::parse(&format!("{}:{}", granted.0, granted.1)).unwrap();
        let (asked, port) = match &asked {
            Some(asked) => (asked.clone(), asked_port),
            None => (granted.0.clone(), granted.1),
        };
        let at: Vec<bool> = HOST_CHAIN
            .iter()
            .map(|level| carried_at(*level, &host, &asked, port))
            .collect();
        for (narrower, wider) in at.iter().zip(at.iter().skip(1)) {
            prop_assert!(!narrower || *wider, "{host} for {asked}:{port}: {at:?} along {HOST_CHAIN:?}");
        }
        // Listed at several scopes, the widest decides.
        let mut levels: Vec<Level> = HOST_CHAIN[1..].to_vec();
        levels.reverse();
        let reach = Reach::of(levels.iter().map(|level| (host.to_string(), By::from(*level))).collect());
        if at[1] {
            prop_assert_eq!(reach.decide(&asked, port, &TUNNEL_PORTS, &[]), Decision::Allowed(By::Project));
        }
        // Where the code keeps each kind, with no lock.
        let locks = Locks::default();
        for level in HOST_CHAIN {
            prop_assert!(locks.refuses_grant(&What::Host(host.clone()), level).is_none(), "{level:?}");
        }
        prop_assert!(locks.refuses_grant(&What::Host(host.clone()), Level::Machine).is_some());
        let local = [What::Socket(socket.into()), What::Service(service)];
        for what in &local {
            for level in [Level::Once, Level::Chat, Level::You, Level::Machine] {
                prop_assert!(locks.refuses_grant(what, level).is_none(), "{what:?} at {level:?}");
            }
            prop_assert!(what.machine_wide() && what.machine_only(), "{what:?}");
        }
        prop_assert!(!What::Host(host.clone()).machine_only());
        // One chat's scopes are the bottom of the chain.
        for (level, one_chat) in [(Level::Once, true), (Level::Chat, true), (Level::You, false), (Level::Machine, false), (Level::Project, false)] {
            prop_assert_eq!(level.one_chat(), one_chat, "{:?}", level);
        }

        // Keep blocked, on one chat's board.
        let heard = Arc::new(Mutex::new(Vec::new()));
        let asks = board(&heard);
        for one in &ops {
            match one {
                Op::Allow(host, by) => { asks.allow(&Host::parse(host).unwrap(), *by); }
                Op::AllowNext(host) => asks.allow_next(&Host::parse(host).unwrap()),
                Op::KeepBlocked(host) => { asks.keep_blocked(&Host::parse(host).unwrap()); }
            }
        }
        let (name, port) = query;
        let Some(target) = host_of(&format!("{name}:{port}")) else { return Ok(()) };
        // Held on its own port, so a host allowed without one carries it, as `covers` reads it.
        let answer = asks.hold(&name, port, &[port]);
        let covering = |one: &Op| match one {
            Op::Allow(host, _) | Op::AllowNext(host) | Op::KeepBlocked(host) => {
                host_of(host).is_some_and(|host| host.covers(&target))
            }
        };
        // The last Keep blocked that covers the target, and whether an Allow covers it since.
        if let Some(blocked) = ops.iter().rposition(|one| matches!(one, Op::KeepBlocked(_)) && covering(one)) {
            let allowed_since = ops[blocked + 1..]
                .iter()
                .any(|one| matches!(one, Op::Allow(..) | Op::AllowNext(_)) && covering(one));
            if !allowed_since {
                prop_assert!(!carries(answer), "kept blocked by op {blocked} of {ops:?}, yet {answer:?}");
            }
        }
        // The later answer wins: the last answer covering it an Allow, it is carried, unless a
        // later Keep blocked of another target ended that Allow (it covers that target, or that
        // target covers it), as the code ends one.
        if let Some(at) = ops.iter().rposition(|one| covering(one))
            && let Op::Allow(allowed, _) = &ops[at]
            && let Some(allowed) = host_of(allowed)
            && !ops[at + 1..].iter().any(|one| matches!(one, Op::KeepBlocked(kept)
                if host_of(kept).is_some_and(|kept| kept.covers(&allowed) || allowed.covers(&kept))))
        {
            let last = &ops[at];
            prop_assert!(carries(answer), "last {last:?} of {ops:?}, yet {answer:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 2. `decide` is total, deterministic and monotone.
// ---------------------------------------------------------------------------------------------

/// How far a decision lets a connection on: refused, asked about, carried.
fn rank(decision: &Decision) -> u8 {
    match decision {
        Decision::Refused(_) => 0,
        Decision::Ask => 1,
        _ => 2,
    }
}

fn own_addresses() -> impl Strategy<Value = Vec<IpAddr>> {
    proptest::collection::vec(any::<IpAddr>(), 0..3)
}

fn defaults() -> impl Strategy<Value = Vec<u16>> {
    proptest::sample::subsequence(vec![80u16, 443, 8080], 0..=3)
}

proptest! {
    #![proptest_config(config())]

    /// **`decide` answers every input, the same way every time, and more listed never lets less
    /// on.** Any string as the host (never a panic), decided twice alike, and alike by the reach
    /// the session host reads back (`Reach::from_written`). Listing more never lowers a
    /// decision (refused < asked < carried); pinning more as never allowed never raises one.
    #[test]
    fn decide_is_total_deterministic_and_monotone(
        listed in proptest::collection::vec((listed_host(), by()), 0..8),
        more in proptest::collection::vec((listed_host(), by()), 0..4),
        never in proptest::collection::vec(listed_host(), 0..3),
        more_never in proptest::collection::vec(listed_host(), 0..3),
        exact in proptest::collection::vec((any::<IpAddr>(), port(), by()), 0..2),
        host in asked_host(),
        port in port(),
        defaults in defaults(),
        own in own_addresses(),
        anything in any::<bool>(),
    ) {
        let pinned = |hosts: &[String]| hosts.iter().map(|one| Host::parse(one).unwrap()).collect::<Vec<_>>();
        let mut all: Vec<(String, By)> = listed.clone();
        all.extend(exact.iter().map(|(ip, port, by)| (exact_entry(*ip, *port), *by)));
        let base = if anything { Reach::anything().and(all.clone()) } else { Reach::of(all.clone()) };
        let reach = base.clone().never(pinned(&never));
        let first = reach.decide(&host, port, &defaults, &own);
        prop_assert_eq!(&first, &reach.decide(&host, port, &defaults, &own));
        prop_assert_eq!(&reach.decide_by_name(&host, port, &defaults, &own), &reach.decide_by_name(&host, port, &defaults, &own));
        let read_back = Reach::from_written(&reach.written()).unwrap();
        prop_assert_eq!(&first, &read_back.decide(&host, port, &defaults, &own));

        let wider = base.clone().and(more.clone()).never(pinned(&never));
        let after = wider.decide(&host, port, &defaults, &own);
        prop_assert!(rank(&after) >= rank(&first), "{first:?} then {after:?} once {more:?} was listed");
        if first.carries() {
            prop_assert!(after.carries());
        }
        let mut stricter = never.clone();
        stricter.extend(more_never);
        let pinned_more = base.never(pinned(&stricter)).decide(&host, port, &defaults, &own);
        prop_assert!(rank(&pinned_more) <= rank(&first), "{first:?} then {pinned_more:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Every encoding of a special-use address.
// ---------------------------------------------------------------------------------------------

/// An IPv4 address `refused_address` refuses: this machine, `0/8`, link-local, multicast,
/// broadcast, or a cloud metadata service.
fn refused_v4() -> impl Strategy<Value = Ipv4Addr> {
    prop_oneof![
        any::<[u8; 3]>().prop_map(|[b, c, d]| Ipv4Addr::new(127, b, c, d)),
        any::<[u8; 3]>().prop_map(|[b, c, d]| Ipv4Addr::new(0, b, c, d)),
        any::<[u8; 2]>().prop_map(|[c, d]| Ipv4Addr::new(169, 254, c, d)),
        (224u8..=239, any::<[u8; 3]>()).prop_map(|(a, [b, c, d])| Ipv4Addr::new(a, b, c, d)),
        Just(Ipv4Addr::BROADCAST),
        proptest::sample::select(vec![
            Ipv4Addr::new(169, 254, 169, 254),
            Ipv4Addr::new(169, 254, 170, 2),
            Ipv4Addr::new(100, 100, 100, 200),
            Ipv4Addr::new(168, 63, 129, 16),
            Ipv4Addr::new(192, 0, 0, 192),
        ]),
    ]
}

/// An IPv6 address refused as itself: loopback, unspecified, link-local, multicast, AWS's
/// metadata range.
fn refused_v6() -> impl Strategy<Value = Ipv6Addr> {
    prop_oneof![
        Just(Ipv6Addr::LOCALHOST),
        Just(Ipv6Addr::UNSPECIFIED),
        any::<[u16; 7]>().prop_map(|s| Ipv6Addr::new(0xfe80 | (s[0] & 0x3f), s[1], s[2], s[3], s[4], s[5], s[6], s[0])),
        any::<[u16; 7]>().prop_map(|s| Ipv6Addr::new(0xff00 | (s[0] & 0xff), s[1], s[2], s[3], s[4], s[5], s[6], s[0])),
        any::<[u16; 6]>().prop_map(|s| Ipv6Addr::new(0xfd00, 0x0ec2, s[0], s[1], s[2], s[3], s[4], s[5])),
    ]
}

/// An IPv4 address in a private or special-use range that a chat is not refused, only asked
/// about: RFC 1918, shared address space, documentation.
fn private_v4() -> impl Strategy<Value = Ipv4Addr> {
    prop_oneof![
        any::<[u8; 3]>().prop_map(|[b, c, d]| Ipv4Addr::new(10, b, c, d)),
        (16u8..32, any::<[u8; 2]>()).prop_map(|(b, [c, d])| Ipv4Addr::new(172, b, c, d)),
        any::<[u8; 2]>().prop_map(|[c, d]| Ipv4Addr::new(192, 168, c, d)),
        (64u8..128, any::<[u8; 2]>()).prop_map(|(b, [c, d])| Ipv4Addr::new(100, b, c, d)),
        any::<u8>().prop_map(|d| Ipv4Addr::new(192, 0, 2, d)),
        any::<u8>().prop_map(|d| Ipv4Addr::new(203, 0, 113, d)),
    ]
    .prop_filter("not a metadata address", |ip| *ip != Ipv4Addr::new(100, 100, 100, 200))
}

fn hex_pair(ip: Ipv4Addr) -> (u16, u16) {
    let [a, b, c, d] = ip.octets();
    (u16::from_be_bytes([a, b]), u16::from_be_bytes([c, d]))
}

/// The IPv6 addresses that carry `ip`: mapped, SIIT, NAT64, IPv4-compatible, 6to4, Teredo (as
/// server and as client) and ISATAP.
fn carrying(ip: Ipv4Addr) -> Vec<Ipv6Addr> {
    let (hi, lo) = hex_pair(ip);
    let mut out = vec![
        ip.to_ipv6_mapped(),
        Ipv6Addr::new(0, 0, 0, 0, 0xffff, 0, hi, lo),
        Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, hi, lo),
        Ipv6Addr::new(0x2002, hi, lo, 0, 0, 0, 0, 1),
        Ipv6Addr::new(0x2001, 0, hi, lo, 0, 0, 0x1234, 0x5678),
        Ipv6Addr::new(0x2001, 0, 0x0808, 0x0808, 0, 0, !hi, !lo),
        Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0x5efe, hi, lo),
    ];
    // IPv4-compatible, where it is not `::` or `::1`.
    if u32::from(ip) > 1 {
        out.push(Ipv6Addr::new(0, 0, 0, 0, 0, 0, hi, lo));
    }
    out
}

/// Every way a resolver or a URL parser reads as `ip`: dotted, one number in decimal, octal or
/// hexadecimal, each part in octal or hexadecimal, the short forms, and each IPv6 address that
/// carries it, written plainly, upper case, bracketed or with a final dot.
fn spellings_v4(ip: Ipv4Addr) -> Vec<String> {
    let n = u32::from(ip);
    let [a, b, c, d] = ip.octets();
    let mut out = vec![
        ip.to_string(),
        format!("{ip}."),
        n.to_string(),
        format!("0{n:o}"),
        format!("0x{n:x}"),
        format!("0X{n:X}"),
        format!("0{a:o}.0{b:o}.0{c:o}.0{d:o}"),
        format!("0x{a:x}.0x{b:x}.0x{c:x}.0x{d:x}"),
        format!("{a}.{}", n & 0x00ff_ffff),
        format!("{a}.{b}.{}", n & 0xffff),
        format!("0{a:o}.{b}.{c}.{d}"),
    ];
    for v6 in carrying(ip) {
        out.extend(spellings_v6(v6));
    }
    out
}

fn spellings_v6(ip: Ipv6Addr) -> Vec<String> {
    let plain = ip.to_string();
    let s = ip.segments();
    let long = s.iter().map(|one| format!("{one:x}")).collect::<Vec<_>>().join(":");
    vec![
        plain.clone(),
        plain.to_ascii_uppercase(),
        format!("[{plain}]"),
        long,
        format!("{plain}%en0"),
    ]
}

/// Whether `spelled`, as the decision reads a host, is an address literal it parses, and which.
fn literal(spelled: &str) -> Option<IpAddr> {
    spelled.strip_suffix('.').unwrap_or(spelled).parse::<IpAddr>().ok()
}

fn special_use_address() -> impl Strategy<Value = IpAddr> {
    prop_oneof![
        refused_v4().prop_map(IpAddr::V4),
        refused_v6().prop_map(IpAddr::V6),
    ]
}

proptest! {
    #![proptest_config(config())]

    /// **Every spelling of an address a chat never reaches is refused outright**, never asked
    /// about, unless that exact address and port is listed: dotted, decimal, octal, hex, short,
    /// mapped and every IPv6 form that carries it, bracketed, upper case, with a final dot or a
    /// zone. The same holds of this machine's own addresses, by a name of this Mac, and of what
    /// a name resolves to. A private address is asked about for a chat unless listed, never
    /// carried, and is special-use for a confined extension in every form that carries it.
    #[test]
    fn every_encoding_of_a_special_use_address_is_refused(
        (address, any_address) in prop_oneof![
            3 => special_use_address().prop_map(|ip| (ip, false)),
            1 => any::<IpAddr>().prop_map(|ip| (ip, true)),
        ],
        own_is_it in any::<bool>(),
        port in port(),
        listed in proptest::collection::vec((listed_host(), by()), 0..5),
        exactly in prop_oneof![3 => Just(None), 1 => by().prop_map(Some)],
        other_port in port(),
        defaults in defaults(),
        private in private_v4(),
        anything in any::<bool>(),
    ) {
        // The address is refused as itself, or is this machine's own.
        let own: Vec<IpAddr> = if own_is_it || any_address { vec![address] } else { vec![IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7))] };
        prop_assume!(refused_address(address, &own));
        let mut entries = listed.clone();
        // The same address on another port is never this one.
        if other_port != port {
            entries.push((exact_entry(address, other_port), By::You));
        }
        if let Some(by) = exactly {
            entries.push((exact_entry(address, port), by));
        }
        let reach = if anything { Reach::anything().and(entries) } else { Reach::of(entries) };
        let spellings = match address {
            IpAddr::V4(ip) => spellings_v4(ip),
            IpAddr::V6(ip) => spellings_v6(ip),
        };
        for spelled in &spellings {
            for decision in [
                reach.decide(spelled, port, &defaults, &own),
                reach.decide_by_name(spelled, port, &defaults, &own),
            ] {
                let let_on = literal(spelled).is_some_and(|ip| reach.lists_exactly(ip, port));
                if decision.carries() {
                    prop_assert!(let_on, "{spelled} on {port} ({address}) carried as {decision:?}");
                } else {
                    prop_assert!(matches!(decision, Decision::Refused(_)), "{spelled} on {port} ({address}): {decision:?}");
                }
            }
        }
        // What a name resolves to is held to the same check, at every connect.
        let resolved = vec![SocketAddr::new(address, port)];
        let reached = egress::reachable("rebound.example.com", port, &|_, _| resolved.clone(), &reach, &own);
        prop_assert_eq!(reached.is_empty(), !reach.lists_exactly(address, port));
        // Every name of this Mac is this machine.
        for name in ["localhost", "LOCALHOST.", "host.docker.internal", "Kubernetes.Docker.Internal"] {
            let decision = reach.decide_by_name(name, port, &defaults, &own);
            prop_assert!(!matches!(decision, Decision::Ask), "{name}: {decision:?}");
        }

        // A private address: asked about for a chat, never carried, unless listed.
        let unlisted = Reach::of(listed);
        for spelled in spellings_v4(private) {
            let decision = unlisted.decide(&spelled, port, &defaults, &[]);
            prop_assert!(!decision.carries(), "{spelled} carried as {decision:?}");
        }
        prop_assert!(special_use(IpAddr::V4(private)));
        for v6 in carrying(private) {
            prop_assert!(special_use(IpAddr::V6(v6)), "{v6} carries {private}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 4. The hosts matcher.
// ---------------------------------------------------------------------------------------------

fn labels(name: &str) -> Vec<&str> {
    name.split('.').collect()
}

/// `text` with each letter's case chosen by `flips`.
fn recased(text: &str, flips: &[bool]) -> String {
    text.chars()
        .zip(flips.iter().cycle())
        .map(|(c, up)| if *up { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() })
        .collect()
}

/// A host built to sit near a listed name: before it, after it, glued to it with no dot.
fn near(name: String) -> impl Strategy<Value = String> {
    (
        "[a-z.-]{0,6}",
        "[a-z.-]{0,4}",
        proptest::sample::select(vec!["", ".", "-"]),
        any::<bool>(),
    )
        .prop_map(move |(before, after, glue, dot)| {
            let host = format!("{before}{glue}{name}{after}");
            if dot { format!("{host}.") } else { host }
        })
}

proptest! {
    #![proptest_config(config())]

    /// **A listed host matches only at a label boundary, in any case.** `*.S` matches a name
    /// only where it ends in the whole labels of `S` with at least one label before them, and
    /// matches every such well-formed name; a name without a wildcard matches only itself; an
    /// address literal never matches a wildcard. Changing the case of either side changes
    /// nothing, and neither does a final dot on the host.
    #[test]
    fn the_hosts_matcher_respects_label_boundaries(
        (suffix, host) in domain().prop_flat_map(|suffix| (Just(suffix.clone()), near(suffix))),
        wildcard in any::<bool>(),
        flips in proptest::collection::vec(any::<bool>(), 1..8),
        other_flips in proptest::collection::vec(any::<bool>(), 1..8),
        ip in any::<IpAddr>(),
    ) {
        let entry = if wildcard { format!("*.{suffix}") } else { suffix.clone() };
        let matched = allows(std::slice::from_ref(&entry), &host);
        let bare = host.strip_suffix('.').unwrap_or(&host).to_ascii_lowercase();
        let (theirs, mine) = (labels(&bare), labels(&suffix));
        let ends_in_whole_labels = theirs.len() > mine.len() && theirs[theirs.len() - mine.len()..] == mine[..];
        if wildcard {
            if matched {
                prop_assert!(ends_in_whole_labels, "{entry} matched {host} across a label");
            }
            let well_formed = theirs.iter().all(|label| !label.is_empty());
            if ends_in_whole_labels && well_formed {
                prop_assert!(matched, "{entry} missed {host}");
            }
        } else {
            prop_assert_eq!(matched, bare == suffix, "{} against {}", entry, host);
        }
        // Case on either side, never.
        let recased_entry = recased(&entry, &flips);
        let recased_host = recased(&host, &other_flips);
        prop_assert_eq!(allows(&[recased_entry.clone()], &recased_host), matched, "{} against {}", recased_entry, recased_host);
        prop_assert_eq!(allows_on(&[recased_entry], &recased_host, 443, &[443]), matched);
        // A wildcard never names an address.
        let over = format!("*.{suffix}");
        prop_assert!(!allows(&[over], &ip.to_string()), "{} names {}", suffix, ip);
    }
}

// ---------------------------------------------------------------------------------------------
// 5. Rule order.
// ---------------------------------------------------------------------------------------------

/// A pool of hosts small enough that a generated list names some of them twice, at different
/// layers, and wildcards that cover others.
fn pooled() -> impl Strategy<Value = String> {
    proptest::sample::select(vec![
        "a.example.com",
        "b.example.com",
        "*.example.com",
        "a.example.com:443",
        "a.example.com:8080",
        "example.com",
        "*.test.example.com",
        "x.test.example.com",
        "10.0.0.5:6443",
        "127.0.0.1:5432",
        "[::1]:5432",
    ])
    .prop_map(str::to_owned)
}

fn pooled_query() -> impl Strategy<Value = (String, u16)> {
    (
        proptest::sample::select(vec![
            "a.example.com",
            "A.Example.com.",
            "b.example.com",
            "c.example.com",
            "example.com",
            "x.test.example.com",
            "10.0.0.5",
            "127.0.0.1",
            "::1",
            "localhost",
            "evil.com",
        ]),
        proptest::sample::select(vec![80u16, 443, 5432, 6443, 8080]),
    )
        .prop_map(|(host, port)| (host.to_owned(), port))
}

proptest! {
    #![proptest_config(config())]

    /// **The order a list is written in never changes a decision.** Shuffling the hosts a chat's
    /// reach lists, at whatever layers, and the hosts policy pins as never allowed, decides every
    /// connection the same, layer and all; shuffling the Allows given on a chat's board carries
    /// the same connections.
    #[test]
    fn permuting_rules_never_changes_a_decision(
        listed in proptest::collection::vec((pooled(), by()), 0..10)
            .prop_flat_map(|listed| (Just(listed.clone()), Just(listed).prop_shuffle())),
        never in proptest::collection::vec(pooled(), 0..3)
            .prop_flat_map(|never| (Just(never.clone()), Just(never).prop_shuffle())),
        allowed in proptest::collection::vec((pooled(), by()), 0..6)
            .prop_flat_map(|allowed| (Just(allowed.clone()), Just(allowed).prop_shuffle())),
        queries in proptest::collection::vec(pooled_query(), 1..8),
    ) {
        let pinned = |hosts: &[String]| hosts.iter().filter_map(|one| Host::parse(one).ok()).collect::<Vec<_>>();
        let one = Reach::of(listed.0.clone()).never(pinned(&never.0));
        let other = Reach::of(listed.1.clone()).never(pinned(&never.1));
        let heard = Arc::new(Mutex::new(Vec::new()));
        let (first, second) = (board(&heard), board(&heard));
        for (board, allowed) in [(&first, &allowed.0), (&second, &allowed.1)] {
            for (host, by) in allowed {
                if let Ok(host) = Host::parse(host) {
                    board.allow(&host, *by);
                }
            }
        }
        for (host, port) in &queries {
            prop_assert_eq!(
                one.decide_by_name(host, *port, &TUNNEL_PORTS, &[]),
                other.decide_by_name(host, *port, &TUNNEL_PORTS, &[]),
                "{}:{} with {:?} and {:?}", host, port, listed.0, listed.1
            );
            prop_assert_eq!(
                carries(first.hold(host, *port, &TUNNEL_PORTS)),
                carries(second.hold(host, *port, &TUNNEL_PORTS)),
                "{}:{} with {:?} and {:?}", host, port, allowed.0, allowed.1
            );
        }
    }
}

/// The regression case the permutation law found (T1.1-23): a host listed as an Open host and
/// taken live for the project decided by whichever came first, so the network record named it
/// `open` or `project` by the order the reach was given its list.
#[test]
fn a_host_listed_open_and_taken_live_for_the_project_decides_open_in_either_order() {
    let open_first = Reach::of(vec![
        ("a.example.com".to_owned(), By::Open),
        ("a.example.com".to_owned(), By::Project),
    ]);
    let project_first = Reach::of(vec![
        ("a.example.com".to_owned(), By::Project),
        ("a.example.com".to_owned(), By::Open),
    ]);
    for reach in [open_first, project_first] {
        assert_eq!(
            reach.decide("a.example.com", 443, &TUNNEL_PORTS, &[]),
            Decision::Open
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 6. One chat's grants and blocks, and another chat's decisions.
// ---------------------------------------------------------------------------------------------

/// What the person does to one chat while another runs.
#[derive(Debug, Clone)]
enum Act {
    Allow(String, By),
    AllowNext(String),
    KeepBlocked(String),
    Forget(String, By),
    Withdraw(u16),
    GiveBack(u16),
    Attempt(String, u16),
}

fn act() -> impl Strategy<Value = Act> {
    let port = proptest::sample::select(vec![80u16, 443, 5432, 6443, 8080]);
    prop_oneof![
        (pooled(), by()).prop_map(|(host, by)| Act::Allow(host, by)),
        pooled().prop_map(Act::AllowNext),
        pooled().prop_map(Act::KeepBlocked),
        (pooled(), by()).prop_map(|(host, by)| Act::Forget(host, by)),
        port.clone().prop_map(Act::Withdraw),
        port.clone().prop_map(Act::GiveBack),
        pooled_query().prop_map(|(host, port)| Act::Attempt(host, port)),
    ]
}

/// One chat as a start makes it: its reach compiled from its own list, and its own board.
struct Chat {
    reach: Reach,
    asks: Asks,
}

impl Chat {
    fn start(listed: &[(String, By)], heard: &Arc<Mutex<Vec<Heard>>>) -> Self {
        Self {
            reach: Reach::of(listed.to_vec()),
            asks: board(heard),
        }
    }

    /// What the chat's proxy answers a connection: decided, then held on its board where asked.
    fn answers(&self, host: &str, port: u16) -> (Decision, Option<Answer>) {
        let decision = self.reach.decide_by_name(host, port, &TUNNEL_PORTS, &[]);
        let held = (decision == Decision::Ask).then(|| self.asks.hold(host, port, &TUNNEL_PORTS));
        (decision, held)
    }

    fn act(&self, act: &Act) {
        let parsed = |host: &str| Host::parse(host).ok();
        match act {
            Act::Allow(host, by) => {
                if let Some(host) = parsed(host) {
                    self.asks.allow(&host, *by);
                }
            }
            Act::AllowNext(host) => {
                if let Some(host) = parsed(host) {
                    self.asks.allow_next(&host);
                }
            }
            Act::KeepBlocked(host) => {
                if let Some(host) = parsed(host) {
                    self.asks.keep_blocked(&host);
                }
            }
            Act::Forget(host, by) => {
                if let Some(host) = parsed(host) {
                    self.asks.forget(&host, *by);
                }
            }
            Act::Withdraw(port) => self.reach.withdraw(*port),
            Act::GiveBack(port) => self.reach.give_back(*port),
            Act::Attempt(host, port) => {
                self.answers(host, *port);
            }
        }
    }
}

proptest! {
    #![proptest_config(config())]

    /// **Chat A's grants and blocks never change chat B's decisions**, even where both start
    /// from the same project's list: whatever the person allows, keeps blocked, allows next
    /// time, forgets, withdraws or gives back for A, and whatever A attempts, B answers each
    /// connection as it did before, and as a B beside no A would.
    #[test]
    fn one_chats_grants_never_change_another_chats_decisions(
        listed in proptest::collection::vec((pooled(), by()), 0..6),
        theirs in proptest::collection::vec(act(), 0..6),
        acts in proptest::collection::vec(act(), 0..16),
        queries in proptest::collection::vec(pooled_query(), 1..8),
    ) {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let a = Chat::start(&listed, &heard);
        let b = Chat::start(&listed, &heard);
        let alone = Chat::start(&listed, &heard);
        // B's own history first, the same for B and for a B with no A beside it.
        for one in &theirs {
            b.act(one);
            alone.act(one);
        }
        // B's answers are read on copies, so reading them uses up nothing of B's.
        let read = |chat: &Chat| -> Vec<Decision> {
            queries
                .iter()
                .map(|(host, port)| chat.reach.decide_by_name(host, *port, &TUNNEL_PORTS, &[]))
                .collect()
        };
        let before = read(&b);
        for one in &acts {
            a.act(one);
        }
        prop_assert_eq!(&read(&b), &before, "A did {:?}", acts);
        for (host, port) in &queries {
            prop_assert_eq!(b.answers(host, *port), alone.answers(host, *port), "{}:{} after A did {:?}", host, port, acts);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 7. Allow once, under interleaved connections.
// ---------------------------------------------------------------------------------------------

/// Waits until `done` holds, failing the case past a generous bound rather than hanging.
fn until(done: impl Fn() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(20);
    while !done() {
        if Instant::now() > end {
            return false;
        }
        std::thread::yield_now();
    }
    true
}

proptest! {
    #![proptest_config(config())]

    /// **Allow next time · once lets exactly one attempt on**, however attempts interleave with
    /// it: several connections to its target held, arriving or landing as it lands, and
    /// connections to other targets beside them. Exactly one attempt at its target is allowed,
    /// its use is told once, and none at another target takes it. Allowed again, one at a time,
    /// each lets exactly one more attempt on.
    #[test]
    fn allow_once_is_consumed_exactly_once_under_interleaving(
        attempts in 1usize..6,
        others in 0usize..3,
        settle in 0usize..6,
        again in 0usize..4,
        port in proptest::sample::select(vec![443u16, 8080]),
    ) {
        let target = format!("once.example.com:{port}");
        let heard: Arc<Mutex<Vec<Heard>>> = Arc::new(Mutex::new(Vec::new()));
        let told = Arc::clone(&heard);
        let asks = Arc::new(Asks::timed(
            Arc::new(move |one| told.lock().unwrap().push(one)),
            Duration::from_secs(30),
            Duration::ZERO,
        ));
        let returned = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::new();
        let spawn = |host: &'static str, threads: &mut Vec<std::thread::JoinHandle<(bool, Answer)>>| {
            let (asks, returned) = (Arc::clone(&asks), Arc::clone(&returned));
            threads.push(std::thread::spawn(move || {
                let answer = asks.hold(host, port, &[port]);
                returned.fetch_add(1, Ordering::SeqCst);
                (host == "once.example.com", answer)
            }));
        };
        // Some attempts before the Allow lands, the rest racing it.
        for _ in 0..settle.min(attempts) {
            spawn("once.example.com", &mut threads);
        }
        for _ in 0..others {
            spawn("other.example.com", &mut threads);
        }
        let landing = {
            let asks = Arc::clone(&asks);
            let host = Host::parse(&target).unwrap();
            std::thread::spawn(move || asks.allow_next(&host))
        };
        for _ in settle.min(attempts)..attempts {
            spawn("once.example.com", &mut threads);
        }
        landing.join().unwrap();
        let total = attempts + others;
        prop_assert!(until(|| asks.next_attempts().is_empty()), "nobody took it");
        prop_assert!(until(|| asks.holding() + returned.load(Ordering::SeqCst) == total), "attempts never arrived");
        asks.give_up(&target);
        asks.give_up(&format!("other.example.com:{port}"));
        let answers: Vec<(bool, Answer)> = threads.into_iter().map(|one| one.join().unwrap()).collect();
        let allowed: Vec<&(bool, Answer)> = answers.iter().filter(|(_, answer)| carries(*answer)).collect();
        prop_assert_eq!(allowed.len(), 1, "{:?}", answers);
        prop_assert!(allowed[0].0, "another target took it: {:?}", answers);
        let used = |heard: &[Heard]| heard.iter().filter(|one| matches!(one, Heard::UsedNext(_))).count();
        prop_assert_eq!(used(&heard.lock().unwrap()), 1);

        // Allowed again, one at a time: each lets exactly one more on, and the next waits.
        let quick = Asks::new(Arc::new(|_| {}), &Timings::production()).asking_while(Arc::new(|| false));
        let host = Host::parse(&target).unwrap();
        for _ in 0..again {
            quick.allow_next(&host);
            prop_assert!(carries(quick.hold("once.example.com", port, &[port])));
            prop_assert_eq!(quick.hold("once.example.com", port, &[port]), Answer::NobodyToAsk);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 8. The decided host, the dialled host and the server name.
// ---------------------------------------------------------------------------------------------

/// A TLS 1.3 ClientHello in one record, naming `server` (RFC 8446 §4.1.2, RFC 6066 §3).
fn client_hello(server: &str) -> Vec<u8> {
    let name = server.as_bytes();
    let len = |n: usize| u16::try_from(n).unwrap().to_be_bytes();
    let mut sni = Vec::new();
    sni.extend(len(name.len() + 3));
    sni.push(0);
    sni.extend(len(name.len()));
    sni.extend(name);
    let mut extensions = vec![0, 0];
    extensions.extend(len(sni.len()));
    extensions.extend(sni);
    let mut hello = vec![3, 3];
    hello.extend([7u8; 32]);
    hello.push(0);
    hello.extend([0, 2, 0x13, 0x01]);
    hello.extend([1, 0]);
    hello.extend(len(extensions.len()));
    hello.extend(extensions);
    let mut handshake = vec![1];
    handshake.extend(&u32::try_from(hello.len()).unwrap().to_be_bytes()[1..]);
    handshake.extend(hello);
    let mut record = vec![0x16, 3, 1];
    record.extend(len(handshake.len()));
    record.extend(handshake);
    record
}

/// One local origin every carried name is mapped to: each connection's bytes, whole, once it
/// ends.
fn origin() -> (SocketAddr, mpsc::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a local origin");
    let at = listener.local_addr().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let send = send.clone();
            std::thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let mut got = Vec::new();
                let _ = stream.read_to_end(&mut got);
                let _ = send.send(got);
            });
        }
    });
    (at, receive)
}

/// The hosts both proxies list, and the hosts a tunnel asks for.
const LISTED: [&str; 3] = ["a.test", "b.test", "*.w.test"];
const ASKED: [&str; 7] = ["a.test", "B.Test", "c.test", "x.w.test", "w.test", "evil.test", "a.test."];

/// The response head a proxy answers a `CONNECT` with, and the stream after it.
fn connect(proxy: u16, host: &str) -> (String, TcpStream) {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, proxy)).expect("the proxy");
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(stream, "CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n\r\n").unwrap();
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => break,
        }
    }
    (String::from_utf8_lossy(&head).into_owned(), stream)
}

/// One proxy under the law, with what it was started on.
struct Under {
    proxy: Proxy,
    names: Arc<Names>,
    confined: bool,
}

fn under(confined: bool, at: SocketAddr) -> Under {
    let names = Names::new();
    for name in ["a.test", "b.test", "x.w.test", "w.test", "c.test", "evil.test"] {
        names.origin(name, 443, at);
    }
    let hosts: Vec<String> = LISTED.iter().map(|one| (*one).to_owned()).collect();
    let timings = Timings::production();
    let mut serving = if confined {
        let serving = Serving::confined(
            hosts,
            Arc::new(ProviderAddresses::resolving(Vec::new(), names.resolver())),
            &timings,
        );
        // A question is being answered, so the confined proxy carries.
        serving.asking.as_ref().unwrap().store(1, Ordering::SeqCst);
        serving
    } else {
        Serving::of(Reach::open(hosts), &timings)
    };
    serving.names = Some(Arc::clone(&names));
    Under {
        proxy: Proxy::serving(serving).expect("a proxy"),
        names,
        confined,
    }
}

/// `name` as the seam spells it.
fn spelled(name: &str) -> String {
    name.strip_suffix('.').unwrap_or(name).to_ascii_lowercase()
}

#[test]
fn the_decided_host_is_the_dialled_host_and_the_sni() {
    let (at, bytes) = origin();
    let proxies = [under(false, at), under(true, at)];
    let reach = Reach::open(LISTED.iter().map(|one| (*one).to_owned()).collect());
    let mut runner = proptest::test_runner::TestRunner::new(config());
    let strategy = (
        0..proxies.len(),
        proptest::sample::select(ASKED.to_vec()),
        prop_oneof![
            2 => Just(None),
            1 => proptest::sample::select(ASKED.to_vec()).prop_map(Some),
        ],
    );
    runner
        .run(&strategy, |(which, host, server)| {
            let under = &proxies[which];
            let server = server.unwrap_or(host);
            let (lookups, dials, hits) = (
                under.names.lookups().len(),
                under.names.dials().len(),
                under.names.origin_hits().len(),
            );
            let decided = reach.decide(host, 443, &TUNNEL_PORTS, &[]);
            let (head, mut stream) = connect(under.proxy.port(), host);
            if !decided.carries() {
                prop_assert!(!head.starts_with("HTTP/1.1 200"), "{host} not listed, yet {head}");
                prop_assert_eq!(under.names.lookups().len(), lookups, "{} was looked up", host);
                prop_assert_eq!(under.names.dials().len(), dials);
                return Ok(());
            }
            prop_assert!(head.starts_with("HTTP/1.1 200"), "{host}: {head}");
            let hello = client_hello(server);
            stream.write_all(&hello).unwrap();
            let _ = stream.shutdown(Shutdown::Write);
            let mut rest = Vec::new();
            let _ = stream.read_to_end(&mut rest);
            let got = bytes
                .recv_timeout(Duration::from_secs(10))
                .expect("the origin heard the connection end");
            // Looked up, dialled and reached as the host decided, and nothing else.
            let looked: Vec<String> = under.names.lookups()[lookups..].iter().map(|one| one.name.clone()).collect();
            prop_assert_eq!(looked, vec![spelled(host)], "for {}", host);
            prop_assert_eq!(under.names.dials()[dials..].to_vec(), vec![at]);
            prop_assert_eq!(under.names.origin_hits()[hits..].to_vec(), vec![(spelled(host), 443)]);
            // The server name: a confined proxy carries the hello only where it names the host.
            let names_it = spelled(server) == spelled(host);
            if under.confined && !names_it {
                prop_assert!(got.is_empty(), "{host} carried a hello naming {server}");
            } else {
                prop_assert_eq!(got, hello, "{} naming {}", host, server);
            }
            Ok(())
        })
        .unwrap();
    // The chats' proxies saw no name but the ones decided.
    let decided: BTreeSet<String> = ASKED
        .iter()
        .filter(|one| reach.decide(one, 443, &TUNNEL_PORTS, &[]).carries())
        .map(|one| spelled(one))
        .collect();
    for under in &proxies {
        for lookup in under.names.lookups() {
            assert!(decided.contains(&lookup.name), "{} was looked up", lookup.name);
        }
    }
}
