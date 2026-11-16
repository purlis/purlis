// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The SOCKS5 handshake** (T1.1-47): any bytes a client sends the proxy's SOCKS port. A
//! request is read only after a greeting that offers no authentication; nothing the client sent
//! past its request is read; and a name it asks for is one a URL's host could be.

#![no_main]

use libfuzzer_sys::fuzz_target;
use purlis_core::sandbox::fuzzing::socks_handshake;

/// Whether `host` is a name a URL's host could be: labels of letters, digits, `-` and `_`, none
/// empty, with one final dot at most, whose last label is not a number. One that is (all digits,
/// or `0x` and hex digits, in either case: WHATWG's "ends in a number") is an IPv4 address in
/// another notation to curl, Node and the resolver (`0x7f.1`, `2130706433`).
fn a_name(host: &str) -> bool {
    let name = host.strip_suffix('.').unwrap_or(host);
    let last = name.rsplit('.').next().unwrap_or(name).to_ascii_lowercase();
    let a_number = (!last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()))
        || last
            .strip_prefix("0x")
            .is_some_and(|hex| hex.bytes().all(|b| b.is_ascii_hexdigit()));
    !a_number
        && name.split('.').all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        })
}

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
            assert!(
                host.parse::<std::net::IpAddr>().is_ok() || a_name(host),
                "{host:?}"
            );
        }
        Some(Err(code)) => {
            assert!(offered);
            assert!([2, 4, 7, 8].contains(code), "{code}");
        }
        None if !offered
            && bytes.len() >= 2
            && bytes[0] == 5
            && bytes[1] > 0
            && bytes.len() >= 2 + usize::from(bytes[1]) =>
        {
            // A greeting that offers no method served is answered "no method", and nothing more.
            assert_eq!(read.answered, [5, 0xff]);
        }
        None => {}
    }
});
