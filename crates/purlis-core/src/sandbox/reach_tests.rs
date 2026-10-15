// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The decision module: a host and port, and the layers a chat's sandbox was compiled with, go
//! in; one decision comes out.

use std::net::{IpAddr, Ipv4Addr};

use super::reach::{By, Decision, Reach, Refused};

const HTTPS: &[u16] = &[443];

fn reach() -> Reach {
    Reach::of(vec![
        ("api.anthropic.com".to_owned(), By::Open),
        ("*.github.com".to_owned(), By::Open),
        ("10.0.0.5:6443".to_owned(), By::Open),
        ("grafana.example.com".to_owned(), By::Persona),
        ("pypi.example.com".to_owned(), By::You),
        ("one-off.example.com:8443".to_owned(), By::Chat),
    ])
}

fn decide(host: &str, port: u16) -> Decision {
    reach().decide(host, port, HTTPS, &[])
}

#[test]
fn a_host_is_decided_by_the_layer_that_lists_it() {
    assert_eq!(decide("api.anthropic.com", 443), Decision::Open);
    assert_eq!(decide("codeload.github.com", 443), Decision::Open);
    assert_eq!(decide("10.0.0.5", 6443), Decision::Open);
    assert_eq!(decide("grafana.example.com", 443), Decision::Persona);
    assert_eq!(decide("pypi.example.com", 443), Decision::Allowed(By::You));
    assert_eq!(
        decide("one-off.example.com", 8443),
        Decision::Allowed(By::Chat)
    );
}

#[test]
fn a_host_nothing_lists_is_asked_about() {
    assert_eq!(decide("example.org", 443), Decision::Ask);
    // Listed, on another port: a host is held to the ports it is listed for.
    assert_eq!(decide("api.anthropic.com", 22), Decision::Ask);
    assert_eq!(decide("10.0.0.5", 443), Decision::Ask);
}

#[test]
fn what_could_never_be_a_host_is_refused_not_asked() {
    assert!(matches!(
        decide("intranet", 443),
        Decision::Refused(Refused::NeverAHost(_))
    ));
    assert!(matches!(
        decide("*.example.org", 443),
        Decision::Refused(Refused::NeverAHost(_))
    ));
}

#[test]
fn this_machine_link_local_and_metadata_are_refused_as_local_addresses() {
    for host in [
        "127.0.0.1",
        "169.254.169.254",
        "0.0.0.0",
        "::1",
        "fe80::1",
        "100.100.100.200",
    ] {
        assert_eq!(
            decide(host, 443),
            Decision::Refused(Refused::LocalAddress),
            "{host}"
        );
    }
    let own = [IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20))];
    assert_eq!(
        reach().decide("192.168.1.20", 443, HTTPS, &own),
        Decision::Refused(Refused::LocalAddress)
    );
    // Another machine on the same network is a host like any other.
    assert_eq!(
        reach().decide("192.168.1.21", 443, HTTPS, &own),
        Decision::Ask
    );
}

/// An address literal is held to the check in every IPv6 spelling that carries a local
/// IPv4 address, and as this machine's own IPv6 address.
#[test]
fn every_ipv6_spelling_of_a_local_address_is_refused() {
    for host in [
        "::",
        "::ffff:127.0.0.1",
        "::ffff:169.254.169.254",
        "64:ff9b::a9fe:a9fe",
        "2002:7f00:1::1",
        "fd00:ec2::254",
    ] {
        assert_eq!(
            decide(host, 443),
            Decision::Refused(Refused::LocalAddress),
            "{host}"
        );
    }
    let own = ["2001:db8::7".parse::<IpAddr>().unwrap()];
    assert_eq!(
        reach().decide("2001:db8::7", 443, HTTPS, &own),
        Decision::Refused(Refused::LocalAddress)
    );
}

#[test]
fn a_local_address_is_carried_only_where_that_exact_address_and_port_is_listed() {
    let listed = Reach::of(vec![
        ("127.0.0.1".to_owned(), By::Open),
        ("127.0.0.1:5432".to_owned(), By::You),
    ]);
    assert_eq!(
        listed.decide("127.0.0.1", 5432, HTTPS, &[]),
        Decision::Allowed(By::You)
    );
    // Listed without its port: not the exact address and port, so still refused.
    assert_eq!(
        listed.decide("127.0.0.1", 443, HTTPS, &[]),
        Decision::Refused(Refused::LocalAddress)
    );
}

#[test]
fn the_exception_is_asked_of_a_resolved_address_by_its_address_and_port() {
    let listed = Reach::of(vec![("127.0.0.1:5432".to_owned(), By::You)]);
    let local: IpAddr = "127.0.0.1".parse().expect("an address");
    assert!(listed.lists_exactly(local, 5432));
    assert!(!listed.lists_exactly(local, 5433));
    assert!(!reach().lists_exactly(local, 443));
}

#[test]
fn the_first_layer_to_list_a_host_decides_it() {
    let twice = Reach::of(vec![
        ("api.example.com".to_owned(), By::Open),
        ("api.example.com".to_owned(), By::You),
    ]);
    assert_eq!(
        twice.decide("api.example.com", 443, HTTPS, &[]),
        Decision::Open
    );
}

#[test]
fn hosts_are_every_listed_entry_once_in_order() {
    let twice = Reach::of(vec![
        ("a.example.com".to_owned(), By::Open),
        ("b.example.com".to_owned(), By::You),
        ("a.example.com".to_owned(), By::Chat),
    ]);
    assert_eq!(twice.hosts(), ["a.example.com", "b.example.com"]);
    assert_eq!(
        Reach::open(vec!["x.example.com".to_owned()]).hosts(),
        ["x.example.com"]
    );
}

