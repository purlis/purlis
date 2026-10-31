// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser opens only http and https pages: an address of any other scheme is refused before
//! the engine is asked to go there, and the refusal says why in words.

use purlis_browser_engine::WebAddress;

#[test]
fn an_http_or_https_address_is_accepted_as_written() {
    for address in [
        "http://localhost:5173/",
        "https://example.com/a/b?c=d#e",
        "HTTPS://Example.com",
        "http://127.0.0.1:8080",
        "https://[::1]:8443/x",
    ] {
        let parsed = WebAddress::parse(address).unwrap_or_else(|why| panic!("{address}: {why}"));
        assert_eq!(parsed.as_str(), address);
    }
}

#[test]
fn an_address_of_any_other_scheme_is_refused_with_its_scheme_named() {
    for (address, scheme) in [
        ("file:///etc/passwd", "file"),
        ("chrome://settings", "chrome"),
        ("javascript:alert(1)", "javascript"),
        ("data:text/html,<p>hi</p>", "data"),
        ("about:blank", "about"),
        ("ftp://example.com/", "ftp"),
        ("blob:https://example.com/1", "blob"),
        ("view-source:https://example.com", "view-source"),
        ("ws://example.com/socket", "ws"),
        ("mailto:someone@example.com", "mailto"),
    ] {
        let why = WebAddress::parse(address).expect_err(address).to_string();
        assert!(
            why.contains(&format!("`{scheme}`")) && why.contains("http and https"),
            "{address}: {why}"
        );
    }
}

#[test]
fn an_address_with_no_scheme_or_no_host_is_refused() {
    for address in [
        "",
        "example.com",
        "//example.com/",
        "http://",
        "https:///path",
        "http:example.com",
    ] {
        assert!(
            WebAddress::parse(address).is_err(),
            "{address:?} was accepted"
        );
    }
}

#[test]
fn an_address_carrying_whitespace_a_control_character_or_a_backslash_is_refused() {
    // Chrome strips or rewrites each of these, so what it would open is not what was checked.
    for address in [
        " https://example.com",
        "https://example.com/ a",
        "https://example.com/\n",
        "https://example.com/\u{7f}",
        "https:\\\\example.com",
        "java\tscript:alert(1)",
        "https://example.com/\u{0}",
    ] {
        assert!(
            WebAddress::parse(address).is_err(),
            "{address:?} was accepted"
        );
    }
}

#[test]
fn an_address_carrying_a_name_or_password_before_its_host_is_refused() {
    // What reads as the host before `@` is not the host the engine would open.
    for address in [
        "https://example.com@evil.test/",
        "https://user:secret@example.com/",
        "http://@example.com/",
    ] {
        let why = WebAddress::parse(address).expect_err(address);
        assert!(
            why.to_string().contains("name or password"),
            "{address}: {why}"
        );
    }
    // An `@` after the host is part of the path or query, and is fine.
    assert!(WebAddress::parse("https://example.com/people/@someone?to=a@b").is_ok());
}
