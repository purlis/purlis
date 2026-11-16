// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-065: the host's copy of a run's token lives in memory only and dies with the run: after
//! the stop, the old token is refused on the run's new listener, and no file the host wrote,
//! nor its event log, nor any line the key proxy told it, carries a token.

use super::StartPath;
use super::start_path::{Desktop, provider, route_to, send, try_send};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (old_port, old_token) = path.start("run-1", route_to(&provider)).await;
    let served = send(old_port, Some(&old_token), "/v1/messages", b"{}").await;
    assert_eq!(
        served.status, 200,
        "the token was refused while its run ran: {served:?}"
    );
    path.stop("run-1").await;

    if let Ok(answered) = try_send(old_port, Some(&old_token), "/v1/messages", b"{}").await {
        assert_eq!(
            answered.status, 400,
            "the stopped run's port served its token"
        );
    }

    let (port, token) = path.start("run-1", route_to(&provider)).await;
    assert_ne!(token, old_token, "the run's next start got its old token");
    let refused = send(port, Some(&old_token), "/v1/messages", b"{}").await;
    assert_eq!(
        refused.status, 400,
        "a stopped run's token was served on its new listener"
    );
    let served = send(port, Some(&token), "/v1/messages", b"{}").await;
    assert_eq!(served.status, 200, "the new token was refused: {served:?}");
    assert_eq!(
        provider.requests().len(),
        2,
        "only the two served requests reach the provider"
    );
    path.stop("run-1").await;

    for line in path.written() {
        assert!(
            !line.contains(&old_token) && !line.contains(&token),
            "the host wrote a token: {line}"
        );
    }
}

#[tokio::test]
async fn the_token_dies_with_the_run() {
    check(Desktop::new().await).await;
}
