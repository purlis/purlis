// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An address the person types for a browser** (#2176): read in the window before anything
//! is sent, so whatever is not an http or https address is refused in words, and an address
//! the browser's reach would refuse outright says why before it loads.
//!
//! - **http and https only.** Any other scheme (`file:`, `javascript:`, `data:`, `chrome:`,
//!   `about:`, `blob:`, `view-source:`) is refused, and so is an address with a name or
//!   password before its host, or with a space, a control character or a backslash. The
//!   engine refuses each again on its own (`purlis_browser_engine::WebAddress`).
//! - **No scheme is https**, but for a port of this machine (`127.0.0.1:5173`), which is http, as
//!   a dev server serves it. Written `localhost:5173`, it is refused: a port of this machine is
//!   reached by its address, never by a name.
//! - **What the reach refuses whatever the person answers** (this machine but a port they
//!   allowed, a private or reserved address nothing lists, a name of this machine, a host
//!   policy pins as never allowed) is refused here in the reach proxy's own words. A site that
//!   is merely new to the project is not refused: the browser holds it back and asks.

use purlis_browser_engine::{NotWebAddress, WebAddress};
use purlis_host::protocol::browsing::BrowserReach;

use super::judge::site_of;
use crate::sandbox::browser_reach::{Lane, Verdict, refusal_said};

/// **The address `typed` names**, whole, or why a browser does not go there (module docs).
pub fn address_of(typed: &str) -> Result<String, String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Err("Type an address to go to: an http or https one.".to_owned());
    }
    let whole = if typed.contains("://") {
        typed.to_owned()
    } else if let Some(scheme) = scheme_of(typed) {
        return Err(other_scheme(scheme));
    } else if this_machine(typed) {
        format!("http://{typed}")
    } else {
        format!("https://{typed}")
    };
    WebAddress::parse(&whole)
        .map(|address| address.as_str().to_owned())
        .map_err(|why| match why {
            NotWebAddress::Scheme(scheme) => other_scheme(&scheme),
            NotWebAddress::NoScheme | NotWebAddress::NoHost => {
                "That address names no host: type one such as https://example.com.".to_owned()
            }
            NotWebAddress::UserInfo => "That address carries a name or password before its \
                                        host, so purlis does not go to it: type the host alone."
                .to_owned(),
            NotWebAddress::Unreadable => "That address carries a space, a control character or a \
                                          backslash: type it without."
                .to_owned(),
        })
}

/// **Why a browser reaching `reach` does not go to `address` whatever the person answers**, in
/// the reach proxy's words, or nothing where it goes or is a site new to the project to ask
/// about. Where this machine's own addresses cannot be read, nothing is carried, and it says so.
pub fn refused_by(reach: &BrowserReach, address: &str) -> Option<String> {
    let (host, port) = site_of(address)?;
    let tunnel = address
        .get(..6)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https:"));
    let lane = Lane::of(super::reach_of(Some(reach)).ok()?.0);
    let own = crate::sandbox::hosts::own_addresses_read();
    refused_on(&lane, &host, port, tunnel, own.as_deref())
}

fn refused_on(
    lane: &Lane,
    host: &str,
    port: u16,
    tunnel: bool,
    own: Option<&[std::net::IpAddr]>,
) -> Option<String> {
    match lane.decide_here(host, port, tunnel, own) {
        Verdict::Refused(why) => Some(format!("{}.", refusal_said(host, port, &why))),
        _ => None,
    }
}

fn other_scheme(scheme: &str) -> String {
    format!(
        "A browser goes only to http and https addresses, and this one is {scheme}: type one \
         that starts with https:// or http://."
    )
}

/// The scheme `typed` starts with where it names one with no `//` (`javascript:`, `data:`,
/// `mailto:`), and not a host and its port (`example.com:8080`).
fn scheme_of(typed: &str) -> Option<&str> {
    let (scheme, rest) = typed.split_once(':')?;
    let named = scheme
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    let port = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let a_port = !port.is_empty() && port.chars().all(|c| c.is_ascii_digit());
    (named && !a_port).then_some(scheme)
}

/// Whether `typed`, written with no scheme, names a port of this machine by its address.
fn this_machine(typed: &str) -> bool {
    let host = typed.split(['/', '?', '#']).next().unwrap_or_default();
    host.starts_with("127.") || host.starts_with("[::1]")
}

#[cfg(test)]
mod tests {
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
}
