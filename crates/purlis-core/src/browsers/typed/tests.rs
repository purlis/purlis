// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_host::protocol::session::{Layer, Listed};

#[test]
fn an_http_or_https_address_is_taken_whole() {
    assert_eq!(
        address_of(" https://example.com/a?b#c "),
        Ok("https://example.com/a?b#c".to_owned())
    );
    assert_eq!(
        address_of("HTTP://example.com"),
        Ok("HTTP://example.com".to_owned())
    );
}

#[test]
fn an_address_with_no_scheme_is_https_and_a_port_of_this_machine_is_http() {
    assert_eq!(
        address_of("example.com/docs"),
        Ok("https://example.com/docs".to_owned())
    );
    assert_eq!(
        address_of("example.com:8443"),
        Ok("https://example.com:8443".to_owned())
    );
    assert_eq!(
        address_of("127.0.0.1:5173/app"),
        Ok("http://127.0.0.1:5173/app".to_owned())
    );
    assert_eq!(address_of("[::1]:3000"), Ok("http://[::1]:3000".to_owned()));
}

#[test]
fn every_other_scheme_is_refused_in_words() {
    for typed in [
        "file:///etc/passwd",
        "chrome://settings",
        "javascript:alert(1)",
        "javascript:1;alert(1)",
        "JavaScript://%0aalert(1)",
        "data:text/html,<b>hi</b>",
        "about:blank",
        "blob:https://example.com/x",
        "view-source:https://example.com",
        "mailto:a@example.com",
        "ftp://example.com",
    ] {
        let said = address_of(typed).expect_err(typed);
        assert!(
            said.starts_with("A browser goes only to http and https addresses"),
            "{typed}: {said}"
        );
    }
}

#[test]
fn an_address_that_hides_its_host_or_carries_a_space_is_refused() {
    assert!(
        address_of("https://good.example@evil.example/")
            .unwrap_err()
            .contains("name or password")
    );
    assert!(
        address_of("https://exa mple.com")
            .unwrap_err()
            .contains("space")
    );
    assert!(
        address_of("https://example.com\\@evil.example")
            .unwrap_err()
            .contains("backslash")
    );
    assert!(address_of("https://").unwrap_err().contains("no host"));
    assert!(address_of("   ").unwrap_err().contains("Type an address"));
}

fn lane(listed: &[&str]) -> Lane {
    let reach = BrowserReach {
        listed: listed
            .iter()
            .map(|host| Listed {
                host: (*host).to_owned(),
                layer: Layer::You,
            })
            .collect(),
        ..BrowserReach::default()
    };
    Lane::of(super::super::reach_of(Some(&reach)).expect("reads").0)
}

const OWN: &[std::net::IpAddr] = &[std::net::IpAddr::V4(std::net::Ipv4Addr::new(
    192, 168, 1, 20,
))];

#[test]
fn this_machine_a_private_address_or_a_name_of_this_machine_is_refused_before_it_loads() {
    let lane = lane(&[]);
    for (host, port, tunnel) in [
        ("127.0.0.1", 5173, false),
        ("localhost", 5173, false),
        ("10.0.0.5", 443, true),
        ("169.254.169.254", 80, false),
        ("192.168.1.20", 80, false),
        ("2130706433", 80, false),
    ] {
        let said = refused_on(&lane, host, port, tunnel, Some(OWN)).expect(host);
        assert!(
            said.starts_with("purlis's reach proxy does not carry"),
            "{said}"
        );
    }
}

#[test]
fn a_site_new_to_the_project_is_not_refused_it_asks_and_a_listed_port_goes() {
    assert_eq!(
        refused_on(&lane(&[]), "example.com", 443, true, Some(OWN)),
        None
    );
    assert_eq!(
        refused_on(
            &lane(&["127.0.0.1:5173"]),
            "127.0.0.1",
            5173,
            false,
            Some(OWN)
        ),
        None
    );
}

#[test]
fn with_this_machines_addresses_unknown_nothing_is_carried() {
    assert!(
        refused_on(&lane(&[]), "example.com", 443, true, None)
            .unwrap()
            .contains("cannot be read")
    );
}

#[test]
fn a_public_site_on_a_port_nothing_lists_is_asked_about_not_refused() {
    assert_eq!(
        refused_on(&lane(&[]), "example.com", 8443, true, Some(OWN)),
        None
    );
    assert_eq!(
        refused_on(&lane(&[]), "example.com", 8080, false, Some(OWN)),
        None
    );
    assert!(
        refused_on(&lane(&[]), "10.0.0.5", 8080, false, Some(OWN)).is_some(),
        "a private address is refused on every port"
    );
}

#[test]
fn only_an_address_of_this_machine_is_http_with_no_scheme_never_a_name_that_starts_like_one() {
    assert_eq!(
        address_of("127.evil.example/x"),
        Ok("https://127.evil.example/x".to_owned())
    );
    assert_eq!(address_of("127.0.0.1"), Ok("http://127.0.0.1".to_owned()));
    assert_eq!(
        address_of("192.168.1.5:80"),
        Ok("https://192.168.1.5:80".to_owned())
    );
}
