// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-089, R-712: Claude Code's recorded startup, replayed against the key proxy. `HEAD
//! /api/hello` gets a 404 with no body and goes nowhere; the turn reaches the fake provider with
//! the key, and is answered: the chat starts and one turn completes.

use super::StartPath;
use super::start_path::{MESSAGE, STAND_IN_KEY, header, provider, route_to};
use super::startup::{PLAIN, replay};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (port, token) = path.start("run-1", route_to(&provider)).await;
    let answered = replay(port, &token, PLAIN).await;

    let hello = &answered[0];
    assert_eq!(hello.status, 404, "{hello:?}");
    assert!(hello.body.is_empty(), "the 404 has a body: {hello:?}");
    let turn = &answered[1];
    assert_eq!(turn.status, 200, "the turn did not complete: {turn:?}");
    assert_eq!(turn.body, MESSAGE.as_bytes());

    let requests = provider.requests();
    assert_eq!(requests.len(), 1, "only the turn goes upstream: {requests:?}");
    assert_eq!(requests[0].path, "/v1/messages?beta=true");
    assert_eq!(header(&requests[0], "x-api-key"), Some(STAND_IN_KEY));
}

#[tokio::test]
async fn claude_code_startup_calls_are_answered_as_the_spike_recorded() {
    check(super::start_path::Desktop::new().await).await;
}
