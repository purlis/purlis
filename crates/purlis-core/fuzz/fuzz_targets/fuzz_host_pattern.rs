// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The host-pattern matcher** (T1.1-47): `egress::allows` and `allows_on`, which every
//! decision of the proxy asks, against the rules its docs state, and against `hosts::Host`, the
//! policy's own reading of an entry. Where the two say different things about one host, the
//! settings a person reads and the proxy that enforces them disagree.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use purlis_core::sandbox::egress::{allows, allows_on};
use purlis_core::sandbox::hosts::Host;

#[derive(Debug, Arbitrary)]
struct Asked {
    listed: Vec<String>,
    host: String,
    port: u16,
}

fuzz_target!(|asked: Asked| {
    let Asked { listed, host, port } = asked;
    let allowed = allows(&listed, &host);
    // Case and a final dot never change an answer.
    assert_eq!(
        allowed,
        allows(&listed, &host.to_ascii_uppercase()),
        "{host:?}"
    );
    let upper: Vec<String> = listed.iter().map(|it| it.to_ascii_uppercase()).collect();
    assert_eq!(allowed, allows(&upper, &host));
    if !host.ends_with('.') {
        assert_eq!(allowed, allows(&listed, &format!("{host}.")), "{host:?}");
    }
    let bare = host.strip_suffix('.').unwrap_or(&host).to_ascii_lowercase();
    if allowed {
        // Allowed only as listed, or under a listed wildcard: never the wildcard's own suffix,
        // and never an address under one.
        let why = listed.iter().any(|entry| {
            let entry = entry.to_ascii_lowercase();
            match entry.strip_prefix("*.") {
                Some(suffix) => {
                    bare.parse::<std::net::IpAddr>().is_err()
                        && bare.len() > suffix.len() + 1
                        && bare.ends_with(&format!(".{suffix}"))
                }
                None => entry == bare,
            }
        });
        assert!(why, "{host:?} under {listed:?}");
    }
    // An entry with no port, on a default port, is `allows`. A bracketed entry is an IPv6
    // address to `allows_on` alone, which refuses one that is not (`[]31`): no listed entry is
    // either, since `hosts::Host` refuses both, and refusing is the safe side.
    let portless: Vec<String> = listed
        .iter()
        .filter(|it| !it.contains([':', '[']))
        .cloned()
        .collect();
    assert_eq!(
        allows_on(&portless, &host, port, &[port]),
        allows(&portless, &host),
        "{host:?} under {portless:?}"
    );
    // The policy's reading and the proxy's agree: an entry `Host` accepts, written back as
    // `Host` writes it, allows exactly the hosts it covers.
    for entry in &listed {
        let Ok(policy) = Host::parse(entry) else {
            continue;
        };
        let written = policy.to_string();
        assert_eq!(Host::parse(&written).as_ref(), Ok(&policy), "{entry:?}");
        let Ok(asked) = Host::parse(&purlis_core::sandbox::egress::host_and_port(&host, port))
        else {
            continue;
        };
        if asked.to_string().starts_with("*.") {
            continue;
        }
        let dialled = asked.to_string();
        let dialled = dialled
            .rsplit_once(':')
            .map_or(dialled.as_str(), |(name, _)| name)
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        assert_eq!(
            policy.covers(&asked),
            allows_on(std::slice::from_ref(&written), &dialled, port, &[port]),
            "{written:?} and {host:?}:{port}"
        );
    }
});
