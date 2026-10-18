// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where an IPv4 address sits in a NAT64 address** (#2530): RFC 6052's layouts for a prefix
//! of 32, 40, 48, 56, 64 or 96 bits, NAT64's local-use prefix `64:ff9b:1::/48` (RFC 8215) read
//! at every length it may be used at, and this machine's own NAT64 prefixes, read from its
//! resolver's answer for `ipv4only.arpa` (RFC 7050).
//!
//! An IPv6 address under a NAT64 prefix reaches the IPv4 address it carries, so whatever judges
//! an address judges it as that one ([`super::hosts::carried_v4`], [`super::internal`]).

use std::net::{Ipv4Addr, Ipv6Addr};

/// The prefix lengths RFC 6052 defines.
const LENGTHS: [u8; 6] = [32, 40, 48, 56, 64, 96];

/// The lengths NAT64's local-use prefix may be used at (RFC 8215 §4): its own /48 and longer.
const LOCAL_USE_LENGTHS: [u8; 4] = [48, 56, 64, 96];

/// NAT64's local-use prefix, `64:ff9b:1::/48`.
const LOCAL_USE: Ipv6Addr = Ipv6Addr::new(0x64, 0xff9b, 1, 0, 0, 0, 0, 0);

/// The two addresses `ipv4only.arpa` names (RFC 7050 §2.2).
const WELL_KNOWN_V4: [Ipv4Addr; 2] = [Ipv4Addr::new(192, 0, 0, 170), Ipv4Addr::new(192, 0, 0, 171)];

/// **A NAT64 prefix**: its bits and its length, one RFC 6052 defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nat64 {
    prefix: Ipv6Addr,
    length: u8,
}

impl Nat64 {
    /// The prefix `prefix`/`length`, where `length` is one RFC 6052 defines; the bits past it
    /// are dropped.
    pub fn new(prefix: Ipv6Addr, length: u8) -> Option<Self> {
        LENGTHS.contains(&length).then(|| Self {
            prefix: masked(prefix, length),
            length,
        })
    }

    /// **The IPv4 address `ip` carries through this prefix**, where it is under it.
    pub fn carried(&self, ip: Ipv6Addr) -> Option<Ipv4Addr> {
        (masked(ip, self.length) == self.prefix).then(|| at(ip, self.length))
    }

    /// **The prefixes an answer for `ipv4only.arpa` says** (RFC 7050 §3): each length at which
    /// `answer` carries one of the two addresses that name stands for.
    pub fn of_answer(answer: Ipv6Addr) -> Vec<Self> {
        LENGTHS
            .iter()
            .filter(|length| WELL_KNOWN_V4.contains(&at(answer, **length)))
            .filter_map(|length| Self::new(answer, *length))
            .collect()
    }
}

/// **The IPv4 addresses an address in NAT64's local-use prefix may carry**: one for each length
/// the prefix may be used at, leaving out a reading in `0.0.0.0/8`, which no NAT64 translates;
/// none for an address outside it.
pub fn local_use(ip: Ipv6Addr) -> Vec<Ipv4Addr> {
    if masked(ip, 48) != LOCAL_USE {
        return Vec::new();
    }
    let mut carried: Vec<Ipv4Addr> = Vec::new();
    for length in LOCAL_USE_LENGTHS {
        let one = at(ip, length);
        if one.octets()[0] != 0 && !carried.contains(&one) {
            carried.push(one);
        }
    }
    carried
}

/// `ip` with every bit past the first `length` cleared.
fn masked(ip: Ipv6Addr, length: u8) -> Ipv6Addr {
    let mask = u128::MAX.checked_shl(128 - u32::from(length)).unwrap_or(0);
    Ipv6Addr::from(u128::from(ip) & mask)
}

/// The IPv4 address at the place RFC 6052 §2.2 puts one after a prefix of `length` bits: the
/// 32 bits after the prefix, skipping bits 64 to 71.
fn at(ip: Ipv6Addr, length: u8) -> Ipv4Addr {
    let bytes = ip.octets();
    let start = usize::from(length / 8);
    let mut carried = [0u8; 4];
    let mut index = start;
    for byte in &mut carried {
        if index == 8 {
            index += 1;
        }
        *byte = bytes.get(index).copied().unwrap_or(0);
        index += 1;
    }
    Ipv4Addr::from(carried)
}

/// **This machine's own NAT64 prefixes**, as its resolver answers `ipv4only.arpa` (RFC 7050):
/// none where it answers with no IPv6 address or not at all. A reading is kept for five
/// minutes. A test build answers what its test set, never the network.
pub fn of_this_machine() -> Vec<Nat64> {
    #[cfg(any(test, feature = "test-support"))]
    {
        TEST_PREFIXES.with(|held| held.borrow().clone())
    }
    #[cfg(not(any(test, feature = "test-support")))]
    {
        read_now_and_then()
    }
}

/// How long a reading of this machine's prefixes is kept.
#[cfg(not(any(test, feature = "test-support")))]
const READ_AGAIN_AFTER: std::time::Duration = std::time::Duration::from_secs(300);

#[cfg(not(any(test, feature = "test-support")))]
fn read_now_and_then() -> Vec<Nat64> {
    use std::sync::Mutex;
    use std::time::Instant;
    static READ: Mutex<Option<(Instant, Vec<Nat64>)>> = Mutex::new(None);
    let kept = READ
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if let Some((when, prefixes)) = kept
        && when.elapsed() < READ_AGAIN_AFTER
    {
        return prefixes;
    }
    let prefixes = read();
    *READ
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        Some((Instant::now(), prefixes.clone()));
    prefixes
}

/// What this machine's resolver answers for `ipv4only.arpa`, now.
#[cfg(not(any(test, feature = "test-support")))]
fn read() -> Vec<Nat64> {
    use std::net::{IpAddr, ToSocketAddrs};
    let Ok(found) = ("ipv4only.arpa", 0).to_socket_addrs() else {
        return Vec::new();
    };
    let mut prefixes: Vec<Nat64> = Vec::new();
    for answer in found {
        if let IpAddr::V6(answer) = answer.ip() {
            for one in Nat64::of_answer(answer) {
                if !prefixes.contains(&one) {
                    prefixes.push(one);
                }
            }
        }
    }
    prefixes
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static TEST_PREFIXES: std::cell::RefCell<Vec<Nat64>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// **A test's NAT64 prefixes** for this thread, in place of this machine's: what
/// [`of_this_machine`] answers until another call. Only in a test build.
#[cfg(any(test, feature = "test-support"))]
pub fn set_for_this_test(prefixes: Vec<Nat64>) {
    TEST_PREFIXES.with(|held| *held.borrow_mut() = prefixes);
}

#[cfg(test)]
mod tests;
