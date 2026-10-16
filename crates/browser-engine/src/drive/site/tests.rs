// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::site;

#[test]
fn a_site_is_the_registrable_domain_of_a_web_address() {
    assert_eq!(
        site("https://mail.example.co.uk/inbox").as_deref(),
        Some("example.co.uk")
    );
    assert_eq!(
        site("http://www.Example.com:8080/").as_deref(),
        Some("example.com")
    );
    assert_eq!(
        site("http://www.fixture.test/a").as_deref(),
        Some("fixture.test")
    );
}

#[test]
fn an_ip_or_localhost_is_its_own_site_by_host_whatever_its_port() {
    assert_eq!(site("http://127.0.0.1:3000/").as_deref(), Some("127.0.0.1"));
    assert_eq!(site("https://localhost/").as_deref(), Some("localhost"));
    assert_eq!(site("http://localhost:8080/").as_deref(), Some("localhost"));
    assert_eq!(site("http://[::1]:8080/x").as_deref(), Some("::1"));
}

#[test]
fn an_address_that_is_not_a_web_address_is_on_no_site() {
    for address in [
        "about:blank",
        "data:text/html,hi",
        "blob:http://x.test/1",
        "file:///a",
        "",
    ] {
        assert_eq!(site(address), None, "{address}");
    }
}
