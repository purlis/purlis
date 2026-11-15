// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-100: core sets `Host`. A harness-sent `Host: evil.example` arrives at the fake provider as
//! the upstream origin's.

use purlis_key_proxy::Reach;

use super::StartPath;
use super::start_path::{Desktop, headers_named, plain_reach, provider, route_to, send_with};

pub(crate) async fn check<P: StartPath>(start: impl AsyncFn(Reach) -> P) {
    let path = start(plain_reach()).await;
    let provider = provider();
    let (port, token) = path.start("run-1", route_to(&provider)).await;

    let served = send_with(
        port,
        Some(&token),
        "/v1/messages",
        b"{}",
        &[("host", "evil.example")],
    )
    .await;
    assert_eq!(served.status, 200, "{served:?}");

    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    let origin = format!("127.0.0.1:{}", provider.port());
    assert_eq!(
        headers_named(&requests[0], "host"),
        [origin.as_str()],
        "the harness's Host reached the provider"
    );
}

#[tokio::test]
async fn core_sets_host_to_the_upstream_origin() {
    check(Desktop::reaching).await;
}
