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

/// The first range of the vendored snapshot that is `len` bits long, as its first address.
fn first_of_length(len: u32) -> String {
    SNAPSHOT
        .lines()
        .filter(|line| !line.starts_with('#') && !line.contains(':'))
        .find_map(|line| {
            let (net, bits) = line.split_once('/')?;
            (bits.parse() == Ok(len)).then(|| net.to_owned())
        })
        .unwrap_or_else(|| panic!("no /{len} in the snapshot"))
}

#[test]
fn the_vendored_snapshot_is_whole_and_holds_the_azure_addresses_github_serves_from() {
    let lines: Vec<&str> = SNAPSHOT
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect();
    assert!(lines.len() > 50, "{} ranges", lines.len());
    assert!(
        lines.iter().all(|line| Range::parse(line).is_some()),
        "a line of the snapshot is no range"
    );
    assert_eq!(snapshot().len(), lines.len());
    // The four ranges the first round held are still held.
    for held in [
        "192.30.252.0/22",
        "140.82.112.0/20",
        "143.55.64.0/20",
        "2a0a:a440::/29",
    ] {
        assert!(lines.contains(&held), "{held}");
    }
}

#[test]
fn an_azure_address_of_github_from_the_snapshot_is_refused_as_a_literal_and_not_carried() {
    let forge = denial(&[]);
    for len in [32, 28] {
        let at = first_of_length(len);
        assert!(forge.refuses(&at, 443), "/{len}: {at}");
        assert!(forge.refuses(&at, 22), "/{len}: {at}:22");
        assert!(!forge.carries("mirror.example", ip(&at)), "/{len}: {at}");
        // A name that resolves to it is not carried either.
        assert!(!forge.carries("anything.wildcard.example", ip(&at)), "{at}");
    }
}

#[test]
fn a_numeric_spelling_of_a_forge_address_is_the_address() {
    let forge = denial(&[]);
    // 140.82.112.3, as `getaddrinfo` reads each.
    for host in [
        "2354212867",
        "0x8c527003",
        "0X8C527003",
        "0214.0122.0160.03",
        "0x8c.82.112.3",
        "140.82.28675",
        "140.5402627",
        "140.82.112.3.",
    ] {
        assert!(forge.refuses(host, 443), "{host}");
    }
    // Others are only others.
    for host in [
        "2354216963",
        "0x8c528003",
        "0214.0122.0200.03",
        "1.2.3.4.5",
        "08",
        "0x1g",
    ] {
        assert!(!forge.refuses(host, 443), "{host}");
    }
}

#[test]
fn a_nat64_literal_is_the_ipv4_address_it_carries() {
    let forge = denial(&[]);
    for host in [
        "[64:ff9b::8c52:7003]",
        "64:ff9b::8c52:7003",
        "[64:ff9b::140.82.112.3]",
        "[::ffff:140.82.112.3]",
    ] {
        assert!(forge.refuses(host, 443), "{host}");
        assert!(
            !forge.carries("mirror.example", ip(host.trim_matches(['[', ']']))),
            "{host}"
        );
    }
    assert!(!forge.refuses("[64:ff9b::5db8:d822]", 443), "93.184.216.34");
    // Only the /96 is NAT64: the same bytes under another prefix are another address.
    assert!(!forge.refuses("[64:ff9c::8c52:7003]", 443));
    assert!(!forge.refuses("[64:ff9b:1::8c52:7003]", 443));
}

#[test]
fn a_subdomain_is_refused_whatever_its_case_and_with_a_final_dot() {
    let forge = denial(&[]);
    for host in [
        "API.GITHUB.COM",
        "Evil.GitHub.com.",
        "x.Y.GITHUBUSERCONTENT.com.",
        "FOO.GitLab.Com",
        "Api.GHE.corp.EXAMPLE.",
        "[2A0A:A440::1]",
    ] {
        assert!(forge.refuses(host, 443), "{host}");
    }
    // The exempt names too are matched in any case, with a final dot.
    for host in ["CodeLoad.GitHub.com.", "OBJECTS.githubusercontent.COM"] {
        assert!(!forge.refuses(host, 443), "{host}");
        assert!(forge.refuses(host, 22), "{host}:22");
    }
}

#[test]
fn a_name_that_looks_like_an_exempt_one_is_not_exempt() {
    let forge = denial(&[]);
    // Under a forge host: refused as any name is.
    for host in ["xcodeload.github.com", "xobjects.githubusercontent.com"] {
        assert!(forge.refuses(host, 443), "{host}");
    }
    // Under another's: no forge name, so carried as a tunnel, but never believed to be exempt
    // for what it says inside.
    for host in [
        "codeload.github.com.evil.example",
        "objects.githubusercontent.com.evil.example",
    ] {
        assert!(!forge.refuses(host, 443), "{host}");
        assert!(
            forge.pins(host, &Hello::Named("github.com".into())),
            "{host}"
        );
        assert!(
            forge.pins(host, &Hello::Named("objects.githubusercontent.com".into())),
            "{host}"
        );
        assert!(!forge.pins(host, &Hello::Named(host.into())), "{host}");
    }
}

