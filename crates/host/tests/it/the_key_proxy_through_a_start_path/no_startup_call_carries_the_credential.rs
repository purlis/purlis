// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-712: no startup call carries the credential upstream, whichever way it is answered: `HEAD
//! /api/hello` and discovery's `GET /v1/models` are answered by the key proxy and reach nothing,
//! so the outbound seam sees an `x-api-key` on the turn alone.

use super::StartPath;
use super::start_path::{STAND_IN_KEY, header, provider, route_to};
use super::startup::{DISCOVERY, HELLO, TURN, replay_one};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (port, token) = path.start("run-1", route_to(&provider)).await;
    for call in [HELLO, DISCOVERY] {
        let answered = replay_one(port, &token, call).await;
        assert!(
            answered.status == 404 || answered.status == 400,
            "{} {} was not refused: {answered:?}",
            call.method,
            call.target
        );
        assert!(
            provider.requests().is_empty(),
            "{} {} reached upstream: {:?}",
            call.method,
            call.target,
            provider.requests()
        );
    }
    assert_eq!(replay_one(port, &token, TURN).await.status, 200);
    let keyed: Vec<_> = provider
        .requests()
        .into_iter()
        .filter(|request| header(request, "x-api-key").is_some())
        .map(|request| (request.method, request.path))
        .collect();
    assert_eq!(
        keyed,
        [("POST".to_owned(), "/v1/messages?beta=true".to_owned())],
        "the key went upstream on something but the turn"
    );
    assert_eq!(
        header(&provider.requests()[0], "x-api-key"),
        Some(STAND_IN_KEY)
    );
}

#[tokio::test]
async fn no_startup_call_carries_the_credential() {
    check(super::start_path::Desktop::new().await).await;
}
