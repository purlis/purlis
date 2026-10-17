// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Which site a page is on (#2303): its registrable domain by the public suffix list (eTLD+1),
//! or, for an IP address or `localhost`, its host exactly. A page at an address that is
//! not a web address (an empty page, `data:`, `blob:`, `file:`) is on no site of its own.

use std::net::IpAddr;

use crate::WebAddress;

/// The site `address` is on, or `None` when it is not a web address. As the browser's own
/// same-site rule, the port is no part of it.
pub(crate) fn site(address: &str) -> Option<String> {
    let address = WebAddress::parse(address).ok()?;
    let (_, rest) = address.as_str().split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?.to_ascii_lowercase();
    let host = match authority.strip_prefix('[') {
        Some(inside) => inside.split_once(']')?.0.to_owned(),
        None => authority
            .rsplit_once(':')
            .map_or(authority.as_str(), |(host, _)| host)
            .to_owned(),
    };
    let host = host.trim_end_matches('.');
    if host.is_empty() {
        return None;
    }
    if host.parse::<IpAddr>().is_ok() || host == "localhost" || host.ends_with(".localhost") {
        return Some(host.to_owned());
    }
    Some(psl::domain_str(host).unwrap_or(host).to_owned())
}

#[cfg(test)]
mod tests;
