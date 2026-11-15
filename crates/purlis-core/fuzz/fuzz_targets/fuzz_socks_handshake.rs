// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The SOCKS5 handshake** (T1.1-47): any bytes a client sends the proxy's SOCKS port. A
//! request is read only after a greeting that offers no authentication; nothing the client sent
//! past its request is read; and a name it asks for is one a URL's host could be.

#![no_main]

use libfuzzer_sys::fuzz_target;
use purlis_core::sandbox::fuzzing::socks_handshake;

fuzz_target!(|bytes: &[u8]| {
    let read = socks_handshake(bytes);
    assert!(read.read <= bytes.len());
    assert!(read.answered.len() <= 2, "{:?}", read.answered);
    let offered = bytes.len() >= 2
        && bytes[0] == 5
        && bytes[1] > 0
        && bytes
            .get(2..2 + usize::from(bytes[1]))
            .is_some_and(|methods| methods.contains(&0));
    match &read.request {
        Some(Ok((host, port))) => {
            assert!(offered);
            assert_eq!(read.answered, [5, 0]);
            let at = 2 + usize::from(bytes[1]);
            // The request: version, CONNECT, reserved, then the address.
            assert_eq!((bytes[at], bytes[at + 1]), (5, 1));
            assert_eq!(&bytes[read.read - 2..read.read], &port.to_be_bytes());
            let ok = host.parse::<std::net::IpAddr>().is_ok()
                || host
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-._".contains(&b));
            assert!(ok && !host.is_empty(), "{host:?}");
        }
        Some(Err(code)) => {
            assert!(offered);
            assert!([2, 4, 7, 8].contains(code), "{code}");
        }
        None if !offered && bytes.len() >= 2 + usize::from(bytes[1]) && bytes[0] == 5 && bytes[1] > 0 => {
            // A greeting that offers no method served is answered "no method", and nothing more.
            assert_eq!(read.answered, [5, 0xff]);
        }
        None => {}
    }
});
