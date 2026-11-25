// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7589: whatever panics inside a request's handler ends that request alone. The connection
//! gives its slot back, so panics never add up to a listener that refuses every chat's `gh`.
//!
//! These bind sockets, which a sandbox may not: they run in CI.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

use purlis_core::forge::proxy::RequestHead;
use purlis_core::forge::proxy::listener::{ForgeListener, Observe};

use crate::support::forge_listener::{Answer, World, gh_get, parse_reply};

/// Panics on a request for `/boom`, as a handler's bug would.
struct Boom;

impl Observe for Boom {
    fn request(&self, head: &RequestHead, _: &[u8]) {
        assert!(!head.target.ends_with("/boom"), "a handler's bug");
    }
}

fn ask(path: &std::path::Path, request: &[u8]) -> Vec<u8> {
    let mut client = UnixStream::connect(path).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    client.write_all(request).unwrap();
    let mut raw = Vec::new();
    let _ = client.read_to_end(&mut raw);
    raw
}

#[test]
fn a_panic_in_a_request_handler_ends_that_request_and_gives_its_slot_back() {
    purlis_core::unsteered!();
    let mut w = World::new();
    w.wiring.observe = Some(Arc::new(Boom));
    w.wiring.limits.connections = 2;
    let dir = tempfile::Builder::new()
        .prefix("fp")
        .tempdir_in(std::env::temp_dir())
        .unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let listener = ForgeListener::serve_socket(w.wiring.clone(), dir.path()).unwrap();
    let purlis_core::forge::proxy::EndpointKind::UnixSocket(path) = listener.kind().clone() else {
        panic!("a socket");
    };

    // More panics than the listener has slots.
    for _ in 0..4 {
        let _ = ask(&path, &gh_get(&w, "/boom"));
    }

    w.fake.answer(Answer::ok(r#"{"login":"octocat"}"#));
    let reply = parse_reply(&ask(&path, &gh_get(&w, "/user")));
    assert_eq!(reply.status, 200, "{}", reply.text());
}
