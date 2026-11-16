// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-089, spec C-19: `POST /v1/messages/count_tokens` is not on Messages' allowlist. It gets the
//! key proxy's 400 with `x-should-retry: false` (R-117's; its final shape is T1a-26's), and the
//! outbound seam sees nothing.

use super::StartPath;
use super::start_path::{provider, route_to, send};
use super::startup::TURN_BODY;

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (port, token) = path.start("run-1", route_to(&provider)).await;
    for target in [
        "/v1/messages/count_tokens",
        "/v1/messages/count_tokens?beta=true",
    ] {
        let answered = send(port, Some(&token), target, TURN_BODY).await;
        assert_eq!(answered.status, 400, "{target}: {answered:?}");
        assert_eq!(answered.headers["x-should-retry"], "false", "{target}");
    }
    assert!(provider.requests().is_empty(), "{:?}", provider.requests());
    assert_eq!(provider.accepts(), 0);
}

#[tokio::test]
async fn count_tokens_is_refused_with_no_credential_upstream() {
    check(super::start_path::Desktop::new().await).await;
}
