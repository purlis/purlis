// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The egress proxy's request line and headers** (T1.1-47): any bytes a sandboxed chat sends
//! the proxy's HTTP port. Beyond never panicking, a head the proxy carries names one host, the
//! one it judges and dials; and what it sends on is one head, with no second request after it and
//! none of the proxy's own headers.

#![no_main]

use libfuzzer_sys::fuzz_target;
use purlis_core::sandbox::fuzzing::{head_of, host_header_agrees, proxy_request_head};

/// Whether `host` is spelled as a URL's host is sent: a name of letters, digits, `-`, `.` and
/// `_`, or an address.
fn sent_as_a_host(host: &str) -> bool {
    host.parse::<std::net::IpAddr>().is_ok()
        || (!host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._".contains(&b)))
}

fuzz_target!(|stream: &[u8]| {
    let Some(head) = head_of(stream) else {
        return;
    };
    let Some(read) = proxy_request_head(head) else {
        return;
    };
    assert!(sent_as_a_host(&read.host), "judged {:?}", read.host);
    let Some(forward) = &read.forward else {
        assert!(read.host_header.is_none() && read.body.is_none());
        return;
    };
    // One head, ending where it says, so nothing after it is read as a second request.
    assert_eq!(forward.find("\r\n\r\n"), Some(forward.len() - 4), "{forward:?}");
    assert!(forward.ends_with("Connection: close\r\n\r\n"));
    let lines: Vec<&str> = forward.trim_end().split("\r\n").collect();
    // Origin form: the target's path, never its authority again.
    assert!(lines[0].split(' ').nth(1).is_some_and(|path| path.starts_with('/')));
    let named = |wanted: &str| {
        lines[1..]
            .iter()
            .filter(|line| {
                line.split_once(':')
                    .is_some_and(|(name, _)| name.eq_ignore_ascii_case(wanted))
            })
            .count()
    };
    assert_eq!(named("host"), 1);
    assert_eq!(named("proxy-authorization") + named("proxy-connection"), 0);
    assert_eq!(named("connection"), 1);
    assert_eq!(named("transfer-encoding"), 0);
    if host_header_agrees(&read) {
        // Past the Host check, the Host names the host judged: what the server is asked for is
        // what was allowed.
        let named = read.host_header.as_deref().unwrap_or_default();
        let name = named
            .rsplit_once(':')
            .filter(|(name, _)| !name.ends_with(']') || name.starts_with('['))
            .map_or(named, |(name, _)| name);
        let plain = |it: &str| {
            it.trim_start_matches('[')
                .trim_end_matches(']')
                .trim_end_matches('.')
                .to_ascii_lowercase()
        };
        assert_eq!(plain(name), plain(&read.host), "{head:?}");
    }
});