#[test]
fn each_decision_has_the_word_the_record_keeps() {
    assert_eq!(Decision::Open.word(), "open");
    assert_eq!(Decision::Persona.word(), "persona");
    assert_eq!(Decision::Allowed(By::You).word(), "you");
    assert_eq!(Decision::Allowed(By::Chat).word(), "chat");
    assert_eq!(Decision::Ask.word(), "ask");
    assert_eq!(Decision::Refused(Refused::LocalAddress).word(), "refused");
    assert!(Decision::Open.carries());
    assert!(Decision::Allowed(By::Chat).carries());
    assert!(!Decision::Ask.carries());
    assert!(!Decision::Refused(Refused::LocalAddress).carries());
}

/// #1709: everyone in the project's Allow, taken live by a running chat, is kept by the scope
/// the person chose, `project`, not by `open`, the layer it compiles to from the next start.
#[test]
fn everyone_in_the_projects_allow_taken_live_is_kept_as_project() {
    let by = By::from(super::grant::Level::Project);
    let decision = Decision::by(by);
    assert_eq!(decision.word(), "project");
    assert!(decision.carries());
    assert_eq!(By::from(super::grant::Level::You), By::You);
    assert_eq!(By::from(super::grant::Level::Chat), By::Chat);
}

#[test]
fn a_host_policy_pins_never_allowed_is_refused_never_asked() {
    let pinned = reach().never(vec![super::hosts::Host::parse("*.paste.example").unwrap()]);
    assert_eq!(
        pinned.decide("drop.paste.example", 443, HTTPS, &[]),
        Decision::Refused(Refused::Policy)
    );
    assert_eq!(pinned.decide("example.org", 443, HTTPS, &[]), Decision::Ask);
}

// qw384b: a service on this Mac reached by name, through the proxy, for a client that never
// proxies this machine's own addresses (Claude Code's NO_PROXY, Go's built-in bypass). The
// names are a fixed list, never looked up (#2097 review).

/// What `localport::reach_entries` lists for a port granted for this chat.
fn granted(port: u16) -> Reach {
    Reach::of(
        super::localport::reach_entries(port)
            .into_iter()
            .map(|entry| (entry, By::Chat))
            .collect(),
    )
}

#[test]
fn the_names_of_this_mac_are_a_fixed_list() {
    assert_eq!(super::localport::THIS_MAC, "this-mac.internal");
    assert_eq!(
        super::localport::NAMES_OF_THIS_MAC,
        [
            "this-mac.internal",
            "kubernetes.docker.internal",
            "host.docker.internal"
        ]
    );
}

#[test]
fn each_name_of_this_mac_is_carried_to_a_granted_port_at_the_layer_that_granted_it() {
    let reach = granted(50795);
    for name in super::localport::NAMES_OF_THIS_MAC {
        assert_eq!(
            reach.decide_by_name(name, 50795, HTTPS, &[]),
            Decision::Allowed(By::Chat),
            "{name}"
        );
        // Pinned to this machine's loopback, on that port alone: never looked up.
        assert_eq!(
            reach.address_of(name, 50795),
            Some(std::net::SocketAddr::from((Ipv4Addr::LOCALHOST, 50795))),
            "{name}"
        );
    }
    // Any spelling of a name: a trailing dot, capitals.
    assert_eq!(
        reach.decide_by_name("This-Mac.Internal.", 50795, HTTPS, &[]),
        Decision::Allowed(By::Chat)
    );
    assert_eq!(reach.address_of("example.org", 443), None);
}

#[test]
fn a_name_of_this_mac_on_a_port_not_granted_is_refused_as_this_machine_never_asked() {
    let held = granted(50795);
    for name in super::localport::NAMES_OF_THIS_MAC {
        for port in [50796, 443, 22] {
            assert_eq!(
                held.decide_by_name(name, port, HTTPS, &[]),
                Decision::Refused(Refused::LocalAddress),
                "{name}:{port}"
            );
            assert_eq!(held.address_of(name, port), None, "{name}:{port}");
        }
        // A chat with no port at all: the same, and never asked about as a host.
        assert_eq!(
            reach().decide_by_name(name, 50795, HTTPS, &[]),
            Decision::Refused(Refused::LocalAddress)
        );
    }
}

#[test]
fn any_other_name_is_decided_as_before_in_a_chat_that_holds_a_port() {
    let held = granted(6443);
    for (name, port) in [
        ("example.org", 6443),
        ("evil.example", 6443),
        ("kubernetes.docker.internal.example.com", 6443),
        ("localhost.example", 6443),
    ] {
        assert_eq!(
            held.decide_by_name(name, port, HTTPS, &[]),
            held.decide(name, port, HTTPS, &[]),
            "{name}"
        );
        // Connected to where it resolves at connect, as before, held to the local-address check.
        assert_eq!(held.address_of(name, port), None, "{name}");
    }
}

#[test]
fn a_withdrawn_port_is_carried_no_longer_by_name_or_by_address() {
    let reach = granted(50795);
    let carrying = reach.clone();
    reach.withdraw(50795);
    // Every copy of it: the proxy's own is a copy of the one compiled.
    for one in [&reach, &carrying] {
        assert_eq!(
            one.decide_by_name(super::localport::THIS_MAC, 50795, HTTPS, &[]),
            Decision::Refused(Refused::LocalAddress)
        );
        assert_eq!(
            one.decide("127.0.0.1", 50795, HTTPS, &[]),
            Decision::Refused(Refused::LocalAddress)
        );
        assert!(!one.lists_exactly(IpAddr::V4(Ipv4Addr::LOCALHOST), 50795));
        assert_eq!(one.address_of(super::localport::THIS_MAC, 50795), None);
    }
}