#[test]
fn an_exempt_tunnel_is_carried_only_where_its_sni_is_exactly_its_name() {
    let forge = denial(&[]);
    let named = |sni: &str| Hello::Named(sni.to_owned());
    for host in [
        "codeload.github.com",
        "release-assets.githubusercontent.com",
        "objects.githubusercontent.com",
    ] {
        assert!(!forge.pins(host, &named(host)), "{host}");
        assert!(
            !forge.pins(&host.to_uppercase(), &named(&format!("{host}."))),
            "{host}"
        );
        // A forge name inside the tunnel, the sign-in host and the `raw` host among them.
        for inside in [
            "github.com",
            "api.github.com",
            "raw.githubusercontent.com",
            "gist.github.com",
        ] {
            assert!(forge.pins(host, &named(inside)), "{host} -> {inside}");
        }
        // Another exempt name, or any other.
        assert!(forge.pins(host, &named("example.org")), "{host}");
        // Nothing, or nothing readable, inside an exempt tunnel.
        for hello in [Hello::NoName, Hello::Ech, Hello::Unreadable, Hello::NotTls] {
            assert!(forge.pins(host, &hello), "{host}: {hello:?}");
        }
    }
    // `objects` may not carry `raw`.
    assert!(forge.pins(
        "objects.githubusercontent.com",
        &named("raw.githubusercontent.com")
    ));
}

#[test]
fn a_tunnel_to_another_name_is_refused_where_its_sni_names_a_forge() {
    let forge = denial(&[]);
    let named = |sni: &str| Hello::Named(sni.to_owned());
    // An address no list holds, a wildcard name that resolves to it, saying its name inside.
    for inside in [
        "github.com",
        "GitHub.com.",
        "api.github.com",
        "evil.github.com",
        "objects.githubusercontent.com",
        "gitlab.com",
        "api.ghe.corp.example",
    ] {
        assert!(
            forge.pins("anything.wildcard.example", &named(inside)),
            "{inside}"
        );
    }
    // A forge address as the name inside is no name, and is held as an address.
    assert!(forge.pins("anything.wildcard.example", &named("140.82.112.3")));
    assert!(!forge.pins("anything.wildcard.example", &named("example.org")));
    assert!(!forge.pins("mirror.example", &named("mirror.example")));
    // A hello that cannot be read is never carried: split across records, it hides its name.
    assert!(forge.pins("mirror.example", &Hello::Unreadable));
    // No name, or a hidden one, to an address no list holds, is the residual; bytes that start
    // no handshake are no TLS.
    for hello in [Hello::NoName, Hello::Ech, Hello::NotTls] {
        assert!(!forge.pins("mirror.example", &hello), "{hello:?}");
    }
}

#[test]
fn a_hello_without_a_name_goes_to_no_address_the_lists_hold() {
    let forge = denial(&[("gitlab.com", "172.65.251.78")]);
    // The address check refuses the connection before any hello: nothing without an SNI reaches
    // a snapshotted or a resolved address, whatever name was asked.
    for at in ["140.82.112.3", "172.65.251.78", "[2a0a:a440::1]"] {
        let at = at.trim_matches(['[', ']']);
        assert!(!forge.carries("mirror.example", ip(at)), "{at}");
    }
    assert!(forge.pins("codeload.github.com", &Hello::NoName));
}

#[test]
fn a_failed_lookup_is_asked_again_and_not_held_as_empty() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    let forge = ForgeDenial::resolving(
        &["ghe.corp.example".to_owned()],
        Arc::new(move |_| {
            // Down for the first two questions, then up.
            if count.fetch_add(1, Ordering::SeqCst) < 2 {
                Vec::new()
            } else {
                vec![ip("10.1.2.3")]
            }
        }),
    );
    assert!(!forge.holds(ip("10.1.2.3")), "down: nothing known yet");
    // Still named, so still refused by name while its address is unknown.
    assert!(forge.refuses("ghe.corp.example", 443));
    assert!(forge.refuses("api.ghe.corp.example", 443));
    assert!(!forge.holds(ip("10.1.2.3")), "still down");
    assert!(
        forge.holds(ip("10.1.2.3")),
        "up: held at once, not after five minutes"
    );
    assert_eq!(asked.load(Ordering::SeqCst), 3);
    assert!(forge.holds(ip("10.1.2.3")));
    assert_eq!(
        asked.load(Ordering::SeqCst),
        3,
        "an answer is held for a while"
    );
}

#[test]
fn one_host_that_does_not_resolve_leaves_the_others_held() {
    let forge = ForgeDenial::resolving(
        &["ghe.down.example".to_owned(), "gitlab.com".to_owned()],
        Arc::new(|name| {
            if name == "gitlab.com" {
                vec![ip("172.65.251.78")]
            } else {
                Vec::new()
            }
        }),
    );
    assert!(forge.holds(ip("172.65.251.78")));
    assert!(forge.refuses("172.65.251.78", 443));
    assert!(forge.refuses("ghe.down.example", 443));
}
