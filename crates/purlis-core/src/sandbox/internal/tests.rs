// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a chat planned on another machine reaches of the networks its runner is in (#2445).

use std::net::{IpAddr, SocketAddr};

use super::{Internal, private_address};
use crate::sandbox::hosts::{Host, Network};
use crate::sandbox::policy::{self, Locks};

fn ip(text: &str) -> IpAddr {
    text.parse().expect("an address")
}

fn at(text: &str) -> SocketAddr {
    text.parse().expect("an address and port")
}

fn listing(hosts: &[&str]) -> Internal {
    Internal::listing(
        hosts
            .iter()
            .map(|host| Host::parse(host).expect("a host"))
            .collect(),
    )
}

#[test]
fn a_private_a_cluster_and_a_link_local_address_are_private() {
    for one in [
        "10.96.0.1",
        "172.16.0.5",
        "172.31.255.254",
        "192.168.1.10",
        "100.64.0.1",
        "169.254.169.254",
        "fd12:3456::1",
        "fe80::1",
        "::ffff:10.0.0.5",
    ] {
        assert!(private_address(ip(one)), "{one}");
    }
}

#[test]
fn a_public_address_is_not_private() {
    for one in [
        "203.0.113.7",
        "172.32.0.1",
        "100.128.0.1",
        "2001:db8::1",
        "8.8.8.8",
    ] {
        assert!(!private_address(ip(one)), "{one}");
    }
}

#[test]
fn a_public_address_is_carried() {
    let carried = listing(&[]).carries("api.example.com", at("203.0.113.7:443"), Some(&[]));

    assert!(carried);
}

#[test]
fn a_private_address_is_carried_only_where_this_machines_policy_lists_it() {
    let none = listing(&[]);
    let by_name = listing(&["*.svc.cluster.local"]);
    let by_address = listing(&["10.96.0.1:443"]);

    assert!(!none.carries(
        "kubernetes.default.svc.cluster.local",
        at("10.96.0.1:443"),
        Some(&[])
    ));
    assert!(by_name.carries(
        "kubernetes.default.svc.cluster.local",
        at("10.96.0.1:443"),
        Some(&[])
    ));
    assert!(by_address.carries("api.example.com", at("10.96.0.1:443"), Some(&[])));
    assert!(!by_address.carries("api.example.com", at("10.96.0.1:6443"), Some(&[])));
}

#[test]
fn an_address_of_this_machine_is_held_as_a_private_one() {
    let own = [Network::host(ip("203.0.113.9"))];

    assert!(!listing(&[]).carries("me.example.com", at("203.0.113.9:22"), Some(&own)));
    assert!(listing(&["203.0.113.9:22"]).carries(
        "me.example.com",
        at("203.0.113.9:22"),
        Some(&own)
    ));
}

#[test]
fn a_metadata_service_is_never_carried_whatever_policy_lists() {
    let wide = listing(&["*.google.internal"]);

    assert!(!wide.carries(
        "metadata.google.internal",
        at("169.254.169.254:80"),
        Some(&[])
    ));
}

/// The loopback is held by the services the runner gives (`super::super::plan::Here::given`).
#[test]
fn the_loopback_is_left_to_the_services_this_machine_gives() {
    assert!(listing(&[]).carries("localhost", at("127.0.0.1:5432"), Some(&[])));
}

#[test]
fn where_this_machines_addresses_cannot_be_read_only_what_policy_lists_is_carried() {
    let listed = listing(&["api.example.com"]);

    assert!(!listed.carries("other.example.com", at("203.0.113.7:443"), None));
    assert!(listed.carries("api.example.com", at("203.0.113.7:443"), None));
}

/// Only `127.0.0.1` and `::1` are the services a chat is given: the rest of `127/8` is held.
#[test]
fn the_rest_of_the_loopback_network_is_held_as_internal() {
    for one in [
        "127.0.0.2:5432",
        "127.255.255.254:22",
        "[::ffff:127.0.0.1]:5432",
    ] {
        assert!(
            !listing(&[]).carries("localhost", at(one), Some(&[])),
            "{one}"
        );
    }
    assert!(listing(&[]).carries("localhost", at("[::1]:5432"), Some(&[])));
}

/// The runner's own networks are its cluster's (#2445): every address on a network one of its
/// interfaces is on is held, whatever range it is in.
#[test]
fn an_address_on_a_network_of_this_machines_is_held_as_internal() {
    let own = [
        Network::of(ip("2600:1f14::42"), 64),
        Network::of(ip("198.51.100.20"), 24),
    ];

    for one in ["[2600:1f14::99]:443", "198.51.100.200:443"] {
        assert!(
            !listing(&[]).carries("node.example", at(one), Some(&own)),
            "{one}"
        );
    }
    for one in ["[2600:1f15::99]:443", "198.51.101.200:443"] {
        assert!(
            listing(&[]).carries("elsewhere.example", at(one), Some(&own)),
            "{one}"
        );
    }
}

#[test]
fn an_address_nat64_carries_from_a_private_network_is_private() {
    assert!(private_address(ip("64:ff9b:1::a00:5")));
    assert!(!private_address(ip("64:ff9b:1::cb00:7107")));
}

/// The runner's policy for relayed chats is a key of its own (ruled 2026-10-18): `hosts`
/// opens no internal address to them.
#[test]
fn only_the_policys_relayed_internal_list_opens_an_internal_address() {
    let project = tempfile::tempdir().expect("a project");
    policy::set_for_this_test(Locks::parse(
        r#"{"sandbox": {"hosts": ["*.corp.example"], "relayed-internal": ["10.96.0.1:443"]}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    ));
    let internal = Internal::of(project.path());

    assert!(internal.carries("kubernetes.default", at("10.96.0.1:443"), Some(&[])));
    assert!(!internal.carries("db.corp.example", at("10.0.0.9:5432"), Some(&[])));
    policy::set_for_this_test(Locks::none());
}

/// A policy narrowed while a relayed chat runs holds its next connection.
#[test]
fn the_runners_policy_is_read_again_at_every_connection() {
    let project = tempfile::tempdir().expect("a project");
    policy::set_for_this_test(Locks::parse(
        r#"{"sandbox": {"relayed-internal": ["10.96.0.1:443"]}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    ));
    let internal = Internal::of(project.path());
    assert!(internal.carries("kubernetes.default", at("10.96.0.1:443"), Some(&[])));

    policy::set_for_this_test(Locks::none());

    assert!(!internal.carries("kubernetes.default", at("10.96.0.1:443"), Some(&[])));
}
