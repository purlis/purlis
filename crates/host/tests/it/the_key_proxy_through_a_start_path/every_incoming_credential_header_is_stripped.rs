// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-097: every credential header the harness sets is absent at the fake provider, and only the
//! declaration's own, `x-api-key`, arrives, carrying the stand-in key.

use super::StartPath;
use super::start_path::{
    Desktop, STAND_IN_KEY, headers_named, plain_reach, provider, route_to, send_with,
};
use purlis_key_proxy::Reach;

pub(crate) async fn check<P: StartPath>(start: impl AsyncFn(Reach) -> P) {
    let path = start(plain_reach()).await;
    let provider = provider();
    let (port, token) = path.start("run-1", route_to(&provider)).await;

    let served = send_with(
        port,
        Some(&token),
        "/v1/messages",
        b"{}",
        &[
            ("proxy-authorization", "Basic aGFybmVzczpvd24="),
            ("x-api-key", "the-harness-s-own"),
            ("api-key", "the-harness-s-own"),
            ("x-goog-api-key", "the-harness-s-own"),
            ("cookie", "session=the-harness-s-own"),
        ],
    )
    .await;
    assert_eq!(served.status, 200, "{served:?}");

    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    for name in [
        "authorization",
        "proxy-authorization",
        "api-key",
        "x-goog-api-key",
        "cookie",
    ] {
        assert!(
            headers_named(&requests[0], name).is_empty(),
            "{name} reached the provider: {:?}",
            requests[0].headers
        );
    }
    assert_eq!(
        headers_named(&requests[0], "x-api-key"),
        [STAND_IN_KEY],
        "the declaration's header carries the key, and only the key"
    );
}

#[tokio::test]
async fn every_incoming_credential_header_is_stripped() {
    check(Desktop::reaching).await;
}
