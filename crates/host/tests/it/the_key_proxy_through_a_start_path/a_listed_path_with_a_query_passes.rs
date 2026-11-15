// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-089: the allowlist matches a listed path with a query: Claude Code's
//! `POST /v1/messages?beta=true` reaches the fake provider, query and all, with the key.

use super::StartPath;
use super::start_path::{MESSAGE, STAND_IN_KEY, header, provider, route_to, send};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (port, token) = path.start("run-1", route_to(&provider)).await;
    let answered = send(port, Some(&token), "/v1/messages?beta=true", b"{}").await;
    assert_eq!(answered.status, 200, "{answered:?}");
    assert_eq!(answered.body, MESSAGE.as_bytes());
    let requests = provider.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/messages?beta=true");
    assert_eq!(header(&requests[0], "x-api-key"), Some(STAND_IN_KEY));
}

#[tokio::test]
async fn a_listed_path_with_a_query_passes() {
    check(super::start_path::Desktop::new().await).await;
}
