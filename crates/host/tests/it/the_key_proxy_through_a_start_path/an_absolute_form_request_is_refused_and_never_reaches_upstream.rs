// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-084 (from T1a-60's review): an absolute-form request target is refused, never passed
//! through. An upstream follows a request's absolute URI over its `Host` (RFC 9112 §3.2.2), so
//! one carried upstream would choose its own destination whatever `Host` core set. Each target
//! here names Messages' one listed path and carries the run's token, so only its form refuses it.

use super::StartPath;
use super::start_path::{provider, raw, route_to};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (port, token) = path.start("run-1", route_to(&provider)).await;
    let elsewhere = format!("http://127.0.0.1:{}/v1/messages?beta=true", provider.port());
    for target in [
        elsewhere.as_str(),
        "http://api.anthropic.com/v1/messages",
        "https://api.anthropic.com/v1/messages?beta=true",
        "http://127.0.0.1:1/v1/messages",
    ] {
        let answered = raw(port, "POST", target, Some(&token)).await;
        assert_eq!(answered.status, 400, "{target}: {answered:?}");
    }
    assert!(provider.requests().is_empty(), "{:?}", provider.requests());
    assert_eq!(
        provider.accepts(),
        0,
        "an absolute-form request opened a connection upstream"
    );
}

#[tokio::test]
async fn an_absolute_form_request_is_refused_and_never_reaches_upstream() {
    check(super::start_path::Desktop::new().await).await;
}
