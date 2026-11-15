// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A `CONNECT`'s authority** (T1.1-47): what a tunnel names is the host and port it is judged
//! and dialled by, and every byte of the authority is accounted for: nothing a client wrote is
//! dropped on the way to the decision.

#![no_main]

use libfuzzer_sys::fuzz_target;
use purlis_core::sandbox::fuzzing::connect_authority;

fuzz_target!(|authority: &str| {
    let Some((host, port)) = connect_authority(authority) else {
        return;
    };
    let named = match host.parse::<std::net::Ipv6Addr>() {
        Ok(_) => format!("[{host}]"),
        Err(_) => host.clone(),
    };
    let (written, written_port) = authority
        .rsplit_once(':')
        .expect("a CONNECT read has a port");
    assert_eq!(written, named, "{authority:?}");
    assert_eq!(written_port.parse::<u16>().ok(), Some(port), "{authority:?}");
    assert!(!host.contains(['@', '/', '?', '#', '\\', '%', '[', ']']), "{host:?}");
    if host.parse::<std::net::Ipv6Addr>().is_err() {
        assert!(!host.contains(':'), "{host:?}");
    }
});
