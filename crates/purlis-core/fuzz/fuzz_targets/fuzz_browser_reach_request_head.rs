// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The browser reach proxy's request head** (T1.1-47, `sandbox/browser_reach.rs`): any bytes
//! the watched browser's page can make it send. The reach proxy reads a head as the egress proxy
//! does, refuses a plain request whose `Host` names another site, and judges the site the head
//! names; that site is the one the `Host` named, and it is written back as it is read.

#![no_main]

use libfuzzer_sys::fuzz_target;
use purlis_core::sandbox::browser_reach::Site;
use purlis_core::sandbox::fuzzing::{head_of, host_header_agrees, proxy_request_head};

/// A `Host` value's name and port, as a client writes one: an IPv6 address's colons are inside
/// its brackets, so its port is only after them.
fn name_and_port(named: &str) -> (&str, Option<&str>) {
    if let Some((inside, after)) = named.strip_prefix('[').and_then(|it| it.split_once(']')) {
        return (inside, after.strip_prefix(':'));
    }
    match named.rsplit_once(':') {
        Some((name, port)) => (name, Some(port)),
        None => (named, None),
    }
}

fuzz_target!(|stream: &[u8]| {
    let Some(head) = head_of(stream) else {
        return;
    };
    let Some(read) = proxy_request_head(head) else {
        return;
    };
    if !host_header_agrees(&read) {
        return;
    }
    let site = Site::of(&read.host, read.port);
    if let Some(named) = &read.host_header {
        let (name, port) = name_and_port(named);
        let port = port.and_then(|it| it.parse().ok()).unwrap_or(80);
        assert_eq!(Site::of(name, port), site, "{head:?}");
    }
    // The site judged is one a site's spelling reads back to.
    assert_eq!(
        Site::parse(&site.to_string()).as_ref(),
        Ok(&site),
        "{site:?}"
    );
});
