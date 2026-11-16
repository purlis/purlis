// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-056: the outbound proxy answers `407 Proxy Authentication Required`. The harness gets a 502
//! whose `purlis:` message names the proxy's refusal, and the proxy saw no
//! `Proxy-Authorization`: purlis sends no proxy credential it wasn't given.

use purlis_key_proxy::{Matcher, Reach};

use super::StartPath;
use super::outbound_proxy::{Answers, OutboundProxy};
use super::start_path::{Desktop, message, reach, route_to_upstream, send, tls_provider};

pub(crate) async fn check<P: StartPath>(start: impl AsyncFn(Reach) -> P) {
    let (provider, root) = tls_provider();
    let proxy = OutboundProxy::start(Answers::AsksForCredentials);

    let proxied = Matcher::builder().https(proxy.url()).build();
    let path = start(reach(provider.address(), Some(&root), proxied)).await;
    let (port, token) = path.start("run-1", route_to_upstream()).await;
    let refused = send(port, Some(&token), "/v1/messages", b"{}").await;
    assert_eq!(refused.status, 502, "{refused:?}");
    assert_eq!(
        message(&refused),
        "purlis: The outbound proxy refused the connection."
    );
    assert!(
        refused.headers.get("x-should-retry").is_none(),
        "a 502 told the harness never to retry: {:?}",
        refused.headers
    );

    let heads = proxy.heads();
    assert_eq!(heads.len(), 1, "{heads:?}");
    assert!(
        !heads[0]
            .to_ascii_lowercase()
            .contains("proxy-authorization"),
        "a proxy credential was sent: {heads:?}"
    );
    assert!(provider.requests().is_empty());
}

#[tokio::test]
async fn a_proxy_asking_for_credentials_gives_a_502() {
    check(Desktop::reaching).await;
}
