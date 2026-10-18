// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where an IPv4 address sits in a NAT64 address (#2530, RFC 6052 and RFC 7050).

use std::net::{Ipv4Addr, Ipv6Addr};

use super::{Nat64, local_use};

fn v6(text: &str) -> Ipv6Addr {
    text.parse().expect("an IPv6 address")
}

fn v4(text: &str) -> Ipv4Addr {
    text.parse().expect("an IPv4 address")
}

/// RFC 6052 §2.4's own examples: 192.0.2.33 under each prefix length it defines.
const EXAMPLES: [(&str, u8, &str); 6] = [
    ("2001:db8::", 32, "2001:db8:c000:221::"),
    ("2001:db8:100::", 40, "2001:db8:1c0:2:21::"),
    ("2001:db8:122::", 48, "2001:db8:122:c000:2:2100::"),
    ("2001:db8:122:300::", 56, "2001:db8:122:3c0:0:221::"),
    ("2001:db8:122:344::", 64, "2001:db8:122:344:c0:2:2100:0"),
    ("2001:db8:122:344::", 96, "2001:db8:122:344::c000:221"),
];

#[test]
fn a_prefix_from_32_to_96_decodes_the_ipv4_address_it_carries() {
    for (prefix, length, address) in EXAMPLES {
        let nat64 = Nat64::new(v6(prefix), length).expect("a NAT64 prefix length");

        assert_eq!(
            nat64.carried(v6(address)),
            Some(v4("192.0.2.33")),
            "{prefix}/{length}"
        );
    }
}

#[test]
fn an_address_outside_the_prefix_carries_nothing_through_it() {
    let nat64 = Nat64::new(v6("2001:db8:122::"), 48).expect("a NAT64 prefix length");

    assert_eq!(nat64.carried(v6("2001:db8:123:c000:2:2100::")), None);
}

#[test]
fn only_the_lengths_rfc_6052_defines_are_a_prefix() {
    for length in [0, 31, 33, 72, 128] {
        assert!(Nat64::new(v6("2001:db8::"), length).is_none(), "/{length}");
    }
}

/// RFC 7050: the machine's resolver answers `ipv4only.arpa` with its NAT64's addresses for
/// 192.0.0.170 and 192.0.0.171, which say where its prefix ends.
#[test]
fn the_prefix_is_read_from_an_answer_for_ipv4only_arpa() {
    assert_eq!(
        Nat64::of_answer(v6("2001:db8:122:344:c0:0:aa00:0")),
        [Nat64::new(v6("2001:db8:122:344::"), 64).expect("a prefix")]
    );
    assert_eq!(
        Nat64::of_answer(v6("64:ff9b::c000:ab")),
        [Nat64::new(v6("64:ff9b::"), 96).expect("a prefix")]
    );
    assert!(Nat64::of_answer(v6("2001:db8::1")).is_empty());
}

/// NAT64's local-use prefix (RFC 8215) may be used at any length from /48: an address in it is
/// read at each, so wherever its NAT64 put the address it is found.
#[test]
fn an_address_in_the_local_use_prefix_is_read_at_every_length_it_may_be_used_at() {
    let at_48 = local_use(v6("64:ff9b:1:a00:0:500::"));
    let at_96 = local_use(v6("64:ff9b:1::a00:5"));

    assert!(at_48.contains(&v4("10.0.0.5")), "{at_48:?}");
    assert!(at_96.contains(&v4("10.0.0.5")), "{at_96:?}");
    assert!(local_use(v6("2001:db8::a00:5")).is_empty());
}

/// A NAT64 translates no address of `0.0.0.0/8`, so a reading at a length the address was not
/// written at, which lands there, is not one it carries.
#[test]
fn a_reading_in_the_unspecified_network_is_not_an_address_carried() {
    assert_eq!(local_use(v6("64:ff9b:1::cb00:7107")), [v4("203.0.113.7")]);
}
