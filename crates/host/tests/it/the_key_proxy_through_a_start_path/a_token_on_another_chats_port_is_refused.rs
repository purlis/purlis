// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-063: one listener per chat, and a request must match both its port and its run's token.
//! Two chats get two ports; chat A's token on chat B's port is refused, and B's own passes.

use super::StartPath;
use super::start_path::{Desktop, provider, route_to, send};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (a_port, a_token) = path.start("chat-a", route_to(&provider)).await;
    let (b_port, b_token) = path.start("chat-b", route_to(&provider)).await;
    assert_ne!(a_port, b_port, "two chats got one port");
    assert_ne!(a_token, b_token, "two chats got one token");

    let refused = send(b_port, Some(&a_token), "/v1/messages", b"{}").await;
    assert_eq!(
        refused.status, 400,
        "chat A's token was served on chat B's port"
    );
    assert!(
        provider.requests().is_empty(),
        "chat A's token on chat B's port reached the provider"
    );

    let served = send(b_port, Some(&b_token), "/v1/messages", b"{}").await;
    assert_eq!(
        served.status, 200,
        "chat B's own token was refused: {served:?}"
    );
    assert_eq!(provider.requests().len(), 1);
}

#[tokio::test]
async fn a_token_on_another_chats_port_is_refused() {
    check(Desktop::new().await).await;
}
