// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-087: core's Messages allowlist refuses every other method and path before the key is
//! swapped in, the run's token carried or not: the outbound seam (the fake provider) sees no
//! connection, so no request and no key.

use super::StartPath;
use super::start_path::{provider, raw, route_to};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (port, token) = path.start("run-1", route_to(&provider)).await;
    for (method, target) in [
        ("GET", "/v1/messages"),
        ("POST", "/v1/files"),
        ("DELETE", "/v1/messages"),
        ("OPTIONS", "*"),
        ("POST", "/v1/messages/batches"),
        ("POST", "http://elsewhere.example/v1/messages"),
        ("CONNECT", "elsewhere.example:443"),
        ("POST", "/v1/../v1/messages"),
        ("POST", "/v1/%2e%2e/v1/messages"),
        ("POST", "/v1%2Fmessages"),
    ] {
        let answered = raw(port, method, target, Some(&token)).await;
        assert_eq!(answered.status, 400, "{method} {target}: {answered:?}");
    }
    assert!(
        provider.requests().is_empty(),
        "a refused request reached upstream: {:?}",
        provider.requests()
    );
    assert_eq!(
        provider.accepts(),
        0,
        "a refused request opened a connection upstream"
    );
}

#[tokio::test]
async fn a_disallowed_messages_path_is_refused_before_the_key_is_swapped_in() {
    check(super::start_path::Desktop::new().await).await;
}
