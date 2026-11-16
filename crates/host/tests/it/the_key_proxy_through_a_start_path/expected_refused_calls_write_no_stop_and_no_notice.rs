// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-089, spec C-3 and C-13: the three calls Claude Code is expected to make and the key proxy
//! refuses by design (`HEAD /api/hello`, `count_tokens`, and discovery's `GET /v1/models`) are
//! each answered, and none writes a stop in the event log or raises a Notice or an Ask.

use super::StartPath;
use super::start_path::{provider, raw, route_to, send};
use super::startup::TURN_BODY;

/// Whether `raised`, one line of [`StartPath::raised`], is a stop, an Ask, a Notice or an Update.
fn stops_or_tells(raised: &str) -> bool {
    let (what, _) = raised.split_once(':').unwrap_or((raised, ""));
    [
        "Ask", "ask.", "Notice", "notice", "Update", "update", "stop", "Stop",
    ]
    .iter()
    .any(|kind| what.contains(kind))
}

pub(crate) async fn check(mut path: impl StartPath) {
    let provider = provider();
    let (port, token) = path.start("run-1", route_to(&provider)).await;
    let _before = path.raised().await;

    assert_eq!(raw(port, "HEAD", "/api/hello", None).await.status, 404);
    assert_eq!(
        send(port, Some(&token), "/v1/messages/count_tokens", TURN_BODY)
            .await
            .status,
        400
    );
    assert_eq!(
        raw(port, "GET", "/v1/models?limit=1000", Some(&token))
            .await
            .status,
        400
    );

    let raised: Vec<String> = path
        .raised()
        .await
        .into_iter()
        .filter(|raised| stops_or_tells(raised))
        .collect();
    assert!(
        raised.is_empty(),
        "an expected refusal wrote a stop or raised something: {raised:?}"
    );
    assert_eq!(provider.accepts(), 0);
}

#[tokio::test]
async fn expected_refused_calls_write_no_stop_and_no_notice() {
    check(super::start_path::Desktop::new().await).await;
}
