// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-712: with `HTTPS_PROXY` set and `NO_PROXY` carrying `127.0.0.1`, Claude Code's recorded
//! startup sends no `HEAD /api/hello`; the chat starts on its first turn and the turn completes.

use super::StartPath;
use super::start_path::{MESSAGE, provider, route_to};
use super::startup::{THROUGH_A_PROXY, replay};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (port, token) = path.start("run-1", route_to(&provider)).await;
    assert!(
        THROUGH_A_PROXY.iter().all(|call| call.target != "/api/hello"),
        "the recording through a proxy holds no hello"
    );
    let answered = replay(port, &token, THROUGH_A_PROXY).await;
    assert_eq!(answered.len(), 1);
    assert_eq!(answered[0].status, 200, "{:?}", answered[0]);
    assert_eq!(answered[0].body, MESSAGE.as_bytes());
    let requests = provider.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0].path, "/v1/messages?beta=true");
    assert_eq!(path.arrivals("run-1"), 1, "the turn alone arrived");
}

#[tokio::test]
async fn the_startup_with_an_http_proxy_configured_skips_api_hello_and_still_starts() {
    check(super::start_path::Desktop::new().await).await;
}
