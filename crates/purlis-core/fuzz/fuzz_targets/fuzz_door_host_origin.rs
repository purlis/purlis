// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The door's `Host` and `Origin`** (T1.1-47, `sandbox/door.rs`): any head the person's
//! browser, the chat's watched browser or a page of another site sends a chat's front door. A
//! head the door carries named this door's own `Host`, and any `Origin` it sent was this door's
//! own; what the door sends on is one head, with no cookie of the browser's in it, and reads
//! the same again.

#![no_main]

use libfuzzer_sys::fuzz_target;
use purlis_core::sandbox::fuzzing::door_judge;

/// The values of every `name` header in `head`.
fn named<'a>(head: &'a str, name: &str) -> Vec<&'a str> {
    head.trim_end()
        .split("\r\n")
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .filter(|(it, _)| it.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim_matches([' ', '\t']))
        .collect()
}

fuzz_target!(|data: &[u8]| {
    let Some((port, head)) = data.split_first_chunk::<2>() else {
        return;
    };
    let port = u16::from_be_bytes(*port).max(1);
    let Ok(head) = std::str::from_utf8(head) else {
        return;
    };
    let Ok(carried) = door_judge(head, port) else {
        return;
    };
    let own = [
        format!("localhost:{port}"),
        format!("127.0.0.1:{port}"),
        format!("[::1]:{port}"),
    ];
    let hosts = named(head, "host");
    assert!(
        hosts.len() == 1 && own.contains(&hosts[0].to_owned()),
        "{hosts:?}"
    );
    for origin in named(head, "origin") {
        assert!(
            own.iter().any(|it| origin == format!("http://{it}")),
            "{origin:?}"
        );
    }
    let sent = &carried.head;
    assert_eq!(sent.find("\r\n\r\n"), Some(sent.len() - 4), "{sent:?}");
    assert!(named(sent, "cookie").is_empty(), "{sent:?}");
    assert_eq!(named(sent, "host"), hosts);
    assert_eq!(named(sent, "connection").len(), 1);
    // What it sends on, it would carry again as it is.
    let again = door_judge(sent, port).expect("the door's own head is one it carries");
    assert_eq!(&again.head, sent);
    assert_eq!(again.body, carried.body);
});
