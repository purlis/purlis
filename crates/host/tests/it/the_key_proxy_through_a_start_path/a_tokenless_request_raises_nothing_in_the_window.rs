// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-745: a request without the run's token is refused and the window shows nothing for it: no
//! Ask, Notice or Update is pushed to it, and nothing of the kind is written in the event log.

use super::StartPath;
use super::start_path::{Desktop, provider, route_to, send};

/// Whether `raised`, one line of [`StartPath::raised`], is an Ask, a Notice or an Update.
fn asks_or_tells(raised: &str) -> bool {
    let (what, _) = raised.split_once(':').unwrap_or((raised, ""));
    [
        "Ask", "ask.", "Notice", "notice", "Update", "update", "stop.",
    ]
    .iter()
    .any(|kind| what.contains(kind))
}

pub(crate) async fn check(mut path: impl StartPath) {
    let provider = provider();
    let (port, _) = path.start("run-1", route_to(&provider)).await;
    let _before = path.raised().await;

    let refused = send(port, None, "/v1/messages", b"{}").await;
    assert_eq!(
        refused.status, 400,
        "a request without the token was served"
    );

    let raised: Vec<String> = path
        .raised()
        .await
        .into_iter()
        .filter(|raised| asks_or_tells(raised))
        .collect();
    assert!(
        raised.is_empty(),
        "a request without the token raised something in the window: {raised:?}"
    );
}

#[tokio::test]
async fn a_tokenless_request_raises_nothing_in_the_window() {
    check(Desktop::new().await).await;
}
