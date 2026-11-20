// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::net::IpAddr;
use std::sync::Arc;

use super::*;

fn ip(text: &str) -> IpAddr {
    text.parse().unwrap()
}

/// A denial of the usual forge hosts and a recorded GHES host, whose names resolve as `table` says.
fn denial(table: &'static [(&'static str, &'static str)]) -> ForgeDenial {
    let bases: Vec<String> = crate::forgewire::denied_hosts(&[crate::forgewire::Repo::new(
        "ghe.corp.example",
        "o",
        "r",
        crate::forgewire::ForgeKind::GitHub,
    )
    .unwrap()]);
    ForgeDenial::resolving(
        &bases,
        Arc::new(move |name: &str| {
            table
                .iter()
                .filter(|(host, _)| *host == name)
                .map(|(_, at)| ip(at))
                .collect()
        }),
    )
}

#[test]
fn every_name_under_a_forge_host_is_refused_not_only_the_host() {
    let forge = denial(&[]);
    for host in [
        "github.com",
        "api.github.com",
        "evil.github.com",
        "a.b.github.com",
        "x.githubusercontent.com",
        "avatars.githubusercontent.com",
        "gitlab.com",
        "foo.gitlab.com",
        "ghe.corp.example",
        "api.ghe.corp.example",
        "GitHub.COM.",
    ] {
        assert!(forge.refuses(host, 443), "{host}");
    }
}

#[test]
fn a_name_that_only_ends_like_a_forge_host_is_not_one() {
    let forge = denial(&[]);
    for host in ["notgithub.com", "github.com.evil.example", "example.org"] {
        assert!(!forge.refuses(host, 443), "{host}");
    }
}

#[test]
fn the_three_download_only_names_are_carried_on_443() {
    let forge = denial(&[]);
    for host in [
        "codeload.github.com",
        "release-assets.githubusercontent.com",
        "objects.githubusercontent.com",
    ] {
        assert!(!forge.refuses(host, 443), "{host}");
    }
}

#[test]
fn nothing_else_under_a_forge_host_is_tunnelled_not_even_ssh_gist_raw_or_uploads() {
    let forge = denial(&[]);
    for host in [
        "ssh.github.com",
        "altssh.gitlab.com",
        "gist.github.com",
        "raw.githubusercontent.com",
        "uploads.github.com",
        "raw.ghe.corp.example",
        "uploads.ghe.corp.example",
        "gist.ghe.corp.example",
    ] {
        for port in [80, 443, 8443] {
            assert!(forge.refuses(host, port), "{host}:{port}");
        }
    }
}

#[test]
fn port_22_is_refused_to_every_forge_name_the_tunnelled_ones_too() {
    let forge = denial(&[]);
    for host in [
        "github.com",
        "ssh.github.com",
        "altssh.gitlab.com",
        "codeload.github.com",
        "gist.github.com",
        "gitlab.com",
        "ghe.corp.example",
        "x.ghe.corp.example",
    ] {
        assert!(forge.refuses(host, 22), "{host}:22");
    }
    // Another host's port 22 is not this denial's.
    assert!(!forge.refuses("example.org", 22));
}

#[test]
fn an_address_literal_in_a_published_range_is_refused() {
    let forge = denial(&[]);
    assert!(forge.refuses("140.82.112.3", 443));
    assert!(forge.refuses("192.30.255.113", 22));
    assert!(forge.refuses("143.55.64.1", 443));
    assert!(forge.refuses("[2a0a:a440::1]", 443));
    assert!(forge.refuses("::ffff:140.82.112.3", 443));
    assert!(!forge.refuses("93.184.216.34", 443));
}

#[test]
fn an_address_literal_that_a_forge_host_resolves_to_is_refused() {
    let forge = denial(&[
        ("gitlab.com", "172.65.251.78"),
        ("ghe.corp.example", "10.1.2.3"),
    ]);
    assert!(forge.refuses("172.65.251.78", 443));
    assert!(forge.refuses("10.1.2.3", 443));
    assert!(!forge.refuses("172.65.251.79", 443));
}

#[test]
fn an_allowed_name_that_resolves_to_a_forge_address_is_not_carried() {
    let forge = denial(&[("gitlab.com", "172.65.251.78")]);
    // `mirror.example` is a name nothing denies; where it resolves, the connection is not made.
    assert!(!forge.carries("mirror.example", ip("172.65.251.78")));
    assert!(!forge.carries("mirror.example", ip("140.82.112.4")));
    assert!(forge.carries("mirror.example", ip("93.184.216.34")));
}

#[test]
fn a_download_only_name_is_carried_at_the_ranges_github_serves_it_from() {
    let forge = denial(&[]);
    assert!(forge.carries("codeload.github.com", ip("140.82.112.10")));
    // Every other name is asked: a name that resolves into the forge's ranges is not carried.
    assert!(!forge.carries("ssh.github.com", ip("140.82.112.35")));
    // An address literal is no tunnelled name.
    assert!(!forge.carries("140.82.112.10", ip("140.82.112.10")));
}

#[test]
fn what_a_forge_host_resolves_to_is_held_for_a_while_and_then_asked_again() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    let forge = ForgeDenial::resolving(
        &["gitlab.com".to_owned()],
        Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            vec![ip("172.65.251.78")]
        }),
    );
    assert!(forge.holds(ip("172.65.251.78")));
    assert!(forge.holds(ip("172.65.251.78")));
    assert!(!forge.holds(ip("203.0.113.9")));
    assert_eq!(asked.load(Ordering::SeqCst), 1, "looked up once, then held");
}

#[test]
fn a_name_that_does_not_resolve_leaves_only_the_published_ranges() {
    let forge = ForgeDenial::resolving(&["gitlab.com".to_owned()], Arc::new(|_| Vec::new()));
    assert!(forge.holds(ip("140.82.112.3")));
    assert!(!forge.holds(ip("172.65.251.78")));
}

#[test]
fn the_ranges_match_by_prefix_only() {
    let range = Range::parse("140.82.112.0/20").unwrap();
    assert!(range.holds(ip("140.82.127.255")));
    assert!(!range.holds(ip("140.82.128.0")));
    assert!(!range.holds(ip("2a0a:a440::1")));
    let six = Range::parse("2a0a:a440::/29").unwrap();
    assert!(six.holds(ip("2a0a:a447:ffff::1")));
    assert!(!six.holds(ip("2a0a:a448::1")));
    assert!(Range::parse("1.2.3.4/33").is_none());
}
