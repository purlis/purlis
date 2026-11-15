// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-056: the upstream is reached over TLS verified by `rustls-platform-verifier`. With the fake
//! provider's test root in the trust-store seam the call goes through; with the OS's own roots
//! alone its certificate, signed by a root nothing trusts, fails with a 502 whose `purlis:`
//! message names the certificate.

use purlis_key_proxy::{Matcher, Reach};

use super::StartPath;
use super::start_path::{
    Desktop, STAND_IN_KEY, UPSTREAM, headers_named, message, reach, route_to_upstream, send,
    tls_provider,
};

pub(crate) async fn check<P: StartPath>(start: impl AsyncFn(Reach) -> P) {
    let (provider, root) = tls_provider();

    let trusted = start(reach(
        provider.address(),
        Some(&root),
        Matcher::builder().build(),
    ))
    .await;
    let (port, token) = trusted.start("run-1", route_to_upstream()).await;
    let served = send(port, Some(&token), "/v1/messages", b"{}").await;
    assert_eq!(served.status, 200, "{served:?}");
    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(headers_named(&requests[0], "x-api-key"), [STAND_IN_KEY]);
    assert_eq!(headers_named(&requests[0], "host"), [UPSTREAM]);

    let untrusted = start(reach(provider.address(), None, Matcher::builder().build())).await;
    let (port, token) = untrusted.start("run-2", route_to_upstream()).await;
    let refused = send(port, Some(&token), "/v1/messages", b"{}").await;
    assert_eq!(refused.status, 502, "{refused:?}");
    assert_eq!(
        message(&refused),
        format!("purlis: {UPSTREAM}'s certificate isn't trusted.")
    );
    assert_eq!(
        provider.requests().len(),
        1,
        "a request went over a connection whose certificate wasn't trusted"
    );
}

#[tokio::test]
async fn upstream_tls_uses_the_platform_verifier() {
    check(Desktop::reaching).await;
}
