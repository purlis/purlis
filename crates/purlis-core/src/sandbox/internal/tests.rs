// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a chat planned on another machine reaches of the networks its runner is in (#2445).

use std::net::{IpAddr, SocketAddr};

use super::{Entry, Internal, private_address};
use crate::sandbox::hosts::Network;
use crate::sandbox::nat64::{self, Nat64};
use crate::sandbox::policy::{self, Locks};

fn ip(text: &str) -> IpAddr {
    text.parse().expect("an address")
}

fn at(text: &str) -> SocketAddr {
    text.parse().expect("an address and port")
}

fn listing(entries: &[&str]) -> Internal {
    Internal::listing(
        entries
            .iter()
            .map(|entry| Entry::parse(entry).expect("an entry"))
            .collect(),
    )
}

fn refused(typed: &str) -> String {
    match Entry::parse(typed) {
        Ok(entry) => panic!("{typed} taken as {entry}"),
        Err(why) => why,
    }
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

/// Where this machine's addresses cannot be read, any address may be one of its own, so only
/// an entry naming that address and port exactly carries it.
#[test]
fn where_this_machines_addresses_cannot_be_read_only_an_exact_address_and_port_is_carried() {
    assert!(!listing(&["api.example.com"]).carries("api.example.com", at("203.0.113.7:443"), None));
    assert!(!listing(&["203.0.113.0/24"]).carries("api.example.com", at("203.0.113.7:443"), None));
    assert!(listing(&["203.0.113.7:443"]).carries("api.example.com", at("203.0.113.7:443"), None));
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

/// A range in `relayed-internal` (#2530): every address in it, on its port where it names one,
/// and nothing outside it.
#[test]
fn a_range_carries_every_address_in_it_and_nothing_more() {
    let range = listing(&["10.96.0.0/12"]);
    let on_a_port = listing(&["[fd00:10:96::/112]:443"]);

    assert!(range.carries("api.example", at("10.96.0.1:443"), Some(&[])));
    assert!(range.carries("api.example", at("10.111.255.254:6443"), Some(&[])));
    assert!(!range.carries("api.example", at("10.112.0.1:443"), Some(&[])));
    assert!(!range.carries("api.example", at("10.95.255.255:443"), Some(&[])));
    assert!(on_a_port.carries("api.example", at("[fd00:10:96::5]:443"), Some(&[])));
    assert!(!on_a_port.carries("api.example", at("[fd00:10:96::5]:80"), Some(&[])));
    assert!(!on_a_port.carries("api.example", at("[fd00:10:97::5]:443"), Some(&[])));
}

#[test]
fn a_range_lists_a_planned_address_in_it_and_never_a_name() {
    let range = listing(&["10.96.0.0/12"]);

    assert!(range.lists_entry("10.96.0.10:53", Some(&[])));
    assert!(!range.lists_entry("10.112.0.10:53", Some(&[])));
    assert!(!range.lists_entry("kube-dns.example:53", Some(&[])));
}

#[test]
fn a_range_is_written_as_its_network() {
    for (typed, written) in [
        ("10.96.0.0/12", "10.96.0.0/12"),
        ("10.96.0.0/12:443", "10.96.0.0/12:443"),
        ("fd00:10:96::/112", "[fd00:10:96::/112]"),
        ("[FD00:10:96::/112]:443", "[fd00:10:96::/112]:443"),
        ("10.96.0.1:443", "10.96.0.1:443"),
        ("*.svc.cluster.local", "*.svc.cluster.local"),
    ] {
        assert_eq!(Entry::parse(typed).expect("an entry").to_string(), written);
    }
}

#[test]
fn a_range_that_is_not_one_says_why() {
    assert!(refused("10.96.0.5/12").contains("10.96.0.0/12"));
    assert!(refused("10.96.0.0/33").contains("prefix"));
    assert!(refused("10.96.0.0/").contains("prefix"));
    assert!(refused("example.com/12").contains("range"));
    assert!(refused("10.96.0.0/12:0").contains("port"));
}

/// A range never takes in what no host may be (#2530): this machine, a link, a broadcast or
/// multicast group, a cloud metadata service, or an IPv6 form that carries an IPv4 address.
#[test]
fn a_range_that_takes_in_what_no_host_may_be_is_refused() {
    for typed in [
        "0.0.0.0/0",
        "[::/0]",
        "127.0.0.0/8",
        "126.0.0.0/7",
        "169.254.0.0/16",
        "100.100.0.0/16",
        "224.0.0.0/4",
        "255.255.255.255/32",
        "[fe80::/10]",
        "[fd00:ec2::/32]",
        "[64:ff9b::/96]",
        "[64:ff9b:1::/48]",
        "[::ffff:0:0/96]",
        "[2002::/16]",
        "[2001::/32]",
        "[2001:0:a00::/48]",
        "[3ffe:831f::/32]",
        "[3ffe:831f:a00::/48]",
        "[3ffe::/16]",
        "[2000::/3]",
    ] {
        assert!(
            refused(typed).contains("relayed chat never reaches"),
            "{typed}: {}",
            refused(typed)
        );
    }
}

/// An IPv4 address carried in this machine's own NAT64 prefix, at whatever length it uses, is
/// judged as that address (#2530, RFC 7050).
#[test]
fn an_address_this_machines_nat64_carries_from_a_private_network_is_held() {
    let ours = Nat64::new("2001:db8:122:344::".parse().unwrap(), 64).expect("a prefix");
    // 10.0.0.5 and 203.0.113.7 under 2001:db8:122:344::/64.
    let private = at("[2001:db8:122:344:a:0:500:0]:443");
    let public = at("[2001:db8:122:344:cb:71:700:0]:443");

    let held = listing(&[]).with_nat64(vec![ours]);
    assert!(!held.carries("db.example", private, Some(&[])));
    assert!(held.carries("api.example", public, Some(&[])));
    assert!(listing(&[]).carries("db.example", private, Some(&[])));
    assert!(
        !listing(&["*.google.internal"])
            .with_nat64(vec![ours])
            .carries(
                "metadata.google.internal",
                at("[2001:db8:122:344:a9:fea9:fe00:0]:80"),
                Some(&[])
            )
    );
}

/// What the runner's policy says reads this machine's own NAT64 prefixes.
#[test]
fn the_runners_own_nat64_prefix_is_read_from_the_machine() {
    let project = tempfile::tempdir().expect("a project");
    let ours = Nat64::new("2001:db8:122:344::".parse().unwrap(), 64).expect("a prefix");
    nat64::set_for_this_test(vec![ours]);
    let held = !Internal::of(project.path()).carries(
        "db.example",
        at("[2001:db8:122:344:a:0:500:0]:443"),
        Some(&[]),
    );
    nat64::set_for_this_test(Vec::new());

    assert!(held);
}

#[test]
fn an_address_nat64s_local_use_prefix_carries_at_48_is_private() {
    assert!(private_address(ip("64:ff9b:1:a00:0:500::")));
}

/// The runner's own address is in the network a range names (#2530), yet only an entry naming
/// that address and port exactly carries it: a range, a name or an address with no port never.
#[test]
fn a_range_of_the_runners_network_never_carries_the_runners_own_address() {
    let own = [Network::of(ip("10.244.1.5"), 24)];
    let pods = listing(&["10.244.0.0/16", "*.svc.cluster.local", "10.244.1.5"]);

    assert!(!pods.carries(
        "runner.svc.cluster.local",
        at("10.244.1.5:8080"),
        Some(&own)
    ));
    assert!(pods.carries("other.svc.cluster.local", at("10.244.2.9:443"), Some(&own)));
    assert!(!pods.lists_entry("10.244.1.5:8080", Some(&own)));
    assert!(pods.lists_entry("10.244.2.9:443", Some(&own)));
    let exact = listing(&["10.244.1.5:8080"]);
    assert!(exact.carries("runner.example", at("10.244.1.5:8080"), Some(&own)));
    assert!(!exact.carries("runner.example", at("10.244.1.5:22"), Some(&own)));
    assert!(exact.lists_entry("10.244.1.5:8080", Some(&own)));
}

/// An address under this machine's own NAT64 prefix is listed as the IPv4 address it carries
/// (#2530): an IPv4 range covers it, and an IPv6 range around the prefix covers nothing behind it.
#[test]
fn an_address_this_machines_nat64_carries_is_listed_as_the_ipv4_one() {
    let ours = Nat64::new("fd00:64:1::".parse().unwrap(), 96).expect("a prefix");
    let ula = listing(&["[fd00:64::/32]"]).with_nat64(vec![ours]);
    let services = listing(&["10.96.0.0/12"]).with_nat64(vec![ours]);
    // 10.0.0.5 and 10.96.0.1 under fd00:64:1::/96.
    assert!(!ula.carries("db.example", at("[fd00:64:1::a00:5]:443"), Some(&[])));
    assert!(ula.carries("v6.example", at("[fd00:64:2::5]:443"), Some(&[])));
    assert!(services.carries("api.example", at("[fd00:64:1::a60:1]:443"), Some(&[])));
    assert!(!services.carries("db.example", at("[fd00:64:1::a00:5]:443"), Some(&[])));
    assert!(services.lists_entry("[fd00:64:1::a60:1]:443", Some(&[])));
    assert!(!ula.lists_entry("[fd00:64:1::a00:5]:443", Some(&[])));
    // The runner's own IPv6 address under its NAT64 prefix is its own, whatever it carries.
    assert!(!services.carries(
        "me.example",
        at("[fd00:64:1::a60:1]:443"),
        Some(&[Network::host(ip("fd00:64:1::a60:1"))])
    ));
}

/// A Teredo address is judged as the server and the client IPv4 addresses it carries, the
/// client's with every bit flipped (#2678).
#[test]
fn a_teredo_address_carrying_an_internal_server_or_client_is_held() {
    // Server 203.0.113.7, client 10.0.0.5 (f5ff:fffa flipped).
    let client = at("[2001:0:cb00:7107::f5ff:fffa]:443");
    // Server 10.0.0.5, client 198.51.100.9 (39cc:9bf6 flipped).
    let server = at("[2001:0:a00:5::39cc:9bf6]:443");
    // Server 203.0.113.7, client 198.51.100.9.
    let public = at("[2001:0:cb00:7107::39cc:9bf6]:443");

    assert!(private_address(client.ip()) && private_address(server.ip()));
    assert!(!private_address(public.ip()));
    assert!(!listing(&[]).carries("db.example", client, Some(&[])));
    assert!(!listing(&[]).carries("db.example", server, Some(&[])));
    assert!(listing(&[]).carries("api.example", public, Some(&[])));
    // Client 169.254.169.254 (5601:5601 flipped) is never carried, whatever is listed.
    assert!(!listing(&["db.example"]).carries(
        "db.example",
        at("[2001:0:cb00:7107::5601:5601]:80"),
        Some(&[])
    ));
}

/// A Teredo address under the prefix used before RFC 4380, `3ffe:831f::/32`, is read as one under
/// `2001::/32` is: as the server and the client IPv4 addresses it carries (#2741).
#[test]
fn a_pre_rfc_teredo_address_carrying_an_internal_server_or_client_is_held() {
    // Server 203.0.113.7, client 10.0.0.5 (f5ff:fffa flipped).
    let client = at("[3ffe:831f:cb00:7107::f5ff:fffa]:443");
    // Server 10.0.0.5, client 198.51.100.9 (39cc:9bf6 flipped).
    let server = at("[3ffe:831f:a00:5::39cc:9bf6]:443");
    // Server 203.0.113.7, client 198.51.100.9.
    let public = at("[3ffe:831f:cb00:7107::39cc:9bf6]:443");

    assert!(private_address(client.ip()) && private_address(server.ip()));
    assert!(!private_address(public.ip()));
    assert!(!listing(&[]).carries("db.example", client, Some(&[])));
    assert!(!listing(&[]).carries("db.example", server, Some(&[])));
    assert!(listing(&[]).carries("api.example", public, Some(&[])));
    // Listed as the IPv4 address it carries, it is carried.
    assert!(listing(&["10.0.0.5"]).carries("db.example", client, Some(&[])));
    // Client 169.254.169.254 (5601:5601 flipped) is never carried, whatever is listed.
    assert!(!listing(&["db.example"]).carries(
        "db.example",
        at("[3ffe:831f:cb00:7107::5601:5601]:80"),
        Some(&[])
    ));
}

/// An ISATAP address, `::5efe:` or `::200:5efe:` before an IPv4 address in its interface
/// identifier, is judged as that IPv4 address as well as itself (#2678).
#[test]
fn an_isatap_address_carrying_an_internal_ipv4_one_is_held() {
    for typed in [
        "[2001:db8:1:2::5efe:a00:5]:443",
        "[2001:db8:1:2:200:5efe:a00:5]:443",
        "[2001:db8:1:2:200:5efe:c0a8:101]:443",
        // The group bit set, alone and with the universal bit.
        "[2001:db8:1:2:100:5efe:a00:5]:443",
        "[2001:db8:1:2:300:5efe:a00:5]:443",
    ] {
        assert!(private_address(at(typed).ip()), "{typed}");
        assert!(
            !listing(&[]).carries("db.example", at(typed), Some(&[])),
            "{typed}"
        );
    }
    for public in [
        "[2001:db8:1:2::5efe:cb00:7107]:443",
        "[2001:db8:1:2:100:5efe:cb00:7107]:443",
        "[2001:db8:1:2:300:5efe:cb00:7107]:443",
        // Any other byte where an ISATAP identifier's flags go is no ISATAP address.
        "[2001:db8:1:2:400:5efe:a00:5]:443",
    ] {
        assert!(
            listing(&[]).carries("api.example", at(public), Some(&[])),
            "{public}"
        );
    }
    // On a unique local prefix it is held as itself whatever IPv4 address it carries.
    assert!(!listing(&[]).carries("db.example", at("[fd00:1::5efe:cb00:7107]:443"), Some(&[])));
}

/// A Teredo or ISATAP address is covered by what lists the IPv4 address it carries, never by an
/// IPv6 range around it (#2678).
#[test]
fn a_tunnelled_address_is_listed_as_the_ipv4_one_it_carries() {
    let services = listing(&["10.0.0.0/8"]);
    let site = listing(&["[2001:db8:1:2::/64]"]);
    let isatap = at("[2001:db8:1:2::5efe:a00:5]:443");
    let teredo = at("[2001:0:cb00:7107::f5ff:fffa]:443");

    assert!(services.carries("db.example", isatap, Some(&[])));
    assert!(services.carries("db.example", teredo, Some(&[])));
    assert!(services.lists_entry("[2001:db8:1:2::5efe:a00:5]:443", Some(&[])));
    assert!(!site.carries("db.example", isatap, Some(&[])));
    assert!(!site.lists_entry("[2001:db8:1:2::5efe:a00:5]:443", Some(&[])));
    // A unique local ISATAP address needs its IPv6 network and the IPv4 address it carries.
    let ula = at("[fd00:1::5efe:a00:5]:443");
    assert!(!services.carries("db.example", ula, Some(&[])));
    assert!(!listing(&["[fd00:1::/64]"]).carries("db.example", ula, Some(&[])));
    assert!(listing(&["[fd00:1::/64]", "10.0.0.0/8"]).carries("db.example", ula, Some(&[])));
    // A listed name covers what it resolves to.
    assert!(listing(&["db.example"]).carries("db.example", isatap, Some(&[])));
}
