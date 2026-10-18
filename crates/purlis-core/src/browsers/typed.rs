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
        // A public site on a port nothing lists is held back and asked about, as the judge
        // does: an Allow of that host and port carries it.
        Verdict::Refused(crate::sandbox::browser_reach::Refusal::Port) => None,
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
    let authority = typed.split(['/', '?', '#']).next().unwrap_or_default();
    let host = match authority.strip_prefix('[') {
        Some(inside) => inside.split_once(']').map_or(inside, |(host, _)| host),
        None => authority
            .rsplit_once(':')
            .map_or(authority, |(host, _)| host),
    };
    host.parse::<std::net::IpAddr>()
        .is_ok_and(|ip| ip.is_loopback())
}

#[cfg(test)]
mod tests;
