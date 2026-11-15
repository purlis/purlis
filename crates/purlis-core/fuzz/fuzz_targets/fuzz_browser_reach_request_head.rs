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
        let (name, port) = match named.rsplit_once(':') {
            Some((name, port)) if !name.ends_with(']') || name.starts_with('[') => {
                (name, port.parse().unwrap_or(80))
            }
            _ => (named.as_str(), 80),
        };
        assert_eq!(Site::of(name, port), site, "{head:?}");
    }
    // The site judged is one a site's spelling reads back to.
    assert_eq!(Site::parse(&site.to_string()).as_ref(), Ok(&site), "{site:?}");
});
