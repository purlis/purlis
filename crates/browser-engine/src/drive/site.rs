// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Which site a page is on (#2303): its registrable domain by the public suffix list (eTLD+1),
//! or, for an IP address or `localhost`, its host and port exactly. A page at an address that is
//! not a web address (an empty page, `data:`, `blob:`, `file:`) is on no site of its own.

use std::net::IpAddr;

use crate::WebAddress;

/// The site `address` is on, or `None` when it is not a web address.
pub(crate) fn site(address: &str) -> Option<String> {
    let address = WebAddress::parse(address).ok()?;
    let (scheme, rest) = address.as_str().split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?.to_ascii_lowercase();
    let (host, port) = match authority.strip_prefix('[') {
        Some(inside) => {
            let (host, after) = inside.split_once(']')?;
            (host.to_owned(), after.strip_prefix(':').map(str::to_owned))
        }
        None => match authority.rsplit_once(':') {
            Some((host, port)) => (host.to_owned(), Some(port.to_owned())),
            None => (authority.clone(), None),
        },
    };
    let host = host.trim_end_matches('.');
    if host.is_empty() {
        return None;
    }
    let exact = || {
        let port = port.clone().filter(|p| !p.is_empty()).unwrap_or_else(|| {
            if scheme.eq_ignore_ascii_case("https") {
                "443".into()
            } else {
                "80".into()
            }
        });
        format!("{host}:{port}")
    };
    if host.parse::<IpAddr>().is_ok() || host == "localhost" || host.ends_with(".localhost") {
        return Some(exact());
    }
    Some(psl::domain_str(host).map_or_else(exact, str::to_owned))
}

#[cfg(test)]
mod tests;
